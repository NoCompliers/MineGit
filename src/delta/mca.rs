use std::{collections::BinaryHeap, io::{self, Cursor, Read, Seek, Write}};
use libdeflater::{CompressionLvl, Compressor};
use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};

use crate::delta::{
    diff::{
        Copy, CopyZipHeader, DiffCommandHeader, InsertZipHeader
    }, diff_gen::DiffGenerator, recover::{Instruction, CHUNK_VIRTUAL_SPACE}, region::RegionFactory, snapshot::{SnapshotHeader, SNAPSHOT_HEADER_SIZE}
};

pub const HEADER_FIELDS_CNT: usize = 1024;
pub const HEADER_SIZE: usize = HEADER_FIELDS_CNT * 4 * 2;
pub const SECTOR_SIZE: u64 = 4096;

pub fn read_u32(data: &[u8], idx: usize) -> u32 {
    return u32::from_be_bytes(data[idx..idx+4].try_into().unwrap());
}

pub fn write_u32(t: &mut[u8], pos: usize, val: u32) {
    t[pos..pos+4].copy_from_slice(&val.to_be_bytes());
}

use super::region::zlib_decompress;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ChunkHeader {
    pub utime: u32,
    pub l_idx: u32,
    pub size: u32,
    pub idx: u16
}
impl ChunkHeader {
    pub const SERIZALIZED_SIZE: usize = 10;

    pub(crate) fn new(header: &[u8; HEADER_SIZE]) -> Vec<ChunkHeader> {
        let mut chunks: Vec<ChunkHeader> = Vec::new();
        for i in 0..HEADER_FIELDS_CNT {
            let offset = read_u32(header, i*4);
            if offset >> 8 == 0 || offset & 0xFF == 0 { continue; }
            let timestamp = read_u32(header, SECTOR_SIZE as usize + i*4);
            chunks.push(ChunkHeader {utime: timestamp, idx: i as u16, l_idx: u32::MAX, size: u32::MAX});
        }
        chunks
    }

    pub(crate) fn serialize_all<W: Write>(header: &Vec<ChunkHeader>, f: &mut W) -> io::Result<()> {
        f.write_u32::<BigEndian>(header.len() as u32)?;
        for chunk in header {
            f.write_u32::<BigEndian>(chunk.utime)?;
            f.write_u32::<BigEndian>(chunk.l_idx)?;
            f.write_u16::<BigEndian>(chunk.idx)?;
        }
        if let Some(c) = header.last() {
            f.write_u32::<BigEndian>(c.size)?;
        } else {
            f.write_u32::<BigEndian>(0)?;
        }
        Ok(())
    }

    pub(crate) fn deserialize_all<R: Read>(f: &mut R) -> io::Result<Vec<ChunkHeader>> {
        let len = f.read_u32::<BigEndian>()?;
        let mut v = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let utime = f.read_u32::<BigEndian>()?;
            let l_idx = f.read_u32::<BigEndian>()?;
            let idx = f.read_u16::<BigEndian>()?;
            v.push(ChunkHeader { utime, idx, l_idx, size: u32::MAX });
        }
        let last_size = f.read_u32::<BigEndian>()?;
        for i in 0..(len-1) as usize {
            v[i].size = v[i+1].l_idx - v[i].l_idx;
        }
        if let Some(l) = v.last_mut() {
            l.size = last_size;
        }
        Ok(v)
    }

    pub(crate) fn skip_data<R: Read + Seek>(f: &mut R) -> io::Result<usize> {
        let len = f.read_u32::<BigEndian>()? as i64 * Self::SERIZALIZED_SIZE as i64 + 4;
        f.seek(io::SeekFrom::Current(len))?;
        Ok(len as usize + 4)
    }

    pub(crate) fn get_serialized_size(len: usize) -> usize {
        len * Self::SERIZALIZED_SIZE + 8
    }

    fn get_chunk_data(header: &[u8; HEADER_SIZE], idx: usize) -> (u32, u32) {
        debug_assert!(idx < HEADER_FIELDS_CNT);
        return (
            ( read_u32(header, idx*4) >> 8 ) * SECTOR_SIZE as u32,
            read_u32(header, SECTOR_SIZE as usize + idx*4)
        )
    }
}

impl Default for ChunkHeader {
    fn default() -> Self {
        Self {
            utime: 0,
            l_idx: u32::MAX,
            size: u32::MAX,
            idx: u16::MAX
        }
    }
}

struct ChunkDescriptor {
    idx: usize,
    len: usize,
}

pub(crate) struct MCA {}
impl MCA {
    fn get_diff_size(payload_len: usize, chunk_data_size: usize, header_cnt: usize) -> usize{
        payload_len - (chunk_data_size + ChunkHeader::get_serialized_size(header_cnt))
    }

    /* assumes that chunk is the last data in buf, inserts the data in the header about update time and position, also adds buffer memory to the end */
    fn update_header_and_normalize_size(buf: &mut Vec<u8>, header: &Vec<ChunkHeader>, pos: usize, len: usize, idx: usize) {
        let segment_cnt = len.div_ceil(SECTOR_SIZE as usize);
        let segment_pos = pos as u32 / SECTOR_SIZE as u32;
        buf.resize(pos + segment_cnt*SECTOR_SIZE as usize, 0);
        
        write_u32(buf, idx*4, (segment_pos << 8) | segment_cnt as u32);
        write_u32(buf, SECTOR_SIZE as usize + idx*4, header[idx].utime);
    }

    pub fn save<R: Read + Seek + Write, R1: Read>(snap: &SnapshotHeader, pack: &mut R, file: &mut R1) -> io::Result<SnapshotHeader> {
        use super::recover::_recover;
        struct ToCompare {
            sidx: usize,
            slen: usize,
            source_idx: usize,
            idx: usize
        }

        pack.seek(io::SeekFrom::Start(snap.pos))?;
        let mut chunk_data = vec![0u8; snap.chunk_data_size as usize];
        pack.read_exact(&mut chunk_data)?;
        let header = ChunkHeader::deserialize_all(pack)?;
        let mut diff_zipped = vec![0u8; MCA::get_diff_size(snap.payload_len as usize, snap.chunk_data_size as usize, header.len())];
        pack.read_exact(&mut diff_zipped)?;

        let mut is_present = vec![false; HEADER_FIELDS_CNT];

        let mut diff = Vec::new();
        zlib_decompress(&diff_zipped, &mut diff)?;

        let mut diff = Cursor::new(diff);
        let mut idx: u64 = 0;
        let mut zips: Vec<(CopyZipHeader, usize)> = Vec::new();
        while let Ok((c, _)) = DiffCommandHeader::deserialize(&mut diff) {
            match c {
                DiffCommandHeader::Copy(c) => idx += c.len,
                DiffCommandHeader::Insert(i) => idx += i.len,
                DiffCommandHeader::CopyZip(c) => {
                    zips.push((c, idx as usize));
                    idx += CHUNK_VIRTUAL_SPACE;
                },
                DiffCommandHeader::InsertZip(i) => {
                    zips.push(( CopyZipHeader { pos: snap.pos + i.pos as u64 }, idx as usize ));
                    idx += CHUNK_VIRTUAL_SPACE;
                }
            }
        }

        let mut file_data = Vec::new();
        file.read_to_end(&mut file_data)?;
        let header_new: &[u8; HEADER_SIZE] = &file_data[..HEADER_SIZE as usize].try_into().unwrap();

        let mut to_cmp: Vec<ToCompare> = Vec::new();
        let mut descriptors: Vec<ChunkHeader> = Vec::new();

        diff_zipped.clear();
        let mut out = Cursor::new(diff_zipped);
        let mut idx: usize = 0;
        let mut vidx: usize = 0;
        let mut to_recover: Vec<Instruction> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        for h in &header {
            while idx != zips.len() && zips[idx].1 + CHUNK_VIRTUAL_SPACE as usize <= h.l_idx as usize {
                idx += 1;
            }
            let is_zip = h.size == CHUNK_VIRTUAL_SPACE as u32 && idx != zips.len() && zips[idx].1 == h.l_idx as usize;
            let i = h.idx as usize;
            is_present[i] = true;
            let (offset, utime) = ChunkHeader::get_chunk_data(header_new, i);
            let offset = offset as usize;
            if utime == h.utime {
                if is_zip {
                    DiffCommandHeader::CopyZip(zips[idx].0).serialize(&mut out)?;
                    descriptors.push(ChunkHeader { l_idx: vidx as u32, size: CHUNK_VIRTUAL_SPACE as u32, utime: utime, idx: i as u16 });
                    vidx += CHUNK_VIRTUAL_SPACE as usize;
                } else {
                    DiffCommandHeader::Copy(
                        Copy {sidx: h.l_idx as u64, len: h.size as u64}
                    ).serialize(&mut out)?;
                    descriptors.push(ChunkHeader { l_idx: vidx as u32, size: h.size, utime: utime, idx: i as u16 });
                    vidx += h.size as usize;
                }
                continue;
            }
            
            if is_zip {
                let t_size = read_u32(&file_data, offset as usize) as usize;
                let t_data = &file_data[offset..offset + t_size+4];
                let mut s_data: Vec<u8> = Vec::new();

                let c = zips[idx].0;
                pack.seek(io::SeekFrom::Start(c.pos))?;
                let len = pack.read_u32::<BigEndian>()? as usize;
                s_data.resize(len+4, 0);
                write_u32(&mut s_data, 0, len as u32);
                pack.read_exact(&mut s_data[4..])?;

                if s_data == t_data {
                    DiffCommandHeader::CopyZip(
                        CopyZipHeader {pos: c.pos + snap.pos}
                    ).serialize(&mut out)?;
                    descriptors.push(ChunkHeader { utime: h.utime, l_idx: vidx as u32, size: CHUNK_VIRTUAL_SPACE as u32, idx: i as u16 });
                    vidx += CHUNK_VIRTUAL_SPACE as usize;
                    continue;
                }

                let mut uncompressed: Vec<u8> = Vec::new();
                RegionFactory::uncompress_chunk_data(&s_data[5..], s_data[4], &mut uncompressed)?;
                let pos = buf.len();
                buf.resize(pos + uncompressed.len(), 0);
                buf[pos..].copy_from_slice(&uncompressed);
                to_cmp.push(ToCompare { sidx: pos as usize, slen: uncompressed.len(), idx: i, source_idx: h.l_idx as usize });
                continue;
            }

            to_recover.push(Instruction { from: h.l_idx as u64, to: vidx as u64, len: h.size as u64 });
            to_cmp.push(ToCompare { sidx: buf.len(), slen: h.size as usize, idx: i, source_idx: h.l_idx as usize });
            buf.resize(buf.len() + h.size as usize, 0);
            vidx += h.size as usize;
        }

        _recover(pack, BinaryHeap::from(to_recover), snap.clone(), &mut buf)?;

        let mut unpacked: Vec<u8> = Vec::new();
        let mut diff_gen = DiffGenerator::new();
        for cmp in to_cmp {
            let (offset, utime) = ChunkHeader::get_chunk_data((&file_data[..HEADER_SIZE]).try_into().unwrap(), cmp.idx);
            let offset = offset as usize;
            let len = read_u32(&file_data, offset as usize) as usize;
            RegionFactory::uncompress_chunk_data(&file_data[offset+5..offset+5+len], file_data[offset+4], &mut unpacked)?;

            diff_gen.init(&buf[cmp.sidx..cmp.sidx+cmp.slen], &unpacked);
            diff_gen.generate(&mut out, cmp.sidx as u64)?;
            descriptors.push(ChunkHeader { utime: utime, l_idx: vidx as u32, size: unpacked.len() as u32, idx: cmp.idx as u16 });
            vidx += unpacked.len();
        }

        // inserting new chunks into file
        chunk_data.clear(); // chunk data is now used for chunk data of new .mca
        for i in 0..HEADER_FIELDS_CNT {
            let (offset, utime) = ChunkHeader::get_chunk_data(header_new, i);
            let offset = offset as usize;
            if offset == 0 || is_present[i] { continue; }
            let len = read_u32(&file_data, offset) as usize;
            let pos = chunk_data.len();
            chunk_data.resize(pos + len+4, 0);
            chunk_data[pos..].copy_from_slice(&file_data[offset..offset+len]);
            DiffCommandHeader::InsertZip(
                InsertZipHeader { pos: pos as u32 }
            ).serialize(&mut out)?;
            descriptors.push(ChunkHeader { utime, l_idx: vidx as u32, size: CHUNK_VIRTUAL_SPACE as u32, idx: i as u16 });
            vidx += CHUNK_VIRTUAL_SPACE as usize;
        }

        let mut comp = Compressor::new(CompressionLvl::fastest());
        let diff = out.into_inner();
        let mut diff_zip = vec![0u8; comp.zlib_compress_bound(diff.len())];
        let n = comp.zlib_compress(&diff, &mut diff_zip).unwrap();
        diff_zip.resize(n, 0);

        // storing the result
        pack.seek(io::SeekFrom::End(0))?;
        let snap_new = SnapshotHeader {
            depend_on: snap.pos - SNAPSHOT_HEADER_SIZE as u64,
            payload_len: ( diff_zip.len() + ChunkHeader::get_serialized_size(descriptors.len()) + chunk_data.len() ) as u64,
            file_len: u64::MAX,
            pos: pack.stream_position()? + SNAPSHOT_HEADER_SIZE as u64,
            chunk_data_size: chunk_data.len() as u32,
            is_zipped: true,
            is_mca_file: true
        };
        snap_new.serialize(pack)?;
        pack.write_all(&chunk_data)?;
        print!("{:?}\n", &descriptors[..70]);

        ChunkHeader::serialize_all(&descriptors, pack)?;
        print!("DiffZipStart: {}, size: {}\n", pack.stream_position()?, diff_zip.len());
        pack.write_all(&diff_zip)?;

        Ok(snap_new)
    }

    pub fn recover<R: Read + Seek>(snap: &SnapshotHeader, pack: &mut R) -> io::Result<Vec<u8>> {
        use crate::delta::recover::_recover;
        pack.seek(io::SeekFrom::Start(snap.pos + snap.chunk_data_size as u64))?;
        let header = ChunkHeader::deserialize_all(pack)?;

        let mut t_idx: u64 = 0;
        let mut idx: u64 = 0;
        let mut to_copy: Vec<u16> = Vec::new();
        let mut instructs: Vec<Instruction> = Vec::new();
        let mut chunk_desc: Vec<ChunkDescriptor> = Vec::new();
        for h in &header {
            if h.size as u64 == CHUNK_VIRTUAL_SPACE {
                to_copy.push(h.idx);
                t_idx += CHUNK_VIRTUAL_SPACE;
                continue;
            }
            match instructs.last_mut() {
                Some(ins) if h.l_idx as u64 == ins.to + ins.len => {
                    ins.len += h.size as u64;
                },
                _ => instructs.push(Instruction { from: t_idx, to: idx, len: h.size as u64 })
            };

            chunk_desc.push(ChunkDescriptor {
                idx: idx as usize, len: h.size as usize
            });
            idx += h.size as u64;
            t_idx += h.size as u64;
        }

        let mut unzipped = vec![0u8; idx as usize];
        _recover(pack, BinaryHeap::from(instructs), snap.clone(), &mut unzipped)?;

        let mut buf = vec![0u8; HEADER_SIZE];
        let mut comp = Compressor::new(CompressionLvl::fastest());

        for c in chunk_desc {
            let chunk = &unzipped[c.idx..c.idx+c.len];
            let max_size = comp.zlib_compress_bound(chunk.len());
            let pos = buf.len();
            buf.resize(pos+5 + max_size, 0);
            let n = comp.zlib_compress(chunk, &mut buf[pos+5..]).unwrap();
            let segments_cnt = (n+5).div_ceil(SECTOR_SIZE as usize);
            write_u32(&mut buf, pos, segments_cnt as u32);
            buf[pos+4] = 0; // hardcoded value for z-lib compression

            Self::update_header_and_normalize_size(&mut buf, &header, pos, n+5, c.idx);
        }        
        
        if to_copy.len() == 0 { 
            return Ok(buf); 
        }

        let mut diff_data_zipped = vec![0u8; snap.payload_len as usize - (ChunkHeader::get_serialized_size(header.len()) + snap.chunk_data_size as usize)];
        pack.seek(io::SeekFrom::Start(
            snap.pos + snap.chunk_data_size as u64
            + ChunkHeader::get_serialized_size(header.len()) as u64))?;
        pack.read_exact(&mut diff_data_zipped)?;

        let mut diff_data = unzipped; // moving buffer
        zlib_decompress(&diff_data_zipped, &mut diff_data)?;

        pack.seek(io::SeekFrom::Start(snap.pos))?;
        let mut chunk_data = diff_data_zipped; // moving buffer
        chunk_data.resize(snap.chunk_data_size as usize, 0);
        pack.read_exact(&mut chunk_data)?;
        
        let mut diff_data = Cursor::new(diff_data);

        let mut chunk_idx = 0;
        while let Ok((command, _)) = DiffCommandHeader::deserialize(&mut diff_data) {
            match command {
                DiffCommandHeader::Copy(_) | DiffCommandHeader::Insert(_) => {},
                DiffCommandHeader::CopyZip(c) => {
                    pack.seek(io::SeekFrom::Start(c.pos))?;
                    let len = pack.read_u32::<BigEndian>()? as usize;
                    let pos = buf.len();
                    buf.resize(buf.len() + len+5, 0);
                    write_u32(&mut buf, pos, len as u32); // ToDo! Check maybe len+1 is required
                    pack.read_exact(&mut buf[pos+4..])?;

                    let c = &header[to_copy[chunk_idx] as usize];
                    chunk_idx += 1;

                    Self::update_header_and_normalize_size(&mut buf, &header, pos, len, c.idx as usize);
                }, DiffCommandHeader::InsertZip(i) => {
                    let c = &header[to_copy[chunk_idx] as usize];
                    chunk_idx += 1;

                    let len = read_u32(&chunk_data, i.pos as usize) as usize + 5;
                    let pos = buf.len();
                    buf.resize(buf.len() + len, 0);
                    buf[pos..].clone_from_slice(&chunk_data[i.pos as usize..i.pos as usize + len]);

                    Self::update_header_and_normalize_size(&mut buf, &header, pos, len, c.idx as usize);
                }
            }
        }

        Ok(buf)
    }

    /* ToDo! figure out where chunk_data is wrote into the file */
    pub fn save_new<W: Write + Seek, R: Read + Seek>(f: &mut R, pack: &mut W) -> io::Result<SnapshotHeader> {
        pack.seek(io::SeekFrom::End(0))?;
        let mut buf: Vec<u8> = Vec::new();
        f.read_to_end(&mut buf)?;
        let file_len = buf.len();
        debug_assert!(buf.len() >= HEADER_SIZE, "Incorrect file format");

        let mut out = Cursor::new(Vec::with_capacity(buf.len()) as Vec<u8>);

        let _header: &[u8; HEADER_SIZE] = &buf[..HEADER_SIZE].try_into().unwrap();
        let mut header = ChunkHeader::new(_header);
        let mut chunk_zip_pos = vec![usize::MAX; header.len()];
        let mut last = 0;
        let mut idx = 0;

        for i in 0..header.len() {
            let chunk = &mut header[i];
            let (offset, _) = ChunkHeader::get_chunk_data(_header, chunk.idx as usize);
            let size = read_u32(&buf, offset as usize);
            out.write_all(&buf[offset as usize .. offset as usize + 5 + size as usize])?;
            chunk.size = CHUNK_VIRTUAL_SPACE as u32;
            chunk.l_idx = idx as u32;
            idx += CHUNK_VIRTUAL_SPACE as usize;
            chunk_zip_pos[i] = last;
            last += size as usize + 5; // changed
        }
        let chunk_data_size = out.position();
        ChunkHeader::serialize_all(&header, &mut out)?;

        buf.clear(); // buf is now used as storage for copy commands
        let mut diff_data = Cursor::new(buf);
        for i in 0..header.len() {
            DiffCommandHeader::InsertZip(
                InsertZipHeader { pos: chunk_zip_pos[i] as u32 }
            ).serialize(&mut diff_data)?;
        }

        let compression_level = CompressionLvl::fastest(); // ToDo! rewrite to zstd
        let mut comp = Compressor::new(compression_level);

        let pos = out.position() as usize;
        let mut buf = out.into_inner();
        let diff_data = diff_data.into_inner();
        let max_zlib_size = comp.zlib_compress_bound(diff_data.len());
        
        buf.resize(pos + max_zlib_size, 0);
        let wrote = comp.zlib_compress(&diff_data, &mut buf[pos..pos+max_zlib_size]).unwrap();
        buf.resize(pos + wrote, 0);

        let snap = SnapshotHeader {
            depend_on: u64::MAX,
            payload_len: buf.len() as u64,
            file_len: file_len as u64,
            pos: pack.stream_position()? + SNAPSHOT_HEADER_SIZE as u64,
            chunk_data_size: chunk_data_size as u32,
            is_zipped: true,
            is_mca_file: true
        };

        snap.serialize(pack)?;
        pack.write_all(&buf)?;

        return Ok(snap);
    }
}
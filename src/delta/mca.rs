use std::{collections::BinaryHeap, fs::File, io::{self, Cursor, Read, Seek, Write}};
use libdeflater::{CompressionLvl, Compressor};
use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};

use crate::delta::{
    diff::{
        DiffCommandHeader, InsertZipHeader
    }, recover::CHUNK_VIRTUAL_SPACE, snapshot::{SnapshotHeader, SNAPSHOT_HEADER_SIZE}
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

#[derive(Debug)]
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
        let mut size = 8;
        f.write_u32::<BigEndian>(header.len() as u32)?;
        for chunk in header {
            f.write_u32::<BigEndian>(chunk.utime)?;
            f.write_u32::<BigEndian>(chunk.l_idx)?;
            f.write_u16::<BigEndian>(chunk.idx)?;
            size += 10;
        }
        if let Some(c) = header.last() {
            f.write_u32::<BigEndian>(c.size)?;
        } else {
            f.write_u32::<BigEndian>(0)?;
        }
        print!("Serialized size: {}\n", size);
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
        print!("Skipped: {}\n", len+4);
        f.seek(io::SeekFrom::Current(len))?;
        Ok(len as usize)
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

struct ChunkDescriptor {
    idx: usize,
    len: usize,
    is_zipped: bool
}

pub(crate) struct MCA {}
impl MCA {
    /* assumes that chunk is the last data in buf, inserts the data in the header about update time and position, also adds buffer memory to the end */
    fn update_header_and_normalize_size(buf: &mut Vec<u8>, header: &Vec<ChunkHeader>, pos: usize, len: usize, idx: usize) {
        let segment_cnt = len.div_ceil(SECTOR_SIZE as usize);
        let segment_pos = pos as u32 / SECTOR_SIZE as u32;
        buf.resize(pos + segment_cnt*SECTOR_SIZE as usize, 0);
        
        write_u32(buf, idx*4, (segment_pos << 8) | segment_cnt as u32);
        write_u32(buf, SECTOR_SIZE as usize + idx*4, header[idx].utime);
    }

    pub fn recover<R: Read + Seek>(snap: &SnapshotHeader, pack: &mut R) -> io::Result<Vec<u8>> {
        use crate::delta::recover::{Instruction, _recover};
        pack.seek(io::SeekFrom::Start(snap.pos + snap.chunk_data_size as u64))?;
        let header = ChunkHeader::deserialize_all(pack)?;
        print!("Header: {:?}\n", &header[..5]);

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
                idx: idx as usize, len: h.size as usize, is_zipped: false
            });
            idx += h.size as u64;
            t_idx += h.size as u64;
        }

        print!("Calling recover\n");
        let unzipped = _recover(pack, BinaryHeap::from(instructs), snap.clone(), idx)?;
        print!("Recovered\n");

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
        while let Ok((command, red)) = DiffCommandHeader::deserialize(&mut diff_data) {
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

    pub fn save(snap: &SnapshotHeader, pack: &mut File) -> io::Result<SnapshotHeader> {
        Ok(SnapshotHeader::default())
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
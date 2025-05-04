use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{self, Read, SeekFrom, Seek};
use flate2::read::GzDecoder;
use byteorder::{ReadBytesExt, BigEndian};
use libdeflater::{Decompressor, DecompressionError};

use crate::delta::recover::Instruction;
use crate::delta::snapshot::SnapshotHeader;

pub const HEADER_FIELDS_CNT: usize = 1024;
pub const HEADER_SIZE: usize = HEADER_FIELDS_CNT * 4 * 2;
pub const SECTOR_SIZE: u64 = 4096;

fn read_u32(data: &[u8], idx: usize) -> u32 {
    return u32::from_be_bytes(data[idx..idx+4].try_into().unwrap());
}

pub(crate) struct ChunkHeader {
    pub offset: u32,
    pub utime: u32,
}
impl ChunkHeader {
    pub fn new(header: &[u8; HEADER_SIZE]) -> Vec<ChunkHeader> {
        let mut chunks: Vec<ChunkHeader> = Vec::new();
        for i in 0..HEADER_FIELDS_CNT {
            let offset = read_u32(header, i*4);
            if offset >> 8 == 0 || offset & 0xFF == 0 { continue; }
            let timestamp = read_u32(header, SECTOR_SIZE as usize + i*4);
            chunks.push(ChunkHeader {offset: (offset >> 8) * SECTOR_SIZE as u32, utime: timestamp});
        }
        chunks.sort_by(|a, b| a.offset.cmp(&b.offset));
        chunks
    }
}

struct Region {}
pub struct RegionFactory {}
impl RegionFactory {
    fn uncompress_chunk_data(data: &[u8], comp_type: u8, chunk: &mut Vec<u8>) -> io::Result<usize> {
        match comp_type {
            1u8 => {
                let mut decoder = GzDecoder::new(&data[..]);
                return decoder.read_to_end(chunk);
            },
            2u8 | 0u8 => {
                let mut decompress = Decompressor::new();
                chunk.resize(chunk.capacity().max(data.len()*4), 0);
                loop {
                    match decompress.zlib_decompress(data, chunk) {
                        Ok(size) => {
                            chunk.truncate(size);
                            return Ok(size);
                        },
                        Err(DecompressionError::InsufficientSpace) => {
                            chunk.resize(chunk.len()*2, 0);
                            continue;
                        },
                        Err(_) => {
                            return Err(io::Error::new(io::ErrorKind::Other, "Incorrect zlib compressed format"))
                        }
                    }
                }
            },
            3u8 => {
                chunk.clear();
                chunk.extend_from_slice(&data);
                Ok(data.len())
            }
            _ => {
                Err(io::Error::new(io::ErrorKind::Other, "Unsupported chunk compression type"))
            }
        }
    }

    pub fn get_chunk_compressed(mut file: &File, offset: u64) -> io::Result<Vec<u8>> {
        let mut buffer: Vec<u8> = Vec::new();

        file.seek(SeekFrom::Start(offset)).unwrap();
        let length = file.read_u32::<BigEndian>().unwrap() as usize;
        if length <= 1 { return Ok(vec![]); }
        file.read_u8().unwrap();
        buffer.resize(length-1, 0);

        file.read_exact(&mut buffer).unwrap();
        return Ok(buffer);
    }

    fn _get_chunk(mut file: &File, offset: u64, chunk: &mut Vec<u8>) -> io::Result<()> {
        let mut buffer: Vec<u8> = Vec::new();

        file.seek(SeekFrom::Start(offset)).unwrap();
        let length = file.read_u32::<BigEndian>().unwrap() as usize;
        if length <= 1 {
            chunk.resize(0, 0);
            return Ok(()); 
        }
        let comp_type = file.read_u8().unwrap();
        buffer.resize(length-1, 0);

        file.read_exact(&mut buffer).unwrap();

        Self::uncompress_chunk_data(&buffer, comp_type, chunk)?;
        Ok(())
    }

    pub fn get_chunk(mut file: &File, offset: u64) -> io::Result<Vec<u8>> {
        let mut chunk = Vec::new();
        Self::_get_chunk(file, offset, &mut chunk)?;
        
        Ok(chunk)
    }

    pub fn unpack_region(file: &mut File) -> Option<Vec<u8>> {
        let mut header = [0u8; HEADER_SIZE];
        file.read_exact(&mut header).unwrap();
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend(header);

        let mut chunk: Vec<u8> = Vec::new();
        for i in 0..HEADER_FIELDS_CNT {
            let offset: u32 = u32::from_be_bytes(header[i*4..(i*4+4)].try_into().unwrap());
            if offset >> 8 == 0 || offset & 0xFF == 0 { 
                continue;
            }

            let offset = (offset >> 8) as u64 * SECTOR_SIZE;
            Self::_get_chunk(file, offset, &mut chunk).unwrap();
            buffer.extend(&chunk);
        }

        Some(buffer)
    }

    pub fn get_header(f: &mut File) -> io::Result<Vec<(u32, u32)>> {
        let mut header = [0u8; HEADER_SIZE];
        f.read_exact(&mut header).unwrap();

        let mut chunks: Vec<(u32, u32)> = Vec::new();
        for i in 0..HEADER_FIELDS_CNT {
            let offset = u32::from_be_bytes(header[i*4..i*4+4].try_into().unwrap());
            if offset >> 8 == 0 || offset & 0xFF == 0 { 
                continue; 
            }
            let timestamp = u32::from_be_bytes(header[i*4..i*4+4].try_into().unwrap());
            chunks.push(((offset >> 8) * SECTOR_SIZE as u32, timestamp));
        }

        Ok(chunks)
    }
}

/*
    get a old and new, check the modification time change, and ckeck for chunk moves, then returns an array of commands to do
*/

struct Interval {
    idx: u32,
    len: u32
}

struct CreateDiff {
    new_idx: u32,
    new_len: u32,
    old_idx: u32,
    old_len: u32
}

enum RegionDiffInstruction {
    Copy(Interval),
    Diff(CreateDiff),
    Insert(Interval)
}

#[derive(Debug, PartialEq, Eq, PartialOrd)]
struct Info {
    idx: usize,
    offset: u32,
    update_time: u32
}

impl Ord for Info {
    fn cmp(&self, other: &Self) -> Ordering {
        other.offset.cmp(&self.offset)
    }
}

impl RegionFactory {
    fn recover_data<R: Read + Seek>(f: &mut R, data: &mut[u8], snap: &SnapshotHeader, ops: &mut BinaryHeap<Instruction>) -> io::Result<()> {
        debug_assert!(snap.is_mca_file && snap.depend_on == u64::MAX, "Invalide snapshot input for recover_data");

        let mut buf: Vec<u8> = vec![0; HEADER_SIZE];
        let mut undecoded_buf: Vec<u8> = Vec::new();

        f.seek(io::SeekFrom::Start(snap.pos))?;
        f.read_exact(&mut buf)?;
        let mut idx: u64 = 0;

        loop {
            while let Some(op) = ops.peek() {
                if op.from >= idx + buf.len() as u64 { break; }
                let op = ops.pop().unwrap();
                let len = op.len.min(idx + buf.len() as u64 - op.from);
                data[op.to as usize..(op.to+len) as usize]
                    .copy_from_slice(&buf[(op.from-idx) as usize..(op.from-idx+len) as usize]);
                if len != op.len {
                    ops.push(Instruction { from: op.from+len, to: op.to+len, len: op.len-len });
                }
            }

            idx += buf.len() as u64;            
            
            if idx == snap.payload_len { break; }
            let length = f.read_u32::<BigEndian>().unwrap() as usize;
            let comp_type = f.read_u8().unwrap();
            undecoded_buf.resize(length-1, 0);
            f.read_exact(&mut undecoded_buf);
            Self::uncompress_chunk_data(data, comp_type, &mut buf);
        }

        Ok(())
    }

    fn _get_changes(old: &[u8; HEADER_SIZE], new: &[u8; HEADER_SIZE], time: u32) -> Vec<RegionDiffInstruction> {
        let mut v: Vec<Info> = Vec::with_capacity(HEADER_FIELDS_CNT);
        for i in 0..HEADER_FIELDS_CNT {
            let offset = read_u32(new, i*4);
            if offset >> 8 == 0 || offset & 0xFF == 0 { continue; }
            v.push(Info {
                idx: i,
                offset,
                update_time: read_u32(new, SECTOR_SIZE as usize + i*4)
            });
        }
        v.sort();
    
        let mut ins: Vec<RegionDiffInstruction> = Vec::with_capacity(v.len());
        for i in v {
            let d = read_u32(old, i.idx*4);
            let (idx, len) = ((d >> 8) * SECTOR_SIZE as u32, (d & 0xFF) * SECTOR_SIZE as u32);
            if i.update_time <= time && d != 0 {
                match ins.last_mut() {
                    Some(RegionDiffInstruction::Copy(c)) if c.idx + c.len == idx => {
                        c.len += len;
                    },
                    _ => { ins.push(RegionDiffInstruction::Copy(Interval {idx, len})); }
                }
                continue;
            }
            let (new_idx, new_len) = ((d >> 8) * SECTOR_SIZE as u32, (d & 0xFF) * SECTOR_SIZE as u32);
            if idx == 0 || len == 0 {
                ins.push(RegionDiffInstruction::Insert(Interval { idx: new_idx, len: new_len }));
            } else {
                ins.push(RegionDiffInstruction::Diff(CreateDiff { 
                    new_idx, 
                    new_len,
                    old_idx: idx,
                    old_len: len
                }));
            }
        }
        
        ins
    }

    pub fn get_changes(pack: &mut File, snap: SnapshotHeader, file: &mut File) -> io::Result<Vec<RegionDiffInstruction>> {
        pack.seek(io::SeekFrom::Start(snap.pos))?;
        let mut data = vec![0u8; snap.payload_len as usize];
        pack.read_exact(&mut data)?;

        Ok(vec![])
    }
}
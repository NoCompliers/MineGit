use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{self, Read, SeekFrom, Seek};
use flate2::read::{GzDecoder, ZlibDecoder};
use byteorder::{ReadBytesExt, BigEndian};

use crate::future::snapshot::SnapshotHeader;
// use crate::

pub const HEADER_FIELDS_CNT: usize = 1024;
pub const HEADER_SIZE: usize = HEADER_FIELDS_CNT * 4 * 2;
pub const SECTOR_SIZE: u64 = 4096;

struct Region {
    // offsets: [u32; HEADER_FIELDS_CNT],
    // mod_times: [u32; HEADER_FIELDS_CNT],
    // buffer: Vec<u8>
}

pub struct RegionFactory {}
impl RegionFactory {
    fn uncompress_chunk_data(data: &[u8], comp_type: u8, chunk: &mut Vec<u8>) -> Result<usize, std::io::Error> {
        match comp_type {
            1u8 => {
                let mut decoder = GzDecoder::new(&data[..]);
                return decoder.read_to_end(chunk);
            },
            2u8 | 0u8 => {
                let mut decoder = ZlibDecoder::new(&data[..]);
                return decoder.read_to_end(chunk);
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

    pub fn get_chunk(mut file: &File, offset: u64) -> Result<Vec<u8>, io::Error> {
        let mut buffer: Vec<u8> = Vec::new();

        file.seek(SeekFrom::Start(offset)).unwrap();
        let length = file.read_u32::<BigEndian>().unwrap() as usize;
        if length <= 1 { return Ok(vec![]); }
        let comp_type = file.read_u8().unwrap();
        buffer.resize(length-1, 0);

        file.read_exact(&mut buffer).unwrap();

        let mut chunk = Vec::new();
        Self::uncompress_chunk_data(&buffer, comp_type, &mut chunk)?;

        Ok(chunk)
    }

    pub fn unpack_region(file: &mut File) -> Option<Vec<u8>> {
        let mut header = [0u8; HEADER_SIZE];
        file.read_exact(&mut header).unwrap();
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend(header);

        for i in 0..HEADER_FIELDS_CNT {
            let offset: u32 = u32::from_be_bytes(header[i*4..(i*4+4)].try_into().unwrap());
            if offset >> 8 == 0 || offset & 0xFF == 0 { 
                continue;
            }

            let offset = (offset >> 8) as u64 * SECTOR_SIZE;
            buffer.extend(Self::get_chunk(file, offset).unwrap());
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
            chunks.push((offset, timestamp));
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

fn read_u32(data: &[u8], idx: usize) -> u32 {
    return u32::from_be_bytes(data[idx..idx+4].try_into().unwrap());
}

impl RegionFactory {
    fn _get_changes(old: & [u8; HEADER_SIZE], new: &[u8; HEADER_SIZE], time: u32) -> Vec<RegionDiffInstruction> {
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

    pub fn get_changes(pack: &mut File, snap: u64, file: &mut File) -> io::Result<Vec<RegionDiffInstruction>> {
        pack.seek(io::SeekFrom::Start(snap))?;
        let snapheader = SnapshotHeader::deserialize(pack)?;
        let mut data = vec![0u8; snapheader.payload_len as usize];
        pack.read_exact(&mut data)?;

        Ok(vec![])
    }
}
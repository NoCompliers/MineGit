use std::io::{self, Cursor, Read, Seek, Write};
use std::fs::File;
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use byteorder::{ReadBytesExt, BigEndian};
use libdeflater::{CompressionLvl, Compressor};
use std::mem;

use crate::delta::diff::DiffCommandHeader;
use crate::delta::snapshot::SnapshotHeader;
use super::mca::{ChunkHeader, read_u32};
use super::region::{zlib_decompress, RegionFactory};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Instruction {
    pub from: u64,
    pub to: u64,
    pub len: u64 
}
impl Ord for Instruction {
    fn cmp(&self, other: &Self) -> Ordering {
        other.from.cmp(&self.from)
    }
}
impl PartialOrd for Instruction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub const CHUNK_VIRTUAL_SPACE: u64 = (u32::MAX / 1024) as u64;
fn chunk_insert_handler(file: &mut [u8], chunk: &[u8], idx: u64, ops: &mut BinaryHeap<Instruction>) {
    while let Some(op) = ops.peek() {
        if idx + CHUNK_VIRTUAL_SPACE <= op.from { break; }
        let op = ops.pop().unwrap();
        
        let skip = op.from - idx;
        debug_assert!(op.from + op.len <= idx + chunk.len() as u64 || skip + ops.len() as u64 == CHUNK_VIRTUAL_SPACE, "Trying to copy data that dont exist, missuse of virtual memory inside recover");
        let len = op.len.min( (chunk.len() as u64).saturating_sub(skip) );
        
        file[(op.to as usize)..((op.to+len) as usize)]
            .copy_from_slice(&chunk[skip as usize..(skip+len) as usize]);
        
        if len == op.len { continue; }
        ops.push(Instruction { 
            from: op.from + len,
            to: op.to + len,
            len: op.len - len
        });
    }
}

fn insert_snap_data<R: Read + Seek>(snap: &SnapshotHeader, pack: &mut R, chunk_data: &mut Vec<u8>, diff: &mut Vec<u8>) -> io::Result<()> {
    pack.seek(io::SeekFrom::Start(snap.pos))?;
    chunk_data.resize(snap.chunk_data_size as usize, 0);
    pack.read_exact(chunk_data)?;

    let header_size = if snap.is_mca_file {
        ChunkHeader::skip_data(pack)?
    } else {
        0
    };
    
    let diff_data_size = snap.payload_len as usize - (snap.chunk_data_size as usize + header_size as usize);
    print!("Recover: DiffZipStart: {}, size: {}\n", pack.stream_position()?, diff_data_size);
    if snap.is_zipped {
        let mut temp = vec![0u8; diff_data_size];
        pack.read_exact(&mut temp)?;
        zlib_decompress(&temp, diff)?;
    } else {
        diff.resize(diff_data_size, 0);
        pack.read_exact(diff)?;
    }
    Ok(())
}

pub(super) fn _recover<R: Read + Seek>(pack: &mut R, mut ops: BinaryHeap<Instruction>, mut snap: SnapshotHeader, file: &mut [u8]) -> io::Result<()> {
    let mut next: Vec<Instruction> = Vec::new();
    let mut chunk_data: Vec<u8> = Vec::with_capacity( snap.chunk_data_size as usize );
    let mut buf: Vec<u8> = Vec::with_capacity(snap.payload_len as usize);
    let mut temp_buf: Vec<u8> = Vec::new();
    let mut temp_buf1: Vec<u8> = Vec::new();

    pack.seek(io::SeekFrom::Start(snap.pos))?;
    while !ops.is_empty() {
        insert_snap_data(&snap, pack, &mut chunk_data, &mut buf)?;

        let mut idx: u64 = 0;
        let mut buf_cursor = Cursor::new(&buf);
        while !ops.is_empty() {
            if ops.peek().unwrap().len == 0 {
                ops.pop();
                continue;
            }
            let (op_head, _) = DiffCommandHeader::deserialize(&mut buf_cursor)?;

            match &op_head {
                DiffCommandHeader::Copy(c) => {
                    while let Some(op) = ops.peek() {
                        if idx + c.len <= op.from { break; }
                        let op = ops.pop().unwrap();
                        let skip = op.from - idx;
                        let len = op.len.min((c.len).saturating_sub(skip));
                        let from = c.sidx + skip;
                        next.push(Instruction { from, len, to: op.to });
                        
                        if op.len == len { continue; }
                        ops.push(Instruction { 
                            from: op.from + len,
                            to: op.to + len,
                            len: op.len - len
                        });
                    }
                    idx += c.len;
                },
                DiffCommandHeader::Insert(ins) => {
                    while let Some(op) = ops.peek() {
                        if idx + ins.len <= op.from { break; }
                        let op = ops.pop().unwrap();

                        let skip = op.from - idx;
                        let len = op.len.min( ins.len.saturating_sub(skip) );
                        
                        let f_idx = buf_cursor.position();
                        file[(op.to as usize)..((op.to+len) as usize)]
                            .copy_from_slice(&buf[(f_idx+skip) as usize..(f_idx+skip+len) as usize]);
                        
                        if len == op.len { continue; }
                        ops.push(Instruction { 
                            from: op.from + len,
                            to: op.to + len,
                            len: op.len - len
                        });
                    }

                    idx += ins.len;
                    buf_cursor.seek(io::SeekFrom::Current(ins.len as i64))?;
                },
                DiffCommandHeader::CopyZip(c) => {
                    let op = ops.peek().unwrap();
                    if op.from >= idx + CHUNK_VIRTUAL_SPACE { 
                        idx += CHUNK_VIRTUAL_SPACE;
                        continue; 
                    }

                    pack.seek(io::SeekFrom::Start(c.pos))?;
                    let size = pack.read_u32::<BigEndian>()? as usize;
                    let comp_type = pack.read_u8()?;
                    temp_buf.resize(size, 0);
                    pack.read_exact(&mut temp_buf)?;
                    RegionFactory::uncompress_chunk_data(&temp_buf, comp_type, &mut temp_buf1)?;
                    
                    chunk_insert_handler(file, &temp_buf1, idx, &mut ops);
                    idx += CHUNK_VIRTUAL_SPACE;
                },
                DiffCommandHeader::InsertZip(i) => {
                    let op = ops.peek().unwrap();
                    if op.from >= idx + CHUNK_VIRTUAL_SPACE { 
                        idx += CHUNK_VIRTUAL_SPACE;
                        continue; 
                    }

                    let pos = i.pos as usize;
                    let size = read_u32(&chunk_data, pos) as usize;
                    let comp_type = chunk_data[pos + 4];
                    RegionFactory::uncompress_chunk_data(&chunk_data[pos+5 .. pos+5+size], comp_type, &mut temp_buf1)?;

                    chunk_insert_handler(file, &temp_buf1, idx, &mut ops);
                    idx += CHUNK_VIRTUAL_SPACE;
                }
            }
        }

        let mut heap_vec = mem::take(&mut ops).into_vec();
        mem::swap(&mut heap_vec, &mut next);
        ops = BinaryHeap::from(heap_vec);
        next.clear();

        if snap.depend_on == u64::MAX || ops.len() == 0 { break; }
        pack.seek(io::SeekFrom::Start(snap.depend_on))?;
        snap = SnapshotHeader::deserialize(pack)?;
    }

    Ok(())
}

pub fn recover(pack: &mut File, idx: u64) -> io::Result<Vec<u8>> {
    Ok(vec![])
}

use std::io::{self, Read, Write, Seek, Cursor};
use std::fs::File;
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use flate2::read::ZlibDecoder;

use crate::diff::DiffCommandHeader;
use crate::future::snapshot::SnapshotHeader;
use std::mem;

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

fn _recover<R: Read + Seek>(pack: &mut File, mut ops: BinaryHeap<Instruction>, mut snap: SnapshotHeader, size: u64, is_zip: bool) -> io::Result<Vec<u8>> {
    let mut file = vec![0u8; size as usize];
    let mut next: Vec<Instruction> = Vec::new();
    let mut buf: Vec<u8> = Vec::new();

    pack.seek(io::SeekFrom::Start(snap.pos))?;
    while snap.depend_on != u64::MAX {
        if snap.is_zipped {
            let mut decoder = ZlibDecoder::new(&mut *pack);
            decoder.read_to_end(&mut buf)?;
        } else {
            buf.resize(snap.payload_len as usize, 0);
            pack.read_exact(&mut buf)?;
        }

        let mut idx: u64 = 0;
        let mut buf_cursor = Cursor::new(&buf);
        while !ops.is_empty() {
            let (op_head, red) = DiffCommandHeader::deserialize(&mut buf_cursor).unwrap();
            let is_zipped = match &op_head {
                DiffCommandHeader::Copy(c) => false,
                DiffCommandHeader::Insert(i) => false,
                _ => true
            };

            match &op_head {
                DiffCommandHeader::Copy(c) | DiffCommandHeader::CopyZip(c) => {
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
                DiffCommandHeader::Insert(ins) | DiffCommandHeader::InsertZip(ins) => {
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
                }
            }
            let mut heap_vec = mem::take(&mut ops).into_vec();
            mem::swap(&mut heap_vec, &mut next);
            ops = BinaryHeap::from(heap_vec);
            next.clear();
        }

        pack.seek(io::SeekFrom::Start(snap.depend_on))?;
        snap = SnapshotHeader::deserialize(pack)?;
    }  

    Ok(vec![])
}

pub fn recover(pack: &mut File, idx: u64) -> io::Result<Vec<u8>> {
    Ok(vec![])
}
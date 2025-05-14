use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::io::{self, Cursor, Read, Seek};
use zstd::Decoder;
use crate::recover::diff::DiffCommandHeader;
use crate::recover::snapshot::SnapshotHeader;

#[derive(Debug, PartialEq, Eq)]
struct Instruction {
    from: u64,
    to: u64,
    len: u64,
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

fn init_data<R: Read>(pack: &mut R, snap: &SnapshotHeader, buf: &mut Vec<u8>, res: &mut Vec<u8>) -> io::Result<()> {
    if snap.is_zipped {
        buf.resize(snap.payload_len as usize, 0);
        pack.read_exact(buf)?;
        let mut decoder = Decoder::new(Cursor::new(&buf))?;
        decoder.read_to_end(res)?;
    } else {
        res.resize(snap.payload_len as usize, 0);
        pack.read_exact(res)?;
    }
    Ok(())
}

fn _recover<R: Read + Seek>(
    pack: &mut R,
    mut ops: BinaryHeap<Instruction>,
    mut snap: SnapshotHeader,
    file: &mut [u8]
) -> io::Result<()> {
    let mut next: Vec<Instruction> = Vec::new();
    let mut buf: Vec<u8> = Vec::new();
    let mut buf_temp: Vec<u8> = Vec::new();
    pack.seek(io::SeekFrom::Start(snap.pos))?;

    while !ops.is_empty() {
        let mut idx: u64 = 0;
        init_data(pack, &snap, &mut buf_temp, &mut buf)?;
        let mut buf_cursor = Cursor::new(&buf);
        while !ops.is_empty() {
            if ops.peek().unwrap().len == 0 {
                ops.pop();
                continue;
            }

            let (op_head, _) = DiffCommandHeader::deserialize(&mut buf_cursor).unwrap();
            match op_head {
                DiffCommandHeader::Copy(c) => {
                    while let Some(op) = ops.peek() {
                        if idx + c.len <= op.from { break; }
                        let op = ops.pop().unwrap();
                        let skip = op.from - idx;
                        let len = op.len.min((c.len).saturating_sub(skip));
                        let from = c.sidx + skip;
                        next.push(Instruction {
                            from,
                            len,
                            to: op.to,
                        });

                        if op.len == len { continue; }
                        ops.push(Instruction {
                            from: op.from + len,
                            to: op.to + len,
                            len: op.len - len,
                        });
                    }
                    idx += c.len;
                }
                DiffCommandHeader::Insert(ins) => {
                    while let Some(op) = ops.peek() {
                        if idx + ins.len <= op.from { break; }
                        let op = ops.pop().unwrap();

                        let skip = op.from - idx;
                        let len = op.len.min(ins.len.saturating_sub(skip));
                        let f_idx = (buf_cursor.position() + skip) as usize;

                        file[(op.to as usize)..((op.to + len) as usize)]
                            .copy_from_slice(&buf[f_idx..(f_idx + len as usize)]);

                        if len == op.len { continue; }
                        ops.push(Instruction {
                            from: op.from + len,
                            to: op.to + len,
                            len: op.len - len,
                        });
                    }

                    idx += ins.len;
                    buf_cursor.seek(io::SeekFrom::Current(ins.len as i64))?;
                }
            }
        }

        let temp = ops.into_vec();
        ops = BinaryHeap::from(next);
        next = temp;
        next.clear();

        if snap.depend_on == u64::MAX || ops.len() == 0 {
            break;
        }
        pack.seek(io::SeekFrom::Start(snap.depend_on))?;
        snap = SnapshotHeader::deserialize(pack)?;
    }

    Ok(())
}

pub fn recover<R: Read + Seek>(pack: &mut R, snap: SnapshotHeader) -> io::Result<Vec<u8>> {
    pack.seek(io::SeekFrom::Start(snap.pos))?;
    let len = snap.file_len;
    let bheap: BinaryHeap<Instruction> = BinaryHeap::from(vec![Instruction {
        from: 0,
        to: 0,
        len,
    }]);
    let mut file = Vec::with_capacity(snap.file_len as usize);
    file.resize(snap.file_len as usize, 0);
    _recover(pack, bheap, snap, &mut file)?;
    Ok(file)
}
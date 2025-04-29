use std::io::{self, Read, Write, Seek, Cursor};
use std::fs::File;
use std::collections::BinaryHeap;
use std::time::Instant;

use crate::diff::DiffCommandHeader;
use crate::future::snapshot::SnapshotHeader;
use crate::recover::*;

pub fn recover_test(files: Vec<&mut File>, last_file: &mut File, size: u64) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; size as usize];
    let mut ops = BinaryHeap::from([Instruction {
        from: 0,
        to: 0,
        len: size
    }]);
    
    let mut next: Vec<Instruction> = Vec::new();

    for f in files {
        let start = Instant::now();
        let mut idx: u64 = 0;
        while let Some(op) = ops.peek() {
            let (op_head, red) = DiffCommandHeader::deserialize(f).unwrap();
            match op_head {
                DiffCommandHeader::Copy(c) | DiffCommandHeader::CopyZip(c) => {
                    if idx + c.len <= op.from {
                        idx += c.len;
                        debug_assert!(idx <= op.from);
                        continue;
                    }
                    let op = ops.pop().unwrap();
                    let skip = op.from - idx; // add separate failure return
                    let len = op.len.min((c.len).saturating_sub(skip));
                    let from = c.sidx + skip;
                    next.push(Instruction { from, len, to: op.to });
                    
                    f.seek(io::SeekFrom::Current(-(red as i64)))?;
                    if op.len == len { continue; }
                    ops.push(Instruction { 
                        from: op.from + len,
                        to: op.to + len,
                        len: op.len - len
                    });
                },
                DiffCommandHeader::Insert(ins) | DiffCommandHeader::InsertZip(ins) => {
                    if idx + ins.len <= op.from {
                        idx += ins.len;
                        debug_assert!(idx <= op.from);
                        f.seek(io::SeekFrom::Current(ins.len as i64))?;
                        continue;
                    }
                    let op = ops.pop().unwrap();

                    let skip = op.from - idx;
                    let len = op.len.min( ins.len.saturating_sub(skip) );
                    
                    f.seek(io::SeekFrom::Current(skip as i64))?;
                    f.read_exact(&mut buf[(op.to as usize)..((op.to+len) as usize)]).unwrap();
                    f.seek(io::SeekFrom::Current(-((red + skip + len) as i64)))?;

                    if len == op.len { continue; }
                    ops.push(Instruction { 
                        from: op.from + len,
                        to: op.to + len,
                        len: op.len - len
                    });
                }
            }
        }
        print!("file recover: {:?}\n", start.elapsed().as_millis());

        let start = Instant::now();
        let temp = ops.into_vec();
        ops = BinaryHeap::from(next);
        next = temp;
        next.clear();
        print!("buffer swap: {:?}\n", start.elapsed().as_millis());
    }

    while let Some(op) = ops.pop() {
        last_file.seek(io::SeekFrom::Start(op.from))?;
        last_file.read_exact(&mut buf[(op.to as usize)..((op.to+op.len) as usize)])?;
    }

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use crate::diff_gen::DiffGenerator;

    fn cmp_files(fa: &mut File, fb: &mut File) -> io::Result<bool> {
        fa.seek(io::SeekFrom::Start(0))?;
        fb.seek(io::SeekFrom::Start(0))?;
        
        let mut fa_data = Vec::new();
        let mut fb_data = Vec::new();
    
        fa.read_to_end(&mut fa_data)?;
        fb.read_to_end(&mut fb_data)?;
    
        Ok(fa_data == fb_data)
    }

    fn test_files_recover(mut files: Vec<File>) -> bool {
        let mut diffs: Vec<File> = Vec::new();
        let mut diff_gen = DiffGenerator::new();
        for i in 1..files.len() {
            let (head, tail) = files.split_at_mut(i);
            head[i-1].seek(io::SeekFrom::Start(0)).unwrap();
            let mut out = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(format!("D:\\projects\\MineGitFork\\test_files\\recover\\generated\\out{}.txt", i)).unwrap();
    
            diff_gen.init(&mut head[i-1], &mut tail[0]).unwrap();
            diff_gen.generate(&mut out).unwrap();
            diffs.push(out);
        }
        
        diffs.reverse();
        let size = files.last().unwrap().metadata().unwrap().len();
    
        let mut _diffs: Vec<&mut File> = Vec::new();
        for f in &mut diffs {
            f.seek(io::SeekFrom::Start(0)).unwrap();
            _diffs.push(f);
        }
        files.last().unwrap().seek(io::SeekFrom::Start(0)).unwrap();
    
        let data = recover_test(_diffs, &mut files[0], size).unwrap();
        let mut recovered = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open("D:\\projects\\MineGitFork\\test_files\\recover\\generated\\recovered.txt").unwrap();
        
        recovered.write_all(&data).unwrap();
        cmp_files(&mut files.last_mut().unwrap(), &mut recovered).unwrap()
    }

    #[test]
    fn tr1() {
        let files = vec![
            File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\r.0.0.mca").unwrap(),
            File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\r.0.1.mca").unwrap(),
            File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\r.0.2.mca").unwrap(),
        ];

        assert_eq!(test_files_recover(files), true);
    }

    // #[test]
    fn t1() {
        let files = vec![
            File::open("D:\\projects\\MineGitFork\\test_files\\recover\\textfs\\fileA.txt").unwrap(),
            File::open("D:\\projects\\MineGitFork\\test_files\\recover\\textfs\\fileB.txt").unwrap(),
            File::open("D:\\projects\\MineGitFork\\test_files\\recover\\textfs\\fileC.txt").unwrap(),
            File::open("D:\\projects\\MineGitFork\\test_files\\recover\\textfs\\fileD.txt").unwrap(),
            File::open("D:\\projects\\MineGitFork\\test_files\\recover\\textfs\\fileE.txt").unwrap(),
            File::open("D:\\projects\\MineGitFork\\test_files\\recover\\textfs\\fileF.txt").unwrap(),
            // File::open("D:\\projects\\MineGitFork\\test_files\\recover\\fileG.txt").unwrap(),
        ];
        assert_eq!(test_files_recover(files), true);

        // let files = vec![
        //     File::open("D:\\projects\\MineGitFork\\test_files\\recover\\fileA.txt").unwrap(),
        //     File::open("D:\\projects\\MineGitFork\\test_files\\recover\\fileD.txt").unwrap(),
        //     File::open("D:\\projects\\MineGitFork\\test_files\\recover\\fileE.txt").unwrap()
        // ];
        // assert_eq!(test_files_recover(files), true);
    }
}
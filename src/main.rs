use std::fs::{File, OpenOptions};
use std::io::{self, Seek, Write, Read};
use std::time::Instant;

use diff_gen::DiffGenerator;
use region::RegionFactory;

use crate::recover_test::recover_test;

// mod package;
// mod snapshot;
mod diff;
mod diff_gen;
mod recover;
mod region;
mod recover_test;
pub mod future {
    pub mod snapshot;
}


fn cmp_files(fa: &mut File, fb: &mut File) -> io::Result<bool> {
    fa.seek(io::SeekFrom::Start(0))?;
    fb.seek(io::SeekFrom::Start(0))?;
    
    let mut fa_data = Vec::new();
    let mut fb_data = Vec::new();

    fa.read_to_end(&mut fa_data)?;
    fb.read_to_end(&mut fb_data)?;

    print!("Fa size: {}, Fb size: {}\n", fa_data.len(), fb_data.len());
    for i in 0..fa_data.len() {
        if fa_data[i] != fb_data[i] {
            print!("Diff at i: {}\n", i);
            return Ok(false);
        }
    }

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

    let mut _diffs: Vec<&mut File> = Vec::new();
    for f in &mut diffs {
        f.seek(io::SeekFrom::Start(0)).unwrap();
        _diffs.push(f);
    }
    /* let mut file1 = File::open("D:\\projects\\MineGitFork\\test_files\\recover\\generated\\out2.txt").unwrap();
    let mut file2 = File::open("D:\\projects\\MineGitFork\\test_files\\recover\\generated\\out1.txt").unwrap();
    let _diffs = vec![ &mut file1, &mut file2 ]; */

    let size = files.last_mut().unwrap().metadata().unwrap().len();
    let original = files.first_mut().unwrap();
    original.seek(io::SeekFrom::Start(0)).unwrap();
    print!("Metadata size: {}\n", size);

    let data = recover_test(_diffs, original, size).unwrap();
    let mut recovered = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open("D:\\projects\\MineGitFork\\test_files\\recover\\generated\\recovered.mca").unwrap();
    
    recovered.write_all(&data).unwrap();
    recovered.flush().unwrap();
    cmp_files(&mut files.last_mut().unwrap(), &mut recovered).unwrap()
}

fn main() {
    let mut files = vec![
        File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\r.0.1.mca").unwrap(),
        File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\r.0.2.mca").unwrap(),
        File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\r.0.0.mca").unwrap(),
    ];

    let mut data: Vec<Vec<u8>> = Vec::new();
    for i in 0..files.len() {
        let f = &mut files[i];
        let start = Instant::now();
        let data = RegionFactory::unpack_region(f).unwrap();
        print!("unpuck time: {:?}\n", start.elapsed());
        f.seek(io::SeekFrom::Start(0)).unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(format!("D:\\projects\\MineGitFork\\test_files\\recover\\generated\\ur{}", i)).unwrap();
        
        file.write_all(&data).unwrap();
        file.seek(io::SeekFrom::Start(0)).unwrap();
        files[i] = file;
    }

    // let mut files = Vec::new();
    // for i in 0..data.len() {
    //     let mut f = File::open(format!("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\g{}", i)).unwrap();
    //     f.write_all(&data[i]).unwrap();
    //     files.push(f);
    // }

    // assert_eq!(test_files_recover(files), true);
}
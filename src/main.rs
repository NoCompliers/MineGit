use std::env;
use std::fs::OpenOptions;
use std::io::{self, Read, Seek};
use std::{fs::File, io::Write};

mod args;
mod cli;
mod committer;
mod ignore_filter;
mod initializer;
mod savefiles;
mod utils;

mod recover {
    pub mod diff;
    pub mod diff_gen;
    pub mod recover;
    pub mod snapshot;
}

use recover::recover::recover;

use crate::recover::diff_gen::DiffGenerator;
use crate::recover::snapshot::{SnapshotHeader, SNAPSHOT_HEADER_SIZE};

fn test() -> io::Result<()> {
    let mut package = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(true)
        .create(true)
        .open("package.pcg")?;

    // return Ok(());
    let mut files: Vec<File> = vec![
        File::open("/home/vr/Documents/University/4_Semester/RUST/MineGit/test/test1.txt")?,
        File::open("/home/vr/Documents/University/4_Semester/RUST/MineGit/test/test2.txt")?, // File::open("D:\\projects\\MineGit\\test_files\\recover\\textfs\\fileA.txt")?,
                                                                                             // File::open("D:\\projects\\MineGit\\test_files\\recover\\textfs\\fileB.txt")?
    ];

    // adding original file to package(file)
    let mut data1 = Vec::new();
    files[0].read_to_end(&mut data1)?;
    SnapshotHeader::store_file(&mut package, &data1, false)?;
    files[0].seek(io::SeekFrom::Start(0))?;

    //
    let mut diff = DiffGenerator::new();
    let (head, tail) = files.split_at_mut(1);
    let mut diff_data: Vec<u8> = Vec::new();
    diff.init(&mut head[0], &mut tail[0])?;
    diff.generate(&mut diff_data)?;
    files[0].seek(io::SeekFrom::Start(0))?;

    let snap2 = SnapshotHeader {
        depend_on: 0, // means depends on the file which descriptor starts at position 0
        payload_len: diff_data.len() as u64,
        file_len: files[1].metadata()?.len() as u64,
        pos: package.stream_position()? + SNAPSHOT_HEADER_SIZE as u64,
        is_zipped: false,
        is_mca_file: true,
    };
    snap2.serialize(&mut package)?;
    package.write_all(&diff_data)?;

    let recovered = recover(&mut package, snap2)?;
    let mut data2: Vec<u8> = Vec::new();
    files[1].seek(io::SeekFrom::Start(0))?;
    files[1].read_to_end(&mut data2)?;

    if recovered == data2 {
        print!("Success\n");
    } else {
        print!("Failure\n");
    }

    let mut out = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("out.txt")?;
    out.write_all(&recovered)?;
    Ok(())
}

fn main() {
    env::set_var("RUST_BACKTRACE", "full");
    //test().unwrap();
    cli::run();
}

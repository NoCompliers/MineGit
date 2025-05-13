use std::fs::{File, OpenOptions};
use std::io::{self, Cursor, Seek, Write};
use std::time::Instant;

use byteorder::{ReadBytesExt, BigEndian};
use delta::region::RegionFactory;

use crate::delta::mca::MCA;

mod delta {
    pub mod diff;
    pub mod diff_gen;
    pub mod mca;
    pub mod recover;
    pub mod region;
    pub mod snapshot;
}

fn main() {
    let mut f  = File::open("D:\\projects\\MineGit\\test_files\\recover\\regions\\r.-1.0.mca").unwrap();
    let mut f1 = File::open("D:\\projects\\MineGit\\test_files\\recover\\regions\\_r.-1.0.mca").unwrap();
    let mut out = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open("D:\\projects\\MineGit\\test_files\\recover\\generated\\out.txt").unwrap();

    let start = Instant::now();
    let snap = MCA::save_new(&mut f, &mut out).unwrap();

    let data = MCA::recover(&snap, &mut out).unwrap();
    let mut data = Cursor::new(data);
    RegionFactory::unpack_region(&mut data).unwrap();
    print!("Region unpacked\n");

    /*f.seek(io::SeekFrom::Start(snap.pos)).unwrap();
    let len = f.read_u32::<BigEndian>().unwrap() as usize;
    let t = f.read_u8().unwrap();
    let buf = vec![0u8; len];
    let mut chunk: Vec<u8> = Vec::new();
    RegionFactory::uncompress_chunk_data(&buf, t, &mut chunk).unwrap();*/

    let snap1 = MCA::save(&snap, &mut out, &mut f1).unwrap();
    let data = MCA::recover(&snap1, &mut out).unwrap();
    print!("{:?}\nTime: {:?}\n", snap1, start.elapsed());

    let mut res = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("D:\\projects\\MineGit\\test_files\\r.0.1.mca").unwrap();
    res.write_all(&data).unwrap();

    print!("DataLen: {}\n", data.len());
    f.seek(io::SeekFrom::Start(0)).unwrap();
    let size = f.seek(io::SeekFrom::End(0)).unwrap();
    print!("Original size: {}\n", size);

    // let start = Instant::now();
    // f.seek(io::SeekFrom::Start(0)).unwrap();
    // let mut buf = vec![0u8; f.metadata().unwrap().len() as usize];
    // f.read_exact(&mut buf).unwrap();
    // out.write_all(&buf).unwrap();
    // print!("{:?}\nTime: {:?}\n", snap, start.elapsed());
}
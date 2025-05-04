use std::fs::{File, OpenOptions};
use std::io::{self, Seek, Write, Read};
use std::time::Instant;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::delta::diff_gen::DiffGenerator;
use crate::delta::region::{ChunkHeader, RegionFactory, HEADER_SIZE};

mod delta {
    pub mod diff;
    pub mod diff_gen;
    pub mod mca;
    pub mod recover;
    pub mod region;
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

    // let data = recover_test(_diffs, original, size).unwrap();
    // let mut recovered = OpenOptions::new()
    //     .read(true)
    //     .write(true)
    //     .create(true)
    //     .truncate(true)
    //     .open("D:\\projects\\MineGitFork\\test_files\\recover\\generated\\recovered.mca").unwrap();
    
    // recovered.write_all(&data).unwrap();
    // recovered.flush().unwrap();
    // cmp_files(&mut files.last_mut().unwrap(), &mut recovered).unwrap()
    true
}

fn _main() {
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

fn get_max_update_time(header: &[u8; HEADER_SIZE]) -> u32 {
    let chunks = ChunkHeader::new(header);
    let mut max_time = 0;
    for c in chunks {
        max_time = max_time.max(c.utime);
    }
    return max_time;
}

fn main() -> io::Result<()> {
    let mut f1 = File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\r.0.0.mca").unwrap();
    let start = Instant::now();
    RegionFactory::unpack_region(&mut f1);
    print!("Unpacking time: {:?}\n", start.elapsed().as_millis());


    /*let mut f1 = File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\r.0.0.mca").unwrap();
    let mut f2 = File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\_r.0.0.mca").unwrap();

    let mut header = vec![0u8; HEADER_SIZE];
    f1.read_exact(&mut header[..])?;
    let u1 = get_max_update_time(&header[..].try_into().unwrap());
    f2.read_exact(&mut header[..])?;
    let u2 = get_max_update_time(&header[..].try_into().unwrap());
    print!("{}, {}\n", u1, u2);*/

    Ok(())
}

fn __main() -> io::Result<()> {
    let mut f = File::open("D:\\projects\\MineGitFork\\test_files\\recover\\regions\\r.0.1.mca").unwrap();
    let header = RegionFactory::get_header(&mut f)?;
    let mut sum1 = 0;
    let mut sum2 = 0;
    for i in 0..header.len() {
        // print!("Offset: {}\n", header[i].0);
        let data = RegionFactory::get_chunk(&mut f, header[i].0 as u64)?;

        let start = Instant::now();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&data).unwrap();
        let compressed_data = encoder.finish().unwrap();
        print!("First: {:?}\n", start.elapsed());

        let chunk_data = RegionFactory::get_chunk_compressed(&mut f, header[i].0 as u64)?;

        let interval = &chunk_data[chunk_data.len() - 4..];
        let uncomp_size = u32::from_le_bytes(interval.try_into().expect("Slice length mismatch"));

        print!("Uncomp Guess: {}, size: {}\n", uncomp_size, data.len());

/*        // print!("Original: {}, Remade: {}\n", chunk_data.len(), compressed_data.len());

        let start = Instant::now();
        let mut compressor = Compressor::new(CompressionLvl::new(9).unwrap());
        let max_compressed_size = compressor.zlib_compress_bound(data.len());
        let mut compressed_data1 = vec![0u8; max_compressed_size];
        let actual_size = compressor
            .zlib_compress(&data, &mut compressed_data1)
            .expect("Compression failed");
        compressed_data1.truncate(actual_size);
        print!("Second: {:?}\n", start.elapsed());

        // assert!(chunk_data.len() >= compressed_data.len());

        sum1 += compressed_data.len();
        sum2 += compressed_data1.len();

        if chunk_data == compressed_data {
            print!("First works");
        } 
        if chunk_data == compressed_data1 {
            print!("Second works");
        }
        if chunk_data != compressed_data && chunk_data != compressed_data1 {
            print!("Unluck i guess, origin: {}, 0: {}, 1: {}\n", chunk_data.len(), compressed_data.len(), compressed_data1.len());
        }*/
    }

    print!("S1: {}, S2: {}\n", sum1, sum2);

    Ok(())
}
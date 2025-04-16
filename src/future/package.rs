use std::fs::{File};
use std::io::{self, Write, Read, Seek, SeekFrom};

use crate::future::snapshot::{SnapshotHeader, SNAPSHOT_HEADER_SIZE};

struct Package {
    fs: File
}

impl Package {
    fn get_file(&mut self, offset: u64) -> io::Result<SnapshotHeader> {
        self.fs.seek(SeekFrom::Current(offset as i64))?;
        Ok(SnapshotHeader::deserialize(&mut self.fs)?)
    }
    
    fn insert_file<R: Read>(&mut self, f: &mut R) -> io::Result<u64> { // maybe move to package module
        let mut buf: Vec<u8> = Vec::new();
        f.read_to_end(&mut buf)?;
        SnapshotHeader {
            depend_on: u64::MAX,
            payload_len: buf.len() as u64,
            file_len: buf.len() as u64,
            is_zipped: false,
            is_mca_file: true,
            pos: 0
        }.serialize(&mut self.fs)?;
        self.fs.write_all(&buf)?;

        Ok((buf.len() + SNAPSHOT_HEADER_SIZE) as u64)
    }
  
    fn insert_file_safe(&mut self, f: &mut File) -> io::Result<u64> {
        let pos = self.fs.seek(SeekFrom::Current(0))?;
        debug_assert!(pos == self.fs.seek(SeekFrom::Current(0))?, "Chatgpt is a liar");

        match self.insert_file(f) {
            Ok(num) => return Ok(num),
            Err(e) => {
                self.fs.seek(SeekFrom::Start(pos))?;
                Err(e)
            }
        }
    }
}
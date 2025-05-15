use std::fs::{File};
use std::io::{self, Cursor, Read, Seek, Write};
use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};
use zstd::encode_all;
use crate::recover::diff::Insert;

use super::diff_gen::DiffGenerator;
use super::recover::recover;

const HEADER_SIZE: usize = 1024 * 4 * 2;

#[derive(Clone)]
pub struct SnapshotHeader {
    pub depend_on: u64,
    pub payload_len: u64,
    pub file_len: u64,
    pub pos: u64,
    pub is_zipped: bool
}

impl Default for SnapshotHeader {
    fn default() -> Self {
        Self {
            depend_on: u64::MAX, 
            payload_len: 0,
            file_len: 0,
            pos: u64::MAX,
            is_zipped: false
        }
    }
}

impl SnapshotHeader {
    pub const SERIZIZED_SIZE: usize = 25;
    pub fn save_new<W: Write + Seek>(f: &mut W, data: &[u8]) -> io::Result<Self> {
        f.seek(io::SeekFrom::End(0))?;
        let snap = SnapshotHeader {
            depend_on: u64::MAX,
            payload_len: data.len() as u64 + Insert::SERIZIZED_SIZE,
            file_len: data.len() as u64,
            pos: f.stream_position()? + Self::SERIZIZED_SIZE as u64,
            is_zipped: false,
        };
        snap.serialize(f)?;
        Insert::serialize(data, f)?;
        Ok(snap)
    }

    pub fn update<F: Read + Seek + Write>(&self, pack: &mut F, f: &[u8]) -> io::Result<Self> {
        let data = recover(pack, self.clone())?; // self.file_len as usize + f.len()
        let mut diff = DiffGenerator::new();
        diff.init_new(data, f)?;
        let mut diff_data: Vec<u8> = Vec::new();
        diff.generate(&mut diff_data)?;

        let diff_data = encode_all(Cursor::new(diff_data), 16).expect("Compression failed");

        pack.seek(io::SeekFrom::End(0))?;
        let snap = Self {
            depend_on: self.pos - Self::SERIZIZED_SIZE as u64,
            payload_len: diff_data.len() as u64,
            file_len: f.len() as u64,
            pos: pack.stream_position()? + Self::SERIZIZED_SIZE as u64,
            is_zipped: true,
        };
        snap.serialize(pack)?;
        pack.write_all(&diff_data)?;
        Ok(snap)
    }

    pub fn restore<R: Read + Seek>(&self, pack: &mut R) -> io::Result<Vec<u8>> {
        Ok(recover(pack, self.clone())?)
    }

    pub fn serialize<W: Write>(&self, out: &mut W) -> io::Result<()> {
        out.write_u64::<BigEndian>(self.depend_on)?;
        out.write_u64::<BigEndian>(self.payload_len)?;
        out.write_u64::<BigEndian>(self.file_len)?;
        out.write_u8(self.is_zipped as u8)?;
        Ok(())
    }

    pub fn deserialize<R: Read + Seek>(r: &mut R) -> io::Result<Self> {
        let mut res = Self::default();
        res.depend_on = r.read_u64::<BigEndian>()?;
        res.payload_len = r.read_u64::<BigEndian>()?;
        res.file_len = r.read_u64::<BigEndian>()?;
        let bits = r.read_u8()?;
        res.is_zipped = (bits & 1) != 0;
        res.pos = r.stream_position()?;
        Ok(res)
    }

    fn _get_header(f: &mut File, offset: u64) -> io::Result<Vec<u8>> {
        f.seek(io::SeekFrom::Start(offset))?;
        let snapheader = SnapshotHeader::deserialize(f)?;
        if !snapheader.is_zipped {
            f.seek(io::SeekFrom::Current(-(offset as i64)))?;
        }
        let mut data = vec![0u8; HEADER_SIZE];
        f.read_exact(&mut data)?;

        Ok(vec![])
    }
}
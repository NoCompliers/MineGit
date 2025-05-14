use std::fs::{File};
use std::io::{self, Write, Read, Seek};
use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};
use crate::recover::diff::Insert;

const HEADER_SIZE: usize = 1024 * 4 * 2;
pub const SNAPSHOT_HEADER_SIZE: usize = 25;

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
    pub fn store_file<W: Write>(f: &mut W, data: &[u8]) -> io::Result<Self> {
        let snap = SnapshotHeader {
            depend_on: u64::MAX,
            payload_len: data.len() as u64,
            file_len: data.len() as u64,
            is_zipped: false,
            pos: u64::MAX
        };
        snap.serialize(f)?;
        Insert::serialize(data, f)?;
        return Ok(snap);
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
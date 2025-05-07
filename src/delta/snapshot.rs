use std::io::{self, Write, Read, Seek};
use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};

use crate::delta::diff::Insert;

pub const SNAPSHOT_HEADER_SIZE: usize = 25;
#[derive(Debug, Clone)]
pub struct SnapshotHeader {
    pub depend_on: u64,
    pub payload_len: u64,
    pub file_len: u64,
    pub pos: u64,
    pub chunk_data_size: u32,
    pub is_zipped: bool,
    pub is_mca_file: bool
}

impl Default for SnapshotHeader {
    fn default() -> Self {
        Self {
            depend_on: u64::MAX, 
            payload_len: 0,
            file_len: 0,
            pos: u64::MAX,
            chunk_data_size: 0,
            is_zipped: false,
            is_mca_file: true
        }
    }
}

impl SnapshotHeader {
    pub fn serialize<W: Write>(&self, out: &mut W) -> io::Result<()> {
        out.write_u64::<BigEndian>(self.depend_on)?;
        out.write_u64::<BigEndian>(self.payload_len)?;
        out.write_u64::<BigEndian>(self.file_len)?;
        out.write_u8(self.is_zipped as u8 | ((self.is_mca_file as u8) << 1))?;
        Ok(())
    }

    pub fn deserialize<R: Read + Seek>(r: &mut R) -> io::Result<Self> {
        let mut res = Self::default();
        res.depend_on = r.read_u64::<BigEndian>()?;
        res.payload_len = r.read_u64::<BigEndian>()?;
        res.file_len = r.read_u64::<BigEndian>()?;
        let bits = r.read_u8()?;
        res.is_zipped = (bits & 1) != 0;
        res.is_mca_file = (bits & 0x10) != 0;
        res.pos = r.stream_position()?;
        Ok(res)
    }

    pub fn store_file<W: Write>(f: &mut W, data: &[u8], is_mca: bool) -> io::Result<Self> {
        let snap = SnapshotHeader {
            depend_on: u64::MAX,
            payload_len: data.len() as u64,
            file_len: data.len() as u64,
            is_zipped: false,
            chunk_data_size: 0,
            is_mca_file: is_mca,
            pos: u64::MAX
        };
        snap.serialize(f)?;

        Insert::serialize(f, data)?;
        Ok(snap)
    }
}


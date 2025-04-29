use std::fs::File;
use std::io::{self, Write, Read, Seek, SeekFrom};
use byteorder::{WriteBytesExt, ReadBytesExt, BigEndian};

use crate::region::{RegionFactory, HEADER_SIZE};
use crate::recover::recover;
pub struct IntervalMapping {
    old_idx: u64,
    new_idx: u64,
    len: u64
}

pub const SNAPSHOT_HEADER_SIZE: usize = 25;
pub struct SnapshotHeader {
    pub depend_on: u64,
    pub payload_len: u64,
    pub file_len: u64,
    pub pos: u64,
    pub is_zipped: bool,
    pub is_mca_file: bool
} // maybe package_in?
/*
to add
*/

impl Default for SnapshotHeader {
    fn default() -> Self {
        Self {
            depend_on: u64::MAX, 
            payload_len: 0,
            file_len: 0,
            pos: u64::MAX,
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

    pub fn get_header(snap: &SnapshotHeader, f: &mut File) -> io::Result<Vec<u8>> {
        f.seek(io::SeekFrom::Start(snap.pos + SNAPSHOT_HEADER_SIZE as u64))?;
        if !snap.is_mca_file {
            return Err(io::Error::new(io::ErrorKind::Other, "Trying to read header of not .mca file"));
        }
        let mut data: Vec<u8>;
        if snap.depend_on == u64::MAX {
            debug_assert!(snap.is_zipped == false, "No support for zipped .mca files");
            data = vec![0u8; HEADER_SIZE];
            f.read_exact(&mut data)?;
        } else {
            data = recover(f, snap.pos)?;
        }

        Ok(data)
    }

    fn get_intervals_mca(&self, f: &mut File, mut ints: Vec<IntervalMapping>, buf: &mut [u8]) -> io::Result<Vec<u8>> {
        if self.depend_on != u64::MAX {
            return Err(io::Error::new(io::ErrorKind::Other, "Trying to get intervals of mca file for not final snapshot, this file relies on the other file"));
        }

        let header = SnapshotHeader::get_header(self, f)?;
        for i in &mut ints {
            if i.old_idx >= HEADER_SIZE as u64 { break; }
            let len = (HEADER_SIZE as u64).min(i.old_idx + i.len) - i.old_idx;
            buf[i.new_idx as usize..(i.new_idx+len) as usize]
                .copy_from_slice(&header[i.old_idx as usize..(i.old_idx+len) as usize]);
            i.old_idx += len;
            i.new_idx += len;
            i.len -= len;
        }

        for i in ints {
            if i.len == 0 { continue; }
            
        }

        Ok(vec![])
    }
}

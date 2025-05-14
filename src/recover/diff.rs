use std::io::{ self, Write, Read };
use byteorder::{ReadBytesExt, BigEndian};

#[derive(Debug)]
pub struct Copy {
    pub sidx: u64,
    pub len: u64
}

#[derive(Debug)]
pub struct Insert {
    pub data: Vec<u8>
}

#[derive(Debug)]
pub struct InsertHeader {
    pub len: u64
}

#[derive(Debug)]
pub enum DiffCommandHeader {
    Copy(Copy),
    Insert(InsertHeader)
}

impl Copy {
    pub fn serialize<W: Write>(&self, out: &mut W) -> io::Result<()> {
        debug_assert!(self.sidx <= u32::MAX as u64);
        debug_assert!(self.len  <= (u32::MAX >> 1) as u64);

        let d = (( self.len as u64 ) << 32) | (self.sidx as u64);
        out.write_all(&d.to_be_bytes())?;
        Ok(())
    }
}

impl Insert {
    pub const SERIZIZED_SIZE: u64 = 8;
    pub fn serialize<W: Write>(data: &[u8], out: &mut W) -> io::Result<()> {
        debug_assert!(data.len() <= (u32::MAX >> 1) as usize);
        let size = ( 1u32 << 31 ) | (data.len() as u32);
        out.write_all(&size.to_be_bytes())?;
        out.write_all(data)?;
        Ok(())
    }
}

impl DiffCommandHeader {
    pub fn deserialize<R: Read>(r: &mut R) -> io::Result<(DiffCommandHeader, u64)> {
        let len: u32 = r.read_u32::<BigEndian>()?;
        match (len >> 31) & 1 {
            0 => {
                return Ok((DiffCommandHeader::Copy( Copy {
                    len: len as u64,
                    sidx: r.read_u32::<BigEndian>()? as u64
                }), 8));
            },
            1 => {
                return Ok((DiffCommandHeader::Insert( InsertHeader {
                    len: (len & !(1 << 31)) as u64
                }), 4));
            },
            _ => panic!("Imposible match for the single bit")
        }
    }
}
use std::io::{ self, Write, Read };
use byteorder::{ReadBytesExt, BigEndian};
use serde::{Serialize, Deserialize};

#[derive(Debug)]
pub(crate) struct Insert {
    pub data: Vec<u8>
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Copy {
    pub sidx: u64,
    pub len: u64,
}

#[derive(Debug)]
pub(crate) struct InsertHeader {
    pub len: u64,
}

#[derive(Debug)]
pub(crate) struct CopyZipHeader {
    pub pos: u64
}

#[derive(Debug)]
pub(crate) struct InsertZipHeader {
    pub pos: u32
}

#[derive(Debug)]
pub(crate) enum DiffCommandHeader {
    Copy(Copy),
    Insert(InsertHeader),
    CopyZip(CopyZipHeader),
    InsertZip(InsertZipHeader),
}

impl DiffCommandHeader {
    pub fn deserialize<R: Read>(r: &mut R) -> io::Result<(Self, u64)> {
        let len: u32 = r.read_u32::<BigEndian>()?;
        let id = (len >> 30) & 3;
        let len = ( len & !(3 << 30) ) as u64;
        match id {
            0 => {
                return Ok((Self::Copy( Copy {
                    len,
                    sidx: r.read_u32::<BigEndian>()? as u64
                }), 8));
            },
            1 => {
                return Ok((Self::CopyZip( CopyZipHeader {
                    pos: ( len << 32 ) | r.read_u32::<BigEndian>()? as u64
                }), 8));
            },
            2 => {
                return Ok((Self::Insert( InsertHeader {
                    len
                }), 8));
            },
            3 => {
                return Ok((Self::InsertZip( InsertZipHeader { 
                    pos: len as u32
                }), 4));
            },
            _ => panic!("Imposible match for the single bit(Note Copy)")
        }
    }

    pub fn serialize<W: Write>(&self, w: &mut W) -> io::Result<()> {
        match self {
            Self::Copy(c) => {
                debug_assert!(c.sidx <= u32::MAX as u64);
                debug_assert!(c.len  <= (u32::MAX >> 2) as u64);
                let d = (( c.len as u64 ) << 32) | (c.sidx as u64);
                w.write_all(&d.to_be_bytes())?;
            },
            Self::CopyZip(c) => {
                debug_assert!(c.pos <= u64::MAX >> 2);
                let d = c.pos | (1u64 << 62);
                w.write_all(&d.to_be_bytes())?;
            },
            Self::Insert(i) => {
                let size = ( 2u32 << 30 ) | (i.len as u32);
                w.write_all(&size.to_be_bytes())?;
            },
            Self::InsertZip(i) => {
                let size = ( 3u32 << 30 ) | i.pos;
                w.write_all(&size.to_be_bytes())?;
            }
        }
        Ok(())
    }
}

impl Insert {
    pub fn serialize<W: Write>(w: &mut W, data: &[u8]) -> io::Result<()> {
        DiffCommandHeader::Insert(InsertHeader { len: data.len() as u64 }).serialize(w)?;
        w.write_all(data)?;
        Ok(())
    }
}
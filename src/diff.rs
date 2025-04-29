use std::io::{ self, Write, Read };
use byteorder::{ReadBytesExt, BigEndian};
use serde::{Serialize, Deserialize};

#[derive(Debug)]
pub struct Insert {
    pub data: Vec<u8>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Copy {
    pub sidx: u64,
    pub len: u64,
}

#[derive(Debug)]
pub struct InsertHeader {
    pub len: u64,
}

#[derive(Debug)]
pub enum DiffCommandHeader {
    Copy(Copy),
    CopyZip(Copy),
    Insert(InsertHeader),
    InsertZip(InsertHeader),
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
                return Ok((Self::CopyZip( Copy {
                    len,
                    sidx: r.read_u32::<BigEndian>()? as u64
                }), 8));
            },
            2 => {
                return Ok((Self::Insert( InsertHeader {
                    len
                }), 4));
            },
            3 => {
                return Ok((Self::InsertZip( InsertHeader {
                    len
                }), 4));
            },
            _ => panic!("Imposible match for the single bit(Note Copy)")
        }
    }

    pub fn serialize<W: Write>(&self, w: &mut W) -> io::Result<()> {
        match self {
            Self::Copy(c) => {
                debug_assert!(c.sidx <= u32::MAX as u64);
                debug_assert!(c.len  <= (u32::MAX >> 1) as u64);
                let d = (( c.len as u64 ) << 32) | (c.sidx as u64);
                w.write_all(&d.to_be_bytes())?;
            },
            Self::CopyZip(c) => {
                debug_assert!(c.sidx <= u32::MAX as u64);
                debug_assert!(c.len  <= (u32::MAX >> 1) as u64);
                let d = (( ( c.len | (1u64 << 30) ) ) << 32) | (c.sidx as u64);
                w.write_all(&d.to_be_bytes())?;
            },
            Self::Insert(i) => {
                let size = ( 2u32 << 31 ) | (i.len as u32);
                w.write_all(&size.to_be_bytes())?;
            },
            Self::InsertZip(i) => {
                let size = ( 3u32 << 31 ) | (i.len as u32);
                w.write_all(&size.to_be_bytes())?;
            }
        }
        Ok(())
    }
}


#[derive(Debug)]
pub enum DiffCommand {
    Copy(Copy),
    CopyZip(Copy),
    Insert(Insert),
    InsertZip(Insert)
}

impl DiffCommand {
    pub fn print(&self) {
        match self {
            Self::Insert(i) => 
                print!("I: \"{:?}\"\n", i.data),
            Self::Copy(c) => 
                print!("C: {} {}\n", c.sidx, c.len),
            Self::InsertZip(i) => 
                print!("IC: \"{:?}\"\n", i.data),
            Self::CopyZip(c) => 
                print!("CC: {} {}\n", c.sidx, c.len),
        }
    }

    /*pub fn print_full(&self, data: &[u8]) {
        match self {
            DiffCommand::Insert(i) => 
                print!("I: \"{:?}\"\n", i.data),
            DiffCommand::Copy(c) => 
                print!("C: {} {}: \"{:?}\"\n", c.sidx, c.len, &data[c.sidx as usize..(c.sidx+c.len) as usize])
        }
    }

    pub fn prints(c: &[DiffCommand], data: &[u8]) {
        for c in c {
            c.print_full(data);
        }
    }*/

    pub fn serialize<W: Write>(&self, w: &mut W) -> io::Result<()> {
        match self {
            Self::Insert(i) => {
                DiffCommandHeader::Insert(InsertHeader { len: i.data.len() as u64 }).serialize(w);
                w.write_all(&i.data)?;
            },
            Self::InsertZip(i) => {
                DiffCommandHeader::InsertZip(InsertHeader { len: i.data.len() as u64 }).serialize(w);
                w.write_all(&i.data)?;
            },
            _ => panic!("Use DiffCommandHeader serializer instead")
        }

        Ok(())
    }
}

impl Insert {
    pub fn serialize<W: Write>(w: &mut W, data: &[u8], is_zipped: bool) -> io::Result<()> {
        if !is_zipped {
            DiffCommandHeader::Insert(InsertHeader { len: data.len() as u64 }).serialize(w);
        } else {
            DiffCommandHeader::InsertZip(InsertHeader { len: data.len() as u64 }).serialize(w);
        }
        w.write_all(data)?;
        Ok(())
    }
}
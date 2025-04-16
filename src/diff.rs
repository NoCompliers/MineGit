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

pub struct InsertHeader {
    pub len: u64
}

pub enum DiffCommandHeader {
    Copy(Copy),
    Insert(InsertHeader)
}

#[derive(Debug)]
pub enum DiffCommand {
    Copy(Copy),
    Insert(Insert)
}

impl DiffCommand {
    pub fn print(&self) {
        match self {
            DiffCommand::Insert(i) => 
                print!("I: \"{:?}\"\n", i.data),
            DiffCommand::Copy(c) => 
                print!("C: {} {}\n", c.sidx, c.len)
        }
    }

    pub fn print_full(&self, data: &[u8]) {
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
    }
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
    pub fn serialize<W: Write>(data: &[u8], out: &mut W) -> io::Result<()> {
        debug_assert!(data.len() <= (u32::MAX >> 1) as usize);
        let size = ( 1u32 << 31 ) | (data.len() as u32);
        out.write_all(&size.to_be_bytes())?;
        out.write_all(data)?;
        Ok(())
    }
}

pub fn read_command_header<R: Read>(r: &mut R) -> io::Result<(DiffCommandHeader, u64)> { // fsize: &mut usize
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

pub fn read_command<R: Read>(r: &mut R) -> io::Result<(DiffCommand, u64)> { // fsize: &mut usize
    let (c, red) = read_command_header(r)?;
    match c {
        DiffCommandHeader::Copy(c) => Ok((DiffCommand::Copy(c), red)),
        DiffCommandHeader::Insert(i) => {
            let len = i.len as usize;
            // if *fsize < len {
            //     return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Trying to read outside of file snapshot"));
            // }
            let mut buf = vec![0; len];
            r.read_exact(&mut buf)?;
            // *fsize -= len;
            Ok((DiffCommand::Insert(Insert {
                data: buf
            }), red + len as u64))
        }
    }
}

// pub fn print_commands<R: Read>(payload: &mut R, mut len: usize) -> io::Result<()> {
//     while len != 0 {
//         let (res, _) = read_command(payload)?;
//         res.print();
//     }
//     Ok(())
// }
use std::fs::File;
use std::io::{self, Read, SeekFrom, Seek};
use flate2::read::GzDecoder;
use byteorder::{ReadBytesExt, BigEndian};
use libdeflater::{Decompressor, DecompressionError};

use crate::delta::mca::*;

pub fn zlib_decompress(source: &[u8], target: &mut Vec<u8>) -> io::Result<usize> {
    let mut decompress = Decompressor::new();
    target.resize(target.capacity().max(source.len()*4), 0);
    loop {
        match decompress.zlib_decompress(source, target) {
            Ok(size) => {
                target.truncate(size);
                return Ok(size);
            },
            Err(DecompressionError::InsufficientSpace) => {
                target.resize(target.len()*2, 0);
                print!("NotEnoughtSpace\n");
                continue;
            },
            Err(DecompressionError::BadData) => {
                return Err(io::Error::new(io::ErrorKind::Other, "Incorrect zlib compressed format"))
            }
        }
    }
}

struct Region {}
pub struct RegionFactory {}
impl RegionFactory {
    pub fn uncompress_chunk_data(data: &[u8], comp_type: u8, chunk: &mut Vec<u8>) -> io::Result<usize> {
        match comp_type {
            1u8 => {
                let mut decoder = GzDecoder::new(&data[..]);
                return decoder.read_to_end(chunk);
            },
            2u8 | 0u8 => {
                Ok(zlib_decompress(data, chunk)?)
            },
            3u8 => {
                chunk.clear();
                chunk.extend_from_slice(&data);
                Ok(data.len())
            }
            _ => {
                Err(io::Error::new(io::ErrorKind::Other, "Unsupported chunk compression type"))
            }
        }
    }

    pub fn get_chunk_compressed(mut file: &File, offset: u64) -> io::Result<Vec<u8>> {
        let mut buffer: Vec<u8> = Vec::new();

        file.seek(SeekFrom::Start(offset)).unwrap();
        let length = file.read_u32::<BigEndian>().unwrap() as usize;
        if length <= 1 { return Ok(vec![]); }
        file.read_u8().unwrap();
        buffer.resize(length-1, 0);

        file.read_exact(&mut buffer).unwrap();
        return Ok(buffer);
    }

    fn _get_chunk<R: Read+Seek>(file: &mut R, offset: u64, chunk: &mut Vec<u8>) -> io::Result<()> {
        let mut buffer: Vec<u8> = Vec::new();

        file.seek(SeekFrom::Start(offset)).unwrap();
        let length = file.read_u32::<BigEndian>().unwrap() as usize;
        if length <= 1 {
            chunk.resize(0, 0);
            return Ok(()); 
        }
        let comp_type = file.read_u8().unwrap();
        buffer.resize(length-1, 0);

        file.read_exact(&mut buffer).unwrap();

        Self::uncompress_chunk_data(&buffer, comp_type, chunk)?;
        Ok(())
    }

    pub fn get_chunk(file: &mut File, offset: u64) -> io::Result<Vec<u8>> {
        let mut chunk = Vec::new();
        Self::_get_chunk(file, offset, &mut chunk)?;
        
        Ok(chunk)
    }

    pub fn unpack_region<R: Read+Seek>(file: &mut R) -> Option<Vec<u8>> {
        let mut header = [0u8; HEADER_SIZE];
        file.read_exact(&mut header).unwrap();
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend(header);

        let mut chunk: Vec<u8> = Vec::new();
        for i in 0..HEADER_FIELDS_CNT {
            let offset: u32 = u32::from_be_bytes(header[i*4..(i*4+4)].try_into().unwrap());
            if offset >> 8 == 0 || offset & 0xFF == 0 { 
                continue;
            }

            let offset = (offset >> 8) as u64 * SECTOR_SIZE;
            Self::_get_chunk(file, offset, &mut chunk).unwrap();
            buffer.extend(&chunk);
        }

        Some(buffer)
    }

    pub fn get_header(f: &mut File) -> io::Result<Vec<(u32, u32)>> {
        let mut header = [0u8; HEADER_SIZE];
        f.read_exact(&mut header).unwrap();

        let mut chunks: Vec<(u32, u32)> = Vec::new();
        for i in 0..HEADER_FIELDS_CNT {
            let offset = u32::from_be_bytes(header[i*4..i*4+4].try_into().unwrap());
            if offset >> 8 == 0 || offset & 0xFF == 0 { 
                continue; 
            }
            let timestamp = u32::from_be_bytes(header[i*4..i*4+4].try_into().unwrap());
            chunks.push(((offset >> 8) * SECTOR_SIZE as u32, timestamp));
        }

        Ok(chunks)
    }
}
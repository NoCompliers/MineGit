use std::fmt;

use bitcode::{Decode, Encode};
use bytemuck::{Pod, Zeroable};
use chrono::DateTime;

pub const DIRECTORY_NAME: &str = ".minegit";
pub const COMMITS_FILE_NAME: &str = "commits.byte";
pub const COMMITS_INFO_FILE_NAME: &str = "commits_info.bitcode";
pub const IGNORE_FILE_NAME: &str = "ignore";

#[derive(Debug, Copy, Clone)]
pub struct Commit {
    pub tag: [u8; 256],
    pub id: u32,
    pub timestamp: i64,
    pub parent_id: u32,
    pub info_pos: u64,
    pub info_length: usize,    
}
unsafe impl Pod for Commit {}
unsafe impl Zeroable for Commit {}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Safely extract tag as string, stopping at first zero byte
        let tag_str = match self.tag_as_str() {
            Ok(s) => s,
            Err(_) => "<invalid UTF-8>",
        };

        // Convert timestamp to readable date (assuming seconds)
        let datetime = DateTime::from_timestamp(self.timestamp, 0)
        .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap());
    

        write!(
            f,
            "Commit {{ tag: \"{}\", id: {}, timestamp: {} ({}), parent_id: {}, info_pos: {}, info_length: {} }}",
            tag_str,
            self.id,
            self.timestamp,
            datetime.format("%Y-%m-%d %H:%M:%S"),
            self.parent_id,
            self.info_pos,
            self.info_length
        )
    }
}

impl Commit {
    pub fn tag_as_str(&self) -> Result<&str, std::str::Utf8Error> {
        // Find the first null (0) byte — if you store C-style null-terminated strings
        let end = self.tag.iter().position(|&b| b == 0).unwrap_or(self.tag.len());
        std::str::from_utf8(&self.tag[..end])
    }
}


#[derive(Debug, Encode, Decode)]
pub struct CommitInfo {
    pub id: u32,
    pub file_info: Vec<FileInfo>
}

impl fmt::Display for CommitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "CommitInfo {{ id: {} }}", self.id)?;
        writeln!(f, "FileInfos ({} items):", self.file_info.len())?;

        for (i, fi) in self.file_info.iter().enumerate() {
            writeln!(f, "  [{}] {}", i, fi)?;
        }

        Ok(())
    }
}

#[derive(Debug, Encode, Decode)]
pub struct FileInfo {
    pub local_path: [u8; 128],
    pub hash: [u8; 256],
    pub package_pos: u64
}

impl fmt::Display for FileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let local_path = match self.local_path_as_str() {
            Ok(s) => s,
            Err(_) => "<invalid UTF-8>",
        };

        let hash = match self.hash_as_str() {
            Ok(s) => s,
            Err(_) => "<invalid UTF-8>",
        };

        write!(
            f,
            "FileInfo {{ local_path: \"{}\", hash: \"{}\", package_pos: {} }}",
            local_path, hash, self.package_pos
        )
    }
}

impl FileInfo {
    pub fn local_path_as_str(&self) -> Result<&str, std::str::Utf8Error> {
        let end = self.local_path.iter().position(|&b| b == 0).unwrap_or(self.local_path.len());
        std::str::from_utf8(&self.local_path[..end])
    }

    pub fn hash_as_str(&self) -> Result<&str, std::str::Utf8Error> {
        let end = self.hash.iter().position(|&b| b == 0).unwrap_or(self.hash.len());
        std::str::from_utf8(&self.hash[..end])
    }
}

use bitcode::{Decode, Encode};
use bytemuck::{Pod, Zeroable};

pub const DIRECTORY_NAME: &str = "minegit";
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

#[derive(Debug, Encode, Decode)]
pub struct CommitInfo {
    pub id: u32,
    pub file_info: Vec<FileInfo>
}

#[derive(Debug, Encode, Decode)]
pub struct FileInfo {
    pub local_path: [u8; 128],
    pub hash: [u8; 256],
}

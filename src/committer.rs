use bitcode::decode;
use bytemuck::from_bytes;
use chrono::Local;
use zstd::{decode_all, encode_all};
use std::error::Error;
use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom};
use std::time::Instant;

use crate::ignore_filter::IgnoreFilter;
use crate::savefiles::{CommitInfo, FileInfo};
use crate::{
    savefiles::{Commit, COMMITS_FILE_NAME, DIRECTORY_NAME, COMMITS_INFO_FILE_NAME},
    utils::fs_utils,
};
pub fn add_commit(target_path: &str, tag: &str, parent_id: u32) -> Result<(), Box<dyn std::error::Error>> {
    let root_path = format!("{target_path}/.{DIRECTORY_NAME}");
    
    // TODO: better path (windows?)
    let commits_path = &format!("{root_path}/{COMMITS_FILE_NAME}");
    let commits_info_path = &format!("{root_path}/{COMMITS_INFO_FILE_NAME}");
    
    // Create commit info
    let commit_info = create_commit_info(&root_path, target_path, 0)?;
    let commit_info_bytes = fs_utils::encode_to_bytes(&commit_info);

    // Compress using zstd
    let compressed_commit_info = encode_all(Cursor::new(commit_info_bytes), 0)?;

    // append commit info file
    let (_, commit_info_pos) = fs_utils::append_file(commits_info_path, &compressed_commit_info)?;
    
    // Create commit
    let commit = create_commit( tag,0, parent_id,commit_info_pos, compressed_commit_info.len())?;
    let commit_bytes = bytemuck::bytes_of(&commit);
    fs_utils::append_file(commits_path, &commit_bytes)?;    
    
    Ok(())
}

pub fn read_all_commits(path: &str) -> io::Result<Vec<Commit>> {
    let mut file = fs_utils::read_file(path)?;
    let commit_size = std::mem::size_of::<Commit>();
    let mut commits = Vec::new();
    let mut buf = Vec::new();

    file.read_to_end(&mut buf)?;

    for chunk in buf.chunks(commit_size) {
        if chunk.len() == commit_size {
            let commit: Commit = *from_bytes(chunk);
            commits.push(commit);
        }
    }

    Ok(commits)
}

pub fn read_commit_info(path: &str, pos: u64, len: usize) -> Result<CommitInfo, Box<dyn Error>> {
    let mut file = fs_utils::read_file(path)?;
    file.seek(SeekFrom::Start(pos))?;

    let mut compressed_buffer = vec![0u8; len as usize];
    file.read_exact(&mut compressed_buffer)?;

    let uncompressed = decode_all(Cursor::new(compressed_buffer))?;

    let commit_info = bitcode::decode(&uncompressed)?;
    Ok(commit_info)
}

fn create_commit_info(root_path: &str, target_path: &str, id: u32)-> Result<CommitInfo, Box<dyn Error>>
{
    let entries = fs_utils::get_all_files_in_directory(&target_path)?;

    let mut file_paths = Vec::new();

    let filter = IgnoreFilter::new(&root_path);

    for entry in entries {
        let full_path = entry.path();
        let path = full_path.strip_prefix(target_path)?;

        if filter.is_ignored(path) {
            continue;
        }

        let pat_buf = path.to_path_buf();
        let path_string = pat_buf.to_str().ok_or("err")?.to_string();
        file_paths.push(path_string); // clone the relative path into owned PathBuf
    }

    let mut files = Vec::new();

    for file_path in file_paths {
        let path = file_path.as_bytes();
        let mut path_bytes = [0u8; 128];
        path_bytes[..path.len()].copy_from_slice(path);

        let hash = fs_utils::file_hash(&file_path)?;
        let mut hash_bytes = [0u8;256];
        hash_bytes[..hash.len()].copy_from_slice(hash.as_bytes());


        files.push(FileInfo { local_path: path_bytes, hash: hash_bytes });
    }

    Ok(CommitInfo{
        id,
        //file_info_length: files.len() as u32,
        file_info: files
    })
}


fn create_commit(
    tag: &str,
    id: u32,
    parent_id: u32,
    info_pos: u64,
    info_length: usize
) -> Result<Commit, Box<dyn Error>> {
    let mut tag_bytes = [0u8; 256];

    // Truncate tag to the first 256 bytes
    let truncated = tag.as_bytes();
    let len = truncated.len().min(256);
    tag_bytes[..len].copy_from_slice(&truncated[..len]);


    Ok(Commit {
        id: id,
        timestamp: Local::now().timestamp(),
        tag: tag_bytes,
        parent_id,
        info_pos,
        info_length
    })
}

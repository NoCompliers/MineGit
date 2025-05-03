use bitcode::decode;
use bytemuck::from_bytes;
use chrono::Local;
use zstd::{decode_all, encode_all};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom, Write};

use crate::ignore_filter::IgnoreFilter;
use crate::recover::diff_gen::DiffGenerator;
use crate::recover::recover::recover;
use crate::recover::snapshot::{self, SnapshotHeader};
use crate::savefiles::{CommitInfo, FileInfo};
use crate::{
    savefiles::{Commit, COMMITS_FILE_NAME, DIRECTORY_NAME, COMMITS_INFO_FILE_NAME},
    utils::fs_utils,
};


fn get_root_path(target_path: &str) -> io::Result<String> {
    fs_utils::build_path([target_path, DIRECTORY_NAME])
}

fn get_commits_path(target_path: &str) -> io::Result<String> {
    fs_utils::build_path([&get_root_path(target_path)?, COMMITS_FILE_NAME])
}

fn get_commits_info_path(target_path: &str) -> io::Result<String> {
    fs_utils::build_path([&get_root_path(target_path)?, COMMITS_INFO_FILE_NAME])
}

pub fn add_commit(target_path: &str, tag: &str, parent_id: u32) -> Result<(), Box<dyn std::error::Error>> {
    let commits_path = get_commits_path(target_path)?;
    let commits_info_path = get_commits_info_path(target_path)?;
    
    // Read previous commits to tie them with new one
    let parent_id = read_all_commits(target_path)?.len() as u32; //TODO: get just size

    // Create commit info
    let commit_info = create_commit_info(target_path, parent_id)?;
    let commit_info_bytes = fs_utils::encode_to_bytes(&commit_info);

    // Compress using zstd
    let compressed_commit_info = encode_all(Cursor::new(commit_info_bytes), 0)?;

    // append commit info file
    let (_, commit_info_pos) = fs_utils::append_file(&commits_info_path, &compressed_commit_info)?;
    
    // Create commit
    let commit = create_commit( tag,parent_id + 1, parent_id, commit_info_pos, compressed_commit_info.len())?;
    let commit_bytes = bytemuck::bytes_of(&commit);
    fs_utils::append_file(&commits_path, &commit_bytes)?;    
    
    Ok(())
}

pub fn print_all_commits(target_path: &str) -> Result<(), Box<dyn Error>>{
    // Get commits
    let commits_info_file = fs_utils::read_file(&get_commits_info_path(target_path)?)?;
    let commits = read_all_commits(&target_path)?;
    
    for commit in commits {
        println!("{}\n{}", commit, read_commit_info(&commits_info_file, commit.info_pos, commit.info_length)?);
    }

    Ok(())
}

pub fn read_all_commits(target_path: &str) -> io::Result<Vec<Commit>> {
    let commits_path = get_commits_path(target_path)?;

    let mut commits = Vec::new();

    // No commit file
    if !fs_utils::is_path_exists(&commits_path)
    {
        return Ok(commits);
    }

    let mut file = fs_utils::read_file(&commits_path)?;
    let commit_size = std::mem::size_of::<Commit>();
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

pub fn restore(target_path: &str, commit_id: u32) -> Result<(), Box<dyn Error>> {
    // Get commits
    let commits = read_all_commits(&target_path)?;
    let commits_info_file = fs_utils::read_file(&get_commits_info_path(target_path)?)?;

    // Get all changed files
    let mut changed_files = HashMap::new();
    for commit in commits.iter().rev() {
        let commit_info = read_commit_info(&commits_info_file, commit.info_pos, commit.info_length)?;

        for file_info in commit_info.file_info
        {
            let file_path = file_info.local_path_as_str()?;
            if file_info.package_pos == 0 && commit.id != commit_id {
                if fs_utils::is_path_exists(file_path) {
                    fs_utils::remove_file(file_path)?;
                }
                changed_files.remove(file_path);
            } else {
                changed_files.insert(file_path.to_owned(), (file_info.package_pos, file_info.hash));
            }
        }

        if commit.id == commit_id {
            break;
        }
    }

    let root_path= get_root_path(&target_path)?;

    // Restore files
    for changed_file in changed_files
    {
        let origin_path = changed_file.0;
        let package_path = fs_utils::build_path([&root_path, "data", &format!("{origin_path}.pkg")])?;

        let mut file = fs_utils::open_to_write(&origin_path, true)?;
        let mut package_file = fs_utils::open_to_write(&package_path, false)?;

        package_file.seek(io::SeekFrom::Start(changed_file.1.0))?;
        let snapshot = SnapshotHeader::deserialize(&mut package_file)?;

        let recovered = recover(&mut package_file, snapshot.clone())?;

        file.write_all(&recovered)?;
    }


    Ok(())
}

pub fn read_commit_info<R: Read + Seek>(mut reader: R, pos: u64, len: usize) -> Result<CommitInfo, Box<dyn Error>> {
    reader.seek(SeekFrom::Start(pos))?;

    let mut compressed_buffer = vec![0u8; len as usize];
    reader.read_exact(&mut compressed_buffer)?;

    let uncompressed = decode_all(Cursor::new(compressed_buffer))?;

    let commit_info = bitcode::decode(&uncompressed)?;
    Ok(commit_info)
}

fn create_commit_info(target_path: &str, id: u32)-> Result<CommitInfo, Box<dyn Error>>
{
    let root_path= get_root_path(&target_path)?;
    let commits_info_file = fs_utils::open_to_write(&get_commits_info_path(target_path)?, false)?;

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

    let mut file_infos = Vec::new();

    for origin_path in file_paths {
        let path = origin_path.as_bytes();
        let mut path_bytes = [0u8; 128];
        path_bytes[..path.len()].copy_from_slice(path);
        
        let hash = fs_utils::file_hash(&origin_path)?;
        let mut hash_bytes = [0u8;256];
        hash_bytes[..hash.len()].copy_from_slice(hash.as_bytes());


        // Check if packageExist
        let output_path = fs_utils::build_path([&root_path, "data", &format!("{origin_path}.pkg")])?;
        let mut package_pos = 0;

        // Check from commits instead
        if fs_utils::is_path_exists(&output_path) {
            let commits = read_all_commits(&target_path)?;

            let mut parent_info: Option<FileInfo> = None;

            'outer: for commit in commits.iter().rev() {
                let commit_info = read_commit_info(&commits_info_file, commit.info_pos, commit.info_length)?;

                // TODO: indexing
                for commit_info_file in commit_info.file_info {
                    if commit_info_file.local_path == path_bytes {
                        parent_info = Some(commit_info_file);
                        break 'outer;
                    }
                }
            }

            
            if let Some(info) = parent_info {
                // Check if file have changed
                if hash_bytes == info.hash {
                    continue;
                }


                // Load parent snapshot
                let mut package = fs_utils::open_to_write(&output_path, false)?;
                print!("{}Size: {}\n",output_path, package.metadata()?.len());
                package.seek(io::SeekFrom::Start(info.package_pos))?;
                let parent_snapshot = SnapshotHeader::deserialize(&mut package)?;
                
                
                // needs whole package and snapshot header
                package.seek(io::SeekFrom::Start(0))?;
                let recovered = recover(&mut package, parent_snapshot.clone())?;
                
                let mut origin = fs_utils::open_to_write(&origin_path, false)?;

                // Generate difference
                let mut diff = DiffGenerator::new();
                let mut diff_data: Vec<u8> = Vec::new();
                let mut cur_data = Cursor::new(recovered);
                diff.init(&mut cur_data, &mut origin)?;
                diff.generate(&mut diff_data)?;
                
                
                
                package.seek(io::SeekFrom::End(0))?;
                
                // Save pos in package
                package_pos = package.stream_position()? as u64;

                // Generate snapshot
                let snapshot = SnapshotHeader {
                    depend_on: parent_snapshot.pos - 25,
                    payload_len: diff_data.len() as u64,
                    file_len: origin.metadata()?.len(),
                    pos: package.stream_position()? as u64, // useless
                    is_zipped: false,
                    is_mca_file: false
                };
                
                // Append snapshot to package
                snapshot.serialize(&mut package)?;
                package.write_all(&diff_data)?;
                
            } else  {
                panic!("parent_info is uninitialized! : {}", output_path);
            }
        } else {
            // Create new package file with original file
            let mut new_package = fs_utils::open_to_write(&output_path, false)?;
            let mut data = Vec::new();
            fs_utils::read_to_end(&origin_path, &mut data)?;
            SnapshotHeader::store_file(&mut new_package, &data, false)?; //TODO: mca?
        }
        file_infos.push(FileInfo { local_path: path_bytes, hash: hash_bytes, package_pos });
    }

    Ok(CommitInfo{
        id,
        //file_info_length: files.len() as u32,
        file_info: file_infos
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

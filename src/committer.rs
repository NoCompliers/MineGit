use bytemuck::{cast_slice, from_bytes};
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::error::Error;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::sync::Arc;
use std::u64::MAX;
use tokio::runtime::Runtime;
use zstd::{decode_all, encode_all};

use crate::ignore_filter::IgnoreFilter;
use crate::recover::snapshot::SnapshotHeader;
use crate::savefiles::{CommitInfo, FileInfo, HEAD_FILE_NAME};
use crate::{
    savefiles::{Commit, COMMITS_FILE_NAME, COMMITS_INFO_FILE_NAME, DIRECTORY_NAME},
    utils::fs_utils,
};

fn get_root_path(target_path: &str) -> io::Result<String> {
    fs_utils::build_path([target_path, DIRECTORY_NAME])
}

fn get_commits_path(target_path: &str) -> io::Result<String> {
    fs_utils::build_path([&get_root_path(target_path)?, COMMITS_FILE_NAME])
}
fn get_head_path(target_path: &str) -> io::Result<String> {
    fs_utils::build_path([&get_root_path(target_path)?, HEAD_FILE_NAME])
}

fn get_commits_info_path(target_path: &str) -> io::Result<String> {
    fs_utils::build_path([&get_root_path(target_path)?, COMMITS_INFO_FILE_NAME])
}

pub fn add_commit(
    target_path: &str,
    tag: &str,
    regions: Vec<[i32; 3]>,
) -> Result<(), Box<dyn std::error::Error>> {
    let commits_path = get_commits_path(target_path)?;
    let commits_info_path = get_commits_info_path(target_path)?;

    // Get current head
    let mut parent_id = 0;
    match get_head(target_path) {
        Ok(value) => {
            parent_id = value;
        }
        Err(_) => {
            write_head(target_path, 0)?;
        }
    }
    // Get commit id
    let id = get_commit_count(target_path)?;
    // Create commit info
    let rt = Runtime::new().unwrap();
    let commit_info = rt.block_on(create_commit_info(target_path, id, parent_id, regions))?;

    let commit_info_bytes = fs_utils::encode_to_bytes(&commit_info);

    // Compress using zstd
    let compressed_commit_info = encode_all(Cursor::new(commit_info_bytes), 0)?;

    // append commit info file
    let (_, commit_info_pos) = fs_utils::append_file(&commits_info_path, &compressed_commit_info)?;

    // Create commit
    let commit = create_commit(
        tag,
        id,
        parent_id,
        commit_info_pos,
        compressed_commit_info.len(),
    )?;
    let commit_bytes = bytemuck::bytes_of(&commit);
    fs_utils::append_file(&commits_path, &commit_bytes)?;

    write_head(target_path, id)?;
    Ok(())
}

pub fn get_head(target_path: &str) -> Result<u32, Box<dyn Error>> {
    let mut head_file = fs_utils::read_file(&get_head_path(target_path)?)?;
    let mut buffer = [0u8; 4];

    head_file.read_exact(&mut buffer)?;
    let num = bytemuck::cast_slice::<u8, u32>(&buffer)[0];
    Ok(num)
}

pub fn write_head(target_path: &str, value: u32) -> Result<(), Box<dyn Error>> {
    let mut file = fs_utils::open_to_write(&get_head_path(target_path)?, true)?;
    let arr = [value];
    let bytes = cast_slice::<u32, u8>(&arr);
    file.write_all(bytes)?;
    Ok(())
}

pub fn print_all_commits(target_path: &str) -> Result<(), Box<dyn Error>> {
    // Get commits
    let commits_info_file = fs_utils::read_file(&get_commits_info_path(target_path)?)?;
    let commits = read_all_commits(&target_path)?;

    for commit in commits {
        let commit_info =
            read_commit_info(&commits_info_file, commit.info_pos, commit.info_length)?;

        let datetime = DateTime::from_timestamp(commit.timestamp, 0)
            .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap());

        println!(
            "Commit:\t\t{}\nId:\t\t{}\nParent commit:\t{}\nDate:\t\t{}\nFiles:\t\t{}\n--------",
            commit.tag_as_str()?,
            commit.id,
            commit.parent_id,
            datetime.format("%Y-%m-%d %H:%M:%S"),
            commit_info.file_info.len(),
        );
    }

    Ok(())
}

fn get_commit_count(target_path: &str) -> io::Result<u32> {
    let commits_path = get_commits_path(target_path)?;

    // No commit file
    if !fs_utils::is_path_exists(&commits_path) {
        return Ok(0);
    }

    let file_size = fs_utils::get_file_size(commits_path)?;
    let commit_size = std::mem::size_of::<Commit>() as u64;

    Ok((file_size / commit_size) as u32)
}

fn get_commit_by_id(target_path: &str, id: u32) -> io::Result<Commit> {
    let commits_path = get_commits_path(target_path)?;
    let commit_size = std::mem::size_of::<Commit>() as u32;

    let mut file = fs_utils::read_file(&commits_path)?;
    file.seek(SeekFrom::Start((commit_size * id) as u64))?;
    let mut buf = vec![0u8; commit_size as usize];
    file.read_exact(&mut buf)?;
    let commit: Commit = *from_bytes(&buf);

    Ok(commit)
}

pub fn read_all_commits(target_path: &str) -> io::Result<Vec<Commit>> {
    let commits_path = get_commits_path(target_path)?;

    let mut commits = Vec::new();

    // No commit file
    if !fs_utils::is_path_exists(&commits_path) {
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

pub fn restore(
    target_path: &str,
    commit_id: u32,
    regions: Vec<[i32; 3]>,
) -> Result<(), Box<dyn Error>> {
    // Get commit
    let commit = get_commit_by_id(target_path, commit_id)?;
    let commit_info_file = fs_utils::read_file(&get_commits_info_path(target_path)?)?;
    let mut commit_info = read_commit_info(&commit_info_file, commit.info_pos, commit.info_length)?;

    // Delete unnecessary files
    let root_path = get_root_path(&target_path)?;
    let mut file_paths = get_not_ignored_files_in_directory(&target_path)?;
    if regions.len() == 0 {
        for entry in file_paths {
            if !commit_info
                .file_info
                .contains_key(&str_to_fixed_bytes::<128>(&entry))
            {
                if fs_utils::is_path_exists(&entry) {
                    fs_utils::remove_file(&entry)?;
                }
            }
        }
    } else {
        // Clean other files files
        file_paths.retain(|file| {
            path_is_in_regions(file, &regions)
                && commit_info
                    .file_info
                    .contains_key(&str_to_fixed_bytes::<128>(&file))
        });

        commit_info
            .file_info
            .retain(|k, _| file_paths.contains(&fixed_bytes_to_str(k)));

        println!("Restored files: {:?}", file_paths);
    }

    // Restore files
    for file_info in commit_info.file_info {
        let origin_path = fixed_bytes_to_str(&file_info.0);
        let package_path =
            fs_utils::build_path([&root_path, "data", &format!("{origin_path}.pkg")])?;

        let mut file = fs_utils::open_to_write(&origin_path, true)?;
        let mut package_file = fs_utils::open_to_write(&package_path, false)?;

        package_file.seek(io::SeekFrom::Start(file_info.1.package_pos))?;
        let snapshot = SnapshotHeader::deserialize(&mut package_file)?;

        let recovered = snapshot.recover(&mut package_file)?;
        file.write_all(&recovered)?;
    }

    write_head(target_path, commit_id)?;

    Ok(())
}

fn path_is_in_regions(path: &str, regions: &Vec<[i32; 3]>) -> bool {
    if !path.ends_with(".mca") || !path.contains("r.") {
        return false;
    }

    // Extract dimension from path
    let dim = if path.starts_with("DIM1") {
        1
    } else if path.starts_with("DIM-1") {
        -1
    } else if path.starts_with("region") || path.starts_with("poi") || path.starts_with("entities")
    {
        0
    } else {
        return false; // Unknown dimension
    };

    // Remove ".mca" and split by '.'
    let trimmed = &path[..path.len() - 4];
    let parts: Vec<&str> = trimmed.split('.').collect();

    if parts.len() != 3 {
        return false;
    }

    let x = parts[1].parse::<i32>().ok();
    let z = parts[2].parse::<i32>().ok();

    match (x, z) {
        (Some(x), Some(z)) => regions
            .iter()
            .any(|f| f[1] == x && f[2] == z && f[0] == dim),
        _ => false,
    }
}

pub fn read_commit_info<R: Read + Seek>(
    mut reader: R,
    pos: u64,
    len: usize,
) -> Result<CommitInfo, Box<dyn Error>> {
    reader.seek(SeekFrom::Start(pos))?;

    let mut compressed_buffer = vec![0u8; len as usize];
    reader.read_exact(&mut compressed_buffer)?;

    let uncompressed = decode_all(Cursor::new(compressed_buffer))?;

    let commit_info = bitcode::decode(&uncompressed)?;
    Ok(commit_info)
}

fn get_not_ignored_files_in_directory(target_path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let root_path = get_root_path(&target_path)?;
    let entries = fs_utils::get_all_files_in_directory(&target_path)?;

    let mut file_paths = Vec::new();

    let filter = IgnoreFilter::new(&root_path);

    // Get all files in directory
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

    Ok(file_paths)
}

fn str_to_fixed_bytes<const N: usize>(s: &str) -> [u8; N] {
    let as_bytes = s.as_bytes();
    let mut bytes = [0u8; N];
    let len = as_bytes.len().min(N);
    bytes[..len].copy_from_slice(&as_bytes[..len]);
    bytes
}

fn fixed_bytes_to_str<const N: usize>(bytes: &[u8; N]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(N);
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

async fn create_commit_info(
    target_path: &str,
    id: u32,
    parent_id: u32,
    regions: Vec<[i32; 3]>,
) -> Result<CommitInfo, Box<dyn Error>> {
    let root_path = get_root_path(&target_path)?;
    let commits_info_file = fs_utils::open_to_write(&get_commits_info_path(target_path)?, false)?;

    let file_paths = get_not_ignored_files_in_directory(&target_path)?;

    let mut file_infos = HashMap::new();

    let mut parent_info: Option<CommitInfo> = None;

    if get_commit_count(target_path)? > 0 {
        //let parent_commit = commits[parent_id as usize];
        let parent_commit = get_commit_by_id(target_path, parent_id)?;
        parent_info = Some(read_commit_info(
            &commits_info_file,
            parent_commit.info_pos,
            parent_commit.info_length,
        )?);
    }

    let mut handels = vec![];

    struct Res {
        k: [u8; 128],
        v: FileInfo,
    }

    let root = Arc::new(root_path);
    let p_inf = Arc::new(parent_info);
    let regions = Arc::new(regions);

    for origin_path in file_paths {
        let origin_p = Arc::new(origin_path);

        let regions = Arc::clone(&regions);
        let root = Arc::clone(&root);
        let p_inf = Arc::clone(&p_inf);

        let handle = tokio::spawn(async move {
            let root_path = Arc::as_ref(&root);
            let origin_path = Arc::as_ref(&origin_p);
            let parent_info = Arc::as_ref(&p_inf);

            let path_bytes = str_to_fixed_bytes::<128>(&origin_path);
            let hash_bytes = str_to_fixed_bytes::<256>(&fs_utils::file_hash(&origin_path).unwrap());

            let include_in_commit = regions.len() == 0 || path_is_in_regions(origin_path, &regions);

            // Check if packageExist
            let output_path =
                fs_utils::build_path([&root_path, "data", &&format!("{}.pkg", origin_path)])
                    .unwrap();

            if fs_utils::is_path_exists(&output_path) {
                if let Some(ref parent_info) = parent_info {
                    let parent_file_info = parent_info
                        .file_info
                        .get(&path_bytes)
                        .ok_or("parent_file_info is uninitialized")
                        .unwrap();

                    // Check if file have changed
                    if hash_bytes == parent_file_info.hash || !include_in_commit {
                        return Res {
                            k: path_bytes,
                            v: FileInfo {
                                hash: hash_bytes,
                                package_pos: parent_file_info.package_pos,
                            },
                        };
                    }

                    // Load parent snapshot
                    let mut package = fs_utils::open_to_write(&output_path, false).unwrap();
                    package
                        .seek(io::SeekFrom::Start(parent_file_info.package_pos))
                        .unwrap();
                    let parent_snapshot = SnapshotHeader::deserialize(&mut package).unwrap();

                    let mut origin = fs_utils::open_to_write(&origin_path, false).unwrap();
                    let mut origin_data: Vec<u8> = Vec::new();
                    origin.read_to_end(&mut origin_data).unwrap();
                    let new_snap = parent_snapshot.update(&mut package, &origin_data).unwrap();

                    return Res {
                        k: path_bytes,
                        v: FileInfo {
                            hash: hash_bytes,
                            package_pos: new_snap.pos - SnapshotHeader::SERIZIZED_SIZE as u64,
                        },
                    };
                } else {
                    panic!("parent_info is uninitialized");
                }
            } else if include_in_commit {
                // Create new package file with original file
                let mut new_package = fs_utils::open_to_write(&output_path, false).unwrap();
                let mut data = Vec::new();
                fs_utils::read_to_end(&origin_path, &mut data).unwrap();
                SnapshotHeader::save_new(&mut new_package, &data).unwrap();

                return Res {
                    k: path_bytes,
                    v: FileInfo {
                        hash: hash_bytes,
                        package_pos: 0,
                    },
                };
            } else {
                return Res {
                    k: path_bytes,
                    v: FileInfo {
                        hash: hash_bytes,
                        package_pos: MAX,
                    },
                };
            }
        });
        handels.push(handle);
    }

    for handle in handels {
        let res = (handle.await)?;
        if res.v.package_pos != MAX {
            file_infos.insert(res.k, res.v);
        }
    }

    Ok(CommitInfo {
        id,
        file_info: file_infos,
    })
}

fn create_commit(
    tag: &str,
    id: u32,
    parent_id: u32,
    info_pos: u64,
    info_length: usize,
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
        info_length,
    })
}

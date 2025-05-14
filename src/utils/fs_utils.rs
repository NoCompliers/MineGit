use std::fs::{self, create_dir_all, DirEntry};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use bitcode::Encode;
use sha2::{Digest, Sha256};

pub fn is_path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

fn path_to_string(path: &Path) -> io::Result<String> {
    let path_str = path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Failed to convert path to string",
        )
    })?;
    Ok(path_str.to_string())
}

pub fn get_file_size<P: AsRef<Path>>(path: P) -> io::Result<u64> {
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.len())
}

pub fn build_path<I: IntoIterator<Item = S>, S: AsRef<Path>>(sequence: I) -> io::Result<String> {
    let mut path = PathBuf::new();
    for part in sequence {
        path.push(part);
    }
    path_to_string(&path)
}

pub fn get_current_path() -> io::Result<String> {
    let path = std::env::current_dir()?;
    path_to_string(&path)
}

pub fn make_dir(path: &str) -> io::Result<()> {
    fs::create_dir(&path)?;
    Ok(())
}

pub fn remove_file(path: &str) -> io::Result<()> {
    fs::remove_file(&path)?;
    Ok(())
}

pub fn write_file(path: &str, buf: &[u8]) -> io::Result<File> {
    let mut file = File::create(path)?;
    file.write_all(buf)?;
    Ok(file)
}

pub fn append_file(path: &str, buf: &[u8]) -> io::Result<(File, u64)> {
    let mut file = OpenOptions::new()
        .create(true)
        .read(true) // Needed for seeking
        .write(true) // Needed for writing
        .open(path)?;

    let pos = file.seek(SeekFrom::End(0))?; // Get current file size (start of append)
    file.write_all(buf)?;
    Ok((file, pos))
}

pub fn open_to_write(path: &str, truncate: bool) -> io::Result<File> {
    let path_obj = Path::new(path);

    // Ensure parent directories exist
    if let Some(parent) = path_obj.parent() {
        create_dir_all(parent)?; // Creates all missing directories in the path
    }

    // Open the file with write options
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(truncate)
        .open(path_obj)
}

pub fn read_file(path: &str) -> Result<File, io::Error> {
    File::open(path)
}

pub fn read_to_end(path: &str, buf: &mut Vec<u8>) -> Result<usize, io::Error> {
    let mut file = read_file(&path)?;

    file.read_to_end(buf)
}

pub fn files_equal(
    path1: &str,
    path2: &str,
    compare_metadata: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Check metadata
    if compare_metadata {
        // Get files metadata
        let meta1 = fs::metadata(path1)?;
        let meta2 = fs::metadata(path2)?;
        // Compare metadata
        if meta1.modified()? != meta2.modified()? {
            return Ok(false);
        }
    }
    // Compare file content hash
    if file_hash(path1)? != file_hash(path2)? {
        return Ok(false);
    }
    // Files are equal
    Ok(true)
}

pub fn get_all_files_in_directory(root_path: &str) -> io::Result<Vec<DirEntry>> {
    let cur_dir_entries = fs::read_dir(root_path)?;
    let mut paths: Vec<DirEntry> = Vec::new();
    for entry in cur_dir_entries {
        let entry = entry?;
        if entry.path().is_dir() {
            let mut inner_entries = get_all_files_in_directory(entry.path().to_str().unwrap())?;

            paths.append(&mut inner_entries);
        } else {
            paths.push(entry);
        }
    }
    Ok(paths)
}

pub fn file_hash(path: &str) -> io::Result<String> {
    // Create hasher
    let mut hasher = Sha256::new();
    // Open file
    let mut file = fs::File::open(path)?;
    // Create and return file hash
    let _n = io::copy(&mut file, &mut hasher)?;

    let bytes = hasher.finalize();
    Ok(bytes.iter().map(|b| format!("{:02X}", b)).collect())
}

pub fn encode_to_bytes<T: Encode>(content: &T) -> Vec<u8> {
    bitcode::encode(content)
}

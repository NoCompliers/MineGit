use std::fs::{self, DirEntry};
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use bitcode::{Decode, DecodeOwned, Encode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub fn is_path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

pub fn get_path() -> io::Result<String> {
    let path = std::env::current_dir()?;
    let path_str = path.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::Other, "Failed to convert path to string")
    })?;
    Ok(path_str.to_string())
}

pub fn make_dir(path: &str, name: &str, hidden: bool) -> io::Result<String> {
    // Create the path string
    let full_path = format!("{}/{}{}", path, if hidden { "." } else { "" }, name);

    // Check if directory already exists and create if it is not
    if !is_path_exists(&full_path) {
        fs::create_dir(&full_path)?;
    }

    Ok(full_path)
}

pub fn write_file(path: &str, buf: &[u8]) -> io::Result<File> {
    let mut file = File::create(path)?;
    file.write_all(buf)?;
    Ok(file)
}

pub fn append_file(path: &str, buf: &[u8]) -> io::Result<(File, u64)> {
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)   // Needed for seeking
        .write(true)  // Needed for writing
        .open(path)?;

    let pos = file.seek(SeekFrom::End(0))?; // Get current file size (start of append)
    file.write_all(buf)?;
    Ok((file, pos))
}

pub fn write_json_file<T: Serialize>(
    dir_path: &str,
    name: &str,
    hidden: bool,
    content: &T,
) -> io::Result<File> {
    // Serialize provided content
    let json = serde_json::to_string(content)?;

    // Create file path
    let path = format!("{}/{}{}", dir_path, if hidden { "." } else { "" }, name);

    // Write json to file
    write_file(&path, json.as_bytes())
}

pub fn read_file(path: &str) -> Result<File, io::Error> {
    File::open(path)
}

pub fn read_json_file<T: DeserializeOwned>(path: &str) -> Result<T, Box<dyn std::error::Error>> {
    let file = read_file(path)?; // Propagate IO errors
    let reader = BufReader::new(file);
    let value = serde_json::from_reader(reader)?; // Propagate JSON parsing errors
    Ok(value)
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

pub fn decode_bytes<T: DecodeOwned>(file: &mut File) -> Result<T, Box<dyn std::error::Error>> {
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let decoded = bitcode::decode(&buffer)?;
    Ok(decoded)
}

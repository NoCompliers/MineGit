use std::fs;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use serde::Serialize;
use sha2::{Digest, Sha256};

pub fn is_path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

pub fn get_path() -> io::Result<PathBuf> {
    std::env::current_dir()
}

pub fn create_dir(path: &str, name: &str, hidden: bool) -> io::Result<String> {
    // Create the path string
    let path = format!("{}/{}{}", path, if hidden { "." } else { "" }, name);

    // Check if directory already exists and create if it is not
    if !is_path_exists(&path) {
        fs::create_dir(&path)?;
    }

    Ok(path)
}

pub fn create_file<T: Serialize>(path: &str, name: &str, hidden: bool, content: &T) {
    // Serialize provided content
    let json = serde_json::to_string(content).unwrap();

    // Create file path
    let path = format!("{}/{}{}", path, if hidden { "." } else { "" }, name);
    // Create file
    let mut file = File::create(path).expect("File creation failed!");

    // Write json to file
    file.write_all(json.as_bytes()).expect("Json write failed!");
}

pub fn files_equal(path1: &str, path2: &str, compare_metadata: bool) -> Result<bool, String> {
    // Check if paths are valid
    for p in [path1, path2] {
        if !is_path_exists(&p) {
            return Err(format!("File {} not exist!", &p));
        }
    }

    // Check metadata
    if compare_metadata {
        // Get files metadata
        let meta1 = fs::metadata(path1).unwrap();
        let meta2 = fs::metadata(path2).unwrap();
        // Compare metadata
        if meta1.modified().unwrap() != meta2.modified().unwrap() {
            return Ok(false);
        }
    }
    // Compare file content hash
    if file_hash(path1) != file_hash(path2) {
        return Ok(false);
    }
    // Files are equal
    Ok(true)
}

pub fn file_hash(path: &str) -> Vec<u8> {
    // Create hasher
    let mut hasher = Sha256::new();
    // Open file
    let mut file = fs::File::open(path).unwrap();
    // Create and return file hash
    let _n = io::copy(&mut file, &mut hasher).unwrap();
    hasher.finalize().to_ascii_uppercase()
}

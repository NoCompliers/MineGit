use std::fs;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use serde::Serialize;

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

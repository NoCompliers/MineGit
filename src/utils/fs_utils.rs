use std::fs;
use std::io;
use std::path::PathBuf;

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

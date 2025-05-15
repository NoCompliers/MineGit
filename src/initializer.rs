use std::error::Error;

use crate::{
    committer::add_commit,
    savefiles::{DIRECTORY_NAME, IGNORE_FILE_NAME},
    utils::fs_utils::{self},
};

pub fn init(target_path: &str) -> Result<(), Box<dyn Error>> {
    let dir_path = fs_utils::build_path([&target_path, DIRECTORY_NAME])?;

    // Check if repo is exists
    if fs_utils::is_path_exists(&dir_path) {
        return Err(".minegit directory already exists!".into());
    }

    // Create a directory
    fs_utils::make_dir(&dir_path)?;
    // Create ignore file
    let patterns = [".git/*", "target/*", ".minegit/*", "src/*"];

    fs_utils::write_file(
        &format!("{dir_path}/{IGNORE_FILE_NAME}"),
        patterns.join("\n").as_bytes(),
    )?;

    add_commit(&target_path, "Initial Commit.", Vec::new())?;
    Ok(())
}

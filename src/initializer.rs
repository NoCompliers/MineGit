use std::error::Error;

use crate::{
    committer::add_commit,
    savefiles::{Commit, COMMITS_FILE_NAME, DIRECTORY_NAME, IGNORE_FILE_NAME},
    utils::fs_utils::{self, write_json_file},
};

pub fn init(target_path: &str) -> Result<(), Box<dyn Error>> {
    //TODO: check if repo is exists

    // Create a directory
    let dir_path = fs_utils::make_dir(&target_path, DIRECTORY_NAME, true)?;

    // Create ignore file
    let patterns = [".git/*", "target/*"];

    fs_utils::write_file(
        &format!("{dir_path}/{IGNORE_FILE_NAME}"),
        patterns.join("\n").as_bytes(),
    )?;

    add_commit(&target_path, "Initial Commit.",0)?;
    Ok(())
}

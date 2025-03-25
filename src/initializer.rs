use crate::{
    savefiles::Config,
    utils::fs_utils::{self, create_file},
};

pub fn init() {
    // Get current path
    let path = match fs_utils::get_path() {
        Ok(x) => x.to_str().unwrap().to_owned(), // Convert to owned String
        Err(e) => {
            init_error(e.to_string());
            return; // Early return on error
        }
    };

    // Create a directory
    if let Err(e) = fs_utils::create_dir(&path, "minegit", true) {
        init_error(e.to_string());
        return;
    }

    // Create config file
    let conf = Config {
        ignored_paths: Vec::new(),
    };
    create_file(&path, "config", false, &conf);
}

fn init_error(error: String) {
    eprintln!("{error}");
    panic!();
}

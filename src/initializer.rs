use crate::utils::fs_utils;

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
    let _ = fs_utils::create_dir(&path, "testName", false);
    // TODO: Handle result
}

fn init_error(error: String) {
    eprintln!("{error}");
    panic!();
}

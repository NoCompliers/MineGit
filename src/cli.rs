use crate::args::*;
use crate::initializer;
use crate::utils::fs_utils;

use clap::Parser;
// Runs the CLI application
pub fn run() {
    let args = MineGitArgs::parse();

    // Handle arguments
    match args.command {
        Commands::Init => {
            println!("Init called");
            initializer::init();
        }
        Commands::Compare(args) => {
            if !fs_utils::is_path_exists(&args.path1) {
                println!("File {} not exist!", &args.path1);
                return;
            }
            if !fs_utils::is_path_exists(&args.path2) {
                println!("File {} not exist!", &args.path2);
                return;
            }

            // Compare files
            let equal = fs_utils::files_equal(&args.path1, &args.path2, args.meta);
            println!("Files are{} equal.", if equal { "" } else { " not" });
        }
    }
}

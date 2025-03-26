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
            // Compare files
            let equal = match fs_utils::files_equal(&args.path1, &args.path2, args.meta) {
                Ok(x) => x,
                Err(e) => panic!("{e}"),
            };
            println!("Files are{} equal.", if equal { "" } else { " not" });
        }
    }
}

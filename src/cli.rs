use crate::args::*;
use crate::committer;
use crate::initializer;
use crate::utils::fs_utils;

use clap::Parser;
// Runs the CLI application
pub fn run() {
    let args = MineGitArgs::parse();

    // Get current path
    let root_path = fs_utils::get_current_path().unwrap();

    // Handle arguments
    match args.command {
        Commands::Init => {
            println!("Init called");
            initializer::init(&root_path).unwrap_or_else(|e| println!("{e}"));
        }
        Commands::Commit(args) => {
            committer::add_commit(&root_path, &args.tag, args.regions).unwrap();
        }
        Commands::List => {
            committer::print_all_commits(&root_path).unwrap();
        }
        Commands::Restore(args) => {
            committer::restore(&root_path, args.id, args.regions).unwrap();
        }
        Commands::Compare(args) => {
            // Compare files
            let equal = fs_utils::files_equal(&args.path1, &args.path2, args.meta)
                .unwrap_or_else(|e| panic!("{e}"));
            println!("Files are{} equal.", if equal { "" } else { " not" });
        }
    }
}

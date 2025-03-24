use crate::args::*;
use crate::initializer;

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
    }
}

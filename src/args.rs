use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[clap(author, version, about)]
// Program arguments
pub struct MineGitArgs {
    #[clap(subcommand)]
    pub command: Commands,
}
// Command types
#[derive(Debug, Subcommand)]
pub enum Commands {
    Init,
}

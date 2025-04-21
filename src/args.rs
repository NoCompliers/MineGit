use clap::{Args, Parser, Subcommand};

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
    Status,
    Commit(CommitArgs),
    Compare(CompareArgs),
}

#[derive(Debug, Args)]
pub struct CompareArgs {
    pub path1: String,
    pub path2: String,
    #[clap(short, long, default_value_t = false)]
    pub meta: bool,
}

#[derive(Debug, Args)]
pub struct CommitArgs {
    pub tag: String,
}
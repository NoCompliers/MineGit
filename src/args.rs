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
    /// Create repo
    Init,
    /// List all commits
    List,
    /// Restore specific commit
    Restore(RestoreArgs),
    /// Add new commit
    Commit(CommitArgs),
    /// Compare hashes of 2 files
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
    /// Commit tag
    pub tag: String,

    /// List of 3-element integer arrays dimension,x,z (e.g. --regions -1,0,0 0,1,0)
    #[clap(short, long, value_parser=parse_region, num_args=1.., value_delimiter = ' ', allow_hyphen_values = true)]
    pub regions: Vec<[i32; 3]>,
}

#[derive(Debug, Args)]
pub struct RestoreArgs {
    /// Commit id
    pub id: u32,

    /// List of 3-element integer arrays dimension,x,z (e.g. --regions -1,0,0 0,1,0)
    #[clap(short, long, value_parser=parse_region, num_args=1.., value_delimiter = ' ', allow_hyphen_values = true)]
    pub regions: Vec<[i32; 3]>,
}

// Custom parser for [i32; 3]
fn parse_region(s: &str) -> Result<[i32; 3], String> {
    let parts: Vec<_> = s.split(',').collect();
    if parts.len() != 3 {
        return Err("Each region must have exactly three comma-separated integers".into());
    }

    let nums: Result<Vec<_>, _> = parts.iter().map(|p| p.trim().parse::<i32>()).collect();

    match nums {
        Ok(v) if v.len() == 3 => Ok([v[0], v[1], v[2]]),
        _ => Err("Failed to parse all three integers".into()),
    }
}

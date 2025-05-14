use std::env;
use std::fs::OpenOptions;
use std::io::{self, Read, Seek};
use std::{fs::File, io::Write};

mod args;
mod cli;
mod committer;
mod ignore_filter;
mod initializer;
mod savefiles;
mod utils;

mod recover {
    pub mod diff;
    pub mod diff_gen;
    pub mod recover;
    pub mod snapshot;
}

use recover::recover::recover;

use crate::recover::diff_gen::DiffGenerator;
use crate::recover::snapshot::{SnapshotHeader, SNAPSHOT_HEADER_SIZE};

fn main() {
    env::set_var("RUST_BACKTRACE", "full");
    //test().unwrap();
    cli::run();
}

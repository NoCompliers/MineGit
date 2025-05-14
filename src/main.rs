use std::env;

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
    mod test;
}

fn main() {
    env::set_var("RUST_BACKTRACE", "1");
    cli::run();
}
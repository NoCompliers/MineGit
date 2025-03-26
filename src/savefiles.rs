use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub ignored_paths: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Commits {
    pub tag: String,
    pub description: String,
    pub storage_path: String,
}

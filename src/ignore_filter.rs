use std::{io::Read, path::Path};

use glob::Pattern;

use crate::savefiles::IGNORE_FILE_NAME;
use crate::utils::fs_utils;

pub struct IgnoreFilter {
    patterns: Vec<String>,
    negated_patterns: Vec<String>,
}

impl IgnoreFilter {
    // Constructor to create a new IgnoreFilter from a list of patterns
    pub fn new(root_path: &str) -> Self {
        let mut file = fs_utils::read_file(&format!("{root_path}/{IGNORE_FILE_NAME}")).unwrap();

        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        let mut patterns_vec = Vec::new();
        let mut negated_patterns_vec = Vec::new();

        for pattern in lines {
            if pattern.starts_with('!') {
                negated_patterns_vec.push(pattern[1..].to_string());
            } else {
                patterns_vec.push(pattern.to_string());
            }
        }

        Self {
            patterns: patterns_vec,
            negated_patterns: negated_patterns_vec,
        }
    }

    // Check if a file or directory should be ignored based on the patterns
    pub fn is_ignored(&self, path: &Path) -> bool {
        for pattern in &self.negated_patterns {
            let glob = Pattern::new(pattern).unwrap();
            if glob.matches_path(path) {
                return false;
            }
        }

        for pattern in &self.patterns {
            let glob = Pattern::new(pattern).unwrap();
            if glob.matches_path(path) {
                return true;
            }
        }

        false
    }
    // TODO: filter list of paths
}

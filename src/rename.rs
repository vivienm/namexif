use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;

pub struct Rename {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
}

impl Rename {
    pub fn new(source_path: PathBuf, target_path: PathBuf) -> Self {
        Self {
            source_path,
            target_path,
        }
    }

    pub fn apply(&self) -> Result<(), io::Error> {
        fs::rename(&self.source_path, &self.target_path)
    }
}

impl fmt::Display for Rename {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} => {}",
            self.source_path.to_str().unwrap(),
            self.target_path.to_str().unwrap()
        )
    }
}

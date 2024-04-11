use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

mod path;
pub use path::*;

mod hash;
pub use hash::*;

#[derive(Serialize, Deserialize)]
pub struct Database {
    source_path: PathBuf,
    mirror_path: PathBuf,
    // Key = Source, Value = Hash
    hashes: BTreeMap<PathBuf, String>,
}

impl Database {
    pub fn new(source_path: PathBuf, mirror_path: PathBuf) -> Self {
        let hashes = BTreeMap::new();
        Self {
            source_path,
            mirror_path,
            hashes,
        }
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn mirror_path(&self) -> &Path {
        &self.mirror_path
    }

    pub fn hashes(&self) -> &BTreeMap<PathBuf, String> {
        &self.hashes
    }

    pub fn set_hashes(&mut self, hashes: BTreeMap<PathBuf, String>) {
        self.hashes = hashes;
    }

    pub fn save(&self) -> Result<(), String> {
        let database_path = database_path_from_mirror(&self.mirror_path)?;
        self.write_to_file(&database_path)
    }

    pub fn load(file_path: &Path) -> Result<Self, String> {
        let mut file = File::open(file_path)
            .map_err(|e| format!("Failed to open {0} for writing: {e}", file_path.display()))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read file {0}: {e}", file_path.display()))?;
        serde_json::from_str(&buf).map_err(|e| format!("Failed to read database from file: {e}"))
    }

    fn write_to_file(&self, file_path: &Path) -> Result<(), String> {
        let file = File::create(file_path)
            .map_err(|e| format!("Failed to open {0} for writing: {e}", file_path.display()))?;
        serde_json::to_writer_pretty(file, self)
            .map_err(|e| format!("Failed to format database to json: {e}"))
    }
}

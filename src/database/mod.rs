mod hash;
mod path;

pub use hash::*;
pub use path::*;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    filter::{find_filter_for_entry, run_filter_for_entry},
    walkdir_result_extension::WalkdirResultExtension,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{copy, create_dir_all, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize)]
pub struct Database {
    filters: Vec<String>,
    source_path: PathBuf,
    mirror_path: PathBuf,
    // Key = Source, Value = Hash
    hashes: BTreeMap<PathBuf, String>,
}

impl Database {
    pub fn new(source_path: PathBuf, mirror_path: PathBuf, filters: Vec<String>) -> Self {
        let hashes = BTreeMap::new();
        Self {
            source_path,
            mirror_path,
            hashes,
            filters,
        }
    }

    pub fn load(file_path: &Path) -> Result<Self, String> {
        let mut file = File::open(file_path)
            .map_err(|e| format!("Failed to open {0} for writing: {e}", file_path.display()))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read file {0}: {e}", file_path.display()))?;
        serde_json::from_str(&buf).map_err(|e| format!("Failed to read database from file: {e}"))
    }

    pub fn sync(&mut self) -> Result<(), String> {
        let new_hashes = Arc::new(Mutex::new(BTreeMap::new()));
        let mirror_list = Arc::new(Mutex::new(BTreeSet::new()));

        // Walk source directory
        let source_entries = WalkDir::new(&self.source_path)
            .into_iter()
            .collect::<Vec<_>>();
        source_entries
            .into_par_iter()
            .try_for_each(|entry| -> Result<(), String> {
                let source_entry = entry.handle_to_string()?.into_path();

                let parts = self.source_path.components().count();

                let mut mirror_entry = self
                    .mirror_path
                    .join(source_entry.components().skip(parts).collect::<PathBuf>());
                let filter = find_filter_for_entry(&source_entry, &mut mirror_entry, &self.filters);
                let mirror_entry = mirror_entry;

                {
                    let mut mirror_list = mirror_list
                        .lock()
                        .map_err(|e| format!("Failed to lock mirror list: {e}"))?;
                    mirror_list.insert(mirror_entry.clone());
                }

                if source_entry.is_dir() {
                    self.handle_dir_entry(&source_entry, &mirror_entry)?;
                } else if source_entry.is_file() {
                    self.handle_file_entry(
                        new_hashes.clone(),
                        filter,
                        &source_entry,
                        &mirror_entry,
                    )?;
                }

                Ok(())
            })?;

        self.hashes = new_hashes
            .lock()
            .map_err(|e| format!("Failed to lock new hashes for database: {e}"))?
            .clone();
        self.save()?;

        let mirror_list = mirror_list
            .lock()
            .map_err(|e| format!("Failed to lock mirror list for cleanup: {e}"))?;

        self.cleanup(&mirror_list)
    }

    fn save(&self) -> Result<(), String> {
        let database_path = database_path_from_mirror(&self.mirror_path)?;
        self.write_to_file(&database_path)
    }

    fn write_to_file(&self, file_path: &Path) -> Result<(), String> {
        let file = File::create(file_path)
            .map_err(|e| format!("Failed to open {0} for writing: {e}", file_path.display()))?;
        serde_json::to_writer_pretty(file, self)
            .map_err(|e| format!("Failed to format database to json: {e}"))
    }

    fn handle_file_entry(
        &self,
        hashes: Arc<Mutex<BTreeMap<PathBuf, String>>>,
        filter: Option<&String>,
        source: &Path,
        mirror: &Path,
    ) -> Result<(), String> {
        create_dir_all(mirror.parent().ok_or("Failed to get file parent")?).map_err(|e| {
            format!(
                "Failed to create mirror directory ({0}) for entry `{1}`: {e}",
                mirror.display(),
                source.display()
            )
        })?;

        let digest = hash_file(source)?;

        {
            let mut hashes = hashes
                .lock()
                .map_err(|e| format!("Failed to lock new hashes: {e}"))?;
            hashes.insert(source.to_path_buf(), digest.clone());
        }
        if let Some(prev_hash) = self.hashes.get(source) {
            if mirror.exists() {
                if &digest == prev_hash {
                    println!("File `{0}` unchanged, skipping...", source.display());
                    return Ok(());
                } else {
                    println!("File `{0}` changed...", source.display());
                }
            } else {
                println!("New file `{0}`...", source.display());
                // TODO: Chain if-let &&
            }
        } else {
            println!("New file `{0}`...", source.display());
        }

        match filter {
            Some(filter) => {
                run_filter_for_entry(source, mirror, filter);
            }
            None => {
                copy(source, mirror).map_err(|e| {
                    format!(
                        "Failed to copy source `{0}` to mirror `{1}`: {e}",
                        source.display(),
                        mirror.display()
                    )
                })?;
            }
        };

        Ok(())
    }

    fn handle_dir_entry(&self, source: &Path, mirror: &Path) -> Result<(), String> {
        create_dir_all(source).map_err(|e| {
            format!(
                "Failed to create mirror directory ({0}) for entry `{1}`: {e}",
                mirror.display(),
                source.display()
            )
        })
    }

    fn cleanup(&self, mirror_list: &BTreeSet<PathBuf>) -> Result<(), String> {
        WalkDir::new(&self.mirror_path)
            .into_iter()
            .try_for_each(|entry| -> Result<(), String> {
                let entry_path = entry.handle_to_string()?.into_path();

                if !mirror_list.contains(&entry_path) {
                    println!("Removing `{0}`...", entry_path.display());
                    if entry_path.is_dir() {
                        std::fs::remove_dir_all(&entry_path).map_err(|e| {
                            format!(
                                "Failed to remove directory `{0}`: {e}",
                                entry_path.display()
                            )
                        })?;
                    } else {
                        std::fs::remove_file(&entry_path).map_err(|e| {
                            format!("Failed to remove file `{0}`: {e}", entry_path.display())
                        })?;
                    }
                }

                Ok(())
            })
    }
}

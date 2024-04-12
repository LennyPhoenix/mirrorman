mod hash;
mod path;

pub use hash::*;
pub use path::*;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::walkdir_result_extension::WalkdirResultExtension;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{copy, create_dir_all, File},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
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

    fn save(&self) -> Result<(), String> {
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
                let entry_path = entry.handle_to_string()?.into_path();

                let parts = self.source_path.components().count();
                let mut entry_mirror = self
                    .mirror_path
                    .join(entry_path.components().skip(parts).collect::<PathBuf>());

                let filter = entry_path.extension().and_then(|ext| {
                    self.filters.iter().find(|filter| {
                        match Command::new(filter).arg("ext").arg(ext).output() {
                            Ok(output) => {
                                if output.status.success() {
                                    let new_extension = match String::from_utf8(output.stdout) {
                                        Ok(output) => output.trim().to_owned(),
                                        Err(e) => {
                                            println!(
                                                "Failed to parse filter `{0}` output: {e}",
                                                filter
                                            );
                                            return false;
                                        }
                                    };
                                    entry_mirror.set_extension(&new_extension);
                                    true
                                } else {
                                    false
                                }
                            }
                            Err(e) => {
                                println!("Failed to invoke filter `{0}`, skipping: {e}", filter);
                                false
                            }
                        }
                    })
                });

                let entry_mirror = entry_mirror;
                {
                    let mut mirror_list = mirror_list
                        .lock()
                        .map_err(|e| format!("Failed to lock mirror list: {e}"))?;
                    mirror_list.insert(entry_mirror.clone());
                }

                if entry_path.is_dir() {
                    create_dir_all(&entry_mirror).map_err(|e| {
                        format!(
                            "Failed to create mirror directory ({0}) for entry `{1}`: {e}",
                            entry_mirror.display(),
                            entry_path.display()
                        )
                    })?;
                } else if entry_path.is_file() {
                    create_dir_all(entry_mirror.parent().ok_or("Failed to get file parent")?)
                        .map_err(|e| {
                            format!(
                                "Failed to create mirror directory ({0}) for entry `{1}`: {e}",
                                entry_mirror.display(),
                                entry_path.display()
                            )
                        })?;

                    let digest = hash_file(&entry_path)?;

                    {
                        let mut new_hashes = new_hashes
                            .lock()
                            .map_err(|e| format!("Failed to lock new hashes: {e}"))?;
                        new_hashes.insert(entry_path.clone(), digest.clone());
                    }
                    if let Some(prev_hash) = self.hashes.get(&entry_path) {
                        if entry_mirror.exists() {
                            if &digest == prev_hash {
                                println!("File `{0}` unchanged, skipping...", entry_path.display());
                                return Ok(());
                            } else {
                                println!("File `{0}` changed...", entry_path.display());
                            }
                        } else {
                            println!("New file `{0}`...", entry_path.display());
                            // TODO: Chain if-let &&
                        }
                    } else {
                        println!("New file `{0}`...", entry_path.display());
                    }

                    let res = filter.is_some_and(|filter| {
                        match Command::new(filter)
                            .arg("run")
                            .arg(&entry_path)
                            .arg(&entry_mirror)
                            .status()
                        {
                            Ok(status) => status.success(),
                            Err(e) => {
                                println!("Failed to invoke filter `{0}`, skipping: {e}", filter);
                                false
                            }
                        }
                    });

                    // No Filter / Filter Failed
                    if !res {
                        copy(&entry_path, &entry_mirror).map_err(|e| {
                            format!(
                                "Failed to copy source `{0}` to mirror `{1}`: {e}",
                                entry_path.display(),
                                entry_mirror.display()
                            )
                        })?;
                    }
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

        // Clean up orphaned mirror files
        WalkDir::new(&self.mirror_path).into_iter().try_for_each(
            |entry| -> Result<(), String> {
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
            },
        )?;

        Ok(())
    }
}

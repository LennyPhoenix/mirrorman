mod hash;
mod path;

pub use hash::*;
pub use path::*;

use crate::filter::{find_filter_for_entry, run_filter_for_entry};
use anyhow::{Context, Result};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{copy, create_dir_all, File},
    io::{stdout, Read, Write},
    iter::repeat,
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

    pub fn load(file_path: &Path) -> Result<Self> {
        let mut file = File::open(file_path)
            .with_context(|| format!("Failed to open {0} for writing", file_path.display()))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .with_context(|| format!("Failed to read file {0}", file_path.display()))?;
        serde_json::from_str(&buf).with_context(|| "Failed to read database from file")
    }

    pub fn sync(&mut self, database_path: &Path) -> Result<()> {
        let old_dir = std::env::current_dir()?;
        let database_folder = database_path
            .parent()
            .context("database file had no parent")?;
        let database_filename = PathBuf::from(
            database_path
                .file_name()
                .context("filename could not be read")?,
        );
        log::debug!("Database path: {0}", database_path.display());
        if database_folder.as_os_str().is_empty() {
            log::debug!("Leaving pwd as before, path is relative.");
        } else {
            log::debug!("Setting pwd to {0}", database_folder.display());
            std::env::set_current_dir(database_folder)?;
        }

        let new_hashes = Arc::new(Mutex::new(BTreeMap::new()));
        let mirror_list = Arc::new(Mutex::new(BTreeSet::new()));
        let counter = Arc::new(Mutex::new(0_usize));

        // Walk source directory
        let source_entries = WalkDir::new(&self.source_path)
            .into_iter()
            .collect::<Vec<_>>();
        let total_entries = source_entries.len();

        source_entries
            .into_par_iter()
            .try_for_each(|entry| -> Result<()> {
                let source_entry = entry?.into_path();

                let parts = self.source_path.components().count();

                let mut mirror_entry = self
                    .mirror_path
                    .join(source_entry.components().skip(parts).collect::<PathBuf>());
                let filter = find_filter_for_entry(&source_entry, &mut mirror_entry, &self.filters);
                let mirror_entry = mirror_entry;

                {
                    let mut mirror_list = match mirror_list.lock() {
                        Ok(mirror_list) => mirror_list,
                        Err(poisoned) => poisoned.into_inner(),
                    };
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

                Self::log_progress(counter.clone(), total_entries)?;

                Ok(())
            })?;

        println!();

        self.hashes = match new_hashes.lock() {
                Ok(new_hashes) => new_hashes,
                Err(poisoned) => {
                    log::warn!("One or more threads panicked, hash list may be incomplete. Consider (re-)running `sync`...");
                    poisoned.into_inner()
                }
            }
            .clone();

        self.save(&database_filename)?;

        let mirror_list = match mirror_list.lock() {
            Ok(mirror_list) => mirror_list,
            Err(poisoned) => {
                log::warn!("One or more threads panicked, mirror list may be incomplete. Consider (re-)running `sync`...");
                poisoned.into_inner()
            }
        };

        self.cleanup(&mirror_list)?;

        std::env::set_current_dir(old_dir)?;

        Ok(())
    }

    pub fn save(&self, database_path: &Path) -> Result<()> {
        self.write_to_file(database_path)
    }

    fn write_to_file(&self, file_path: &Path) -> Result<()> {
        let file = File::create(file_path)
            .with_context(|| format!("Failed to open {0} for writing", file_path.display()))?;
        serde_json::to_writer_pretty(file, self)
            .with_context(|| "Failed to format database to json")
    }

    fn handle_file_entry(
        &self,
        hashes: Arc<Mutex<BTreeMap<PathBuf, String>>>,
        filter: Option<&String>,
        source: &Path,
        mirror: &Path,
    ) -> Result<()> {
        create_dir_all(
            mirror
                .parent()
                .with_context(|| "Failed to get file parent")?,
        )
        .with_context(|| {
            format!(
                "Failed to create mirror directory ({0}) for entry `{1}`",
                mirror.display(),
                source.display()
            )
        })?;

        let digest = hash_file(source)?;

        {
            let mut hashes = match hashes.lock() {
                Ok(hashes) => hashes,
                Err(poisoned) => poisoned.into_inner(),
            };
            hashes.insert(source.to_path_buf(), digest.clone());
        }
        if let Some(prev_hash) = self.hashes.get(source) {
            if mirror.exists() {
                if &digest == prev_hash {
                    log::trace!("File `{0}` unchanged, skipping...", source.display());
                    return Ok(());
                } else {
                    log::info!("File `{0}` changed...", source.display());
                }
            } else {
                log::info!("New file `{0}`...", source.display());
                // TODO: Chain if-let &&
            }
        } else {
            log::info!("New file `{0}`...", source.display());
        }

        match filter {
            Some(filter) => {
                run_filter_for_entry(source, mirror, filter);
            }
            None => {
                copy(source, mirror).with_context(|| {
                    format!(
                        "Failed to copy source `{0}` to mirror `{1}`",
                        source.display(),
                        mirror.display()
                    )
                })?;
            }
        };

        Ok(())
    }

    fn handle_dir_entry(&self, source: &Path, mirror: &Path) -> Result<()> {
        create_dir_all(source).with_context(|| {
            format!(
                "Failed to create mirror directory ({0}) for entry `{1}`",
                mirror.display(),
                source.display()
            )
        })
    }

    fn cleanup(&self, mirror_list: &BTreeSet<PathBuf>) -> Result<()> {
        WalkDir::new(&self.mirror_path)
            .into_iter()
            .try_for_each(|entry| -> Result<()> {
                let entry_path = entry?.into_path();

                if !mirror_list.contains(&entry_path) {
                    log::info!("Removing `{0}`...", entry_path.display());
                    if entry_path.is_dir() {
                        std::fs::remove_dir_all(&entry_path).with_context(|| {
                            format!("Failed to remove directory `{0}`", entry_path.display())
                        })?;
                    } else {
                        std::fs::remove_file(&entry_path).with_context(|| {
                            format!("Failed to remove file `{0}`", entry_path.display())
                        })?;
                    }
                }

                Ok(())
            })
    }

    fn log_progress(counter: Arc<Mutex<usize>>, max_count: usize) -> Result<()> {
        let mut counter = match counter.lock() {
            Ok(counter) => counter,
            Err(poisoned) => poisoned.into_inner(),
        };
        *counter += 1;

        let progress = 100.0 * (*counter as f64 / max_count as f64);

        const BLOCK_COUNT: usize = 20;
        let num_blocks = 20 * *counter / max_count;

        let mut bar = Vec::<char>::new();
        bar.extend(repeat('=').take(num_blocks));
        let count = if num_blocks < BLOCK_COUNT {
            bar.push('>');
            BLOCK_COUNT - num_blocks - 1
        } else {
            0
        };
        bar.extend(repeat(' ').take(count));
        let bar = bar.into_iter().collect::<String>();

        print!("\r[{bar}] {progress:.1}%");
        stdout()
            .flush()
            .with_context(|| "Failed to flush output buffer")?;

        Ok(())
    }
}

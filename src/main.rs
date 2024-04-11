mod database;

use clap::Parser;
use database::{database_path_from_mirror, hash_file, Database};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{copy, create_dir_all},
    path::{Path, PathBuf},
};
use walkdir::{DirEntry, WalkDir};

#[derive(Parser)]
struct Cli {
    source: PathBuf,
    mirror: PathBuf,
}

// - For each directory in source, create mirror directory
// - Load previous run database, if available
// - For each file, check if source file matches hash in database, if not then...
//   - Copy result to mirror directory, and convert if necessary
//   - Hash source file and store against target file, store this in database alongside source path
// - Remove all files/folders in mirror directory that are not in source (should be listed in database)
// - Save database to current directory for next run

fn walk<F>(path: &Path, mut op: F) -> Result<(), String>
where
    F: FnMut(DirEntry) -> Result<(), String>,
{
    WalkDir::new(path)
        .into_iter()
        .try_for_each(|entry| match entry {
            Ok(entry) => op(entry),
            Err(e) => {
                let depth = e.depth();

                let start = match e.path() {
                    None => format!("Traversal aborted at depth {depth}"),
                    Some(path) => {
                        format!("Traversal aborted at `{0}` (depth {depth})", path.display())
                    }
                };

                let full = match e.io_error() {
                    Some(io_error) => format!("{start}: {io_error}"),
                    None => format!("{start}: unknown error"),
                };

                Err(full)
            }
        })
}

fn main() -> Result<(), String> {
    let args = Cli::parse();

    if !args.source.exists() {
        return Err(format!(
            "Invalid source directory, `{0}` does not exist.",
            args.source.display()
        ));
    }

    // Make sure mirror directory is available
    if !args.mirror.exists() {
        create_dir_all(&args.mirror).unwrap();
    }

    let database_path = database_path_from_mirror(&args.mirror)?;
    let mut database = if database_path.exists() {
        Database::load(&database_path)?
    } else {
        Database::new(args.source.clone(), args.mirror.clone())
    };

    let mut new_hashes = BTreeMap::new();
    let mut mirror_list = BTreeSet::new();

    // Walk source directory
    walk(&args.source, |entry| {
        let entry_path = entry.into_path();

        let parts = database.source_path().components().count();
        let entry_mirror = database
            .mirror_path()
            .join(entry_path.components().skip(parts).collect::<PathBuf>());

        mirror_list.insert(entry_mirror.clone());

        if entry_path.is_dir() {
            create_dir_all(&entry_mirror).map_err(|e| {
                format!(
                    "Failed to create mirror directory ({0}) for entry `{1}`: {e}",
                    entry_mirror.display(),
                    entry_path.display()
                )
            })?;
        } else if entry_path.is_file() {
            let digest = hash_file(&entry_path)?;
            let hashes = database.hashes();

            new_hashes.insert(entry_path.clone(), digest.clone());
            if let Some(prev_hash) = hashes.get(&entry_path) {
                if entry_mirror.exists() {
                    if &digest == prev_hash {
                        println!("File `{0}` unchanged, skipping...", entry_path.display());
                        return Ok(());
                    } else {
                        println!("File `{0}` changed...", entry_path.display());
                    }
                } else {
                    println!("New file `{0}`...", entry_path.display()); // TODO: Chain if-let &&
                }
            } else {
                println!("New file `{0}`...", entry_path.display());
            }

            copy(&entry_path, &entry_mirror).map_err(|e| {
                format!(
                    "Failed to copy source `{0}` to mirror `{1}`: {e}",
                    entry_path.display(),
                    entry_mirror.display()
                )
            })?;
        }

        Ok(())
    })?;

    database.set_hashes(new_hashes);
    database.save()?;

    // Clean up orphaned mirror files
    walk(&args.mirror, |entry| {
        let entry_path = entry.into_path();

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
    })?;

    Ok(())
}

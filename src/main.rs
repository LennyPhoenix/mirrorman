mod database;
mod filter;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use database::{database_path_from_mirror, Database};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialise a new database, taking files from `source_directory`, and copying them to
    /// `mirror_directory` after passing them through the given `filters`
    Init {
        /// Directory to take files and directory structure from when mirroring
        source_directory: PathBuf,
        /// Directory to mirror to, all files will be copied or filtered to here
        mirror_directory: PathBuf,
        /// A set of executable filter programs
        filters: Vec<String>,
    },
    /// Syncs any databases (`.mmdb` files) in the current directory, or optionally one or many specific databases
    Sync {
        /// An optional set of databases to explicitly sync
        databases: Vec<PathBuf>,

        /// Use recursive directory traversal
        #[arg(short, long)]
        recursive: bool,
    },
    /// Outputs the example filter
    ExampleFilter,
}

fn init(source: &Path, mirror: &Path, filters: &[String]) -> Result<()> {
    if !source.exists() {
        bail!(
            "Invalid source directory, `{0}` does not exist.",
            source.display()
        )
    }

    let database_path = database_path_from_mirror(mirror)?;
    if database_path.exists() {
        bail!(
            "Database file `{0}` already exists. Run `sync` instead.",
            database_path.display(),
        )
    }

    if mirror.exists()
        && mirror
            .read_dir()
            .with_context(|| "Failed to inspect mirror directory")?
            .next()
            .is_some()
    {
        bail!("Mirror directory `{0}` is not empty, mirroring would erase all existing files. Mirrorman will now abort, if you really wish to proceed (are you sure?) please clear the directory and try again.", mirror.display())
    }

    let mut database = Database::new(source.to_path_buf(), mirror.to_path_buf(), filters.to_vec());
    println!(
        "Beginning first sync of database `{0}`...",
        database_path.display()
    );
    database.sync(&database_path)?;

    println!(
        "`{1}` mirrored at `{2}` successfully! (Database created at `{0}`)",
        database_path.display(),
        source.display(),
        mirror.display()
    );

    Ok(())
}

fn sync_database(database_path: &Path) -> Result<()> {
    let mut database = Database::load(database_path)?;
    println!("Syncing database `{0}`...", database_path.display());
    database.sync(database_path)?;
    Ok(())
}

fn sync(databases: Vec<PathBuf>, recursive: bool) -> Result<()> {
    pretty_env_logger::init();

    if databases.is_empty() {
        let mut any_db = false;

        let mut walkdir = WalkDir::new(Path::new("."));
        if !recursive {
            walkdir = walkdir.max_depth(1);
        }

        walkdir.into_iter().try_for_each(|entry| -> Result<()> {
            let entry_path = entry?.into_path();
            if entry_path.is_file() && entry_path.extension().unwrap_or_default() == "mmdb" {
                if let Err(e) = sync_database(&entry_path) {
                    log::error!(
                        "Failed to syncronise database `{0}`: {e}",
                        entry_path.display()
                    );
                }

                any_db = true;
            }
            Ok(())
        })?;

        if !any_db {
            println!("No databases were found in the current directory to sync, are you in the right place?");
            println!("[hint] I'm looking for `.mmdb` files...");
        }
    } else {
        databases
            .iter()
            .try_for_each(|database_path| -> Result<()> {
                if database_path.is_file()
                    && database_path.extension().unwrap_or_default() == "mmdb"
                {
                    sync_database(database_path)?
                } else {
                    log::error!(
                        "Invalid database file `{0}`, skipping...",
                        database_path.display()
                    )
                }
                Ok(())
            })?;
    }

    println!("Sync complete!");

    Ok(())
}

fn example_filter() -> Result<()> {
    println!("{}", include_str!("../example_filter.sh"));
    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::parse();

    match args.cmd {
        Commands::Init {
            source_directory,
            mirror_directory,
            filters,
        } => init(&source_directory, &mirror_directory, &filters),
        Commands::Sync {
            databases,
            recursive,
        } => sync(databases, recursive),
        Commands::ExampleFilter => example_filter(),
    }
}

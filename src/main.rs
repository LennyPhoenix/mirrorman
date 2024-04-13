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
    Init {
        source_directory: PathBuf,
        mirror_directory: PathBuf,
        filters: Vec<String>,
    },
    Sync,
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
    database.sync()?;

    println!(
        "`{1}` mirrored at `{2}` successfully! (Database created at `{0}`)",
        database_path.display(),
        source.display(),
        mirror.display()
    );

    Ok(())
}

fn sync() -> Result<()> {
    pretty_env_logger::init();

    WalkDir::new(Path::new("."))
        .max_depth(1)
        .into_iter()
        .try_for_each(|entry| -> Result<()> {
            let entry_path = entry?.into_path();
            if entry_path.is_file() && entry_path.extension().unwrap_or_default() == "mmdb" {
                let mut database = Database::load(&entry_path)?;
                println!("Syncing database `{0}`...", entry_path.display());
                database.sync()?;
            }
            Ok(())
        })?;

    println!("Sync complete!");

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
        Commands::Sync => sync(),
    }
}

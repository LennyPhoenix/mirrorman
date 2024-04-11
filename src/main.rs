mod database;
mod walkdir_result_extension;

use clap::{Parser, Subcommand};
use database::{database_path_from_mirror, Database};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use walkdir_result_extension::WalkdirResultExtension;

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
    },
    Sync,
}

fn init(source: &Path, mirror: &Path) -> Result<(), String> {
    if !source.exists() {
        return Err(format!(
            "Invalid source directory, `{0}` does not exist.",
            source.display()
        ));
    }

    let database_path = database_path_from_mirror(mirror)?;
    if database_path.exists() {
        return Err(format!(
            "Database file `{0}` already exists. Run `sync` instead.",
            database_path.display(),
        ));
    }

    let mut database = Database::new(source.to_path_buf(), mirror.to_path_buf());
    database.sync()?;

    Ok(())
}

fn sync() -> Result<(), String> {
    WalkDir::new(Path::new("."))
        .max_depth(1)
        .into_iter()
        .try_for_each(|entry| -> Result<(), String> {
            let entry_path = entry.handle_to_string()?.into_path();
            if entry_path.is_file() && entry_path.extension().unwrap_or_default() == "mmdb" {
                let mut database = Database::load(&entry_path)?;
                database.sync()?;
            }
            Ok(())
        })?;

    Ok(())
}

fn main() -> Result<(), String> {
    let args = Cli::parse();

    match args.cmd {
        Commands::Init {
            source_directory,
            mirror_directory,
        } => init(&source_directory, &mirror_directory),
        Commands::Sync => sync(),
    }
}

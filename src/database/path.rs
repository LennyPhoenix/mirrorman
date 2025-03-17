use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

pub fn database_path_from_mirror(mirror_path: &Path) -> Result<PathBuf> {
    let path = mirror_path
        .components()
        .filter_map(|c| c.as_os_str().to_ascii_lowercase().into_string().ok())
        .reduce(|a, b| format!("{0} {1}", a, b))
        .with_context(|| "Failed to build database filename")?
        .replace("/", " ")
        .replace(".", " ")
        .trim()
        .replace(" ", "_");

    let path = PathBuf::from_str(&format!("{path}.mmdb"))
        .with_context(|| "Failed to construct database path from mirror path")?;

    Ok(path)
}

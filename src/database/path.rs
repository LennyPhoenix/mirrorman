use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

pub fn database_path_from_mirror(mirror_path: &Path) -> Result<PathBuf, String> {
    let path = mirror_path
        .components()
        .filter_map(|c| c.as_os_str().to_ascii_lowercase().into_string().ok())
        .reduce(|a, b| format!("{0}_{1}", a, b))
        .ok_or("Failed to build database filename")?;

    let path = PathBuf::from_str(&format!("{path}.mmdb"))
        .map_err(|e| format!("Failed to construct database path from mirror path: {e}"))?;

    Ok(path)
}

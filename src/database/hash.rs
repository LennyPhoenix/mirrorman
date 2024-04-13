use anyhow::{Context, Result};
use base32::{encode, Alphabet};
use sha2::{Digest, Sha256};
use std::{fs::File, io::copy, path::Path};

pub fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .with_context(|| format!("Failed to open `{0}` for reading", path.display()))?;
    let mut hasher = Sha256::new();
    copy(&mut file, &mut hasher)
        .with_context(|| format!("Failed to hash file `{0}`", path.display()))?;
    Ok(encode(Alphabet::Crockford, &hasher.finalize()))
}

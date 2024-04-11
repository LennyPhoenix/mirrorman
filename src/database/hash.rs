use std::{fs::File, io::copy, path::Path};

use base32::{encode, Alphabet};
use sha2::{Digest, Sha256};

pub fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open `{0}` for reading: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    copy(&mut file, &mut hasher)
        .map_err(|e| format!("Failed to hash file `{0}`: {e}", path.display()))?;
    Ok(encode(Alphabet::Crockford, &hasher.finalize()))
}

use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn run_filter_for_entry(source_entry: &Path, mirror_entry: &Path, filter: &str) {
    match Command::new(filter)
        .arg("run")
        .arg(source_entry)
        .arg(mirror_entry)
        .status()
    {
        Ok(status) => {
            if !status.success() {
                log::error!(
                    "Filter `{0}` failed for `{1}`, skipping...",
                    filter,
                    source_entry.display()
                );
            }
        }
        Err(e) => {
            log::error!("Failed to invoke filter `{0}`, skipping: {e}", filter);
        }
    }
}

pub fn find_filter_for_entry<'a>(
    entry: &Path,
    mirror_entry: &mut PathBuf,
    filters: &'a [String],
) -> Option<&'a String> {
    entry.extension().and_then(|ext| {
        filters.iter().find(
            |filter| match Command::new(filter).arg("ext").arg(ext).output() {
                Ok(output) => {
                    if output.status.success() {
                        let new_extension = match String::from_utf8(output.stdout) {
                            Ok(output) => output.trim().to_owned(),
                            Err(e) => {
                                log::error!("Failed to parse filter `{0}` output: {e}", filter);
                                return false;
                            }
                        };
                        mirror_entry.set_extension(&new_extension);
                        true
                    } else {
                        false
                    }
                }
                Err(e) => {
                    log::error!("Failed to invoke filter `{0}`, skipping: {e}", filter);
                    false
                }
            },
        )
    })
}

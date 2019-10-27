use std::ffi::{OsString};
use std::path::{Path, PathBuf};

pub fn real_path(target: &OsString, partial: &Path) -> OsString {
    PathBuf::from(target)
            .join(partial.strip_prefix("/").unwrap())
            .into_os_string()
}


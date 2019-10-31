use std::ffi::{OsString};
use std::fs::{File};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::path::{Path, PathBuf};

use crate::libc_bridge::libc_wrappers;
use crate::libc_bridge::libc::c_int;
use crate::libc_bridge as br;
use fuse_mt::FileAttr;

pub fn real_path(target: &OsString, partial: &Path) -> OsString {
    PathBuf::from(target)
            .join(partial.strip_prefix("/").unwrap())
            .into_os_string()
}

pub fn getattr(path: OsString) -> Result<FileAttr, c_int> {
    match libc_wrappers::lstat(path) {
        Ok(stat) => {
            Ok(br::stat_to_fuse(stat))
        },
        Err(e) => {
            Err(e)
        }
    }
}

/// A file that is not closed upon leaving scope.
pub struct UnmanagedFile {
    inner: Option<File>,
}

impl UnmanagedFile {
    pub unsafe fn new(fd: u64) -> UnmanagedFile {
        UnmanagedFile {
            inner: Some(File::from_raw_fd(fd as i32))
        }
    }
}

impl Drop for UnmanagedFile {
    fn drop(&mut self) {
        // Release control of the file descriptor so it is not closed.
        let file = self.inner.take().unwrap();
        file.into_raw_fd();
    }
}

impl Read for UnmanagedFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.as_ref().unwrap().read(buf)
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.as_ref().unwrap().read_to_end(buf)
    }
}

impl Seek for UnmanagedFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.as_ref().unwrap().seek(pos)
    }
}


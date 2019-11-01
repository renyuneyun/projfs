use std::ffi::{CStr, OsStr, OsString};
use std::fs::{self};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use bimap::BiMap;
use fuse_mt::*;
use mime_guess::{self, mime, Mime};
use time::Timespec;

use crate::fsop::{self, UnmanagedFile};
use crate::libc_bridge::libc;
use crate::libc_bridge::libc_wrappers;
use crate::libc_bridge as br;

const TTL: Timespec = Timespec { sec: 1, nsec: 0 };

fn _should_project(mime_type: &Mime) -> bool {
    return mime_type.type_() == mime::AUDIO || mime_type.type_() == mime::VIDEO
}

fn _do_proj(input: &OsString, output: &OsString) {
    debug!("do_proj() call: {:?} -> {:?}", input, output);
    let mut cmd = Command::new("ffmpeg") // Streaming
        .args(&["-i", input.as_os_str().to_str().unwrap(), "-vn", output.as_os_str().to_str().unwrap()])
        .spawn()
        .expect("failed to execute process");
    cmd.wait().unwrap();
}

fn _filename_conv(partial: &Path) -> OsString {
    let mut path_buf = partial.to_path_buf();
    path_buf.set_extension("ogg");
    path_buf.into_os_string()
}


trait ProjectionResolver {
    fn source(&self, partial: &Path) -> OsString;
    fn cache(&self, partial: &Path) -> OsString;
}

pub struct ProjectionFS {
    pub source_dir: OsString,
    pub cache_dir: OsString,
    pm: ProjectionManager,
}

impl ProjectionFS {
    pub fn new(source_dir: OsString, cache_dir: OsString) -> ProjectionFS {
        ProjectionFS {
            source_dir: source_dir,
            cache_dir: cache_dir,
            pm: ProjectionManager::new(),
        }
    }

    fn resolve<T: AsRef<Path>>(&self, partial: T) -> (AccessType, OsString) {
        let partial = partial.as_ref();
        match self.pm.source(&partial.as_os_str().to_os_string()) {
            Some(_source) => {
                debug!("{:?} is a projected file", partial);
                (AccessType::Projected, self.cache_path(partial))
            },
            None => {
                debug!("{:?} is a non-projected file", partial);
                (AccessType::PassThrough, self.source_path(partial))
            },
        }
    }

    fn sniff_projection(&self, dir_path: &Path, filename: &OsStr) -> OsString {
        let partial = &PathBuf::from(dir_path).join(filename);
        match self.pm.access_type(self.source_path(partial)) {
            AccessType::PassThrough => {
                self.source_path(partial)
            },
            AccessType::Projected => {
                let partial_os_string = partial.as_os_str().to_os_string();
                match self.pm.destination(&partial_os_string) {
                    Some(dest) => {
                        debug!("readdir file already projected {:?}", partial);
                        self.cache_path(dest)
                    },
                    None => {
                        let dest_partial = self.pm.project(partial, self);
                        self.cache_path(dest_partial)
                    }
                }
            },
        }
    }

    fn source_path<T: AsRef<Path>>(&self, partial: T) -> OsString {
        self.source(partial.as_ref())
    }

    fn cache_path<T: AsRef<Path>>(&self, partial: T) -> OsString {
        self.cache(partial.as_ref())
    }

}

impl ProjectionResolver for ProjectionFS {
    fn source(&self, partial: &Path) -> OsString {
        fsop::real_path(&self.source_dir, partial)
    }

    fn cache(&self, partial: &Path) -> OsString {
        fsop::real_path(&self.cache_dir, partial)
    }
}

impl FilesystemMT for ProjectionFS {
    fn init(&self, _req: RequestInfo) -> ResultEmpty {
        debug!("init");
        Ok(())
    }

    fn destroy(&self, _req: RequestInfo) {
        debug!("destroy");
    }

    fn getattr(&self, _req: RequestInfo, path: &Path, fh: Option<u64>) -> ResultEntry {
        debug!("getattr: {:?} ({} filehandle)", path, if let Some(_) = fh {"with"} else {"without"});

        if let Some(fh) = fh { // Only used in setattr. Never used for read-only filesystem
            match libc_wrappers::fstat(fh) {
                Ok(stat) => Ok((TTL, br::stat_to_fuse(stat))),
                Err(e) => Err(e)
            }
        } else {
            let (access_type, real) = self.resolve(path);

            match fsop::getattr(real) {
                Ok(stat) => {
                    match access_type {
                        AccessType::PassThrough => {
                            Ok((TTL, stat))
                        },
                        AccessType::Projected => {
                            match fsop::getattr(self.source_path(self.pm.source(&path.as_os_str().to_os_string()).unwrap())) {
                                Ok(mut stat_real) => {
                                    stat_real.size = stat.size;
                                    stat_real.blocks = stat.blocks;
                                    Ok((TTL, stat_real))
                                },
                                Err(e) => {
                                    let err = io::Error::from_raw_os_error(e);
                                    error!("lstat({:?}): {}", path, err);
                                    Err(err.raw_os_error().unwrap())
                                }
                            }
                        },
                    }
                },
                Err(e) => {
                    let err = io::Error::from_raw_os_error(e);
                    error!("lstat({:?}): {}", path, err);
                    Err(err.raw_os_error().unwrap())
                }
            }
        }
    }

    //checked
    fn opendir(&self, _req: RequestInfo, path: &Path, _flags: u32) -> ResultOpen {
        let real = self.source_path(path);
        debug!("opendir: {:?} (flags = {:#o})", real, _flags);
        match libc_wrappers::opendir(real) {
            Ok(fh) => Ok((fh, 0)),
            Err(e) => {
                let ioerr = io::Error::from_raw_os_error(e);
                error!("opendir({:?}): {}", path, ioerr);
                Err(e)
            }
        }
    }

    //checked
    fn releasedir(&self, _req: RequestInfo, path: &Path, fh: u64, _flags: u32) -> ResultEmpty {
        debug!("releasedir: {:?}", path);
        libc_wrappers::closedir(fh)
    }

    //checked
    fn readdir(&self, _req: RequestInfo, path: &Path, fh: u64) -> ResultReaddir {
        debug!("readdir: {:?}", path);
        let mut entries: Vec<DirectoryEntry> = vec![];

        if fh == 0 {
            error!("readdir: missing fh");
            return Err(libc::EINVAL);
        }

        loop {
            match libc_wrappers::readdir(fh) {
                Ok(Some(entry)) => {
                    let name_c = unsafe { CStr::from_ptr(entry.d_name.as_ptr()) };
                    let name = OsStr::from_bytes(name_c.to_bytes()).to_owned();

                    let filetype = match entry.d_type {
                        libc::DT_DIR => FileType::Directory,
                        libc::DT_REG => FileType::RegularFile,
                        libc::DT_LNK => FileType::Symlink,
                        libc::DT_BLK => FileType::BlockDevice,
                        libc::DT_CHR => FileType::CharDevice,
                        libc::DT_FIFO => FileType::NamedPipe,
                        libc::DT_SOCK => {
                            warn!("FUSE doesn't support Socket file type; translating to NamedPipe instead.");
                            FileType::NamedPipe
                        },
                        0 | _ => {
                            let entry_path = PathBuf::from(path).join(&name);
                            let source_path = self.source_path(&entry_path);
                            match libc_wrappers::lstat(source_path) {
                                Ok(stat64) => br::mode_to_filetype(stat64.st_mode),
                                Err(errno) => {
                                    let ioerr = io::Error::from_raw_os_error(errno);
                                    panic!("lstat failed after readdir_r gave no file type for {:?}: {}",
                                           entry_path, ioerr);
                                }
                            }
                        }
                    };

                    info!("readdir() :: filename: {:?}", &name);
                    if filetype == FileType::RegularFile {
                        let result_path = self.sniff_projection(path, &name);
                        let name = Path::new(&result_path).file_name().unwrap().to_owned();
                        entries.push(DirectoryEntry {
                            name,
                            kind: filetype,
                        })
                    } else {
                        entries.push(DirectoryEntry {
                            name,
                            kind: filetype,
                        })
                    }
                },
                Ok(None) => { break; },
                Err(e) => {
                    error!("readdir: {:?}: {}", path, e);
                    return Err(e);
                }
            }
        }

        Ok(entries)
    }

    fn open(&self, _req: RequestInfo, path: &Path, flags: u32) -> ResultOpen {
        debug!("open: {:?} flags={:#x}", path, flags);

        let (_, real) = self.resolve(path);
        match libc_wrappers::open(real, flags as libc::c_int) {
            Ok(fh) => Ok((fh, flags)),
            Err(e) => {
                error!("open({:?}): {}", path, io::Error::from_raw_os_error(e));
                Err(e)
            }
        }
    }

    fn release(&self, _req: RequestInfo, path: &Path, fh: u64, _flags: u32, _lock_owner: u64, _flush: bool) -> ResultEmpty {
        debug!("release: {:?}", path);
        libc_wrappers::close(fh)
    }

    fn read(&self, _req: RequestInfo, path: &Path, fh: u64, offset: u64, size: u32, result: impl FnOnce(Result<&[u8], libc::c_int>)) {
        debug!("read: {:?} {:#x} @ {:#x}", path, size, offset);
        let mut file = unsafe { UnmanagedFile::new(fh) };

        let mut data = Vec::<u8>::with_capacity(size as usize);
        unsafe { data.set_len(size as usize) };

        if let Err(e) = file.seek(SeekFrom::Start(offset)) {
            error!("seek({:?}, {}): {}", path, offset, e);
            result(Err(e.raw_os_error().unwrap()));
            return;
        }
        match file.read(&mut data) {
            Ok(n) => { data.truncate(n); },
            Err(e) => {
                error!("read {:?}, {:#x} @ {:#x}: {}", path, size, offset, e);
                result(Err(e.raw_os_error().unwrap()));
                return;
            }
        }

        result(Ok(&data));
    }

}

struct ProjectionManager {
    projection: Mutex<BiMap<OsString, OsString>>,
    should_project: fn (mime_type: &Mime) -> bool,
    do_proj: fn (input: &OsString, output: &OsString),
    filename_conv: fn (partial: &Path) -> OsString,
}

impl ProjectionManager {
    fn new() -> ProjectionManager {
        ProjectionManager {
            projection: Mutex::new(BiMap::new()),
            should_project: _should_project,
            do_proj: _do_proj,
            filename_conv: _filename_conv,
        }
    }

    fn destination(&self, filepath: &OsString) -> Option<OsString> {
        match self.projection.lock().unwrap().get_by_left(filepath) {
            Some(dest) => {
                Some(dest.clone())
            },
            None => {
                None
            },
        }
    }

    fn source(&self, filepath: &OsString) -> Option<OsString> {
        match self.projection.lock().unwrap().get_by_right(filepath) {
            Some(source) => {
                Some(source.clone())
            },
            None => {
                None
            },
        }
    }

    fn access_type<T: AsRef<Path>>(&self, file_path: T) -> AccessType {
        let file_path = file_path.as_ref();
        if file_path.is_dir() {
            error!("atype() shouldn't be called on a directory ({:?})", file_path);
        }
        let guess = mime_guess::from_path(file_path);
        if guess.is_empty() {
            warn!("MIME for filepath {} can't guess", file_path.display());
            AccessType::PassThrough
        } else {
            let first_guess = guess.first().unwrap();
            info!("MIME for {:?} is {}", file_path, first_guess);
            if (self.should_project)(&first_guess) {
                AccessType::Projected
            } else {
                AccessType::PassThrough
            }
        }
    }

    /// parameter `partial` is the relative partial path, pointing to the *file* to be projected
    fn project<T: AsRef<Path>>(&self, partial: T, resolver: &dyn ProjectionResolver) -> OsString {
        let source_partial = partial.as_ref();
        let dest_partial = &Path::new(&(self.filename_conv)(source_partial)).to_owned();
        let dest = &resolver.cache(dest_partial);
        fs::create_dir_all(Path::new(dest).parent().unwrap()).expect(&format!("cache directory {:?} can't be created", dest));
        self.projection.lock().unwrap().insert(OsString::from(source_partial), OsString::from(dest_partial));
        (self.do_proj)(&resolver.source(source_partial), dest);
        dest_partial.as_os_str().to_os_string()
    }
}

#[derive(Debug)]
enum AccessType {
    Projected,
    PassThrough,
}

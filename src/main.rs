use std::ffi::{OsStr, OsString};
use std::hash::{Hash, Hasher};
use std::path::Path;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate log;

use clap::App;
use seahash::SeaHasher;

mod fsop;
mod libc_bridge;
mod projfs;

fn repr_of_path<T: AsRef<Path>>(path: T) -> String {
    let path = path.as_ref();
    let mut hasher = SeaHasher::new();
    path.hash(&mut hasher);
    let hash: u64 = hasher.finish();
    format!("{:x}", hash)
}

fn main() {
    env_logger::init();

    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let mountpoint = matches.value_of_os("MOUNTPOINT").unwrap();
    let source_dir = matches.value_of_os("SOURCE_DIR").unwrap();
    let cache_dir = if let Some(cache_dir) = matches.value_of_os("cache") {
        OsString::from(cache_dir)
    } else {
        if let Some(mut cache_dir) = dirs::cache_dir() {
            cache_dir.push("projfs");
            cache_dir.push(if let Ok(abs_path) = std::fs::canonicalize(&source_dir) {
                repr_of_path(&abs_path)
            } else {
                repr_of_path(&source_dir)
            });
            cache_dir.into_os_string()
        } else {
            println!("Couldn't get cache directory automatically. Please explicitly specify a cache directory.");
            std::process::exit(-1);
        }
    };

    info!(
        "source_dir: {:?} :: cache_dir: {:?}",
        &source_dir, &cache_dir
    );

    let filesystem =
        projfs::ProjectionFS::new(OsString::from(source_dir), OsString::from(cache_dir));

    let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("ro,auto_unmount")];

    fuse_mt::mount(fuse_mt::FuseMT::new(filesystem, 1), &mountpoint, &fuse_args).unwrap();
}

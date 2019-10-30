use std::env;
use std::ffi::{OsStr, OsString};

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

mod fsop;
mod libc_bridge;
mod projfs;

fn main() {
    env_logger::init();

    let args: Vec<OsString> = env::args_os().collect();

    let (source_dir, cache_dir) = match args.len() {
        3 => {
            if let Some(mut cache_dir) = dirs::cache_dir() {
                cache_dir.push("projfs");
                (args[2].clone(), cache_dir.into_os_string())
            } else {
                println!("Couldn't get cache directory automatically. Please explicitly specify a cache directory.");
                std::process::exit(-1);
            }
        }
        4 => {
            (args[2].clone(), args[3].clone())
        },
        _ => {
            println!("Usage: {} <mountpoint> <source_dir> [<cache_dir>]", &env::args().next().unwrap());
            std::process::exit(-1);
        }
    };

    info!("source_dir: {:?} :: cache_dir: {:?}", &source_dir, &cache_dir);

    let filesystem = projfs::ProjectionFS::new(source_dir, cache_dir);

    let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("ro,auto_unmount")];

    fuse_mt::mount(fuse_mt::FuseMT::new(filesystem, 1), &args[1], &fuse_args).unwrap();
}

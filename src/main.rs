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

    if args.len() != 4 {
        println!("Usage: {} <mountpoint> <source> <cache_dir>", &env::args().next().unwrap());
        std::process::exit(-1);
    }

    let filesystem = projfs::ProjectionFS::new(args[2].clone(), args[3].clone());

    let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("ro,auto_unmount")];

    fuse_mt::mount(fuse_mt::FuseMT::new(filesystem, 1), &args[1], &fuse_args).unwrap();
}

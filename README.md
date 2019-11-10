# ***Proj***ection ***F***ile***S***ystem [![Build Status](https://travis-ci.org/renyuneyun/projfs.svg?branch=master)](https://travis-ci.org/renyuneyun/projfs)
- - - - - - -

ProjFS is a FUSE filesystem aiming at providing a general way of doing filesystem projection: project directories and files to a new mount point by converting specified files through a custom projection command.

It can provide use cases such as automatically converting audio files while keeping the original file.

# Usage

## Basic usage

```
cargo run <source> <mountpoint>
```

The `<mountpoint>` is where the new projected filesystem hierarchy will appear. It is (apparently) read-only.

By default, the program performs the projection by using `ffmpeg` to convert every audio and video file to `ogg` file (audio).

It identifies files by MIME type (using the `mime_guess` crate). All files are provided as-is except for `audio/*` and `video/*` files which are going to be projected. The command used to convert is `ffmpeg -i <original_file> -vn <output_file>`. File suffix is changed to `ogg` where applicable.

## Advanced usage

Please see the help document using:

```
cargo run --help
```

An example projection specification is available in `example_projection.yml`. It also corresponds to the default behaviour.

# TODO

* [x] Having a default cache dir
* [x] Copying file attributes from source file (except for size)
* [x] Different cache dirs for different source dirs
* [x] Update cache only when necessary
* [ ] Return placeholder information for files under-projection
* [x] Accept configuration
    * [x] Custom filetype
    * [x] Custom projection command
    * [ ] A list of configurations
* [ ] One-to-many projection
* [ ] Background automatic async cache
* [ ] Update cache while running
* [ ] Validate configuration before loading

# License

Except for certain files specified afterwards, this project is licensed under the [Apache 2.0 License](http://www.apache.org/licenses/LICENSE-2.0) as stated below.

## Copyright notice

Copyright 2019 renyuneyun (Rui Zhao)

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

## Exception

The following files under the source directory (`src/`) are directly obtained from the [fuse-mt](https://github.com/wfraser/fuse-mt) project ([commit](https://github.com/wfraser/fuse-mt/tree/97e115667682b4a7e54c1831360b8c572c667db3/example/src)), licensed under Apache 2.0 license and MIT license:

* `libc_bridge/libc_extras.rs`
* `libc_bridge/libc_bridge.rs`

The `projfs.rs` and `libc_bridge/mod.rs` files contain unmodified code from [fuse-mt](https://github.com/wfraser/fuse-mt/blob/97e115667682b4a7e54c1831360b8c572c667db3/example/src/passthrough.rs).


# Thumbnailer Bridge
[![Crates.io](https://img.shields.io/crates/v/thumbnailer-bridge?logo=rust)](https://crates.io/crates/thumbnailer-bridge)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow)](https://opensource.org/licenses/MIT)
[![Static Badge](https://img.shields.io/badge/Home-%23ED6663?logo=GitHub&label=Wiki)](../../wiki/Home)

This tool makes requests to create thumbnails through D-Bus following
[org.freedesktop.thumbnails.Thumbnailer1](https://wiki.gnome.org/DraftSpecs/ThumbnailerSpec#org.freedesktop.thumbnails.Thumbnailer1)
specification. It doesn't create thumbnails on it's own, but acts as a bridge between your file manager and thumbnailer.  

To create thumbnails you will need a daemon, like [tumbler](https://docs.xfce.org/xfce/tumbler/start).

## Features
What are the advantages of using this instead of a shell script with `dbus-send`?

* Ease of use.
* Multithreading.
* Paths with commas and other symbols not compatible with `dbus-send`.
* Direct usage of `libmagic` and `dbus` without additional processes.

## Dependencies

* dbus     (communication)
* libmagic (mime detection)

## Usage
```
Bridge between your file manager and thumbnail daemon.

Usage: thumbnailer-bridge [OPTIONS] [FILE]...

Arguments:
  [FILE]...  

Options:
  -t, --thumbnail              Print path to thumbnail if it exists and up to date
  -f, --flavor <FLAVOR>        Flavor of the thumbnails [default: normal]
  -s, --scheduler <SCHEDULER>  Scheduler for thumbnail generation [default: default]
  -u, --unchecked              Do not check if thumbnail already exists and up to date
  -l, --listen                 Listen for notifications
      --list-flavors           List supported schedulers
      --list-schedulers        List supported thumbnail flavors
      --list-mime              List supported media types
  -h, --help                   Print help
  -V, --version                Print version
```

This is how you request thumbnails. Flavor `-f` or `--flavor` is usually responsible for the size of a thumbnail.  

```sh
thumbnailer-bridge -f x-large $PWD/*
```

I recommend you to use full path to your current directory that your file manager provides, instead of using relative path,
this way, if you're inside a sym-linked location `/home/user/pictures -> /mnt/nas`, your thumbnails will be preserved
if you decide to remount original location `/home/user/pictures -> /mnt/nas-old`

If you want to be notified when thumbnails are ready to use, add `--listen` flag.
```sh
$ thumbnailer-bridge --listen
/home/user/pictures/meal-2023-02-22.png
/home/user/pictures/booty.jpg
/home/user/books/how_to_eat_chicken.epub
...
```

You will find your thumbnails at `${XDG_CACHE_HOME:-~/.cache}/thumbnails/(flavor)/`.

To find a thumbnail, corresponding to your file, you can use `-t` or `--thumbnail` flag.
```sh
$ thumbnailer-bridge -f x-large -t ~/pictures/cat.png
/home/user/.cache/thumbnails/x-large/722b4dbaa8bf716e9f9f7f33f72fd9d4.png
```

For additional information:
[Thumbnail Managing Standard](https://specifications.freedesktop.org/thumbnail-spec/thumbnail-spec-latest.html).

## Installation

Can be installed from [crates.io](https://crates.io/) with `cargo`:

```sh
cargo install thumbnailer-bridge
```

## Building

To build this little thing, you'll need some [Rust](https://www.rust-lang.org/).

```sh
git clone --depth 1 https://github.com/Elvyria/thumbnailer-bridge
cd thumbnailer-bridge
cargo build --release
```

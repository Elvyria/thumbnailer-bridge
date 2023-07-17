use std::{env, path::PathBuf};

pub fn cache_dir() -> PathBuf {
    env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|mut p| { p.push(".cache"); p })
        })
    .expect("couldn't find cache directory")
}

pub fn thumbnails_dir(flavor: &str) -> PathBuf {
    let mut thumbnails_dir = cache_dir();
    thumbnails_dir.push("thumbnails");
    thumbnails_dir.push(flavor);

    thumbnails_dir
}

mod rpc;

use std::{path::PathBuf, env, collections::HashSet, sync::{Once, Mutex}, io::Write, cmp::max};
use magic::{Cookie, CookieFlags};
use rayon::prelude::*;

use anyhow::Error;
use clap::Parser;
use path_absolutize::Absolutize;
use rustbus::RpcConn;

const URI_PREFIX: &str = "file://";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(value_name = "FILE")]
    paths: Vec<PathBuf>,

    /// Flavor of the thumbnails
    #[arg(short, long, default_value = "normal")]
    flavor: String,

    /// Scheduler for thumbnail generation
    #[arg(short, long, default_value = "default")]
    scheduler: String,

    /// Do not check if thumbnail already exists
    #[arg(short, long)]
    unchecked: bool,

    /// Listen for notifications
    #[arg(short, long)]
    listen: bool,

    /// List supported schedulers
    #[arg(long)]
    list_flavors: bool,

    /// List supported thumbnail flavors
    #[arg(long)]
    list_schedulers: bool,

    /// List supported media types
    #[arg(long)]
    list_mime: bool,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let mut conn = RpcConn::session_conn(rustbus::connection::Timeout::Infinite)?;

    if args.listen {
        return rpc::listen(&mut conn).map_err(Into::into)
    }

    let list = if args.list_flavors
    {
        Some(rpc::list_flavors(&mut conn)?)
    }
    else if args.list_schedulers
    {
        Some(rpc::list_schedulers(&mut conn)?)
    }
    else if args.list_mime
    {
        Some(rpc::list_supported(&mut conn)?.1)
    }
    else { None };

    if let Some(mut list) = list {
        list.par_sort_unstable();

        let s = list.iter().fold(vec![0; 0], |mut v, b| {
            v.extend_from_slice(b.as_bytes());
            v.push(b'\n');
            v
        });

        std::io::stdout().lock().write_all(&s)?;

        return Ok(())
    }

    if !args.paths.is_empty() {
        if args.unchecked {
            create_all(&mut conn, args.paths, &args.flavor, &args.scheduler)?;
        } else {
            create_missing(&mut conn, args.paths, &args.flavor, &args.scheduler)?;
        }
    }

    Ok(())
}

fn cache_dir() -> PathBuf {
    env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|mut p| { p.push(".cache"); p })
        })
    .expect("couldn't find cache directory")
}

fn create_missing(conn: &mut RpcConn, paths: Vec<PathBuf>, flavor: &str, scheduler: &str) -> Result<(), Error> {
    let mut thumbnails_dir = cache_dir();
    thumbnails_dir.push("thumbnails");
    thumbnails_dir.push(flavor);

    let mtx = Mutex::new((vec![], vec![]));

    let (_, supported) = rpc::list_supported(conn)?;
    let supported = HashSet::<String>::from_iter(supported);

    paths.par_chunks(max(1, paths.len() / 4)).for_each_init(|| {
        let cookie = Cookie::open(CookieFlags::ERROR | CookieFlags::MIME_TYPE).expect("opening libmagic cookie");
        (cookie, Once::new())
    },
    |(cookie, once), chunk| {
        for p in chunk {
            if !p.is_file() { continue }

            let Ok(abs) = p.absolutize() else { continue };

            let Some(abs_str) = abs.to_str() else {
                println!("Warning! A non-valid UTF-8 path was provided, this is not supported.\n{}\n", abs.to_string_lossy());
                continue
            };

            let mut uri = String::from(URI_PREFIX);
            uri.push_str(abs_str);

            let sum = format!("{:x}.png", md5::compute(&uri));

            let mut thumbnail = thumbnails_dir.clone();
            thumbnail.push(&sum);

            if !thumbnail.exists() {
                once.call_once(|| cookie.load::<&str>(&[]).expect("loading cookie database"));

                if let Ok(mime) = cookie.file(&uri[URI_PREFIX.len()..]) {
                    if supported.contains(&mime) {
                        let mut lock = mtx.lock().expect("locking vectors to push new uri and mime");

                        lock.0.push(uri);
                        lock.1.push(mime);
                    }
                };
            }
        }
    });

    let (uris, mimes) = mtx.into_inner().unwrap();

    if !uris.is_empty() {
        rpc::queue_thumbnails(conn, uris, mimes, flavor, scheduler)?;
    }

    Ok(())
}

fn create_all(conn: &mut RpcConn, paths: Vec<PathBuf>, flavor: &str, scheduler: &str) -> Result<(), Error> {
    let mtx = Mutex::new((vec![], vec![]));

    let (_, supported) = rpc::list_supported(conn)?;
    let supported = HashSet::<String>::from_iter(supported);

    paths.par_chunks(max(1, paths.len() / 4)).for_each_init(|| {
        let cookie = Cookie::open(CookieFlags::ERROR | CookieFlags::MIME_TYPE).expect("opening libmagic cookie");
        cookie.load::<&str>(&[]).expect("loading cookie database");
        cookie
    },
    |cookie, chunk| {
        for p in chunk {
            if !p.is_file() { continue }

            let Ok(abs) = p.absolutize() else { continue };

            let Some(abs_str) = abs.to_str() else {
                println!("Warning! A non-valid UTF-8 path was provided, this is not supported.\n{}\n", abs.to_string_lossy());
                continue
            };

            let mut uri = String::from(URI_PREFIX);
            uri.push_str(abs_str);

            if let Ok(mime) = cookie.file(&uri[URI_PREFIX.len()..]) {
                if supported.contains(&mime) {
                    let mut lock = mtx.lock().expect("locking vectors to push new uri and mime");

                    lock.0.push(uri);
                    lock.1.push(mime);
                }
            };

        }
    });

    let (uris, mimes) = mtx.into_inner().unwrap();

    if !uris.is_empty() {
        rpc::queue_thumbnails(conn, uris, mimes, flavor, scheduler)?;
    }

    Ok(())
}

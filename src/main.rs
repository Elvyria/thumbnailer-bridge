#![allow(unreachable_code)]
mod rpc;
mod png;
mod c;

use std::{path::{PathBuf, Path}, env, collections::HashSet, sync::{Once, Mutex, Arc, atomic::{AtomicBool, Ordering, AtomicUsize}, RwLock}, io::{Write, BufRead, self, Read}, cmp::max, process::ExitCode, fs::{File, Metadata}, time::UNIX_EPOCH, thread, os::{fd::AsRawFd, unix::prelude::{OsStringExt, OsStrExt}}, ffi::{CString, OsString, OsStr}, vec};
use io_uring::{opcode, types::Fd, IoUring, squeue::{EntryMarker, self}, cqueue};
use libc::{statx, AT_FDCWD, O_RDONLY, O_NONBLOCK, O_NOFOLLOW, O_DIRECT};
use magic::{Cookie, CookieFlags};
use rayon::prelude::*;

use anyhow::Error;
use clap::Parser;
use path_absolutize::Absolutize;
use rustbus::{RpcConn, connection::Timeout};

pub const URI_PREFIX: &str = "file://";

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

    /// Do not check if thumbnail already exists and valid
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

fn main() -> Result<ExitCode, Error> {
    let mut args = Args::parse();

    let mut conn = RpcConn::session_conn(Timeout::Infinite)?;

    if args.listen {
        return rpc::listen(&mut conn)
            .map_err(Into::into)
            .map(|_| ExitCode::SUCCESS);
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
        Some(rpc::request_supported(&mut conn).and_then(|id| rpc::wait_supported(&mut conn, id))?.1)
    }
    else { None };

    if let Some(mut list) = list {
        list.par_sort_unstable();

        let s = list.iter().fold(vec![0; 0], |mut v, b| {
            v.extend_from_slice(b.as_bytes());
            v.push(b'\n');
            v
        });

        io::stdout().lock().write_all(&s)?;

        return Ok(ExitCode::SUCCESS)
    }

    if args.paths.is_empty() {
        args.paths = io::stdin().lock()
            .lines()
            .map_while(Result::ok)
            .map(PathBuf::from)
            .collect();

        if let Some(path) = args.paths.first() {
            if !path.exists() {
                println!("\"{}\": No such file or directory", path.to_str().unwrap());

                return Ok(ExitCode::FAILURE)
            }
        }
    }

    if !args.paths.is_empty() {
        if args.unchecked {
            create_all(&mut conn, args.paths, &args.flavor, &args.scheduler)?;
        } else {
            create_missing(&mut conn, args.paths, &args.flavor, &args.scheduler)?;
        }
    }

    Ok(ExitCode::SUCCESS)
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

fn thumbnail_is_valid(p_meta: Metadata, t: impl AsRef<Path>) -> bool {
    let Ok(mut fd) = File::open(t) else {
        return false
    };

    let mut buf = vec![0; 1024];

    fd.read(&mut buf).unwrap();

    let Some(time) = png::mtime(&buf) else {
        return false
    };

    let modified = p_meta.modified().unwrap();

    let p_secs = modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    let Some(t_secs) = time.split('.').next().and_then(|s| s.parse::<u64>().ok()) else {
        return false
    };

    p_secs == t_secs
}

fn mime(b: &[u8]) {
    let cookie = Cookie::open(CookieFlags::ERROR | CookieFlags::MIME_TYPE).expect("opening libmagic cookie");
    cookie.load::<&str>(&[]).expect("loading cookie database");

    cookie.buffer(b).unwrap();
}

enum EntryType {
    StatPath,
    OpenThumbnail,
    ReadThumbnail,
    OpenFile,
    ReadFile,
}

struct UserData {
    op:     EntryType,
    uri:    String,
    p:      CString,
    p_stat: statx,
    p_buf:  Vec<u8>,
    t:      CString,
    t_buf:  Vec<u8>
}

impl Default for UserData {
    fn default() -> Self {
        unsafe { UserData {
            op:     EntryType::StatPath,
            uri:    String::default(),
            p:      CString::default(),
            p_stat: std::mem::zeroed::<statx>(),
            p_buf:  Vec::default(),
            t:      CString::default(),
            t_buf:  Vec::default(),
        }}
    }
}

fn create_missing(conn: &mut RpcConn, paths: Vec<PathBuf>, flavor: &str, scheduler: &str) -> Result<(), Error> {
    let request_id = rpc::request_supported(conn)?;

    let mut ring: IoUring<squeue::Entry, cqueue::Entry> =
        IoUring::builder()
        .setup_sqpoll(10)
        .build(max(1024, paths.len() as u32))?;

    let pwd = Fd(AT_FDCWD);
    let to_process = Arc::new(AtomicUsize::new(paths.len()));

    paths.into_iter().for_each(|p| unsafe {
        let p = CString::from_vec_unchecked(p.into_os_string().into_vec());

        let user_data = Box::new(UserData { op: EntryType::StatPath, p, ..UserData::default()});
        let user_ref  = Box::leak(user_data);

        let op = opcode::Statx::new(pwd, user_ref.p.as_ptr(), &user_ref.p_stat as *const libc::statx as *mut _).build().user_data(user_ref as *const _ as _);
        ring.submission().push(&op).expect("submission queue is full");
    });

    ring.submit()?;

    let mut thumbnails_dir = cache_dir();
    thumbnails_dir.push("thumbnails");
    thumbnails_dir.push(flavor);

    let mut thumbnails_dir = thumbnails_dir.into_os_string().into_vec();
    let thumbnails_dir_end = thumbnails_dir.len();
    thumbnails_dir.resize(thumbnails_dir.len() + 38, 0);

    let work = Arc::new(Mutex::new(Vec::<UserData>::new()));

    {

    let to_process = to_process.clone();
    let work = work.clone();

    thread::spawn(move || {
        while to_process.load(Ordering::Acquire) != 0 { for entry in unsafe { ring.completion_shared() } {
            let mut user_data = unsafe { Box::from_raw(entry.user_data() as *mut UserData) };

            match user_data.op {
                EntryType::StatPath => {
                    if entry.result() != 0 || !c::is_file(user_data.p_stat.stx_mode as u32) {
                        to_process.fetch_sub(1, Ordering::Release);

                        continue;
                    }

                    let p = Path::new(OsStr::from_bytes(user_data.p.as_bytes()));

                    let Ok(abs) = p.absolutize() else {
                        to_process.fetch_sub(1, Ordering::Release);

                        continue
                    };

                    let mut uri = String::from(URI_PREFIX);
                    uri.push_str(abs.to_str().unwrap());

                    let sum = format!("/{:x}.png", md5::compute(&uri));

                    let mut thumbnail = thumbnails_dir.clone();
                    thumbnail[thumbnails_dir_end..thumbnails_dir_end + 37].copy_from_slice(sum.as_bytes());

                    user_data.op = EntryType::OpenThumbnail;
                    user_data.uri = uri;
                    user_data.t  = unsafe { CString::from_vec_with_nul_unchecked(thumbnail) };

                    let user_ref = Box::leak(user_data);

                    let op = opcode::OpenAt::new(pwd, user_ref.t.as_ptr()).flags(O_RDONLY).build().user_data(user_ref as *const _ as _);

                    unsafe { ring.submission_shared().push(&op).expect("submission queue is full"); }
                }
                EntryType::OpenThumbnail => {
                    let op = if entry.result().is_negative() {
                        user_data.op = EntryType::OpenFile;
                        let user_ref = Box::leak(user_data);

                        opcode::OpenAt::new(pwd, user_ref.p.as_ptr()).flags(O_RDONLY).build().user_data(user_ref as *const _ as _)
                    } else {
                        user_data.op = EntryType::ReadThumbnail;
                        user_data.t_buf = vec![0; 1024];
                        let user_ref = Box::leak(user_data);

                        opcode::Read::new(Fd(entry.result()), user_ref.t_buf.as_mut_ptr(), user_ref.t_buf.len() as _).build().user_data(user_ref as *const _ as _)
                    };

                    unsafe { ring.submission_shared().push(&op).expect("submission queue is full"); }
                }
                EntryType::ReadThumbnail => {
                    if entry.result().is_positive() {
                        let t_mtime = png::mtime(&user_data.t_buf);
                        let t_mtime_sec = t_mtime.and_then(|s| s.split('.').next().and_then(|s| s.parse::<u64>().ok()));

                        if t_mtime_sec == Some(user_data.p_stat.stx_mtime.tv_sec as u64) {
                            to_process.fetch_sub(1, Ordering::Release);

                            continue
                        }
                    }

                    user_data.op = EntryType::OpenFile;
                    let user_ref = Box::leak(user_data);

                    let op = opcode::OpenAt::new(pwd, user_ref.p.as_ptr()).flags(O_RDONLY).build().user_data(user_ref as *const _ as _);
                    unsafe { ring.submission_shared().push(&op).expect("submission queue is full"); }
                }
                EntryType::OpenFile => {
                    if entry.result().is_positive() {
                        user_data.p_buf.resize(1024, 0);

                        user_data.op = EntryType::ReadFile;
                        let user_ref = Box::leak(user_data);

                        let op = opcode::Read::new(Fd(entry.result()), user_ref.p_buf.as_mut_ptr(), 1024).build().user_data(user_ref as *const _ as _);
                        unsafe { ring.submission_shared().push(&op).expect("submission queue is full"); }
                    }
                    else { to_process.fetch_sub(1, Ordering::Release); }
                }
                EntryType::ReadFile => {
                    if entry.result().is_positive() {
                        let mut lock = work.lock().unwrap();
                        lock.push(*user_data);
                    }
                    else { to_process.fetch_sub(1, Ordering::Release); }
                }
            }
        }}
    });

    }

    let mtx = Mutex::new((vec![], vec![]));

    let (_, supported) = rpc::wait_supported(conn, request_id)?;
    let supported = HashSet::<String>::from_iter(supported);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .spawn_handler(|thread| {
            std::thread::spawn(|| thread.run());
            Ok(())
        })
        .build()?;

    pool.broadcast(|_| {
        let cookie = magic::Cookie::open(CookieFlags::ERROR | CookieFlags::MIME_TYPE).unwrap();
        cookie.load::<&str>(&[]).expect("loading cookie database");

        while to_process.load(Ordering::Acquire) != 0 {
            let user_data = { 
                let mut lock = work.lock().unwrap();
                lock.pop()
            };

            let Some(user_data) = user_data else {
                continue
            };

            if let Ok(mime) = cookie.buffer(&user_data.p_buf) {
                if supported.contains(&mime) {
                    to_process.fetch_sub(1, Ordering::AcqRel);

                    let mut lock = mtx.lock().expect("locking vectors to push new uri and mime");

                    lock.0.push(user_data.uri);
                    lock.1.push(mime);
                }
            }
        }
    });

    let (uris, mimes) = mtx.into_inner().unwrap();

    // if !uris.is_empty() {
        // rpc::queue_thumbnails(conn, uris, mimes, flavor, scheduler)?;
    // }

    Ok(())
}

fn create_all(conn: &mut RpcConn, paths: Vec<PathBuf>, flavor: &str, scheduler: &str) -> Result<(), Error> {
    let mtx = Mutex::new((vec![], vec![]));

    let request_id = rpc::request_supported(conn)?;
    let (_, supported) = rpc::wait_supported(conn, request_id)?;
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

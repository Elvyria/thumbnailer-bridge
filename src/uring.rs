use std::path::PathBuf;

use anyhow::Error;
use io_uring::{opcode::{self, Statx}, types::{self, Fd}, IoUring};
use rustbus::RpcConn;

fn create_missing(conn: &mut RpcConn, paths: Vec<PathBuf>, flavor: &str, scheduler: &str) -> Result<(), Error> {

    let ring = IoUring::new(64)?;

    // let fd = fs::File::open("README.md")?;
    // let mut buf = vec![0; 1024];

    // let statx = Statx::new(Fd(fd.as_raw_fd()), buf.as_mut_ptr(), buf.len() as _).build();


        // let buf = vec![0; 1024];


            // File::open(thumbnail).;

            // let statx = opcode::Statx::new(types::Fd(fd.as_raw_fd()), buf.as_mut_ptr(), buf.len() as _).build();

            // unsafe {
                // ring.submission().push(&read_e);
            // }



    Ok(())
}

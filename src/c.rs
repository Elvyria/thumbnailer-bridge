use libc::{S_IFREG, S_IFMT};

pub fn is_file(stx_mode: u32) -> bool {
    stx_mode & S_IFMT == S_IFREG
}

use std::borrow::Cow::{self, Borrowed};

pub fn mtime(mut b: &[u8]) -> Option<Cow<'_, str>> {

    if b.len() <= 12 { return None }

    // Header
    b = &b[8..];

    loop {
        // Chunk Length
        let len = u32::from_be_bytes(b[..4].try_into().unwrap()) as usize;

        if b[4..8] == *b"tEXt".as_slice() {
            let mut iter = b[8..len + 8].split(|b| *b == 0).map(String::from_utf8_lossy);

            if iter.next() == Some(Borrowed("Thumb::MTime")) {
                return iter.next()
            }
        }

        match b.get(len + 12..).filter(|b| b.len() > 12) {
            Some(r) => b = r,
            None => break
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow::Borrowed;

    #[test]
    fn mtime() {
        let b = include_bytes!("../assets/test_thumbnail.png");

        assert_eq!(Some(Borrowed("1664435861.573808")), super::mtime(b))
    }
}

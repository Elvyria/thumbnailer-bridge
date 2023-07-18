use std::path::Path;

static EXCLUDED: [[u8; 3]; 0x3F] = { 
    let mut b = [[0u8; 3]; 0x3F];

    b[0x00] = *b"%00"; b[0x01] = *b"%01"; b[0x02] = *b"%02";
    b[0x03] = *b"%03"; b[0x04] = *b"%04"; b[0x05] = *b"%05";
    b[0x06] = *b"%06"; b[0x07] = *b"%07"; b[0x08] = *b"%08";
    b[0x09] = *b"%09"; b[0x0A] = *b"%0A"; b[0x0B] = *b"%0B";
    b[0x0C] = *b"%0C"; b[0x0D] = *b"%0D"; b[0x0E] = *b"%0E";
    b[0x0F] = *b"%0F"; b[0x11] = *b"%11"; b[0x12] = *b"%12";
    b[0x13] = *b"%13"; b[0x14] = *b"%14"; b[0x15] = *b"%15";
    b[0x16] = *b"%16"; b[0x17] = *b"%17"; b[0x18] = *b"%18";
    b[0x19] = *b"%19"; b[0x1A] = *b"%1A"; b[0x1B] = *b"%1B";
    b[0x1C] = *b"%1C"; b[0x1D] = *b"%1D"; b[0x1E] = *b"%1E";
    b[0x1F] = *b"%1F";

    b[' ' as usize] = *b"%20";
    b['"' as usize] = *b"%22";
    b['#' as usize] = *b"%23";
    b['%' as usize] = *b"%25";
    b['<' as usize] = *b"%3C";
    b['>' as usize] = *b"%3E";

    b
};

pub const FILE_PREFIX: &str = "file://";

pub fn file(p: impl AsRef<Path>) -> Option<String> {
    let p = p.as_ref().to_str()?;

    let mut b = Vec::with_capacity(FILE_PREFIX.len() + p.len());
    b.extend_from_slice(FILE_PREFIX.as_bytes());

    let mut last = 0;

    for (i, c) in p.char_indices() {
        if (c as usize) < EXCLUDED.len() && EXCLUDED[c as usize][0] == b'%' {
            b.extend_from_slice(&p.as_bytes()[last..i]);
            b.extend_from_slice(&EXCLUDED[c as usize]);

            last = i + 1;
        }
    }

    b.extend_from_slice(&p.as_bytes()[last..]);

    Some(unsafe { String::from_utf8_unchecked(b) })
}

#[cfg(test)]
mod tests {
    use crate::uri;

    #[test]
    fn file() {
        assert_eq!(Some("file:///home/user/pictures/cats%20in%20space/69%25.jpg".to_string()), uri::file("/home/user/pictures/cats in space/69%.jpg"));
        assert_eq!(Some("file:///home/user/novels/Rebuild%20World/リビルドワールドIII〈上〉　埋もれた遺跡.epub".to_string()), uri::file("/home/user/novels/Rebuild World/リビルドワールドIII〈上〉　埋もれた遺跡.epub"));

        assert_ne!(Some("file:///home/user/screenshots/Fallout 4/gun.png".to_string()), uri::file("/home/user/screenshots/Fallout 4/gun.png"));
        assert_ne!(Some("/home/user/videos/Gotta Stay Optimistic.mp4".to_string()), uri::file("/home/user/videos/Gotta Stay Optimistic.mp4"));
        assert_ne!(Some("/home/user/videos/(ﾉ◕ヮ◕)ﾉ＊：･ﾟ✧.webm".to_string()), uri::file("/home/user/videos/(ﾉ◕ヮ◕)ﾉ＊：･ﾟ✧.webm"));
    }
}

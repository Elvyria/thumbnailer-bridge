use std::path::Path;

static EXCLUDED: [&str; 0x3F] = { 
    let mut b = [""; 0x3F];

    b[0x00] = "00"; b[0x01] = "01"; b[0x02] = "02";
    b[0x03] = "03"; b[0x04] = "04"; b[0x05] = "05";
    b[0x06] = "06"; b[0x07] = "07"; b[0x08] = "08";
    b[0x09] = "09"; b[0x0A] = "0A"; b[0x0B] = "0B";
    b[0x0C] = "0C"; b[0x0D] = "0D"; b[0x0E] = "0E";
    b[0x0F] = "0F"; b[0x11] = "11"; b[0x12] = "12";
    b[0x13] = "13"; b[0x14] = "14"; b[0x15] = "15";
    b[0x16] = "16"; b[0x17] = "17"; b[0x18] = "18";
    b[0x19] = "19"; b[0x1A] = "1A"; b[0x1B] = "1B";
    b[0x1C] = "1C"; b[0x1D] = "1D"; b[0x1E] = "1E";
    b[0x1F] = "1F";

    b[' ' as usize] = "20";
    b['"' as usize] = "22";
    b['#' as usize] = "23";
    b['%' as usize] = "25";
    b['<' as usize] = "3C";
    b['>' as usize] = "3E";

    b
};

pub const FILE_PREFIX: &str = "file://";

pub fn file(p: impl AsRef<Path>) -> Option<String> {
    let p = p.as_ref().to_str()?;

    let mut uri = p.to_owned();
    let mut d = 0;

    for (i, c) in p.char_indices() {
        if (c as usize) < EXCLUDED.len() && !EXCLUDED[c as usize].is_empty() {
            unsafe { uri.as_bytes_mut()[i + d] = b'%' };
            uri.insert_str(i + d + 1, EXCLUDED[c as usize]);
            d += 2;
        }
    }

    uri.insert_str(0, FILE_PREFIX);

    Some(uri)
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

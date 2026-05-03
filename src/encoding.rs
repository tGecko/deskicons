use std::path::{Path, PathBuf};

use crate::path_display;

pub fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'.'
            | b'-'
            | b'_'
            | b' '
            | b'\\'
            | b'/'
            | b':' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn percent_decode(value: &str) -> String {
    let mut out = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

pub fn encode_path(path: &Path) -> String {
    percent_encode(&path_display(path))
}

pub fn decode_path(value: &str) -> PathBuf {
    PathBuf::from(percent_decode(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_round_trips_unicode_names() {
        let value = r#"Desktop\ä-東京-😀 %.txt"#;
        let encoded = percent_encode(value);
        assert_ne!(encoded, value);
        assert_eq!(percent_decode(&encoded), value);
        assert!(encoded.contains("%C3%A4"));
        assert!(encoded.contains("%F0%9F%98%80"));
    }

    #[test]
    fn percent_decode_preserves_invalid_utf8_lossily() {
        assert_eq!(percent_decode("%FF.txt"), "�.txt");
    }
}

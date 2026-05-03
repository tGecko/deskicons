use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::encoding::{percent_decode, percent_encode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IconPosition {
    pub x: i32,
    pub y: i32,
}

pub fn encode_layout_name(name: &str) -> String {
    percent_encode(name)
}

pub fn load_layout_file(path: &Path) -> (BTreeMap<String, IconPosition>, usize) {
    let mut result = BTreeMap::new();
    let mut skipped = 0;
    let Ok(file) = File::open(path) else {
        return (result, skipped);
    };
    for line in BufReader::new(file)
        .lines()
        .map_while(std::result::Result::ok)
    {
        let parts: Vec<_> = line.split('\t').collect();
        if parts.len() >= 3 {
            if let (Ok(x), Ok(y)) = (parts[1].parse::<i32>(), parts[2].parse::<i32>()) {
                result.insert(percent_decode(parts[0]), IconPosition { x, y });
            } else {
                skipped += 1;
            }
        } else {
            skipped += 1;
        }
    }
    (result, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn load_layout_file_decodes_unicode_names_and_skips_bad_lines() {
        let path =
            std::env::temp_dir().join(format!("deskicons-layout-{}.tsv", std::process::id()));
        fs::write(
            &path,
            format!(
                "{}\t10\t20\nbad-line\n{}\tx\t1\n",
                encode_layout_name("ä-東京-😀.txt"),
                encode_layout_name("bad.txt")
            ),
        )
        .unwrap();

        let (layout, skipped) = load_layout_file(&path);

        assert_eq!(layout["ä-東京-😀.txt"], IconPosition { x: 10, y: 20 });
        assert_eq!(skipped, 2);
        let _ = fs::remove_file(path);
    }
}

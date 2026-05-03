use std::fs;
use std::path::{Path, PathBuf};

use crate::path_display;

pub fn normalized_path(path: &Path) -> Option<PathBuf> {
    fs::canonicalize(path).ok().or_else(|| {
        path.parent()
            .and_then(|parent| fs::canonicalize(parent).ok())
            .and_then(|parent| path.file_name().map(|name| parent.join(name)))
    })
}

pub fn path_equal_ci(a: &Path, b: &Path) -> bool {
    path_display(a).eq_ignore_ascii_case(&path_display(b))
}

pub fn is_under_dir(child: &Path, parent: &Path, strict: bool) -> bool {
    let child_norm = normalized_path(child).unwrap_or_else(|| child.to_path_buf());
    let parent_norm = normalized_path(parent).unwrap_or_else(|| parent.to_path_buf());
    if path_equal_ci(&child_norm, &parent_norm) {
        return !strict;
    }
    child_norm
        .ancestors()
        .skip(1)
        .any(|ancestor| path_equal_ci(ancestor, &parent_norm))
}

pub fn relative_name_for_desktop_item(item_path: &Path, desktop: &Path) -> String {
    let item_norm = normalized_path(item_path).unwrap_or_else(|| item_path.to_path_buf());
    let desktop_norm = normalized_path(desktop).unwrap_or_else(|| desktop.to_path_buf());
    if !is_under_dir(&item_norm, &desktop_norm, true) {
        return String::new();
    }
    let item_parts: Vec<_> = item_norm.components().collect();
    let desktop_len = desktop_norm.components().count();
    if item_parts.len() <= desktop_len {
        return String::new();
    }
    let mut rel = PathBuf::new();
    for part in item_parts.iter().skip(desktop_len) {
        rel.push(part.as_os_str());
    }
    path_display(&rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("deskicons-path-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn relative_name_handles_unicode_children() {
        let desktop = temp_dir("unicode");
        let item = desktop.join("folder").join("ä-東京-😀.txt");
        fs::create_dir_all(item.parent().unwrap()).unwrap();
        fs::write(&item, "x").unwrap();

        assert_eq!(
            relative_name_for_desktop_item(&item, &desktop),
            path_display(Path::new("folder").join("ä-東京-😀.txt").as_path())
        );
        let _ = fs::remove_dir_all(desktop);
    }

    #[test]
    fn containment_rejects_sibling_paths() {
        let parent = temp_dir("parent");
        let sibling = parent.parent().unwrap().join(format!(
            "{}-sibling",
            parent.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir_all(&sibling).unwrap();

        assert!(!is_under_dir(&sibling.join("file.txt"), &parent, true));
        let _ = fs::remove_dir_all(parent);
        let _ = fs::remove_dir_all(sibling);
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};
use crate::path_display;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

pub fn is_skipped_desktop_entry(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("desktop.ini"))
}

pub fn child_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let Ok(read_dir) = fs::read_dir(dir) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        if !is_skipped_desktop_entry(&entry.path()) {
            entries.push(entry.path());
        }
    }
    entries.sort();
    entries
}

pub fn should_manage_entry(path: &Path, manage_non_shortcuts: bool) -> bool {
    manage_non_shortcuts
        || path
            .extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("lnk"))
}

pub fn validate_moves(moves: &[PlannedMove]) -> Result<()> {
    for mv in moves {
        if !mv.from.exists() {
            return Err(AppError::message(format!(
                "Source disappeared before move: {}",
                path_display(&mv.from)
            )));
        }
        if mv.to.exists() {
            return Err(AppError::message(format!(
                "Refusing to overwrite existing path: {}",
                path_display(&mv.to)
            )));
        }
    }
    Ok(())
}

pub fn move_path(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(rename_err) if from.is_dir() => {
            copy_dir_all(from, to).map_err(|copy_err| {
                AppError::message(format!(
                    "Could not copy directory {} -> {} after rename failed ({rename_err}): {copy_err}",
                    path_display(from),
                    path_display(to)
                ))
            })?;
            fs::remove_dir_all(from).map_err(|remove_err| {
                AppError::message(format!(
                    "Copied directory to {}, but could not remove original {}: {remove_err}",
                    path_display(to),
                    path_display(from)
                ))
            })?;
            Ok(())
        }
        Err(rename_err) => {
            fs::copy(from, to).map_err(|copy_err| {
                AppError::message(format!(
                    "Could not copy file {} -> {} after rename failed ({rename_err}): {copy_err}",
                    path_display(from),
                    path_display(to)
                ))
            })?;
            fs::remove_file(from).map_err(|remove_err| {
                AppError::message(format!(
                    "Copied file to {}, but could not remove original {}: {remove_err}",
                    path_display(to),
                    path_display(from)
                ))
            })?;
            Ok(())
        }
    }
}

pub fn move_completed_or_finish(mv: &PlannedMove) -> Result<bool> {
    let src_exists = mv.from.exists();
    let dst_exists = mv.to.exists();
    if src_exists && dst_exists {
        return Err(AppError::message(format!(
            "Recovery conflict: both source and destination exist for {}",
            path_display(&mv.from)
        )));
    }
    if src_exists && !dst_exists {
        move_path(&mv.from, &mv.to)?;
        return Ok(true);
    }
    if !src_exists && !dst_exists {
        return Err(AppError::message(format!(
            "Recovery lost both source and destination: {} -> {}",
            path_display(&mv.from),
            path_display(&mv.to)
        )));
    }
    Ok(dst_exists)
}

pub fn finish_move_set(moves: &[PlannedMove]) -> Result<()> {
    for mv in moves {
        move_completed_or_finish(mv)?;
    }
    Ok(())
}

pub fn rollback_move_set(moves: &[PlannedMove]) -> Result<()> {
    for mv in moves.iter().rev() {
        move_completed_or_finish(&PlannedMove {
            from: mv.to.clone(),
            to: mv.from.clone(),
        })?;
    }
    Ok(())
}

pub fn copy_dir_all(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = to.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("deskicons-fs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn move_file_creates_destination_parent() {
        let base = temp_dir("move-file");
        let from = base.join("a.txt");
        let to = base.join("nested").join("b.txt");
        fs::write(&from, "hello").unwrap();

        move_path(&from, &to).unwrap();

        assert!(!from.exists());
        assert_eq!(fs::read_to_string(to).unwrap(), "hello");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn child_entries_skips_desktop_ini_and_sorts() {
        let base = temp_dir("children");
        fs::write(base.join("z.txt"), "").unwrap();
        fs::write(base.join("Desktop.ini"), "").unwrap();
        fs::write(base.join("a.txt"), "").unwrap();

        let entries: Vec<_> = child_entries(&base)
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(entries, vec!["a.txt", "z.txt"]);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn shortcut_only_policy_is_case_insensitive() {
        assert!(should_manage_entry(Path::new("App.LNK"), false));
        assert!(!should_manage_entry(Path::new("notes.txt"), false));
        assert!(should_manage_entry(Path::new("notes.txt"), true));
    }

    #[test]
    fn recovery_errors_when_both_move_paths_are_missing() {
        let base = temp_dir("missing");
        let mv = PlannedMove {
            from: base.join("missing-source"),
            to: base.join("missing-destination"),
        };
        let err = move_completed_or_finish(&mv).unwrap_err();
        assert!(
            err.to_string()
                .contains("Recovery lost both source and destination")
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn validate_moves_rejects_overwrite() {
        let base = temp_dir("overwrite");
        let from = base.join("from.txt");
        let to = base.join("to.txt");
        fs::write(&from, "from").unwrap();
        fs::write(&to, "to").unwrap();

        let err = validate_moves(&[PlannedMove { from, to }]).unwrap_err();

        assert!(err.to_string().contains("Refusing to overwrite"));
        let _ = fs::remove_dir_all(base);
    }
}

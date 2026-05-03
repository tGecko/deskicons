use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    En,
    De,
}

pub fn language_code(lang: Language) -> &'static str {
    match lang {
        Language::En => "en",
        Language::De => "de",
    }
}

pub fn language_from_code(code: &str) -> Language {
    if code == "de" {
        Language::De
    } else {
        Language::En
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub manage_non_shortcuts: bool,
    pub poll_ms: u64,
    pub language: Option<Language>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            manage_non_shortcuts: true,
            poll_ms: 750,
            language: None,
        }
    }
}

pub fn trim_ascii(value: &str) -> &str {
    value.trim_matches(|c: char| c.is_ascii_whitespace())
}

pub fn load_config_file(path: &Path) -> Config {
    let Ok(file) = File::open(path) else {
        return Config::default();
    };
    let mut config = Config::default();
    for line in BufReader::new(file)
        .lines()
        .map_while(std::result::Result::ok)
    {
        let trimmed = trim_ascii(&line);
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        match trim_ascii(key) {
            "enabled" => config.enabled = trim_ascii(value) != "0",
            "manage_non_shortcuts" => config.manage_non_shortcuts = trim_ascii(value) != "0",
            "poll_ms" => {
                config.poll_ms = trim_ascii(value)
                    .parse::<u64>()
                    .unwrap_or(750)
                    .clamp(250, 10_000);
            }
            "language" => config.language = Some(language_from_code(trim_ascii(value))),
            _ => {}
        }
    }
    config
}

pub fn save_config_file(root: &Path, path: &Path, config: &Config) -> Result<()> {
    fs::create_dir_all(root)?;
    let mut out = File::create(path)?;
    writeln!(out, "enabled={}", if config.enabled { 1 } else { 0 })?;
    writeln!(
        out,
        "manage_non_shortcuts={}",
        if config.manage_non_shortcuts { 1 } else { 0 }
    )?;
    writeln!(out, "poll_ms={}", config.poll_ms)?;
    if let Some(language) = config.language {
        writeln!(out, "language={}", language_code(language))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parser_clamps_poll_and_reads_language() {
        let path =
            std::env::temp_dir().join(format!("deskicons-config-{}.ini", std::process::id()));
        fs::write(
            &path,
            "enabled=0\nmanage_non_shortcuts=0\npoll_ms=5\nlanguage=de\nunknown=ignored\n",
        )
        .unwrap();

        let config = load_config_file(&path);

        assert!(!config.enabled);
        assert!(!config.manage_non_shortcuts);
        assert_eq!(config.poll_ms, 250);
        assert_eq!(config.language, Some(Language::De));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn config_save_round_trips_values() {
        let root =
            std::env::temp_dir().join(format!("deskicons-config-root-{}", std::process::id()));
        let path = root.join("config.ini");
        let config = Config {
            enabled: false,
            manage_non_shortcuts: false,
            poll_ms: 1250,
            language: Some(Language::En),
        };

        save_config_file(&root, &path, &config).unwrap();

        assert_eq!(load_config_file(&path), config);
        let _ = fs::remove_dir_all(root);
    }
}

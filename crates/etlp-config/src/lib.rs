//! INI configuration loading and string-match rules for etlp.
//!
//! Wraps the on-disk `embyToLocalPlayer*.ini` file and exposes typed getters
//! plus the [`matching`] helpers used throughout the pipeline. Parsing of the
//! match rules lives in [`matching`] so it can be tested without IO.

pub mod matching;

use std::path::{Path, PathBuf};

use ini::Ini;
use thiserror::Error;

/// Errors raised while locating or parsing the configuration file.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// No candidate ini file existed in the search directory.
    #[error("no config file found in {0}")]
    NotFound(PathBuf),

    /// The file could not be read.
    #[error("failed to read config {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The file was not valid INI.
    #[error("failed to parse config {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: ini::ParseError,
    },
}

/// Convenience alias for config results.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Loaded configuration backed by an INI file.
#[derive(Debug, Clone)]
pub struct Config {
    ini: Ini,
    path: PathBuf,
}

/// The Python `platform.system()` name used in the platform-specific ini file
/// (`embyToLocalPlayer-<Platform>.ini`).
#[must_use]
pub fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "Darwin",
        _ => "Linux",
    }
}

/// Candidate config file names, in the same priority order as the Python
/// implementation.
fn candidate_names() -> [String; 3] {
    [
        format!("embyToLocalPlayer-{}.ini", platform_name()),
        "embyToLocalPlayer.ini".to_owned(),
        "embyToLocalPlayer_config.ini".to_owned(),
    ]
}

impl Config {
    /// Load the first existing candidate config from `dir`.
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        for name in candidate_names() {
            let path = dir.join(&name);
            if path.is_file() {
                return Self::load_file(&path);
            }
        }
        Err(ConfigError::NotFound(dir.to_path_buf()))
    }

    /// Load a specific config file, tolerating a UTF-8 BOM (`utf-8-sig`).
    pub fn load_file(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|source| {
            ConfigError::Io {
                path: path.to_path_buf(),
                source,
            }
        })?;
        let trimmed = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
        // Disable backslash escaping so Windows paths like `F:\media` survive
        // verbatim (the ini format does not unescape backslashes).
        let opt = ini::ParseOption {
            enabled_quote: false,
            enabled_escape: false,
            ..ini::ParseOption::default()
        };
        let ini = Ini::load_from_str_opt(trimmed, opt).map_err(|source| {
            ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            }
        })?;
        Ok(Self {
            ini,
            path: path.to_path_buf(),
        })
    }

    /// Reload from the originally loaded path.
    pub fn reload(&mut self) -> Result<()> {
        let fresh = Self::load_file(&self.path)?;
        self.ini = fresh.ini;
        Ok(())
    }

    /// The path this config was loaded from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Raw option value, if present.
    #[must_use]
    pub fn get(&self, section: &str, option: &str) -> Option<&str> {
        self.ini.get_from(Some(section), option)
    }

    /// Raw option value or a default.
    #[must_use]
    pub fn get_or<'a>(
        &'a self,
        section: &str,
        option: &str,
        default: &'a str,
    ) -> &'a str {
        self.get(section, option).unwrap_or(default)
    }

    /// Boolean option using configparser-style truthy strings.
    #[must_use]
    pub fn get_bool(&self, section: &str, option: &str, default: bool) -> bool {
        self.get(section, option)
            .and_then(parse_bool)
            .unwrap_or(default)
    }

    /// Integer option, falling back to `default` on absence or parse failure.
    #[must_use]
    pub fn get_int(&self, section: &str, option: &str, default: i64) -> i64 {
        self.get(section, option)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Float option, falling back to `default` on absence or parse failure.
    #[must_use]
    pub fn get_float(&self, section: &str, option: &str, default: f64) -> f64 {
        self.get(section, option)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Split an option into trimmed tokens ([`matching::split_list`]).
    #[must_use]
    pub fn split_list(
        &self,
        section: &str,
        option: &str,
        split_by: char,
    ) -> Vec<String> {
        matching::split_list(self.get_or(section, option, ""), split_by)
    }

    /// All `(key, value)` entries in a section, in file order. Used to read the
    /// `[src]` / `[dst]` path-translation tables.
    #[must_use]
    pub fn section_entries(&self, section: &str) -> Vec<(String, String)> {
        match self.ini.section(Some(section)) {
            Some(props) => props
                .iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Pair the ordered `[src]` prefixes with their matching `[dst]` prefixes
    /// by key, producing `(src_prefix, dst_prefix)` translation pairs.
    #[must_use]
    pub fn path_translation_pairs(&self) -> Vec<(String, String)> {
        let dst = self.ini.section(Some("dst"));
        self.section_entries("src")
            .into_iter()
            .filter_map(|(key, src_prefix)| {
                dst.and_then(|d| d.get(&key))
                    .map(|dst_prefix| (src_prefix, dst_prefix.to_owned()))
            })
            .collect()
    }
}

/// Parse a configparser-style boolean (`1/yes/true/on`, `0/no/false/off`),
/// case-insensitively. Returns `None` for unrecognized values.
#[must_use]
pub fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "yes" | "true" | "on" => Some(true),
        "0" | "no" | "false" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;

    use super::*;

    const SAMPLE: &str = "\
[emby]
player = mpv
fullscreen = no
update_progress = yes

[playlist]
item_limit = 50

[dev]
version_prefer = VCB, Baha
speed = 1.5
";

    fn write_config(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create temp config");
        f.write_all(body.as_bytes()).expect("write temp config");
        path
    }

    #[test]
    fn parse_bool_truthy_and_falsy() {
        assert_eq!(parse_bool("YES"), Some(true));
        assert_eq!(parse_bool(" on "), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("maybe"), None);
    }

    #[test]
    fn typed_getters_read_values() {
        let dir = tempdir().expect("tempdir");
        write_config(dir.path(), "embyToLocalPlayer.ini", SAMPLE);
        let cfg = Config::load_from_dir(dir.path()).expect("load");

        assert_eq!(cfg.get("emby", "player"), Some("mpv"));
        assert!(!cfg.get_bool("emby", "fullscreen", true));
        assert!(cfg.get_bool("emby", "update_progress", false));
        assert_eq!(cfg.get_int("playlist", "item_limit", -1), 50);
        assert!((cfg.get_float("dev", "speed", 1.0) - 1.5).abs() < 1e-9);
        assert_eq!(
            cfg.split_list("dev", "version_prefer", ','),
            vec!["VCB".to_string(), "Baha".to_string()]
        );
        // Missing option falls back.
        assert_eq!(cfg.get_or("dev", "absent", "def"), "def");
    }

    #[test]
    fn bom_is_tolerated() {
        let dir = tempdir().expect("tempdir");
        let body = format!("\u{feff}{SAMPLE}");
        write_config(dir.path(), "embyToLocalPlayer.ini", &body);
        let cfg = Config::load_from_dir(dir.path()).expect("load bom");
        assert_eq!(cfg.get("emby", "player"), Some("mpv"));
    }

    #[test]
    fn missing_config_dir_errors() {
        let dir = tempdir().expect("tempdir");
        let err = Config::load_from_dir(dir.path()).unwrap_err();
        assert!(matches!(err, ConfigError::NotFound(_)));
    }

    #[test]
    fn path_translation_pairs_zip_src_dst_by_key() {
        let body = "\
[src]
a = /mnt/disk1
b = /mnt/disk2/media

[dst]
a = E:
b = F:\\media
";
        let dir = tempdir().expect("tempdir");
        write_config(dir.path(), "embyToLocalPlayer.ini", body);
        let cfg = Config::load_from_dir(dir.path()).expect("load");
        assert_eq!(
            cfg.path_translation_pairs(),
            vec![
                ("/mnt/disk1".to_owned(), "E:".to_owned()),
                ("/mnt/disk2/media".to_owned(), "F:\\media".to_owned()),
            ]
        );
    }
}

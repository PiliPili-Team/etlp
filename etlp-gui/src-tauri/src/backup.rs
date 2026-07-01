//! Config backup, restore and reset.
//!
//! A backup is a `.zip` archive (named after the local time it was taken)
//! containing the single `config.toml`. Archives live under the data dir's
//! `backup/` folder; only the most recent [`MAX_BACKUPS`] are retained.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use etlp_server::platform;

/// Maximum number of backup archives kept on disk; older ones are pruned.
pub const MAX_BACKUPS: usize = 5;

/// Name of the single file stored inside each backup archive.
const CONFIG_ENTRY: &str = "config.toml";

/// One backup archive surfaced to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupEntry {
    /// Archive file name, e.g. `2026-06-20_15-30-45.zip`.
    pub name: String,
    /// Absolute path to the archive.
    pub path: String,
    /// Archive size in bytes.
    pub size: u64,
    /// Last-modified time in milliseconds since the Unix epoch.
    pub created_ms: u64,
}

/// Resolve the `backup/` directory, creating it if necessary.
fn backup_dir() -> Result<PathBuf, String> {
    let dir = platform::backup_dir()
        .ok_or_else(|| "cannot determine data directory".to_owned())?;
    match std::fs::create_dir_all(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Let elevated copy create the parent for backup writes. Read-only
            // listing still falls back to an empty list in list_backups().
        }
        Err(e) => return Err(format!("create backup dir: {e}")),
    }
    Ok(dir)
}

/// Resolve the active `config.toml` path.
fn config_path() -> Result<PathBuf, String> {
    let dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    Ok(etlp_config::existing_config_path(&dir)
        .unwrap_or_else(|| dir.join("config.toml")))
}

/// File's last-modified time in milliseconds since the Unix epoch (0 on error).
fn modified_ms(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// List existing backups, newest first.
pub fn list_backups() -> Result<Vec<BackupEntry>, String> {
    let dir = backup_dir()?;
    let mut out: Vec<BackupEntry> = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_zip = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("zip"))
            .unwrap_or(false);
        if !is_zip {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        out.push(BackupEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            size: meta.len(),
            created_ms: modified_ms(&meta),
        });
    }
    out.sort_by(|a, b| b.created_ms.cmp(&a.created_ms));
    Ok(out)
}

/// Remove the oldest archives until at most [`MAX_BACKUPS`] remain.
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut zips: Vec<(PathBuf, u64)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let is_zip = path
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("zip"))
                .unwrap_or(false);
            if !is_zip {
                return None;
            }
            let ms = e.metadata().map(|m| modified_ms(&m)).unwrap_or(0);
            Some((path, ms))
        })
        .collect();
    if zips.len() <= MAX_BACKUPS {
        return;
    }
    // Oldest first, then drop the leading surplus.
    zips.sort_by(|a, b| a.1.cmp(&b.1));
    for (path, _) in zips.iter().take(zips.len() - MAX_BACKUPS) {
        let _ = std::fs::remove_file(path);
    }
}

/// Back up the current config, returning the freshly created entry.
///
/// The archive is named after the local time and contains `config.toml`. When
/// no config exists yet the default template is written first so the backup is
/// never empty. Old archives beyond [`MAX_BACKUPS`] are pruned afterwards.
pub fn create_backup() -> Result<BackupEntry, String> {
    let cfg = config_path()?;
    let content = match std::fs::read_to_string(&cfg) {
        Ok(c) => c,
        Err(_) => {
            crate::commands::write_default_config(&cfg)?;
            std::fs::read_to_string(&cfg)
                .map_err(|e| format!("read config: {e}"))?
        }
    };

    let dir = backup_dir()?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    // Disambiguate sub-second repeats so two quick backups never collide.
    let mut name = format!("{stamp}.zip");
    let mut n = 1;
    while dir.join(&name).exists() {
        n += 1;
        name = format!("{stamp}_{n}.zip");
    }
    let path = dir.join(&name);

    write_zip_with_elevation(&path, &content)?;
    prune(&dir);

    let meta =
        std::fs::metadata(&path).map_err(|e| format!("stat backup: {e}"))?;
    Ok(BackupEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        size: meta.len(),
        created_ms: modified_ms(&meta),
    })
}

/// Write `content` into a new zip at `path` under the [`CONFIG_ENTRY`] name.
fn write_zip(path: &Path, content: &str) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            "create zip: permission denied".to_owned()
        } else {
            format!("create zip: {e}")
        }
    })?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::SimpleFileOptions =
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(CONFIG_ENTRY, opts)
        .map_err(|e| format!("zip entry: {e}"))?;
    zip.write_all(content.as_bytes())
        .map_err(|e| format!("zip write: {e}"))?;
    zip.finish().map_err(|e| format!("zip finish: {e}"))?;
    Ok(())
}

fn write_zip_with_elevation(path: &Path, content: &str) -> Result<(), String> {
    match write_zip(path, content) {
        Ok(()) => Ok(()),
        Err(e) if e.contains("permission denied") => {
            let tmp = std::env::temp_dir().join(format!(
                "etlp-backup-{}-{}.zip",
                std::process::id(),
                chrono::Local::now().timestamp_millis()
            ));
            write_zip(&tmp, content)?;
            let result = crate::elevated_fs::copy_file(&tmp, path);
            let _ = std::fs::remove_file(&tmp);
            result.map_err(|e| format!("create zip with elevation: {e}"))
        }
        Err(e) => Err(e),
    }
}

/// Read the `config.toml` payload from a backup zip at `path`.
fn read_zip(path: &Path) -> Result<String, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("read zip: {e}"))?;
    let mut entry = archive
        .by_name(CONFIG_ENTRY)
        .map_err(|_| format!("backup is missing {CONFIG_ENTRY}"))?;
    let mut content = String::new();
    entry
        .read_to_string(&mut content)
        .map_err(|e| format!("extract config: {e}"))?;
    Ok(content)
}

/// Restore the config from a backup zip at `path`.
///
/// Validates that the archive holds parseable TOML before overwriting the live
/// `config.toml`, so a corrupt or wrong archive cannot destroy a good config.
pub fn restore_backup(path: &str) -> Result<(), String> {
    let content = read_zip(Path::new(path))?;
    // Strip a BOM (Windows backups carry one) before validating.
    let body = content.strip_prefix('\u{feff}').unwrap_or(&content);
    body.parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("backup config is not valid TOML: {e}"))?;
    let cfg = config_path()?;
    match etlp_config::write_config_str(&cfg, body) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let tmp = std::env::temp_dir().join(format!(
                "etlp-restore-config-{}.toml",
                std::process::id()
            ));
            etlp_config::write_config_str(&tmp, body)
                .map_err(|e| format!("write temp config: {e}"))?;
            let result = crate::elevated_fs::copy_file(&tmp, &cfg);
            let _ = std::fs::remove_file(&tmp);
            result.map_err(|e| format!("write config with elevation: {e}"))
        }
        Err(e) => Err(format!("write config: {e}")),
    }
}

/// Delete a backup archive at `path`.
pub fn delete_backup(path: &str) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            crate::elevated_fs::remove_file(Path::new(path))
                .map_err(|e| format!("delete backup with elevation: {e}"))
        }
        Err(e) => Err(format!("delete backup: {e}")),
    }
}

/// Reset the live config to the bundled default template.
pub fn reset_config() -> Result<(), String> {
    let cfg = config_path()?;
    crate::commands::write_default_config(&cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_roundtrips_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("backup.zip");
        let body = "[emby]\nplayer = \"mpv\"\n";
        write_zip(&path, body).expect("write zip");
        assert!(path.is_file());
        let read = read_zip(&path).expect("read zip");
        assert_eq!(read, body);
    }

    #[test]
    fn read_zip_rejects_archive_without_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("empty.zip");
        let file = std::fs::File::create(&path).expect("create");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("other.txt", zip::write::SimpleFileOptions::default())
            .expect("entry");
        zip.write_all(b"nope").expect("write");
        zip.finish().expect("finish");

        assert!(read_zip(&path).is_err());
    }
}

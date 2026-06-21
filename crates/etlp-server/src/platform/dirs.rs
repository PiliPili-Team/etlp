//! Standard platform directories for etlp files.
//!
//! | Platform | Config               | Data                    |
//! |----------|----------------------|-------------------------|
//! | Linux    | `$XDG_CONFIG_HOME/etlp` (→ `~/.config/etlp`) | `$XDG_DATA_HOME/etlp` (→ `~/.local/share/etlp`) |
//! | macOS    | `~/.config/etlp`     | `~/.local/share/etlp`   |
//! | Windows  | `%APPDATA%\etlp`     | `%LOCALAPPDATA%\etlp`   |
//!
//! macOS deliberately uses XDG-style paths instead of `~/Library/Application Support`
//! so configuration files are easy to locate and edit from a terminal.

use std::path::PathBuf;

const APP_NAME: &str = "etlp";

/// Environment variable that distinguishes the packaged GUI app from the CLI
/// binary. Set to `"app"` by the Tauri entry point before any threads spawn;
/// absent or any other value means CLI binary.
pub const ENV_RUNTIME: &str = "ETLP_RUNTIME";

/// Whether the current process was launched as the CLI binary or the GUI app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Binary,
    App,
}

impl RuntimeMode {
    /// Read the current mode from [`ENV_RUNTIME`]. Defaults to `Binary`.
    #[must_use]
    pub fn detect() -> Self {
        if std::env::var(ENV_RUNTIME).as_deref() == Ok("app") {
            Self::App
        } else {
            Self::Binary
        }
    }

    #[must_use]
    pub fn is_app(self) -> bool {
        self == Self::App
    }
}

// macOS: prefer XDG-style dirs over ~/Library/Application Support.
// All other platforms delegate to the `dirs` crate which already follows XDG
// on Linux and uses %APPDATA% / %LOCALAPPDATA% on Windows.
#[cfg(target_os = "macos")]
fn base_config() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".config"))
}

#[cfg(not(target_os = "macos"))]
fn base_config() -> Option<PathBuf> {
    dirs::config_dir()
}

#[cfg(target_os = "macos")]
fn base_data() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".local").join("share"))
}

#[cfg(not(target_os = "macos"))]
fn base_data() -> Option<PathBuf> {
    dirs::data_dir()
}

/// Config directory: stores `config.toml`, Trakt/BGM credentials.
///
/// Returns `None` only when the home directory cannot be determined.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    base_config().map(|d| d.join(APP_NAME))
}

/// Data directory: the root for all runtime files. Sub-directories organise the
/// content: `log/` (etlp.log, mpv.log), `cache/` (per-feature caches such as
/// `cache/bangumi/`), `backup/` (config backup archives), plus credential files
/// (`device_id`, `trakt_token.json`) kept at the root.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    base_data().map(|d| d.join(APP_NAME))
}

/// `log/` sub-directory under `base` — holds `etlp.log` and `mpv.log`.
#[must_use]
pub fn log_dir_in(base: &std::path::Path) -> PathBuf {
    base.join("log")
}

/// `cache/` sub-directory under `base` — root for per-feature caches.
#[must_use]
pub fn cache_dir_in(base: &std::path::Path) -> PathBuf {
    base.join("cache")
}

/// `cache/<name>/` sub-directory under `base` for one feature's cache files.
#[must_use]
pub fn cache_subdir_in(base: &std::path::Path, name: &str) -> PathBuf {
    cache_dir_in(base).join(name)
}

/// `backup/` sub-directory under `base` — holds config backup archives.
#[must_use]
pub fn backup_dir_in(base: &std::path::Path) -> PathBuf {
    base.join("backup")
}

/// `log/` directory under the platform data dir.
#[must_use]
pub fn log_dir() -> Option<PathBuf> {
    data_dir().map(|d| log_dir_in(&d))
}

/// `cache/` directory under the platform data dir.
#[must_use]
pub fn cache_dir() -> Option<PathBuf> {
    data_dir().map(|d| cache_dir_in(&d))
}

/// `backup/` directory under the platform data dir.
#[must_use]
pub fn backup_dir() -> Option<PathBuf> {
    data_dir().map(|d| backup_dir_in(&d))
}

/// Ensure a directory exists, creating all missing ancestors.
///
/// Returns the directory path on success so callers can chain it.
pub fn ensure_dir(dir: &std::path::Path) -> std::io::Result<&std::path::Path> {
    std::fs::create_dir_all(dir)?;
    Ok(dir)
}

/// Move legacy flat-layout runtime files into the new sub-directory layout.
///
/// Earlier versions wrote `etlp.log`, `mpv.log` and `bangumi_subjects.json`
/// directly under `base`. This relocates them into `log/` and `cache/bangumi/`.
/// Idempotent: a file is only moved when it still exists at the old location and
/// no file already occupies the new one, so repeated calls are safe no-ops.
pub fn migrate_layout(base: &std::path::Path) {
    let log = log_dir_in(base);
    relocate(&base.join("etlp.log"), &log.join("etlp.log"));
    relocate(&base.join("mpv.log"), &log.join("mpv.log"));

    let bangumi = cache_subdir_in(base, "bangumi");
    relocate(
        &base.join("bangumi_subjects.json"),
        &bangumi.join("bangumi_subjects.json"),
    );
}

/// Move `from` to `to`, creating `to`'s parent. No-op when `from` is missing or
/// `to` already exists. Falls back to copy+remove across filesystems.
fn relocate(from: &std::path::Path, to: &std::path::Path) {
    if !from.is_file() || to.exists() {
        return;
    }
    if let Some(parent) = to.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    if std::fs::rename(from, to).is_ok() {
        return;
    }
    // Cross-device rename fails with EXDEV; fall back to copy then delete.
    if std::fs::copy(from, to).is_ok() {
        let _ = std::fs::remove_file(from);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_layout_relocates_legacy_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        std::fs::write(base.join("etlp.log"), b"app").expect("write log");
        std::fs::write(base.join("mpv.log"), b"mpv").expect("write mpv");
        std::fs::write(base.join("bangumi_subjects.json"), b"{}")
            .expect("write cache");

        migrate_layout(base);

        assert!(log_dir_in(base).join("etlp.log").is_file());
        assert!(log_dir_in(base).join("mpv.log").is_file());
        assert!(
            cache_subdir_in(base, "bangumi")
                .join("bangumi_subjects.json")
                .is_file()
        );
        // Old locations are emptied out.
        assert!(!base.join("etlp.log").exists());
        assert!(!base.join("mpv.log").exists());
    }

    #[test]
    fn migrate_layout_is_idempotent_and_preserves_new_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        // A file already in the new location must not be overwritten.
        let new_log = log_dir_in(base);
        std::fs::create_dir_all(&new_log).expect("mkdir log");
        std::fs::write(new_log.join("etlp.log"), b"new").expect("write new");
        std::fs::write(base.join("etlp.log"), b"old").expect("write old");

        migrate_layout(base);
        migrate_layout(base); // second call is a no-op

        let content =
            std::fs::read(new_log.join("etlp.log")).expect("read new");
        assert_eq!(content, b"new", "existing new-location file preserved");
        // The legacy file is left untouched when the target already exists.
        assert!(base.join("etlp.log").is_file());
    }
}

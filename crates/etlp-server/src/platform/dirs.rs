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

/// Data directory: stores `device_id`, `mpv.log`, `etlp.log`, download cache.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    base_data().map(|d| d.join(APP_NAME))
}

/// Ensure a directory exists, creating all missing ancestors.
///
/// Returns the directory path on success so callers can chain it.
pub fn ensure_dir(dir: &std::path::Path) -> std::io::Result<&std::path::Path> {
    std::fs::create_dir_all(dir)?;
    Ok(dir)
}

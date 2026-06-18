//! Cross-platform path utilities: folder opening, media launch, and
//! Windows ↔ WSL path conversion.

use std::path::Path;
use std::process::Command;

use tracing::{info, warn};

/// Open `path` in the platform's default file manager.
///
/// - macOS: `open`
/// - Windows: `explorer`
/// - Linux/WSL: `xdg-open`
pub fn open_folder(path: &str) -> std::io::Result<()> {
    info!("open_folder: {path:?}");
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(path).spawn()?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

/// Launch `file_path` with the given player binary (or shell-open it).
///
/// Falls back to shell-open if `player_path` is empty.
pub fn open_media_file(
    file_path: &str,
    player_path: &str,
) -> std::io::Result<()> {
    info!("open_media_file: {file_path:?} player={player_path:?}");
    if player_path.is_empty() {
        return open_folder(file_path);
    }
    Command::new(player_path).arg(file_path).spawn()?;
    Ok(())
}

/// Convert a WSL path (`/mnt/c/…`) to its Windows equivalent (`C:\…`).
///
/// Returns the original string unchanged if it does not start with `/mnt/`.
#[must_use]
pub fn wsl_to_windows(path: &str) -> String {
    let Some(rest) = path.strip_prefix("/mnt/") else {
        return path.to_owned();
    };
    let mut chars = rest.chars();
    let Some(drive) = chars.next() else {
        return path.to_owned();
    };
    let tail = chars.as_str().replace('/', "\\");
    format!("{}:{}", drive.to_ascii_uppercase(), tail)
}

/// Convert a Windows path (`C:\…`) to its WSL equivalent (`/mnt/c/…`).
///
/// Returns the original string unchanged if it does not look like a Windows
/// drive-letter path.
#[must_use]
pub fn windows_to_wsl(path: &str) -> String {
    if path.len() >= 2
        && path.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && path.get(1..2) == Some(":")
    {
        let drive = path.chars().next().unwrap_or('c').to_ascii_lowercase();
        let tail = path.get(2..).unwrap_or("").replace('\\', "/");
        format!("/mnt/{drive}{tail}")
    } else {
        path.to_owned()
    }
}

/// Apply the configured `[src]`/`[dst]` path-translation pairs to `path`.
///
/// Mirrors `tools.translate_path_by_config`: replaces the longest matching
/// src prefix with its dst counterpart (first match wins).
#[must_use]
pub fn translate_path(path: &str, pairs: &[(String, String)]) -> String {
    for (src, dst) in pairs {
        if path.starts_with(src.as_str()) {
            let remainder = path.get(src.len()..).unwrap_or("");
            return format!("{dst}{remainder}");
        }
    }
    path.to_owned()
}

/// Log a warning if the path does not exist.
pub fn warn_if_not_exists(path: &str) {
    if !Path::new(path).exists() {
        warn!("path does not exist: {path:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_to_windows_converts_mnt_paths() {
        assert_eq!(wsl_to_windows("/mnt/c/Users/foo"), "C:\\Users\\foo");
        assert_eq!(
            wsl_to_windows("/mnt/d/media/film.mkv"),
            "D:\\media\\film.mkv"
        );
    }

    #[test]
    fn wsl_to_windows_leaves_non_mnt_unchanged() {
        assert_eq!(wsl_to_windows("/home/user/file"), "/home/user/file");
        assert_eq!(wsl_to_windows("/mnt/"), "/mnt/");
    }

    #[test]
    fn windows_to_wsl_converts_drive_paths() {
        assert_eq!(windows_to_wsl("C:\\Users\\foo"), "/mnt/c/Users/foo");
        assert_eq!(windows_to_wsl("D:\\media"), "/mnt/d/media");
    }

    #[test]
    fn windows_to_wsl_leaves_unix_unchanged() {
        assert_eq!(windows_to_wsl("/home/user"), "/home/user");
        assert_eq!(windows_to_wsl("relative/path"), "relative/path");
    }

    #[test]
    fn translate_path_applies_first_matching_pair() {
        let pairs = vec![
            ("/mnt/disk1".to_owned(), "E:".to_owned()),
            ("/mnt/disk2".to_owned(), "F:\\media".to_owned()),
        ];
        assert_eq!(translate_path("/mnt/disk1/a.mkv", &pairs), "E:/a.mkv");
        assert_eq!(
            translate_path("/mnt/disk2/b.mkv", &pairs),
            "F:\\media/b.mkv"
        );
        assert_eq!(translate_path("/other/path", &pairs), "/other/path");
    }

    #[test]
    fn translate_path_empty_pairs_returns_original() {
        assert_eq!(translate_path("/some/path", &[]), "/some/path");
    }
}

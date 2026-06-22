//! Resolve a user-provided player path to an actually runnable executable.
//!
//! On macOS a player is usually distributed as a `.app` bundle — a *directory*,
//! not a binary. Spawning the directory fails with `Permission denied`
//! (`os error 13`), which is what a user sees after pointing the player path at,
//! say, `/Applications/IINA.app`. This module unwraps such a bundle to the
//! executable inside it. On Windows and Linux players are plain binaries, so the
//! input is returned unchanged.

use std::path::Path;

/// Map a player path to the binary that should actually be spawned.
///
/// For a macOS `.app` bundle this returns the executable inside
/// `Contents/MacOS`:
/// - IINA → `iina-cli`, the command-line front-end etlp drives (it accepts mpv
///   options via `--mpv-` flags); the bundle's GUI binary is **not** equivalent.
/// - otherwise → `Contents/MacOS/<bundle name>` when present (this is the usual
///   `CFBundleExecutable`, e.g. `VLC.app` → `VLC`), falling back to the sole
///   file in `Contents/MacOS`, then to the bundle-name guess so any resulting
///   error still names a concrete path.
///
/// Non-bundle paths — plain binaries, Windows `.exe` — are returned unchanged.
#[must_use]
pub fn resolve_player_executable(path: &str) -> String {
    // Only macOS app bundles need unwrapping; everywhere else the path is the
    // binary already. Gating on the target OS keeps a stray `foo.app` name on
    // Linux/Windows from being rewritten into a non-existent bundle path.
    let trimmed = path.trim_end_matches('/');
    if !cfg!(target_os = "macos") || !ends_with_app(trimmed) {
        return path.to_owned();
    }

    let bundle = Path::new(trimmed);
    let macos = bundle.join("Contents/MacOS");

    // IINA ships a dedicated CLI that takes mpv options as `--mpv-…` flags.
    if trimmed.to_lowercase().contains("iina") {
        let cli = macos.join("iina-cli");
        if cli.is_file() {
            return cli.to_string_lossy().into_owned();
        }
    }

    // Generic bundle: Contents/MacOS/<AppName> matches CFBundleExecutable for
    // the common players (VLC, mpv, …).
    let stem = bundle.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let guess = macos.join(stem);
    if guess.is_file() {
        return guess.to_string_lossy().into_owned();
    }

    // Otherwise fall back to the only file in Contents/MacOS, when unambiguous.
    if let Ok(entries) = std::fs::read_dir(&macos) {
        let files: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        if let [only] = files.as_slice() {
            return only.to_string_lossy().into_owned();
        }
    }

    // Nothing resolved; return the best guess so the spawn error names a real
    // candidate rather than the directory itself.
    guess.to_string_lossy().into_owned()
}

/// Case-insensitive check for a `.app` suffix.
fn ends_with_app(path: &str) -> bool {
    path.to_lowercase().ends_with(".app")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_bundle_paths_are_unchanged() {
        assert_eq!(
            resolve_player_executable("/opt/homebrew/bin/mpv"),
            "/opt/homebrew/bin/mpv"
        );
        assert_eq!(
            resolve_player_executable(
                r"C:\Program Files\PotPlayer\PotPlayerMini64.exe"
            ),
            r"C:\Program Files\PotPlayer\PotPlayerMini64.exe"
        );
    }

    #[test]
    fn ends_with_app_is_case_insensitive() {
        assert!(ends_with_app("/Applications/IINA.app"));
        assert!(ends_with_app("/Applications/IINA.APP"));
        assert!(!ends_with_app("/usr/bin/mpv"));
        assert!(!ends_with_app(".app/x"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn missing_bundle_falls_back_to_named_guess() {
        // A bundle that does not exist still resolves to a concrete candidate
        // path inside Contents/MacOS rather than the directory itself.
        let out = resolve_player_executable("/no/such/IINA.app");
        assert!(
            out.ends_with("/Contents/MacOS/iina-cli")
                || out.ends_with("/Contents/MacOS/IINA")
        );
        assert_ne!(out, "/no/such/IINA.app");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn app_suffix_untouched_off_macos() {
        // Off macOS a `.app` path is not a bundle and must pass through as-is.
        assert_eq!(resolve_player_executable("/x/IINA.app"), "/x/IINA.app");
    }
}

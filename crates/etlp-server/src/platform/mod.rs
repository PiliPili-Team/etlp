//! Platform-specific utilities for process management, file access, and
//! path translation.

pub mod device_id;
pub mod kill;
pub mod path;

pub use kill::kill_matching_processes;
pub use path::{
    open_folder, open_media_file, translate_path, warn_if_not_exists,
    windows_to_wsl, wsl_to_windows,
};

/// Activate a running window by its process ID.
///
/// On Windows this uses the `windows` crate to enumerate top-level windows
/// and call `SetForegroundWindow`. On all other platforms this is a no-op
/// (macOS apps surface themselves, and Linux desktops vary too much).
pub fn activate_window_by_pid(_pid: u32) {
    #[cfg(target_os = "windows")]
    {
        // Windows-only: enumerate HWNDs and bring the target to the foreground.
        // Requires the `windows` crate with the `Win32_UI_WindowsAndMessaging`
        // feature—left as a platform-specific TODO.
        tracing::warn!(
            "activate_window_by_pid: Windows implementation not yet wired"
        );
    }
}

//! Platform-specific utilities for process management, file access, and
//! path translation.

pub mod device_id;
pub mod dirs;
pub mod kill;
pub mod path;

pub use dirs::{ENV_RUNTIME, RuntimeMode, config_dir, data_dir};
pub use kill::kill_matching_processes;
pub use path::{
    open_folder, open_media_file, translate_path, warn_if_not_exists,
    windows_to_wsl, wsl_to_windows,
};

/// Activate a running window by its process ID.
///
/// On Windows this uses `EnumWindows` + `SetForegroundWindow` to bring the
/// target process's top-level window to the foreground. On all other platforms
/// this is a no-op (macOS apps surface themselves; Linux desktops vary).
pub fn activate_window_by_pid(_pid: u32) {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::{HWND, LPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowThreadProcessId, SW_RESTORE,
            SetForegroundWindow, ShowWindow,
        };
        use windows::core::BOOL;

        unsafe extern "system" fn find_and_activate(
            hwnd: HWND,
            lparam: LPARAM,
        ) -> BOOL {
            let target_pid = lparam.0 as u32;
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == target_pid {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
                BOOL(0) // stop enumeration
            } else {
                BOOL(1) // continue
            }
        }

        let _ = unsafe {
            EnumWindows(Some(find_and_activate), LPARAM(_pid as isize))
        };
    }
}

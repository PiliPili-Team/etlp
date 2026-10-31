//! Platform-specific utilities for process management, file access, and
//! path translation.

pub mod device_id;
pub mod dirs;
pub mod kill;
pub mod path;

pub use dirs::{
    ENV_PORTABLE, ENV_RUNTIME, RuntimeMode, backup_dir, backup_dir_in,
    cache_dir, cache_dir_in, cache_subdir_in, config_dir, data_dir,
    is_portable, log_dir, log_dir_in, migrate_layout, portable_requested,
    portable_update_dir,
};
pub use kill::kill_matching_processes;
pub use path::{
    open_folder, open_media_file, open_url, translate_path, warn_if_not_exists,
    windows_to_wsl, wsl_to_windows,
};

/// Activate a running window by its process ID.
///
/// On Windows this brings the target process's most prominent top-level window
/// to the foreground. A bare `SetForegroundWindow` is unreliable because the
/// OS focus-stealing guard silently ignores it unless the caller already owns
/// the foreground; we therefore briefly `AttachThreadInput` to the current
/// foreground thread (which satisfies the "received the last input event"
/// rule) before raising the window. A single process can own several windows,
/// so we pick the largest visible, titled one — matching the behaviour of the
/// upstream embyToLocalPlayer Win32 helper.
///
/// On all other platforms this is a no-op (macOS players surface themselves
/// via `--focus-on=open`; Linux desktops vary too much to special-case).
#[cfg_attr(target_os = "windows", allow(unsafe_code))]
pub fn activate_window_by_pid(_pid: u32) {
    #[cfg(target_os = "windows")]
    {
        tokio::spawn(async move {
            for _ in 0..30 {
                if try_activate_window_once(_pid) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        });
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn try_activate_window_once(pid: u32) -> bool {
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::System::Threading::{
        AttachThreadInput, GetCurrentThreadId,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, BringWindowToTop, EnumWindows,
        GetForegroundWindow, GetWindowRect, GetWindowTextLengthW,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, SW_RESTORE,
        SW_SHOW, SetForegroundWindow, ShowWindow,
    };
    use windows::core::BOOL;

    struct FindCtx {
        pid: u32,
        best: HWND,
        best_area: i64,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let ctx = &mut *(lparam.0 as *mut FindCtx);
            let mut win_pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut win_pid));
            if win_pid != ctx.pid
                || !IsWindowVisible(hwnd).as_bool()
                || GetWindowTextLengthW(hwnd) == 0
            {
                return BOOL(1);
            }
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return BOOL(1);
            }
            let area = i64::from(rect.right - rect.left)
                * i64::from(rect.bottom - rect.top);
            if area > ctx.best_area {
                ctx.best_area = area;
                ctx.best = hwnd;
            }
            BOOL(1)
        }
    }

    let mut ctx = FindCtx {
        pid,
        best: HWND::default(),
        best_area: 0,
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(&mut ctx as *mut FindCtx as isize),
        );
        if ctx.best.0.is_null() {
            return false;
        }
        let target = ctx.best;
        let _ = AllowSetForegroundWindow(pid);

        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let target_thread = GetWindowThreadProcessId(target, None);
        let cur_thread = GetCurrentThreadId();

        let attached_target = if target_thread != 0
            && fg_thread != 0
            && target_thread != fg_thread
        {
            AttachThreadInput(target_thread, fg_thread, true).as_bool()
        } else {
            false
        };
        let attached_cur =
            if cur_thread != 0 && fg_thread != 0 && cur_thread != fg_thread {
                AttachThreadInput(cur_thread, fg_thread, true).as_bool()
            } else {
                false
            };

        if IsIconic(target).as_bool() {
            let _ = ShowWindow(target, SW_RESTORE);
        } else {
            let _ = ShowWindow(target, SW_SHOW);
        }
        let _ = BringWindowToTop(target);
        let _ = SetForegroundWindow(target);

        if attached_target {
            let _ = AttachThreadInput(target_thread, fg_thread, false);
        }
        if attached_cur {
            let _ = AttachThreadInput(cur_thread, fg_thread, false);
        }

        GetForegroundWindow() == target
    }
}

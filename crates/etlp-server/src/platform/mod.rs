//! Platform-specific utilities for process management, file access, and
//! path translation.

pub mod device_id;
pub mod dirs;
pub mod kill;
pub mod path;

pub use dirs::{ENV_RUNTIME, RuntimeMode, config_dir, data_dir};
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
        use windows::Win32::Foundation::{HWND, LPARAM, RECT};
        use windows::Win32::System::Threading::{
            AttachThreadInput, GetCurrentThreadId,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowRect,
            GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic,
            IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindow,
        };
        use windows::core::BOOL;

        /// Accumulator threaded through `EnumWindows` to find the best match.
        struct FindCtx {
            pid: u32,
            best: HWND,
            best_area: i64,
        }

        unsafe extern "system" fn enum_proc(
            hwnd: HWND,
            lparam: LPARAM,
        ) -> BOOL {
            // edition 2024: unsafe fn body is safe by default; each unsafe
            // call needs its own unsafe {} block.
            unsafe {
                // SAFETY: `lparam` carries a `&mut FindCtx` that outlives the
                // enumeration (it lives on the caller's stack below).
                let ctx = &mut *(lparam.0 as *mut FindCtx);
                let mut win_pid = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut win_pid));
                if win_pid != ctx.pid
                    || !IsWindowVisible(hwnd).as_bool()
                    || GetWindowTextLengthW(hwnd) == 0
                {
                    return BOOL(1); // continue
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
                BOOL(1) // continue: keep looking for a larger window
            }
        }

        let mut ctx = FindCtx {
            pid: _pid,
            best: HWND::default(),
            best_area: 0,
        };
        // SAFETY: all calls below are plain Win32 FFI with valid arguments;
        // `enum_proc` only dereferences the `&mut ctx` we hand it.
        unsafe {
            let _ = EnumWindows(
                Some(enum_proc),
                LPARAM(&mut ctx as *mut FindCtx as isize),
            );
            if ctx.best.0.is_null() {
                return;
            }
            let target = ctx.best;
            let fg = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg, None);
            let cur_thread = GetCurrentThreadId();
            // Share input state with the current foreground thread so the OS
            // permits the focus change, then detach again.
            let attached =
                AttachThreadInput(cur_thread, fg_thread, true).as_bool();
            if IsIconic(target).as_bool() {
                let _ = ShowWindow(target, SW_RESTORE);
            }
            let _ = SetForegroundWindow(target);
            let _ = BringWindowToTop(target);
            if attached {
                let _ = AttachThreadInput(cur_thread, fg_thread, false);
            }
        }
    }
}

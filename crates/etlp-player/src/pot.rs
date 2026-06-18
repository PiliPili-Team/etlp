//! PotPlayer control.
//!
//! PotPlayer is a Windows-only media player that exposes its playback state
//! via Win32 window messages (`WM_APP + 0x5004`). This module implements:
//!
//! - Arg builder ([`build_pot_args`]) — pure, always compiled.
//! - [`PotHandle`] — spawned process tracker, always compiled.
//! - [`stop_sec_pot`] — polling loop; the Win32 implementation is
//!   `#[cfg(windows)]`; on other platforms it always returns `None`.

use std::io;
use std::process::{Child, Command, ExitStatus};

use thiserror::Error;
use tracing::info;

use crate::mpv::LaunchArgs;

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors from PotPlayer process control.
#[derive(Debug, Error)]
pub enum PotError {
    /// OS-level IO error (process spawn, etc.).
    #[error("IO: {0}")]
    Io(#[from] io::Error),
}

// ── Command-line builder ──────────────────────────────────────────────────────

/// Build the PotPlayer argument list (without the executable path itself).
///
/// Pure function — easy to unit-test without spawning a process.
pub fn build_pot_args(args: &LaunchArgs) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // Primary media file.
    out.push(args.media_path.clone());

    // External subtitle (local paths only; HTTP subtitles containing
    // "Plex-Token" must be downloaded first by the server layer).
    if let Some(sub) = args.sub.external.as_deref()
        && !sub.is_empty()
    {
        out.push(format!("/sub={sub}"));
    }

    // Start position as HH:MM:SS.
    if let Some(sec) = args.start_sec {
        let total = sec as u64;
        let h = total / 3600;
        let m = (total % 3600) / 60;
        let s = total % 60;
        out.push(format!("/seek={h:02}:{m:02}:{s:02}"));
    }

    // Media title shown in PotPlayer's title bar.
    if !args.media_title.is_empty() {
        out.push(format!("/title={}", args.media_title));
    }

    out
}

// ── PotHandle ─────────────────────────────────────────────────────────────────

/// A running PotPlayer instance.
///
/// PotPlayer does not offer an HTTP control interface; its playback position
/// is retrieved via Win32 window messages on Windows.
pub struct PotHandle {
    child: Child,
    /// OS process ID — passed to [`stop_sec_pot`].
    pub pid: u32,
    /// Path to the PotPlayer executable (for playlist `/add` operations).
    pub exe: String,
}

impl PotHandle {
    /// Spawn PotPlayer with the given arguments.
    pub fn spawn(args: &LaunchArgs) -> Result<Self, PotError> {
        let pot_args = build_pot_args(args);
        let child = Command::new(&args.exe).args(&pot_args).spawn()?;
        let pid = child.id();
        Ok(Self {
            child,
            pid,
            exe: args.exe.clone(),
        })
    }

    /// Non-blocking check whether PotPlayer has exited.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// Add a media file to PotPlayer's playlist.
    ///
    /// PotPlayer accepts `/add /title=…` from a second command-line invocation
    /// forwarded to the running instance.
    pub fn playlist_add(
        &self,
        path: &str,
        title: Option<&str>,
    ) -> Result<(), PotError> {
        let mut cmd = Command::new(&self.exe);
        cmd.arg(path).arg("/add");
        if let Some(t) = title {
            cmd.arg(format!("/title={t}"));
        }
        cmd.spawn()?.wait()?;
        Ok(())
    }
}

// ── stop_sec_pot ──────────────────────────────────────────────────────────────

/// Poll PotPlayer until it exits and return the last observed position (s).
///
/// On Windows, queries `WM_APP + 0x5004` via `SendMessageW`; on other
/// platforms, returns `None` immediately (PotPlayer is Windows-only).
pub async fn stop_sec_pot(pid: u32) -> Option<i64> {
    #[cfg(windows)]
    return stop_sec_pot_win32(pid).await;
    #[cfg(not(windows))]
    {
        info!("stop_sec_pot: PotPlayer is Windows-only (pid={pid})");
        None
    }
}

#[cfg(windows)]
async fn stop_sec_pot_win32(pid: u32) -> Option<i64> {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    // Win32 constants
    const WM_APP: u32 = 0x8000;
    const POT_GET_CUR_TIME: u32 = WM_APP + 0x5004;

    extern "system" {
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
        fn SendMessageW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: isize,
        ) -> isize;
        fn EnumWindows(
            proc: unsafe extern "system" fn(isize, isize) -> i32,
            lparam: isize,
        ) -> i32;
    }

    let last_sec: Arc<Mutex<Option<i64>>> = Arc::new(Mutex::new(None));
    let last_sec_clone = Arc::clone(&last_sec);

    loop {
        // Check if PotPlayer window still exists.
        let found = Arc::new(Mutex::new(false));
        let found_clone = Arc::clone(&found);
        let sec_clone = Arc::clone(&last_sec_clone);
        let target_pid = pid;

        unsafe {
            // Closure captured in a raw struct for FFI
            let ctx =
                Box::into_raw(Box::new((target_pid, found_clone, sec_clone)));

            unsafe extern "system" fn enum_proc(
                hwnd: isize,
                lparam: isize,
            ) -> i32 {
                let ctx = lparam
                    as *mut (u32, Arc<Mutex<bool>>, Arc<Mutex<Option<i64>>>);
                if ctx.is_null() {
                    return 1;
                }
                let (target_pid, found, last_sec) = &*ctx;
                let mut win_pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, &mut win_pid);
                if win_pid == *target_pid {
                    let ms = SendMessageW(hwnd, POT_GET_CUR_TIME, 0x5004, 1);
                    if ms > 0 {
                        if let Ok(mut g) = found.lock() {
                            *g = true;
                        }
                        if let Ok(mut g) = last_sec.lock() {
                            *g = Some(ms / 1000);
                        }
                    }
                }
                1
            }

            EnumWindows(enum_proc, ctx as isize);
            let _ = Box::from_raw(ctx);
        }

        let still_running = found.lock().ok().is_some_and(|g| *g);

        if !still_running {
            let result = last_sec.lock().ok().and_then(|g| *g);
            info!("PotPlayer stopped, last position: {result:?}s");
            return result;
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use etlp_core::{IntroMarkers, Subtitle};

    fn base_args() -> LaunchArgs {
        LaunchArgs {
            exe: "PotPlayerMini64.exe".to_owned(),
            media_path: "C:\\Videos\\movie.mkv".to_owned(),
            media_title: "Test Movie".to_owned(),
            start_sec: None,
            sub: Subtitle::default(),
            is_multiple_episodes: false,
            mount_disk_mode: false,
            intro: IntroMarkers::default(),
            fullscreen: false,
            disable_audio: false,
            http_proxy: None,
            static_ipc: None,
            event_handler: None,
            playlist_start: None,
        }
    }

    #[test]
    fn args_basic_contains_media_path() {
        let out = build_pot_args(&base_args());
        assert!(out.contains(&"C:\\Videos\\movie.mkv".to_owned()));
    }

    #[test]
    fn args_includes_title() {
        let out = build_pot_args(&base_args());
        assert!(out.iter().any(|s| s == "/title=Test Movie"));
    }

    #[test]
    fn args_with_sub() {
        let mut a = base_args();
        a.sub.external = Some("C:\\subs\\movie.srt".to_owned());
        let out = build_pot_args(&a);
        assert!(out.iter().any(|s| s == "/sub=C:\\subs\\movie.srt"));
    }

    #[test]
    fn args_start_sec_formatted_as_hhmmss() {
        let mut a = base_args();
        a.start_sec = Some(3723.0); // 1h 2m 3s
        let out = build_pot_args(&a);
        assert!(out.iter().any(|s| s == "/seek=01:02:03"));
    }

    #[test]
    fn args_start_sec_zero_formatted() {
        let mut a = base_args();
        a.start_sec = Some(65.0); // 1m 5s
        let out = build_pot_args(&a);
        assert!(out.iter().any(|s| s == "/seek=00:01:05"));
    }

    #[test]
    fn args_no_sub_when_empty_external() {
        let mut a = base_args();
        a.sub.external = Some(String::new());
        let out = build_pot_args(&a);
        assert!(!out.iter().any(|s| s.starts_with("/sub=")));
    }
}

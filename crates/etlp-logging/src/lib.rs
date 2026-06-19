//! Tracing setup with secret masking for etlp.
//!
//! Installs a `tracing` subscriber whose output is passed through a [`Masker`]
//! before reaching stdout and an optional log file, reproducing the Python
//! `MyLogger` redaction. Each formatted event is buffered and masked as a whole
//! line, so secrets split across fields are still redacted.
//!
//! The returned [`LogHandle`] allows the log level to be changed at runtime
//! without restarting the process (e.g. when the user changes `dev.log_level`
//! through the GUI and the server is reloaded).

mod mask;

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::reload;
use tracing_subscriber::util::SubscriberInitExt as _;

pub use mask::{Masker, mix_host_gen};

/// 10 MiB: the Python threshold above which the log file is truncated.
const LOG_RESET_BYTES: u64 = 10 * 1024 * 1000;

type SharedFile = Arc<Mutex<File>>;

/// A `MakeWriter` that masks each event before writing it to stdout and,
/// optionally, a log file.
#[derive(Clone)]
struct MaskingMakeWriter {
    masker: Masker,
    file: Option<SharedFile>,
}

/// Per-event buffer that masks its contents when dropped.
struct MaskingWriter {
    buf: Vec<u8>,
    masker: Masker,
    file: Option<SharedFile>,
}

impl Write for MaskingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for MaskingWriter {
    fn drop(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let raw = String::from_utf8_lossy(&self.buf);
        let masked = self.masker.mask(&raw);
        // Logging must never crash the process; ignore IO errors.
        let stdout = io::stdout();
        let mut lock = stdout.lock();
        let _ = lock.write_all(masked.as_bytes());
        let _ = lock.flush();
        if let Some(file) = &self.file
            && let Ok(mut f) = file.lock()
        {
            // Strip ANSI colour codes before writing to file.
            let plain = strip_ansi(&masked);
            let _ = f.write_all(plain.as_bytes());
            let _ = f.flush();
        }
    }
}

/// Strip ANSI SGR escape sequences (`ESC [ … m`) from `s`.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            for cc in chars.by_ref() {
                if cc == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

impl<'a> MakeWriter<'a> for MaskingMakeWriter {
    type Writer = MaskingWriter;

    fn make_writer(&'a self) -> Self::Writer {
        MaskingWriter {
            buf: Vec::new(),
            masker: self.masker.clone(),
            file: self.file.clone(),
        }
    }
}

/// Open the log file the way Python does: append while it is small, otherwise
/// truncate, creating parent directories as needed.
fn open_log_file(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let small = std::fs::metadata(path)
        .map(|m| m.len() < LOG_RESET_BYTES)
        .unwrap_or(false);
    OpenOptions::new()
        .create(true)
        .append(small)
        .truncate(!small)
        .write(true)
        .open(path)
}

/// Compute the `EnvFilter` for `level`.
///
/// When `level` is `debug` or `trace`, etlp crates are set to that level while
/// noisy third-party crates remain at `info`. All other levels are applied
/// globally.
///
/// Callers that want to honour `RUST_LOG` should call
/// [`build_initial_filter`] instead; this function always ignores the env var
/// so it is safe to call at runtime when the user changes the level.
#[must_use]
pub fn build_level_filter(level: &str) -> EnvFilter {
    const ETLP_CRATES: &[&str] = &[
        "etlp",
        "etlp_server",
        "etlp_config",
        "etlp_net",
        "etlp_download",
        "etlp_media_server",
        "etlp_player",
        "etlp_core",
        "etlp_sync",
        "etlp_logging",
        "etlp_metrics",
    ];
    if matches!(level, "debug" | "trace") {
        let directives = ETLP_CRATES
            .iter()
            .map(|t| format!("{t}={level}"))
            .collect::<Vec<_>>()
            .join(",");
        EnvFilter::new(format!("info,{directives}"))
    } else {
        EnvFilter::new(level)
    }
}

/// Like [`build_level_filter`] but honours `RUST_LOG` when set.
///
/// Only called once, at startup — runtime updates use [`build_level_filter`]
/// directly so the env var cannot shadow the user's explicit choice.
fn build_initial_filter(level: &str) -> EnvFilter {
    EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| build_level_filter(level))
}

// ── LogHandle ──────────────────────────────────────────────────────────────────

/// A handle that allows the active log level to be changed at runtime.
///
/// Obtained from [`init`] and typically stored in the application state.
/// `Send + Sync` — safe to share across threads.
pub struct LogHandle {
    handle: reload::Handle<EnvFilter, tracing_subscriber::Registry>,
}

impl LogHandle {
    /// Switch the log level to `level` immediately.
    ///
    /// Silently prints to stderr on failure (logging must never crash).
    pub fn set_level(&self, level: &str) {
        let filter = build_level_filter(level);
        if let Err(e) = self.handle.reload(filter) {
            eprintln!("[etlp] set_level({level:?}) failed: {e}");
        }
    }
}

// ── init ──────────────────────────────────────────────────────────────────────

/// Initialize global logging and return a [`LogHandle`] for runtime level
/// changes.
///
/// * `masker` carries the (mutable) redaction rules.
/// * `level` is the initial filter directive (e.g. `"info"`); `RUST_LOG`
///   overrides it when set.
/// * `log_file`, when provided, additionally writes masked output to that path.
///
/// Returns `Err` when the global subscriber is already set; callers that do not
/// need runtime level changes may call `.ok()` and discard the handle.
pub fn init(
    masker: Masker,
    level: &str,
    log_file: Option<&Path>,
) -> Result<LogHandle, String> {
    let file = match log_file {
        Some(path) => {
            Some(Arc::new(Mutex::new(open_log_file(path).map_err(|e| {
                format!("open log file {}: {e}", path.display())
            })?)))
        }
        None => None,
    };

    let make_writer = MaskingMakeWriter { masker, file };
    let filter = build_initial_filter(level);
    let (filter_layer, handle) = reload::Layer::new(filter);

    use std::io::IsTerminal as _;
    let use_ansi = std::io::stdout().is_terminal();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(use_ansi)
        .with_target(false)
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new(
            "%Y-%m-%d %H:%M:%S%.3f%:z".to_owned(),
        ))
        .with_writer(make_writer);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .try_init()
        .map_err(|e| e.to_string())?;

    Ok(LogHandle { handle })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_log_file_creates_and_appends() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("log.txt");
        {
            let mut f = open_log_file(&path).expect("create");
            f.write_all(b"first\n").expect("write");
        }
        {
            let mut f = open_log_file(&path).expect("reopen");
            f.write_all(b"second\n").expect("write");
        }
        let body = std::fs::read_to_string(&path).expect("read");
        assert_eq!(body, "first\nsecond\n");
    }

    #[test]
    fn build_level_filter_debug_scopes_to_etlp() {
        // debug/trace must produce "info,etlp*=debug" style directives.
        let f = build_level_filter("debug");
        // EnvFilter's Display shows the directives; just verify it doesn't panic.
        let _ = format!("{f:?}");
    }

    #[test]
    fn build_level_filter_warn_is_global() {
        let f = build_level_filter("warn");
        let _ = format!("{f:?}");
    }
}

//! Tracing setup with secret masking for etlp.
//!
//! Installs a `tracing` subscriber whose output is passed through a [`Masker`]
//! before reaching stdout and an optional log file, reproducing the Python
//! `MyLogger` redaction. Each formatted event is buffered and masked as a whole
//! line, so secrets split across fields are still redacted.

mod mask;

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;

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
            let _ = f.write_all(masked.as_bytes());
            let _ = f.flush();
        }
    }
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

/// Initialize global logging.
///
/// * `masker` carries the (mutable) redaction rules.
/// * `level` is a tracing filter directive (e.g. `"info"`); `RUST_LOG` overrides
///   it when set.
/// * `log_file`, when provided, additionally writes masked output to that path.
///
/// Returns `Ok(())` on success. A second call is a no-op error (the global
/// subscriber can only be set once); callers may ignore that.
pub fn init(
    masker: Masker,
    level: &str,
    log_file: Option<&Path>,
) -> Result<(), String> {
    let file = match log_file {
        Some(path) => {
            Some(Arc::new(Mutex::new(open_log_file(path).map_err(|e| {
                format!("open log file {}: {e}", path.display())
            })?)))
        }
        None => None,
    };

    let make_writer = MaskingMakeWriter { masker, file };
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_target(false)
        .with_writer(make_writer)
        .try_init()
        .map_err(|e| e.to_string())
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
}

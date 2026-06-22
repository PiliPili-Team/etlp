//! Size-based log-file rotation.
//!
//! [`RotatingFile`] appends masked log lines to a base file and, once that file
//! would exceed the configured size, shifts the existing files up
//! (`etlp.log` → `etlp.log.1` → … → `etlp.log.{max_files-1}`) and starts a
//! fresh base file. At most `max_files` files are kept; the oldest is dropped.
//!
//! Rotation is deliberately size-driven rather than time-driven: a long-running
//! bridge produces bursts of debug output around playback, so a byte budget
//! bounds disk use far more predictably than a daily cadence.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Default per-file size budget, in mebibytes (50 MiB).
pub const DEFAULT_MAX_SIZE_MB: u64 = 50;

/// Default number of rotated files kept on disk (the active file plus six
/// historical ones).
pub const DEFAULT_MAX_FILES: usize = 7;

/// Rotation policy: the per-file size budget and how many files to keep.
///
/// Construct with [`LogRotation::from_mb`] so the megabyte unit and the safety
/// floors (at least 1 MiB per file, at least 1 file kept) are applied once.
#[derive(Debug, Clone, Copy)]
pub struct LogRotation {
    /// Maximum size of a single log file, in bytes.
    pub max_size_bytes: u64,
    /// Maximum number of log files to keep (active file included).
    pub max_files: usize,
}

impl Default for LogRotation {
    fn default() -> Self {
        Self::from_mb(DEFAULT_MAX_SIZE_MB, DEFAULT_MAX_FILES)
    }
}

impl LogRotation {
    /// Build a policy from a size in mebibytes and a file count, clamping both
    /// to safe minimums (≥ 1 MiB per file, ≥ 1 file kept) so a misconfigured
    /// value can never make the writer rotate on every line.
    #[must_use]
    pub fn from_mb(max_size_mb: u64, max_files: usize) -> Self {
        Self {
            max_size_bytes: max_size_mb.max(1) * 1024 * 1024,
            max_files: max_files.max(1),
        }
    }
}

/// A log file that rotates when it grows past [`LogRotation::max_size_bytes`].
pub(crate) struct RotatingFile {
    /// The active (base) file path; rotated files derive their names from it.
    path: PathBuf,
    /// The current write handle. `None` only if (re)opening failed; writes are
    /// then dropped silently so logging never crashes the process.
    file: Option<File>,
    /// Bytes written to the active file so far (drives the rotation decision).
    written: u64,
    rotation: LogRotation,
}

impl RotatingFile {
    /// Open `path` for appending, creating parent directories as needed.
    ///
    /// Unlike a truncate-on-open scheme, an existing file is continued so a
    /// restart does not discard recent history; its current length seeds the
    /// rotation counter.
    pub(crate) fn open(path: &Path, rotation: LogRotation) -> io::Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let written = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            path: path.to_path_buf(),
            file: Some(file),
            written,
            rotation,
        })
    }

    /// Replace the rotation policy at runtime (e.g. when the GUI changes it).
    pub(crate) fn set_rotation(&mut self, rotation: LogRotation) {
        self.rotation = rotation;
    }

    /// Append `buf`, rotating first when the active file is already at budget.
    ///
    /// Rotation is checked before the write so each file stays within one line
    /// of the budget; an empty active file is never rotated, guaranteeing
    /// forward progress even if a single line exceeds the budget.
    pub(crate) fn write(&mut self, buf: &[u8]) -> io::Result<()> {
        if self.written >= self.rotation.max_size_bytes && self.written > 0 {
            self.rotate()?;
        }
        if let Some(file) = self.file.as_mut() {
            file.write_all(buf)?;
            file.flush()?;
            self.written += buf.len() as u64;
        }
        Ok(())
    }

    /// Shift the existing files up and start a fresh active file.
    ///
    /// The live handle is dropped before any rename so Windows (which forbids
    /// renaming an open file) is satisfied. The oldest file is overwritten by
    /// the rename chain; the active file is reopened truncated afterwards.
    fn rotate(&mut self) -> io::Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.flush()?;
        }
        // Drop the handle so the base file can be renamed on every platform.
        self.file = None;

        // Walk from the oldest slot down: …→.6, .5→.6, …, base→.1.
        for i in (1..self.rotation.max_files).rev() {
            let src = rotated_path(&self.path, i - 1);
            let dst = rotated_path(&self.path, i);
            if src.exists() {
                // rename overwrites dst, discarding the oldest file.
                let _ = std::fs::rename(&src, &dst);
            }
        }

        // With max_files == 1 the loop above is empty and the base file was not
        // renamed; truncating it here keeps exactly one (active) file.
        match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
        {
            Ok(file) => {
                self.file = Some(file);
                self.written = 0;
            }
            Err(e) => {
                // Leave the handle as None; writes are dropped until the next
                // successful open. Surface the error to the caller's logger.
                return Err(e);
            }
        }
        Ok(())
    }

    /// Empty the active file and delete every rotated file, for a clean slate.
    ///
    /// Truncates in place and rewinds so the next write lands at byte 0 rather
    /// than in a sparse hole left by the old append offset.
    pub(crate) fn clear(&mut self) -> io::Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.flush()?;
            file.set_len(0)?;
            use std::io::{Seek, SeekFrom};
            file.seek(SeekFrom::Start(0))?;
        }
        self.written = 0;
        for i in 1..self.rotation.max_files {
            let path = rotated_path(&self.path, i);
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
        }
        Ok(())
    }
}

/// The path of the `index`-th rotated file: `index == 0` is the base path,
/// higher indices append a `.N` suffix (`etlp.log.1`, `etlp.log.2`, …).
fn rotated_path(base: &Path, index: usize) -> PathBuf {
    if index == 0 {
        return base.to_path_buf();
    }
    let mut name = base.as_os_str().to_owned();
    name.push(format!(".{index}"));
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_mb_clamps_to_safe_minimums() {
        let r = LogRotation::from_mb(0, 0);
        assert_eq!(r.max_size_bytes, 1024 * 1024);
        assert_eq!(r.max_files, 1);

        let r = LogRotation::from_mb(50, 7);
        assert_eq!(r.max_size_bytes, 50 * 1024 * 1024);
        assert_eq!(r.max_files, 7);
    }

    #[test]
    fn rotated_path_appends_index_suffix() {
        let base = Path::new("/tmp/etlp.log");
        assert_eq!(rotated_path(base, 0), base.to_path_buf());
        assert_eq!(rotated_path(base, 1), PathBuf::from("/tmp/etlp.log.1"));
        assert_eq!(rotated_path(base, 6), PathBuf::from("/tmp/etlp.log.6"));
    }

    #[test]
    fn open_continues_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("etlp.log");
        {
            let mut rf = RotatingFile::open(&path, LogRotation::default())
                .expect("open");
            rf.write(b"first\n").expect("write");
        }
        {
            let mut rf = RotatingFile::open(&path, LogRotation::default())
                .expect("reopen");
            rf.write(b"second\n").expect("write");
        }
        let body = std::fs::read_to_string(&path).expect("read");
        assert_eq!(body, "first\nsecond\n");
    }

    #[test]
    fn rotation_shifts_files_and_caps_count() {
        // A 1 MiB budget with 3 files kept: each 600 KiB write forces a roll.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("etlp.log");
        let rotation = LogRotation {
            max_size_bytes: 1024 * 1024,
            max_files: 3,
        };
        let mut rf = RotatingFile::open(&path, rotation).expect("open");

        let chunk = vec![b'x'; 600 * 1024];
        // Tag each chunk so we can identify which file holds which write.
        for tag in [b'A', b'B', b'C', b'D', b'E'] {
            let mut line = vec![tag];
            line.extend_from_slice(&chunk);
            rf.write(&line).expect("write");
        }

        // Only the base file and two rotations may exist; .3 must not.
        assert!(path.exists(), "active file present");
        assert!(rotated_path(&path, 1).exists(), ".1 present");
        assert!(rotated_path(&path, 2).exists(), ".2 present");
        assert!(!rotated_path(&path, 3).exists(), ".3 must be discarded");
    }

    #[test]
    fn clear_empties_active_and_removes_rotations() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("etlp.log");
        let rotation = LogRotation {
            max_size_bytes: 1024 * 1024,
            max_files: 3,
        };
        let mut rf = RotatingFile::open(&path, rotation).expect("open");

        let chunk = vec![b'y'; 600 * 1024];
        for _ in 0..4 {
            rf.write(&chunk).expect("write");
        }
        assert!(rotated_path(&path, 1).exists(), ".1 present before clear");

        rf.clear().expect("clear");
        rf.write(b"fresh\n").expect("write after clear");

        let body = std::fs::read_to_string(&path).expect("read");
        assert_eq!(body, "fresh\n", "active file restarts at head");
        assert!(!rotated_path(&path, 1).exists(), ".1 removed");
        assert!(!rotated_path(&path, 2).exists(), ".2 removed");
    }
}

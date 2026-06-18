//! Cross-platform exclusive file lock for download task files.
//!
//! Uses the `fs4` crate for cross-platform locking. The lock file lives
//! alongside the task JSON:
//! `{task_path}.lock`. Acquiring returns `false` (non-blocking) if another
//! process already holds the lock, so the caller can read-only the state
//! without clobbering it.

use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use fs4::fs_std::FileExt;
use thiserror::Error;

/// Errors from file-lock operations.
#[derive(Debug, Error)]
pub enum LockError {
    /// OS-level IO error opening or manipulating the lock file.
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
}

/// An exclusive, non-blocking file lock for a single download task.
///
/// The lock file is `{task_path}.lock` (e.g. `video.mkv.json.lock`).
/// Dropping the struct releases the lock automatically.
pub struct TaskFileLock {
    /// Path to the task state JSON file this lock guards.
    pub task_path: PathBuf,
    /// Path to the OS-level lock file.
    pub lock_path: PathBuf,
    lock_file: Option<File>,
    /// Whether this instance currently holds the exclusive lock.
    pub has_lock: bool,
}

impl TaskFileLock {
    /// Create a new lock guard for the given task-state file path.
    ///
    /// The lock file is derived as `task_path` + `.lock`.  No lock is
    /// acquired at construction time; call [`try_lock`](Self::try_lock).
    pub fn new(task_path: impl Into<PathBuf>) -> Self {
        let task_path = task_path.into();
        let mut lock_path = task_path.clone();
        let name = lock_path
            .file_name()
            .map(|n| {
                let mut s = n.to_os_string();
                s.push(".lock");
                s
            })
            .unwrap_or_default();
        lock_path.set_file_name(name);
        Self {
            task_path,
            lock_path,
            lock_file: None,
            has_lock: false,
        }
    }

    /// Try to acquire an exclusive lock without blocking.
    ///
    /// Returns `Ok(true)` if the lock was acquired, `Ok(false)` if another
    /// process already holds it.
    pub fn try_lock(&mut self) -> Result<bool, LockError> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)?;
        let acquired = file.try_lock_exclusive().map_err(LockError::Io)?;
        if acquired {
            self.lock_file = Some(file);
            self.has_lock = true;
        }
        Ok(acquired)
    }

    /// Release the lock (safe to call multiple times).
    pub fn unlock(&mut self) {
        if let Some(f) = self.lock_file.take() {
            let _ = f.unlock();
            self.has_lock = false;
        }
    }
}

impl Drop for TaskFileLock {
    fn drop(&mut self) {
        self.unlock();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn lock_path_appends_dot_lock() {
        let tmp = TempDir::new().unwrap();
        let task = tmp.path().join("video.mkv.json");
        let lock = TaskFileLock::new(&task);
        assert_eq!(lock.lock_path, tmp.path().join("video.mkv.json.lock"));
    }

    #[test]
    fn try_lock_succeeds_when_no_contention() {
        let tmp = TempDir::new().unwrap();
        let task = tmp.path().join("task.json");
        let mut lock = TaskFileLock::new(&task);
        assert!(lock.try_lock().unwrap());
        assert!(lock.has_lock);
        assert!(lock.lock_path.exists());
    }

    #[test]
    fn second_try_lock_fails_while_first_held() {
        let tmp = TempDir::new().unwrap();
        let task = tmp.path().join("task.json");

        let mut lock1 = TaskFileLock::new(&task);
        assert!(lock1.try_lock().unwrap());

        let mut lock2 = TaskFileLock::new(&task);
        assert!(!lock2.try_lock().unwrap());
    }

    #[test]
    fn unlock_releases_so_second_can_acquire() {
        let tmp = TempDir::new().unwrap();
        let task = tmp.path().join("task.json");

        let mut lock1 = TaskFileLock::new(&task);
        assert!(lock1.try_lock().unwrap());
        lock1.unlock();

        let mut lock2 = TaskFileLock::new(&task);
        assert!(lock2.try_lock().unwrap());
    }

    #[test]
    fn drop_releases_lock() {
        let tmp = TempDir::new().unwrap();
        let task = tmp.path().join("task.json");
        {
            let mut lock = TaskFileLock::new(&task);
            lock.try_lock().unwrap();
        }
        // After drop, a new lock should succeed.
        let mut lock2 = TaskFileLock::new(&task);
        assert!(lock2.try_lock().unwrap());
    }

    #[test]
    fn has_lock_false_after_unlock() {
        let tmp = TempDir::new().unwrap();
        let task = tmp.path().join("task.json");
        let mut lock = TaskFileLock::new(&task);
        lock.try_lock().unwrap();
        lock.unlock();
        assert!(!lock.has_lock);
    }
}

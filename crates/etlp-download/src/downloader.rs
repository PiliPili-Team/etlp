//! HTTP range downloader with state persistence.
//!
//! [`Downloader`] fetches a URL in byte-range segments and writes them into a
//! local file, resuming from where it left off on restart. State is persisted
//! as a sidecar JSON file so a crashed or paused download can resume.
//!
//! Cancel and pause are driven by [`std::sync::atomic::AtomicBool`] flags so
//! they can be set from any task without needing a mutable borrow.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, info};

use crate::lock::{LockError, TaskFileLock};

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors from the downloader.
#[derive(Debug, Error)]
pub enum DownloadError {
    /// OS-level IO failure.
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    /// HTTP error (connection, timeout, status).
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    /// JSON state (de)serialization failure.
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// File-lock operation failed.
    #[error("lock: {0}")]
    Lock(#[from] LockError),
    /// Server did not return a `Content-Length` header.
    #[error("Content-Length missing from server response")]
    NoContentLength,
    /// Caller set the cancel flag mid-download.
    #[error("download cancelled")]
    Cancelled,
    /// Caller set the pause flag mid-download.
    #[error("download paused")]
    Paused,
}

// ── Serialised task state ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct TaskState {
    id: String,
    stream_url: String,
    size: Option<u64>,
    progress: f64,
    download_only: bool,
    pause: bool,
}

// ── Downloader ────────────────────────────────────────────────────────────────

/// A single HTTP range downloader instance.
///
/// Construction attempts a non-blocking file lock; if another process holds
/// the lock the instance enters read-only mode (`has_lock == false`).
pub struct Downloader {
    /// Unique identifier — used as the data file name.
    pub id: String,
    /// Source URL.
    pub url: String,
    /// Destination file path.
    pub file: PathBuf,
    /// State / lock sidecar.
    pub file_lock: TaskFileLock,
    /// Set `true` to abort the next chunk write.
    pub cancel: Arc<AtomicBool>,
    /// Set `true` to pause (returns `Paused` error from range loop).
    pub pause: Arc<AtomicBool>,
    /// Total file size in bytes (filled on first HEAD request).
    pub size: Option<u64>,
    /// Download progress as a fraction 0.0–1.0.
    pub progress: f64,
    /// When `true`, do not trigger playback after download completes.
    pub download_only: bool,
    /// Shortcut: file exists and is not locked — already fully downloaded.
    pub is_done: bool,
    client: reqwest::Client,
}

impl Downloader {
    /// Construct a downloader for `url` with the given `id`.
    ///
    /// If `save_path` is given, data is written there (e.g. `/dev/null` for
    /// prefetch-only). Otherwise data goes to `{cache_path}/{id}`.
    pub fn new(
        url: String,
        id: String,
        client: reqwest::Client,
        cache_path: &Path,
        save_path: Option<&Path>,
    ) -> Result<Self, DownloadError> {
        let file = save_path
            .map(PathBuf::from)
            .unwrap_or_else(|| cache_path.join(&id));
        let task_file = cache_path.join(format!("{id}.json"));
        let file_lock = TaskFileLock::new(&task_file);

        let is_done = file.exists() && !file_lock.lock_path.exists();

        let mut dl = Self {
            id,
            url,
            file,
            file_lock,
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
            size: None,
            progress: 0.0,
            download_only: false,
            is_done,
            client,
        };

        if is_done {
            dl.restore_state();
        } else {
            let locked = dl.file_lock.try_lock()?;
            if locked {
                info!("dl: lock success: {}", dl.id);
                if !dl.restore_state() {
                    dl.save_state()?;
                }
            } else {
                info!(
                    "dl: lock failed: already locked by another process. {}",
                    dl.id
                );
                dl.restore_state();
            }
        }

        Ok(dl)
    }

    /// Persist current progress/state to the task JSON sidecar.
    pub fn save_state(&self) -> Result<(), DownloadError> {
        let state = TaskState {
            id: self.id.clone(),
            stream_url: self.url.clone(),
            size: self.size,
            progress: self.progress,
            download_only: self.download_only,
            pause: self.pause.load(Ordering::Relaxed),
        };
        let json = serde_json::to_string_pretty(&state)?;
        std::fs::write(self.file_lock.task_path.as_path(), json)?;
        Ok(())
    }

    /// Load state from the task JSON sidecar.
    ///
    /// Returns `true` if state was found and applied, `false` otherwise.
    pub fn restore_state(&mut self) -> bool {
        let Ok(content) =
            std::fs::read_to_string(self.file_lock.task_path.as_path())
        else {
            return false;
        };
        let Ok(state): Result<TaskState, _> = serde_json::from_str(&content)
        else {
            return false;
        };
        self.progress = state.progress;
        self.download_only = state.download_only;
        self.size = state.size;
        if state.pause {
            self.pause.store(true, Ordering::Relaxed);
        }
        true
    }

    /// Mark the download as fully complete and release the lock.
    pub fn mark_done(&mut self) {
        self.is_done = true;
        self.file_lock.unlock();
        let _ = std::fs::remove_file(&self.file_lock.lock_path);
    }

    /// Fetch and cache the file size via a HEAD request.
    pub async fn get_size(&mut self) -> Result<u64, DownloadError> {
        if let Some(sz) = self.size {
            return Ok(sz);
        }
        let resp = self.client.head(&self.url).send().await?;
        let len = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or(DownloadError::NoContentLength)?;
        self.size = Some(len);
        Ok(len)
    }

    /// Download the byte range `start..=end` into the file.
    ///
    /// Returns the byte offset reached. On success this equals `end`; a lower
    /// value indicates a partial write (retry from that offset).
    ///
    /// When `start == 0`, the existing file (if any) is deleted and a new
    /// sparse file of the full size is pre-allocated.
    ///
    /// `speed_bps`: bytes-per-second throttle, 0 = unlimited.
    pub async fn range_download(
        &mut self,
        start: u64,
        end: u64,
        speed_bps: u64,
    ) -> Result<u64, DownloadError> {
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};

        let size = self.get_size().await?;

        if start == 0 && self.file.exists() {
            std::fs::remove_file(&self.file)?;
        }
        if !self.file.exists() {
            // Pre-allocate a sparse file (Linux/macOS: truly sparse; Windows:
            // zero-filled). Ignored for save_path = /dev/null / NUL.
            if let Some(parent) = self.file.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)?;
            }
            let f = std::fs::File::create(&self.file)?;
            f.set_len(size)?;
        }

        let range_header = format!("bytes={start}-{end}");
        let mut resp = self
            .client
            .get(&self.url)
            .header(reqwest::header::RANGE, &range_header)
            .send()
            .await?;

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&self.file)
            .await?;
        file.seek(std::io::SeekFrom::Start(start)).await?;

        let mut pos = start;
        let sleep_per_chunk = if speed_bps > 0 {
            // Throttle: sleep so throughput ≈ speed_bps.
            let chunk_sz = 1024u64 * 256; // target chunk size for rate calc
            Some(std::time::Duration::from_micros(
                chunk_sz * 1_000_000 / speed_bps,
            ))
        } else {
            None
        };

        while let Some(chunk) = resp.chunk().await? {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(DownloadError::Cancelled);
            }
            if self.pause.load(Ordering::Relaxed) {
                return Err(DownloadError::Paused);
            }
            file.write_all(&chunk).await?;
            pos += chunk.len() as u64;
            if pos > end {
                break;
            }
            if let Some(d) = sleep_per_chunk {
                tokio::time::sleep(d).await;
            }
        }
        file.flush().await?;
        Ok(pos.min(end + 1).saturating_sub(1).min(end))
    }

    /// Download the fraction `start_frac..end_frac` of the file (0.0–1.0).
    ///
    /// Retries with exponential back-off on partial failures.
    /// `update`: when `true`, advances `self.progress` to `end_frac` on success.
    pub async fn percent_download(
        &mut self,
        start_frac: f64,
        end_frac: f64,
        speed_bps: u64,
        update: bool,
    ) -> Result<(), DownloadError> {
        let size = self.get_size().await?;
        info!(
            "dl: start {}% end {}% {}",
            (start_frac * 100.0) as u32,
            (end_frac * 100.0) as u32,
            self.id,
        );
        let byte_start = (size as f64 * start_frac) as u64;
        let byte_end = ((size as f64 * end_frac) as u64).saturating_sub(1);
        if byte_end < byte_start {
            return Ok(());
        }

        let mut pos = byte_start;
        let mut retry_sleep = std::time::Duration::from_secs(1);

        loop {
            match self.range_download(pos, byte_end, speed_bps).await {
                Ok(reached) if reached >= byte_end => break,
                Ok(reached) => {
                    error!(
                        "dl: partial download, retrying from {reached}. {}",
                        self.id
                    );
                    pos = reached;
                    tokio::time::sleep(retry_sleep).await;
                    retry_sleep = retry_sleep
                        .saturating_add(retry_sleep)
                        .min(std::time::Duration::from_secs(60));
                }
                Err(DownloadError::Cancelled | DownloadError::Paused) => {
                    return Err(DownloadError::Paused);
                }
                Err(e) => {
                    error!("dl: error {e}, retrying. {}", self.id);
                    tokio::time::sleep(retry_sleep).await;
                    retry_sleep = retry_sleep
                        .saturating_add(retry_sleep)
                        .min(std::time::Duration::from_secs(60));
                }
            }
        }

        if update {
            self.progress = end_frac;
        }
        Ok(())
    }

    /// Download the first 1 % and last 1 % of the file.
    ///
    /// This gives players enough metadata and index to start and seek without
    /// waiting for the full download.
    pub async fn download_first_last(&mut self) -> Result<(), DownloadError> {
        self.percent_download(0.0, 0.01, 0, false).await?;
        self.percent_download(0.99, 1.0, 0, false).await?;
        self.progress = 0.01;
        Ok(())
    }

    /// Cancel the download, release the lock, and delete all sidecar files.
    ///
    /// Returns `true` if any files were deleted.
    pub async fn cancel_download(&mut self) -> bool {
        self.cancel.store(true, Ordering::Relaxed);
        self.file_lock.unlock();
        let mut deleted = false;
        let paths = [
            self.file_lock.lock_path.clone(),
            self.file_lock.task_path.clone(),
            self.file.clone(),
        ];
        for path in &paths {
            if path.exists() {
                match std::fs::remove_file(path) {
                    Ok(()) => deleted = true,
                    Err(e) => info!(
                        "dl: file is locked, cannot delete: {e}. {}",
                        self.id
                    ),
                }
            }
        }
        if deleted {
            info!("dl: delete done {}", self.id);
        }
        deleted
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_client() -> reqwest::Client {
        reqwest::Client::new()
    }

    #[test]
    fn new_without_existing_file_acquires_lock() {
        let tmp = TempDir::new().unwrap();
        let dl = Downloader::new(
            "http://example.com/video.mkv".into(),
            "video.mkv".into(),
            make_client(),
            tmp.path(),
            None,
        )
        .unwrap();
        assert!(dl.file_lock.has_lock);
        assert!(!dl.is_done);
    }

    #[test]
    fn second_instance_same_id_fails_lock() {
        let tmp = TempDir::new().unwrap();
        let dl1 = Downloader::new(
            "http://example.com/video.mkv".into(),
            "video.mkv".into(),
            make_client(),
            tmp.path(),
            None,
        )
        .unwrap();
        assert!(dl1.file_lock.has_lock);

        let dl2 = Downloader::new(
            "http://example.com/video.mkv".into(),
            "video.mkv".into(),
            make_client(),
            tmp.path(),
            None,
        )
        .unwrap();
        assert!(!dl2.file_lock.has_lock);
    }

    #[test]
    fn restore_state_reads_saved_json() {
        let tmp = TempDir::new().unwrap();
        let mut dl = Downloader::new(
            "http://example.com/v.mkv".into(),
            "v.mkv".into(),
            make_client(),
            tmp.path(),
            None,
        )
        .unwrap();
        dl.size = Some(1_000_000);
        dl.progress = 0.5;
        dl.save_state().unwrap();

        drop(dl); // releases lock

        let dl2 = Downloader::new(
            "http://example.com/v.mkv".into(),
            "v.mkv".into(),
            make_client(),
            tmp.path(),
            None,
        )
        .unwrap();
        assert!((dl2.progress - 0.5).abs() < f64::EPSILON);
        assert_eq!(dl2.size, Some(1_000_000));
    }

    #[tokio::test]
    async fn get_size_reads_content_length() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/v.mkv"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("Content-Length", "4096000"),
            )
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let url = format!("{}/v.mkv", server.uri());
        let mut dl = Downloader::new(
            url,
            "v.mkv".into(),
            make_client(),
            tmp.path(),
            None,
        )
        .unwrap();
        assert_eq!(dl.get_size().await.unwrap(), 4_096_000);
        assert_eq!(dl.size, Some(4_096_000));
    }

    #[tokio::test]
    async fn percent_download_writes_to_file() {
        let data = vec![0xABu8; 1024];
        let server = MockServer::start().await;

        Mock::given(method("HEAD"))
            .and(path("/tiny.bin"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("Content-Length", "1024"),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/tiny.bin"))
            .respond_with(ResponseTemplate::new(206).set_body_bytes(data))
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let url = format!("{}/tiny.bin", server.uri());
        let mut dl = Downloader::new(
            url,
            "tiny.bin".into(),
            make_client(),
            tmp.path(),
            None,
        )
        .unwrap();
        dl.percent_download(0.0, 0.1, 0, true).await.unwrap();
        assert!(dl.file.exists());
        assert!((dl.progress - 0.1).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn cancel_download_removes_files() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/c.mkv"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("Content-Length", "8192"),
            )
            .mount(&server)
            .await;

        let tmp = TempDir::new().unwrap();
        let url = format!("{}/c.mkv", server.uri());
        let mut dl = Downloader::new(
            url,
            "c.mkv".into(),
            make_client(),
            tmp.path(),
            None,
        )
        .unwrap();
        // Save state so task_file exists.
        dl.save_state().unwrap();
        dl.cancel_download().await;
        // Task JSON should be gone.
        assert!(!dl.file_lock.task_path.exists());
    }
}

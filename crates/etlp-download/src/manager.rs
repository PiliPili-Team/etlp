//! Download orchestration: concurrent HTTP downloads with per-domain throttling.
//!
//! [`DownloadManager`] wraps a pool of [`Downloader`] tasks behind two
//! semaphores — a global concurrency cap and a per-hostname cap — so that
//! many parallel downloads neither saturate the link nor hammer a single
//! origin server.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{Mutex, Semaphore};
use tracing::{error, info};
use url::Url;

use crate::downloader::{DownloadError, Downloader};

/// Default global concurrency limit.
pub const DEFAULT_MAX_CONCURRENT: usize = 3;
/// Default per-hostname concurrency limit.
pub const DEFAULT_MAX_PER_DOMAIN: usize = 2;

/// Where to source media for playback after [`DownloadManager::download_play`].
#[derive(Debug, Clone)]
pub enum PlaySource {
    /// Data is cached locally; play from this path.
    LocalFile(PathBuf),
    /// No local cache yet; stream directly from the origin URL.
    StreamUrl,
}

struct TaskEntry {
    dl: Arc<Mutex<Downloader>>,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
}

/// Manages concurrent HTTP downloads with global and per-domain rate limiting.
///
/// All mutable state lives behind `Arc<Mutex<…>>` so the manager can be
/// cheaply cloned and shared across tokio tasks.
pub struct DownloadManager {
    cache_path: PathBuf,
    tasks: Arc<Mutex<HashMap<String, TaskEntry>>>,
    speed_limit: u64,
    semaphore: Arc<Semaphore>,
    max_per_domain: usize,
    domain_semaphores: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    client: reqwest::Client,
}

impl DownloadManager {
    /// Create a new manager.
    ///
    /// - `speed_limit`: bytes per second (0 = unlimited).
    /// - `max_concurrent`: global download cap.
    /// - `max_per_domain`: per-hostname cap.
    pub fn new(
        cache_path: PathBuf,
        speed_limit: u64,
        max_concurrent: usize,
        max_per_domain: usize,
        client: reqwest::Client,
    ) -> Self {
        Self {
            cache_path,
            tasks: Arc::new(Mutex::new(HashMap::new())),
            speed_limit,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_per_domain,
            domain_semaphores: Arc::new(Mutex::new(HashMap::new())),
            client,
        }
    }

    async fn domain_semaphore(&self, url: &str) -> Arc<Semaphore> {
        let domain = Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_default();
        let mut map = self.domain_semaphores.lock().await;
        map.entry(domain)
            .or_insert_with(|| Arc::new(Semaphore::new(self.max_per_domain)))
            .clone()
    }

    /// Start a download and return the playback source.
    ///
    /// - Already done → [`PlaySource::LocalFile`] immediately.
    /// - Another process holds the lock → [`PlaySource::StreamUrl`].
    /// - Otherwise: pre-warm the first+last 1 % of the file so playback can
    ///   start, then continue downloading the middle in a background task.
    pub async fn download_play(
        &self,
        url: String,
        id: String,
    ) -> Result<PlaySource, DownloadError> {
        // Fast path: task already tracked.
        {
            let tasks = self.tasks.lock().await;
            if let Some(entry) = tasks.get(&id) {
                // Non-blocking lock attempt: if the bg task holds the mutex we
                // just stream rather than block.
                if let Ok(dl) = entry.dl.try_lock()
                    && dl.is_done
                {
                    return Ok(PlaySource::LocalFile(dl.file.clone()));
                }
                return Ok(PlaySource::StreamUrl);
            }
        }

        let mut dl = Downloader::new(
            url.clone(),
            id.clone(),
            self.client.clone(),
            &self.cache_path,
            None,
        )?;
        dl.download_only = false;

        if dl.is_done {
            return Ok(PlaySource::LocalFile(dl.file.clone()));
        }
        if !dl.file_lock.has_lock {
            return Ok(PlaySource::StreamUrl);
        }

        // Pre-warm first 1 % and last 1 % so the player can start immediately.
        dl.download_first_last().await?;

        let local_file = dl.file.clone();
        let cancel = dl.cancel.clone();
        let pause = dl.pause.clone();
        let dl_arc = Arc::new(Mutex::new(dl));

        {
            let mut tasks = self.tasks.lock().await;
            tasks.insert(
                id.clone(),
                TaskEntry {
                    dl: dl_arc.clone(),
                    cancel,
                    pause,
                },
            );
        }

        // Spawn background task to fill in the middle (1 %–99 %).
        let global_sem = self.semaphore.clone();
        let domain_sem = self.domain_semaphore(&url).await;
        let speed = self.speed_limit;
        let dl_bg = dl_arc.clone();
        tokio::spawn(async move {
            let Ok(_gp) = global_sem.acquire_owned().await else {
                return;
            };
            let Ok(_dp) = domain_sem.acquire_owned().await else {
                return;
            };
            let mut dl = dl_bg.lock().await;
            if let Err(e) = dl.percent_download(0.01, 0.99, speed, true).await {
                error!("dl bg: {e}. {}", dl.id);
                return;
            }
            dl.mark_done();
            info!("dl: complete {}", dl.id);
        });

        Ok(PlaySource::LocalFile(local_file))
    }

    /// Download a file without triggering playback.
    ///
    /// The full download runs in a background task. Returns immediately.
    pub async fn download_only(
        &self,
        url: String,
        id: String,
    ) -> Result<(), DownloadError> {
        {
            let tasks = self.tasks.lock().await;
            if tasks.contains_key(&id) {
                return Ok(());
            }
        }

        let mut dl = Downloader::new(
            url.clone(),
            id.clone(),
            self.client.clone(),
            &self.cache_path,
            None,
        )?;
        dl.download_only = true;

        if dl.is_done || !dl.file_lock.has_lock {
            return Ok(());
        }

        let cancel = dl.cancel.clone();
        let pause = dl.pause.clone();
        let dl_arc = Arc::new(Mutex::new(dl));

        {
            let mut tasks = self.tasks.lock().await;
            tasks.insert(
                id.clone(),
                TaskEntry {
                    dl: dl_arc.clone(),
                    cancel,
                    pause,
                },
            );
        }

        let global_sem = self.semaphore.clone();
        let domain_sem = self.domain_semaphore(&url).await;
        let speed = self.speed_limit;
        tokio::spawn(async move {
            let Ok(_gp) = global_sem.acquire_owned().await else {
                return;
            };
            let Ok(_dp) = domain_sem.acquire_owned().await else {
                return;
            };
            let mut dl = dl_arc.lock().await;
            if let Err(e) = dl.percent_download(0.0, 1.0, speed, true).await {
                error!("dl only bg: {e}. {}", dl.id);
                return;
            }
            dl.mark_done();
            info!("dl: complete {}", dl.id);
        });

        Ok(())
    }

    /// Cancel and delete a download by id.
    ///
    /// Sets the cancel flag and waits for the background task to exit before
    /// deleting associated files. Returns `true` if the task existed.
    pub async fn delete(&self, id: &str) -> bool {
        let (task_arc, cancel) = {
            let mut tasks = self.tasks.lock().await;
            let Some(entry) = tasks.remove(id) else {
                return false;
            };
            (entry.dl, entry.cancel)
        };
        // Signal the background task to stop on the next chunk.
        cancel.store(true, Ordering::Relaxed);
        // Acquire the mutex (blocks until the bg task notices the cancel and returns).
        let mut dl = task_arc.lock().await;
        dl.cancel_download().await
    }

    /// Set the cancel flag for a running download without waiting for it to stop.
    ///
    /// The background task checks the flag at each chunk boundary and exits
    /// on the next iteration. Returns `true` when the task was found.
    pub async fn cancel_only(&self, id: &str) -> bool {
        let tasks = self.tasks.lock().await;
        if let Some(entry) = tasks.get(id) {
            entry.cancel.store(true, Ordering::Relaxed);
            info!("dl: cancel signalled for {id}");
            true
        } else {
            false
        }
    }

    /// Toggle the pause flag for an active download.
    pub async fn resume_or_pause(&self, id: &str) {
        let pause = {
            let tasks = self.tasks.lock().await;
            tasks.get(id).map(|e| e.pause.clone())
        };
        if let Some(flag) = pause {
            let was = flag.load(Ordering::Relaxed);
            flag.store(!was, Ordering::Relaxed);
            info!("dl: {} {id}", if was { "resumed" } else { "paused" });
        }
    }

    /// Spawn a background loop that persists task state every `interval_secs`.
    pub fn start_update_db_loop(&self, interval_secs: u64) {
        let tasks = self.tasks.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                let snapshot: Vec<Arc<Mutex<Downloader>>> = {
                    let guard = tasks.lock().await;
                    guard.values().map(|e| e.dl.clone()).collect()
                };
                for dl_arc in snapshot {
                    let dl = dl_arc.lock().await;
                    if !dl.is_done
                        && let Err(e) = dl.save_state()
                    {
                        error!("dl: save_state: {e}. {}", dl.id);
                    }
                }
            }
        });
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_manager(tmp: &TempDir) -> DownloadManager {
        DownloadManager::new(
            tmp.path().to_path_buf(),
            0,
            DEFAULT_MAX_CONCURRENT,
            DEFAULT_MAX_PER_DOMAIN,
            reqwest::Client::new(),
        )
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);
        assert!(!mgr.delete("nonexistent").await);
    }

    #[tokio::test]
    async fn domain_semaphore_same_host_returns_same_arc() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);
        let s1 = mgr.domain_semaphore("http://example.com/a").await;
        let s2 = mgr.domain_semaphore("http://example.com/b").await;
        assert!(Arc::ptr_eq(&s1, &s2));
    }

    #[tokio::test]
    async fn domain_semaphore_different_host_returns_different_arc() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);
        let s1 = mgr.domain_semaphore("http://a.com/x").await;
        let s2 = mgr.domain_semaphore("http://b.com/x").await;
        assert!(!Arc::ptr_eq(&s1, &s2));
    }

    #[tokio::test]
    async fn resume_or_pause_noop_for_missing_id() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);
        // Should not panic
        mgr.resume_or_pause("no-such-id").await;
    }

    #[tokio::test]
    async fn download_only_noop_for_duplicate_id() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(&tmp);

        // Insert sentinel entry directly.
        let dl = Downloader::new(
            "http://example.com/v.mkv".into(),
            "dup".into(),
            reqwest::Client::new(),
            tmp.path(),
            None,
        )
        .unwrap();
        let cancel = dl.cancel.clone();
        let pause = dl.pause.clone();
        let dl_arc = Arc::new(Mutex::new(dl));
        mgr.tasks.lock().await.insert(
            "dup".into(),
            TaskEntry {
                dl: dl_arc,
                cancel,
                pause,
            },
        );

        // Second call for the same id should return Ok without creating a task.
        let result = mgr
            .download_only("http://example.com/v.mkv".into(), "dup".into())
            .await;
        assert!(result.is_ok());
        assert_eq!(mgr.tasks.lock().await.len(), 1);
    }
}

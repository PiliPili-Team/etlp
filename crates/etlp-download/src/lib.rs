//! HTTP range downloader, task lock, download manager and Emby prefetch for etlp.

pub mod downloader;
pub mod lock;
pub mod manager;
pub mod prefetch;

pub use downloader::{DownloadError, Downloader};
pub use lock::{LockError, TaskFileLock};
pub use manager::{
    DEFAULT_MAX_CONCURRENT, DEFAULT_MAX_PER_DOMAIN, DownloadManager, PlaySource,
};
pub use prefetch::prefetch_resume_tv;

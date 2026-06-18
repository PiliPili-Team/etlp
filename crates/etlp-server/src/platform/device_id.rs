//! Persistent device identifier for the etlp client.
//!
//! On first run, a UUID v4 is generated and written to the platform cache
//! directory. Subsequent runs read the same UUID so the Emby/Jellyfin server
//! recognises the client across sessions.

use uuid::Uuid;

/// Load the persisted device ID from `path`, or generate and save a new one.
///
/// Creates parent directories if they do not exist. Returns an in-memory UUID
/// (not persisted) only when the file cannot be read or written.
pub fn load_or_create_at(path: &std::path::Path) -> String {
    if let Ok(id) = std::fs::read_to_string(path) {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }

    let id = Uuid::new_v4().to_string();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, &id);
    id
}

/// Load the persisted device ID from the platform data directory.
///
/// Falls back to a new in-memory UUID when the data directory cannot be
/// determined or the file cannot be written.
#[must_use]
pub fn load_or_create() -> String {
    match crate::platform::dirs::data_dir() {
        Some(dir) => load_or_create_at(&dir.join("device_id")),
        None => Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn load_or_create_returns_valid_uuid() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("device_id");
        let id = load_or_create_at(&path);
        assert!(!id.is_empty());
        assert_eq!(id.len(), 36, "unexpected format: {id}");
    }

    #[test]
    fn load_or_create_is_stable() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("device_id");
        let a = load_or_create_at(&path);
        let b = load_or_create_at(&path);
        assert_eq!(a, b, "device_id must not change between calls");
    }
}

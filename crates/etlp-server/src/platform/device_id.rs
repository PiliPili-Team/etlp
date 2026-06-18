//! Persistent device identifier for the etlp client.
//!
//! On first run, a UUID v4 is generated and written to the platform cache
//! directory. Subsequent runs read the same UUID so the Emby/Jellyfin server
//! recognises the client across sessions.

use std::path::PathBuf;

use uuid::Uuid;

fn device_id_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("etlp").join("device_id"))
}

/// Load the persisted device ID, or generate and save a new one.
///
/// Returns a new random UUID if the cache directory cannot be determined or
/// the file cannot be written; the UUID will not be persisted in that case.
#[must_use]
pub fn load_or_create() -> String {
    let Some(path) = device_id_path() else {
        return Uuid::new_v4().to_string();
    };

    if let Ok(id) = std::fs::read_to_string(&path) {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }

    let id = Uuid::new_v4().to_string();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, &id);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_or_create_returns_valid_uuid() {
        let id = load_or_create();
        assert!(!id.is_empty());
        // UUID v4 canonical form is 36 chars (8-4-4-4-12 with dashes).
        assert_eq!(id.len(), 36, "unexpected format: {id}");
    }

    #[test]
    fn load_or_create_is_stable() {
        let a = load_or_create();
        let b = load_or_create();
        assert_eq!(a, b, "device_id must not change between calls");
    }
}

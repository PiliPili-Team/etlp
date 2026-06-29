//! Core domain types and trait contracts shared across the etlp workspace.
//!
//! This crate carries no business logic and performs no IO. It defines the
//! vocabulary (media-server kinds, player kinds, subtitle/intro descriptors)
//! and—incrementally—the trait boundaries that upper layers depend on, so they
//! can be mocked in tests.

mod agent;
mod error;
mod media;
mod playback;
mod player;

pub use agent::{UA_DOWNLOAD, UA_ETLP, UA_PREFETCH};
pub use error::{CoreError, Result};
pub use media::{IntroMarkers, Subtitle};
pub use playback::{
    PLAYBACK_COMPLETION_PERCENT, PLAYBACK_COMPLETION_RATIO, PlaybackData,
};
pub use player::PlayerKind;

use serde::{Deserialize, Serialize};

/// The media server a playback request originated from.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Server {
    #[default]
    Emby,
    Jellyfin,
    Plex,
}

impl Server {
    /// Lowercase identifier matching the Python `data['server']` values.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Server::Emby => "emby",
            Server::Jellyfin => "jellyfin",
            Server::Plex => "plex",
        }
    }

    /// Whether this server speaks the Emby/Jellyfin API dialect (as opposed to
    /// Plex). Emby and Jellyfin share most endpoints.
    #[must_use]
    pub fn is_emby_like(self) -> bool {
        matches!(self, Server::Emby | Server::Jellyfin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_as_str_matches_python_values() {
        assert_eq!(Server::Emby.as_str(), "emby");
        assert_eq!(Server::Jellyfin.as_str(), "jellyfin");
        assert_eq!(Server::Plex.as_str(), "plex");
    }

    #[test]
    fn server_emby_like_classification() {
        assert!(Server::Emby.is_emby_like());
        assert!(Server::Jellyfin.is_emby_like());
        assert!(!Server::Plex.is_emby_like());
    }

    #[test]
    fn server_serde_roundtrip_is_lowercase() {
        let json = serde_json::to_string(&Server::Emby).unwrap_or_default();
        assert_eq!(json, "\"emby\"");
    }
}

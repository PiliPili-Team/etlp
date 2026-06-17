//! Core domain types and trait contracts shared across the etlp workspace.
//!
//! This crate carries no business logic and performs no IO. It defines the
//! vocabulary (media-server kinds, player kinds, playback data) and the trait
//! boundaries that upper layers depend on, so they can be mocked in tests.

/// The media server a playback request originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Server {
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
}

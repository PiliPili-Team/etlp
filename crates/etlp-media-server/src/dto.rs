//! JSON DTOs for the Emby/Jellyfin API responses.
//!
//! Only the fields etlp actually consumes are modeled; everything else is
//! ignored by serde. Field names use the server's PascalCase via `rename`.

use serde::{Deserialize, Serialize};

/// One media stream (video / audio / subtitle) inside a media source.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct MediaStream {
    #[serde(rename = "Type", default)]
    pub stream_type: String,
    #[serde(rename = "Index", default)]
    pub index: Option<i64>,
    #[serde(rename = "IsExternal", default)]
    pub is_external: bool,
    #[serde(rename = "Title", default)]
    pub title: Option<String>,
    #[serde(rename = "DisplayTitle", default)]
    pub display_title: String,
    #[serde(rename = "Codec", default)]
    pub codec: Option<String>,
    #[serde(rename = "DeliveryUrl", default)]
    pub delivery_url: Option<String>,
    #[serde(rename = "Path", default)]
    pub path: Option<String>,
}

impl MediaStream {
    /// Whether this stream is a subtitle track.
    #[must_use]
    pub fn is_subtitle(&self) -> bool {
        self.stream_type == "Subtitle"
    }

    /// The lowercased `"{title},{display_title}"` key used for priority
    /// matching, mirroring the Python `_get_sub_order_by_ini`.
    #[must_use]
    pub fn priority_key(&self) -> String {
        let title = self.title.as_deref().unwrap_or("");
        format!("{title},{}", self.display_title).to_lowercase()
    }
}

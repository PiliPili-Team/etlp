//! Normalized media description types.

use serde::{Deserialize, Serialize};

/// The resolved subtitle choice for a media item.
///
/// Either an external subtitle URL/path to load, or the index of an embedded
/// (internal) subtitle track to select, or neither.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subtitle {
    /// External subtitle URL (network stream) or local path, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<String>,

    /// Embedded subtitle track index to select in the player (mpv `--sid`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inner_index: Option<i64>,
}

impl Subtitle {
    /// Whether an external subtitle stream/file is present.
    #[must_use]
    pub fn has_external(&self) -> bool {
        self.external.as_ref().is_some_and(|s| !s.is_empty())
    }
}

/// Intro (opening) chapter markers, in whole seconds.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize,
)]
pub struct IntroMarkers {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
}

impl IntroMarkers {
    /// Both markers are present, so an opening chapter can be built.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.start.is_some() && self.end.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_has_external_ignores_empty() {
        assert!(!Subtitle::default().has_external());
        let empty = Subtitle {
            external: Some(String::new()),
            inner_index: None,
        };
        assert!(!empty.has_external());
        let present = Subtitle {
            external: Some("http://x/s.srt".into()),
            inner_index: None,
        };
        assert!(present.has_external());
    }

    #[test]
    fn intro_completeness() {
        assert!(!IntroMarkers::default().is_complete());
        assert!(
            IntroMarkers {
                start: Some(0),
                end: Some(90)
            }
            .is_complete()
        );
        assert!(
            !IntroMarkers {
                start: Some(0),
                end: None
            }
            .is_complete()
        );
    }
}

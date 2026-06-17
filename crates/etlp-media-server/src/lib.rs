//! Emby/Jellyfin/Plex clients and playback payload parsing for etlp.
//!
//! This crate ports `emby_api*.py`, `plex_api.py` and the large
//! `data_parser.py`. It is built up incrementally (see `docs/TODO.md`):
//! version selection and comparison land first, followed by the JSON DTOs,
//! the thin API clients, and finally the payload/episode-list parsing.

pub mod dto;
pub mod emby;
pub mod meta;
pub mod path_map;
pub mod received;
pub mod subtitle;
pub mod version;

pub use meta::{emby_title, intro_markers};
pub use path_map::translate_path;

pub use dto::{
    Chapter, Item, ItemList, MediaSource, MediaStream, PlaybackInfo,
};
pub use emby::EmbyClient;
pub use received::ReceivedData;
pub use subtitle::{SubtitleSelection, subtitle_checker};
pub use version::{match_version_range, select_version_index};

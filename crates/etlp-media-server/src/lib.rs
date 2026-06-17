//! Emby/Jellyfin/Plex clients and playback payload parsing for etlp.
//!
//! This crate ports `emby_api*.py`, `plex_api.py` and the large
//! `data_parser.py`. It is built up incrementally (see `docs/TODO.md`):
//! version selection and comparison land first, followed by the JSON DTOs,
//! the thin API clients, and finally the payload/episode-list parsing.

pub mod dto;
pub mod emby;
pub mod episodes;
pub mod list_episodes;
pub mod meta;
pub mod parse;
pub mod path_map;
pub mod playlist;
pub mod plex;
pub mod prefer;
pub mod received;
pub mod resolve;
pub mod stream_url;
pub mod subtitle;
pub mod version;
pub mod version_filter;

pub use meta::{emby_title, intro_markers};
pub use path_map::translate_path;
pub use resolve::{ResolveInput, StreamResolution, resolve_stream};
pub use stream_url::{StreamUrlInput, build_stream_url};

pub use dto::{
    Chapter, Item, ItemList, MediaSource, MediaStream, PlaybackInfo,
};
pub use emby::EmbyClient;
pub use episodes::{
    EpisodeContext, TitleIntroMaps, build_title_intro_maps, parse_episode_item,
};
pub use list_episodes::{ListContext, assemble_episodes};
pub use parse::{EmbyParseConfig, ParseError, parse_received_data_emby};
pub use playlist::{
    PlaylistWindow, build_window, locate_current, playlist_window,
};
pub use plex::{PlexParseConfig, PlexReceivedData, parse_received_data_plex};
pub use prefer::version_prefer_for_playlist;
pub use received::ReceivedData;
pub use subtitle::{SubtitleSelection, subtitle_checker};
pub use version::{match_version_range, select_version_index};
pub use version_filter::{
    VersionFilterInput, episode_key, filter_by_raw_name, shortest_episode,
    version_filter,
};

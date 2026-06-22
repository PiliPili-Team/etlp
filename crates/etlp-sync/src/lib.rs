//! Third-party watch-history sync for etlp: Trakt.tv and Bangumi (bgm.tv).
//!
//! # Trakt
//! [`trakt::TraktApi`] implements the OAuth 2.0 Device Flow and exposes
//! methods to add items to a user's watch history. [`trakt::sync_history`]
//! is the high-level orchestrator that skips already-watched items.
//!
//! # Bangumi
//! [`bangumi::BangumiApi`] authenticates with a personal Bearer token and
//! provides methods to search subjects, query episodes, and mark progress.
//! [`bangumi::sync_episode_by_bangumi_id`] is the entry point for the Emby
//! integration: it accepts a Bangumi subject ID from `ProviderIds.Bangumi`
//! and marks the given episode sort numbers as watched.

pub mod bangumi;
pub mod bangumi_map;
mod curl;
pub mod error;
pub mod trakt;

pub use bangumi::{
    BangumiApi, CollectionState, SubjectCache, sync_episode_by_bangumi_id,
    sync_episodes,
};
pub use bangumi_map::{
    MapError, MapProvider, SubjectMapping, match_mapping, parse_mapping,
    parse_mappings,
};
pub use error::{Result, SyncError};
pub use trakt::{
    DeviceCodeResponse, ScrobbleAction, TraktApi, TraktEpisode,
    TraktHistoryItem, TraktIds, TraktItemKind, TraktToken, sync_history,
    trakt_authorize_url,
};

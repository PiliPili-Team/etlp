//! Playlist window construction and current-episode location.
//!
//! Ports the index logic at the top of `players.playlist_add_mpv` and fixes the
//! multi-version crash it suffered from: the Python code located the current
//! episode purely by file basename (`_basename == data['basename']`), leaving
//! `cur_index = None` when a multi-version source's basename did not match, and
//! then computed `cur_index - limit`, raising
//! `TypeError: unsupported operand type(s) for -: 'NoneType' and 'int'`.
//!
//! Here the current episode is located by `item_id` first (robust across
//! multi-version sources whose basenames differ), with a basename fallback, and
//! the surrounding window is sliced saturatingly so a missing or out-of-range
//! current index can never underflow or panic.

use etlp_core::PlaybackData;

/// Locate the index of the currently playing episode within `episodes`.
///
/// Matches on `item_id` first, falling back to `basename` for entries that lack
/// an item id. Returns `None` when nothing matches, letting the caller disable
/// the playlist gracefully instead of crashing.
#[must_use]
pub fn locate_current(
    episodes: &[PlaybackData],
    current: &PlaybackData,
) -> Option<usize> {
    if !current.item_id.is_empty() {
        if let Some(idx) =
            episodes.iter().position(|e| e.item_id == current.item_id)
        {
            return Some(idx);
        }
    }
    if current.basename.is_empty() {
        return None;
    }
    episodes.iter().position(|e| e.basename == current.basename)
}

/// A bounded slice of the playlist around the current episode.
///
/// `pre` is the up-to-`limit` episodes immediately before the current one
/// (exclusive of it) and `suf` is the current episode plus the up-to-`limit`
/// episodes after it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistWindow<'a> {
    /// The clamped index of the current episode within `episodes`.
    pub current_index: usize,
    /// Episodes before the current one (closest last).
    pub pre: &'a [PlaybackData],
    /// The current episode followed by the episodes after it.
    pub suf: &'a [PlaybackData],
}

/// Slice a `±limit` window around `current`, saturating at both ends.
#[must_use]
pub fn playlist_window(
    episodes: &[PlaybackData],
    current: usize,
    limit: usize,
) -> PlaylistWindow<'_> {
    let last = episodes.len().saturating_sub(1);
    let current = current.min(last);
    let pre_start = current.saturating_sub(limit);
    let suf_end = current.saturating_add(limit).min(episodes.len());
    PlaylistWindow {
        current_index: current,
        pre: episodes.get(pre_start..current).unwrap_or(&[]),
        suf: episodes.get(current..suf_end).unwrap_or(&[]),
    }
}

/// Locate the current episode and build its window, or `None` when the current
/// episode is not present.
#[must_use]
pub fn build_window<'a>(
    episodes: &'a [PlaybackData],
    current: &PlaybackData,
    limit: usize,
) -> Option<PlaylistWindow<'a>> {
    let index = locate_current(episodes, current)?;
    Some(playlist_window(episodes, index, limit))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(item_id: &str, basename: &str) -> PlaybackData {
        PlaybackData {
            item_id: item_id.to_owned(),
            basename: basename.to_owned(),
            ..PlaybackData::default()
        }
    }

    #[test]
    fn locates_by_item_id_when_basename_differs() {
        // The multi-version case: episodes carry version-specific basenames
        // that do not equal the chosen source's basename, but item ids match.
        let episodes = vec![
            ep("100", "S01E01.VCB.mkv"),
            ep("101", "S01E02.VCB.mkv"),
            ep("102", "S01E03.VCB.mkv"),
        ];
        let current = ep("101", "S01E02.Baha.mkv");
        assert_eq!(locate_current(&episodes, &current), Some(1));
    }

    #[test]
    fn falls_back_to_basename_without_item_id() {
        let episodes = vec![ep("", "a.mkv"), ep("", "b.mkv")];
        let current = ep("", "b.mkv");
        assert_eq!(locate_current(&episodes, &current), Some(1));
    }

    #[test]
    fn returns_none_when_absent() {
        let episodes = vec![ep("1", "a.mkv"), ep("2", "b.mkv")];
        let current = ep("9", "z.mkv");
        assert_eq!(locate_current(&episodes, &current), None);
    }

    #[test]
    fn window_saturates_at_start() {
        let episodes: Vec<PlaybackData> =
            (0..6).map(|i| ep(&i.to_string(), "x")).collect();
        let w = playlist_window(&episodes, 1, 3);
        // pre_start = max(0, 1-3) = 0; pre = [0,1); suf = [1, min(6,4)) = [1,4).
        assert_eq!(w.current_index, 1);
        assert_eq!(w.pre.len(), 1);
        assert_eq!(w.suf.len(), 3);
    }

    #[test]
    fn window_saturates_at_end() {
        let episodes: Vec<PlaybackData> =
            (0..6).map(|i| ep(&i.to_string(), "x")).collect();
        let w = playlist_window(&episodes, 5, 3);
        // pre = [2,5) len 3; suf = [5, min(6,8)) = [5,6) len 1.
        assert_eq!(w.current_index, 5);
        assert_eq!(w.pre.len(), 3);
        assert_eq!(w.suf.len(), 1);
    }

    #[test]
    fn out_of_range_current_is_clamped_not_panicked() {
        let episodes = vec![ep("1", "a"), ep("2", "b")];
        let w = playlist_window(&episodes, 99, 5);
        assert_eq!(w.current_index, 1);
        assert_eq!(w.suf.len(), 1);
    }

    #[test]
    fn build_window_returns_none_for_missing_current() {
        let episodes = vec![ep("1", "a.mkv")];
        let current = ep("9", "z.mkv");
        assert!(build_window(&episodes, &current, 5).is_none());
    }

    #[test]
    fn empty_episodes_never_panic() {
        let episodes: Vec<PlaybackData> = Vec::new();
        let w = playlist_window(&episodes, 0, 5);
        assert_eq!(w.current_index, 0);
        assert!(w.pre.is_empty());
        assert!(w.suf.is_empty());
    }
}

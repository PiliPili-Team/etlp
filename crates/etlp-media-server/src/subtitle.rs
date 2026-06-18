//! Subtitle track selection.
//!
//! Given all media streams and the subtitle index requested by the player
//! (`>0` selected, `-1` unspecified, `-3` playlist external-only probe),
//! decide the embedded subtitle index to force (`sub_inner_idx`, mpv `--sid`)
//! and/or the external subtitle stream to load.

use etlp_config::matching::match_order;

use crate::dto::MediaStream;

/// The resolved subtitle selection.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubtitleSelection {
    /// The (possibly updated) subtitle index.
    pub sub_index: i64,
    /// 1-based index of the embedded subtitle to force, or 0 for none.
    pub sub_inner_idx: i64,
    /// The chosen external subtitle stream, if any.
    pub selected: Option<MediaStream>,
}

/// Priority order (1-based, 0 = no match) of a subtitle by config rules.
fn order_of(stream: &MediaStream, priority: &[String]) -> usize {
    match_order(&stream.priority_key(), priority)
}

/// Pick the global index of the highest-priority stream among `candidates`
/// (each is `(global_index, stream)`), or `None` if none match a rule.
fn best_by_priority(
    candidates: &[(usize, &MediaStream)],
    priority: &[String],
) -> Option<usize> {
    candidates
        .iter()
        .filter_map(|(gi, s)| {
            let order = order_of(s, priority);
            if order == 0 { None } else { Some((order, *gi)) }
        })
        .min_by_key(|(order, _)| *order)
        .map(|(_, gi)| gi)
}

/// Select subtitles. `sub_index` is the player-provided index, `priority` the
/// `dev.subtitle_priority` rule list.
#[must_use]
pub fn subtitle_checker(
    streams: &[MediaStream],
    sub_index: i64,
    mount_disk_mode: bool,
    priority: &[String],
) -> SubtitleSelection {
    let mut result = SubtitleSelection {
        sub_index,
        sub_inner_idx: 0,
        selected: None,
    };

    let subs: Vec<(usize, &MediaStream)> = streams
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_subtitle())
        .collect();
    let inner: Vec<(usize, &MediaStream)> = subs
        .iter()
        .filter(|(_, s)| !s.is_external)
        .copied()
        .collect();
    let external: Vec<(usize, &MediaStream)> = subs
        .iter()
        .filter(|(_, s)| s.is_external)
        .copied()
        .collect();

    // -1 with no external but some embedded: pick embedded by priority.
    if sub_index == -1 && external.is_empty() && !inner.is_empty() {
        if let Some(global) = best_by_priority(&inner, priority) {
            let pos = inner.iter().position(|(gi, _)| *gi == global);
            if let Some(pos) = pos {
                result.sub_inner_idx = (pos + 1) as i64;
            }
        }
    }

    // Explicit selection.
    if sub_index > 0
        && let Ok(idx) = usize::try_from(sub_index)
        && let Some(stream) = streams.get(idx)
    {
        if stream.is_external {
            result.selected = if mount_disk_mode {
                None
            } else {
                Some(stream.clone())
            };
        } else if let Some(pos) = inner.iter().position(|(gi, _)| *gi == idx) {
            result.sub_inner_idx = (pos + 1) as i64;
        }
    }

    // -1 / -3 without disk mode: pick an external subtitle by priority.
    if (sub_index == -1 || sub_index == -3) && !mount_disk_mode {
        if let Some(global) = best_by_priority(&external, priority) {
            if let Some(stream) = streams.get(global) {
                result.selected = Some(stream.clone());
                result.sub_index = stream.index.unwrap_or(sub_index);
            }
        } else {
            result.selected = None;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(
        index: i64,
        external: bool,
        title: &str,
        display: &str,
    ) -> MediaStream {
        MediaStream {
            stream_type: "Subtitle".to_owned(),
            index: Some(index),
            is_external: external,
            title: Some(title.to_owned()),
            display_title: display.to_owned(),
            ..MediaStream::default()
        }
    }

    fn video() -> MediaStream {
        MediaStream {
            stream_type: "Video".to_owned(),
            index: Some(0),
            ..MediaStream::default()
        }
    }

    fn priority() -> Vec<String> {
        ["中英", "简", "chi"]
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn unspecified_picks_embedded_by_priority() {
        // streams: video(0), sub eng(1), sub chs "简"(2)
        let streams = vec![
            video(),
            sub(1, false, "", "English"),
            sub(2, false, "", "简体中文"),
        ];
        let r = subtitle_checker(&streams, -1, false, &priority());
        // Embedded "简" matches; it is the 2nd embedded subtitle -> sid 2.
        assert_eq!(r.sub_inner_idx, 2);
        assert!(r.selected.is_none());
    }

    #[test]
    fn unspecified_picks_external_by_priority() {
        let streams = vec![
            video(),
            sub(1, true, "", "English"),
            sub(2, true, "", "中英双语"),
        ];
        let r = subtitle_checker(&streams, -1, false, &priority());
        // External present -> embedded branch skipped; external "中英" chosen.
        assert_eq!(r.sub_inner_idx, 0);
        assert_eq!(r.selected.map(|s| s.index), Some(Some(2)));
        assert_eq!(r.sub_index, 2);
    }

    #[test]
    fn explicit_embedded_selection_sets_sid() {
        let streams = vec![
            video(),
            sub(1, false, "", "English"),
            sub(2, false, "", "简体中文"),
        ];
        let r = subtitle_checker(&streams, 2, false, &priority());
        // Stream at global index 2 is the 2nd embedded subtitle -> sid 2.
        assert_eq!(r.sub_inner_idx, 2);
    }

    #[test]
    fn explicit_external_dropped_in_disk_mode() {
        let streams = vec![video(), sub(1, true, "", "中英")];
        let r = subtitle_checker(&streams, 1, true, &priority());
        assert!(r.selected.is_none());
        assert_eq!(r.sub_inner_idx, 0);
    }

    #[test]
    fn no_matching_priority_selects_nothing() {
        let streams = vec![video(), sub(1, true, "", "Deutsch")];
        let r = subtitle_checker(&streams, -1, false, &priority());
        assert!(r.selected.is_none());
    }
}

//! Title and intro-marker derivation, ported from `tools.py`.
//!
//! * [`emby_title`] reproduces `main_ep_to_title`: a pretty display title for
//!   movies and episodes.
//! * [`intro_markers`] reproduces `main_ep_intro_time`: extract opening
//!   start/end seconds from chapter markers, with the same filtering.

use etlp_core::IntroMarkers;

use crate::dto::Item;

/// Tick values are 100ns; `10^7` ticks per second.
const TICKS_PER_SEC: i64 = 10_000_000;

/// Build the Emby display title.
///
/// * Movie (no `SeasonId`): `Name` or `Name (Year)`.
/// * Episode missing index numbers: `Series - Name`.
/// * Episode: `Series S{p}:E{i} - Name`, with an `-E{end}` range when present.
#[must_use]
pub fn emby_title(item: &Item) -> String {
    let name = item.name.as_deref().unwrap_or("");
    // Movie: no season id.
    if item.season_id.is_none() {
        return match item.production_year {
            Some(year) => format!("{name} ({year})"),
            None => name.to_owned(),
        };
    }
    let series = item.series_name.as_deref().unwrap_or("");
    let (Some(parent), Some(index)) =
        (item.parent_index_number, item.index_number)
    else {
        return format!("{series} - {name}");
    };
    match item.index_number_end {
        Some(end) => format!("{series} S{parent}:E{index}-{end} - {name}"),
        None => format!("{series} S{parent}:E{index} - {name}"),
    }
}

/// Whether a tick value should be ignored: it ends in nine zeros (a "round"
/// placeholder), matching the Python `str(ticks).endswith('000000000')`.
fn is_placeholder_ticks(ticks: i64) -> bool {
    ticks.to_string().ends_with("000000000")
}

/// Extract intro (opening) markers from the item's chapters.
///
/// Considers the first five chapters, keeps those with a marker type that are
/// neither placeholder ticks nor a zero-position generic `Chapter`, and only
/// yields markers when one or two such chapters remain (matching Python).
#[must_use]
pub fn intro_markers(item: &Item) -> IntroMarkers {
    let mut result = IntroMarkers::default();
    let candidates: Vec<&crate::dto::Chapter> = item
        .chapters
        .iter()
        .take(5)
        .filter(|c| {
            let Some(marker) = c.marker_type.as_deref() else {
                return false;
            };
            if is_placeholder_ticks(c.start_position_ticks) {
                return false;
            }
            !(c.start_position_ticks == 0 && marker == "Chapter")
        })
        .collect();

    if candidates.is_empty() || candidates.len() > 2 {
        return result;
    }
    for c in candidates {
        match c.marker_type.as_deref() {
            Some("IntroStart") => {
                result.start = Some(c.start_position_ticks / TICKS_PER_SEC);
            }
            Some("IntroEnd") => {
                result.end = Some(c.start_position_ticks / TICKS_PER_SEC);
            }
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::Chapter;

    fn chapter(marker: &str, ticks: i64) -> Chapter {
        Chapter {
            marker_type: Some(marker.to_owned()),
            start_position_ticks: ticks,
        }
    }

    #[test]
    fn movie_title_with_and_without_year() {
        let mut m = Item {
            name: Some("Inception".into()),
            ..Item::default()
        };
        assert_eq!(emby_title(&m), "Inception");
        m.production_year = Some(2010);
        assert_eq!(emby_title(&m), "Inception (2010)");
    }

    #[test]
    fn episode_title_with_index() {
        let item = Item {
            name: Some("The Beginning".into()),
            season_id: Some("s1".into()),
            series_name: Some("Show".into()),
            parent_index_number: Some(1),
            index_number: Some(3),
            ..Item::default()
        };
        assert_eq!(emby_title(&item), "Show S1:E3 - The Beginning");
    }

    #[test]
    fn episode_title_with_range_and_missing_index() {
        let mut item = Item {
            name: Some("Two-parter".into()),
            season_id: Some("s1".into()),
            series_name: Some("Show".into()),
            parent_index_number: Some(1),
            index_number: Some(3),
            index_number_end: Some(4),
            ..Item::default()
        };
        assert_eq!(emby_title(&item), "Show S1:E3-4 - Two-parter");
        item.index_number = None;
        assert_eq!(emby_title(&item), "Show - Two-parter");
    }

    #[test]
    fn intro_markers_extracted_from_two_chapters() {
        let item = Item {
            chapters: vec![
                chapter("IntroStart", 0),
                chapter("IntroEnd", 900_000_000),
            ],
            ..Item::default()
        };
        let intro = intro_markers(&item);
        assert_eq!(intro.start, Some(0));
        assert_eq!(intro.end, Some(90));
    }

    #[test]
    fn placeholder_ticks_and_too_many_chapters_ignored() {
        // Nine-zero ticks are filtered out.
        let item = Item {
            chapters: vec![chapter("IntroEnd", 9_000_000_000)],
            ..Item::default()
        };
        assert_eq!(intro_markers(&item), IntroMarkers::default());

        // More than two valid chapters -> nothing.
        let busy = Item {
            chapters: vec![
                chapter("IntroStart", 100_000_001),
                chapter("IntroEnd", 900_000_001),
                chapter("Other", 950_000_001),
            ],
            ..Item::default()
        };
        assert_eq!(intro_markers(&busy), IntroMarkers::default());
    }
}

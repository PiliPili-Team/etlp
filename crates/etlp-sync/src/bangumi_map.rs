//! User-defined provider→Bangumi subject mappings.
//!
//! Auto-resolution (provider id / title search) sometimes maps to the wrong
//! Bangumi subject — most often because a service groups several Bangumi
//! "subjects" (each cour is its own subject on bgm.tv) under one TMDB/TVDB
//! season. A mapping lets the user pin a season to an exact subject and shift
//! the episode numbering with a per-season offset.
//!
//! # DSL
//!
//! One mapping per line, `LHS -> RHS`:
//!
//! ```text
//! tmdb:10000|type:tv|S4 -> bgm:20000|E+59        # all of S4, offset +59
//! tmdb:10001|type:movie -> bgm:20001              # a movie, no offset
//! tvdb:30000|S2 -> bgm:21000|E+12                # type inferred from S2
//! imdb:tt1234567 -> bgm:22000                     # type inferred as movie
//! tmdb:79481|type:tv|S5E106~S5E157 -> bgm:449353  # closed episode range
//! tmdb:79481|type:tv|S5E158++ -> bgm:562145       # open-ended range
//! ```
//!
//! ## Season + episode range syntax
//!
//! The season token may carry an episode range to split a single TMDB season
//! across several Bangumi subjects:
//!
//! | Token           | Meaning                                   |
//! |-----------------|-------------------------------------------|
//! | `S5`            | Season 5, all episodes (original form)    |
//! | `S5E106~S5E157` | Season 5, episodes 106–157 (inclusive)    |
//! | `S5E158++`      | Season 5, episode 158 and beyond          |
//!
//! The episode numbers in the range refer to the Emby/TMDB episode index
//! (`IndexNumber`). Bangumi subjects that continue a previous collection's
//! numbering (i.e. not starting from 1) map 1-to-1 when the episode numbers
//! happen to match; use `E±N` on the RHS when they differ.
//!
//! * `provider` is one of `tmdb`, `tvdb` (numeric id) or `imdb` (`tt…` id).
//! * `type` is optional: when omitted it is `tv` if a season is present, else
//!   `movie`.
//! * The RHS offset `E±N` is added to the local episode number to get the
//!   Bangumi episode (`E+59` turns local ep 5 into subject ep 64). It is only
//!   valid for TV mappings and defaults to `0`.

/// External metadata provider a mapping keys on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapProvider {
    Tmdb,
    Imdb,
    Tvdb,
}

impl MapProvider {
    fn as_str(self) -> &'static str {
        match self {
            MapProvider::Tmdb => "tmdb",
            MapProvider::Imdb => "imdb",
            MapProvider::Tvdb => "tvdb",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "tmdb" => Some(MapProvider::Tmdb),
            "imdb" => Some(MapProvider::Imdb),
            "tvdb" => Some(MapProvider::Tvdb),
            _ => None,
        }
    }
}

/// The episode-range constraint on a TV season mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpRange {
    /// No restriction — the mapping covers every episode of the season.
    All,
    /// Closed interval `[start, end]` (both inclusive).
    Closed(u32, u32),
    /// Open-ended interval `[start, ∞)`.
    From(u32),
}

impl EpRange {
    /// Whether `episode` falls inside this range.
    ///
    /// A `None` episode (index unknown) is considered to match every range so
    /// the caller can decide how to handle it (the mapping is not ruled out).
    #[must_use]
    pub fn contains(&self, episode: Option<u32>) -> bool {
        match (self, episode) {
            (_, None) => true,
            (EpRange::All, _) => true,
            (EpRange::Closed(start, end), Some(ep)) => {
                ep >= *start && ep <= *end
            }
            (EpRange::From(start), Some(ep)) => ep >= *start,
        }
    }

    fn as_suffix(&self, season: u32) -> String {
        match self {
            EpRange::All => String::new(),
            EpRange::Closed(s, e) => {
                format!("|S{season}E{s}~S{season}E{e}")
            }
            EpRange::From(s) => format!("|S{season}E{s}++"),
        }
    }
}

/// A parsed provider→Bangumi subject mapping entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectMapping {
    /// The provider whose id the LHS keys on.
    pub provider: MapProvider,
    /// The provider id (numeric string for tmdb/tvdb, `tt…` for imdb).
    pub provider_id: String,
    /// Whether the mapping targets a movie (no season / offset).
    pub is_movie: bool,
    /// Season number for TV mappings; `None` for movies.
    pub season: Option<u32>,
    /// Episode range within the season; `EpRange::All` when unspecified.
    pub ep_range: EpRange,
    /// The target Bangumi subject id.
    pub subject_id: u64,
    /// Episode offset added to the local episode number (TV only).
    pub ep_offset: i64,
}

/// Why a mapping line failed to parse. Each variant maps to a stable i18n key
/// via [`MapError::code`] so the GUI can localise the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// The line is empty or whitespace only.
    Empty,
    /// The overall `LHS -> RHS` shape is malformed.
    Format,
    /// The provider keyword is not one of tmdb/imdb/tvdb.
    Provider,
    /// The provider id is empty or has the wrong shape for the provider.
    ProviderId,
    /// An unknown `type:` value (must be `tv` or `movie`).
    Type,
    /// The season token (`S<n>`) is malformed.
    Season,
    /// The episode range token is malformed or inverted.
    EpRange,
    /// The Bangumi subject id is missing or not a positive integer.
    Subject,
    /// The episode offset token (`E±<n>`) is malformed.
    Offset,
    /// A movie mapping carried a season or episode offset.
    MovieWithSeason,
}

impl MapError {
    /// Stable identifier used as the i18n key for the error message.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            MapError::Empty => "map_err_empty",
            MapError::Format => "map_err_format",
            MapError::Provider => "map_err_provider",
            MapError::ProviderId => "map_err_provider_id",
            MapError::Type => "map_err_type",
            MapError::Season => "map_err_season",
            MapError::EpRange => "map_err_ep_range",
            MapError::Subject => "map_err_subject",
            MapError::Offset => "map_err_offset",
            MapError::MovieWithSeason => "map_err_movie_season",
        }
    }
}

/// Parse the LHS `provider:id` plus optional `type:`/`S<n>[E<m>~S<n>E<k>]` parts.
struct Lhs {
    provider: MapProvider,
    provider_id: String,
    explicit_type: Option<bool>, // Some(true)=movie, Some(false)=tv
    season: Option<u32>,
    ep_range: EpRange,
}

/// Split `s` at the first non-ASCII-digit character, returning the leading
/// digit run and the remainder.
fn split_leading_digits(s: &str) -> (&str, &str) {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s.split_at(end)
}

/// Parse a season token after the leading `S`/`s` has been stripped.
///
/// Recognised forms:
/// * `"5"` → season 5, [`EpRange::All`]
/// * `"5E106~S5E157"` or `"5E106~157"` → season 5, [`EpRange::Closed`]
/// * `"5E158++"` → season 5, [`EpRange::From`]
fn parse_season_suffix(s: &str) -> Result<(u32, EpRange), MapError> {
    let (season_str, rest) = split_leading_digits(s);
    if season_str.is_empty() {
        return Err(MapError::Season);
    }
    let season: u32 = season_str.parse().map_err(|_| MapError::Season)?;
    if season == 0 {
        return Err(MapError::Season);
    }

    if rest.is_empty() {
        return Ok((season, EpRange::All));
    }

    // Must be followed by E/e introducing an episode range.
    let rest = rest
        .strip_prefix('E')
        .or_else(|| rest.strip_prefix('e'))
        .ok_or(MapError::Season)?;

    let (ep_start_str, rest) = split_leading_digits(rest);
    if ep_start_str.is_empty() {
        return Err(MapError::EpRange);
    }
    let ep_start: u32 = ep_start_str.parse().map_err(|_| MapError::EpRange)?;

    // Open-ended: `E158++`
    if rest == "++" {
        return Ok((season, EpRange::From(ep_start)));
    }

    // Closed range: `E106~S5E157` or `E106~157`
    let rest = rest.strip_prefix('~').ok_or(MapError::EpRange)?;

    // Skip optional `S<n>E` prefix in the upper bound.
    let rest = if let Some(r) =
        rest.strip_prefix('S').or_else(|| rest.strip_prefix('s'))
    {
        let (_, r2) = split_leading_digits(r);
        r2.strip_prefix('E')
            .or_else(|| r2.strip_prefix('e'))
            .ok_or(MapError::EpRange)?
    } else {
        rest
    };

    let (ep_end_str, _) = split_leading_digits(rest);
    if ep_end_str.is_empty() {
        return Err(MapError::EpRange);
    }
    let ep_end: u32 = ep_end_str.parse().map_err(|_| MapError::EpRange)?;
    if ep_end < ep_start {
        return Err(MapError::EpRange);
    }
    Ok((season, EpRange::Closed(ep_start, ep_end)))
}

fn parse_lhs(lhs: &str) -> Result<Lhs, MapError> {
    let mut parts = lhs.split('|').map(str::trim);
    let head = parts.next().ok_or(MapError::Format)?;
    let (prov, id) = head.split_once(':').ok_or(MapError::Format)?;
    let provider = MapProvider::parse(prov).ok_or(MapError::Provider)?;
    let provider_id = id.trim().to_owned();
    if !valid_provider_id(provider, &provider_id) {
        return Err(MapError::ProviderId);
    }

    let mut explicit_type: Option<bool> = None;
    let mut season: Option<u32> = None;
    let mut ep_range = EpRange::All;

    for part in parts {
        if part.is_empty() {
            continue;
        }
        if let Some(val) = part.strip_prefix("type:") {
            explicit_type =
                Some(match val.trim().to_ascii_lowercase().as_str() {
                    "movie" => true,
                    "tv" => false,
                    _ => return Err(MapError::Type),
                });
        } else if let Some(rest) =
            part.strip_prefix('S').or_else(|| part.strip_prefix('s'))
        {
            let (s, r) = parse_season_suffix(rest)?;
            season = Some(s);
            ep_range = r;
        } else {
            return Err(MapError::Format);
        }
    }

    Ok(Lhs {
        provider,
        provider_id,
        explicit_type,
        season,
        ep_range,
    })
}

/// Whether `id` is well-formed for `provider` (numeric, or `tt…` for imdb).
fn valid_provider_id(provider: MapProvider, id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    match provider {
        MapProvider::Imdb => {
            id.starts_with("tt")
                && id[2..].chars().all(|c| c.is_ascii_digit())
                && id.len() > 2
        }
        MapProvider::Tmdb | MapProvider::Tvdb => {
            id.chars().all(|c| c.is_ascii_digit())
        }
    }
}

/// Parse the RHS `bgm:<subject>` plus optional `E±<offset>`.
fn parse_rhs(rhs: &str) -> Result<(u64, i64), MapError> {
    let mut parts = rhs.split('|').map(str::trim);
    let head = parts.next().ok_or(MapError::Format)?;
    let (tag, sub) = head.split_once(':').ok_or(MapError::Format)?;
    if !tag.trim().eq_ignore_ascii_case("bgm") {
        return Err(MapError::Format);
    }
    let subject_id: u64 = sub.trim().parse().map_err(|_| MapError::Subject)?;
    if subject_id == 0 {
        return Err(MapError::Subject);
    }

    let mut ep_offset: i64 = 0;
    for part in parts {
        if part.is_empty() {
            continue;
        }
        let body = part
            .strip_prefix('E')
            .or_else(|| part.strip_prefix('e'))
            .ok_or(MapError::Offset)?;
        // Accept `+59`, `-3`, or a bare `59`.
        ep_offset = body.trim().parse().map_err(|_| MapError::Offset)?;
    }
    Ok((subject_id, ep_offset))
}

/// Parse a single mapping line into a [`SubjectMapping`].
///
/// # Errors
/// Returns a [`MapError`] describing the first problem found.
pub fn parse_mapping(line: &str) -> Result<SubjectMapping, MapError> {
    let line = line.trim();
    if line.is_empty() {
        return Err(MapError::Empty);
    }
    let (lhs, rhs) = line.split_once("->").ok_or(MapError::Format)?;
    let lhs = parse_lhs(lhs.trim())?;
    let (subject_id, ep_offset) = parse_rhs(rhs.trim())?;

    // Infer the kind: explicit `type:` wins, else a season implies TV.
    let is_movie = match lhs.explicit_type {
        Some(movie) => movie,
        None => lhs.season.is_none(),
    };

    if is_movie && (lhs.season.is_some() || ep_offset != 0) {
        return Err(MapError::MovieWithSeason);
    }

    // A TV mapping always carries a season; default to 1 when unspecified.
    let season = if is_movie {
        None
    } else {
        Some(lhs.season.unwrap_or(1))
    };

    // Episode ranges are only meaningful on TV mappings.
    let ep_range = if is_movie { EpRange::All } else { lhs.ep_range };

    Ok(SubjectMapping {
        provider: lhs.provider,
        provider_id: lhs.provider_id,
        is_movie,
        season,
        ep_range,
        subject_id,
        ep_offset,
    })
}

/// Strip a leading `@GroupName@` prefix from a stored mapping string.
///
/// The GUI may attach a group label to mapping entries for organisational
/// purposes. The label is transparent to the server — this function strips it
/// so `parse_mapping` and `parse_mappings` work correctly regardless of
/// whether a group prefix is present.
pub fn strip_group_prefix(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix('@')
        && let Some(pos) = rest.find('@')
    {
        return &rest[pos + 1..];
    }
    s
}

/// Parse every line, silently dropping the invalid ones. Used on the server
/// side where a bad entry must not abort the whole sync; the GUI validates
/// strictly before an entry is ever stored.
///
/// Lines may carry an optional `@GroupName@` prefix added by the GUI for
/// organisational grouping; the prefix is stripped transparently before
/// parsing so group-annotated entries resolve exactly as plain ones do.
#[must_use]
pub fn parse_mappings(lines: &[String]) -> Vec<SubjectMapping> {
    lines
        .iter()
        .filter_map(|l| parse_mapping(strip_group_prefix(l)).ok())
        .collect()
}

impl SubjectMapping {
    /// Render the mapping back to its canonical single-line DSL form.
    #[must_use]
    pub fn to_canonical(&self) -> String {
        let kind = if self.is_movie { "movie" } else { "tv" };
        let mut lhs = format!(
            "{}:{}|type:{}",
            self.provider.as_str(),
            self.provider_id,
            kind
        );
        if let Some(s) = self.season {
            // Base season token without the range suffix.
            lhs.push_str(&format!("|S{s}"));
            // Append the range suffix inline with the season.
            let suffix = self.ep_range.as_suffix(s);
            // as_suffix already includes the `|S<n>E…` or `|S<n>E…++` prefix;
            // strip the leading `|S<n>` we already wrote.
            let stripped =
                suffix.strip_prefix(&format!("|S{s}")).unwrap_or(&suffix);
            lhs.push_str(stripped);
        }
        let mut rhs = format!("bgm:{}", self.subject_id);
        if self.ep_offset != 0 {
            rhs.push_str(&format!("|E{:+}", self.ep_offset));
        }
        format!("{lhs} -> {rhs}")
    }

    /// Whether this mapping applies to an item identified by `provider`/`id`,
    /// its `is_movie` kind, `season` (TV only; defaults to season 1), and
    /// `episode` (the Emby `IndexNumber`; `None` matches all ranges).
    #[must_use]
    pub fn matches(
        &self,
        provider: MapProvider,
        id: &str,
        is_movie: bool,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> bool {
        if self.provider != provider
            || self.provider_id != id
            || self.is_movie != is_movie
        {
            return false;
        }
        if self.is_movie {
            return true;
        }
        if self.season.unwrap_or(1) != season.unwrap_or(1) {
            return false;
        }
        self.ep_range.contains(episode)
    }

    /// Whether applying this mapping's offset to a local episode `index` yields
    /// a usable (positive) Bangumi episode number.
    ///
    /// Movies and an unknown index are always usable. A non-positive result
    /// (e.g. `E-59` while watching episode 1) means the offset is too large for
    /// this episode, so the caller should skip the mapping and fall back to the
    /// normal id/title resolution instead of producing an invalid episode.
    #[must_use]
    pub fn yields_positive_episode(&self, index: Option<i64>) -> bool {
        self.is_movie || index.is_none_or(|idx| idx + self.ep_offset > 0)
    }
}

/// Find the first mapping that applies to an item carrying the given
/// `(provider, id)` identities, kind, season, and episode index.
#[must_use]
pub fn match_mapping<'a>(
    mappings: &'a [SubjectMapping],
    ids: &[(MapProvider, &str)],
    is_movie: bool,
    season: Option<u32>,
    episode: Option<u32>,
) -> Option<&'a SubjectMapping> {
    mappings.iter().find(|m| {
        ids.iter()
            .any(|(p, id)| m.matches(*p, id, is_movie, season, episode))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tv_with_offset() {
        let m = parse_mapping("tmdb:10000|type:tv|S4 -> bgm:20000|E+59")
            .expect("valid");
        assert_eq!(m.provider, MapProvider::Tmdb);
        assert_eq!(m.provider_id, "10000");
        assert!(!m.is_movie);
        assert_eq!(m.season, Some(4));
        assert_eq!(m.ep_range, EpRange::All);
        assert_eq!(m.subject_id, 20000);
        assert_eq!(m.ep_offset, 59);
    }

    #[test]
    fn parses_tv_without_offset() {
        let m =
            parse_mapping("tmdb:10000|type:tv|S4 -> bgm:20000").expect("valid");
        assert!(!m.is_movie);
        assert_eq!(m.season, Some(4));
        assert_eq!(m.ep_range, EpRange::All);
        assert_eq!(m.subject_id, 20000);
        assert_eq!(m.ep_offset, 0);
        assert_eq!(m.to_canonical(), "tmdb:10000|type:tv|S4 -> bgm:20000");
    }

    #[test]
    fn parses_movie() {
        let m =
            parse_mapping("tmdb:10001|type:movie -> bgm:20001").expect("valid");
        assert!(m.is_movie);
        assert_eq!(m.season, None);
        assert_eq!(m.ep_range, EpRange::All);
        assert_eq!(m.ep_offset, 0);
        assert_eq!(m.subject_id, 20001);
    }

    #[test]
    fn infers_tv_from_season() {
        let m =
            parse_mapping("tvdb:30000|S2 -> bgm:21000|E+12").expect("valid");
        assert!(!m.is_movie);
        assert_eq!(m.season, Some(2));
        assert_eq!(m.ep_offset, 12);
    }

    #[test]
    fn infers_movie_without_season() {
        let m = parse_mapping("imdb:tt1234567 -> bgm:22000").expect("valid");
        assert!(m.is_movie);
        assert_eq!(m.provider, MapProvider::Imdb);
        assert_eq!(m.provider_id, "tt1234567");
    }

    #[test]
    fn defaults_tv_season_to_one() {
        let m = parse_mapping("tmdb:9|type:tv -> bgm:8").expect("valid");
        assert_eq!(m.season, Some(1));
    }

    #[test]
    fn accepts_negative_offset() {
        let m = parse_mapping("tmdb:1|S1 -> bgm:2|E-3").expect("ok");
        assert_eq!(m.ep_offset, -3);
    }

    #[test]
    fn parses_closed_range_verbose() {
        let m = parse_mapping("tmdb:79481|type:tv|S5E106~S5E157 -> bgm:449353")
            .expect("valid");
        assert!(!m.is_movie);
        assert_eq!(m.season, Some(5));
        assert_eq!(m.ep_range, EpRange::Closed(106, 157));
        assert_eq!(m.subject_id, 449353);
        assert_eq!(m.ep_offset, 0);
    }

    #[test]
    fn parses_closed_range_compact() {
        let m = parse_mapping("tmdb:79481|type:tv|S5E106~157 -> bgm:449353")
            .expect("valid");
        assert_eq!(m.ep_range, EpRange::Closed(106, 157));
    }

    #[test]
    fn parses_open_ended_range() {
        let m = parse_mapping("tmdb:79481|type:tv|S5E158++ -> bgm:562145")
            .expect("valid");
        assert_eq!(m.season, Some(5));
        assert_eq!(m.ep_range, EpRange::From(158));
        assert_eq!(m.subject_id, 562145);
    }

    #[test]
    fn rejects_inverted_range() {
        assert_eq!(
            parse_mapping("tmdb:1|S1E10~S1E5 -> bgm:2").unwrap_err(),
            MapError::EpRange
        );
    }

    #[test]
    fn rejects_unknown_provider() {
        assert_eq!(
            parse_mapping("foo:1 -> bgm:2").unwrap_err(),
            MapError::Provider
        );
    }

    #[test]
    fn rejects_non_numeric_tmdb_id() {
        assert_eq!(
            parse_mapping("tmdb:abc -> bgm:2").unwrap_err(),
            MapError::ProviderId
        );
    }

    #[test]
    fn rejects_bad_imdb_id() {
        assert_eq!(
            parse_mapping("imdb:1234 -> bgm:2").unwrap_err(),
            MapError::ProviderId
        );
    }

    #[test]
    fn rejects_missing_arrow() {
        assert_eq!(
            parse_mapping("tmdb:1 bgm:2").unwrap_err(),
            MapError::Format
        );
    }

    #[test]
    fn rejects_zero_subject() {
        assert_eq!(
            parse_mapping("tmdb:1|S1 -> bgm:0").unwrap_err(),
            MapError::Subject
        );
    }

    #[test]
    fn rejects_movie_with_offset() {
        assert_eq!(
            parse_mapping("tmdb:1|type:movie -> bgm:2|E+3").unwrap_err(),
            MapError::MovieWithSeason
        );
    }

    #[test]
    fn rejects_bad_season() {
        assert_eq!(
            parse_mapping("tmdb:1|S0 -> bgm:2").unwrap_err(),
            MapError::Season
        );
    }

    #[test]
    fn canonical_roundtrips() {
        let line = "tmdb:10000|type:tv|S4 -> bgm:20000|E+59";
        let m = parse_mapping(line).expect("valid");
        assert_eq!(m.to_canonical(), line);
        assert_eq!(parse_mapping(&m.to_canonical()).expect("valid"), m);
    }

    #[test]
    fn canonical_roundtrips_closed_range() {
        let line = "tmdb:79481|type:tv|S5E106~S5E157 -> bgm:449353";
        let m = parse_mapping(line).expect("valid");
        assert_eq!(m.to_canonical(), line);
        assert_eq!(parse_mapping(&m.to_canonical()).expect("valid"), m);
    }

    #[test]
    fn canonical_roundtrips_open_range() {
        let line = "tmdb:79481|type:tv|S5E158++ -> bgm:562145";
        let m = parse_mapping(line).expect("valid");
        assert_eq!(m.to_canonical(), line);
        assert_eq!(parse_mapping(&m.to_canonical()).expect("valid"), m);
    }

    #[test]
    fn canonical_normalises_inferred_type() {
        let m = parse_mapping("tvdb:30000|S2 -> bgm:21000|E+12").expect("ok");
        assert_eq!(m.to_canonical(), "tvdb:30000|type:tv|S2 -> bgm:21000|E+12");
    }

    #[test]
    fn matches_tv_by_provider_season() {
        let m = parse_mapping("tmdb:10000|S4 -> bgm:20000|E+59").expect("ok");
        assert!(m.matches(MapProvider::Tmdb, "10000", false, Some(4), None));
        assert!(!m.matches(MapProvider::Tmdb, "10000", false, Some(3), None));
        assert!(!m.matches(MapProvider::Tmdb, "99999", false, Some(4), None));
        assert!(!m.matches(MapProvider::Tvdb, "10000", false, Some(4), None));
    }

    #[test]
    fn matches_movie_ignores_season() {
        let m = parse_mapping("imdb:tt1 -> bgm:5").expect("ok");
        assert!(m.matches(MapProvider::Imdb, "tt1", true, None, None));
        assert!(!m.matches(MapProvider::Imdb, "tt1", false, Some(1), None));
    }

    #[test]
    fn matches_closed_range() {
        let m = parse_mapping("tmdb:79481|S5E106~S5E157 -> bgm:449353")
            .expect("ok");
        // Inside range.
        assert!(m.matches(
            MapProvider::Tmdb,
            "79481",
            false,
            Some(5),
            Some(106)
        ));
        assert!(m.matches(
            MapProvider::Tmdb,
            "79481",
            false,
            Some(5),
            Some(157)
        ));
        assert!(m.matches(
            MapProvider::Tmdb,
            "79481",
            false,
            Some(5),
            Some(130)
        ));
        // Outside range.
        assert!(!m.matches(
            MapProvider::Tmdb,
            "79481",
            false,
            Some(5),
            Some(105)
        ));
        assert!(!m.matches(
            MapProvider::Tmdb,
            "79481",
            false,
            Some(5),
            Some(158)
        ));
        // Unknown episode → matches (let caller decide).
        assert!(m.matches(MapProvider::Tmdb, "79481", false, Some(5), None));
    }

    #[test]
    fn matches_open_ended_range() {
        let m = parse_mapping("tmdb:79481|S5E158++ -> bgm:562145").expect("ok");
        assert!(m.matches(
            MapProvider::Tmdb,
            "79481",
            false,
            Some(5),
            Some(158)
        ));
        assert!(m.matches(
            MapProvider::Tmdb,
            "79481",
            false,
            Some(5),
            Some(204)
        ));
        assert!(!m.matches(
            MapProvider::Tmdb,
            "79481",
            false,
            Some(5),
            Some(157)
        ));
    }

    #[test]
    fn positive_episode_gate() {
        let neg = parse_mapping("tmdb:1|S4 -> bgm:2|E-59").expect("ok");
        assert!(!neg.yields_positive_episode(Some(1)));
        assert!(neg.yields_positive_episode(Some(60)));
        assert!(neg.yields_positive_episode(None));
        let zero = parse_mapping("tmdb:1|S4 -> bgm:2").expect("ok");
        assert!(zero.yields_positive_episode(Some(1)));
        let movie = parse_mapping("tmdb:1|type:movie -> bgm:2").expect("ok");
        assert!(movie.yields_positive_episode(Some(0)));
    }

    #[test]
    fn match_mapping_picks_first_applicable() {
        let maps = parse_mappings(&[
            "tmdb:1|S1 -> bgm:10".to_owned(),
            "tvdb:2|S1 -> bgm:20".to_owned(),
        ]);
        let ids = [(MapProvider::Tvdb, "2"), (MapProvider::Tmdb, "1")];
        let got =
            match_mapping(&maps, &ids, false, Some(1), None).expect("match");
        assert_eq!(got.subject_id, 10);
    }

    #[test]
    fn match_mapping_selects_by_episode_range() {
        let maps = parse_mappings(&[
            "tmdb:79481|type:tv|S5E106~S5E157 -> bgm:449353".to_owned(),
            "tmdb:79481|type:tv|S5E158++ -> bgm:562145".to_owned(),
        ]);
        let ids = [(MapProvider::Tmdb, "79481")];

        // Episode 106 → first mapping.
        let m = match_mapping(&maps, &ids, false, Some(5), Some(106))
            .expect("match");
        assert_eq!(m.subject_id, 449353);

        // Episode 157 → first mapping.
        let m = match_mapping(&maps, &ids, false, Some(5), Some(157))
            .expect("match");
        assert_eq!(m.subject_id, 449353);

        // Episode 158 → second mapping.
        let m = match_mapping(&maps, &ids, false, Some(5), Some(158))
            .expect("match");
        assert_eq!(m.subject_id, 562145);

        // Episode 204 → second mapping.
        let m = match_mapping(&maps, &ids, false, Some(5), Some(204))
            .expect("match");
        assert_eq!(m.subject_id, 562145);

        // Episode 105 → no match.
        assert!(
            match_mapping(&maps, &ids, false, Some(5), Some(105)).is_none()
        );
    }

    #[test]
    fn parse_mappings_drops_invalid_lines() {
        let maps = parse_mappings(&[
            "tmdb:1|S1 -> bgm:10".to_owned(),
            "garbage".to_owned(),
            "imdb:tt9 -> bgm:99".to_owned(),
        ]);
        assert_eq!(maps.len(), 2);
    }

    #[test]
    fn parse_mappings_strips_group_prefix() {
        let maps = parse_mappings(&[
            "@斗破苍穹@tmdb:1|S1 -> bgm:10".to_owned(),
            "@#@imdb:tt9 -> bgm:99".to_owned(),
            "tvdb:5|S2 -> bgm:55".to_owned(),
        ]);
        assert_eq!(maps.len(), 3);
        assert_eq!(maps.first().map(|m| m.subject_id), Some(10));
        assert_eq!(maps.get(1).map(|m| m.subject_id), Some(99));
        assert_eq!(maps.get(2).map(|m| m.subject_id), Some(55));
    }

    #[test]
    fn strip_group_prefix_handles_variants() {
        assert_eq!(strip_group_prefix("tmdb:1 -> bgm:2"), "tmdb:1 -> bgm:2");
        assert_eq!(
            strip_group_prefix("@GroupA@tmdb:1 -> bgm:2"),
            "tmdb:1 -> bgm:2"
        );
        assert_eq!(
            strip_group_prefix("@斗破苍穹@tmdb:1 -> bgm:2"),
            "tmdb:1 -> bgm:2"
        );
        assert_eq!(strip_group_prefix("@#@tmdb:1 -> bgm:2"), "tmdb:1 -> bgm:2");
        // Malformed prefix (no closing @) → unchanged.
        assert_eq!(strip_group_prefix("@notclosed"), "@notclosed");
    }
}

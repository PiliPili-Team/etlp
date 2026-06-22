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
//! tmdb:10000|type:tv|S4 -> bgm:20000|E+59   # S4 episode 5 → subject 20000 ep 64
//! tmdb:10001|type:movie -> bgm:20001        # a movie, no offset
//! tvdb:30000|S2 -> bgm:21000|E+12           # type inferred as tv from S2
//! imdb:tt1234567 -> bgm:22000               # type inferred as movie
//! ```
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
            MapError::Subject => "map_err_subject",
            MapError::Offset => "map_err_offset",
            MapError::MovieWithSeason => "map_err_movie_season",
        }
    }
}

/// Parse the LHS `provider:id` plus optional `type:`/`S<n>` parts.
struct Lhs {
    provider: MapProvider,
    provider_id: String,
    explicit_type: Option<bool>, // Some(true)=movie, Some(false)=tv
    season: Option<u32>,
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
        } else if let Some(num) =
            part.strip_prefix('S').or_else(|| part.strip_prefix('s'))
        {
            let n: u32 = num.trim().parse().map_err(|_| MapError::Season)?;
            if n == 0 {
                return Err(MapError::Season);
            }
            season = Some(n);
        } else {
            return Err(MapError::Format);
        }
    }

    Ok(Lhs {
        provider,
        provider_id,
        explicit_type,
        season,
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

    Ok(SubjectMapping {
        provider: lhs.provider,
        provider_id: lhs.provider_id,
        is_movie,
        season,
        subject_id,
        ep_offset,
    })
}

/// Parse every line, silently dropping the invalid ones. Used on the server
/// side where a bad entry must not abort the whole sync; the GUI validates
/// strictly before an entry is ever stored.
#[must_use]
pub fn parse_mappings(lines: &[String]) -> Vec<SubjectMapping> {
    lines.iter().filter_map(|l| parse_mapping(l).ok()).collect()
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
            lhs.push_str(&format!("|S{s}"));
        }
        let mut rhs = format!("bgm:{}", self.subject_id);
        if self.ep_offset != 0 {
            rhs.push_str(&format!("|E{:+}", self.ep_offset));
        }
        format!("{lhs} -> {rhs}")
    }

    /// Whether this mapping applies to an item identified by `provider`/`id`,
    /// its `is_movie` kind and `season` (TV only; defaults to season 1).
    #[must_use]
    pub fn matches(
        &self,
        provider: MapProvider,
        id: &str,
        is_movie: bool,
        season: Option<u32>,
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
        self.season.unwrap_or(1) == season.unwrap_or(1)
    }
}

/// Find the first mapping that applies to an item carrying the given
/// `(provider, id)` identities, kind and season.
#[must_use]
pub fn match_mapping<'a>(
    mappings: &'a [SubjectMapping],
    ids: &[(MapProvider, &str)],
    is_movie: bool,
    season: Option<u32>,
) -> Option<&'a SubjectMapping> {
    mappings.iter().find(|m| {
        ids.iter()
            .any(|(p, id)| m.matches(*p, id, is_movie, season))
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
        assert_eq!(m.subject_id, 20000);
        assert_eq!(m.ep_offset, 59);
    }

    #[test]
    fn parses_movie() {
        let m =
            parse_mapping("tmdb:10001|type:movie -> bgm:20001").expect("valid");
        assert!(m.is_movie);
        assert_eq!(m.season, None);
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
        let m = parse_mapping("tmdb:1|S1 -> bgm:2|E-3").expect("valid");
        assert_eq!(m.ep_offset, -3);
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
        // Re-parsing the canonical form yields the same mapping.
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
        assert!(m.matches(MapProvider::Tmdb, "10000", false, Some(4)));
        assert!(!m.matches(MapProvider::Tmdb, "10000", false, Some(3)));
        assert!(!m.matches(MapProvider::Tmdb, "99999", false, Some(4)));
        assert!(!m.matches(MapProvider::Tvdb, "10000", false, Some(4)));
    }

    #[test]
    fn matches_movie_ignores_season() {
        let m = parse_mapping("imdb:tt1 -> bgm:5").expect("ok");
        assert!(m.matches(MapProvider::Imdb, "tt1", true, None));
        assert!(!m.matches(MapProvider::Imdb, "tt1", false, Some(1)));
    }

    #[test]
    fn match_mapping_picks_first_applicable() {
        let maps = parse_mappings(&[
            "tmdb:1|S1 -> bgm:10".to_owned(),
            "tvdb:2|S1 -> bgm:20".to_owned(),
        ]);
        let ids = [(MapProvider::Tvdb, "2"), (MapProvider::Tmdb, "1")];
        let got = match_mapping(&maps, &ids, false, Some(1)).expect("match");
        assert_eq!(got.subject_id, 10);
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
}

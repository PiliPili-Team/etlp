//! TOML configuration loading and string-match rules for etlp.
//!
//! Wraps the on-disk `config.toml` file and exposes typed section structs.
//! Every field carries a sensible default so a minimal config file only needs
//! to specify the keys that differ from the defaults.

pub mod matching;

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

/// Errors raised while locating or parsing the configuration file.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// No `config.toml` existed in the search directory.
    #[error("no config file found in {0}")]
    NotFound(PathBuf),

    /// The file could not be read.
    #[error("failed to read config {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The file was not valid TOML.
    #[error("failed to parse config {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

/// Convenience alias for config results.
pub type Result<T> = std::result::Result<T, ConfigError>;

// ── Section structs ───────────────────────────────────────────────────────────

/// `[emby]` section — player selection and launch options.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EmbySection {
    /// Player to launch (`mpv` / `vlc` / `mpc` / `potplayer` / `iina` / …).
    pub player: String,
    /// Launch the player in full-screen mode.
    pub fullscreen: bool,
    /// Mute audio on launch (mpv `--mute=yes`).
    pub disable_audio: bool,
}

impl Default for EmbySection {
    fn default() -> Self {
        Self {
            player: "mpv".to_owned(),
            fullscreen: false,
            disable_audio: false,
        }
    }
}

/// Bounds for `[dev] log_max_size_mb` (in mebibytes). These mirror the clamps
/// `etlp-logging`'s `LogRotation::from_mb` applies at consumption; keep the two
/// in sync. Enforced again at load time so a hand-edited config can never push
/// an out-of-range value into the app state or the GUI.
pub const LOG_MAX_SIZE_MB_MIN: u64 = 20;
pub const LOG_MAX_SIZE_MB_MAX: u64 = 200;

/// Bounds for `[dev] log_max_files` (the active file included). Mirror
/// `etlp-logging`'s `MAX_MAX_FILES`; see [`LOG_MAX_SIZE_MB_MIN`].
pub const LOG_MAX_FILES_MIN: usize = 1;
pub const LOG_MAX_FILES_MAX: usize = 14;

/// `[dev]` section — developer / advanced options.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DevSection {
    /// Absolute path to the player binary; overrides `emby.player` for lookup.
    pub player_path: Option<String>,
    /// HTTP proxy passed to the player (`host:port`).
    pub http_proxy: Option<String>,
    /// mpv `--input-ipc-server` socket path; enables IPC control.
    pub mpv_input_ipc_server: Option<String>,
    /// Enable log masking (mix sensitive tokens into placeholder text).
    pub mix_log: bool,
    /// Minimum log level: `trace` / `debug` / `info` / `warn` / `error`.
    pub log_level: String,
    /// Optional path to a log file; absent means stderr only.
    pub log_file: Option<PathBuf>,
    /// Maximum size of a single log file, in megabytes, before it rotates.
    pub log_max_size_mb: u64,
    /// Maximum number of log files to keep (the active file included).
    pub log_max_files: usize,
    /// Kill leftover player processes on startup.
    pub kill_process_at_start: bool,
    /// HTTP proxy for the etlp process itself (`host:port`).
    pub proxy: Option<String>,
    /// Disable TLS certificate verification (insecure; for local dev only).
    pub skip_certificate_verify: bool,
    /// Hosts whose `.strm` files are played in-place without redirect.
    pub strm_direct_host: Vec<String>,
    /// `[from, to, …]` pairs used to rewrite the resolved stream URL.
    pub stream_redirect: Vec<String>,
    /// Hosts probed for a 30x redirect before handing the URL to the player.
    pub redirect_check_host: Vec<String>,
    /// Literal prefix prepended to the stream URL.
    pub stream_prefix: Vec<String>,
    /// Path prefixes that force read-from-disk mode.
    pub force_disk_mode_path: Vec<String>,
    /// Ordered keywords for multi-version preference (first wins).
    pub version_prefer: Vec<String>,
    /// Ordered keywords for subtitle selection.
    pub subtitle_priority: Vec<String>,
    /// Prepend the server title to the player window title.
    pub pretty_title: bool,
    /// Disable the playlist when the current episode is the last one.
    pub last_ep_disable_playlist: bool,
    /// Fill remaining playlist slots using `version_prefer` order.
    pub version_prefer_for_playlist: bool,
    /// Bearer token required by `GET /send_media_file`; absent disables auth.
    pub http_server_token: Option<String>,
    /// Ordered keywords for cross-version subtitle fallback extraction.
    ///
    /// When the selected media version has no subtitle, etlp scans other
    /// available versions and picks the first subtitle track whose
    /// `"{title},{display_title}"` matches one of these keywords.
    pub sub_extract_priority: Vec<String>,
    /// Character-translation pairs for `media_title`, in full-width-comma
    /// (`，`) separated format: `src1，dst1，src2，dst2，…`
    ///
    /// Example: `'，＇，"，＂` maps ASCII quotes to their full-width equivalents.
    /// Empty string disables translation.
    pub media_title_translate: String,
    /// Custom `User-Agent` for normal HTTP requests.
    ///
    /// Absent or empty falls back to the built-in default (`"etlp"`).
    /// Download and prefetch clients always use their own fixed User-Agents
    /// regardless of this setting.
    pub user_agent: Option<String>,
    /// When true, etlp will not report playback progress back to the
    /// Emby / Jellyfin server (no `/Sessions/Playing/Progress` calls).
    pub disable_progress_report: bool,
}

impl Default for DevSection {
    fn default() -> Self {
        Self {
            player_path: None,
            http_proxy: None,
            mpv_input_ipc_server: None,
            mix_log: true,
            log_level: "info".to_owned(),
            log_file: None,
            log_max_size_mb: 50,
            log_max_files: 7,
            kill_process_at_start: true,
            proxy: None,
            skip_certificate_verify: false,
            strm_direct_host: Vec::new(),
            stream_redirect: Vec::new(),
            redirect_check_host: Vec::new(),
            stream_prefix: Vec::new(),
            force_disk_mode_path: Vec::new(),
            version_prefer: Vec::new(),
            subtitle_priority: Vec::new(),
            pretty_title: true,
            last_ep_disable_playlist: false,
            version_prefer_for_playlist: true,
            http_server_token: None,
            sub_extract_priority: Vec::new(),
            media_title_translate: String::new(),
            user_agent: None,
            disable_progress_report: false,
        }
    }
}

/// Upper bound for `[playlist] item_limit`; mirrors the GUI's maximum. `0` means
/// "use default 100 episodes" at runtime. Enforced at load time so a hand-edited
/// config cannot exceed the supported cap.
pub const ITEM_LIMIT_MAX: u32 = 100;

/// `[playlist]` section — playlist assembly options.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PlaylistSection {
    /// Maximum episodes in the player playlist (`0` = default 100).
    pub item_limit: u32,
    /// Regex that selects one version per episode (empty = no filter).
    pub version_filter: String,
}

impl Default for PlaylistSection {
    fn default() -> Self {
        Self {
            item_limit: 0,
            version_filter: String::new(),
        }
    }
}

/// `[dandan]` section — local DanDanPlay integration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DanDanSection {
    /// Local DanDanPlay API port.
    pub port: u16,
    /// Optional API key for DanDanPlay.
    pub api_key: Option<String>,
}

impl Default for DanDanSection {
    fn default() -> Self {
        Self {
            port: 8080,
            api_key: None,
        }
    }
}

/// `[gui]` section — download and cache options.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GuiSection {
    /// Download speed cap in MiB/s (0 = unlimited).
    pub speed_limit_mb: u64,
    /// Directory for the download cache; defaults to `{working_dir}/cache`.
    pub server_cache_path: Option<PathBuf>,
    /// Launch hidden to the tray instead of showing the main window. Pairs with
    /// OS autostart so the app starts quietly on login.
    pub silent_start: bool,
    /// Periodically check GitHub for a newer release and surface an update hint.
    pub check_update: bool,
    /// Legacy launch-at-login flag. The OS registration (LaunchAgent) is now the
    /// source of truth; this field is read only once, to carry an old AppleScript
    /// preference forward, after which the migration strips it from the config.
    pub autostart: bool,
}

impl Default for GuiSection {
    fn default() -> Self {
        Self {
            speed_limit_mb: 0,
            server_cache_path: None,
            silent_start: false,
            check_update: true,
            autostart: false,
        }
    }
}

fn default_redirect_uri() -> String {
    "http://localhost:58000/trakt_auth".to_owned()
}

/// Lower bound (seconds) for the repeated-mark throttle window. Any configured
/// value below this — migrated from an old config or hand-edited — is clamped
/// up to it on read, so a too-short window can never defeat de-duplication.
pub const MIN_DUPLICATE_THROTTLE_SECS: u64 = 120;

/// Throttle window (seconds) used when the key is absent, e.g. a config written
/// before the field existed (older versions hard-coded 600s; migrated installs
/// adopt this shorter, configurable default).
pub const DEFAULT_DUPLICATE_THROTTLE_SECS: u64 = 300;

fn default_duplicate_throttle_secs() -> u64 {
    DEFAULT_DUPLICATE_THROTTLE_SECS
}

/// `[trakt]` section — Trakt.tv scrobble integration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TraktSection {
    pub client_id: String,
    pub client_secret: String,
    /// Trakt username (not the display nickname) used for history lookups.
    pub user_name: String,
    /// OAuth redirect URI registered with Trakt.
    #[serde(default = "default_redirect_uri")]
    pub redirect_uri: String,
    /// Comma-separated host keywords that trigger the Trakt scrobble.
    pub enable_host: String,
    /// Allow re-marking the same episode/movie on every completion instead of
    /// throttling repeats. When `true`, the just-watched item bypasses the
    /// in-process throttle so finishing it again immediately adds another play
    /// to the Trakt history. Backfill of earlier episodes is always
    /// de-duplicated against the existing history regardless of this flag.
    pub allow_duplicate: bool,
    /// Throttle window, in seconds, for de-duplicating repeated marks of the
    /// same item while `allow_duplicate` is `false`: finishing the same item
    /// again within this window is recorded only once. Configs predating this
    /// field migrate to [`DEFAULT_DUPLICATE_THROTTLE_SECS`]; the stored value is
    /// clamped up to [`MIN_DUPLICATE_THROTTLE_SECS`] by [`Self::duplicate_throttle`].
    #[serde(default = "default_duplicate_throttle_secs")]
    pub duplicate_throttle_secs: u64,
}

impl TraktSection {
    /// Effective repeated-mark throttle window, clamped to at least
    /// [`MIN_DUPLICATE_THROTTLE_SECS`] so a too-small configured value can never
    /// shrink the de-duplication window below the supported floor.
    pub fn duplicate_throttle(&self) -> Duration {
        Duration::from_secs(
            self.duplicate_throttle_secs
                .max(MIN_DUPLICATE_THROTTLE_SECS),
        )
    }
}

impl Default for TraktSection {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            user_name: String::new(),
            redirect_uri: default_redirect_uri(),
            enable_host: String::new(),
            allow_duplicate: false,
            duplicate_throttle_secs: DEFAULT_DUPLICATE_THROTTLE_SECS,
        }
    }
}

fn default_bangumi_genres() -> String {
    "动画|anime".to_owned()
}

fn default_true() -> bool {
    true
}

/// `[bangumi]` section — bgm.tv integration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BangumiSection {
    /// Comma-separated host keywords that enable the sync; empty disables it.
    pub enable_host: String,
    /// bgm.tv username or UID used to read the user's own collection.
    pub username: String,
    /// Personal access token for the bgm.tv API.
    pub access_token: String,
    /// Whether new collection entries are marked private. Defaults to `true`.
    #[serde(default = "default_true")]
    pub private: bool,
    /// Regex matched against the series genres; only matching series sync.
    #[serde(default = "default_bangumi_genres")]
    pub genres: String,
    /// When an item lacks a `ProviderIds.Bangumi`, resolve the subject by
    /// searching bgm by title (native title / series name) and walking the
    /// sequel chain to the right season. Defaults to enabled.
    #[serde(default = "default_true")]
    pub title_search_fallback: bool,
    /// User-defined provider→Bangumi subject mappings, one DSL line each, e.g.
    /// `tmdb:10000|type:tv|S4 -> bgm:20000|E+59`. Parsed and applied as the
    /// highest-priority subject resolver (see `etlp-sync::bangumi_map`).
    #[serde(default)]
    pub subject_map: Vec<String>,
}

impl Default for BangumiSection {
    fn default() -> Self {
        Self {
            enable_host: String::new(),
            username: String::new(),
            access_token: String::new(),
            private: true,
            genres: default_bangumi_genres(),
            title_search_fallback: true,
            subject_map: Vec::new(),
        }
    }
}

/// A single path-translation entry (one element of `[[path_map]]`).
#[derive(Debug, Clone, Deserialize)]
pub struct PathMapEntry {
    /// Source path prefix (as seen by the media server).
    pub src: String,
    /// Destination path prefix (as seen by the local player).
    pub dst: String,
}

// ── Internal serde target ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct RawConfig {
    emby: EmbySection,
    dev: DevSection,
    playlist: PlaylistSection,
    dandan: DanDanSection,
    gui: GuiSection,
    trakt: TraktSection,
    bangumi: BangumiSection,
    path_map: Vec<PathMapEntry>,
}

// ── Public Config ─────────────────────────────────────────────────────────────

/// Loaded configuration backed by a TOML file.
///
/// Every section has a `Default` implementation, so missing sections or keys
/// silently fall back to the documented defaults.
#[derive(Debug, Clone)]
pub struct Config {
    pub emby: EmbySection,
    pub dev: DevSection,
    pub playlist: PlaylistSection,
    pub dandan: DanDanSection,
    pub gui: GuiSection,
    pub trakt: TraktSection,
    pub bangumi: BangumiSection,
    /// Ordered src→dst path-translation pairs (`[[path_map]]` array).
    pub path_map: Vec<PathMapEntry>,
    path: PathBuf,
}

/// The single canonical config file name. The app reads and writes exactly
/// this file, so the loaded path always matches the written path.
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// The path of the config file in `dir`, if it exists.
///
/// Returns `None` when no config is present yet. Because there is only one
/// canonical name, the file callers write to always matches the one the app
/// loaded — no shadow file can ever reset the user's settings.
#[must_use]
pub fn existing_config_path(dir: &Path) -> Option<PathBuf> {
    let path = dir.join(CONFIG_FILE_NAME);
    path.is_file().then_some(path)
}

/// Write TOML config text to `path` with platform-appropriate encoding,
/// creating any missing parent directories.
///
/// On Windows the content is prefixed with a UTF-8 BOM so editors that would
/// otherwise fall back to the legacy ANSI code page (e.g. Notepad on older
/// builds) render multi-byte comments correctly instead of as mojibake. On
/// other platforms any leading BOM is stripped to keep files clean. Because
/// [`Config::load_file`] tolerates a BOM, the round-trip is lossless on every
/// OS — a config saved on Windows still parses on macOS/Linux and vice versa.
pub fn write_config_str(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let body = content.strip_prefix('\u{feff}').unwrap_or(content);
    #[cfg(target_os = "windows")]
    {
        let mut bytes = Vec::with_capacity(body.len() + 3);
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        bytes.extend_from_slice(body.as_bytes());
        std::fs::write(path, bytes)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::fs::write(path, body)
    }
}

impl Config {
    /// Load `config.toml` from `dir`.
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let path = dir.join(CONFIG_FILE_NAME);
        if path.is_file() {
            return Self::load_file(&path);
        }
        Err(ConfigError::NotFound(dir.to_path_buf()))
    }

    /// Load a specific TOML config file, tolerating a UTF-8 BOM.
    pub fn load_file(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|source| {
            ConfigError::Io {
                path: path.to_path_buf(),
                source,
            }
        })?;
        let content = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
        let inner: RawConfig =
            toml::from_str(content).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(Self::from_raw(inner, path.to_path_buf()))
    }

    /// Reload from the originally loaded path.
    pub fn reload(&mut self) -> Result<()> {
        *self = Self::load_file(&self.path)?;
        Ok(())
    }

    /// The path this config was loaded from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Src→dst translation pairs as `(src, dst)` tuples.
    ///
    /// Convenience method for callers that already use `Vec<(String, String)>`.
    #[must_use]
    pub fn path_translation_pairs(&self) -> Vec<(String, String)> {
        self.path_map
            .iter()
            .map(|e| (e.src.clone(), e.dst.clone()))
            .collect()
    }

    /// Write a minimal default config to `path`, creating parent dirs.
    ///
    /// The written file is valid TOML that loads with all defaults; callers
    /// can pass the same path immediately to [`Config::load_file`].
    pub fn write_default(path: &Path) -> std::io::Result<()> {
        write_config_str(path, DEFAULT_CONFIG_TOML)
    }

    /// Return a `Config` with all fields at their defaults, pointing at `path`.
    ///
    /// Use this when the config file exists but cannot be parsed — the caller
    /// gets a safe in-memory default without touching the user's file on disk.
    pub fn with_defaults(path: PathBuf) -> Self {
        Self::from_raw(RawConfig::default(), path)
    }

    fn from_raw(raw: RawConfig, path: PathBuf) -> Self {
        // Clamp every range-bounded field at the single load boundary that all
        // entry points (file, reload, in-memory defaults) funnel through, so a
        // hand-edited config can never bypass a GUI-enforced limit: the stored
        // values the rest of the app and the GUI observe are always in range.
        let mut dev = raw.dev;
        dev.log_max_size_mb = dev
            .log_max_size_mb
            .clamp(LOG_MAX_SIZE_MB_MIN, LOG_MAX_SIZE_MB_MAX);
        dev.log_max_files = dev
            .log_max_files
            .clamp(LOG_MAX_FILES_MIN, LOG_MAX_FILES_MAX);

        let mut playlist = raw.playlist;
        // `0` means "default 100 episodes" at runtime; cap non-zero values.
        if playlist.item_limit > 0 {
            playlist.item_limit = playlist.item_limit.min(ITEM_LIMIT_MAX);
        }

        let mut trakt = raw.trakt;
        trakt.duplicate_throttle_secs = trakt
            .duplicate_throttle_secs
            .max(MIN_DUPLICATE_THROTTLE_SECS);

        Self {
            emby: raw.emby,
            dev,
            playlist,
            dandan: raw.dandan,
            gui: raw.gui,
            trakt,
            bangumi: raw.bangumi,
            path_map: raw.path_map,
            path,
        }
    }
}

/// Minimal default configuration written on first run.
const DEFAULT_CONFIG_TOML: &str = "\
# etlp configuration — https://github.com/your-org/etlp
# All keys shown here are the built-in defaults; uncomment and edit as needed.

[emby]
# player = \"mpv\"
# fullscreen = false
# disable_audio = false

[dev]
# log_level = \"info\"
# log_max_size_mb = 50   # rotate once the log exceeds this size (20–200 MB)
# log_max_files = 7      # number of rotated log files to keep (1–14)
# kill_process_at_start = true
# pretty_title = true
# user_agent = \"etlp\"   # custom User-Agent for normal requests; download/prefetch UAs are fixed
";

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;

    use super::*;

    const SAMPLE: &str = r#"
[emby]
player = "mpv"
fullscreen = false

[playlist]
item_limit = 50

[dev]
version_prefer = ["VCB", "Baha"]
speed_dummy = 1.5
"#;

    fn write_config(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create temp config");
        f.write_all(body.as_bytes()).expect("write temp config");
        path
    }

    #[test]
    fn typed_fields_read_values() {
        let dir = tempdir().expect("tempdir");
        write_config(dir.path(), "config.toml", SAMPLE);
        let cfg = Config::load_from_dir(dir.path()).expect("load");

        assert_eq!(cfg.emby.player, "mpv");
        assert!(!cfg.emby.fullscreen);
        assert_eq!(cfg.playlist.item_limit, 50);
        assert_eq!(cfg.dev.version_prefer, vec!["VCB", "Baha"]);
    }

    #[test]
    fn missing_keys_use_defaults() {
        let dir = tempdir().expect("tempdir");
        write_config(dir.path(), "config.toml", "[emby]\nplayer = \"vlc\"\n");
        let cfg = Config::load_from_dir(dir.path()).expect("load");

        // explicit
        assert_eq!(cfg.emby.player, "vlc");
        // all defaulted
        assert!(!cfg.emby.disable_audio);
        assert!(cfg.dev.mix_log);
        assert_eq!(cfg.dev.log_level, "info");
        assert_eq!(cfg.dev.log_max_size_mb, 50);
        assert_eq!(cfg.dev.log_max_files, 7);
        assert!(cfg.dev.kill_process_at_start);
        assert_eq!(cfg.playlist.item_limit, 0);
        assert_eq!(cfg.dandan.port, 8080);
        assert_eq!(cfg.gui.speed_limit_mb, 0);
        assert!(cfg.dev.version_prefer.is_empty());
        assert_eq!(cfg.trakt.redirect_uri, "http://localhost:58000/trakt_auth");
        assert!(cfg.trakt.user_name.is_empty());
        // A config predating the field migrates to the default window.
        assert_eq!(
            cfg.trakt.duplicate_throttle_secs,
            DEFAULT_DUPLICATE_THROTTLE_SECS
        );
        // bangumi defaults
        assert!(cfg.bangumi.enable_host.is_empty());
        assert!(cfg.bangumi.access_token.is_empty());
        assert!(cfg.bangumi.private);
        assert_eq!(cfg.bangumi.genres, "动画|anime");
    }

    #[test]
    fn duplicate_throttle_clamped_on_load() {
        let dir = tempdir().expect("tempdir");
        // A hand-edited value below the floor is normalised at load time, so
        // the stored field itself — not just the derived window — is clamped.
        // This prevents bypassing the minimum by editing the config file, and
        // ensures the GUI never displays a sub-floor value.
        write_config(
            dir.path(),
            "config.toml",
            "[trakt]\nduplicate_throttle_secs = 30\n",
        );
        let cfg = Config::load_from_dir(dir.path()).expect("load");
        assert_eq!(
            cfg.trakt.duplicate_throttle_secs,
            MIN_DUPLICATE_THROTTLE_SECS
        );
        assert_eq!(
            cfg.trakt.duplicate_throttle().as_secs(),
            MIN_DUPLICATE_THROTTLE_SECS
        );

        // A value at or above the floor is honoured as-is.
        write_config(
            dir.path(),
            "config.toml",
            "[trakt]\nduplicate_throttle_secs = 900\n",
        );
        let cfg = Config::load_from_dir(dir.path()).expect("load");
        assert_eq!(cfg.trakt.duplicate_throttle_secs, 900);
        assert_eq!(cfg.trakt.duplicate_throttle().as_secs(), 900);
    }

    #[test]
    fn numeric_bounds_clamped_on_load() {
        let dir = tempdir().expect("tempdir");
        // Out-of-range values a user could only set by hand-editing the file are
        // pulled back into the supported range at load time, so they cannot
        // bypass the limits the GUI enforces on these same fields.
        write_config(
            dir.path(),
            "config.toml",
            "[dev]\n\
             log_max_size_mb = 99999\n\
             log_max_files = 0\n\
             [playlist]\n\
             item_limit = 5000\n",
        );
        let cfg = Config::load_from_dir(dir.path()).expect("load");
        assert_eq!(cfg.dev.log_max_size_mb, LOG_MAX_SIZE_MB_MAX);
        assert_eq!(cfg.dev.log_max_files, LOG_MAX_FILES_MIN);
        assert_eq!(cfg.playlist.item_limit, ITEM_LIMIT_MAX);

        // `item_limit = 0` means "use default 100 at runtime"; the stored value
        // stays 0 (it is not raised to ITEM_LIMIT_MAX at load time).
        write_config(dir.path(), "config.toml", "[playlist]\nitem_limit = 0\n");
        let cfg = Config::load_from_dir(dir.path()).expect("load");
        assert_eq!(cfg.playlist.item_limit, 0);
    }

    #[test]
    fn bangumi_section_parses_custom_values() {
        let dir = tempdir().expect("tempdir");
        write_config(
            dir.path(),
            "config.toml",
            "[bangumi]\n\
             enable_host = \"localhost, 192.168.\"\n\
             username = \"tester\"\n\
             access_token = \"tok\"\n\
             private = false\n\
             genres = \"动画\"\n",
        );
        let cfg = Config::load_from_dir(dir.path()).expect("load");
        assert_eq!(cfg.bangumi.enable_host, "localhost, 192.168.");
        assert_eq!(cfg.bangumi.username, "tester");
        assert_eq!(cfg.bangumi.access_token, "tok");
        assert!(!cfg.bangumi.private);
        assert_eq!(cfg.bangumi.genres, "动画");
    }

    #[test]
    fn bom_is_tolerated() {
        let dir = tempdir().expect("tempdir");
        let body = "\u{feff}[emby]\nplayer = \"mpv\"\n".to_string();
        write_config(dir.path(), "config.toml", &body);
        let cfg = Config::load_from_dir(dir.path()).expect("load bom");
        assert_eq!(cfg.emby.player, "mpv");
    }

    #[test]
    fn write_config_str_roundtrips() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("config.toml");
        // Parent dir does not exist yet; the writer must create it.
        write_config_str(&path, "[emby]\nplayer = \"iina\"\n")
            .expect("write config");
        let cfg = Config::load_file(&path).expect("load written config");
        assert_eq!(cfg.emby.player, "iina");
    }

    #[test]
    fn write_config_str_strips_incoming_bom() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        // Even when the source carries a BOM, the file still parses cleanly
        // (on Windows a single BOM is re-added; load_file tolerates it).
        write_config_str(&path, "\u{feff}[emby]\nplayer = \"vlc\"\n")
            .expect("write config");
        let cfg = Config::load_file(&path).expect("load");
        assert_eq!(cfg.emby.player, "vlc");
        let raw = std::fs::read(&path).expect("read raw");
        // The body must never contain a double BOM.
        assert!(!raw.starts_with(&[0xEF, 0xBB, 0xBF, 0xEF, 0xBB, 0xBF]));
    }

    #[test]
    fn existing_config_path_none_when_absent() {
        let dir = tempdir().expect("tempdir");
        assert_eq!(existing_config_path(dir.path()), None);
    }

    #[test]
    fn existing_config_path_resolves_config_toml() {
        let dir = tempdir().expect("tempdir");
        assert_eq!(existing_config_path(dir.path()), None);
        write_config(dir.path(), "config.toml", "[emby]\n");
        assert_eq!(
            existing_config_path(dir.path()),
            Some(dir.path().join("config.toml")),
            "config.toml must resolve once present"
        );
    }

    #[test]
    fn missing_config_dir_errors() {
        let dir = tempdir().expect("tempdir");
        let err = Config::load_from_dir(dir.path()).unwrap_err();
        assert!(matches!(err, ConfigError::NotFound(_)));
    }

    #[test]
    fn path_translation_pairs_from_path_map() {
        let body = r#"
[[path_map]]
src = "/mnt/disk1"
dst = "E:"

[[path_map]]
src = "/mnt/disk2/media"
dst = 'F:\media'
"#;
        let dir = tempdir().expect("tempdir");
        write_config(dir.path(), "config.toml", body);
        let cfg = Config::load_from_dir(dir.path()).expect("load");
        assert_eq!(
            cfg.path_translation_pairs(),
            vec![
                ("/mnt/disk1".to_owned(), "E:".to_owned()),
                ("/mnt/disk2/media".to_owned(), r"F:\media".to_owned()),
            ]
        );
    }
}

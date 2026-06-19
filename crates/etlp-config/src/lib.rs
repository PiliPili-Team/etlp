//! TOML configuration loading and string-match rules for etlp.
//!
//! Wraps the on-disk `embyToLocalPlayer*.toml` file and exposes typed section
//! structs. Every field carries a sensible default so a minimal config file
//! only needs to specify the keys that differ from the defaults.

pub mod matching;

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

/// Errors raised while locating or parsing the configuration file.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// No candidate toml file existed in the search directory.
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
    /// Mute audio on launch (mpv `--no-audio`).
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

/// `[playlist]` section — playlist assembly options.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PlaylistSection {
    /// Maximum episodes to append to the player playlist (`0` = no limit).
    pub item_limit: u32,
    /// Regex that selects one version per episode (empty = no filter).
    pub version_filter: String,
}

impl Default for PlaylistSection {
    fn default() -> Self {
        Self {
            item_limit: 10,
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
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct GuiSection {
    /// Download speed cap in MiB/s (0 = unlimited).
    pub speed_limit_mb: u64,
    /// Directory for the download cache; defaults to `{working_dir}/cache`.
    pub server_cache_path: Option<PathBuf>,
}

fn default_redirect_uri() -> String {
    "http://localhost:58000/trakt_auth".to_owned()
}

/// `[trakt]` section — Trakt.tv scrobble integration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TraktSection {
    pub client_id: String,
    pub client_secret: String,
    /// OAuth redirect URI registered with Trakt.
    #[serde(default = "default_redirect_uri")]
    pub redirect_uri: String,
    /// Host suffix that triggers Trakt scrobble.
    pub enable_host: String,
}

impl Default for TraktSection {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: default_redirect_uri(),
            enable_host: String::new(),
        }
    }
}

/// `[bangumi]` section — bgm.tv integration.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct BangumiSection {
    /// Personal access token for the bgm.tv API.
    pub access_token: Option<String>,
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

/// The `platform.system()` name used in the platform-specific config file
/// (`embyToLocalPlayer-<Platform>.toml`).
#[must_use]
pub fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "Darwin",
        _ => "Linux",
    }
}

fn candidate_names() -> [String; 4] {
    [
        "config.toml".to_owned(),
        format!("embyToLocalPlayer-{}.toml", platform_name()),
        "embyToLocalPlayer.toml".to_owned(),
        "embyToLocalPlayer_config.toml".to_owned(),
    ]
}

/// The path of the first existing candidate config in `dir`, if any.
///
/// Mirrors [`Config::load_from_dir`]'s search order so that callers writing
/// config changes can patch the exact file the app loaded, instead of
/// silently creating a competing `config.toml` that would shadow the user's
/// real config (e.g. `embyToLocalPlayer.toml`) on the next launch.
#[must_use]
pub fn existing_config_path(dir: &Path) -> Option<PathBuf> {
    candidate_names()
        .into_iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

impl Config {
    /// Load the first existing candidate config from `dir`.
    ///
    /// Search order: `config.toml`, `embyToLocalPlayer-<Platform>.toml`,
    /// `embyToLocalPlayer.toml`, `embyToLocalPlayer_config.toml`.
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        for name in candidate_names() {
            let path = dir.join(&name);
            if path.is_file() {
                return Self::load_file(&path);
            }
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
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, DEFAULT_CONFIG_TOML)
    }

    /// Return a `Config` with all fields at their defaults, pointing at `path`.
    ///
    /// Use this when the config file exists but cannot be parsed — the caller
    /// gets a safe in-memory default without touching the user's file on disk.
    pub fn with_defaults(path: PathBuf) -> Self {
        Self::from_raw(RawConfig::default(), path)
    }

    fn from_raw(raw: RawConfig, path: PathBuf) -> Self {
        Self {
            emby: raw.emby,
            dev: raw.dev,
            playlist: raw.playlist,
            dandan: raw.dandan,
            gui: raw.gui,
            trakt: raw.trakt,
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
        write_config(dir.path(), "embyToLocalPlayer.toml", SAMPLE);
        let cfg = Config::load_from_dir(dir.path()).expect("load");

        assert_eq!(cfg.emby.player, "mpv");
        assert!(!cfg.emby.fullscreen);
        assert_eq!(cfg.playlist.item_limit, 50);
        assert_eq!(cfg.dev.version_prefer, vec!["VCB", "Baha"]);
    }

    #[test]
    fn missing_keys_use_defaults() {
        let dir = tempdir().expect("tempdir");
        write_config(
            dir.path(),
            "embyToLocalPlayer.toml",
            "[emby]\nplayer = \"vlc\"\n",
        );
        let cfg = Config::load_from_dir(dir.path()).expect("load");

        // explicit
        assert_eq!(cfg.emby.player, "vlc");
        // all defaulted
        assert!(!cfg.emby.disable_audio);
        assert!(cfg.dev.mix_log);
        assert_eq!(cfg.dev.log_level, "info");
        assert!(cfg.dev.kill_process_at_start);
        assert_eq!(cfg.playlist.item_limit, 10);
        assert_eq!(cfg.dandan.port, 8080);
        assert_eq!(cfg.gui.speed_limit_mb, 0);
        assert!(cfg.dev.version_prefer.is_empty());
        assert_eq!(cfg.trakt.redirect_uri, "http://localhost:58000/trakt_auth");
    }

    #[test]
    fn bom_is_tolerated() {
        let dir = tempdir().expect("tempdir");
        let body = "\u{feff}[emby]\nplayer = \"mpv\"\n".to_string();
        write_config(dir.path(), "embyToLocalPlayer.toml", &body);
        let cfg = Config::load_from_dir(dir.path()).expect("load bom");
        assert_eq!(cfg.emby.player, "mpv");
    }

    #[test]
    fn existing_config_path_none_when_absent() {
        let dir = tempdir().expect("tempdir");
        assert_eq!(existing_config_path(dir.path()), None);
    }

    #[test]
    fn existing_config_path_follows_search_order() {
        let dir = tempdir().expect("tempdir");
        // Only a legacy file exists: it must be resolved (not config.toml).
        write_config(dir.path(), "embyToLocalPlayer.toml", "[emby]\n");
        assert_eq!(
            existing_config_path(dir.path()),
            Some(dir.path().join("embyToLocalPlayer.toml")),
            "legacy config must be found when it is the only one present"
        );
        // Once config.toml exists it takes precedence, matching load_from_dir.
        write_config(dir.path(), "config.toml", "[emby]\n");
        assert_eq!(
            existing_config_path(dir.path()),
            Some(dir.path().join("config.toml")),
            "config.toml must win to mirror the load search order"
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
        write_config(dir.path(), "embyToLocalPlayer.toml", body);
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

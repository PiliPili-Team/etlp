# etlp

A Rust rewrite of [embyToLocalPlayer](https://github.com/kjtsune/embyToLocalPlayer) — a
lightweight local HTTP service that bridges an Emby / Jellyfin / Plex browser userscript to
a local media player, with playlist management, progress write-back, and optional Trakt /
Bangumi sync.

## Why Rust

| Original (Python) | This rewrite |
|---|---|
| Requires a Python runtime | Single static binary, zero runtime deps |
| `TypeError` / `KeyError` crashes in production | Type-safe, no unchecked indexing or unwraps |
| Shared mutable state guarded by convention | Explicit `Arc<RwLock<…>>` / `AtomicBool` / `Mutex` |
| Tested on one platform | CI cross-compiles for Linux, macOS, Windows |

## Requirements

| Component | Minimum |
|---|---|
| Rust toolchain | **1.89** (see `rust-toolchain.toml`) |
| Media player | one of: **mpv** · iina · vlc · mpc-hc · potplayer · dandanplay |
| OS | Linux · macOS · Windows |

No other system libraries are required. The release binaries are fully statically linked on
Linux (musl) and have no extra dependencies on macOS and Windows.

## Installation

### From pre-built release

Download the archive for your platform from the [Releases](../../releases) page, extract
it, and place the `etlp` binary somewhere on your `PATH`.

### Build from source

```bash
git clone https://github.com/PiliPili-Team/etlp
cd etlp
cargo build --release
# binary: target/release/etlp
```

## Configuration

`etlp` looks for `embyToLocalPlayer.toml` in the current working directory when launched.
Create the file by copying and editing the template below.

```toml
[emby]
# Name or full path of your media player executable.
# One of: mpv, iina, vlc, mpc-hc, potplayer, dandanplay
player = "mpv"

# Start the player in fullscreen mode.
# fullscreen = true

# Force a subtitle track by display-title keyword (case-insensitive substring match).
# subtitle_priority = "chs,简体,中文"

# Maximum number of playlist episodes per session.
# item_limit = 10

[dev]
# Skip TLS certificate verification for self-signed Emby servers.
# skip_certificate_verify = false

# Override the player executable path (e.g. "/usr/local/bin/mpv").
# player_path = ""

# HTTP proxy for media stream requests (e.g. "http://127.0.0.1:7890").
# http_proxy = ""

# Hosts whose redirects should be followed and cached.
# redirect_check_host = ""

[trakt]
# Enable Trakt sync for this Emby host (substring match against the server URL).
# enable_host = "emby.example.com"
# client_id = ""
# client_secret = ""

[bangumi]
# Bearer token from https://bgm.tv/dev/app
# access_token = ""

[dandan]
# DandanPlay API port (Windows only).
# port = 9001
# api_key = ""
```

### Path translation

To remap network paths to local mount points (useful with mount-disk mode), add pairs to
`[path_map]`:

```toml
[path_map]
# network prefix = local prefix
"/mnt/share" = "/Volumes/nas"
```

## Running

```bash
# Start the server on http://127.0.0.1:58000 (default)
./etlp

# Authenticate with Trakt via device-flow (run once)
./etlp trakt-auth

# Mark a Bangumi episode as watched
./etlp bgm-mark-played <subject_id> <episode_sort>
```

The userscript on the browser then posts playback events to `http://127.0.0.1:58000`.
Install the original userscript from the
[embyToLocalPlayer repository](https://github.com/kjtsune/embyToLocalPlayer) — it is
compatible with this server.

## Platform support

| Platform | Architecture | Notes |
|---|---|---|
| Linux | x86-64, aarch64 | Static musl binary; no system deps |
| macOS | x86-64 (Intel), aarch64 (Apple Silicon) | Requires mpv / iina in PATH |
| Windows | x86-64 | mpc-hc / potplayer / dandanplay supported |

## Development

```bash
# Format
cargo fmt --all

# Lint (warnings are errors)
cargo clippy --workspace --all-targets -- -D warnings

# Test (307 tests across 20 suites)
cargo test --workspace
```

## Workspace layout

| Crate | Responsibility |
|---|---|
| `etlp-core` | Domain types (`PlaybackData`, `Server`, `PlayerKind`) and trait contracts |
| `etlp-config` | TOML/INI config loading, hot-reload, string-match rules |
| `etlp-logging` | `tracing` setup with secret masking and 10 MB rotating file output |
| `etlp-net` | `reqwest`/`rustls` HTTP client, redirect cache, progress write-back |
| `etlp-media-server` | Emby/Jellyfin/Plex API clients, payload parsing, version selection |
| `etlp-player` | mpv JSON IPC, VLC/MPC/PotPlayer/DandanPlay launchers, playlist mgmt |
| `etlp-download` | Concurrent download manager with file locking and resume |
| `etlp-sync` | Trakt OAuth (device flow + auth code) and Bangumi API |
| `etlp-server` | axum HTTP server, `AppState`, all route handlers |
| `etlp` | Binary entry point and `clap` CLI |

## License

[GNU General Public License v3.0](LICENSE)

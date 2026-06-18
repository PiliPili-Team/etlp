# etlp

[![CI Rust](https://github.com/PiliPili-Team/etlp/actions/workflows/ci-rust.yml/badge.svg)](https://github.com/PiliPili-Team/etlp/actions/workflows/ci-rust.yml)
[![CI App](https://github.com/PiliPili-Team/etlp/actions/workflows/ci-app.yml/badge.svg)](https://github.com/PiliPili-Team/etlp/actions/workflows/ci-app.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey)](https://github.com/PiliPili-Team/etlp/releases)

**etlp** is a Genshin-powered media-player bridge, primarily for Emby.
It runs a lightweight local HTTP service that receives playback requests from a
browser userscript and dispatches them to a local media player — handling
playlist construction, progress write-back, and optional watch-history sync.

**Key features**

- Supports **mpv · iina · vlc · mpc-hc · potplayer · dandanplay**
- Playlist management with version/subtitle preference filtering
- Progress write-back to Emby / Jellyfin / Plex
- Trakt.tv and Bangumi.tv watch-history sync
- Concurrent download manager with pause / resume and rate limiting
- Native GUI for macOS (Tauri, vibrancy)
- Single static binary — zero runtime dependencies on every platform

---

## Requirements

| Component    | Minimum                                                           |
| ------------ | ----------------------------------------------------------------- |
| Media player | one of: **mpv** · iina · vlc · mpc-hc · potplayer · dandanplay   |
| OS           | Linux · macOS · Windows                                           |
| Rust (build) | **1.89** (see `rust-toolchain.toml`)                              |

Release binaries are fully statically linked on Linux (musl) and have no
extra system dependencies on macOS and Windows.

---

## Installation

### Pre-built binary

Download the archive for your platform from the
[Releases](https://github.com/PiliPili-Team/etlp/releases) page, extract it,
and place the `etlp` binary somewhere on your `PATH`.

### Build from source

```bash
git clone https://github.com/PiliPili-Team/etlp
cd etlp
cargo build --release
# binary: target/release/etlp
```

---

## Quick start

```bash
# Start the HTTP server (default: http://127.0.0.1:58000)
./etlp

# One-time Trakt device-flow authentication
./etlp trakt-auth

# Mark a Bangumi episode as watched
./etlp bgm-mark-played <subject_id> <episode_sort>
```

Install the browser userscript from the
[embyToLocalPlayer repository](https://github.com/kjtsune/embyToLocalPlayer)
— it is fully compatible with this server.

Configuration is read from `embyToLocalPlayer.toml` in the working directory.
See the [Wiki](https://github.com/PiliPili-Team/etlp/wiki) for the full
configuration reference and advanced settings.

---

## Platform support

| Platform | Architecture          | Notes                                          |
| -------- | --------------------- | ---------------------------------------------- |
| Linux    | x86-64 · aarch64      | Static musl binary, no system deps             |
| macOS    | x86-64 · aarch64 (AS) | mpv / iina must be in PATH                     |
| Windows  | x86-64                | mpc-hc / potplayer / dandanplay supported      |

---

## Development

```bash
# Format
cargo fmt --all

# Lint (warnings are errors)
cargo clippy --workspace --all-targets --frozen -- -D warnings

# Test (316 tests across 20 suites)
cargo test --workspace --frozen
```

---

## Workspace layout

| Crate              | Responsibility                                               |
| ------------------ | ------------------------------------------------------------ |
| `etlp-core`        | Domain types (`PlaybackData`, `Server`, `PlayerKind`) and trait contracts |
| `etlp-config`      | TOML config loading, hot-reload, string-match rules          |
| `etlp-logging`     | `tracing` setup, secret masking, 10 MB rotating file output  |
| `etlp-net`         | `reqwest`/`rustls` HTTP client, redirect cache, progress write-back |
| `etlp-media-server`| Emby/Jellyfin/Plex API clients, payload parsing, version selection |
| `etlp-player`      | mpv JSON IPC, VLC/MPC/PotPlayer/DandanPlay launchers, playlist management |
| `etlp-download`    | Concurrent download manager with file locking and resume     |
| `etlp-sync`        | Trakt OAuth (device flow + auth code) and Bangumi API        |
| `etlp-server`      | axum HTTP server, `AppState`, all route handlers             |
| `etlp`             | Binary entry point and `clap` CLI                            |

---

## Acknowledgements

This project was inspired by
[embyToLocalPlayer](https://github.com/kjtsune/embyToLocalPlayer) by
[@kjtsune](https://github.com/kjtsune). The browser userscript and the
Emby/Jellyfin communication protocol originate from that project.

---

## License

[GNU General Public License v3.0](LICENSE)

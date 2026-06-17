# etlp (embyToLocalPlayer, Rust)

A Rust rewrite of [`embyToLocalPlayer`](https://github.com/kjtsune/embyToLocalPlayer):
a local HTTP service that bridges a browser userscript (Emby / Jellyfin / Plex
web) to local media players (mpv / iina / vlc / mpc-hc / potplayer /
dandanplay), managing playlists, progress write-back, and optional Trakt /
Bangumi sync.

> Status: **work in progress.** See [`docs/PLAN.md`](docs/PLAN.md),
> [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/TODO.md`](docs/TODO.md).

## Why rewrite

- A single static binary per platform — no Python runtime, no extra libs.
- Explicit error handling (no crash-prone APIs), strong typing, and tested
  concurrency around the shared playback/download state.

## Workspace layout

| Crate | Responsibility |
|---|---|
| `etlp-core` | Domain types and trait contracts (no IO) |
| `etlp-config` | INI config loading and string-match rules |
| `etlp-logging` | tracing setup with secret masking |
| `etlp-net` | HTTP client, redirect cache, progress write-back |
| `etlp-media-server` | Emby/Plex clients and payload parsing |
| `etlp-player` | Player launchers + mpv JSON IPC + playlist mgmt |
| `etlp-download` | Download/cache manager with file locking |
| `etlp-sync` | Trakt / Bangumi sync |
| `etlp-server` | axum HTTP server and routing |
| `etlp` | Binary entry point |

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Licensed under MIT.

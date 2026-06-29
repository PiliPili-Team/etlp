# etlp Userscript

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](../../LICENSE)
[![Linux](https://img.shields.io/badge/Linux-FCC624?logo=linux&logoColor=black)](https://github.com/PiliPili-Team/etlp/releases)
[![macOS](https://img.shields.io/badge/macOS-000000?logo=apple&logoColor=white)](https://github.com/PiliPili-Team/etlp/releases)
[![Windows](https://img.shields.io/badge/Windows-0078D4?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9Ii0yIC0yIDI4IDI4IiBmaWxsPSJ3aGl0ZSI%2BPHBhdGggZD0iTTAgMy40NDkgOS43NSAyLjF2OS40NTFIMG0xMC45NDktOS42MDJMMjQgMHYxMS40SDEwLjk0OU0wIDEyLjZoOS43NXY5LjQ1MUwwIDIwLjY5OU0xMC45NDkgMTIuNkgyNFYyNGwtMTIuOS0xLjgwMSIvPjwvc3ZnPg==&logoColor=white)](https://github.com/PiliPili-Team/etlp/releases)

GitHub: <https://github.com/PiliPili-Team/etlp>

**etlp** is a Genshin-powered media-player bridge, primarily for Emby.
It runs a lightweight local HTTP service that receives playback requests from a
browser userscript and dispatches them to a local media player, handling
playlist construction, progress write-back, and optional watch-history sync.

## Key Features

- Supports **mpv · iina · vlc · mpc-hc · potplayer · dandanplay**
- Playlist management with version/subtitle preference filtering
- Progress write-back to Emby (Jellyfin: **experimental, untested**)
- Trakt.tv and Bangumi.tv watch-history sync
- Concurrent download manager with pause / resume and rate limiting
- Native GUI for macOS and Windows (Tauri, vibrancy on macOS)

## Userscript Features

- Uses `/etlp` first and falls back to legacy `/embyToLocalPlayer`.
- Keeps Plex compatible through `/plexToLocalPlayer`.
- Lets you configure the local service port from the userscript menu.
- Shows localized in-page notifications instead of browser alerts.
- Avoids the Emby incompatible-stream flicker by intercepting playback early.

> [!IMPORTANT]
> The Genshin-themed icon is copyrighted by miHoYo. It is used only because
> we love the work. If this usage infringes any rights, it will be removed
> immediately.

## Install the Companion App

1. Open the release page:
   <https://github.com/PiliPili-Team/etlp/releases>
2. Download the package for your platform.
3. Install and start the app.
4. Keep the userscript port the same as the app service port.

Recommended packages:

- macOS: download the `.dmg`, then drag the app into `Applications`.
- Windows: download the `.msi` or `.exe`, then run the installer.
- Windows Portable: download the `.zip`, extract it, then run `Genshin.exe`.

The default service port is `58000`.

## First-Launch Security Prompts

The release builds are not code-signed with paid Apple/Microsoft certificates,
so macOS or Windows may show generic security prompts on first launch. This is
expected for unsigned builds.

### macOS: app is damaged or cannot be opened

If macOS says `Genshin` is damaged or cannot be opened, remove the quarantine
attribute once and reopen the app:

```bash
sudo xattr -dr com.apple.quarantine /Applications/Genshin.app
```

### Windows: Defender, firewall, or antivirus prompt

Allow the app through Windows Firewall and add it to your antivirus allow-list
if needed. For SmartScreen, choose **More info** and then **Run anyway**.

### Windows Portable: cannot run from `C:\` or `Program Files`

The portable build stores `config/` and `data/` next to the executable. Drive
roots, `C:\Program Files\`, and folders with restrictive NTFS permissions may
require administrator rights. Prefer a user-writable directory such as:

```text
C:\Users\<you>\Apps\etlp\
```

If the app asks for UAC elevation, allow it or move the portable folder to a
location where your user account has write permission.

## More Guidance

full troubleshooting, read the main README:

<https://github.com/PiliPili-Team/etlp/blob/main/README.md>

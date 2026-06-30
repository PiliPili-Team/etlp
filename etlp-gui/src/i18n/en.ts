import { zhCN } from "./zh-CN";

export const en: typeof zhCN = {
    ...zhCN,

    // App
    app_name: "Genshin",

    // Nav
    nav_overview: "Overview",
    nav_player: "Player",
    nav_version_prefer: "Version",
    nav_network: "Network",
    nav_config: "Config",
    nav_system: "System",
    nav_logs: "Logs",
    nav_sec_play: "Playback",
    nav_sec_settings: "Settings",
    nav_sec_sync: "Sync",
    nav_bangumi: "Bangumi",
    nav_trakt: "Trakt",
    nav_sec_debug: "Debug",
    nav_download: "Download",

    // Common
    add: "Add",
    add_placeholder: "Type then press Enter to add",
    open_dir: "Open Folder",
    loading: "Loading config…",

    // Overview
    page_overview: "Overview",
    ov_service: "Local Service",
    ov_running: "Running",
    ov_stopped: "Stopped",
    ov_port: "Port",
    ov_port_desc: "Local listen address",
    ov_uptime: "Uptime",
    ov_uptime_desc: "Since service started",
    ov_address: "Address",
    ov_address_desc: "Localhost only",
    ov_config: "Settings",
    ov_config_file: "Config File",
    ov_config_file_desc: "View or open in an external editor",
    ov_edit_config: "Edit Config",
    ov_restart: "Restart Service",
    ov_restart_desc: "Stop service, flush resources, then restart with the latest config",
    ov_about: "About",
    ov_about_desc: "Version info and open-source credits",
    ov_view: "View",
    ov_start: "Start",
    ov_stop: "Stop",

    // Toasts
    toast_started: "Service started on port {port}",
    toast_stopped: "Service stopped",
    toast_restarted: "Service restarted on port {port}",
    toast_start_failed: "Failed to start service",
    toast_stop_failed: "Failed to stop service",
    toast_restart_failed: "Failed to restart service",
    toast_open_failed: "Failed to open",
    sync_not_configured: "Not configured yet — fill in the fields first",

    // Player
    page_player: "Player",
    pl_type: "Player Type",
    pl_type_desc: "Choose a local media player",
    pl_startup: "Launch Options",
    pl_fullscreen: "Fullscreen",
    pl_fullscreen_desc: "Start the player in fullscreen mode",
    pl_mute: "Start Muted",
    pl_mute_desc: "Launch muted (mpv --mute=yes; press m in player to unmute)",
    pl_pretty_title: "Pretty Title",
    pl_pretty_title_desc: "Prepend server name to the player window title",
    pl_kill_start: "Kill on Startup",
    pl_kill_start_desc: "Clear port usage and kill existing player processes on startup",
    pl_path: "Player Path",
    pl_path_desc: "Optional — leave empty to use the player from system PATH",
    pl_browse: "Browse…",
    pl_path_error: "Path not found — please check the input",
    pl_progress_support:
        "Progress reporting: mpv / IINA are fully supported — live updates while playing, resume position written back on exit, watched marking, Trakt / Bangumi sync and per-episode tracking. Other players only write the final position and sync on exit, with no live reporting during playback; VLC plays the whole season continuously, MPC and dandanplay are single-episode, and PotPlayer position read-back is Windows-only",

    // Version prefer
    page_vp: "Version Preference",
    vp_priority: "Version Priority Order",
    vp_keywords: "Version Labels",
    vp_keywords_desc:
        'When multiple files exist for the same episode, the one whose path matches the earliest label in this list wins. Example: "TeamX → GroupA → StreamB" — if all three versions are available, TeamX is chosen; if not, GroupA; and so on',
    vp_keywords_placeholder: "e.g. TeamX, GroupA, StreamB",
    vp_playlist: "Apply to Playlist",
    vp_playlist_desc: "Use version priority when building the playlist",
    vp_subtitle: "Subtitle Preference",
    vp_sub_priority: "Subtitle Priority",
    vp_sub_priority_desc: "Match subtitle track keywords in order",
    vp_sub_priority_placeholder: "e.g. Simplified, CHS",
    vp_sub_extract: "Cross-version Subtitle Extract",
    vp_sub_extract_desc:
        "Extract subtitles from other versions when none found in current",
    vp_sub_extract_placeholder: "e.g. CHS, Simplified",
    vp_limits: "Playlist Limits",
    vp_max_eps: "Max Episodes per Session",
    vp_max_eps_desc:
        "Episodes are truncated once this limit is reached; 0 or empty means " +
        "unlimited (recommended: 10–100)",
    vp_last_ep: "Disable at Last Episode",
    vp_last_ep_desc:
        "On: when playing the season's last episode, build no playlist and open only that episode (nothing follows it); Off: always build the playlist (current + later episodes)",
    vp_filter: "Version Fingerprint",
    vp_filter_desc:
        'Extracts version tokens from the currently playing file\'s path as a "fingerprint". Only episodes whose paths match the exact same set of tokens are added to the playlist, locking the whole season to the same version. Example: if the regex matches "TeamX" and "1080p" in the current file, only episodes containing both tokens are included (leave empty to disable)',
    vp_filter_placeholder: "e.g. |TeamX|1080p|CHS",
    vp_filter_valid: "Valid regex",
    vp_filter_invalid: "Invalid regex",

    // Network
    page_network: "Network",
    net_proxy_http: "HTTP Proxy",
    net_proxy_https: "HTTPS Proxy",
    net_proxy_socks5: "SOCKS5 Proxy",
    net_proxy_desc:
        "Host:port only (e.g. 127.0.0.1:6152); paste a full URL to auto-detect the scheme; leave empty to disable",
    net_proxy_https_desc:
        "Used for encrypted (HTTPS) connections; falls back to HTTP proxy if empty; same format as HTTP",
    net_proxy_socks5_desc:
        "Proxies all protocol traffic; ideal for networks without an HTTP tunnel; leave empty to disable",
    net_proxy_enabled: "Enable Proxy",
    net_proxy_enabled_desc:
        "When enabled: mpv HTTP/HTTPS traffic uses the HTTP proxy; Bangumi / Trakt / TMDB API requests use the matching-protocol proxy; private IPs (192.168.x, 10.x, 172.16–31.x, etc.) are always direct — no need to disable for LAN servers. When off, all connections are direct.",
    net_skip_tls: "Skip TLS Verification",
    net_skip_tls_desc: "For self-signed media servers — insecure",
    net_redirect: "Redirect Detection",
    net_redirect_hosts: "Hosts to Probe for Redirects",
    net_redirect_hosts_desc:
        "Stream URLs for these hosts are probed for 30x redirects before handing off to the player (empty by default)",

    // System
    page_system: "System",
    sys_appearance: "Appearance",
    sys_theme: "Theme",
    sys_theme_desc: "Light, dark, or follow the system",
    sys_lang: "Language",
    sys_lang_desc: "UI display language",
    sys_theme_system: "System",
    sys_theme_light: "Light",
    sys_theme_dark: "Dark",
    sys_lang_system: "System",
    sys_liquid_glass: "Liquid Glass (Experimental)",
    sys_liquid_glass_desc:
        "Use the macOS 26 Liquid Glass window material; restart the app to apply changes",
    sys_liquid_glass_unavailable:
        "Liquid Glass is not supported on this system, so this option is disabled",
    sys_display: "Display",
    sys_font_size: "Font Size",
    sys_font_size_desc: "Adjust the UI text size",
    sys_zoom: "UI Scale",
    sys_zoom_desc: "HiDPI / high-res overall zoom — current DPR: {dpr}",
    sys_font: "UI Font",
    sys_font_desc: "Choose the interface font",
    sys_font_default: "Default (system-ui)",
    sys_startup: "Startup",
    sys_autostart: "Launch at Login",
    sys_autostart_desc: "Automatically start the app after logging in",
    sys_silent_start: "Silent Start",
    sys_silent_start_desc:
        "Start hidden in the tray without showing the main window (quieter with launch-at-login)",
    sys_service: "Local Service",
    sys_listen_port: "Listen Port",
    sys_listen_port_desc:
        "Port used by the browser userscript to reach the local service. Changing it restarts the service automatically and must match the userscript port.",
    sys_listen_port_invalid: "Port must be between 1 and 65535; corrected automatically",
    sys_logs_title: "Logs",
    sys_log_level: "Log Level",
    sys_log_level_desc: "Set to Debug for more verbose output when troubleshooting",
    sys_log_max_size: "Max Log Size (MB)",
    sys_log_max_size_desc:
        "Rotate to a new file once the current one exceeds this size (20–200 MB)",
    sys_log_max_size_capped: "Capped at the 200 MB maximum",
    sys_log_max_size_floored: "Raised to the 20 MB minimum",
    sys_log_max_files: "Max Log Files",
    sys_log_max_files_desc:
        "Number of rotated log files to keep (1–14); the oldest is removed",
    sys_log_max_files_capped: "Capped at the 14-file maximum",
    sys_log_mask: "Mask Sensitive Tokens",
    sys_log_mask_desc: "Replace sensitive text in logs with placeholders",
    sys_cache: "Cache",
    sys_cache_size: "Current Cache Size",
    sys_cache_size_desc: "Disk space used by logs and other runtime cache",
    sys_cache_clear: "Clear Cache",
    sys_cache_clear_desc: "Empty the log files to free disk space",
    cache_confirm_title: "Clear Cache",
    cache_confirm_message:
        "The service must be stopped before clearing the cache, otherwise logs being written may end up inconsistent. Confirm the service is stopped and proceed?",
    cache_confirm_ok: "Clear",
    cache_confirm_cancel: "Cancel",
    cache_stop_first: "Stop the service before clearing the cache",
    cache_cleared: "Cache cleared, freed {size}",
    sys_general: "General",
    sys_about: "About",
    sys_about_desc: "Version info and open-source credits",
    sys_download: "Downloads",
    sys_speed_limit: "Speed Limit (MiB/s)",
    sys_speed_limit_desc:
        "Caps the bandwidth used by downloads and preload caching (MiB/s); 0 = unlimited",
    sys_download_note:
        'Preload and download mode are triggered by the browser userscript\'s commands, not toggled here: the script\'s "cache while playing" is preload, and "download only" is download mode; download mode also requires your media server account to permit resource downloads',
    sys_trakt: "Trakt.tv Scrobbling",
    sys_trakt_sync_note:
        'When playback ends, your viewing is synced to Trakt automatically: reaching about 80% or more marks the episode watched, below that it stays unmarked; other episodes of the same season you already finished in your media server are marked too, without duplicating ones already there. Below 80% your position is remembered so you can pick up later, and the next episode shows under Continue Watching; re-watching the same episode records it again — whether a short time gap is allowed is controlled by the "allow duplicate" switch below.',
    sys_trakt_dashboard: "Open Trakt dashboard",
    sys_trakt_enabled: "Enable Trakt Sync",
    sys_trakt_enabled_desc: "When off, no viewing data is synced to Trakt",
    sys_trakt_setup_title: "Setup",
    sys_trakt_setup_step1: "1. Create an app on Trakt: ",
    sys_trakt_setup_link: "trakt.tv/oauth/applications",
    sys_trakt_setup_step2: '2. Set the app\'s "Redirect uri" to the address below:',
    sys_trakt_setup_copy: "Copy",
    sys_trakt_setup_copied: "Redirect URI copied",
    sys_trakt_setup_copy_failed: "Copy failed — please select and copy manually",
    sys_trakt_id: "Client ID",
    sys_trakt_id_desc:
        "Obtained after creating an app on trakt.tv — leave empty to disable",
    sys_trakt_id_placeholder: "Leave empty to disable Trakt",
    sys_trakt_secret: "Client Secret",
    sys_trakt_secret_desc:
        "Obtained after creating an app on trakt.tv — leave empty to disable",
    sys_trakt_secret_placeholder: "Leave empty to disable Trakt",
    sys_trakt_user: "Username",
    sys_trakt_user_desc: "Your Trakt username (not the display nickname)",
    sys_trakt_user_placeholder: "e.g. your_trakt_user",
    sys_trakt_host: "Enable Host",
    sys_trakt_host_desc:
        'Comma-separated host keywords; leave empty to disable. e.g. emby.local, 192.168.1 — enter "." to enable all',
    sys_trakt_host_placeholder: "e.g. localhost, 192.168., emby.example.com",
    sys_trakt_dup: "Allow Duplicate Marking",
    sys_trakt_dup_desc:
        "When on, every completion re-marks the same episode/movie; when off, throttled de-duplication applies: the same item finished again within the throttle window set below is marked only once (back-filled earlier episodes are always de-duplicated regardless)",
    sys_trakt_dup_throttle: "Duplicate-Mark Throttle (seconds)",
    sys_trakt_dup_throttle_desc:
        'Effective when "Allow Duplicate Marking" is off: the same item finished again within this many seconds is recorded only once. Minimum 120s',
    sys_trakt_dup_throttle_floored:
        "Throttle cannot be below 120 seconds; corrected to 120",
    sys_bangumi: "Bangumi.tv Tracking",
    sys_bangumi_sync_note:
        "When playback ends, your viewing is synced to Bangumi automatically: ≥ 80% progress marks the current episode watched; below that the current episode is not marked, though any earlier episodes you already finished in your media server are still backfilled.",
    sys_bangumi_enabled: "Enable Bangumi Sync",
    sys_bangumi_enabled_desc: "When off, no viewing data is synced to Bangumi",
    sys_bangumi_host: "Enable Host",
    sys_bangumi_host_desc:
        'Comma-separated host keywords; sync is enabled when any keyword matches. Leave empty to disable sync for all servers. To enable for every server, type "." in the input box and click Add.',
    sys_bangumi_host_placeholder: "e.g. localhost, 192.168., emby.example.com",
    sys_bangumi_user: "Username / UID",
    sys_bangumi_user_desc: "bgm.tv username or the digits in bgm.tv/user/123456",
    sys_bangumi_user_placeholder: "e.g. 123456",
    sys_bangumi_token: "Access Token",
    sys_bangumi_token_desc:
        "Generated at next.bgm.tv/demo/access-token — leave empty to disable",
    sys_bangumi_token_placeholder: "Leave empty to disable Bangumi",
    sys_bangumi_private: "Private Collection",
    sys_bangumi_private_desc: "Hide newly synced entries from your public profile",
    sys_bangumi_genres: "Genre Filter",
    sys_bangumi_genres_desc:
        "Regex matched against series genres; only matching series sync",
    sys_bangumi_genres_placeholder: "动画|anime",
    sys_bangumi_map: "ID Mapping",
    sys_bangumi_map_desc:
        "Pin a tmdb/imdb/tvdb show or movie to an exact Bangumi subject; takes top priority. Three season formats: whole season (S4), closed episode range (S5E106~S5E157, episodes 106–157 only), open range (S5E158++, episode 158 onwards). The RHS E±N shifts the local episode index to the Bangumi sort number. Examples: tmdb:10000|type:tv|S4 -> bgm:20000|E+59; tmdb:10000|type:tv|S5E1~S5E50 -> bgm:20001; tmdb:10000|type:tv|S5E51++ -> bgm:20002; tmdb:10001|type:movie -> bgm:30000. If type is omitted it is inferred from the presence of a season.",
    map_placeholder: "tmdb:10000|type:tv|S4 -> bgm:20000|E+59",
    map_check: "Check & Add",
    map_remove: "Remove",
    map_copy: "Copy",
    map_group_add: "New Group",
    map_group_name_placeholder: "Group name",
    map_group_add_confirm: "Create",
    map_group_delete: "Delete Group",
    map_group_delete_confirm: 'Delete group "{name}" and all its mappings?',
    map_item_delete_title: "Remove Mapping",
    map_item_delete_confirm: "Remove this entry?\n{entry}",
    map_group_default_label: "Default",
    map_export: "Export",
    map_export_done: "Mappings exported",
    map_import: "Import",
    map_import_prefer: "Prefer imported (overwrite local conflicts)",
    map_import_done: "Import done: {added} added, {replaced} replaced",
    map_import_url: "Import from URL",
    map_import_url_placeholder: "https://example.com/bangumi_map.json",
    map_import_url_confirm: "Import",
    cfg_backup_busy: "Backing up…",
    cfg_importing: "Importing…",
    bgm_auto_mark_subject_watched: "Auto Mark Watched",
    bgm_auto_mark_subject_watched_desc:
        "Automatically mark the entire subject as Watched when all its main episodes are marked as watched",
    bgm_history_follow_media_server: "History Follows Media Server",
    bgm_history_follow_media_server_desc:
        "When a media-server season maps to several Bangumi collections, also backfill the earlier collections the server reports as played. When off, only the collection containing the episode you're watching is backfilled.",
    bgm_mark_watching: "Mark as Watching",
    bgm_mark_watching_desc:
        "When on, a partially-watched episode still marks the subject as watching; when off, only a fully watched episode updates the collection status",
    map_err_empty: "Enter a mapping",
    map_err_format: "Malformed — expected “LHS -> RHS”",
    map_err_provider: "Unknown source; only tmdb / imdb / tvdb are supported",
    map_err_provider_id: "Bad ID (tmdb/tvdb numeric, imdb starts with tt)",
    map_err_type: "type must be tv or movie",
    map_err_season: "Bad season; expected a positive integer like S4",
    map_err_ep_range:
        "Bad episode range; use S5E106~S5E157 (closed) or S5E158++ (open), and start must not exceed end",
    map_err_subject: "Bad Bangumi subject ID; expected a positive integer",
    map_err_offset: "Bad episode offset; expected an integer like E+59 or E-3",
    map_err_movie_season: "A movie cannot carry a season or episode offset",
    map_err_duplicate: "An identical mapping already exists",
    sync_refresh: "Refresh authorization",
    sync_refreshing: "Refreshing…",
    sync_authorize_opened: "Authorization page opened — finish it in your browser",
    sync_auth_valid: "Authorization is valid",
    sync_start_service_first: "Start the service first",
    sync_refresh_confirm_title: "Refresh Authorization",
    sync_refresh_confirm_message:
        "Manually refresh the authorization now? If the current token is invalid, the authorization page will open in your browser.",
    sync_refresh_confirm_ok: "Refresh",
    sync_test: "Check Authorization",
    sync_test_desc: "Check whether the current credentials work",
    sync_testing: "Checking…",
    sync_test_ok: "Authorization works",
    sync_test_fail:
        "Authorization failed — the config may be wrong or not yet authorized. Click “Refresh authorization” at the top right.",
    sync_incomplete:
        "Configuration is incomplete — fill in the required fields before checking",

    // Config tab (config file + backup / restore / reset / update)
    page_config: "Config",
    cfg_file_title: "Config File",
    cfg_backup_title: "Backup & Restore",
    cfg_backup_now: "Back Up Now",
    cfg_backup_now_desc: "Package the current config into a timestamped zip backup",
    cfg_backup_done: "Config backed up",
    cfg_backup_list: "Backups",
    cfg_backup_list_desc: "Up to 5 backups are kept — {count} now",
    cfg_backup_empty: "No backups yet",
    cfg_view: "View",
    cfg_restore: "Restore",
    cfg_delete: "Delete",
    cfg_import: "Import Backup",
    cfg_import_desc: "Import and restore the config from an external zip file",
    cfg_restore_done: "Config restored",
    cfg_restore_confirm_title: "Restore Config",
    cfg_restore_confirm_message:
        "Overwrite the current config with backup “{name}”? This cannot be undone.",
    cfg_restore_progress_title: "Restoring config",
    cfg_restore_progress_message:
        "The config is being written and applied to the local service. Keep the app open until this finishes.",
    cfg_restore_progress_label: "Applying config {progress}%",
    cfg_restore_progress_waiting: "Finishing config application…",
    cfg_restore_failed_title: "Restore failed",
    cfg_restore_error_ack: "OK",
    cfg_restore_error_read:
        "The backup file could not be read. Check that it still exists and is accessible.",
    cfg_restore_error_zip:
        "The backup file could not be parsed. Choose a valid config backup zip.",
    cfg_restore_error_missing_config:
        "The backup is missing config content. Choose the correct backup file.",
    cfg_restore_error_invalid_config:
        "The backup config format is invalid. Your current config was not overwritten.",
    cfg_restore_error_write:
        "The config could not be written. Check disk space and file permissions, then try again.",
    cfg_restore_error_apply:
        "The config was written, but could not be applied to the local service. Restart the app and check the config.",
    cfg_restore_error_internal:
        "The restore task stopped unexpectedly. Restart the app and try again.",
    cfg_restore_error_generic: "The config could not be restored. Try again later.",
    cfg_import_confirm_title: "Import & Restore Config",
    cfg_import_confirm_message:
        "Import this backup and overwrite the current config? This cannot be undone.",
    cfg_delete_confirm_title: "Delete Backup",
    cfg_delete_confirm_message: "Delete backup “{name}”?",
    cfg_reset_title: "Reset",
    cfg_reset: "Reset to Defaults",
    cfg_reset_desc: "Restore all settings to their default values",
    cfg_reset_done: "Config reset to defaults",
    cfg_reset_confirm_title: "Reset Config",
    cfg_reset_confirm_message:
        "Reset to the default config? The current config will be overwritten — this cannot be undone.",
    cfg_update_title: "Update",
    cfg_update_auto: "Auto-check for updates",
    cfg_update_auto_desc:
        "Check GitHub for new releases on startup and show a hint on the overview",
    cfg_update_check: "Check Now",
    cfg_update_check_desc: "Check GitHub for a newer version right now",
    cfg_update_checking: "Checking…",
    cfg_update_available: "New version v{version} available",
    cfg_update_latest: "You are on the latest version v{version}",
    cfg_update_current_ver: "Current version",
    cfg_update_latest_ver: "Latest version",
    cfg_update_up_to_date: "Up to date",
    cfg_update_install: "Download & Install",

    // Update banner (overview)
    ov_update_available: "New version v{version} available",
    ov_update_action: "Install Update",
    ov_update_dismiss: "Dismiss this version",
    ov_update_downloading: "Downloading update…",
    ov_update_failed: "Update failed",
    ov_update_extracting: "Extracting update…",
    ov_update_installing: "Installing the new version…",
    sys_privacy: "Privacy",
    sys_no_progress: "Disable Progress Reporting",
    sys_no_progress_desc: "Do not report playback progress to the Emby/Jellyfin server",
    sys_accent: "Accent Color",
    sys_accent_desc: "UI highlight color — affects buttons, active states, and badges",
    sys_center_nav: "Center Sidebar",
    sys_center_nav_desc: "Vertically center the sidebar tabs as a group",

    // Log levels
    log_error: "Error — crashes only",
    log_warn: "Warn — abnormal conditions",
    log_info: "Info — default, everyday operation",
    log_debug: "Debug — troubleshooting",
    log_trace: "Trace — full detail",

    // Logs page
    page_logs: "Logs",
    logs_app: "App Log",
    logs_mpv: "mpv Log",
    logs_filter: "Filter…",
    logs_clear: "Clear",
    logs_bottom: "↓ Bottom",
    logs_empty: "Waiting for log output…",
    logs_no_mpv: "No mpv log found — click “Choose mpv Log” to load one",
    logs_lines: "lines",
    logs_loading_older: "Loading older logs…",
    logs_scroll_older: "Scroll up to load older logs",
    logs_open_folder: "Open Log Folder",
    logs_pick_mpv: "Choose mpv Log",
    logs_reset_mpv: "Reset to Default",
    logs_reset_mpv_title: "Switch back to the default mpv log in the log folder",
    logs_anon: "Anonymous",
    logs_anon_title:
        "Hide device id, tokens, IPs, user id, URL host and Bangumi / Trakt usernames in the view only, handy for sharing screenshots; the log file is untouched — file redaction still follows the “Sensitive text” switch",

    // About modal
    about_thanks: "Credits",
    about_thanks_desc: "for endless inspiration",
    about_version_label: "Version",

    // Autostart toasts
    autostart_on: "Launch at login enabled",
    autostart_off: "Launch at login disabled",

    // Font size options
    font_12: "12px (compact)",
    font_13: "13px (default)",
    font_14: "14px (comfortable)",
    font_15: "15px (large)",
    font_16: "16px (extra large)",

    // Download
    page_download: "Download",
    dl_folder: "Download Folder",
    dl_folder_desc: "Leave empty to use system default",
    dl_browse: "Browse…",
    dl_placeholder: "",
    dl_path_error: "Path does not exist, please check the input",

    // Bangumi duplicate throttle
    sys_bangumi_dup: "Allow Duplicate Marks",
    sys_bangumi_dup_desc:
        "When on, every completed watch re-marks the same episode/movie. When off, a throttle window deduplicates: the same item is only marked once within the configured window.",
    sys_bangumi_dup_throttle: "Duplicate Mark Throttle (seconds)",
    sys_bangumi_dup_throttle_desc:
        "Active when Allow Duplicate Marks is off: the same item is recorded at most once within this many seconds. Minimum 120 s.",
    sys_bangumi_dup_throttle_floored:
        "Throttle cannot be less than 120 s, corrected to 120",

    // TMDB
    sys_tmdb: "TMDB Integration",
    sys_tmdb_key: "API Key",
    sys_tmdb_key_desc:
        "TMDB API Key used to fetch metadata missing from the media server during sync.",
    sys_tmdb_api_link: "Create an API key",
    sys_tmdb_key_placeholder: "",
};

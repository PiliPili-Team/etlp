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
    nav_system: "System",
    nav_logs: "Logs",
    nav_sec_play: "Playback",
    nav_sec_config: "Settings",
    nav_sec_debug: "Debug",

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

    // Player
    page_player: "Player",
    pl_type: "Player Type",
    pl_type_desc: "Choose a local media player",
    pl_startup: "Launch Options",
    pl_fullscreen: "Fullscreen",
    pl_fullscreen_desc: "Start the player in fullscreen mode",
    pl_mute: "Start Muted",
    pl_mute_desc: "Launch with audio disabled (mpv --no-audio)",
    pl_pretty_title: "Pretty Title",
    pl_pretty_title_desc: "Prepend server name to the player window title",
    pl_kill_start: "Kill on Startup",
    pl_kill_start_desc: "Kill existing player processes when etlp starts",
    pl_path: "Player Path",
    pl_path_desc: "Optional — leave empty to use the player from system PATH",
    pl_browse: "Browse…",
    pl_path_error: "Path not found — please check the input",

    // Version prefer
    page_vp: "Version Preference",
    vp_priority: "Version Priority",
    vp_keywords: "Version Keywords",
    vp_keywords_desc: "Match media version keywords in order — earlier entries win",
    vp_keywords_placeholder: "e.g. VCB-Studio, ANi, DBD-Raws",
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
    vp_last_ep_desc: "Do not add subsequent episodes when current is the last",
    vp_filter: "Version Filter Regex",
    vp_filter_desc:
        "Only versions matching this regex are added to the playlist (empty = no filter)",
    vp_filter_placeholder: "e.g. |VCB-Studio|ANi|Simplified",

    // Network
    page_network: "Network",
    net_proxy: "HTTP Proxy",
    net_proxy_desc: "Format: host:port (leave empty to disable)",
    net_skip_tls: "Skip TLS Verification",
    net_skip_tls_desc: "For self-signed Emby servers — insecure",
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
    sys_autostart_desc: "Automatically start etlp after logging in",
    sys_logs_title: "Logs",
    sys_log_level: "Log Level",
    sys_log_level_desc: "Set to Debug for more verbose output when troubleshooting",
    sys_log_mask: "Mask Sensitive Tokens",
    sys_log_mask_desc: "Replace sensitive tokens in logs with placeholders",
    sys_download: "Downloads",
    sys_speed_limit: "Speed Limit (MiB/s)",
    sys_speed_limit_desc: "0 = unlimited",
    sys_trakt: "Trakt.tv Scrobbling",
    sys_trakt_id: "Client ID",
    sys_trakt_id_desc:
        "Obtained after creating an app on trakt.tv — leave empty to disable",
    sys_trakt_id_placeholder: "Leave empty to disable Trakt",
    sys_trakt_secret: "Client Secret",
    sys_trakt_secret_desc:
        "Obtained after creating an app on trakt.tv — leave empty to disable",
    sys_trakt_secret_placeholder: "Leave empty to disable Trakt",
    sys_trakt_host: "Enable Host",
    sys_trakt_host_desc: "Only requests from this host trigger Trakt scrobbling",
    sys_trakt_host_placeholder: "e.g. emby.example.com",
    sys_bangumi: "Bangumi.tv Tracking",
    sys_bangumi_host: "Enable Host",
    sys_bangumi_host_desc:
        "Comma-separated host keywords; leave empty to disable, a single dot enables all",
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
    sys_privacy: "Privacy",
    sys_no_progress: "Disable Progress Reporting",
    sys_no_progress_desc: "Do not report playback progress to the Emby/Jellyfin server",
    sys_accent: "Accent Color",
    sys_accent_desc: "UI highlight color — affects buttons, active states, and badges",

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
    logs_open_folder: "Open Log Folder",
    logs_pick_mpv: "Choose mpv Log",

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
};

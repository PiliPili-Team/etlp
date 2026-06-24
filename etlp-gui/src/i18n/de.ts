import { zhCN } from "./zh-CN";

export const de: typeof zhCN = {
    ...zhCN,

    // App
    app_name: "Genshin",

    // Nav
    nav_overview: "Übersicht",
    nav_player: "Player",
    nav_version_prefer: "Version",
    nav_network: "Netzwerk",
    nav_config: "Konfiguration",
    nav_system: "System",
    nav_logs: "Protokolle",
    nav_sec_play: "Wiedergabe",
    nav_sec_settings: "Einstellungen",
    nav_sec_sync: "Synchronisierung",
    nav_bangumi: "Bangumi",
    nav_trakt: "Trakt",
    nav_sec_debug: "Debug",
    nav_download: "Downloads",

    // Common
    add: "Hinzufügen",
    add_placeholder: "Eingeben und mit Enter hinzufügen",
    open_dir: "Ordner öffnen",
    loading: "Konfiguration wird geladen…",

    // Overview
    page_overview: "Übersicht",
    ov_service: "Lokaler Dienst",
    ov_running: "Läuft",
    ov_stopped: "Gestoppt",
    ov_port: "Port",
    ov_port_desc: "Lokale Lauschadresse",
    ov_uptime: "Laufzeit",
    ov_uptime_desc: "Seit Dienststart",
    ov_address: "Adresse",
    ov_address_desc: "Nur localhost",
    ov_config: "Einstellungen",
    ov_config_file: "Konfigurationsdatei",
    ov_config_file_desc: "Anzeigen oder in einem externen Editor öffnen",
    ov_edit_config: "Konfiguration bearbeiten",
    ov_restart: "Dienst neu starten",
    ov_restart_desc:
        "Dienst stoppen, Ressourcen freigeben und mit der neuesten Konfiguration neu starten",
    ov_about: "Über",
    ov_about_desc: "Versionsinformationen und Open-Source-Danksagungen",
    ov_view: "Anzeigen",
    ov_start: "Starten",
    ov_stop: "Stoppen",

    // Toasts
    toast_started: "Dienst auf Port {port} gestartet",
    toast_stopped: "Dienst gestoppt",
    toast_restarted: "Dienst auf Port {port} neu gestartet",
    toast_start_failed: "Dienst konnte nicht gestartet werden",
    toast_stop_failed: "Dienst konnte nicht gestoppt werden",
    toast_restart_failed: "Dienst konnte nicht neu gestartet werden",
    toast_open_failed: "Öffnen fehlgeschlagen",
    sync_not_configured: "Noch nicht konfiguriert – füllen Sie zuerst die Felder aus",

    // Player
    page_player: "Player",
    pl_type: "Player-Typ",
    pl_type_desc: "Einen lokalen Medienplayer auswählen",
    pl_startup: "Startoptionen",
    pl_fullscreen: "Vollbild",
    pl_fullscreen_desc: "Den Player im Vollbildmodus starten",
    pl_mute: "Stumm starten",
    pl_mute_desc: "Stumm starten (mpv --mute=yes)",
    pl_pretty_title: "Titel verschönern",
    pl_pretty_title_desc: "Den Servernamen dem Fenstertitel des Players voranstellen",
    pl_kill_start: "Beim Start beenden",
    pl_kill_start_desc: "Vorhandene Player-Prozesse beim Start beenden",
    pl_path: "Player-Pfad",
    pl_path_desc:
        "Optional – leer lassen, um den Player aus dem System-PATH zu verwenden",
    pl_browse: "Durchsuchen…",
    pl_path_error: "Pfad nicht gefunden – bitte Eingabe prüfen",
    pl_progress_support:
        "Fortschrittsmeldung: mpv / IINA werden vollständig unterstützt – Live-Aktualisierungen während der Wiedergabe, Zurückschreiben der Wiedergabeposition beim Beenden, Markieren als gesehen, Trakt-/Bangumi-Synchronisierung und Episodenverfolgung. Andere Player schreiben nur die finale Position und synchronisieren beim Beenden, ohne Live-Meldung während der Wiedergabe; VLC spielt die ganze Staffel durchgehend ab, MPC und dandanplay sind auf eine Episode beschränkt, und das Zurücklesen der Position bei PotPlayer funktioniert nur unter Windows",

    // Version prefer
    page_vp: "Versionseinstellungen",
    vp_priority: "Versionspriorität",
    vp_keywords: "Versionsschlüsselwörter",
    vp_keywords_desc:
        "Versionsschlüsselwörter der Reihe nach abgleichen – frühere Einträge gewinnen",
    vp_keywords_placeholder: "z. B. VCB-Studio, ANi, DBD-Raws",
    vp_playlist: "Auf Wiedergabeliste anwenden",
    vp_playlist_desc: "Versionspriorität beim Erstellen der Wiedergabeliste verwenden",
    vp_subtitle: "Untertiteleinstellung",
    vp_sub_priority: "Untertitelpriorität",
    vp_sub_priority_desc:
        "Schlüsselwörter der Untertitelspuren der Reihe nach abgleichen",
    vp_sub_priority_placeholder: "z. B. Vereinfacht, CHS",
    vp_sub_extract: "Versionsübergreifende Untertitelextraktion",
    vp_sub_extract_desc:
        "Untertitel aus anderen Versionen extrahieren, wenn in der aktuellen keine gefunden werden",
    vp_sub_extract_placeholder: "z. B. CHS, Vereinfacht",
    vp_limits: "Wiedergabelisten-Limits",
    vp_max_eps: "Max. Episoden pro Sitzung",
    vp_max_eps_desc:
        "Episoden werden bei Erreichen dieses Limits abgeschnitten; 0 oder leer bedeutet unbegrenzt (empfohlen: 10–100)",
    vp_last_ep: "Bei letzter Episode deaktivieren",
    vp_last_ep_desc:
        "Ein: Bei der letzten Episode der Staffel wird keine Wiedergabeliste erstellt und nur diese Episode geöffnet (nichts folgt). Aus: Immer die Wiedergabeliste erstellen (aktuelle + spätere Episoden)",
    vp_filter: "Versionsfilter-Regex",
    vp_filter_desc:
        "Nur Versionen, die diesem Regex entsprechen, werden zur Wiedergabeliste hinzugefügt (leer = kein Filter)",
    vp_filter_placeholder: "z. B. |VCB-Studio|ANi|Vereinfacht",
    vp_filter_valid: "Gültiger Regex",
    vp_filter_invalid: "Ungültiger Regex",

    // Network
    page_network: "Netzwerk",
    net_proxy: "HTTP-Proxy",
    net_proxy_desc: "Format: host:port (leer lassen zum Deaktivieren)",
    net_skip_tls: "TLS-Überprüfung überspringen",
    net_skip_tls_desc: "Für selbstsignierte Emby-Server – unsicher",
    net_redirect: "Weiterleitungserkennung",
    net_redirect_hosts: "Auf Weiterleitungen zu prüfende Hosts",
    net_redirect_hosts_desc:
        "Stream-URLs dieser Hosts werden vor der Übergabe an den Player auf 30x-Weiterleitungen geprüft (standardmäßig leer)",

    // System
    page_system: "System",
    sys_appearance: "Erscheinungsbild",
    sys_theme: "Design",
    sys_theme_desc: "Hell, dunkel oder dem System folgen",
    sys_lang: "Sprache",
    sys_lang_desc: "Anzeigesprache der Oberfläche",
    sys_theme_system: "System",
    sys_theme_light: "Hell",
    sys_theme_dark: "Dunkel",
    sys_lang_system: "System",
    sys_display: "Anzeige",
    sys_font_size: "Schriftgröße",
    sys_font_size_desc: "Die Textgröße der Oberfläche anpassen",
    sys_zoom: "UI-Skalierung",
    sys_zoom_desc: "HiDPI-/Hochauflösungs-Gesamtzoom – aktueller DPR: {dpr}",
    sys_font: "UI-Schriftart",
    sys_font_desc: "Die Schriftart der Oberfläche auswählen",
    sys_font_default: "Standard (system-ui)",
    sys_startup: "Start",
    sys_autostart: "Bei Anmeldung starten",
    sys_autostart_desc: "Die App nach der Anmeldung automatisch starten",
    sys_silent_start: "Stiller Start",
    sys_silent_start_desc:
        "Versteckt im Infobereich starten, ohne das Hauptfenster anzuzeigen (in Kombination mit Autostart unauffälliger)",
    sys_logs_title: "Protokolle",
    sys_log_level: "Protokollstufe",
    sys_log_level_desc: "Für ausführlichere Ausgabe bei der Fehlersuche auf Debug setzen",
    sys_log_max_size: "Max. Protokollgröße (MB)",
    sys_log_max_size_desc:
        "Bei einer neuen Datei rotieren, sobald die aktuelle diese Größe überschreitet (20–200 MB)",
    sys_log_max_size_capped: "Auf das Maximum von 200 MB begrenzt",
    sys_log_max_size_floored: "Auf das Minimum von 20 MB angehoben",
    sys_log_max_files: "Max. Protokolldateien",
    sys_log_max_files_desc:
        "Anzahl der zu behaltenden rotierten Protokolldateien (1–14); die älteste wird entfernt",
    sys_log_max_files_capped: "Auf das Maximum von 14 Dateien begrenzt",
    sys_log_mask: "Sensible Tokens maskieren",
    sys_log_mask_desc: "Sensiblen Text in Protokollen durch Platzhalter ersetzen",
    sys_cache: "Cache",
    sys_cache_size: "Aktuelle Cache-Größe",
    sys_cache_size_desc:
        "Von Protokollen und anderem Laufzeit-Cache belegter Speicherplatz",
    sys_cache_clear: "Cache leeren",
    sys_cache_clear_desc: "Die Protokolldateien leeren, um Speicherplatz freizugeben",
    cache_confirm_title: "Cache leeren",
    cache_confirm_message:
        "Der Dienst muss vor dem Leeren des Caches gestoppt werden, da sonst gerade geschriebene Protokolle inkonsistent werden können. Bestätigen Sie, dass der Dienst gestoppt ist, und fortfahren?",
    cache_confirm_ok: "Leeren",
    cache_confirm_cancel: "Abbrechen",
    cache_stop_first: "Stoppen Sie den Dienst, bevor Sie den Cache leeren",
    cache_cleared: "Cache geleert, {size} freigegeben",
    sys_general: "Allgemein",
    sys_about: "Über",
    sys_about_desc: "Versionsinformationen und Open-Source-Danksagungen",
    sys_download: "Downloads",
    sys_speed_limit: "Geschwindigkeitslimit (MiB/s)",
    sys_speed_limit_desc:
        "Begrenzt die von Downloads und Vorab-Caching genutzte Bandbreite (MiB/s); 0 = unbegrenzt",
    sys_download_note:
        "Vorab-Laden und Download-Modus werden durch die Befehle des Browser-Userscripts ausgelöst, nicht hier umgeschaltet: „Während der Wiedergabe cachen“ des Skripts ist Vorab-Laden und „Nur herunterladen“ ist der Download-Modus; der Download-Modus erfordert außerdem, dass Ihr Medienserver-Konto Ressourcen-Downloads erlaubt",
    sys_trakt: "Trakt.tv-Scrobbling",
    sys_trakt_sync_note:
        "Wenn die Wiedergabe endet, wird Ihr Sehverlauf automatisch mit Trakt synchronisiert: Ab etwa 80 % wird die Episode als gesehen markiert, darunter bleibt sie unmarkiert; weitere Episoden derselben Staffel, die Sie in Ihrem Medienserver bereits beendet haben, werden ebenfalls markiert, ohne bereits vorhandene zu duplizieren. Unter 80 % wird Ihre Position gemerkt, damit Sie später fortfahren können, und die nächste Episode erscheint unter „Weiter ansehen“; erneutes Ansehen derselben Episode wird wieder aufgezeichnet – ob ein kurzer Zeitabstand zulässig ist, steuert der Schalter „Duplikate zulassen“ unten.",
    sys_trakt_dashboard: "Trakt-Dashboard öffnen",
    sys_trakt_setup_title: "Einrichtung",
    sys_trakt_setup_step1: "1. Eine App auf Trakt erstellen: ",
    sys_trakt_setup_link: "trakt.tv/oauth/applications",
    sys_trakt_setup_step2:
        "2. Die „Redirect uri“ der App auf die folgende Adresse setzen:",
    sys_trakt_setup_copy: "Kopieren",
    sys_trakt_setup_copied: "Redirect-URI kopiert",
    sys_trakt_setup_copy_failed:
        "Kopieren fehlgeschlagen – bitte manuell auswählen und kopieren",
    sys_trakt_id: "Client-ID",
    sys_trakt_id_desc:
        "Nach dem Erstellen einer App auf trakt.tv erhältlich – leer lassen zum Deaktivieren",
    sys_trakt_id_placeholder: "Leer lassen, um Trakt zu deaktivieren",
    sys_trakt_secret: "Client-Secret",
    sys_trakt_secret_desc:
        "Nach dem Erstellen einer App auf trakt.tv erhältlich – leer lassen zum Deaktivieren",
    sys_trakt_secret_placeholder: "Leer lassen, um Trakt zu deaktivieren",
    sys_trakt_user: "Benutzername",
    sys_trakt_user_desc: "Ihr Trakt-Benutzername (nicht der Anzeigename)",
    sys_trakt_user_placeholder: "z. B. your_trakt_user",
    sys_trakt_host: "Host aktivieren",
    sys_trakt_host_desc:
        "Kommagetrennte Host-Schlüsselwörter; leer lassen zum Deaktivieren, ein einzelner Punkt aktiviert alle",
    sys_trakt_host_placeholder: "z. B. localhost, 192.168., emby.example.com",
    sys_trakt_dup: "Doppeltes Markieren zulassen",
    sys_trakt_dup_desc:
        "Wenn aktiviert, wird bei jedem Abschluss dieselbe Episode/derselbe Film erneut markiert; wenn deaktiviert, gilt eine gedrosselte Duplikatentfernung: dasselbe Element, das innerhalb des unten festgelegten Drosselfensters erneut beendet wird, wird nur einmal markiert (nachgetragene frühere Episoden werden immer dedupliziert)",
    sys_trakt_dup_throttle: "Drosselung für doppeltes Markieren (Sekunden)",
    sys_trakt_dup_throttle_desc:
        "Wirksam, wenn „Doppeltes Markieren zulassen“ deaktiviert ist: dasselbe Element, das innerhalb dieser Sekundenzahl erneut beendet wird, wird nur einmal aufgezeichnet. Minimum 120 s",
    sys_trakt_dup_throttle_floored:
        "Drosselung darf nicht unter 120 Sekunden liegen; auf 120 korrigiert",
    sys_bangumi: "Bangumi.tv-Verfolgung",
    sys_bangumi_sync_note:
        "Wenn die Wiedergabe endet, wird Ihr Sehverlauf automatisch mit Bangumi synchronisiert: Ab etwa 80 % wird die Episode als gesehen markiert, darunter bleibt sie unmarkiert; weitere Episoden derselben Staffel, die Sie in Ihrem Medienserver bereits beendet haben, werden ebenfalls markiert, ohne bereits vorhandene zu duplizieren. Das Markieren als gesehen setzt das Werk außerdem auf „schaue gerade“.",
    sys_bangumi_host: "Host aktivieren",
    sys_bangumi_host_desc:
        "Kommagetrennte Host-Schlüsselwörter; leer lassen zum Deaktivieren, ein einzelner Punkt aktiviert alle",
    sys_bangumi_host_placeholder: "z. B. localhost, 192.168., emby.example.com",
    sys_bangumi_user: "Benutzername / UID",
    sys_bangumi_user_desc: "bgm.tv-Benutzername oder die Ziffern in bgm.tv/user/123456",
    sys_bangumi_user_placeholder: "z. B. 123456",
    sys_bangumi_token: "Zugriffstoken",
    sys_bangumi_token_desc:
        "Erzeugt unter next.bgm.tv/demo/access-token – leer lassen zum Deaktivieren",
    sys_bangumi_token_placeholder: "Leer lassen, um Bangumi zu deaktivieren",
    sys_bangumi_private: "Private Sammlung",
    sys_bangumi_private_desc:
        "Neu synchronisierte Einträge in Ihrem öffentlichen Profil verbergen",
    sys_bangumi_genres: "Genre-Filter",
    sys_bangumi_genres_desc:
        "Regex, der mit den Serien-Genres abgeglichen wird; nur passende Serien werden synchronisiert",
    sys_bangumi_genres_placeholder: "动画|anime",
    sys_bangumi_map: "ID-Zuordnung",
    sys_bangumi_map_desc:
        "Eine tmdb-/imdb-/tvdb-Serie oder einen Film an ein genaues Bangumi-Subjekt binden; hat höchste Priorität. Drei Staffelformate: ganze Staffel (S4), geschlossener Episodenbereich (S5E1~S5E50, nur Episoden 1–50), offener Bereich (S5E51++, ab Episode 51). E±N rechts verschiebt den lokalen Episodenindex auf die Bangumi-Sortiernummer. Beispiele: tmdb:10000|type:tv|S4 -> bgm:20000|E+59; tmdb:10000|type:tv|S5E1~S5E50 -> bgm:20001; tmdb:10000|type:tv|S5E51++ -> bgm:20002; tmdb:10001|type:movie -> bgm:30000. Ohne type wird er aus der Staffel abgeleitet (eine Staffel bedeutet TV, sonst Film)",
    map_placeholder: "tmdb:10000|type:tv|S4 -> bgm:20000|E+59",
    map_check: "Prüfen & hinzufügen",
    map_remove: "Entfernen",
    map_err_empty: "Geben Sie eine Zuordnung ein",
    map_err_format: "Fehlerhaft – erwartet „LHS -> RHS“",
    map_err_provider: "Unbekannte Quelle; nur tmdb / imdb / tvdb werden unterstützt",
    map_err_provider_id: "Ungültige ID (tmdb/tvdb numerisch, imdb beginnt mit tt)",
    map_err_type: "type muss tv oder movie sein",
    map_err_season: "Ungültige Staffel; erwartet eine positive Ganzzahl wie S4",
    map_err_ep_range:
        "Ungültiger Episodenbereich; verwende S5E106~S5E157 (geschlossen) oder S5E158++ (offen); Start darf nicht größer als Ende sein",
    map_err_subject: "Ungültige Bangumi-Subjekt-ID; erwartet eine positive Ganzzahl",
    map_err_offset:
        "Ungültiger Episoden-Offset; erwartet eine Ganzzahl wie E+59 oder E-3",
    map_err_movie_season:
        "Ein Film kann keine Staffel oder keinen Episoden-Offset tragen",
    map_err_duplicate: "Eine identische Zuordnung existiert bereits",
    sync_refresh: "Autorisierung aktualisieren",
    sync_refreshing: "Wird aktualisiert…",
    sync_authorize_opened:
        "Autorisierungsseite geöffnet – schließen Sie sie im Browser ab",
    sync_auth_valid: "Autorisierung ist gültig",
    sync_start_service_first: "Starten Sie zuerst den Dienst",
    sync_refresh_confirm_title: "Autorisierung aktualisieren",
    sync_refresh_confirm_message:
        "Autorisierung jetzt manuell aktualisieren? Wenn das aktuelle Token ungültig ist, wird die Autorisierungsseite in Ihrem Browser geöffnet.",
    sync_refresh_confirm_ok: "Aktualisieren",
    sync_test: "Autorisierung prüfen",
    sync_test_desc: "Prüfen, ob die aktuellen Anmeldedaten funktionieren",
    sync_testing: "Wird geprüft…",
    sync_test_ok: "Autorisierung funktioniert",
    sync_test_fail:
        "Autorisierung fehlgeschlagen – die Konfiguration ist möglicherweise falsch oder noch nicht autorisiert. Klicken Sie oben rechts auf „Autorisierung aktualisieren“.",
    sync_incomplete:
        "Konfiguration unvollständig – füllen Sie die Pflichtfelder vor der Prüfung aus",

    // Config tab (config file + backup / restore / reset / update)
    page_config: "Konfiguration",
    cfg_file_title: "Konfigurationsdatei",
    cfg_backup_title: "Sichern & Wiederherstellen",
    cfg_backup_now: "Jetzt sichern",
    cfg_backup_now_desc:
        "Die aktuelle Konfiguration in ein ZIP-Backup mit Zeitstempel packen",
    cfg_backup_done: "Konfiguration gesichert",
    cfg_backup_list: "Backups",
    cfg_backup_list_desc: "Es werden bis zu 5 Backups aufbewahrt – aktuell {count}",
    cfg_backup_empty: "Noch keine Backups",
    cfg_view: "Anzeigen",
    cfg_restore: "Wiederherstellen",
    cfg_delete: "Löschen",
    cfg_import: "Backup importieren",
    cfg_import_desc:
        "Konfiguration aus einer externen ZIP-Datei importieren und wiederherstellen",
    cfg_restore_done: "Konfiguration wiederhergestellt",
    cfg_restore_confirm_title: "Konfiguration wiederherstellen",
    cfg_restore_confirm_message:
        "Aktuelle Konfiguration mit Backup „{name}“ überschreiben? Dies kann nicht rückgängig gemacht werden.",
    cfg_import_confirm_title: "Konfiguration importieren & wiederherstellen",
    cfg_import_confirm_message:
        "Dieses Backup importieren und die aktuelle Konfiguration überschreiben? Dies kann nicht rückgängig gemacht werden.",
    cfg_delete_confirm_title: "Backup löschen",
    cfg_delete_confirm_message: "Backup „{name}“ löschen?",
    cfg_reset_title: "Zurücksetzen",
    cfg_reset: "Auf Standard zurücksetzen",
    cfg_reset_desc: "Alle Einstellungen auf ihre Standardwerte zurücksetzen",
    cfg_reset_done: "Konfiguration auf Standard zurückgesetzt",
    cfg_reset_confirm_title: "Konfiguration zurücksetzen",
    cfg_reset_confirm_message:
        "Auf die Standardkonfiguration zurücksetzen? Die aktuelle Konfiguration wird überschrieben – dies kann nicht rückgängig gemacht werden.",
    cfg_update_title: "Aktualisierung",
    cfg_update_auto: "Automatisch nach Updates suchen",
    cfg_update_auto_desc:
        "Beim Start GitHub auf neue Releases prüfen und einen Hinweis in der Übersicht anzeigen",
    cfg_update_check: "Jetzt prüfen",
    cfg_update_check_desc: "Jetzt GitHub auf eine neuere Version prüfen",
    cfg_update_checking: "Wird geprüft…",
    cfg_update_available: "Neue Version v{version} gefunden",
    cfg_update_latest: "Sie verwenden die neueste Version v{version}",
    cfg_update_current_ver: "Aktuell: v{version}",
    cfg_update_latest_ver: "Neueste: v{version}",
    cfg_update_up_to_date: "Aktuell",
    cfg_update_install: "Herunterladen & Installieren",

    // Update banner (overview)
    ov_update_available: "Neue Version v{version} verfügbar",
    ov_update_action: "Update installieren",
    ov_update_dismiss: "Diese Version ignorieren",
    ov_update_downloading: "Update wird heruntergeladen…",
    ov_update_failed: "Update fehlgeschlagen",
    sys_privacy: "Datenschutz",
    sys_no_progress: "Fortschrittsmeldung deaktivieren",
    sys_no_progress_desc:
        "Den Wiedergabefortschritt nicht an den Emby/Jellyfin-Server melden",
    sys_accent: "Akzentfarbe",
    sys_accent_desc:
        "UI-Hervorhebungsfarbe – betrifft Schaltflächen, aktive Zustände und Badges",
    sys_center_nav: "Seitenleiste zentrieren",
    sys_center_nav_desc: "Die Tabs der Seitenleiste als Gruppe vertikal zentrieren",

    // Log levels
    log_error: "Error – nur Abstürze",
    log_warn: "Warn – ungewöhnliche Zustände",
    log_info: "Info – Standard, alltäglicher Betrieb",
    log_debug: "Debug – Fehlersuche",
    log_trace: "Trace – volle Details",

    // Logs page
    page_logs: "Protokolle",
    logs_app: "App-Protokoll",
    logs_mpv: "mpv-Protokoll",
    logs_filter: "Filtern…",
    logs_clear: "Leeren",
    logs_bottom: "↓ Nach unten",
    logs_empty: "Warte auf Protokollausgabe…",
    logs_no_mpv:
        "Kein mpv-Protokoll gefunden – klicken Sie auf „mpv-Protokoll wählen“, um eines zu laden",
    logs_lines: "Zeilen",
    logs_loading_older: "Ältere Protokolle werden geladen…",
    logs_scroll_older: "Nach oben scrollen, um ältere Protokolle zu laden",
    logs_open_folder: "Protokollordner öffnen",
    logs_pick_mpv: "mpv-Protokoll wählen",
    logs_reset_mpv: "Auf Standard zurücksetzen",
    logs_reset_mpv_title: "Zurück zum Standard-mpv-Protokoll im Protokollordner wechseln",
    logs_anon: "Anonym",
    logs_anon_title:
        "Geräte-ID, Tokens, IPs, Benutzer-ID, URL-Host und Bangumi-/Trakt-Benutzernamen nur in der Ansicht ausblenden, praktisch zum Teilen von Screenshots; die Protokolldatei bleibt unverändert – die Dateischwärzung folgt weiterhin dem Schalter „Sensibler Text“",

    // About modal
    about_thanks: "Danksagungen",
    about_thanks_desc: "für endlose Inspiration",
    about_version_label: "Version",

    // Autostart toasts
    autostart_on: "Start bei Anmeldung aktiviert",
    autostart_off: "Start bei Anmeldung deaktiviert",

    // Font size options
    font_12: "12px (kompakt)",
    font_13: "13px (Standard)",
    font_14: "14px (komfortabel)",
    font_15: "15px (groß)",
    font_16: "16px (sehr groß)",

    // Download
    page_download: "Downloads",
    dl_folder: "Download-Ordner",
    dl_folder_desc:
        "Speicherort für Mediendateien. Leer lassen für den Standard (~/Downloads)",
    dl_browse: "Durchsuchen…",
    dl_placeholder: "Leer lassen für Standard (~/Downloads)",
    dl_path_error: "Pfad existiert nicht, bitte Eingabe prüfen",
};

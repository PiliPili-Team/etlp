//! Tauri application entry point for etlp GUI.

pub mod backup;
pub mod commands;
pub mod config_patch;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use commands::GuiState;

// ── Language helpers ──────────────────────────────────────────────────────────

/// Detects whether the system UI language is Chinese using platform-native
/// APIs (via sys-locale). Falls back to LANG/LC_ALL/LANGUAGE env vars for
/// environments where the native API is unavailable.
fn sys_is_chinese() -> bool {
    // sys-locale uses NSLocale on macOS, GetUserDefaultLocaleName on Windows,
    // and setlocale() on Linux — all more reliable than env vars for GUI apps.
    let native = sys_locale::get_locale()
        .or_else(|| sys_locale::get_locales().next())
        .unwrap_or_default()
        .to_lowercase();
    if !native.is_empty() {
        return native.starts_with("zh");
    }
    // Fallback: check common locale env vars.
    for key in ["LANG", "LANGUAGE", "LC_ALL", "LC_MESSAGES"] {
        let val = std::env::var(key).unwrap_or_default().to_lowercase();
        if val.starts_with("zh") {
            return true;
        }
    }
    false
}

struct TrayLabels {
    tooltip: &'static str,
    show: &'static str,
    start: &'static str,
    stop: &'static str,
    reload: &'static str,
    about: &'static str,
    quit: &'static str,
}

impl TrayLabels {
    fn detect() -> Self {
        if sys_is_chinese() {
            Self {
                tooltip: "原神",
                show: "显示主界面",
                start: "启动服务",
                stop: "停止服务",
                reload: "重载配置",
                about: "关于",
                quit: "退出",
            }
        } else {
            Self {
                tooltip: "Genshin",
                show: "Show Window",
                start: "Start Service",
                stop: "Stop Service",
                reload: "Reload Config",
                about: "About",
                quit: "Quit",
            }
        }
    }
}

// ── PATH augmentation (macOS) ─────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn augment_path() {
    const EXTRA: &[&str] = &[
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
    ];
    let current = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<&str> = current.split(':').collect();
    for &p in EXTRA.iter().rev() {
        if !parts.contains(&p) {
            parts.insert(0, p);
        }
    }
    // SAFETY: called before any threads spawn.
    unsafe { std::env::set_var("PATH", parts.join(":")) };
}

#[cfg(not(target_os = "macos"))]
fn augment_path() {}

// ── Tray icon decoding ────────────────────────────────────────────────────────

/// Decode the bundled tray PNG into `(rgba_bytes, width, height)`.
///
/// Returns an error instead of panicking so a corrupt/unsupported asset only
/// costs the custom tray icon rather than the whole process.
fn decode_tray_icon(
    bytes: &[u8],
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error>> {
    use image::ImageDecoder as _;
    let cursor = std::io::Cursor::new(bytes);
    let decoder = image::codecs::png::PngDecoder::new(cursor)?;
    let (w, h) = decoder.dimensions();
    let mut buf = vec![0u8; usize::try_from(decoder.total_bytes())?];
    decoder.read_image(&mut buf)?;
    Ok((buf, w, h))
}

// ── Tray menu builder ─────────────────────────────────────────────────────────

fn build_tray_menu(
    app: &impl tauri::Manager<tauri::Wry>,
    labels: &TrayLabels,
    running: bool,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let show = MenuItemBuilder::with_id("show", labels.show).build(app)?;
    let toggle = MenuItemBuilder::with_id(
        "toggle",
        if running { labels.stop } else { labels.start },
    )
    .build(app)?;
    let reload =
        MenuItemBuilder::with_id("reload", labels.reload).build(app)?;
    let about = MenuItemBuilder::with_id("about", labels.about).build(app)?;
    let quit = MenuItemBuilder::with_id("quit", labels.quit).build(app)?;

    MenuBuilder::new(app)
        .items(&[&show, &toggle, &reload, &about, &quit])
        .build()
}

/// Rebuild the tray menu to reflect the service's running state. Best-effort:
/// a missing tray or menu-build error is ignored.
fn refresh_tray_running(app: &tauri::AppHandle, running: bool) {
    if let Some(tray) = app.tray_by_id("")
        && let Ok(menu) = build_tray_menu(app, &TrayLabels::detect(), running)
    {
        let _ = tray.set_menu(Some(menu));
    }
}

/// Show a best-effort system notification. Failures (e.g. permissions denied)
/// are ignored so notification problems never affect the app.
fn notify(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt as _;
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Show or hide the macOS dock icon by switching the activation policy.
///
/// `Regular` shows the dock icon; `Accessory` hides it so the app keeps running
/// in the menu-bar tray with no dock presence. No-op on non-macOS platforms.
#[cfg(target_os = "macos")]
fn set_dock_visible(app: &tauri::AppHandle, visible: bool) {
    let policy = if visible {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    };
    let _ = app.set_activation_policy(policy);
}

#[cfg(not(target_os = "macos"))]
fn set_dock_visible(_app: &tauri::AppHandle, _visible: bool) {}

/// Reveal the main window and restore the dock icon together, keeping the dock
/// state in sync with window visibility. Used by every code path that brings the
/// window back from the tray.
fn show_main_window(app: &tauri::AppHandle) {
    set_dock_visible(app, true);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run() {
    augment_path();

    // SAFETY: single-threaded at this point.
    unsafe { std::env::set_var(etlp_server::platform::ENV_RUNTIME, "app") };

    // Initialise logging to a file in the data directory so the Logs tab can
    // tail it. Do this before any Tauri threads start.
    let data_dir = etlp_server::platform::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    std::fs::create_dir_all(&data_dir).ok();
    // Relocate legacy flat-layout files (etlp.log, mpv.log, bangumi cache) into
    // the new log/ and cache/ sub-directories before opening any of them.
    etlp_server::platform::migrate_layout(&data_dir);
    let log_dir = etlp_server::platform::log_dir_in(&data_dir);
    std::fs::create_dir_all(&log_dir).ok();
    let log_file = log_dir.join("etlp.log");

    // Read the config early so we can honour dev.log_level from the very first
    // log line and decide whether to start hidden.  Failures are silently
    // ignored here — the server will re-read and report the error on startup.
    let initial_config = etlp_server::platform::config_dir()
        .and_then(|d| etlp_config::Config::load_from_dir(&d).ok());
    let initial_log_level = initial_config
        .as_ref()
        .map(|c| c.dev.log_level.clone())
        .unwrap_or_else(|| "info".to_owned());
    // Silent start: when enabled, launch straight to the tray without showing
    // the main window (pairs with OS autostart for a quiet login).
    let silent_start = initial_config
        .as_ref()
        .map(|c| c.gui.silent_start)
        .unwrap_or(false);

    let masker = etlp_logging::Masker::new(false);
    let log_handle = etlp_logging::init(
        masker,
        &initial_log_level,
        Some(log_file.as_path()),
    )
    .ok();

    if let Some(d) = etlp_server::platform::config_dir() {
        eprintln!("[etlp] config dir: {}", d.display());
    }
    eprintln!("[etlp] data   dir: {}", data_dir.display());
    eprintln!("[etlp] log    file: {}", log_file.display());

    // Decode the monochrome PNG to raw RGBA at startup. A decode failure must
    // not crash the app — the tray simply launches without a custom icon.
    let tray_icon_bytes: &[u8] = include_bytes!("../icons/tray-icon.png");
    let tray_rgba: Option<(Vec<u8>, u32, u32)> =
        decode_tray_icon(tray_icon_bytes)
            .map_err(|e| eprintln!("[etlp] tray icon decode failed: {e}"))
            .ok();

    let gui_state = {
        let s = GuiState::default();
        if let Ok(mut lf) = s.log_file.lock() {
            *lf = log_file;
        }
        if let Ok(mut h) = s.log_handle.lock() {
            *h = log_handle;
        }
        s
    };

    let labels = TrayLabels::detect();

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::AppleScript,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .manage(gui_state)
        .setup(move |app| {
            // ── Tray icon ──────────────────────────────────────────────────────
            let menu = build_tray_menu(app.handle(), &labels, false)?;

            let mut tray_builder = TrayIconBuilder::new();
            if let Some((tray_buf, tray_w, tray_h)) = tray_rgba {
                let tray_img =
                    tauri::image::Image::new_owned(tray_buf, tray_w, tray_h);
                tray_builder =
                    tray_builder.icon(tray_img).icon_as_template(true);
            }

            let _tray = tray_builder
                .tooltip(labels.tooltip)
                .menu(&menu)
                .on_menu_event(|app, event| {
                    let state = app.state::<GuiState>();
                    match event.id().as_ref() {
                        "show" => show_main_window(app),
                        "toggle" => {
                            let running = state
                                .running
                                .load(std::sync::atomic::Ordering::Acquire);
                            let app_c = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let state2 = app_c.state::<GuiState>();
                                if running {
                                    let _ = commands::stop_server(state2).await;
                                } else {
                                    let _ =
                                        commands::start_server(state2).await;
                                }
                                // Rebuild menu to reflect new state
                                let new_running = app_c
                                    .state::<GuiState>()
                                    .running
                                    .load(std::sync::atomic::Ordering::Acquire);
                                if let Some(tray) = app_c.tray_by_id("")
                                    && let Ok(m) = build_tray_menu(
                                        &app_c,
                                        &TrayLabels::detect(),
                                        new_running,
                                    )
                                {
                                    let _ = tray.set_menu(Some(m));
                                }
                            });
                        }
                        "reload" => {
                            let app_c = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let state2 = app_c.state::<GuiState>();
                                let _ = commands::restart_server(state2).await;
                                let new_running = app_c
                                    .state::<GuiState>()
                                    .running
                                    .load(std::sync::atomic::Ordering::Acquire);
                                if let Some(tray) = app_c.tray_by_id("")
                                    && let Ok(m) = build_tray_menu(
                                        &app_c,
                                        &TrayLabels::detect(),
                                        new_running,
                                    )
                                {
                                    let _ = tray.set_menu(Some(m));
                                }
                            });
                        }
                        "about" => {
                            // Bring window to front and signal frontend to open about modal
                            show_main_window(app);
                            app.emit("show-about", ()).ok();
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::TrayIconEvent;
                    // Left-click: bring the window to front.
                    // Do NOT rebuild the menu here — rebuilding while the menu
                    // animation is in progress causes visible flicker on macOS.
                    if let TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                // ── macOS vibrancy ─────────────────────────────────────────────
                // Pin to window-vibrancy 0.6 (same version bundled in tauri
                // 2.11.3) so Cargo deduplicates to a single copy — no multiply-
                // defined symbol. Tauri's set_effects() sets NSVisualEffectView
                // interactionType differently and breaks CSS drag regions.
                #[cfg(target_os = "macos")]
                {
                    use window_vibrancy::{
                        NSVisualEffectMaterial, apply_vibrancy,
                    };
                    apply_vibrancy(
                        &window,
                        NSVisualEffectMaterial::Sidebar,
                        None,
                        Some(12.0),
                    )
                    .unwrap_or_else(|e| eprintln!("[etlp] vibrancy: {e}"));
                }
                // Show the main window on launch; tauri.conf.json sets
                // visible:false so the OS doesn't flash an unstyled frame.
                // When silent start is enabled the window stays hidden and the
                // app lives in the tray until the user opens it — so the dock
                // icon is hidden too, keeping the dock in sync with the window.
                if silent_start {
                    set_dock_visible(app.handle(), false);
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            // Silent start also brings the local service up so the app is
            // usable straight from the tray. A startup failure (e.g. a port
            // conflict) is logged and surfaced as a system notification — never
            // a crash, since there is no window to report it.
            if silent_start {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<GuiState>();
                    match commands::start_server(state).await {
                        Ok(port) => {
                            tracing::info!(
                                port,
                                "silent start: service started"
                            );
                            refresh_tray_running(&app_handle, true);
                        }
                        Err(e) => {
                            tracing::error!(
                                "silent start: service failed to start: {e}"
                            );
                            let body = if sys_is_chinese() {
                                format!("服务启动失败：{e}")
                            } else {
                                format!("Failed to start service: {e}")
                            };
                            notify(&app_handle, "etlp", &body);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::restart_server,
            commands::get_server_status,
            commands::get_config,
            commands::update_config_field,
            commands::reload_config,
            commands::open_config_folder,
            commands::edit_config,
            commands::refresh_trakt_auth,
            commands::refresh_bangumi_auth,
            commands::test_trakt_auth,
            commands::test_bangumi_auth,
            commands::get_log_lines,
            commands::tail_log,
            commands::read_log_before,
            commands::clear_log_position,
            commands::clear_log_file,
            commands::open_log_folder,
            commands::get_log_paths,
            commands::get_cache_size,
            commands::clear_cache,
            commands::list_config_backups,
            commands::backup_config,
            commands::restore_config,
            commands::delete_config_backup,
            commands::reveal_config_backup,
            commands::reset_config,
            commands::pick_player_path,
            commands::path_exists,
            commands::get_app_version,
            commands::check_update,
            commands::list_system_fonts,
            commands::set_autostart,
            commands::get_autostart,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Closing the window only hides it — the app keeps running in the
                // tray. Drop the dock icon too so the dock stays in sync with the
                // window; it is restored whenever the window is shown again.
                let _ = window.hide();
                set_dock_visible(window.app_handle(), false);
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!());

    if let Err(e) = result {
        eprintln!("[etlp] fatal: failed to run application: {e}");
        std::process::exit(1);
    }
}

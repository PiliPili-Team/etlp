//! Tauri application entry point for etlp GUI.

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
    let log_file = data_dir.join("etlp.log");

    // Read the config early so we can honour dev.log_level from the very first
    // log line.  Failures are silently ignored here — the server will re-read
    // and report the error when it starts up.
    let initial_log_level = etlp_server::platform::config_dir()
        .and_then(|d| etlp_config::Config::load_from_dir(&d).ok())
        .map(|c| c.dev.log_level.clone())
        .unwrap_or_else(|| "info".to_owned());

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

    // Decode the monochrome PNG to raw RGBA at startup.
    let tray_icon_bytes: &[u8] = include_bytes!("../icons/tray-icon.png");
    let tray_rgba = {
        use image::ImageDecoder as _;
        let cursor = std::io::Cursor::new(tray_icon_bytes);
        let decoder = image::codecs::png::PngDecoder::new(cursor)
            .expect("tray-icon.png is a valid PNG");
        let (w, h) = decoder.dimensions();
        let mut buf = vec![0u8; decoder.total_bytes() as usize];
        decoder.read_image(&mut buf).expect("decode tray icon");
        (buf, w, h)
    };

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

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::AppleScript,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(gui_state)
        .setup(move |app| {
            let (tray_buf, tray_w, tray_h) = tray_rgba;

            // ── Tray icon ──────────────────────────────────────────────────────
            let tray_img =
                tauri::image::Image::new_owned(tray_buf, tray_w, tray_h);
            let menu = build_tray_menu(app.handle(), &labels, false)?;

            let _tray = TrayIconBuilder::new()
                .icon(tray_img)
                .icon_as_template(true)
                .tooltip(labels.tooltip)
                .menu(&menu)
                .on_menu_event(|app, event| {
                    let state = app.state::<GuiState>();
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
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
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
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
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
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
                let _ = window.show();
                let _ = window.set_focus();
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
            commands::get_log_lines,
            commands::clear_log_position,
            commands::open_log_folder,
            commands::get_log_paths,
            commands::pick_player_path,
            commands::path_exists,
            commands::get_app_version,
            commands::list_system_fonts,
            commands::set_autostart,
            commands::get_autostart,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running etlp");
}

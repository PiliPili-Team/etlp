//! Tauri application entry point for etlp GUI.

pub mod commands;
pub mod config_patch;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use commands::GuiState;

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

    let masker = etlp_logging::Masker::new(false);
    etlp_logging::init(masker, "info", Some(log_file.as_path())).ok();

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
        // Store the resolved log file path so get_log_lines knows where to read.
        if let Ok(mut lf) = s.log_file.lock() {
            *lf = log_file;
        }
        s
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .manage(gui_state)
        .setup(move |app| {
            let (tray_buf, tray_w, tray_h) = tray_rgba;
            let show =
                MenuItemBuilder::with_id("show", "Show Window").build(app)?;
            let hide =
                MenuItemBuilder::with_id("hide", "Hide Window").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .items(&[&show, &hide, &quit])
                .build()?;

            let tray_img = tauri::image::Image::new_owned(tray_buf, tray_w, tray_h);

            let _tray = TrayIconBuilder::new()
                .icon(tray_img)
                .icon_as_template(true)
                .tooltip("etlp")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::TrayIconEvent;
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::get_server_status,
            commands::get_config,
            commands::update_config_field,
            commands::reload_config,
            commands::open_config_folder,
            commands::edit_config,
            commands::get_log_lines,
            commands::clear_log_position,
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

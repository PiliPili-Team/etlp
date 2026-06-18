//! Tauri application entry point for etlp GUI.
//!
//! Wires up plugins, the system tray/menu-bar icon, the managed state,
//! and all command handlers, then calls `Builder::run`.

pub mod commands;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use commands::GuiState;

pub fn run() {
    // Mark this process as the packaged GUI app so platform code can query
    // RuntimeMode::detect(). Called here, before Builder spawns any threads.
    // SAFETY: single-threaded at this point; no concurrent env reads.
    unsafe { std::env::set_var(etlp_server::platform::ENV_RUNTIME, "app") };

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
        .manage(GuiState::default())
        .setup(|app| {
            let show =
                MenuItemBuilder::with_id("show", "Show Window").build(app)?;
            let hide =
                MenuItemBuilder::with_id("hide", "Hide Window").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .items(&[&show, &hide, &quit])
                .build()?;

            let _tray = TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .ok_or("no default icon")?,
                )
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
            commands::reload_config,
            commands::open_config_folder,
            commands::edit_config,
            commands::set_autostart,
            commands::get_autostart,
        ])
        .on_window_event(|window, event| {
            // Keep the app alive in the tray when the user closes the window.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running etlp");
}

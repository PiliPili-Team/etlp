//! Tauri application entry point for etlp GUI.

pub mod backup;
pub mod commands;
pub mod config_patch;

use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use commands::GuiState;

// ── Language helpers ──────────────────────────────────────────────────────────

/// Detects whether the system UI language is Chinese using platform-native
/// APIs (via sys-locale). Falls back to LANG/LC_ALL/LANGUAGE env vars for
/// environments where the native API is unavailable.
pub fn sys_is_chinese() -> bool {
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
    reload: &'static str,
    about: &'static str,
    quit: &'static str,
}

impl TrayLabels {
    fn detect() -> Self {
        if sys_is_chinese() {
            Self {
                tooltip: "原神",
                show: "显示主窗口",
                reload: "重载配置",
                about: "关于",
                quit: "退出",
            }
        } else {
            Self {
                tooltip: "Genshin",
                show: "Show Window",
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

/// The tray icon asset and whether it should render as a monochrome template.
///
/// macOS and Linux menu bars use the Liyue emblem: macOS gets a black *template*
/// image that the system recolours to match the light/dark menu bar, while Linux
/// (which has no template support) gets the white emblem, kept visible on the
/// commonly dark panel. Windows has no template support either and a silhouette
/// renders nearly invisible on its taskbar, so it keeps the full-colour app
/// logo — the squircle-rounded variant so it matches the rounded app icon.
fn tray_icon_asset() -> (&'static [u8], bool) {
    #[cfg(target_os = "macos")]
    {
        (include_bytes!("../icons/tray-icon.png"), true)
    }
    #[cfg(target_os = "linux")]
    {
        (include_bytes!("../icons/tray-icon-linux.png"), false)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        (include_bytes!("../icons/tray-icon-windows.png"), false)
    }
}

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
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let about = MenuItemBuilder::with_id("about", labels.about).build(app)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let show = MenuItemBuilder::with_id("show", labels.show).build(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let reload =
        MenuItemBuilder::with_id("reload", labels.reload).build(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", labels.quit).build(app)?;

    MenuBuilder::new(app)
        .items(&[&about, &sep1, &show, &sep2, &reload, &sep3, &quit])
        .build()
}

/// Show a best-effort system notification. Failures (e.g. permissions denied)
/// are ignored so notification problems never affect the app.
fn notify(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt as _;
    let _ = app.notification().builder().title(title).body(body).show();
}

/// On Windows Portable, re-register the autostart entry to the current exe
/// path so moving the portable directory doesn't break launch-at-login.
///
/// `tauri-plugin-autostart` stores the exe path in the registry when `enable()`
/// is called. Re-calling `enable()` on each launch keeps the stored path in sync
/// with the exe's actual location. No-op on non-Windows or non-portable builds.
#[cfg(target_os = "windows")]
fn refresh_portable_autostart(app: &tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt as _;
    if !etlp_server::platform::is_portable() {
        return;
    }
    let launcher = app.autolaunch();
    if let Ok(true) = launcher.is_enabled()
        && let Err(e) = launcher.enable()
    {
        eprintln!("[etlp] portable autostart path refresh failed: {e}");
    }
}

#[cfg(not(target_os = "windows"))]
fn refresh_portable_autostart(_app: &tauri::AppHandle) {}

/// One-time migration from the legacy AppleScript launch-at-login backend to
/// LaunchAgent.
///
/// The AppleScript backend could neither disable itself reliably (`osascript`
/// returned status 1) nor read its own state, so the preference was mirrored in
/// `gui.autostart`. LaunchAgent fixes both, so this:
///   1. carries a saved `autostart = true` forward by registering the agent,
///   2. best-effort removes any stale login item the AppleScript backend left,
///   3. drops the now-redundant `gui.autostart` key from the config.
///
/// A marker file guards the migration so it runs at most once. Every step is
/// best-effort: failures are logged (and surfaced to the user for the login-item
/// cleanup) but never block startup.
fn migrate_autostart(app: &tauri::AppHandle, legacy_pref: bool) {
    let Some(cfg_dir) = etlp_server::platform::config_dir() else {
        return;
    };
    let marker = cfg_dir.join(".autostart_launchagent_migrated");
    if marker.exists() {
        return;
    }

    // 1. Carry a saved "on" preference forward to the reliable backend.
    if legacy_pref {
        use tauri_plugin_autostart::ManagerExt;
        let launcher = app.autolaunch();
        if !launcher.is_enabled().unwrap_or(false)
            && let Err(e) = launcher.enable()
        {
            eprintln!("[etlp] autostart migrate enable failed: {e}");
        }
    }

    // 2. Remove any stale login item the AppleScript backend left behind.
    if let Err(e) = remove_legacy_login_item() {
        eprintln!("[etlp] autostart legacy login-item cleanup failed: {e}");
        let (title, body) = if sys_is_chinese() {
            (
                "开机自启方式已更新",
                "若“系统设置 → 通用 → 登录项”中仍残留 Genshin 项，请手动移除。",
            )
        } else {
            (
                "Launch-at-login updated",
                "If a stale \"Genshin\" entry remains under System Settings → \
                 General → Login Items, please remove it manually.",
            )
        };
        notify(app, title, body);
    }

    // 3. Drop the redundant persisted preference; OS state is now the truth.
    let path = etlp_config::existing_config_path(&cfg_dir)
        .unwrap_or_else(|| cfg_dir.join("config.toml"));
    if path.exists()
        && let Err(e) = config_patch::patch_field(
            &path,
            "gui",
            "autostart",
            &serde_json::Value::Null,
        )
    {
        eprintln!("[etlp] autostart config strip failed: {e}");
    }

    // 4. Record completion so the migration runs only once.
    if let Err(e) = std::fs::write(&marker, b"1") {
        eprintln!("[etlp] autostart migration marker write failed: {e}");
    }
}

/// Remove the legacy macOS login item the AppleScript autostart backend
/// registered, if present.
///
/// The `exists` guard makes the call succeed whether or not the item is there;
/// it errors only when Automation permission for System Events is denied — the
/// case the caller surfaces to the user. No-op on non-macOS platforms.
#[cfg(target_os = "macos")]
fn remove_legacy_login_item() -> std::io::Result<()> {
    let status = std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\"")
        .arg("-e")
        .arg(
            "if exists login item \"Genshin\" then \
             delete login item \"Genshin\"",
        )
        .arg("-e")
        .arg("end tell")
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "osascript exited with {status}"
        )))
    }
}

/// No-op stub on platforms without a legacy AppleScript login item.
#[cfg(not(target_os = "macos"))]
fn remove_legacy_login_item() -> std::io::Result<()> {
    Ok(())
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
        // Restore from a minimised state first: `show()` only un-hides a
        // hidden window and is a no-op while the window is minimised, so a
        // tray click would otherwise fail to bring a minimised window back.
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(target_os = "macos")]
fn is_macos_26_or_newer() -> bool {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|version| version.split('.').next()?.parse::<u32>().ok())
        .is_some_and(|major| major >= 26)
}

#[cfg(target_os = "macos")]
fn apply_liquid_glass(window: &tauri::WebviewWindow) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAppKitVersionNumber, NSAutoresizingMaskOptions, NSColor,
        NSGlassEffectView, NSGlassEffectViewStyle, NSView,
        NSWindowOrderingMode,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // SAFETY: AppKit exports this process-wide read-only version number.
    let appkit_version = unsafe { NSAppKitVersionNumber };
    if appkit_version < 2685.0 {
        return Err("Liquid Glass requires macOS 26.0 or newer".into());
    }

    let RawWindowHandle::AppKit(handle) = window
        .window_handle()
        .map_err(|e| format!("window handle: {e}"))?
        .as_raw()
    else {
        return Err("Liquid Glass requires an AppKit window".into());
    };

    let mtm = MainThreadMarker::new().ok_or_else(|| {
        "Liquid Glass must be applied on the main thread".to_string()
    })?;

    unsafe {
        let view: &NSView = handle.ns_view.cast().as_ref();
        let glass =
            NSGlassEffectView::initWithFrame(mtm.alloc(), view.bounds());
        let tint = NSColor::colorWithRed_green_blue_alpha(
            242.0 / 255.0,
            242.0 / 255.0,
            247.0 / 255.0,
            0.50,
        );

        glass.setStyle(NSGlassEffectViewStyle::Regular);
        glass.setCornerRadius(12.0);
        glass.setTintColor(Some(&tint));
        glass.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        view.addSubview_positioned_relativeTo(
            &glass,
            NSWindowOrderingMode::Below,
            None,
        );
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn apply_window_material(window: &tauri::WebviewWindow) {
    use window_vibrancy::{NSVisualEffectMaterial, apply_vibrancy};

    if is_macos_26_or_newer() {
        match apply_liquid_glass(window) {
            Ok(()) => return,
            Err(e) => eprintln!("[etlp] liquid glass: {e}"),
        }
    }

    apply_vibrancy(window, NSVisualEffectMaterial::Sidebar, None, Some(12.0))
        .unwrap_or_else(|e| eprintln!("[etlp] vibrancy: {e}"));
}

#[cfg(target_os = "windows")]
fn apply_window_material(window: &tauri::WebviewWindow) {
    // Match the macOS translucent sidebar treatment with native Windows
    // acrylic. Blur is a compatibility fallback for older Windows builds.
    window_vibrancy::apply_acrylic(window, Some((242, 242, 247, 160)))
        .or_else(|_| {
            window_vibrancy::apply_blur(window, Some((242, 242, 247, 120)))
        })
        .unwrap_or_else(|e| eprintln!("[etlp] acrylic: {e}"));
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn apply_window_material(_window: &tauri::WebviewWindow) {}

// ── Windows UAC elevation ─────────────────────────────────────────────────────

/// Check whether `portable.bin` is present alongside the exe but the directory
/// is not writable, and if so relaunch the process via the UAC "runas" verb.
///
/// Returns `true` if an elevated copy was launched (caller should exit).
/// Returns `false` when elevation is not needed or the user cancelled UAC.
#[cfg(target_os = "windows")]
fn try_request_elevation() -> bool {
    use std::os::windows::ffi::OsStrExt as _;
    use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{PCWSTR, w};

    // Only intervene when portable mode was requested but is not active
    // (i.e. portable.bin marker exists but exe dir is not writable).
    if !etlp_server::platform::portable_requested() {
        return false;
    }
    if etlp_server::platform::is_portable() {
        return false; // already writable — nothing to do
    }
    // Already elevated but still can't write — a different problem; don't loop.
    // SAFETY: returns BOOL with no preconditions.
    if unsafe { IsUserAnAdmin() }.as_bool() {
        return false;
    }

    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let mut exe_wide: Vec<u16> = exe.as_os_str().encode_wide().collect();
    exe_wide.push(0);

    // Reconstruct the argument string (skip argv[0]).
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_joined = args.join(" ");
    let args_wide: Vec<u16> = if args_joined.is_empty() {
        Vec::new()
    } else {
        let mut v: Vec<u16> = args_joined.encode_utf16().collect();
        v.push(0);
        v
    };

    // SAFETY: exe_wide and args_wide are valid null-terminated UTF-16 buffers
    // that outlive the call; all other parameters are constants or null.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            PCWSTR(exe_wide.as_ptr()),
            if args_wide.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(args_wide.as_ptr())
            },
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns a value > 32 on success.
    result.0.addr() > 32
}

#[cfg(not(target_os = "windows"))]
fn try_request_elevation() -> bool {
    false
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run() {
    augment_path();

    // Windows Portable: if portable.bin is present but the exe directory is not
    // writable, request UAC elevation and relaunch. The elevated copy will have
    // write access; this process exits immediately after the UAC prompt.
    // If the user cancels UAC the app continues normally with the %APPDATA%
    // directory fallback (no data is lost — config never existed there yet).
    if try_request_elevation() {
        std::process::exit(0);
    }

    // SAFETY: single-threaded at this point.
    unsafe { std::env::set_var(etlp_server::platform::ENV_RUNTIME, "app") };

    // Announce portable mode via environment variable so child processes
    // (e.g. updater.exe) can inherit the layout information.
    if etlp_server::platform::is_portable() {
        // SAFETY: single-threaded at this point.
        unsafe { std::env::set_var(etlp_server::platform::ENV_PORTABLE, "1") };
    }

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
    let log_file = log_dir.join(commands::APP_LOG_FILE);

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
    // Saved launch-at-login preference; reconciled against the OS in `setup` so
    // the persisted value stays the source of truth even if the registration
    // drifted (e.g. the macOS backend that cannot read its own state back).
    let want_autostart = initial_config
        .as_ref()
        .map(|c| c.gui.autostart)
        .unwrap_or(false);

    let rotation = initial_config
        .as_ref()
        .map(|c| {
            etlp_logging::LogRotation::from_mb(
                c.dev.log_max_size_mb,
                c.dev.log_max_files,
            )
        })
        .unwrap_or_default();
    let masker = etlp_logging::Masker::new(false);
    let log_handle = etlp_logging::init(
        masker,
        &initial_log_level,
        Some(log_file.as_path()),
        rotation,
    )
    .ok();

    if let Some(d) = etlp_server::platform::config_dir() {
        eprintln!("[etlp] config dir: {}", d.display());
    }
    eprintln!("[etlp] data   dir: {}", data_dir.display());
    eprintln!("[etlp] log    file: {}", log_file.display());

    // Decode the tray PNG to raw RGBA at startup. A decode failure must not
    // crash the app — the tray simply launches without a custom icon. macOS
    // uses a monochrome template; Windows/Linux use the colour app logo.
    let (tray_icon_bytes, tray_is_template) = tray_icon_asset();
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
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .manage(gui_state)
        .setup(move |app| {
            // Migrate the legacy AppleScript launch-at-login backend to
            // LaunchAgent (best-effort, runs once). LaunchAgent reports its own
            // state reliably, so afterwards the OS registration is the single
            // source of truth and there is no per-startup reconcile.
            migrate_autostart(app.handle(), want_autostart);

            // Keep the portable autostart registry entry pointing at the
            // current exe location (no-op on non-Windows / non-portable).
            refresh_portable_autostart(app.handle());

            // ── Tray icon ──────────────────────────────────────────────────────
            let menu = build_tray_menu(app.handle(), &labels)?;

            let mut tray_builder = TrayIconBuilder::new();
            if let Some((tray_buf, tray_w, tray_h)) = tray_rgba {
                let tray_img =
                    tauri::image::Image::new_owned(tray_buf, tray_w, tray_h);
                tray_builder = tray_builder
                    .icon(tray_img)
                    .icon_as_template(tray_is_template);
            }

            let _tray = tray_builder
                .tooltip(labels.tooltip)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main_window(app),
                    "reload" => {
                        let app_c = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app_c.state::<GuiState>();
                            let _ = commands::restart_server(state).await;
                        });
                    }
                    "about" => {
                        show_main_window(app);
                        app.emit("show-about", ()).ok();
                    }
                    "quit" => app.exit(0),
                    _ => {}
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
                // ── Window material ───────────────────────────────────────────
                // macOS uses window-vibrancy directly instead of Tauri's
                // set_effects(): Tauri's NSVisualEffectView setup changes
                // hit-testing and breaks the custom drag region. On macOS 26+
                // this applies NSGlassEffectView, falling back to vibrancy on
                // older systems or when the runtime rejects Liquid Glass.
                apply_window_material(&window);
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
            commands::pick_folder,
            commands::path_exists,
            commands::default_download_dir,
            commands::get_app_version,
            commands::check_update,
            commands::download_and_apply_update,
            commands::check_is_portable,
            commands::list_system_fonts,
            commands::set_autostart,
            commands::get_autostart,
            commands::validate_bangumi_mapping,
            commands::export_bangumi_map,
            commands::import_bangumi_map,
            commands::import_bangumi_map_url,
            commands::validate_regex,
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

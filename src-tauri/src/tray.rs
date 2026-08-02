#[cfg(any(target_os = "macos", target_os = "windows"))]
use tauri::Manager;
use tauri::{App, AppHandle};

#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_ID: &str = "codex-usage-tray";
const TRAY_WINDOW_WIDTH: f64 = 400.0;
const TRAY_LOADING_HEIGHT: f64 = 120.0;
const TRAY_MAX_HEIGHT: f64 = 620.0;

#[cfg(any(target_os = "macos", target_os = "windows"))]
const MENU_CHECK_UPDATE: &str = "tray-check-update";
#[cfg(any(target_os = "macos", target_os = "windows"))]
const MENU_QUIT: &str = "tray-quit";

/// Pinned popups survive focus loss; the flag lives in memory only, so a
/// restart always begins unpinned.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static TRAY_PINNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Last physical window position, kept in memory only (per user request:
/// remembered while the app lives, forgotten on exit). Updated by both user
/// drags and programmatic centering, so "remembered" means "where it last was".
#[cfg(any(target_os = "macos", target_os = "windows"))]
static TRAY_POSITION: std::sync::Mutex<Option<(i32, i32)>> = std::sync::Mutex::new(None);

#[tauri::command]
pub fn set_usage_tray_pinned(pinned: bool) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    TRAY_PINNED.store(pinned, std::sync::atomic::Ordering::Relaxed);

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = pinned;
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn remembered_position() -> Option<(i32, i32)> {
    TRAY_POSITION.lock().ok().and_then(|stored| *stored)
}

/// Event emitted to the main window when the tray menu "Check for Updates"
/// item is clicked. The frontend opens About and runs the existing updater flow.
pub const TRAY_CHECK_UPDATES_EVENT: &str = "tray-check-updates";

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct TrayLabels {
    tooltip: &'static str,
    check_update: &'static str,
    quit: &'static str,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn tray_labels(locale: &str) -> TrayLabels {
    if locale.to_ascii_lowercase().starts_with("zh") {
        TrayLabels {
            tooltip: "监控面板",
            check_update: "检查更新",
            quit: "退出",
        }
    } else {
        TrayLabels {
            tooltip: "Monitor Panel",
            check_update: "Check for Updates",
            quit: "Quit",
        }
    }
}

/// Owned tray menu items so locale switches can rewrite their titles in place.
#[cfg(any(target_os = "macos", target_os = "windows"))]
struct TrayMenuHandles {
    check_update: tauri::menu::MenuItem<tauri::Wry>,
    quit: tauri::menu::MenuItem<tauri::Wry>,
}

#[tauri::command]
pub fn resize_usage_tray(app: AppHandle, height: f64) {
    let height = height.clamp(TRAY_LOADING_HEIGHT, TRAY_MAX_HEIGHT);

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    resize_centered_on_current_monitor(&app, height);

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;
}

/// Open the usage popup from the main window (sidebar button). Same window,
/// same behavior as a tray-icon click: remembered position wins, otherwise it
/// centers on the main window's monitor at compact loading height.
#[tauri::command]
pub fn open_usage_tray(app: AppHandle) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use tauri::{Emitter, LogicalSize, PhysicalPosition, Position};

        if let Some(window) = app.get_webview_window("codex-usage") {
            let _ = window.set_size(LogicalSize::new(TRAY_WINDOW_WIDTH, TRAY_LOADING_HEIGHT));
            if let Some((x, y)) = remembered_position() {
                let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
            } else {
                let monitor = app
                    .get_webview_window("main")
                    .and_then(|main| main.current_monitor().ok().flatten())
                    .or_else(|| window.current_monitor().ok().flatten());
                if let Some(monitor) = monitor {
                    position_on_monitor(&window, &monitor, TRAY_LOADING_HEIGHT);
                }
            }
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.emit("usage-tray-opened", ());
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;
}

/// Refresh native tray menu / tooltip strings after the user switches language.
pub fn apply_locale(app: &AppHandle, locale: &str) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let labels = tray_labels(locale);
        if let Some(handles) = app.try_state::<TrayMenuHandles>() {
            let _ = handles.check_update.set_text(labels.check_update);
            let _ = handles.quit.set_text(labels.quit);
        }
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_tooltip(Some(labels.tooltip));
        }
        if let Some(window) = app.get_webview_window("codex-usage") {
            let _ = window.set_title(labels.tooltip);
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (app, locale);
    }
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    setup_desktop(app)?;

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn current_locale_tag(app: &App) -> String {
    app.try_state::<std::sync::Mutex<crate::state::AppState>>()
        .and_then(|state| state.lock().ok().map(|s| s.locale.tag().to_string()))
        .unwrap_or_else(|| crate::i18n::Locale::detect().tag().to_string())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn setup_desktop(app: &mut App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::{Emitter, WebviewUrl, WebviewWindowBuilder, WindowEvent};

    let labels = tray_labels(&current_locale_tag(app));

    let mut window_builder = WebviewWindowBuilder::new(
        app,
        "codex-usage",
        WebviewUrl::App("index.html?view=codex-usage".into()),
    )
    .title(labels.tooltip)
    .inner_size(TRAY_WINDOW_WIDTH, TRAY_LOADING_HEIGHT)
    .center()
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .transparent(true)
    .visible(false);

    #[cfg(target_os = "macos")]
    {
        window_builder = window_builder.visible_on_all_workspaces(true);
    }

    let window = window_builder.build()?;

    let window_to_hide = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            if !TRAY_PINNED.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = window_to_hide.hide();
            }
        }
        WindowEvent::Moved(position) => {
            if let Ok(mut stored) = TRAY_POSITION.lock() {
                *stored = Some((position.x, position.y));
            }
        }
        _ => {}
    });

    // Right-click context menu: Check for Updates + Quit.
    // Left click remains reserved for the usage popup (show_menu_on_left_click=false).
    let check_update =
        MenuItem::with_id(app, MENU_CHECK_UPDATE, labels.check_update, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&check_update, &quit])?;
    app.manage(TrayMenuHandles {
        check_update: check_update.clone(),
        quit: quit.clone(),
    });

    // Tray icon: monochrome cutout logo (transparent gaps), embedded as raw
    // RGBA to avoid the image-png feature. Rendered as a macOS template so the
    // silhouette adapts to light/dark menu bars.
    let icon = tauri::image::Image::new(include_bytes!("../icons/tray-icon.rgba"), 128, 128);

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID).icon(icon);
    #[cfg(target_os = "macos")]
    {
        tray_builder = tray_builder.icon_as_template(true);
    }

    tray_builder
        .menu(&menu)
        .tooltip(labels.tooltip)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                MENU_CHECK_UPDATE => {
                    // Hide the usage popup if open, then hand off to the main
                    // window's existing About / updater UI.
                    if let Some(usage) = app.get_webview_window("codex-usage") {
                        let _ = usage.hide();
                    }
                    if let Some(main) = app.get_webview_window("main") {
                        let _ = main.show();
                        let _ = main.unminimize();
                        let _ = main.set_focus();
                        let _ = main.emit(TRAY_CHECK_UPDATES_EVENT, ());
                    }
                }
                MENU_QUIT => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            else {
                return;
            };

            let app = tray.app_handle().clone();
            if let Some(window) = app.get_webview_window("codex-usage") {
                // Reuse the in-memory position when we have one (the user may
                // have dragged the popup); only the very first open centers.
                if let Some((x, y)) = remembered_position() {
                    use tauri::{LogicalSize, PhysicalPosition, Position};
                    let _ = window.set_size(LogicalSize::new(TRAY_WINDOW_WIDTH, TRAY_LOADING_HEIGHT));
                    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
                } else {
                    resize_and_position_window(&app, &window, rect, TRAY_LOADING_HEIGHT);
                }
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.emit("usage-tray-opened", ());
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn resize_and_position_window(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    rect: tauri::Rect,
    height: f64,
) {
    use tauri::LogicalSize;

    // Resizing must never depend on monitor detection. On macOS the tray rect
    // and window coordinates can use different scale spaces; a failed monitor
    // lookup previously left the popup stuck at its compact loading height.
    let _ = window.set_size(LogicalSize::new(TRAY_WINDOW_WIDTH, height));

    let window_scale = window.scale_factor().unwrap_or(1.0);
    let tray_position = rect.position.to_physical::<f64>(window_scale);
    let tray_size = rect.size.to_physical::<f64>(window_scale);

    let center_x = tray_position.x + tray_size.width / 2.0;
    let center_y = tray_position.y + tray_size.height / 2.0;
    let monitor = app
        .monitor_from_point(center_x, center_y)
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        position_on_monitor(window, &monitor, height);
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn resize_centered_on_current_monitor(app: &AppHandle, height: f64) {
    use tauri::LogicalSize;

    let Some(window) = app.get_webview_window("codex-usage") else {
        return;
    };

    // Apply the requested content size first and unconditionally. Re-centering
    // is secondary and may legitimately be unavailable during a window resize.
    // A remembered position (dragged or previously centered) wins over
    // re-centering so resizing never yanks the popup back to screen center.
    let _ = window.set_size(LogicalSize::new(TRAY_WINDOW_WIDTH, height));
    if remembered_position().is_none() {
        if let Ok(Some(monitor)) = window.current_monitor() {
            position_on_monitor(&window, &monitor, height);
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn position_on_monitor(window: &tauri::WebviewWindow, monitor: &tauri::Monitor, height: f64) {
    use tauri::{PhysicalPosition, Position};

    let scale = monitor.scale_factor();
    let window_width = TRAY_WINDOW_WIDTH * scale;
    let window_height = height * scale;
    let (x, y) = centered_window_position(
        window_width,
        window_height,
        PhysicalBounds {
            x: monitor.position().x as f64,
            y: monitor.position().y as f64,
            width: monitor.size().width as f64,
            height: monitor.size().height as f64,
        },
    );

    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
#[derive(Clone, Copy)]
struct PhysicalBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn centered_window_position(
    window_width: f64,
    window_height: f64,
    screen: PhysicalBounds,
) -> (i32, i32) {
    let x = screen.x + (screen.width - window_width).max(0.0) / 2.0;
    let y = screen.y + (screen.height - window_height).max(0.0) / 2.0;
    (x.round() as i32, y.round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_window_is_centered_on_the_active_monitor() {
        let screen = PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: 3_024.0,
            height: 1_964.0,
        };
        let position = centered_window_position(800.0, 1_040.0, screen);
        assert_eq!(position, (1_112, 462));
    }

    #[test]
    fn tray_window_centers_on_a_monitor_with_a_nonzero_origin() {
        let screen = PhysicalBounds {
            x: 3_024.0,
            y: -120.0,
            width: 1_920.0,
            height: 1_080.0,
        };
        let position = centered_window_position(800.0, 1_040.0, screen);
        assert_eq!(position, (3_584, -100));
    }
}

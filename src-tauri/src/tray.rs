use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tauri::Manager;
use tauri::{App, AppHandle};

use crate::switch::commands::{
    get_codex_reset_credits, get_codex_usage, CodexResetCreditsResponse, CodexUsageResponse,
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_ID: &str = "codex-usage-tray";
const TRAY_WINDOW_WIDTH: f64 = 400.0;
const TRAY_LOADING_HEIGHT: f64 = 120.0;
const TRAY_MAX_HEIGHT: f64 = 620.0;

#[derive(Clone, Serialize)]
pub struct CodexTraySnapshot {
    pub usage: CodexUsageResponse,
    pub reset_credits: Option<CodexResetCreditsResponse>,
    pub last_query_at: u64,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn refresh_snapshot() -> Result<CodexTraySnapshot, String> {
    let (usage_result, credits_result) =
        futures_util::future::join(get_codex_usage(), get_codex_reset_credits()).await;

    let usage = usage_result?;
    Ok(CodexTraySnapshot {
        usage,
        reset_credits: credits_result.ok(),
        last_query_at: unix_now(),
    })
}

#[tauri::command]
pub async fn get_codex_tray_usage() -> Result<CodexTraySnapshot, String> {
    refresh_snapshot().await
}

#[tauri::command]
pub fn resize_usage_tray(app: AppHandle, height: f64) {
    let height = height.clamp(TRAY_LOADING_HEIGHT, TRAY_MAX_HEIGHT);

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    resize_centered_on_current_monitor(&app, height);

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    setup_desktop(app)?;

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn setup_desktop(app: &mut App) -> tauri::Result<()> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::{Emitter, WebviewUrl, WebviewWindowBuilder, WindowEvent};

    let mut window_builder = WebviewWindowBuilder::new(
        app,
        "codex-usage",
        WebviewUrl::App("index.html?view=codex-usage".into()),
    )
    .title("用量查询")
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
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Focused(false)) {
            let _ = window_to_hide.hide();
        }
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
        .tooltip("用量查询")
        .show_menu_on_left_click(false)
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
                resize_and_position_window(&app, &window, rect, TRAY_LOADING_HEIGHT);
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
    let _ = window.set_size(LogicalSize::new(TRAY_WINDOW_WIDTH, height));
    if let Ok(Some(monitor)) = window.current_monitor() {
        position_on_monitor(&window, &monitor, height);
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

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::App;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tauri::{AppHandle, Manager};

use crate::switch::commands::{
    current_codex_account_id, get_codex_reset_credits, get_codex_usage, CodexResetCreditsResponse,
    CodexUsageResponse,
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_ID: &str = "codex-usage-tray";
const CACHE_TTL_SECONDS: u64 = 10 * 60;
#[cfg(target_os = "macos")]
const MAC_WINDOW_WIDTH: f64 = 420.0;
#[cfg(target_os = "macos")]
const MAC_WINDOW_HEIGHT: f64 = 500.0;

#[derive(Clone)]
struct CachedUsage {
    account_id: String,
    snapshot: CodexTraySnapshot,
}

#[derive(Default)]
struct CacheState {
    entry: Option<CachedUsage>,
    refreshing: bool,
}

#[derive(Default)]
pub struct CodexTrayState {
    cache: Mutex<CacheState>,
}

#[derive(Clone, Serialize)]
pub struct CodexTraySnapshot {
    pub usage: CodexUsageResponse,
    pub reset_credits: Option<CodexResetCreditsResponse>,
    pub last_query_at: u64,
    pub next_query_at: u64,
    pub cached: bool,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cached_snapshot(
    entry: &Option<CachedUsage>,
    account_id: &str,
    now: u64,
) -> Option<CodexTraySnapshot> {
    let cached = entry.as_ref()?;
    if cached.account_id != account_id || now >= cached.snapshot.next_query_at {
        return None;
    }
    let mut snapshot = cached.snapshot.clone();
    snapshot.cached = true;
    Some(snapshot)
}

async fn refresh_snapshot(state: &CodexTrayState) -> Result<CodexTraySnapshot, String> {
    let account_id = current_codex_account_id()?;
    let now = unix_now();

    {
        let mut cache = state.cache.lock().map_err(|e| e.to_string())?;
        if let Some(snapshot) = cached_snapshot(&cache.entry, &account_id, now) {
            return Ok(snapshot);
        }
        if cache.refreshing {
            if let Some(entry) = cache
                .entry
                .as_ref()
                .filter(|entry| entry.account_id.as_str() == account_id)
            {
                let mut snapshot = entry.snapshot.clone();
                snapshot.cached = true;
                return Ok(snapshot);
            }
            return Err("Codex 用量正在查询，请稍候".to_string());
        }
        cache.refreshing = true;
    }

    let (usage_result, credits_result) =
        futures_util::future::join(get_codex_usage(), get_codex_reset_credits()).await;

    let mut cache = state.cache.lock().map_err(|e| e.to_string())?;
    cache.refreshing = false;

    let usage = usage_result?;
    let queried_at = unix_now();
    let snapshot = CodexTraySnapshot {
        usage,
        reset_credits: credits_result.ok(),
        last_query_at: queried_at,
        next_query_at: queried_at + CACHE_TTL_SECONDS,
        cached: false,
    };
    cache.entry = Some(CachedUsage {
        account_id,
        snapshot: snapshot.clone(),
    });
    Ok(snapshot)
}

#[tauri::command]
pub async fn get_codex_tray_usage(
    state: tauri::State<'_, CodexTrayState>,
) -> Result<CodexTraySnapshot, String> {
    refresh_snapshot(&state).await
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = app;

    #[cfg(target_os = "macos")]
    setup_macos(app)?;

    #[cfg(target_os = "windows")]
    setup_windows(app)?;

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn spawn_refresh(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let result = {
            let state = app.state::<CodexTrayState>();
            refresh_snapshot(&state).await
        };

        #[cfg(target_os = "macos")]
        emit_macos_result(&app, result);

        #[cfg(target_os = "windows")]
        update_windows_menu(&app, result.as_ref().ok(), result.as_ref().err());
    });
}

#[cfg(target_os = "macos")]
fn setup_macos(app: &mut App) -> tauri::Result<()> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::{Emitter, WebviewUrl, WebviewWindowBuilder, WindowEvent};

    let window = WebviewWindowBuilder::new(
        app,
        "codex-usage",
        WebviewUrl::App("index.html?view=codex-usage".into()),
    )
    .title("Codex 用量")
    .inner_size(MAC_WINDOW_WIDTH, MAC_WINDOW_HEIGHT)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .decorations(false)
    .always_on_top(true)
    .visible_on_all_workspaces(true)
    .skip_taskbar(true)
    .shadow(true)
    .transparent(true)
    .visible(false)
    .build()?;

    let window_to_hide = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Focused(false)) {
            let _ = window_to_hide.hide();
        }
    });

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Codex 用量")
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
                position_macos_window(&app, &window, rect);
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.emit("codex-tray-loading", ());
            }
            spawn_refresh(app);
        })
        .build(app)?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn position_macos_window(app: &AppHandle, window: &tauri::WebviewWindow, rect: tauri::Rect) {
    use tauri::{PhysicalPosition, Position};

    let window_scale = window.scale_factor().unwrap_or(1.0);
    let tray_position = rect.position.to_physical::<f64>(window_scale);
    let tray_size = rect.size.to_physical::<f64>(window_scale);

    let center_x = tray_position.x + tray_size.width / 2.0;
    let center_y = tray_position.y + tray_size.height / 2.0;
    let Ok(Some(monitor)) = app.monitor_from_point(center_x, center_y) else {
        return;
    };
    let scale = monitor.scale_factor();
    let window_width = MAC_WINDOW_WIDTH * scale;
    let window_height = MAC_WINDOW_HEIGHT * scale;
    let (x, y) = anchored_window_position(
        center_x,
        tray_position.y + tray_size.height,
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

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Copy)]
struct PhysicalBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[cfg(any(target_os = "macos", test))]
fn anchored_window_position(
    tray_center_x: f64,
    tray_bottom_y: f64,
    window_width: f64,
    window_height: f64,
    screen: PhysicalBounds,
) -> (i32, i32) {
    const MARGIN: f64 = 8.0;
    const TRAY_GAP: f64 = 6.0;

    let min_x = screen.x + MARGIN;
    let max_x = screen.x + screen.width - window_width - MARGIN;
    let x = (tray_center_x - window_width / 2.0).clamp(min_x, max_x.max(min_x));

    let min_y = screen.y + TRAY_GAP;
    let max_y = screen.y + screen.height - window_height - MARGIN;
    let y = (tray_bottom_y + TRAY_GAP).clamp(min_y, max_y.max(min_y));
    (x.round() as i32, y.round() as i32)
}

#[cfg(target_os = "macos")]
fn emit_macos_result(app: &AppHandle, result: Result<CodexTraySnapshot, String>) {
    use tauri::Emitter;

    let Some(window) = app.get_webview_window("codex-usage") else {
        return;
    };
    match result {
        Ok(snapshot) => {
            let _ = window.emit("codex-tray-updated", snapshot);
        }
        Err(message) => {
            let _ = window.emit("codex-tray-error", message);
        }
    }
}

#[cfg(target_os = "windows")]
fn setup_windows(app: &mut App) -> tauri::Result<()> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let menu = build_windows_menu(app.handle(), None, None)?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Codex Usage")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                spawn_refresh(tray.app_handle().clone());
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn update_windows_menu(
    app: &AppHandle,
    snapshot: Option<&CodexTraySnapshot>,
    error: Option<&String>,
) {
    use tauri::tray::TrayIconId;

    let Ok(menu) = build_windows_menu(app, snapshot, error) else {
        return;
    };
    if let Some(tray) = app.tray_by_id(&TrayIconId::new(TRAY_ID)) {
        let _ = tray.set_menu(Some(menu));
        let _ = tray.with_inner_tray_icon(|inner| inner.show_menu());
    }
}

#[cfg(target_os = "windows")]
fn build_windows_menu(
    app: &AppHandle,
    snapshot: Option<&CodexTraySnapshot>,
    error: Option<&String>,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{IsMenuItem, Menu, MenuItem};

    let zh = app
        .state::<crate::state::SafeState>()
        .lock()
        .map(|state| state.locale == crate::i18n::Locale::ZhCn)
        .unwrap_or(true);
    let lines = windows_menu_lines(snapshot, error.map(String::as_str), zh);
    let items = lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            MenuItem::with_id(
                app,
                format!("codex-usage-{index}"),
                line,
                false,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let item_refs: Vec<&dyn IsMenuItem<tauri::Wry>> = items.iter().map(|item| item as _).collect();
    Menu::with_items(app, &item_refs)
}

#[cfg(any(target_os = "windows", test))]
fn windows_menu_lines(
    snapshot: Option<&CodexTraySnapshot>,
    error: Option<&str>,
    zh: bool,
) -> Vec<String> {
    if let Some(message) = error {
        let prefix = if zh { "查询失败" } else { "Query failed" };
        return vec![prefix.to_string(), message.to_string()];
    }
    let Some(snapshot) = snapshot else {
        return if zh {
            vec!["Codex 用量：尚未查询".into(), "重置卡：尚未查询".into()]
        } else {
            vec![
                "Codex usage: Not queried".into(),
                "Reset credit: Not queried".into(),
            ]
        };
    };

    let mut lines: Vec<String> = [
        snapshot.usage.primary_window.as_ref(),
        snapshot.usage.secondary_window.as_ref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|window| {
        window_label(window.window_seconds).map(|label| format_windows_window(&label, window, zh))
    })
    .collect();
    let available = snapshot
        .reset_credits
        .as_ref()
        .map(|credits| credits.available_count)
        .or_else(|| {
            snapshot
                .usage
                .reset_credits
                .as_ref()
                .map(|credits| credits.available_count)
        })
        .unwrap_or(0);
    let expires = snapshot
        .reset_credits
        .as_ref()
        .and_then(|credits| credits.next_expires_at.as_deref())
        .and_then(seconds_until_iso);
    let credit = if available == 0 {
        if zh {
            "重置卡：无可用".into()
        } else {
            "Reset credit: None available".into()
        }
    } else if let Some(seconds) = expires {
        if zh {
            format!(
                "重置卡：{} 张，{}后到期",
                available,
                format_duration(seconds, true)
            )
        } else {
            format!(
                "Reset credit: {available}, expires in {}",
                format_duration(seconds, false)
            )
        }
    } else if zh {
        format!("重置卡：{} 张，到期时间未知", available)
    } else {
        format!("Reset credit: {available}, expiry unknown")
    };

    lines.push(credit);
    lines
}

#[cfg(any(target_os = "windows", test))]
fn window_label(seconds: u64) -> Option<String> {
    if seconds == 0 {
        return None;
    }
    if seconds.abs_diff(18_000) <= 600 {
        return Some("5h".into());
    }
    if seconds.abs_diff(604_800) <= 3_600 {
        return Some("7d".into());
    }
    if seconds.abs_diff(2_592_000) <= 86_400 {
        return Some("30d".into());
    }
    if seconds >= 86_400 {
        return Some(format!("{}d", (seconds as f64 / 86_400.0).round() as u64));
    }
    Some(format!("{}h", (seconds as f64 / 3_600.0).round() as u64))
}

#[cfg(any(target_os = "windows", test))]
fn format_windows_window(
    label: &str,
    window: &crate::switch::commands::UsageWindow,
    zh: bool,
) -> String {
    if zh {
        format!(
            "{}: 已用 {}%，{}后重置",
            label,
            window.used_percent,
            format_duration(window.reset_after_seconds, true)
        )
    } else {
        format!(
            "{}: {}% used, resets in {}",
            label,
            window.used_percent,
            format_duration(window.reset_after_seconds, false)
        )
    }
}

#[cfg(any(target_os = "windows", test))]
fn seconds_until_iso(value: &str) -> Option<u64> {
    let expires = chrono::DateTime::parse_from_rfc3339(value)
        .ok()?
        .timestamp();
    Some(
        expires
            .saturating_sub(chrono::Utc::now().timestamp())
            .max(0) as u64,
    )
}

#[cfg(any(target_os = "windows", test))]
fn format_duration(seconds: u64, zh: bool) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if zh {
        if days > 0 {
            format!("{}天{}小时", days, hours)
        } else {
            format!("{}小时{}分钟", hours, minutes)
        }
    } else if days > 0 {
        format!("{days}d {hours}h")
    } else {
        format!("{hours}h {minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::switch::commands::ResetCredits;

    fn snapshot(next_query_at: u64) -> CodexTraySnapshot {
        CodexTraySnapshot {
            usage: CodexUsageResponse {
                plan_type: "plus".into(),
                primary_window: None,
                secondary_window: None,
                reset_credits: Some(ResetCredits { available_count: 1 }),
            },
            reset_credits: None,
            last_query_at: 100,
            next_query_at,
            cached: false,
        }
    }

    #[test]
    fn cache_is_scoped_to_account_and_ten_minute_window() {
        let entry = Some(CachedUsage {
            account_id: "account-a".into(),
            snapshot: snapshot(700),
        });

        assert!(cached_snapshot(&entry, "account-a", 699).unwrap().cached);
        assert!(cached_snapshot(&entry, "account-a", 700).is_none());
        assert!(cached_snapshot(&entry, "account-b", 200).is_none());
    }

    #[test]
    fn windows_text_menu_formats_usage_and_resets() {
        let mut data = snapshot(700);
        data.usage.primary_window = Some(crate::switch::commands::UsageWindow {
            used_percent: 39,
            remaining_percent: 61,
            reset_after_seconds: 7_740,
            reset_at: 0,
            window_seconds: 18_000,
        });
        data.usage.secondary_window = Some(crate::switch::commands::UsageWindow {
            used_percent: 61,
            remaining_percent: 39,
            reset_after_seconds: 291_600,
            reset_at: 0,
            window_seconds: 604_800,
        });

        let lines = windows_menu_lines(Some(&data), None, true);
        assert_eq!(lines[0], "5h: 已用 39%，2小时9分钟后重置");
        assert_eq!(lines[1], "7d: 已用 61%，3天9小时后重置");
        assert_eq!(lines[2], "重置卡：1 张，到期时间未知");
    }

    #[test]
    fn windows_text_menu_uses_real_window_duration_and_hides_missing_windows() {
        let mut data = snapshot(700);
        data.usage.primary_window = Some(crate::switch::commands::UsageWindow {
            used_percent: 60,
            remaining_percent: 40,
            reset_after_seconds: 518_832,
            reset_at: 0,
            window_seconds: 604_800,
        });

        let lines = windows_menu_lines(Some(&data), None, true);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "7d: 已用 60%，6天0小时后重置");
        assert_eq!(lines[1], "重置卡：1 张，到期时间未知");
        assert_eq!(window_label(18_000).as_deref(), Some("5h"));
        assert_eq!(window_label(2_592_000).as_deref(), Some("30d"));
        assert_eq!(window_label(0), None);
    }

    #[test]
    fn expired_credit_countdown_clamps_to_zero() {
        assert_eq!(seconds_until_iso("2000-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn mac_window_is_centered_below_tray_icon() {
        let screen = PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: 3_024.0,
            height: 1_964.0,
        };
        let position = anchored_window_position(1_000.0, 48.0, 840.0, 1_000.0, screen);
        assert_eq!(position, (580, 54));
    }

    #[test]
    fn mac_window_stays_inside_right_and_bottom_edges() {
        let screen = PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: 3_024.0,
            height: 1_964.0,
        };
        let position = anchored_window_position(2_990.0, 1_850.0, 840.0, 1_000.0, screen);
        assert_eq!(position, (2_176, 956));

        let left_edge = anchored_window_position(10.0, 48.0, 840.0, 1_000.0, screen);
        assert_eq!(left_edge, (8, 54));
    }
}

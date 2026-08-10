//! Shared (in-memory) usage-monitor settings, consumed by BOTH the main
//! window (Accounts view + sidebar settings modal) and the tray popup.
//! Two webviews cannot share Pinia state, so the source of truth lives here:
//! every setter emits `usage-monitor-settings-changed` with the full snapshot
//! and both windows re-apply it. Values are deliberately process-lifetime
//! only — a restart always begins with the defaults.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use tauri::{AppHandle, Emitter};

pub const SETTINGS_CHANGED_EVENT: &str = "usage-monitor-settings-changed";

pub const MIN_REFRESH_MINUTES: u32 = 1;
pub const MAX_REFRESH_MINUTES: u32 = 10;
const DEFAULT_REFRESH_MINUTES: u32 = 5;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMonitorSettings {
    pub refresh_minutes: u32,
    /// The agent selected in Accounts / tray provider tabs. `None` until the
    /// user picks one; frontends fall back to their localStorage preference.
    pub selected_agent: Option<String>,
    /// Per-agent listening switches. Absent key = enabled (default); the map
    /// only stores explicit choices.
    pub listening: HashMap<String, bool>,
}

impl Default for UsageMonitorSettings {
    fn default() -> Self {
        Self {
            refresh_minutes: DEFAULT_REFRESH_MINUTES,
            selected_agent: None,
            listening: HashMap::new(),
        }
    }
}

static SETTINGS: LazyLock<Mutex<UsageMonitorSettings>> =
    LazyLock::new(|| Mutex::new(UsageMonitorSettings::default()));

fn emit_settings(app: &AppHandle, settings: &UsageMonitorSettings) {
    let _ = app.emit(SETTINGS_CHANGED_EVENT, settings.clone());
}

#[tauri::command]
pub fn get_usage_monitor_settings() -> UsageMonitorSettings {
    SETTINGS.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn set_usage_refresh_minutes(app: AppHandle, minutes: u32) -> UsageMonitorSettings {
    let snapshot = {
        let mut settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        settings.refresh_minutes = minutes.clamp(MIN_REFRESH_MINUTES, MAX_REFRESH_MINUTES);
        settings.clone()
    };
    emit_settings(&app, &snapshot);
    snapshot
}

#[tauri::command]
pub fn set_usage_selected_agent(app: AppHandle, agent: Option<String>) -> UsageMonitorSettings {
    let snapshot = {
        let mut settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        settings.selected_agent = agent.filter(|value| !value.trim().is_empty());
        settings.clone()
    };
    emit_settings(&app, &snapshot);
    snapshot
}

#[tauri::command]
pub fn set_usage_agent_listening(
    app: AppHandle,
    agent: String,
    enabled: bool,
) -> UsageMonitorSettings {
    let snapshot = {
        let mut settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        settings.listening.insert(agent, enabled);
        settings.clone()
    };
    emit_settings(&app, &snapshot);
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_minutes_are_clamped_to_the_supported_range() {
        let mut settings = UsageMonitorSettings::default();
        settings.refresh_minutes = 0u32.clamp(MIN_REFRESH_MINUTES, MAX_REFRESH_MINUTES);
        assert_eq!(settings.refresh_minutes, 1);
        settings.refresh_minutes = 99u32.clamp(MIN_REFRESH_MINUTES, MAX_REFRESH_MINUTES);
        assert_eq!(settings.refresh_minutes, 10);
    }

    #[test]
    fn listening_defaults_to_enabled_for_untouched_agents() {
        let settings = UsageMonitorSettings::default();
        assert!(settings.listening.get("codex").copied().unwrap_or(true));
    }
}

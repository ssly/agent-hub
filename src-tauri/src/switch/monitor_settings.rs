//! Shared usage-monitor settings, consumed by BOTH the main window
//! (Accounts view + sidebar settings modal) and the tray popup.
//!
//! Two webviews cannot share Pinia state, so the source of truth lives here:
//! every setter emits `usage-monitor-settings-changed` with the full snapshot
//! and both windows re-apply it. The snapshot is also written to
//! `~/.agent-hub/usage-monitor.json` so a restart keeps the last choices.
//!
//! Per-agent listening defaults to **off**. Auto-query only starts after the
//! user turns the toggle on.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use tauri::{AppHandle, Emitter};

pub const SETTINGS_CHANGED_EVENT: &str = "usage-monitor-settings-changed";

pub const MIN_REFRESH_MINUTES: u32 = 1;
pub const MAX_REFRESH_MINUTES: u32 = 10;
const DEFAULT_REFRESH_MINUTES: u32 = 5;
const SETTINGS_FILE: &str = "usage-monitor.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMonitorSettings {
    #[serde(default = "default_refresh_minutes")]
    pub refresh_minutes: u32,
    /// The agent selected in Accounts / tray provider tabs. `None` until the
    /// user picks one; frontends fall back to their localStorage preference.
    #[serde(default)]
    pub selected_agent: Option<String>,
    /// Per-agent listening switches. Absent key = **disabled**; auto-query
    /// starts only after the user turns the toggle on.
    #[serde(default)]
    pub listening: HashMap<String, bool>,
}

fn default_refresh_minutes() -> u32 {
    DEFAULT_REFRESH_MINUTES
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
    LazyLock::new(|| Mutex::new(load_from_disk()));

fn persist_path() -> PathBuf {
    crate::paths::home_dir()
        .join(".agent-hub")
        .join(SETTINGS_FILE)
}

fn load_from_disk() -> UsageMonitorSettings {
    load_from_path(&persist_path())
}

fn load_from_path(path: &Path) -> UsageMonitorSettings {
    let Ok(raw) = fs::read_to_string(path) else {
        return UsageMonitorSettings::default();
    };
    let Ok(mut settings) = serde_json::from_str::<UsageMonitorSettings>(&raw) else {
        return UsageMonitorSettings::default();
    };
    settings.refresh_minutes = settings
        .refresh_minutes
        .clamp(MIN_REFRESH_MINUTES, MAX_REFRESH_MINUTES);
    if let Some(agent) = settings.selected_agent.as_ref() {
        if agent.trim().is_empty() {
            settings.selected_agent = None;
        }
    }
    settings
}

fn save_to_disk(settings: &UsageMonitorSettings) {
    let _ = save_to_path(&persist_path(), settings);
}

fn save_to_path(path: &Path, settings: &UsageMonitorSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("unable to create settings directory: {error}"))?;
    }
    let encoded = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("unable to serialize usage-monitor settings: {error}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, encoded)
        .map_err(|error| format!("unable to write usage-monitor settings: {error}"))?;
    crate::paths::replace_file(&tmp, path)
        .map_err(|error| format!("unable to replace usage-monitor settings: {error}"))?;
    Ok(())
}

fn emit_settings(app: &AppHandle, settings: &UsageMonitorSettings) {
    let _ = app.emit(SETTINGS_CHANGED_EVENT, settings.clone());
}

fn with_settings_mut(update: impl FnOnce(&mut UsageMonitorSettings)) -> UsageMonitorSettings {
    let snapshot = {
        let mut settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        update(&mut settings);
        settings.clone()
    };
    save_to_disk(&snapshot);
    snapshot
}

#[tauri::command]
pub fn get_usage_monitor_settings() -> UsageMonitorSettings {
    SETTINGS.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn set_usage_refresh_minutes(app: AppHandle, minutes: u32) -> UsageMonitorSettings {
    let snapshot = with_settings_mut(|settings| {
        settings.refresh_minutes = minutes.clamp(MIN_REFRESH_MINUTES, MAX_REFRESH_MINUTES);
    });
    emit_settings(&app, &snapshot);
    snapshot
}

#[tauri::command]
pub fn set_usage_selected_agent(app: AppHandle, agent: Option<String>) -> UsageMonitorSettings {
    let snapshot = with_settings_mut(|settings| {
        settings.selected_agent = agent.filter(|value| !value.trim().is_empty());
    });
    emit_settings(&app, &snapshot);
    snapshot
}

#[tauri::command]
pub fn set_usage_agent_listening(
    app: AppHandle,
    agent: String,
    enabled: bool,
) -> UsageMonitorSettings {
    let snapshot = with_settings_mut(|settings| {
        settings.listening.insert(agent, enabled);
    });
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
    fn listening_defaults_to_disabled_for_untouched_agents() {
        let settings = UsageMonitorSettings::default();
        assert!(!settings.listening.get("codex").copied().unwrap_or(false));
    }

    #[test]
    fn persist_round_trip_keeps_listening_and_interval() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("usage-monitor.json");
        let mut original = UsageMonitorSettings::default();
        original.refresh_minutes = 3;
        original.selected_agent = Some("codex".to_string());
        original.listening.insert("codex".to_string(), true);
        original.listening.insert("grok-build".to_string(), false);
        save_to_path(&path, &original).expect("save");
        let loaded = load_from_path(&path);
        assert_eq!(loaded.refresh_minutes, 3);
        assert_eq!(loaded.selected_agent.as_deref(), Some("codex"));
        assert_eq!(loaded.listening.get("codex").copied(), Some(true));
        assert_eq!(loaded.listening.get("grok-build").copied(), Some(false));
        assert!(!loaded.listening.get("kimi-code").copied().unwrap_or(false));
    }

    #[test]
    fn missing_or_invalid_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("nope.json");
        let missing_loaded = load_from_path(&missing);
        assert_eq!(missing_loaded.refresh_minutes, DEFAULT_REFRESH_MINUTES);
        assert!(missing_loaded.listening.is_empty());

        let junk = dir.path().join("junk.json");
        fs::write(&junk, "{not json").expect("write");
        let junk_loaded = load_from_path(&junk);
        assert_eq!(junk_loaded.refresh_minutes, DEFAULT_REFRESH_MINUTES);
        assert!(junk_loaded.selected_agent.is_none());
    }
}

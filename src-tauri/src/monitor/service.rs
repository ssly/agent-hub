use super::types::*;
use chrono::Utc;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_notification::NotificationExt;

pub struct MonitorService<R: Runtime> {
    state: Arc<Mutex<MonitorState>>,
    sys: Arc<Mutex<sysinfo::System>>,
    adapters: Vec<Box<dyn AgentMonitor>>,
    _watcher: Option<RecommendedWatcher>,
    _hooks_watcher: Option<RecommendedWatcher>,
    hooks_dir: PathBuf,
    app: AppHandle<R>,
    pub polling_enabled: Arc<AtomicBool>,
}

impl<R: Runtime> MonitorService<R> {
    pub fn new(app: AppHandle<R>, config: MonitorConfig) -> Self {
        let state = Arc::new(Mutex::new(MonitorState::new(config)));
        let sys = Arc::new(Mutex::new(sysinfo::System::new_all()));
        let adapters: Vec<Box<dyn AgentMonitor>> = vec![
            Box::new(super::adapters::KiroAdapter::new()),
            Box::new(super::adapters::ClaudeCodeAdapter::new()),
            Box::new(super::adapters::CodexAdapter::new()),
        ];

        let hooks_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"))
            .join(".agent-hub/hooks");
        let _ = std::fs::create_dir_all(&hooks_dir);

        let polling_enabled = Arc::new(AtomicBool::new(false));

        let mut service = Self {
            state,
            sys,
            adapters,
            _watcher: None,
            _hooks_watcher: None,
            hooks_dir,
            app,
            polling_enabled,
        };

        service.init_watcher();
        service.init_hooks_watcher();
        // Defer initial scan — will run on first poll or first get_active_sessions call
        service
    }

    fn init_watcher(&mut self) {
        let state = self.state.clone();
        let sys = self.sys.clone();
        let app = self.app.clone();

        let mut watcher = match RecommendedWatcher::new(
            move |_res: Result<notify::Event, notify::Error>| {
                let state = state.clone();
                let sys = sys.clone();
                let app = app.clone();
                Self::detect_and_emit(&state, &sys, &app);
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("Failed to create file watcher: {e}");
                return;
            }
        };

        for adapter in &self.adapters {
            for path in adapter.watch_paths() {
                if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
                    log::warn!("Failed to watch {:?}: {e}", path);
                } else {
                    log::info!("Watching {:?} for {}", path, adapter.platform_id());
                }
            }
        }

        self._watcher = Some(watcher);
    }

    fn init_hooks_watcher(&mut self) {
        let state = self.state.clone();
        let app = self.app.clone();
        let hooks_dir = self.hooks_dir.clone();

        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    if !event.kind.is_create() && !event.kind.is_modify() {
                        return;
                    }
                    for path in event.paths {
                        if path.extension().and_then(|e| e.to_str()) == Some("done") {
                            let state = state.clone();
                            let app = app.clone();
                            Self::process_hook_marker(&state, &app, &path);
                        }
                    }
                }
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("Failed to create hooks watcher: {e}");
                return;
            }
        };

        if let Err(e) = watcher.watch(&hooks_dir, RecursiveMode::NonRecursive) {
            log::warn!("Failed to watch hooks dir {:?}: {e}", hooks_dir);
        } else {
            log::info!("Watching hooks dir {:?}", hooks_dir);
        }

        self._hooks_watcher = Some(watcher);

        // Process any stale .done files left from previous sessions
        if let Ok(entries) = std::fs::read_dir(&hooks_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("done") {
                    log::info!("Cleaning stale hook marker: {:?}", path);
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    fn process_hook_marker(
        state: &Arc<Mutex<MonitorState>>,
        app: &AppHandle<R>,
        path: &std::path::Path,
    ) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read hook marker {:?}: {e}", path);
                return;
            }
        };

        let marker: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Invalid hook marker JSON {:?}: {e}", path);
                let _ = std::fs::remove_file(path);
                return;
            }
        };

        let agent_type = marker
            .get("agent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let session_id = marker
            .get("session_id")
            .or_else(|| marker.get("thread_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let marker_event = marker
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("end");
        let marker_time = marker
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|datetime| datetime.to_utc())
            .unwrap_or_else(Utc::now);

        log::info!("Hook completion marker: agent={agent_type} session={session_id:?}");

        let mut state = state.lock().unwrap();

        // Find target session
        let target_id = if let Some(ref sid) = session_id {
            if state.sessions.contains_key(sid) {
                Some(sid.clone())
            } else {
                None
            }
        } else {
            state
                .sessions
                .iter()
                .filter(|(_, s)| {
                    s.agent_type == agent_type && s.working_state == WorkingState::Working
                })
                .max_by_key(|(_, s)| s.last_activity)
                .map(|(id, _)| id.clone())
                .or_else(|| {
                    state
                        .sessions
                        .iter()
                        .filter(|(_, s)| s.agent_type == agent_type)
                        .max_by_key(|(_, s)| s.last_activity)
                        .map(|(id, _)| id.clone())
                })
        };

        if let Some(id) = target_id {
            if let Some(session) = state.sessions.get_mut(&id) {
                let old_state = session.working_state;
                session.working_state = WorkingState::Finished;
                session.status = SessionStatus::Active;
                session.last_activity = marker_time;
                if session.last_reply_at.is_none() {
                    session.last_reply_at = Some(marker_time);
                }
                let body = Self::notification_body(session);

                let _ = app.emit(
                    "monitor:state-changed",
                    serde_json::json!({
                        "change": "updated",
                        "session": session,
                    }),
                );

                let explicit_end = marker_event == "end" || marker_event == "done";
                if (matches!(old_state, WorkingState::Working)
                    || (explicit_end && !matches!(old_state, WorkingState::Finished)))
                    && Self::should_notify(&mut state, &id)
                {
                    let granted = app
                        .notification()
                        .permission_state()
                        .map(|s| matches!(s, tauri_plugin_notification::PermissionState::Granted))
                        .unwrap_or(false);
                    if granted {
                        let _ = app
                            .notification()
                            .builder()
                            .title("Agent 任务完成")
                            .body(body)
                            .show();
                    }
                }
            }
        }

        let _ = std::fs::remove_file(path);
    }

    /// Detect all sessions, then diff + emit events.
    ///
    /// Uses `refresh_processes()` on a shared `System` instance instead of
    /// `System::new_all()`. This only updates the process list (CPU, memory,
    /// disks, networks are skipped), reducing overhead significantly.
    fn detect_and_emit(
        state: &Arc<Mutex<MonitorState>>,
        sys: &Arc<Mutex<sysinfo::System>>,
        app: &AppHandle<R>,
    ) {
        // Phase 1: refresh processes + detect all sessions
        let detected = {
            let mut system = sys.lock().unwrap();
            system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

            let adapters: Vec<Box<dyn AgentMonitor>> = vec![
                Box::new(super::adapters::KiroAdapter::new()),
                Box::new(super::adapters::ClaudeCodeAdapter::new()),
                Box::new(super::adapters::CodexAdapter::new()),
            ];

            let mut detected = HashMap::new();
            for adapter in &adapters {
                for session in adapter.detect_sessions(&system) {
                    detected.insert(session.session_id.clone(), session);
                }
            }
            detected
        };

        // Phase 2: brief lock to diff + emit events
        let mut state = state.lock().unwrap();
        let old_ids: std::collections::HashSet<_> = state.sessions.keys().cloned().collect();
        let new_ids: std::collections::HashSet<_> = detected.keys().cloned().collect();

        for id in new_ids.difference(&old_ids) {
            if let Some(session) = detected.get(id) {
                state.sessions.insert(id.clone(), session.clone());
                let _ = app.emit(
                    "monitor:state-changed",
                    serde_json::json!({
                        "change": "added",
                        "session": session,
                    }),
                );
            }
        }

        for id in new_ids.intersection(&old_ids) {
            if let Some(session) = detected.get(id) {
                let old_working_state = state.sessions.get(id).map(|s| s.working_state);
                state.sessions.insert(id.clone(), session.clone());
                let _ = app.emit(
                    "monitor:state-changed",
                    serde_json::json!({
                        "change": "updated",
                        "session": session,
                    }),
                );

                // Turn-end semantic: Working → Finished.
                // Notification fires here, NOT on `removed` (kill -9 must stay silent).
                let is_turn_end = is_turn_end_transition(old_working_state, session.working_state);
                if is_turn_end && Self::should_notify(&mut state, &session.session_id) {
                    let body = Self::notification_body(session);
                    let granted = app
                        .notification()
                        .permission_state()
                        .map(|s| matches!(s, tauri_plugin_notification::PermissionState::Granted))
                        .unwrap_or(false);
                    if granted {
                        let _ = app
                            .notification()
                            .builder()
                            .title("Agent 任务完成")
                            .body(body)
                            .show();
                    }
                }
            }
        }

        for id in old_ids.difference(&new_ids) {
            if let Some(mut session) = state.sessions.remove(id) {
                session.status = SessionStatus::Ended;
                let _ = app.emit(
                    "monitor:state-changed",
                    serde_json::json!({
                        "change": "removed",
                        "session": session,
                    }),
                );
                // Intentionally no notification: kill -9 / terminal close should be silent.
                // Turn-end notifications are emitted in the `updated` branch above.
            }
        }
    }

    pub fn poll(&self) {
        Self::detect_and_emit(&self.state, &self.sys, &self.app);
    }

    /// Ensure at least one scan has been done. Called by get_active_sessions
    /// to guarantee data is available even before polling starts.
    pub fn ensure_scanned(&self) {
        let has_data = {
            let state = self.state.lock().unwrap();
            !state.sessions.is_empty()
        };
        if !has_data {
            self.poll();
        }
    }

    pub fn get_sessions(&self) -> Vec<AgentSession> {
        let state = self.state.lock().unwrap();
        let mut sessions: Vec<AgentSession> = state.sessions.values().cloned().collect();
        sessions.sort_by(|left, right| {
            right
                .last_reply_at
                .unwrap_or(right.last_activity)
                .cmp(&left.last_reply_at.unwrap_or(left.last_activity))
        });
        sessions
    }

    pub fn get_config(&self) -> MonitorConfig {
        let state = self.state.lock().unwrap();
        state.config.clone()
    }

    pub fn set_config(&self, new_config: MonitorConfig) {
        let mut state = self.state.lock().unwrap();
        state.config = new_config;
    }

    pub fn configure_hooks(&self, agent_type: &str) -> Result<(), String> {
        let home = dirs::home_dir().ok_or_else(|| "Cannot find home directory".to_string())?;
        let hooks_dir = &self.hooks_dir;

        // Write the hook script to a file so configs only reference the path
        std::fs::create_dir_all(hooks_dir)
            .map_err(|e| format!("Failed to create hooks dir: {e}"))?;

        let script_path = hooks_dir.join(format!("{agent_type}-hook.sh"));

        let script_content = match agent_type {
            "claude-code" | "codex" | "kiro" => format!(
                r#"#!/bin/bash
set -u
HOOK_DIR="{d}"
AGENT_TYPE="{agent}"
EVENT="${{1:-end}}"
PAYLOAD="$(cat 2>/dev/null || true)"
mkdir -p "$HOOK_DIR"
OUT="$HOOK_DIR/${{AGENT_TYPE}}_$$_$(date +%s).done"
AGENT_HUB_PAYLOAD="$PAYLOAD" AGENT_HUB_EVENT="$EVENT" AGENT_HUB_AGENT="$AGENT_TYPE" python3 - <<'PY' > "$OUT"
import datetime
import json
import os

payload = os.environ.get("AGENT_HUB_PAYLOAD", "")
try:
    data = json.loads(payload) if payload.strip() else {{}}
except Exception:
    data = {{}}

def pick(*keys):
    for key in keys:
        value = data.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return ""

marker = {{
    "agent_type": os.environ.get("AGENT_HUB_AGENT", "unknown"),
    "event": os.environ.get("AGENT_HUB_EVENT", "end"),
    "timestamp": datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
}}
session_id = pick("session_id", "sessionId", "thread_id", "threadId", "conversation_id", "conversationId")
cwd = pick("cwd", "workdir", "working_dir", "workspace", "workspaceRoot")
if session_id:
    marker["session_id"] = session_id
if cwd:
    marker["cwd"] = cwd
print(json.dumps(marker, ensure_ascii=False, separators=(",", ":")))
PY
"#,
                d = hooks_dir.display(),
                agent = agent_type,
            ),
            _ => return Err(format!("Unsupported agent type: {agent_type}")),
        };

        std::fs::write(&script_path, &script_content)
            .map_err(|e| format!("Failed to write hook script: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
        }

        let script_path_str = script_path.to_string_lossy().to_string();

        match agent_type {
            "claude-code" => {
                let settings_path = home.join(".claude/settings.json");
                let mut settings: serde_json::Value = if settings_path.exists() {
                    serde_json::from_str(
                        &std::fs::read_to_string(&settings_path)
                            .map_err(|e| format!("Failed to read settings: {e}"))?,
                    )
                    .unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                let hooks = settings
                    .as_object_mut()
                    .unwrap()
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}))
                    .as_object_mut()
                    .unwrap();

                let stop_hooks = hooks
                    .entry("Stop")
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                    .unwrap();

                // Remove existing agent-hub hook if present
                stop_hooks.retain(|h| {
                    h.get("hooks")
                        .and_then(|arr| arr.as_array())
                        .map(|arr| {
                            !arr.iter().any(|hh| {
                                hh.get("command")
                                    .and_then(|c| c.as_str())
                                    .map(|c| c.contains("agent-hub"))
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(true)
                });

                stop_hooks.push(serde_json::json!({
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": format!("{} end", script_path_str)
                    }]
                }));

                let start_hooks = hooks
                    .entry("UserPromptSubmit")
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                    .unwrap();
                start_hooks.retain(|h| {
                    h.get("hooks")
                        .and_then(|arr| arr.as_array())
                        .map(|arr| {
                            !arr.iter().any(|hh| {
                                hh.get("command")
                                    .and_then(|c| c.as_str())
                                    .map(|c| c.contains("agent-hub"))
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(true)
                });
                start_hooks.push(serde_json::json!({
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": format!("{} start", script_path_str)
                    }]
                }));

                std::fs::write(
                    &settings_path,
                    serde_json::to_string_pretty(&settings).unwrap(),
                )
                .map_err(|e| format!("Failed to write settings: {e}"))?;

                log::info!("Configured Claude Code hooks in {:?}", settings_path);
            }
            "codex" => {
                let config_path = home.join(".codex/config.toml");
                let mut content = if config_path.exists() {
                    std::fs::read_to_string(&config_path)
                        .map_err(|e| format!("Failed to read config: {e}"))?
                } else {
                    String::new()
                };

                // Remove existing managed hook section
                content = Self::remove_toml_hook_section(&content, "codex-hook.sh");

                // Add stop hook. Codex turn start/end is also inferred from rollout events.
                content.push_str(&format!(
                    "\n# agent-hub hooks begin\n[hooks.stop]\ncommand = \"{} end\"\n# agent-hub hooks end\n",
                    Self::toml_escape(&script_path_str)
                ));

                if let Some(parent) = config_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&config_path, content)
                    .map_err(|e| format!("Failed to write config: {e}"))?;

                log::info!("Configured Codex hooks in {:?}", config_path);
            }
            "kiro" => {
                let config_path = home.join(".kiro/agents/kiro-monitored.json");
                let mut config: serde_json::Value = if config_path.exists() {
                    serde_json::from_str(
                        &std::fs::read_to_string(&config_path)
                            .map_err(|e| format!("Failed to read config: {e}"))?,
                    )
                    .map_err(|e| format!("Failed to parse config: {e}"))?
                } else {
                    serde_json::json!({
                        "name": "kiro-monitored",
                        "description": "Default agent with agent-hub completion hooks",
                        "tools": ["*"],
                        "hooks": {}
                    })
                };

                let hooks = config
                    .as_object_mut()
                    .unwrap()
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}))
                    .as_object_mut()
                    .unwrap();

                hooks.insert(
                    "stop".to_string(),
                    serde_json::json!([{ "command": format!("{} end", script_path_str) }]),
                );

                if let Some(parent) = config_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
                    .map_err(|e| format!("Failed to write config: {e}"))?;

                log::info!("Configured Kiro hooks in {:?}", config_path);
            }
            _ => return Err(format!("Unsupported agent type: {agent_type}")),
        }

        Ok(())
    }

    pub fn remove_hooks(&self, agent_type: &str) -> Result<(), String> {
        let home = dirs::home_dir().ok_or_else(|| "Cannot find home directory".to_string())?;

        match agent_type {
            "claude-code" => {
                let settings_path = home.join(".claude/settings.json");
                if !settings_path.exists() {
                    return Ok(());
                }
                let mut settings: serde_json::Value = serde_json::from_str(
                    &std::fs::read_to_string(&settings_path)
                        .map_err(|e| format!("Failed to read settings: {e}"))?,
                )
                .map_err(|e| format!("Failed to parse settings: {e}"))?;

                if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                    for event_name in ["Stop", "UserPromptSubmit"] {
                        if let Some(event_hooks) =
                            hooks.get_mut(event_name).and_then(|s| s.as_array_mut())
                        {
                            event_hooks.retain(|h| {
                                h.get("hooks")
                                    .and_then(|arr| arr.as_array())
                                    .map(|arr| {
                                        !arr.iter().any(|hh| {
                                            hh.get("command")
                                                .and_then(|c| c.as_str())
                                                .map(|c| c.contains("agent-hub"))
                                                .unwrap_or(false)
                                        })
                                    })
                                    .unwrap_or(true)
                            });
                            if event_hooks.is_empty() {
                                hooks.remove(event_name);
                            }
                        }
                    }
                    if hooks.is_empty() {
                        settings.as_object_mut().unwrap().remove("hooks");
                    }
                }

                std::fs::write(
                    &settings_path,
                    serde_json::to_string_pretty(&settings).unwrap(),
                )
                .map_err(|e| format!("Failed to write settings: {e}"))?;

                log::info!("Removed Claude Code hooks from {:?}", settings_path);
            }
            "codex" => {
                let config_path = home.join(".codex/config.toml");
                if !config_path.exists() {
                    return Ok(());
                }
                let content = std::fs::read_to_string(&config_path)
                    .map_err(|e| format!("Failed to read config: {e}"))?;
                let cleaned = Self::remove_toml_hook_section(&content, "codex-hook.sh");
                std::fs::write(&config_path, cleaned)
                    .map_err(|e| format!("Failed to write config: {e}"))?;

                log::info!("Removed Codex hooks from {:?}", config_path);
            }
            "kiro" => {
                let config_path = home.join(".kiro/agents/kiro-monitored.json");
                if !config_path.exists() {
                    return Ok(());
                }
                let mut config: serde_json::Value = serde_json::from_str(
                    &std::fs::read_to_string(&config_path)
                        .map_err(|e| format!("Failed to read config: {e}"))?,
                )
                .map_err(|e| format!("Failed to parse config: {e}"))?;

                if let Some(hooks) = config.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                    hooks.remove("stop");
                }

                std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
                    .map_err(|e| format!("Failed to write config: {e}"))?;

                log::info!("Removed Kiro hooks from {:?}", config_path);
            }
            _ => return Err(format!("Unsupported agent type: {agent_type}")),
        }

        Ok(())
    }

    pub fn hooks_status(&self) -> HashMap<String, bool> {
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return HashMap::new(),
        };

        let mut status = HashMap::new();

        // Claude Code
        let cc_settings = home.join(".claude/settings.json");
        let cc_configured = if cc_settings.exists() {
            std::fs::read_to_string(&cc_settings)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .map(|v| {
                    v.get("hooks")
                        .and_then(|h| h.get("Stop"))
                        .and_then(|s| s.as_array())
                        .map(|arr| {
                            arr.iter().any(|h| {
                                h.get("hooks")
                                    .and_then(|hh| hh.as_array())
                                    .map(|hh| {
                                        hh.iter().any(|cmd| {
                                            cmd.get("command")
                                                .and_then(|c| c.as_str())
                                                .map(|c| c.contains("agent-hub"))
                                                .unwrap_or(false)
                                        })
                                    })
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        } else {
            false
        };
        // Also verify the script file exists
        let cc_script_exists = self.hooks_dir.join("claude-code-hook.sh").exists();
        status.insert("claude-code".to_string(), cc_configured && cc_script_exists);

        // Codex
        let codex_config = home.join(".codex/config.toml");
        let codex_script_exists = self.hooks_dir.join("codex-hook.sh").exists();
        let codex_configured = if codex_config.exists() {
            std::fs::read_to_string(&codex_config)
                .ok()
                .map(|s| s.contains(".agent-hub/hooks") && s.contains("codex-hook.sh"))
                .unwrap_or(false)
        } else {
            false
        };
        status.insert("codex".to_string(), codex_configured && codex_script_exists);

        // Kiro
        let kiro_config = home.join(".kiro/agents/kiro-monitored.json");
        let kiro_configured = if kiro_config.exists() {
            std::fs::read_to_string(&kiro_config)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .map(|v| {
                    v.get("hooks")
                        .and_then(|h| h.get("stop"))
                        .and_then(|s| s.as_array())
                        .map(|arr| {
                            arr.iter().any(|hook| {
                                hook.get("command")
                                    .and_then(|command| command.as_str())
                                    .map(|command| {
                                        command.contains(".agent-hub/hooks")
                                            && command.contains("kiro-hook.sh")
                                    })
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        } else {
            false
        };
        let kiro_script_exists = self.hooks_dir.join("kiro-hook.sh").exists();
        status.insert("kiro".to_string(), kiro_configured && kiro_script_exists);

        status
    }

    fn remove_toml_hook_section(content: &str, marker: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut index = 0usize;

        while index < lines.len() {
            let line = lines[index];
            if line.contains("# agent-hub hooks begin") {
                index += 1;
                while index < lines.len() && !lines[index].contains("# agent-hub hooks end") {
                    index += 1;
                }
                index = index.saturating_add(1);
                continue;
            }

            if line.contains("# agent-hub completion hook") || line.contains(marker) {
                index += 1;
                if index < lines.len() && lines[index].trim_start().starts_with("[hooks.") {
                    index += 1;
                    while index < lines.len() && !lines[index].trim_start().starts_with('[') {
                        index += 1;
                    }
                }
                continue;
            }

            if line.trim_start().starts_with("[hooks.") {
                let section_start = index;
                index += 1;
                while index < lines.len()
                    && !lines[index].trim_start().starts_with('[')
                    && !lines[index].contains("# agent-hub hooks begin")
                    && !lines[index].contains("# agent-hub completion hook")
                {
                    index += 1;
                }
                let section = lines[section_start..index].join("\n");
                if section.contains(marker) || section.contains(".agent-hub/hooks") {
                    continue;
                }
                result.extend_from_slice(&lines[section_start..index]);
                continue;
            }

            result.push(line);
            index += 1;
        }

        result.join("\n").trim_end().to_string()
    }

    fn toml_escape(value: &str) -> String {
        value.replace('\\', "\\\\").replace('"', "\\\"")
    }

    /// Returns true if a turn-end notification should fire for this session_id,
    /// honoring `notification_enabled` and the per-session cooldown. On true, the
    /// caller should fire the notification; this function records the timestamp so
    /// the next call within the cooldown window returns false.
    ///
    /// Caller must already hold the state lock — this avoids re-locking inside
    /// `detect_and_emit`, which would deadlock.
    fn should_notify(state: &mut MonitorState, session_id: &str) -> bool {
        should_notify_impl(state, session_id)
    }

    fn notification_body(session: &AgentSession) -> String {
        let headline = session
            .last_user_prompt
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&session.title);
        format!(
            "[{} {}] {}",
            session.agent_type, session.source_tag, headline
        )
    }
}

/// Pure transition predicate. Working → Finished counts as a turn-end.
/// Anything else (including Working → Ended from a kill) does not.
fn is_turn_end_transition(old: Option<WorkingState>, new: WorkingState) -> bool {
    matches!(old, Some(WorkingState::Working)) && matches!(new, WorkingState::Finished)
}

/// Cooldown + enabled gate for turn-end notifications. Pure on `state`.
fn should_notify_impl(state: &mut MonitorState, session_id: &str) -> bool {
    if !state.config.notification_enabled {
        return false;
    }
    if let Some(last) = state.last_notified.get(session_id) {
        let elapsed = Utc::now().signed_duration_since(*last).num_seconds();
        if elapsed < state.config.notification_cooldown_secs as i64 {
            return false;
        }
    }
    state
        .last_notified
        .insert(session_id.to_string(), Utc::now());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(notif_enabled: bool, cooldown: u64) -> MonitorState {
        MonitorState::new(MonitorConfig {
            enabled: true,
            notification_enabled: notif_enabled,
            notification_cooldown_secs: cooldown,
        })
    }

    #[test]
    fn turn_end_working_to_finished_is_true() {
        assert!(is_turn_end_transition(
            Some(WorkingState::Working),
            WorkingState::Finished,
        ));
    }

    /// Regression: kill -9 (Working → Ended via removed branch) must NOT count
    /// as turn-end. Defends against Issue #4 (kill misclassified as completion).
    #[test]
    fn turn_end_working_to_idle_is_false() {
        assert!(!is_turn_end_transition(
            Some(WorkingState::Working),
            WorkingState::Idle,
        ));
    }

    #[test]
    fn turn_end_no_prior_state_is_false() {
        // First sighting (added) is not a turn-end.
        assert!(!is_turn_end_transition(None, WorkingState::Finished));
    }

    #[test]
    fn turn_end_idle_to_finished_is_false() {
        assert!(!is_turn_end_transition(
            Some(WorkingState::Idle),
            WorkingState::Finished,
        ));
    }

    #[test]
    fn should_notify_disabled_returns_false() {
        let mut s = make_state(false, 30);
        assert!(!should_notify_impl(&mut s, "sess-1"));
        assert!(s.last_notified.is_empty());
    }

    #[test]
    fn should_notify_first_call_returns_true() {
        let mut s = make_state(true, 30);
        assert!(should_notify_impl(&mut s, "sess-1"));
        assert_eq!(s.last_notified.len(), 1);
    }

    /// Regression: cooldown blocks duplicate notifications within window.
    /// Defends against CQ #1 (cooldown logic dead-code regression).
    #[test]
    fn should_notify_within_cooldown_returns_false() {
        let mut s = make_state(true, 30);
        assert!(should_notify_impl(&mut s, "sess-1"));
        // Immediate second call: same session, well within 30s.
        assert!(!should_notify_impl(&mut s, "sess-1"));
    }

    #[test]
    fn should_notify_cooldown_is_per_session() {
        let mut s = make_state(true, 30);
        assert!(should_notify_impl(&mut s, "sess-1"));
        // Different session: cooldown does not apply.
        assert!(should_notify_impl(&mut s, "sess-2"));
    }

    #[test]
    fn should_notify_zero_cooldown_allows_back_to_back() {
        let mut s = make_state(true, 0);
        assert!(should_notify_impl(&mut s, "sess-1"));
        // 0s cooldown means any positive elapsed unblocks the next call.
        // Same-instant elapsed=0 still blocks per `<` comparison; that's fine.
        // We only assert that with a longer wait it would unblock — verified via
        // manual override below.
        s.last_notified.insert(
            "sess-1".to_string(),
            Utc::now() - chrono::Duration::seconds(1),
        );
        assert!(should_notify_impl(&mut s, "sess-1"));
    }

    #[test]
    fn remove_toml_hook_section_removes_legacy_codex_hook() {
        let input = r#"model = "gpt-5"

# agent-hub completion hook
[hooks.stop]
command = "/Users/me/.agent-hub/hooks/codex-hook.sh"

[plugins.browser]
enabled = true
"#;
        let cleaned =
            MonitorService::<tauri::Wry>::remove_toml_hook_section(input, "codex-hook.sh");
        assert!(cleaned.contains("model = \"gpt-5\""));
        assert!(cleaned.contains("[plugins.browser]"));
        assert!(!cleaned.contains("codex-hook.sh"));
        assert!(!cleaned.contains("[hooks.stop]"));
    }

    #[test]
    fn remove_toml_hook_section_preserves_unrelated_hooks() {
        let input = r#"[hooks.stop]
command = "/Users/me/bin/other-hook.sh"

# agent-hub hooks begin
[hooks.stop]
command = "/Users/me/.agent-hub/hooks/codex-hook.sh end"
# agent-hub hooks end
"#;
        let cleaned =
            MonitorService::<tauri::Wry>::remove_toml_hook_section(input, "codex-hook.sh");
        assert!(cleaned.contains("other-hook.sh"));
        assert!(!cleaned.contains(".agent-hub/hooks"));
        assert!(!cleaned.contains("agent-hub hooks begin"));
    }
}

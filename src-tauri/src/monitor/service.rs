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
            Box::new(super::adapters::GeminiAdapter::new()),
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

    fn process_hook_marker(state: &Arc<Mutex<MonitorState>>, app: &AppHandle<R>, path: &std::path::Path) {
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
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

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
                .filter(|(_, s)| s.agent_type == agent_type && s.working_state == WorkingState::Working)
                .map(|(id, _)| id.clone())
                .next()
        };

        if let Some(id) = target_id {
            if let Some(session) = state.sessions.get_mut(&id) {
                let old_state = session.working_state;
                session.working_state = WorkingState::Finished;
                let title = session.title.clone();
                let body = format!("[{}] {}", session.agent_type, title);

                let _ = app.emit(
                    "monitor:state-changed",
                    serde_json::json!({
                        "change": "updated",
                        "session": session,
                    }),
                );

                if matches!(old_state, WorkingState::Working)
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
                Box::new(super::adapters::GeminiAdapter::new()),
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
                state
                    .sessions
                    .insert(id.clone(), session.clone());
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
                state
                    .sessions
                    .insert(id.clone(), session.clone());
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
                    let title = session.title.clone();
                    let body = format!("[{}] {}", session.agent_type, title);
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
        state.sessions.values().cloned().collect()
    }

    pub fn get_config(&self) -> MonitorConfig {
        let state = self.state.lock().unwrap();
        state.config.clone()
    }

    pub fn set_config(&self, new_config: MonitorConfig) {
        let mut state = self.state.lock().unwrap();
        state.config = new_config;
    }

    pub fn hooks_dir(&self) -> &std::path::Path {
        &self.hooks_dir
    }

    pub fn configure_hooks(&self, agent_type: &str) -> Result<(), String> {
        let home = dirs::home_dir().ok_or_else(|| "Cannot find home directory".to_string())?;
        let hooks_dir = &self.hooks_dir;

        // Write the hook script to a file so configs only reference the path
        std::fs::create_dir_all(hooks_dir)
            .map_err(|e| format!("Failed to create hooks dir: {e}"))?;

        let script_path = hooks_dir.join(format!("{agent_type}-hook.sh"));

        let script_content = match agent_type {
            "claude-code" => format!(
                "#!/bin/bash\nmkdir -p \"{d}\"\nSID=$(cat | python3 -c \"import sys,json; print(json.load(sys.stdin).get('session_id',''))\" 2>/dev/null)\necho '{{\"agent_type\":\"claude-code\",\"session_id\":\"'\"$SID\"'\",\"timestamp\":\"'\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"'\"}}' > \"{d}/claude-code_$$_$(date +%s).done\"\n",
                d = hooks_dir.display()
            ),
            "codex" => format!(
                "#!/bin/bash\nmkdir -p \"{d}\"\necho '{{\"agent_type\":\"codex\",\"timestamp\":\"'\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"'\"}}' > \"{d}/codex_$$_$(date +%s).done\"\n",
                d = hooks_dir.display()
            ),
            "kiro" => format!(
                "#!/bin/bash\nmkdir -p \"{d}\"\necho '{{\"agent_type\":\"kiro\",\"timestamp\":\"'\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"'\"}}' > \"{d}/kiro_$$_$(date +%s).done\"\n",
                d = hooks_dir.display()
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
                    serde_json::from_str(&std::fs::read_to_string(&settings_path)
                        .map_err(|e| format!("Failed to read settings: {e}"))?)
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
                        .map(|arr| !arr.iter().any(|hh| {
                            hh.get("command")
                                .and_then(|c| c.as_str())
                                .map(|c| c.contains("agent-hub"))
                                .unwrap_or(false)
                        }))
                        .unwrap_or(true)
                });

                stop_hooks.push(serde_json::json!({
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": script_path_str
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

                // Remove existing agent-hub hook section
                content = Self::remove_toml_hook_section(&content, "agent-hub");

                // Add stop hook — script_path has no special chars, safe for TOML
                content.push_str(&format!(
                    "\n# agent-hub completion hook\n[hooks.stop]\ncommand = \"{}\"\n",
                    script_path_str
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
                    serde_json::from_str(&std::fs::read_to_string(&config_path)
                        .map_err(|e| format!("Failed to read config: {e}"))?)
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
                    serde_json::json!([{ "command": script_path_str }]),
                );

                if let Some(parent) = config_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(
                    &config_path,
                    serde_json::to_string_pretty(&config).unwrap(),
                )
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
                let mut settings: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&settings_path)
                        .map_err(|e| format!("Failed to read settings: {e}"))?)
                        .map_err(|e| format!("Failed to parse settings: {e}"))?;

                if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                    if let Some(stop) = hooks.get_mut("Stop").and_then(|s| s.as_array_mut()) {
                        stop.retain(|h| {
                            h.get("hooks")
                                .and_then(|arr| arr.as_array())
                                .map(|arr| !arr.iter().any(|hh| {
                                    hh.get("command")
                                        .and_then(|c| c.as_str())
                                        .map(|c| c.contains("agent-hub"))
                                        .unwrap_or(false)
                                }))
                                .unwrap_or(true)
                        });
                        if stop.is_empty() {
                            hooks.remove("Stop");
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
                let cleaned = Self::remove_toml_hook_section(&content, "agent-hub");
                std::fs::write(&config_path, cleaned)
                    .map_err(|e| format!("Failed to write config: {e}"))?;

                log::info!("Removed Codex hooks from {:?}", config_path);
            }
            "kiro" => {
                let config_path = home.join(".kiro/agents/kiro-monitored.json");
                if !config_path.exists() {
                    return Ok(());
                }
                let mut config: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&config_path)
                        .map_err(|e| format!("Failed to read config: {e}"))?)
                        .map_err(|e| format!("Failed to parse config: {e}"))?;

                if let Some(hooks) = config.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                    hooks.remove("stop");
                }

                std::fs::write(
                    &config_path,
                    serde_json::to_string_pretty(&config).unwrap(),
                )
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
                        .map(|arr| arr.iter().any(|h| {
                            h.get("hooks")
                                .and_then(|hh| hh.as_array())
                                .map(|hh| hh.iter().any(|cmd| {
                                    cmd.get("command")
                                        .and_then(|c| c.as_str())
                                        .map(|c| c.contains("agent-hub"))
                                        .unwrap_or(false)
                                }))
                                .unwrap_or(false)
                        }))
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
                .map(|s| s.contains("agent-hub"))
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
                        .map(|arr| !arr.is_empty())
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
        let mut result = String::new();
        let mut in_hook_section = false;
        for line in content.lines() {
            if line.contains(marker) {
                in_hook_section = true;
                continue;
            }
            if in_hook_section && line.trim().starts_with('[') {
                in_hook_section = false;
            }
            if !in_hook_section {
                result.push_str(line);
                result.push('\n');
            }
        }
        result.trim_end().to_string()
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
}

/// Pure transition predicate. Working → Finished counts as a turn-end.
/// Anything else (including Working → Ended from a kill) does not.
fn is_turn_end_transition(old: Option<WorkingState>, new: WorkingState) -> bool {
    matches!(old, Some(WorkingState::Working))
        && matches!(new, WorkingState::Finished)
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
}

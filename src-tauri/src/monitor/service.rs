use super::types::*;
use chrono::Utc;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};

pub struct MonitorService<R: Runtime> {
    state: Arc<Mutex<MonitorState>>,
    adapters: Vec<Box<dyn AgentMonitor>>,
    _watcher: Option<RecommendedWatcher>,
    app: AppHandle<R>,
    pub polling_enabled: Arc<AtomicBool>,
}

impl<R: Runtime> MonitorService<R> {
    pub fn new(app: AppHandle<R>, config: MonitorConfig) -> Self {
        let state = Arc::new(Mutex::new(MonitorState::new(config)));
        let adapters: Vec<Box<dyn AgentMonitor>> = vec![
            Box::new(super::adapters::KiroAdapter::new()),
            Box::new(super::adapters::ClaudeCodeAdapter::new()),
            Box::new(super::adapters::CodexAdapter::new()),
            Box::new(super::adapters::GeminiAdapter::new()),
        ];

        let polling_enabled = Arc::new(AtomicBool::new(false));

        let mut service = Self {
            state,
            adapters,
            _watcher: None,
            app,
            polling_enabled,
        };

        service.init_watcher();
        // Defer initial scan — will run on first poll or first get_active_sessions call
        service
    }

    fn init_watcher(&mut self) {
        let state = self.state.clone();
        let app = self.app.clone();

        let mut watcher = match RecommendedWatcher::new(
            move |_res: Result<notify::Event, notify::Error>| {
                let state = state.clone();
                let app = app.clone();
                Self::detect_and_emit(&state, &app);
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

    /// Detect all sessions (lock-free), then briefly lock state to diff + emit.
    fn detect_and_emit(state: &Arc<Mutex<MonitorState>>, app: &AppHandle<R>) {
        let sys = sysinfo::System::new_all();
        let adapters: Vec<Box<dyn AgentMonitor>> = vec![
            Box::new(super::adapters::KiroAdapter::new()),
            Box::new(super::adapters::ClaudeCodeAdapter::new()),
            Box::new(super::adapters::CodexAdapter::new()),
            Box::new(super::adapters::GeminiAdapter::new()),
        ];

        // Phase 1: detect all sessions (no lock held)
        let mut detected = HashMap::new();
        for adapter in &adapters {
            for session in adapter.detect_sessions(&sys) {
                detected.insert(session.session_id.clone(), session);
            }
        }

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
            }
        }
    }

    pub fn poll(&self) {
        Self::detect_and_emit(&self.state, &self.app);
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

    #[allow(dead_code)]
    pub fn check_and_notify(&self, session: &AgentSession) -> bool {
        let mut state = self.state.lock().unwrap();
        if !state.config.notification_enabled {
            return false;
        }

        if let Some(last) = state.last_notified.get(&session.session_id) {
            let elapsed = Utc::now()
                .signed_duration_since(*last)
                .num_seconds();
            if elapsed < state.config.notification_cooldown_secs as i64 {
                return false;
            }
        }

        state
            .last_notified
            .insert(session.session_id.clone(), Utc::now());
        true
    }
}

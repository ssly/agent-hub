use super::types::*;
use chrono::Utc;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};

pub struct MonitorService<R: Runtime> {
    state: Arc<Mutex<MonitorState>>,
    adapters: Vec<Box<dyn AgentMonitor>>,
    _watcher: Option<RecommendedWatcher>,
    app: AppHandle<R>,
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

        let mut service = Self {
            state,
            adapters,
            _watcher: None,
            app,
        };

        service.init_watcher();
        service.initial_scan();
        service
    }

    fn init_watcher(&mut self) {
        let state = self.state.clone();
        let app = self.app.clone();

        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(_event) = res {
                    let state = state.clone();
                    let app = app.clone();
                    // We can't mutably borrow adapters here, so we do a full refresh
                    // on fs events instead
                    Self::handle_fs_event_static(&state, &app);
                }
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
                if let Err(e) =
                    watcher.watch(&path, RecursiveMode::Recursive)
                {
                    log::warn!(
                        "Failed to watch {:?}: {e}",
                        path
                    );
                } else {
                    log::info!("Watching {:?} for {}", path, adapter.platform_id());
                }
            }
        }

        self._watcher = Some(watcher);
    }

    fn initial_scan(&mut self) {
        let mut all_sessions = Vec::new();
        for adapter in &self.adapters {
            all_sessions.extend(adapter.detect_sessions());
        }

        let mut state = self.state.lock().unwrap();
        for session in all_sessions {
            state
                .sessions
                .insert(session.session_id.clone(), session);
        }
    }

    fn handle_fs_event_static(state: &Arc<Mutex<MonitorState>>, app: &AppHandle<R>) {
        // Re-detect all sessions on any fs event
        let adapters: Vec<Box<dyn AgentMonitor>> = vec![
            Box::new(super::adapters::KiroAdapter::new()),
            Box::new(super::adapters::ClaudeCodeAdapter::new()),
            Box::new(super::adapters::CodexAdapter::new()),
            Box::new(super::adapters::GeminiAdapter::new()),
        ];

        let mut detected = HashMap::new();
        for adapter in &adapters {
            for session in adapter.detect_sessions() {
                detected.insert(session.session_id.clone(), session);
            }
        }

        let mut state = state.lock().unwrap();
        let old_ids: std::collections::HashSet<_> = state.sessions.keys().cloned().collect();
        let new_ids: std::collections::HashSet<_> = detected.keys().cloned().collect();

        // New sessions
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

        // Updated sessions
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

        // Ended sessions
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
        Self::handle_fs_event_static(&self.state, &self.app);
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

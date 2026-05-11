use super::types::*;
use chrono::Utc;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_notification::NotificationExt;

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
    ///
    /// Stale-snapshot policy: `sysinfo::System::new_all()` is rebuilt every call,
    /// and `MonitorState::new` starts with an empty session map, so there is no
    /// cross-restart cache that can go stale. When Tier-1 fingerprint caching
    /// (jsonl size+mtime+inode) is added, this function must reset that cache on
    /// app start so a restarted agent's first poll sees an "added" event, not an
    /// "updated" one against a stale fingerprint.
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
                    let _ = app
                        .notification()
                        .builder()
                        .title("Agent 任务完成")
                        .body(body)
                        .show();
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

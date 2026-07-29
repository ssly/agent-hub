use super::kiro::KiroWatcher;
use super::types::{AgentKind, HookEvent, KiroMonitorStatus, MonitorSnapshot, RuntimeStatus, SessionState};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};
use uuid::Uuid;

const MAX_SESSIONS: usize = 100;
/// Sessions older than this are pruned on every snapshot query.
const SESSION_TTL_MILLIS: i64 = 24 * 60 * 60 * 1000;
/// Kiro running/ended status comes from lock-file pid liveness, which
/// produces no file events — re-check it on this interval and push changes.
const KIRO_STATUS_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);

struct AgentSlot {
    snapshot: Mutex<MonitorSnapshot>,
    state_path: PathBuf,
}

type Slots = Arc<HashMap<AgentKind, AgentSlot>>;

pub struct SessionMonitorService<R: Runtime> {
    slots: Slots,
    kiro_sessions_dir: PathBuf,
    kiro_toggle_path: PathBuf,
    kiro_enabled: Arc<AtomicBool>,
    kiro_watcher: Mutex<Option<KiroWatcher>>,
    _inbox_watcher: Option<RecommendedWatcher>,
    _app: AppHandle<R>,
}

impl<R: Runtime> SessionMonitorService<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        let root = dirs::home_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(".agent-hub")
            .join("session-monitor");
        let inbox = root.join("inbox");
        let _ = fs::create_dir_all(&inbox);

        let slots: Slots = Arc::new(
            AgentKind::ALL
                .into_iter()
                .map(|agent| {
                    let state_path = root.join(agent.state_file_name());
                    let slot = AgentSlot {
                        snapshot: Mutex::new(load_snapshot(&state_path)),
                        state_path,
                    };
                    (agent, slot)
                })
                .collect(),
        );

        let inbox_watcher = init_watcher(&inbox, slots.clone(), app.clone());
        process_pending_events(&inbox, &slots, &app);

        // Kiro: stable kiro-cli 2.x does not load hook configs, so
        // ~/.kiro/sessions/cli is watched directly (read-only). The watcher
        // and the status thread can be toggled by the user; the choice
        // persists in kiro-monitor.json.
        let kiro_sessions_dir = dirs::home_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(".kiro")
            .join("sessions")
            .join("cli");
        let kiro_toggle_path = root.join("kiro-monitor.json");
        let kiro_enabled = Arc::new(AtomicBool::new(load_kiro_enabled(&kiro_toggle_path)));
        let kiro_watcher = Mutex::new(if kiro_enabled.load(Ordering::Acquire) {
            create_kiro_watcher(&kiro_sessions_dir, &slots, &app)
        } else {
            None
        });
        // Kiro sessions going idle produce no file event (the CLI just exits
        // and its lock pid dies), so poll pid liveness on a slow interval and
        // emit only when a status actually flips. Without this the UI would
        // show "running" until a manual refresh. While monitoring is off the
        // thread just sleeps — a few nanoseconds every interval.
        {
            let slots = slots.clone();
            let dir = kiro_sessions_dir.clone();
            let app = app.clone();
            let enabled = kiro_enabled.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(KIRO_STATUS_REFRESH_INTERVAL);
                if enabled.load(Ordering::Acquire) {
                    refresh_kiro_statuses(&slots, &dir, &app);
                }
            });
        }

        Self {
            slots,
            kiro_sessions_dir,
            kiro_toggle_path,
            kiro_enabled,
            kiro_watcher,
            _inbox_watcher: inbox_watcher,
            _app: app,
        }
    }

    pub fn snapshot(&self, agent: AgentKind) -> MonitorSnapshot {
        self.prune_expired(agent);
        if agent == AgentKind::Kiro && self.kiro_enabled.load(Ordering::Acquire) {
            refresh_kiro_statuses(&self.slots, &self.kiro_sessions_dir, &self._app);
        }
        self.slots
            .get(&agent)
            .and_then(|slot| slot.snapshot.lock().ok().map(|snapshot| snapshot.clone()))
            .unwrap_or_default()
    }

    pub fn kiro_status(&self) -> KiroMonitorStatus {
        KiroMonitorStatus {
            available: self.kiro_sessions_dir.is_dir(),
            sessions_dir: self.kiro_sessions_dir.display().to_string(),
            enabled: self.kiro_enabled.load(Ordering::Acquire),
        }
    }

    /// Toggle the Kiro file watcher + status thread. Persisted so the choice
    /// survives restarts.
    pub fn set_kiro_enabled(&self, enabled: bool) -> Result<KiroMonitorStatus, String> {
        self.kiro_enabled.store(enabled, Ordering::Release);
        if let Ok(mut watcher) = self.kiro_watcher.lock() {
            match (enabled, watcher.is_some()) {
                (true, false) => {
                    *watcher = create_kiro_watcher(&self.kiro_sessions_dir, &self.slots, &self._app);
                }
                (false, true) => {
                    // Dropping the watcher unregisters it from the OS.
                    *watcher = None;
                }
                _ => {}
            }
        }
        persist_kiro_enabled(&self.kiro_toggle_path, enabled)?;
        Ok(self.kiro_status())
    }

    /// Manually delete one session row. No-op when the id is unknown.
    pub fn remove_session(&self, agent: AgentKind, session_id: &str) -> Result<(), String> {
        let Some(slot) = self.slots.get(&agent) else {
            return Ok(());
        };
        let next_snapshot = {
            let Ok(mut current) = slot.snapshot.lock() else {
                return Err("session monitor state is unavailable".to_string());
            };
            if !remove_session_from(&mut current, session_id) {
                return Ok(());
            }
            current.revision = current.revision.saturating_add(1);
            current.clone()
        };
        persist_snapshot(&slot.state_path, &next_snapshot)?;
        let _ = self._app.emit(agent.changed_event(), &next_snapshot);
        Ok(())
    }

    /// Drop sessions older than SESSION_TTL_MILLIS. Called on every snapshot
    /// query so stale rows age out without a background timer.
    fn prune_expired(&self, agent: AgentKind) {
        let cutoff = now_millis() - SESSION_TTL_MILLIS;
        let Some(slot) = self.slots.get(&agent) else {
            return;
        };
        let next_snapshot = {
            let Ok(mut current) = slot.snapshot.lock() else {
                return;
            };
            if !prune_sessions_older_than(&mut current, cutoff) {
                return;
            }
            current.revision = current.revision.saturating_add(1);
            current.clone()
        };
        if persist_snapshot(&slot.state_path, &next_snapshot).is_ok() {
            let _ = self._app.emit(agent.changed_event(), &next_snapshot);
        }
    }
}

/// Turn-level status comes from the event stream (Prompt → running,
/// AssistantMessage → ended), same as Codex/Claude. The lock pid is only a
/// one-way safety net: a session whose CLI process died mid-turn is flipped
/// running → ended. A live-but-idle chat process (sitting at the prompt)
/// must NOT flip a finished turn back to running. Called on snapshot queries
/// and on a slow background interval; emits only when a status flips.
fn refresh_kiro_statuses<R: Runtime>(slots: &Slots, sessions_dir: &Path, app: &AppHandle<R>) {
    let Some(slot) = slots.get(&AgentKind::Kiro) else {
        return;
    };
    let next_snapshot = {
        let Ok(mut current) = slot.snapshot.lock() else {
            return;
        };
        if current.sessions.is_empty() {
            return;
        }
        let mut changed = false;
        for session in &mut current.sessions {
            if session.status != RuntimeStatus::Running {
                continue;
            }
            if kiro_session_status(sessions_dir, &session.session_id) == RuntimeStatus::Ended {
                session.status = RuntimeStatus::Ended;
                changed = true;
            }
        }
        if !changed {
            return;
        }
        current.revision = current.revision.saturating_add(1);
        current.clone()
    };
    if persist_snapshot(&slot.state_path, &next_snapshot).is_ok() {
        let _ = app.emit(AgentKind::Kiro.changed_event(), &next_snapshot);
    }
}

fn kiro_session_status(sessions_dir: &Path, session_id: &str) -> RuntimeStatus {
    let lock_path = sessions_dir.join(format!("{session_id}.lock"));
    // kiro-cli holds an OS-level exclusive lock on this file for its whole
    // lifetime (q_cli `PidLock`). On Unix that lock is advisory (flock), so
    // the read below still succeeds and we can inspect the pid. On Windows
    // it is a mandatory LockFileEx byte-range lock, so the read FAILS with
    // a lock/sharing violation while the CLI is alive. An existing-but-
    // unreadable lock therefore means "CLI still running", not "ended" —
    // only a missing lock file means the process exited cleanly.
    let content = match fs::read_to_string(&lock_path) {
        Ok(content) => content,
        Err(_) => {
            return if lock_path.exists() {
                RuntimeStatus::Running
            } else {
                RuntimeStatus::Ended
            };
        }
    };
    let pid = serde_json::from_str::<serde_json::Value>(&content)
        .ok()
        .and_then(|value| value.get("pid").and_then(serde_json::Value::as_u64))
        .map(|pid| pid as u32);
    match pid {
        Some(pid) if pid_alive(pid) => RuntimeStatus::Running,
        _ => RuntimeStatus::Ended,
    }
}

fn pid_alive(pid: u32) -> bool {
    let mut system = sysinfo::System::new();
    system.refresh_processes(
        sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
        true,
    ) > 0
}

fn create_kiro_watcher<R: Runtime>(
    sessions_dir: &Path,
    slots: &Slots,
    app: &AppHandle<R>,
) -> Option<KiroWatcher> {
    let slots = slots.clone();
    let app = app.clone();
    KiroWatcher::new(sessions_dir.to_path_buf(), move |event| {
        apply_and_emit(&slots, &app, event);
    })
}

fn load_kiro_enabled(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .and_then(|content| serde_json::from_slice::<serde_json::Value>(&content).ok())
        .and_then(|value| value.get("enabled").and_then(serde_json::Value::as_bool))
        .unwrap_or(true)
}

fn persist_kiro_enabled(path: &Path, enabled: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "kiro monitor toggle has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("unable to create toggle directory: {error}"))?;
    let temp_path = parent.join(format!(".kiro-monitor-{}.tmp", Uuid::new_v4()));
    let payload = serde_json::json!({ "enabled": enabled }).to_string();
    fs::write(&temp_path, payload)
        .map_err(|error| format!("unable to persist monitor toggle: {error}"))?;
    crate::paths::replace_file(&temp_path, path)
        .map_err(|error| format!("unable to persist monitor toggle: {error}"))
}

fn init_watcher<R: Runtime>(
    inbox: &Path,
    slots: Slots,
    app: AppHandle<R>,
) -> Option<RecommendedWatcher> {
    let mut watcher = match RecommendedWatcher::new(
        move |result: Result<notify::Event, notify::Error>| {
            let Ok(event) = result else {
                return;
            };
            if !event.kind.is_create() && !event.kind.is_modify() {
                return;
            }
            for path in event.paths {
                if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
                    process_event_file(&path, &slots, &app);
                }
            }
        },
        notify::Config::default(),
    ) {
        Ok(watcher) => watcher,
        Err(error) => {
            log::warn!("Unable to create session monitor watcher: {error}");
            return None;
        }
    };

    if let Err(error) = watcher.watch(inbox, RecursiveMode::NonRecursive) {
        log::warn!("Unable to watch session monitor event inbox: {error}");
        None
    } else {
        Some(watcher)
    }
}

fn process_pending_events<R: Runtime>(inbox: &Path, slots: &Slots, app: &AppHandle<R>) {
    let Ok(entries) = fs::read_dir(inbox) else {
        return;
    };
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        process_event_file(&path, slots, app);
    }
}

fn process_event_file<R: Runtime>(path: &Path, slots: &Slots, app: &AppHandle<R>) {
    let content = match fs::read(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            log::warn!(
                "Unable to read session monitor event {}: {error}",
                path.display()
            );
            return;
        }
    };
    let event: HookEvent = match serde_json::from_slice(&content) {
        Ok(event) => event,
        Err(error) => {
            log::warn!(
                "Discarding invalid session monitor event {}: {error}",
                path.display()
            );
            let _ = fs::remove_file(path);
            return;
        }
    };

    apply_and_emit(slots, app, event);
    let _ = fs::remove_file(path);
}

fn apply_and_emit<R: Runtime>(slots: &Slots, app: &AppHandle<R>, event: HookEvent) {
    let agent = event.agent;
    let Some(slot) = slots.get(&agent) else {
        return;
    };
    let next_snapshot = {
        let Ok(mut current) = slot.snapshot.lock() else {
            return;
        };
        apply_event(&mut current, event);
        current.revision = current.revision.saturating_add(1);
        current.clone()
    };
    if let Err(error) = persist_snapshot(&slot.state_path, &next_snapshot) {
        log::warn!("Unable to persist session monitor state: {error}");
        return;
    }
    let _ = app.emit(agent.changed_event(), &next_snapshot);
}

fn apply_event(snapshot: &mut MonitorSnapshot, event: HookEvent) {
    // Defense in depth: Codex events captured before the hook-side filter
    // existed (or left over in the inbox) must not surface internal desktop
    // turns.
    if event.agent == AgentKind::Codex && event.hook_event_name == "UserPromptSubmit" {
        if let Some(prompt) = event.user_prompt.as_deref() {
            if super::capture::is_internal_system_prompt(prompt) {
                snapshot
                    .sessions
                    .retain(|session| session.session_id != event.session_id);
                return;
            }
        }
    }

    let index = snapshot
        .sessions
        .iter()
        .position(|session| session.session_id == event.session_id);
    let status = if event.hook_event_name == "UserPromptSubmit" {
        RuntimeStatus::Running
    } else {
        RuntimeStatus::Ended
    };

    if let Some(index) = index {
        let session = &mut snapshot.sessions[index];
        session.turn_id = event.turn_id;
        session.source = event.source;
        session.status = status;
        session.updated_at = event.occurred_at;
        if event.cwd.is_some() {
            session.cwd = event.cwd;
        }
        if event.user_prompt.is_some() {
            session.user_prompt = event.user_prompt;
            session.assistant_reply = None;
        }
        if event.assistant_reply.is_some() {
            session.assistant_reply = event.assistant_reply;
        }
    } else {
        snapshot.sessions.push(SessionState {
            session_id: event.session_id,
            turn_id: event.turn_id,
            source: event.source,
            status,
            cwd: event.cwd,
            user_prompt: event.user_prompt,
            assistant_reply: event.assistant_reply,
            updated_at: event.occurred_at,
        });
    }

    snapshot
        .sessions
        .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    snapshot.sessions.truncate(MAX_SESSIONS);
}

/// Remove a session by id. Returns true when a row was actually removed.
fn remove_session_from(snapshot: &mut MonitorSnapshot, session_id: &str) -> bool {
    let before = snapshot.sessions.len();
    snapshot
        .sessions
        .retain(|session| session.session_id != session_id);
    snapshot.sessions.len() != before
}

/// Drop sessions last updated before `cutoff`. Returns true when anything was pruned.
fn prune_sessions_older_than(snapshot: &mut MonitorSnapshot, cutoff: i64) -> bool {
    let before = snapshot.sessions.len();
    snapshot
        .sessions
        .retain(|session| session.updated_at >= cutoff);
    snapshot.sessions.len() != before
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

fn load_snapshot(path: &Path) -> MonitorSnapshot {
    let mut snapshot: MonitorSnapshot = fs::read(path)
        .ok()
        .and_then(|content| serde_json::from_slice(&content).ok())
        .unwrap_or_default();
    // One-time cleanup of internal desktop turns captured before filtering
    // existed (ambient suggestions, safety reviewer, memory consolidation).
    snapshot.sessions.retain(|session| {
        session
            .user_prompt
            .as_deref()
            .map(|prompt| !super::capture::is_internal_system_prompt(prompt))
            .unwrap_or(true)
    });
    snapshot
}

fn persist_snapshot(path: &Path, snapshot: &MonitorSnapshot) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "session monitor state has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("unable to create state directory: {error}"))?;
    let file_stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("state");
    let temp_path = parent.join(format!(".{file_stem}-{}.tmp", Uuid::new_v4()));
    let payload = serde_json::to_vec(snapshot)
        .map_err(|error| format!("unable to serialize monitor state: {error}"))?;
    let mut temp = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .map_err(|error| format!("unable to create temporary state: {error}"))?;
    temp.write_all(&payload)
        .and_then(|_| temp.sync_all())
        .map_err(|error| format!("unable to persist monitor state: {error}"))?;

    crate::paths::replace_file(&temp_path, path)
        .map_err(|error| format!("unable to replace state: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_monitor::types::SessionSource;

    fn event(name: &str, prompt: Option<&str>, reply: Option<&str>) -> HookEvent {
        HookEvent {
            event_id: Uuid::new_v4().to_string(),
            agent: AgentKind::Codex,
            hook_event_name: name.to_string(),
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            source: SessionSource::Terminal,
            cwd: Some("/tmp/project".to_string()),
            user_prompt: prompt.map(ToOwned::to_owned),
            assistant_reply: reply.map(ToOwned::to_owned),
            occurred_at: 42,
        }
    }

    #[test]
    fn prompt_and_stop_form_one_session_row() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("UserPromptSubmit", Some("question"), None),
        );
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Running);
        apply_event(&mut snapshot, event("Stop", None, Some("answer")));
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Ended);
        assert_eq!(
            snapshot.sessions[0].user_prompt.as_deref(),
            Some("question")
        );
        assert_eq!(
            snapshot.sessions[0].assistant_reply.as_deref(),
            Some("answer")
        );
    }

    #[test]
    fn internal_system_prompt_creates_no_session_row() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event(
                "UserPromptSubmit",
                Some("You are an expert at upholding safety and compliance standards for Codex ambient suggestions."),
                None,
            ),
        );
        assert!(snapshot.sessions.is_empty());
    }

    #[test]
    fn internal_prompt_filter_does_not_apply_to_other_agents() {
        let mut snapshot = MonitorSnapshot::default();
        let mut claude_event = event(
            "UserPromptSubmit",
            Some("You are an expert at upholding safety and compliance standards for Codex ambient suggestions."),
            None,
        );
        claude_event.agent = AgentKind::Claude;
        apply_event(&mut snapshot, claude_event);
        assert_eq!(snapshot.sessions.len(), 1);
    }

    #[test]
    fn internal_system_prompt_removes_existing_row() {
        // A session captured before the filter existed gets purged when its
        // internal prompt is re-classified.
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("UserPromptSubmit", Some("real question"), None),
        );
        assert_eq!(snapshot.sessions.len(), 1);
        apply_event(
            &mut snapshot,
            event(
                "UserPromptSubmit",
                Some("## Memory Writing Agent: Phase 2 (Consolidation)"),
                None,
            ),
        );
        assert!(snapshot.sessions.is_empty());
    }

    #[test]
    fn remove_session_from_deletes_only_the_matching_row() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("UserPromptSubmit", Some("question"), None),
        );
        assert!(remove_session_from(&mut snapshot, "session-1"));
        assert!(snapshot.sessions.is_empty());
        // Unknown id is a no-op and reports no change.
        assert!(!remove_session_from(&mut snapshot, "session-1"));
    }

    #[test]
    fn kiro_session_status_missing_lock_is_ended() {
        let directory = tempfile::tempdir().expect("temp dir should create");
        assert_eq!(
            kiro_session_status(directory.path(), "no-such-session"),
            RuntimeStatus::Ended
        );
    }

    #[test]
    fn kiro_session_status_dead_pid_is_ended() {
        let directory = tempfile::tempdir().expect("temp dir should create");
        // pid 2^22 + 12345 is practically never live on the test machine.
        fs::write(
            directory.path().join("s1.lock"),
            r#"{"pid":4206649,"started_at":"2026-01-01T00:00:00Z"}"#,
        )
        .expect("lock file should write");
        assert_eq!(
            kiro_session_status(directory.path(), "s1"),
            RuntimeStatus::Ended
        );
    }

    #[test]
    fn kiro_session_status_live_pid_is_running() {
        let directory = tempfile::tempdir().expect("temp dir should create");
        let pid = std::process::id();
        fs::write(
            directory.path().join("s2.lock"),
            format!(r#"{{"pid":{pid},"started_at":"2026-01-01T00:00:00Z"}}"#),
        )
        .expect("lock file should write");
        assert_eq!(
            kiro_session_status(directory.path(), "s2"),
            RuntimeStatus::Running
        );
    }

    /// Windows keeps the CLI's LockFileEx byte-range lock mandatory, so the
    /// lock file of a LIVE session is unreadable there. An existing lock we
    /// cannot read must stay Running. Simulated on Unix via permissions.
    #[cfg(unix)]
    #[test]
    fn kiro_session_status_unreadable_lock_is_running() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().expect("temp dir should create");
        let lock = directory.path().join("s3.lock");
        fs::write(&lock, r#"{"pid":1}"#).expect("lock file should write");
        fs::set_permissions(&lock, fs::Permissions::from_mode(0o000))
            .expect("permissions should change");
        assert_eq!(
            kiro_session_status(directory.path(), "s3"),
            RuntimeStatus::Running
        );
    }
}

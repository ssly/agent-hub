use super::types::{AgentKind, HookEvent, MonitorSnapshot, RuntimeStatus, SessionState};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};
use uuid::Uuid;

const MAX_SESSIONS: usize = 100;
/// Sessions older than this are pruned on every snapshot query.
const SESSION_TTL_MILLIS: i64 = 24 * 60 * 60 * 1000;

struct AgentSlot {
    snapshot: Mutex<MonitorSnapshot>,
    state_path: PathBuf,
}

type Slots = Arc<HashMap<AgentKind, AgentSlot>>;

pub struct SessionMonitorService<R: Runtime> {
    slots: Slots,
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

        Self {
            slots,
            _inbox_watcher: inbox_watcher,
            _app: app,
        }
    }

    pub fn snapshot(&self, agent: AgentKind) -> MonitorSnapshot {
        self.prune_expired(agent);
        self.slots
            .get(&agent)
            .and_then(|slot| slot.snapshot.lock().ok().map(|snapshot| snapshot.clone()))
            .unwrap_or_default()
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
    // Only the turn boundaries decide status: UserPromptSubmit → Running,
    // Stop → Ended. Cursor's AssistantResponse (afterAgentResponse) may fire
    // multiple times within one generation, so it must only attach the reply
    // text, never flip the state; the generation's stop event marks the end.
    let status = match event.hook_event_name.as_str() {
        "UserPromptSubmit" => RuntimeStatus::Running,
        "Stop" => RuntimeStatus::Ended,
        _ => match index {
            Some(index) => snapshot.sessions[index].status,
            // A reply event without a preceding prompt (hook installed
            // mid-session) defaults to Ended rather than a stuck Running.
            None => RuntimeStatus::Ended,
        },
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
    fn cursor_three_event_lifecycle_forms_one_session_row() {
        let mut snapshot = MonitorSnapshot::default();
        let mut prompt = event("UserPromptSubmit", Some("Cursor question"), None);
        prompt.agent = AgentKind::Cursor;
        prompt.session_id = "cursor-conversation-1".to_string();
        prompt.turn_id = "cursor-generation-1".to_string();
        prompt.source = SessionSource::Cursor;
        apply_event(&mut snapshot, prompt);
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Running);

        // afterAgentResponse only attaches the reply — it may fire multiple
        // times within one generation and must not end the turn.
        let mut response = event("AssistantResponse", None, Some("Cursor answer"));
        response.agent = AgentKind::Cursor;
        response.session_id = "cursor-conversation-1".to_string();
        response.turn_id = "cursor-generation-1".to_string();
        response.source = SessionSource::Cursor;
        apply_event(&mut snapshot, response);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Running);
        assert_eq!(
            snapshot.sessions[0].assistant_reply.as_deref(),
            Some("Cursor answer")
        );

        let mut stop = event("Stop", None, None);
        stop.agent = AgentKind::Cursor;
        stop.session_id = "cursor-conversation-1".to_string();
        stop.turn_id = "cursor-generation-1".to_string();
        stop.source = SessionSource::Cursor;
        apply_event(&mut snapshot, stop);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Ended);
        assert_eq!(
            snapshot.sessions[0].user_prompt.as_deref(),
            Some("Cursor question")
        );
    }

    #[test]
    fn assistant_response_without_prompt_defaults_to_ended() {
        // Hook installed mid-session: the reply event arrives first. The row
        // defaults to Ended instead of a stuck "running".
        let mut snapshot = MonitorSnapshot::default();
        apply_event(&mut snapshot, event("AssistantResponse", None, Some("reply")));
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Ended);
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
}

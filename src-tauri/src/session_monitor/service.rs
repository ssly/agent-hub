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
const SESSION_TTL_MILLIS: i64 = 30 * 24 * 60 * 60 * 1000;

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
        let Ok(mut current) = slot.snapshot.lock() else {
            return Err("session monitor state is unavailable".to_string());
        };
        if !remove_session_from(&mut current, session_id) {
            return Ok(());
        }
        current.revision = current.revision.saturating_add(1);
        let next_snapshot = current.clone();
        persist_snapshot(&slot.state_path, &next_snapshot)?;
        let _ = self._app.emit(agent.changed_event(), &next_snapshot);
        Ok(())
    }

    /// Clear a row's unread marker only if it is still the version the caller
    /// observed. This prevents a late hover from acknowledging an update that
    /// arrived after the pointer entered the row.
    pub fn mark_session_read(
        &self,
        agent: AgentKind,
        session_id: &str,
        observed_updated_at: i64,
    ) -> Result<(), String> {
        let Some(slot) = self.slots.get(&agent) else {
            return Ok(());
        };
        // Keep the mutation, disk write and event in one critical section.
        // A hook event that arrives immediately after this acknowledgement
        // must be persisted and broadcast after it, so a stale hover can
        // never overwrite that newer unread version.
        let Ok(mut current) = slot.snapshot.lock() else {
            return Err("session monitor state is unavailable".to_string());
        };
        if !mark_session_read_in(&mut current, session_id, observed_updated_at) {
            return Ok(());
        }
        current.revision = current.revision.saturating_add(1);
        let next_snapshot = current.clone();
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
        let Ok(mut current) = slot.snapshot.lock() else {
            return;
        };
        if !prune_sessions_older_than(&mut current, cutoff) {
            return;
        }
        current.revision = current.revision.saturating_add(1);
        let next_snapshot = current.clone();
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
    // Serialize persistence and emission with all other snapshot mutations.
    // In particular, an incoming hook update cannot be followed by an older
    // mark-read snapshot escaping after it.
    let Ok(mut current) = slot.snapshot.lock() else {
        return;
    };
    apply_event(&mut current, event);
    current.revision = current.revision.saturating_add(1);
    let next_snapshot = current.clone();
    if let Err(error) = persist_snapshot(&slot.state_path, &next_snapshot) {
        log::warn!("Unable to persist session monitor state: {error}");
        return;
    }
    let _ = app.emit(agent.changed_event(), &next_snapshot);
}

/// Only a normally completed agent turn is a new item for the user to read.
/// Start, progress, approval, failure, and interruption events still update
/// the monitor state, but must not light the unread badge before a reply ends.
fn marks_unread_on_completion(event_name: &str) -> bool {
    event_name == "Stop"
}

/// Clear an earlier unread marker as soon as a fresh turn is known to be in
/// progress. This also repairs rows that were marked by the old policy before
/// the user upgrades, without clearing a completed reply on a stray event.
fn clears_unread_while_active(event_name: &str, status: RuntimeStatus) -> bool {
    match event_name {
        "UserPromptSubmit" | "PermissionRequest" | "StopFailure" | "Interrupted" => true,
        "PermissionResult" | "PermissionDenied" | "PostToolUse" | "AssistantResponse" => {
            matches!(status, RuntimeStatus::Running | RuntimeStatus::Waiting)
        }
        _ => false,
    }
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
    // Turn boundaries decide running/ended. PermissionRequest (and Grok's
    // Notification permission_prompt, already remapped in capture) paints
    // waiting; PermissionResult / PermissionDenied / PostToolUse resume the
    // turn without ending it. Cursor's AssistantResponse may fire multiple
    // times within one generation and must never flip the state.
    let status = match event.hook_event_name.as_str() {
        "UserPromptSubmit" => RuntimeStatus::Running,
        "Stop" => RuntimeStatus::Ended,
        "StopFailure" => RuntimeStatus::Failed,
        "Interrupted" => RuntimeStatus::Ended,
        "PermissionRequest" => RuntimeStatus::Waiting,
        "PermissionResult" | "PermissionDenied" | "PostToolUse" => match index {
            Some(index) if snapshot.sessions[index].status == RuntimeStatus::Waiting => {
                RuntimeStatus::Running
            }
            Some(index) => snapshot.sessions[index].status,
            None => RuntimeStatus::Ended,
        },
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
        if marks_unread_on_completion(&event.hook_event_name) {
            session.unread = true;
        } else if clears_unread_while_active(&event.hook_event_name, session.status) {
            session.unread = false;
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
            unread: marks_unread_on_completion(&event.hook_event_name),
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

/// Compare-and-set the unread marker against the version that was visible to
/// the caller. Returns true only when a row was actually acknowledged.
fn mark_session_read_in(
    snapshot: &mut MonitorSnapshot,
    session_id: &str,
    observed_updated_at: i64,
) -> bool {
    let Some(session) = snapshot
        .sessions
        .iter_mut()
        .find(|session| session.session_id == session_id)
    else {
        return false;
    };
    if !session.unread || session.updated_at != observed_updated_at {
        return false;
    }
    session.unread = false;
    true
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
    // Before this policy change, every running-state update could persist an
    // unread marker. Those rows are not completed replies, so never resurrect
    // their stale badge after an app restart.
    for session in &mut snapshot.sessions {
        if session.status != RuntimeStatus::Ended {
            session.unread = false;
        }
    }
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
        assert!(!snapshot.sessions[0].unread);
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
        assert!(snapshot.sessions[0].unread);
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
        assert!(!snapshot.sessions[0].unread);

        let mut stop = event("Stop", None, None);
        stop.agent = AgentKind::Cursor;
        stop.session_id = "cursor-conversation-1".to_string();
        stop.turn_id = "cursor-generation-1".to_string();
        stop.source = SessionSource::Cursor;
        apply_event(&mut snapshot, stop);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Ended);
        assert!(snapshot.sessions[0].unread);
        assert_eq!(
            snapshot.sessions[0].user_prompt.as_deref(),
            Some("Cursor question")
        );
    }

    #[test]
    fn stop_failure_marks_failed() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("UserPromptSubmit", Some("question"), None),
        );
        apply_event(&mut snapshot, event("StopFailure", None, None));
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Failed);
        assert!(!snapshot.sessions[0].unread);
    }

    #[test]
    fn permission_request_marks_waiting_and_result_resumes() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("UserPromptSubmit", Some("question"), None),
        );
        apply_event(&mut snapshot, event("PermissionRequest", None, None));
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Waiting);
        assert!(!snapshot.sessions[0].unread);
        assert_eq!(
            snapshot.sessions[0].user_prompt.as_deref(),
            Some("question")
        );
        apply_event(&mut snapshot, event("PermissionResult", None, None));
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Running);
        apply_event(&mut snapshot, event("Stop", None, Some("answer")));
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Ended);
        assert!(snapshot.sessions[0].unread);
    }

    #[test]
    fn post_tool_use_only_resumes_waiting_rows() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("UserPromptSubmit", Some("question"), None),
        );
        apply_event(&mut snapshot, event("PostToolUse", None, None));
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Running);
        apply_event(&mut snapshot, event("PermissionRequest", None, None));
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Waiting);
        apply_event(&mut snapshot, event("PostToolUse", None, None));
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Running);
    }

    #[test]
    fn orphan_post_tool_use_does_not_plant_a_running_row() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(&mut snapshot, event("PostToolUse", None, None));
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Ended);
    }

    #[test]
    fn assistant_response_without_prompt_defaults_to_ended() {
        // Hook installed mid-session: the reply event arrives first. The row
        // defaults to Ended instead of a stuck "running".
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("AssistantResponse", None, Some("reply")),
        );
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].status, RuntimeStatus::Ended);
        assert!(!snapshot.sessions[0].unread);
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
    fn unread_waits_for_normal_turn_completion() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("UserPromptSubmit", Some("question"), None),
        );
        assert!(!snapshot.sessions[0].unread);

        let mut waiting = event("PermissionRequest", None, None);
        waiting.occurred_at = 43;
        apply_event(&mut snapshot, waiting);
        assert!(!snapshot.sessions[0].unread);

        let mut reply = event("AssistantResponse", None, Some("answer"));
        reply.occurred_at = 44;
        apply_event(&mut snapshot, reply);
        assert!(!snapshot.sessions[0].unread);

        let mut completion = event("Stop", None, Some("answer"));
        completion.occurred_at = 45;
        apply_event(&mut snapshot, completion);
        assert!(snapshot.sessions[0].unread);
    }

    #[test]
    fn every_agent_marks_unread_only_after_normal_completion() {
        for agent in AgentKind::ALL {
            let mut snapshot = MonitorSnapshot::default();
            let mut start = event("UserPromptSubmit", Some("question"), None);
            start.agent = agent;
            apply_event(&mut snapshot, start);
            assert!(
                !snapshot.sessions[0].unread,
                "{} marked unread before replying",
                agent.as_str()
            );

            let mut progress = event("AssistantResponse", None, Some("partial reply"));
            progress.agent = agent;
            progress.occurred_at = 43;
            apply_event(&mut snapshot, progress);
            assert!(
                !snapshot.sessions[0].unread,
                "{} marked unread before the turn ended",
                agent.as_str()
            );

            let mut completion = event("Stop", None, Some("final reply"));
            completion.agent = agent;
            completion.occurred_at = 44;
            apply_event(&mut snapshot, completion);
            assert!(
                snapshot.sessions[0].unread,
                "{} did not mark the completed reply unread",
                agent.as_str()
            );
        }
    }

    #[test]
    fn failure_and_interruption_do_not_mark_unread() {
        for event_name in ["StopFailure", "Interrupted"] {
            let mut snapshot = MonitorSnapshot::default();
            apply_event(
                &mut snapshot,
                event("UserPromptSubmit", Some("question"), None),
            );
            apply_event(&mut snapshot, event(event_name, None, None));
            assert!(!snapshot.sessions[0].unread, "{event_name} marked unread");
        }
    }

    #[test]
    fn new_turn_clears_prior_completed_unread() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("UserPromptSubmit", Some("question"), None),
        );
        let mut completion = event("Stop", None, Some("answer"));
        completion.occurred_at = 43;
        apply_event(&mut snapshot, completion);
        assert!(snapshot.sessions[0].unread);

        let mut new_turn = event("UserPromptSubmit", Some("next question"), None);
        new_turn.occurred_at = 44;
        apply_event(&mut snapshot, new_turn);
        assert!(!snapshot.sessions[0].unread);
    }

    #[test]
    fn read_acknowledgement_requires_the_observed_version() {
        let mut snapshot = MonitorSnapshot::default();
        apply_event(
            &mut snapshot,
            event("UserPromptSubmit", Some("question"), None),
        );
        let mut completion = event("Stop", None, Some("answer"));
        completion.occurred_at = 43;
        apply_event(&mut snapshot, completion);

        assert!(mark_session_read_in(&mut snapshot, "session-1", 43));
        assert!(!snapshot.sessions[0].unread);

        let mut next_prompt = event("UserPromptSubmit", Some("next question"), None);
        next_prompt.occurred_at = 44;
        apply_event(&mut snapshot, next_prompt);
        let mut next_completion = event("Stop", None, Some("new answer"));
        next_completion.occurred_at = 45;
        apply_event(&mut snapshot, next_completion);
        assert!(snapshot.sessions[0].unread);
        assert!(!mark_session_read_in(&mut snapshot, "session-1", 43));
        assert!(snapshot.sessions[0].unread);
    }

    #[test]
    fn historical_state_without_unread_defaults_to_read() {
        let snapshot: MonitorSnapshot = serde_json::from_str(
            r#"{"sessions":[{"sessionId":"old","turnId":"turn","source":"terminal","status":"ended","updatedAt":1}]}"#,
        )
        .expect("old monitor state should deserialize");
        assert!(!snapshot.sessions[0].unread);
    }

    #[test]
    fn legacy_active_or_failed_unread_rows_are_read_on_load() {
        let path = std::env::temp_dir().join(format!("agent-hub-monitor-{}.json", Uuid::new_v4()));
        let snapshot = MonitorSnapshot {
            revision: 0,
            sessions: vec![
                SessionState {
                    session_id: "running".to_string(),
                    turn_id: "turn-1".to_string(),
                    source: SessionSource::Terminal,
                    status: RuntimeStatus::Running,
                    cwd: None,
                    user_prompt: None,
                    assistant_reply: None,
                    updated_at: 1,
                    unread: true,
                },
                SessionState {
                    session_id: "failed".to_string(),
                    turn_id: "turn-2".to_string(),
                    source: SessionSource::Terminal,
                    status: RuntimeStatus::Failed,
                    cwd: None,
                    user_prompt: None,
                    assistant_reply: None,
                    updated_at: 2,
                    unread: true,
                },
                SessionState {
                    session_id: "completed".to_string(),
                    turn_id: "turn-3".to_string(),
                    source: SessionSource::Terminal,
                    status: RuntimeStatus::Ended,
                    cwd: None,
                    user_prompt: None,
                    assistant_reply: Some("answer".to_string()),
                    updated_at: 3,
                    unread: true,
                },
            ],
        };
        fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();

        let loaded = load_snapshot(&path);
        assert!(!loaded.sessions[0].unread);
        assert!(!loaded.sessions[1].unread);
        assert!(loaded.sessions[2].unread);

        let _ = fs::remove_file(path);
    }
}

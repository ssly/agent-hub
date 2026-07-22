use super::types::{CodexHookEvent, CodexMonitorSnapshot, CodexSessionState, RuntimeStatus};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};
use uuid::Uuid;

const CHANGED_EVENT: &str = "session-monitor:codex-changed";
const MAX_SESSIONS: usize = 100;

pub struct CodexSessionMonitorService<R: Runtime> {
    snapshot: Arc<Mutex<CodexMonitorSnapshot>>,
    state_path: PathBuf,
    _watcher: Option<RecommendedWatcher>,
    _app: AppHandle<R>,
}

impl<R: Runtime> CodexSessionMonitorService<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        let root = dirs::home_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(".agent-hub/session-monitor");
        let inbox = root.join("inbox");
        let state_path = root.join("codex-state.json");
        let _ = fs::create_dir_all(&inbox);
        let snapshot = Arc::new(Mutex::new(load_snapshot(&state_path)));

        let watcher = init_watcher(&inbox, snapshot.clone(), state_path.clone(), app.clone());
        process_pending_events(&inbox, &snapshot, &state_path, &app);

        Self {
            snapshot,
            state_path,
            _watcher: watcher,
            _app: app,
        }
    }

    pub fn snapshot(&self) -> CodexMonitorSnapshot {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn state_path(&self) -> &Path {
        &self.state_path
    }
}

fn init_watcher<R: Runtime>(
    inbox: &Path,
    snapshot: Arc<Mutex<CodexMonitorSnapshot>>,
    state_path: PathBuf,
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
                    process_event_file(&path, &snapshot, &state_path, &app);
                }
            }
        },
        notify::Config::default(),
    ) {
        Ok(watcher) => watcher,
        Err(error) => {
            log::warn!("Unable to create Codex session monitor watcher: {error}");
            return None;
        }
    };

    if let Err(error) = watcher.watch(inbox, RecursiveMode::NonRecursive) {
        log::warn!("Unable to watch Codex session event inbox: {error}");
        None
    } else {
        Some(watcher)
    }
}

fn process_pending_events<R: Runtime>(
    inbox: &Path,
    snapshot: &Arc<Mutex<CodexMonitorSnapshot>>,
    state_path: &Path,
    app: &AppHandle<R>,
) {
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
        process_event_file(&path, snapshot, state_path, app);
    }
}

fn process_event_file<R: Runtime>(
    path: &Path,
    snapshot: &Arc<Mutex<CodexMonitorSnapshot>>,
    state_path: &Path,
    app: &AppHandle<R>,
) {
    let content = match fs::read(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            log::warn!(
                "Unable to read Codex session event {}: {error}",
                path.display()
            );
            return;
        }
    };
    let event: CodexHookEvent = match serde_json::from_slice(&content) {
        Ok(event) => event,
        Err(error) => {
            log::warn!(
                "Discarding invalid Codex session event {}: {error}",
                path.display()
            );
            let _ = fs::remove_file(path);
            return;
        }
    };

    let next_snapshot = {
        let Ok(mut current) = snapshot.lock() else {
            return;
        };
        apply_event(&mut current, event);
        current.revision = current.revision.saturating_add(1);
        current.clone()
    };
    if let Err(error) = persist_snapshot(state_path, &next_snapshot) {
        log::warn!("Unable to persist Codex session monitor state: {error}");
        return;
    }
    let _ = fs::remove_file(path);
    let _ = app.emit(CHANGED_EVENT, &next_snapshot);
}

fn apply_event(snapshot: &mut CodexMonitorSnapshot, event: CodexHookEvent) {
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
        snapshot.sessions.push(CodexSessionState {
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

fn load_snapshot(path: &Path) -> CodexMonitorSnapshot {
    fs::read(path)
        .ok()
        .and_then(|content| serde_json::from_slice(&content).ok())
        .unwrap_or_default()
}

fn persist_snapshot(path: &Path, snapshot: &CodexMonitorSnapshot) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "session monitor state has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("unable to create state directory: {error}"))?;
    let temp_path = parent.join(format!(".codex-state-{}.tmp", Uuid::new_v4()));
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

    #[cfg(not(target_os = "windows"))]
    fs::rename(&temp_path, path).map_err(|error| format!("unable to replace state: {error}"))?;
    #[cfg(target_os = "windows")]
    {
        let _ = fs::remove_file(path);
        fs::rename(&temp_path, path)
            .map_err(|error| format!("unable to replace state: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_monitor::types::SessionSource;

    fn event(name: &str, prompt: Option<&str>, reply: Option<&str>) -> CodexHookEvent {
        CodexHookEvent {
            event_id: Uuid::new_v4().to_string(),
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
        let mut snapshot = CodexMonitorSnapshot::default();
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
}

use super::types::{CodexHookEvent, SessionSource};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const HOOK_ARG: &str = "--agent-hub-codex-hook";
const MAX_HOOK_INPUT_BYTES: u64 = 256 * 1024;
const MAX_IGNORED_SESSIONS: usize = 200;

/// Prompt signatures of Codex desktop's internal background turns. The
/// desktop app runs its own hidden turns (ambient-suggestion generation, the
/// safety/compliance reviewer for those suggestions, memory consolidation),
/// and they fire UserPromptSubmit/Stop hooks just like real user turns. They
/// are never persisted to the Codex threads DB, so prompt matching is the
/// only way to recognize them. Automation prompts are user-configured and
/// intentionally NOT filtered here.
pub fn is_internal_system_prompt(prompt: &str) -> bool {
    let trimmed = prompt.trim_start();
    // Ambient-suggestion generator ("# Overview\n\nGenerate 0 to 3 hyperpersonalized…").
    if trimmed.starts_with("# Overview") && trimmed.contains("hyperpersonalized suggestions") {
        return true;
    }
    // Safety/compliance reviewer for ambient suggestions.
    if trimmed.contains("Codex ambient suggestions") {
        return true;
    }
    // Memory consolidation agent ("## Memory Writing Agent: Phase 2 …").
    if trimmed.starts_with("## Memory Writing Agent") {
        return true;
    }
    false
}

pub fn try_capture_codex_hook_event() -> bool {
    if !std::env::args().any(|arg| arg == HOOK_ARG) {
        return false;
    }

    if let Err(error) = capture_stdin_event() {
        // Hooks must never block or fail a Codex turn. This process intentionally
        // exits successfully even when the local event inbox is unavailable.
        eprintln!("agent-hub Codex hook capture skipped: {error}");
    }
    true
}

fn capture_stdin_event() -> Result<(), String> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(MAX_HOOK_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("unable to read hook input: {error}"))?;
    if bytes.len() as u64 > MAX_HOOK_INPUT_BYTES {
        return Err("hook input exceeded 256 KiB".to_string());
    }

    let input: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid hook JSON: {error}"))?;
    let event_name = input
        .get("hook_event_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "hook_event_name is missing".to_string())?;
    if event_name != "UserPromptSubmit" && event_name != "Stop" {
        return Ok(());
    }

    let event_id = Uuid::new_v4().to_string();
    let session_id = string_field(&input, "session_id")
        .or_else(|| std::env::var("CODEX_THREAD_ID").ok())
        .unwrap_or_else(|| format!("unknown-{event_id}"));
    let user_prompt = string_field(&input, "prompt");

    // Drop internal desktop turns (ambient suggestions, safety reviewer,
    // memory consolidation). Their Stop event carries no prompt, so remember
    // the session id and drop the matching Stop when it arrives.
    if event_name == "UserPromptSubmit" {
        if let Some(prompt) = user_prompt.as_deref() {
            if is_internal_system_prompt(prompt) {
                let _ = mark_session_ignored(&session_id);
                return Ok(());
            }
        }
    }
    if event_name == "Stop" && take_ignored_session(&session_id) {
        return Ok(());
    }

    let turn_id = string_field(&input, "turn_id").unwrap_or_else(|| event_id.clone());
    let event = CodexHookEvent {
        event_id: event_id.clone(),
        hook_event_name: event_name.to_string(),
        session_id,
        turn_id,
        source: detect_source(),
        cwd: string_field(&input, "cwd"),
        user_prompt,
        assistant_reply: string_field(&input, "last_assistant_message"),
        occurred_at: now_millis(),
    };

    let inbox = monitor_root()?.join("inbox");
    fs::create_dir_all(&inbox).map_err(|error| format!("unable to create event inbox: {error}"))?;
    let final_path = inbox.join(format!("{}-{event_id}.json", event.occurred_at));
    let temp_path = inbox.join(format!(".{event_id}.tmp"));
    let payload = serde_json::to_vec(&event)
        .map_err(|error| format!("unable to serialize hook event: {error}"))?;

    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .map_err(|error| format!("unable to create event file: {error}"))?;
    file.write_all(&payload)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("unable to persist hook event: {error}"))?;
    fs::rename(&temp_path, &final_path)
        .map_err(|error| format!("unable to publish hook event: {error}"))?;
    Ok(())
}

fn string_field(input: &serde_json::Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn detect_source() -> SessionSource {
    let originator = std::env::var("CODEX_INTERNAL_ORIGINATOR_OVERRIDE")
        .unwrap_or_default()
        .to_ascii_lowercase();
    source_from_originator(&originator)
}

fn source_from_originator(originator: &str) -> SessionSource {
    if originator.contains("desktop") || originator.contains("chatgpt") {
        SessionSource::Chatgpt
    } else {
        SessionSource::Terminal
    }
}

fn monitor_root() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".agent-hub").join("session-monitor"))
        .ok_or_else(|| "home directory is unavailable".to_string())
}

fn ignored_sessions_path() -> Result<PathBuf, String> {
    Ok(monitor_root()?.join("ignored-sessions.json"))
}

fn load_ignored_sessions() -> Vec<String> {
    let Ok(path) = ignored_sessions_path() else {
        return Vec::new();
    };
    fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_ignored_sessions(ids: &[String]) -> Result<(), String> {
    let path = ignored_sessions_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("unable to create ignored-session directory: {error}"))?;
    }
    let temp_path = path.with_file_name(format!(".ignored-sessions-{}.tmp", Uuid::new_v4()));
    let payload = serde_json::to_vec(ids)
        .map_err(|error| format!("unable to serialize ignored sessions: {error}"))?;
    fs::write(&temp_path, payload)
        .map_err(|error| format!("unable to persist ignored sessions: {error}"))?;
    crate::paths::replace_file(&temp_path, &path)
        .map_err(|error| format!("unable to persist ignored sessions: {error}"))
}

/// Remember a session whose events must be dropped (internal desktop turn).
fn mark_session_ignored(session_id: &str) -> Result<(), String> {
    let mut ids = load_ignored_sessions();
    if !ids.iter().any(|id| id == session_id) {
        ids.push(session_id.to_string());
    }
    if ids.len() > MAX_IGNORED_SESSIONS {
        let overflow = ids.len() - MAX_IGNORED_SESSIONS;
        ids.drain(..overflow);
    }
    save_ignored_sessions(&ids)
}

/// Remove and return whether the session was marked as ignored.
fn take_ignored_session(session_id: &str) -> bool {
    let mut ids = load_ignored_sessions();
    let Some(position) = ids.iter().position(|id| id == session_id) else {
        return false;
    };
    ids.remove(position);
    let _ = save_ignored_sessions(&ids);
    true
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_distinguishes_desktop_and_terminal_originators() {
        assert_eq!(
            source_from_originator("codex desktop"),
            SessionSource::Chatgpt
        );
        assert_eq!(source_from_originator("chatgpt"), SessionSource::Chatgpt);
        assert_eq!(source_from_originator("codex cli"), SessionSource::Terminal);
    }

    #[test]
    fn internal_prompt_matches_ambient_suggestion_generator() {
        let prompt = "# Overview\n\nGenerate 0 to 3 hyperpersonalized suggestions for what this user can do with Codex in this local project: /tmp/x";
        assert!(is_internal_system_prompt(prompt));
    }

    #[test]
    fn internal_prompt_matches_ambient_safety_reviewer() {
        let prompt = "You are an expert at upholding safety and compliance standards for Codex ambient suggestions.\n\nI will present…";
        assert!(is_internal_system_prompt(prompt));
    }

    #[test]
    fn internal_prompt_matches_memory_writing_agent() {
        let prompt = "## Memory Writing Agent: Phase 2 (Consolidation)\n\nYou are a Memory Writing Agent.";
        assert!(is_internal_system_prompt(prompt));
    }

    #[test]
    fn internal_prompt_keeps_automations_and_real_user_prompts() {
        // Automations are user-configured scheduled tasks — they stay visible.
        let automation = "Automation: 吾日三省\nAutomation ID: automation\nAutomation memory: $CODEX_HOME/automations/automation/memory.md\n\n扮演一名心理咨询师…";
        assert!(!is_internal_system_prompt(automation));
        assert!(!is_internal_system_prompt("帮我修一下这个 bug"));
        assert!(!is_internal_system_prompt("# Overview of my project\n\n请写一份项目概览"));
    }
}

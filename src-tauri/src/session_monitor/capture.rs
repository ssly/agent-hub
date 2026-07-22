use super::types::{CodexHookEvent, SessionSource};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const HOOK_ARG: &str = "--agent-hub-codex-hook";
const MAX_HOOK_INPUT_BYTES: u64 = 256 * 1024;

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
    let turn_id = string_field(&input, "turn_id").unwrap_or_else(|| event_id.clone());
    let event = CodexHookEvent {
        event_id: event_id.clone(),
        hook_event_name: event_name.to_string(),
        session_id,
        turn_id,
        source: detect_source(),
        cwd: string_field(&input, "cwd"),
        user_prompt: string_field(&input, "prompt"),
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
        .map(|home| home.join(".agent-hub/session-monitor"))
        .ok_or_else(|| "home directory is unavailable".to_string())
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
}

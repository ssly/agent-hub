use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::paths::join_relative;
use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "grok";

pub fn count_grok_sessions() -> Result<usize, String> {
    let sessions = collect_grok_sessions()?;
    Ok(sessions.len())
}

pub fn list_grok_sessions_all() -> Result<Vec<SessionSummary>, String> {
    collect_grok_sessions()
}

pub fn get_grok_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let root = grok_sessions_dir()?;
    let dir = find_grok_session_dir_in(&root, session_id)?;
    let fallback_ts = read_grok_summary(&dir.join("summary.json"))
        .map(|summary| summary.started_at)
        .unwrap_or(0);
    read_grok_messages_from_jsonl(&dir.join("chat_history.jsonl"), fallback_ts, offset, limit)
}

pub fn delete_grok_session(session_id: &str) -> Result<(), String> {
    let root = grok_sessions_dir()?;
    delete_grok_session_in_dir(&root, session_id)
}

/// Streaming scan that keeps only the latest user/assistant message, for the
/// resume preview. Never collects the full transcript into memory.
pub fn last_grok_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let root = grok_sessions_dir()?;
    let dir = find_grok_session_dir_in(&root, session_id)?;
    let fallback_ts = read_grok_summary(&dir.join("summary.json"))
        .map(|summary| summary.started_at)
        .unwrap_or(0);
    let file = fs::File::open(dir.join("chat_history.jsonl")).map_err(|err| err.to_string())?;
    let reader = BufReader::new(file);
    let mut last_user = None;
    let mut last_assistant = None;

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(message) = parse_grok_message_line(&value, fallback_ts) else {
            continue;
        };
        if message.role == "user" {
            last_user = Some(message);
        } else {
            last_assistant = Some(message);
        }
    }

    Ok((last_user, last_assistant))
}

pub fn search_grok_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let root = grok_sessions_dir()?;
    let sessions = collect_grok_sessions()?;
    let mut results = Vec::new();
    for session in sessions {
        let Ok(dir) = find_grok_session_dir_in(&root, &session.id) else {
            continue;
        };
        if let Ok(messages) =
            read_grok_messages_from_jsonl(&dir.join("chat_history.jsonl"), session.started_at, 0, 999999)
        {
            for msg in messages {
                if msg.content.to_lowercase().contains(query_lower) {
                    results.push(crate::session::SessionSearchResult {
                        session_id: session.id.clone(),
                        session_title: session.title.clone(),
                        project_path: session.project_path.clone(),
                        platform_id: PLATFORM_ID.to_string(),
                        message: msg,
                    });
                }
            }
        }
    }
    Ok(results)
}

fn collect_grok_sessions() -> Result<Vec<SessionSummary>, String> {
    let root = grok_sessions_dir()?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for summary_path in list_grok_summary_paths(&root) {
        if let Some(summary) = read_grok_summary(&summary_path) {
            sessions.push(summary);
        }
    }

    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(sessions)
}

/// Enumerate `sessions/<encoded-cwd>/<session-id>/summary.json` candidates.
fn list_grok_summary_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let project_entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return paths,
    };
    for project_entry in project_entries.flatten() {
        if !project_entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let session_entries = match fs::read_dir(project_entry.path()) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for session_entry in session_entries.flatten() {
            if !session_entry
                .file_type()
                .map(|file_type| file_type.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            let summary_path = session_entry.path().join("summary.json");
            if summary_path.is_file() {
                paths.push(summary_path);
            }
        }
    }
    paths
}

fn read_grok_summary(path: &Path) -> Option<SessionSummary> {
    let fallback_id = path
        .parent()
        .and_then(|dir| dir.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let fallback_updated_at = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(system_time_to_ms)
        .unwrap_or(0);
    let content = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&content).ok()?;
    parse_grok_summary(&value, &fallback_id, fallback_updated_at)
}

fn parse_grok_summary(
    value: &Value,
    fallback_id: &str,
    fallback_updated_at: i64,
) -> Option<SessionSummary> {
    let id = value
        .get("info")
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| fallback_id.to_string());

    if id.is_empty() {
        return None;
    }

    let title = value
        .get("session_summary")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| id.clone());

    let project_path = value
        .get("info")
        .and_then(|v| v.get("cwd"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .unwrap_or_default();

    let model = value
        .get("current_model_id")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let started_at = value
        .get("created_at")
        .and_then(|v| v.as_str())
        .and_then(parse_rfc3339_to_ms)
        .unwrap_or(fallback_updated_at);

    let updated_at = value
        .get("updated_at")
        .and_then(|v| v.as_str())
        .and_then(parse_rfc3339_to_ms)
        .unwrap_or(fallback_updated_at);

    let message_count = value
        .get("num_messages")
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());

    Some(SessionSummary {
        id,
        title,
        project_path,
        model,
        started_at,
        updated_at,
        message_count,
        tokens_used: None,
        platform_id: PLATFORM_ID.to_string(),
        source: None,
    })
}

/// Locate `sessions/<encoded-cwd>/<session-id>/`. The directory name is the
/// session UUID, so a name match is the fast path; fall back to comparing
/// `summary.json`'s `info.id` for possible future naming differences.
fn find_grok_session_dir_in(root: &Path, session_id: &str) -> Result<PathBuf, String> {
    if !root.exists() {
        return Err("Grok session directory not found: ~/.grok/sessions".to_string());
    }

    let project_entries = fs::read_dir(root).map_err(|err| err.to_string())?;
    let mut project_dirs = Vec::new();
    for entry in project_entries.flatten() {
        if entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            project_dirs.push(entry.path());
        }
    }

    for project_dir in &project_dirs {
        let direct = project_dir.join(session_id);
        if direct.is_dir() {
            return Ok(direct);
        }
    }

    for project_dir in &project_dirs {
        let session_entries = match fs::read_dir(project_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in session_entries.flatten() {
            if !entry
                .file_type()
                .map(|file_type| file_type.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            let summary_path = entry.path().join("summary.json");
            let Ok(content) = fs::read_to_string(&summary_path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&content) else {
                continue;
            };
            let matches = value
                .get("info")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
                .map(|id| id == session_id)
                .unwrap_or(false);
            if matches {
                return Ok(entry.path());
            }
        }
    }

    Err(format!("Grok session not found for id: {}", session_id))
}

fn delete_grok_session_in_dir(root: &Path, session_id: &str) -> Result<(), String> {
    let dir = find_grok_session_dir_in(root, session_id)?;
    fs::remove_dir_all(&dir).map_err(|err| {
        format!(
            "Failed to delete Grok session {}: {}",
            dir.display(),
            err
        )
    })
}

fn read_grok_messages_from_jsonl(
    path: &Path,
    fallback_ts: i64,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let file = fs::File::open(path).map_err(|err| err.to_string())?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut matched = 0usize;
    let page_limit = limit.max(1);

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(message) = parse_grok_message_line(&value, fallback_ts) else {
            continue;
        };

        if matched >= offset {
            messages.push(message);
            if messages.len() >= page_limit {
                break;
            }
        }
        matched += 1;
    }

    Ok(messages)
}

/// chat_history.jsonl records: `system`/`reasoning` are skipped; synthetic
/// `user` records (system reminders, project instructions) carry
/// `synthetic_reason` and are skipped too — real prompts carry `prompt_index`.
/// Records have no per-message timestamp, so the session start time is used.
fn parse_grok_message_line(value: &Value, fallback_ts: i64) -> Option<SessionMessage> {
    let kind = value.get("type").and_then(|v| v.as_str())?;
    match kind {
        "user" => {
            value.get("prompt_index")?;
            let content = extract_grok_text_content(value.get("content")?)?;
            Some(SessionMessage {
                role: "user".to_string(),
                content: strip_user_query_wrapper(&content),
                timestamp: fallback_ts,
            })
        }
        "assistant" => {
            let content = extract_grok_text_content(value.get("content")?)?;
            Some(SessionMessage {
                role: "assistant".to_string(),
                content,
                timestamp: fallback_ts,
            })
        }
        _ => None,
    }
}

fn extract_grok_text_content(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if item.get("type").and_then(|v| v.as_str()) != Some("text") {
                    continue;
                }
                let Some(text) = item.get("text").and_then(|v| v.as_str()) else {
                    continue;
                };
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}

/// Real user prompts are wrapped in `<user_query>…</user_query>`; unwrap for display.
fn strip_user_query_wrapper(content: &str) -> String {
    const OPEN: &str = "<user_query>";
    const CLOSE: &str = "</user_query>";
    let trimmed = content.trim();
    if trimmed.starts_with(OPEN) && trimmed.ends_with(CLOSE) {
        trimmed[OPEN.len()..trimmed.len() - CLOSE.len()]
            .trim()
            .to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_rfc3339_to_ms(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

fn system_time_to_ms(time: std::time::SystemTime) -> Option<i64> {
    time.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

fn grok_sessions_dir() -> Result<PathBuf, String> {
    match std::env::var("GROK_HOME") {
        Ok(dir) => Ok(PathBuf::from(dir).join("sessions")),
        Err(_) => {
            let home =
                dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
            Ok(join_relative(home, ".grok/sessions"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn write_session(root: &Path, project_dir: &str, session_id: &str) -> PathBuf {
        let dir = root.join(project_dir).join(session_id);
        fs::create_dir_all(&dir).expect("session dir should create");
        fs::write(
            dir.join("summary.json"),
            json!({
                "info": {"id": session_id, "cwd": "/tmp/work"},
                "session_summary": "test-title",
                "created_at": "2026-07-17T04:56:35.092418Z",
                "updated_at": "2026-07-17T05:01:32.609015Z",
                "num_messages": 4,
                "current_model_id": "grok-4.5"
            })
            .to_string(),
        )
        .expect("summary should write");
        dir
    }

    #[test]
    fn parse_grok_summary_maps_required_fields() {
        let value = json!({
            "info": {"id": "abc-123", "cwd": "/tmp/work"},
            "session_summary": "test-title",
            "created_at": "2026-07-17T04:56:35.092418Z",
            "updated_at": "2026-07-17T05:01:32.609015Z",
            "num_messages": 4,
            "current_model_id": "grok-4.5"
        });

        let summary = parse_grok_summary(&value, "fallback", 0).expect("summary should parse");
        assert_eq!(summary.id, "abc-123");
        assert_eq!(summary.title, "test-title");
        assert_eq!(summary.project_path, "/tmp/work");
        assert_eq!(summary.model.as_deref(), Some("grok-4.5"));
        assert_eq!(summary.message_count, Some(4));
        assert_eq!(summary.platform_id, "grok");
        assert!(summary.started_at > 0);
        assert!(summary.updated_at > 0);
    }

    #[test]
    fn parse_grok_summary_fallbacks_when_fields_missing() {
        let value = json!({"info": {"id": ""}});

        let summary = parse_grok_summary(&value, "fallback-id", 123).expect("summary should parse");
        assert_eq!(summary.id, "fallback-id");
        assert_eq!(summary.title, "fallback-id");
        assert_eq!(summary.started_at, 123);
        assert_eq!(summary.updated_at, 123);
        assert_eq!(summary.message_count, None);
    }

    #[test]
    fn parse_grok_message_line_maps_real_prompt_and_assistant() {
        let prompt = json!({
            "type": "user",
            "content": [{"type": "text", "text": "<user_query>\n你好\n</user_query>"}],
            "prompt_index": 0
        });
        let assistant = json!({
            "type": "assistant",
            "content": "你好！",
            "model_id": "grok-4.5"
        });

        let user_msg = parse_grok_message_line(&prompt, 42).expect("prompt should parse");
        assert_eq!(user_msg.role, "user");
        assert_eq!(user_msg.content, "你好");
        assert_eq!(user_msg.timestamp, 42);

        let assistant_msg = parse_grok_message_line(&assistant, 42).expect("assistant should parse");
        assert_eq!(assistant_msg.role, "assistant");
        assert_eq!(assistant_msg.content, "你好！");
        assert_eq!(assistant_msg.timestamp, 42);
    }

    #[test]
    fn parse_grok_message_line_skips_synthetic_and_non_message_records() {
        let synthetic = json!({
            "type": "user",
            "content": [{"type": "text", "text": "reminder"}],
            "synthetic_reason": "system_reminder"
        });
        let system = json!({"type": "system", "content": "..."});
        let reasoning = json!({"type": "reasoning", "summary": "[]"});

        assert!(parse_grok_message_line(&synthetic, 0).is_none());
        assert!(parse_grok_message_line(&system, 0).is_none());
        assert!(parse_grok_message_line(&reasoning, 0).is_none());
    }

    #[test]
    fn read_grok_messages_respects_offset_limit() {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "agent-hub-grok-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        );
        path.push(unique);

        let mut file = fs::File::create(&path).expect("temp file should create");
        writeln!(
            file,
            "{}",
            json!({"type":"user","content":[{"type":"text","text":"u1"}],"prompt_index":0})
        )
        .expect("line should write");
        writeln!(file, "{}", json!({"type":"assistant","content":"a1"}))
            .expect("line should write");
        writeln!(
            file,
            "{}",
            json!({"type":"user","content":[{"type":"text","text":"u2"}],"prompt_index":1})
        )
        .expect("line should write");
        file.flush().expect("flush should succeed");

        let page = read_grok_messages_from_jsonl(&path, 0, 1, 2).expect("page should load");
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].content, "a1");
        assert_eq!(page[1].content, "u2");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn delete_grok_session_removes_session_directory() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        let dir = write_session(temp.path(), "%2Ftmp%2Fwork", "session-1");
        assert!(dir.exists());

        delete_grok_session_in_dir(temp.path(), "session-1").expect("delete should succeed");

        assert!(!dir.exists());
    }

    #[test]
    fn delete_grok_session_returns_not_found() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        let err = delete_grok_session_in_dir(temp.path(), "missing")
            .expect_err("missing session should fail");
        assert!(err.contains("not found"));
    }

    #[test]
    fn find_grok_session_dir_falls_back_to_summary_id() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        // Directory name differs from the id inside summary.json.
        write_session(temp.path(), "%2Ftmp%2Fwork", "dir-name");
        let summary_path = temp
            .path()
            .join("%2Ftmp%2Fwork/dir-name/summary.json");
        fs::write(
            &summary_path,
            json!({"info": {"id": "real-id", "cwd": "/tmp/work"}}).to_string(),
        )
        .expect("summary should rewrite");

        let found = find_grok_session_dir_in(temp.path(), "real-id").expect("dir should be found");
        assert!(found.ends_with("dir-name"));
    }
}

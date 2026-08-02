use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::paths::join_relative;
use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "kimi";

pub fn count_kimi_sessions() -> Result<usize, String> {
    let sessions = collect_kimi_sessions()?;
    Ok(sessions.len())
}

pub fn list_kimi_sessions_all() -> Result<Vec<SessionSummary>, String> {
    collect_kimi_sessions()
}

pub fn get_kimi_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let root = kimi_sessions_dir()?;
    let dir = find_kimi_session_dir_in(&root, session_id)?;
    read_kimi_messages_from_jsonl(&wire_file_path(&dir), offset, limit)
}

pub fn delete_kimi_session(session_id: &str) -> Result<(), String> {
    let root = kimi_sessions_dir()?;
    delete_kimi_session_in_dir(&root, session_id)
}

/// Streaming scan that keeps only the latest user/assistant message, for the
/// resume preview. Never collects the full transcript into memory.
pub fn last_kimi_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let root = kimi_sessions_dir()?;
    let dir = find_kimi_session_dir_in(&root, session_id)?;
    let file = fs::File::open(wire_file_path(&dir)).map_err(|err| err.to_string())?;
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
        let Some(message) = parse_kimi_wire_line(&value) else {
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

pub fn search_kimi_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let root = kimi_sessions_dir()?;
    let sessions = collect_kimi_sessions()?;
    let mut results = Vec::new();
    for session in sessions {
        let Ok(dir) = find_kimi_session_dir_in(&root, &session.id) else {
            continue;
        };
        if let Ok(messages) = read_kimi_messages_from_jsonl(&wire_file_path(&dir), 0, 999999) {
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

/// Kimi Code stores sessions under
/// `~/.kimi-code/sessions/<wd_<name>_<hash>>/session_<uuid>/` with a
/// `state.json` (title + RFC3339 timestamps). The working directory is not in
/// `state.json` — it comes from `~/.kimi-code/session_index.jsonl`
/// (`{sessionId, sessionDir, workDir}` per line).
fn collect_kimi_sessions() -> Result<Vec<SessionSummary>, String> {
    let root = kimi_sessions_dir()?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let work_dirs = read_session_index(&root);
    let mut sessions = Vec::new();
    for state_path in list_kimi_state_paths(&root) {
        if let Some(summary) = read_kimi_state(&state_path, &work_dirs) {
            sessions.push(summary);
        }
    }

    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(sessions)
}

/// sessionId (the `session_<uuid>` directory name) → workDir.
fn read_session_index(sessions_root: &Path) -> HashMap<String, String> {
    let index_path = sessions_root
        .parent()
        .map(|dir| dir.join("session_index.jsonl"))
        .unwrap_or_default();
    let mut map = HashMap::new();
    let Ok(file) = fs::File::open(&index_path) else {
        return map;
    };
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { continue };
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let (Some(id), Some(work_dir)) = (
            value.get("sessionId").and_then(|v| v.as_str()),
            value.get("workDir").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        map.insert(id.to_string(), work_dir.to_string());
    }
    map
}

/// Enumerate `sessions/<wd-dir>/<session-dir>/state.json` candidates.
fn list_kimi_state_paths(root: &Path) -> Vec<PathBuf> {
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
            let state_path = session_entry.path().join("state.json");
            if state_path.is_file() {
                paths.push(state_path);
            }
        }
    }
    paths
}

fn read_kimi_state(
    path: &Path,
    work_dirs: &HashMap<String, String>,
) -> Option<SessionSummary> {
    let id = path
        .parent()
        .and_then(|dir| dir.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    if id.is_empty() {
        return None;
    }
    let fallback_ts = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(system_time_to_ms)
        .unwrap_or(0);
    let content = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&content).ok()?;

    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| id.clone());

    let started_at = value
        .get("createdAt")
        .and_then(|v| v.as_str())
        .and_then(parse_rfc3339_to_ms)
        .unwrap_or(fallback_ts);

    let updated_at = value
        .get("updatedAt")
        .and_then(|v| v.as_str())
        .and_then(parse_rfc3339_to_ms)
        .unwrap_or(fallback_ts);

    let project_path = work_dirs.get(&id).cloned().unwrap_or_default();

    Some(SessionSummary {
        id,
        title,
        project_path,
        model: None,
        started_at,
        updated_at,
        message_count: None,
        tokens_used: None,
        platform_id: PLATFORM_ID.to_string(),
        source: None,
    })
}

/// Locate `sessions/<wd-dir>/<session-id>/`. The directory name is the session
/// id (`session_<uuid>`), so a name match is the only lookup needed.
fn find_kimi_session_dir_in(root: &Path, session_id: &str) -> Result<PathBuf, String> {
    if !root.exists() {
        return Err("Kimi session directory not found: ~/.kimi-code/sessions".to_string());
    }

    let project_entries = fs::read_dir(root).map_err(|err| err.to_string())?;
    for entry in project_entries.flatten() {
        if !entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let direct = entry.path().join(session_id);
        if direct.is_dir() {
            return Ok(direct);
        }
    }

    Err(format!("Kimi session not found for id: {}", session_id))
}

fn delete_kimi_session_in_dir(root: &Path, session_id: &str) -> Result<(), String> {
    let dir = find_kimi_session_dir_in(root, session_id)?;
    fs::remove_dir_all(&dir).map_err(|err| {
        format!(
            "Failed to delete Kimi session {}: {}",
            dir.display(),
            err
        )
    })
}

/// The main agent's transcript lives in `agents/main/wire.jsonl`; subagent
/// transcripts are ignored, matching what the user sees in the CLI.
fn wire_file_path(session_dir: &Path) -> PathBuf {
    session_dir.join("agents").join("main").join("wire.jsonl")
}

fn read_kimi_messages_from_jsonl(
    path: &Path,
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
        let Some(message) = parse_kimi_wire_line(&value) else {
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

/// wire.jsonl is Kimi's event log, not a message list. Two event kinds map to
/// chat messages:
/// - `turn.prompt` with `origin.kind == "user"` — a real user prompt. Synthetic
///   injections (system reminders etc.) only appear as `context.append_message`,
///   never as `turn.prompt`, so they are naturally excluded.
/// - `context.append_loop_event` wrapping `content.part` with `part.type ==
///   "text"` — an assistant reply chunk. `think` parts are reasoning and skipped.
/// Timestamps are the top-level `time` field (epoch millis).
fn parse_kimi_wire_line(value: &Value) -> Option<SessionMessage> {
    let kind = value.get("type").and_then(|v| v.as_str())?;
    let timestamp = value.get("time").and_then(|v| v.as_i64()).unwrap_or(0);
    match kind {
        "turn.prompt" => {
            if value
                .get("origin")
                .and_then(|v| v.get("kind"))
                .and_then(|v| v.as_str())
                != Some("user")
            {
                return None;
            }
            let content = extract_text_parts(value.get("input")?)?;
            Some(SessionMessage {
                role: "user".to_string(),
                content,
                timestamp,
            })
        }
        "context.append_loop_event" => {
            let event = value.get("event")?;
            if event.get("type").and_then(|v| v.as_str()) != Some("content.part") {
                return None;
            }
            let part = event.get("part")?;
            if part.get("type").and_then(|v| v.as_str()) != Some("text") {
                return None;
            }
            let text = part.get("text").and_then(|v| v.as_str())?.trim();
            if text.is_empty() {
                return None;
            }
            Some(SessionMessage {
                role: "assistant".to_string(),
                content: text.to_string(),
                timestamp,
            })
        }
        _ => None,
    }
}

fn extract_text_parts(content: &Value) -> Option<String> {
    let items = content.as_array()?;
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

fn kimi_sessions_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    Ok(join_relative(home, ".kimi-code/sessions"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn write_session(root: &Path, project_dir: &str, session_id: &str) -> PathBuf {
        let dir = root.join(project_dir).join(session_id);
        fs::create_dir_all(dir.join("agents/main")).expect("session dir should create");
        fs::write(
            dir.join("state.json"),
            json!({
                "createdAt": "2026-07-17T15:03:19.201Z",
                "updatedAt": "2026-07-17T15:06:20.663Z",
                "title": "test-title"
            })
            .to_string(),
        )
        .expect("state should write");
        dir
    }

    #[test]
    fn read_kimi_state_maps_required_fields() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        let state_path = write_session(temp.path(), "wd_demo_abc", "session_1").join("state.json");
        let work_dirs = HashMap::from([("session_1".to_string(), "/tmp/work".to_string())]);

        let summary = read_kimi_state(&state_path, &work_dirs).expect("state should parse");
        assert_eq!(summary.id, "session_1");
        assert_eq!(summary.title, "test-title");
        assert_eq!(summary.project_path, "/tmp/work");
        assert_eq!(summary.platform_id, "kimi");
        assert!(summary.started_at > 0);
        assert!(summary.updated_at > summary.started_at);
    }

    #[test]
    fn read_kimi_state_falls_back_to_id_as_title() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        let dir = temp.path().join("wd_demo_abc/session_2");
        fs::create_dir_all(&dir).expect("session dir should create");
        let state_path = dir.join("state.json");
        fs::write(&state_path, json!({"title": "  "}).to_string()).expect("state should write");

        let summary = read_kimi_state(&state_path, &HashMap::new()).expect("state should parse");
        assert_eq!(summary.title, "session_2");
        assert_eq!(summary.project_path, "");
    }

    #[test]
    fn parse_kimi_wire_line_maps_prompt_and_assistant_text() {
        let prompt = json!({
            "type": "turn.prompt",
            "input": [{"type": "text", "text": "你好"}],
            "origin": {"kind": "user"},
            "time": 1784300693539i64
        });
        let assistant = json!({
            "type": "context.append_loop_event",
            "event": {"type": "content.part", "part": {"type": "text", "text": "你好！"}},
            "time": 1784300703620i64
        });

        let user_msg = parse_kimi_wire_line(&prompt).expect("prompt should parse");
        assert_eq!(user_msg.role, "user");
        assert_eq!(user_msg.content, "你好");
        assert_eq!(user_msg.timestamp, 1784300693539);

        let assistant_msg = parse_kimi_wire_line(&assistant).expect("assistant should parse");
        assert_eq!(assistant_msg.role, "assistant");
        assert_eq!(assistant_msg.content, "你好！");
        assert_eq!(assistant_msg.timestamp, 1784300703620);
    }

    #[test]
    fn parse_kimi_wire_line_skips_synthetic_and_non_message_events() {
        // Synthetic user injections arrive as context.append_message, not turn.prompt.
        let synthetic = json!({
            "type": "context.append_message",
            "message": {"role": "user", "content": [{"type": "text", "text": "reminder"}]},
            "time": 1
        });
        let think = json!({
            "type": "context.append_loop_event",
            "event": {"type": "content.part", "part": {"type": "think", "think": "..."}},
            "time": 1
        });
        let tool_call = json!({
            "type": "context.append_loop_event",
            "event": {"type": "tool.call", "name": "Read"},
            "time": 1
        });

        assert!(parse_kimi_wire_line(&synthetic).is_none());
        assert!(parse_kimi_wire_line(&think).is_none());
        assert!(parse_kimi_wire_line(&tool_call).is_none());
    }

    #[test]
    fn read_kimi_messages_respects_offset_limit() {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "agent-hub-kimi-{}.jsonl",
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
            json!({"type":"turn.prompt","input":[{"type":"text","text":"u1"}],"origin":{"kind":"user"},"time":1})
        )
        .expect("line should write");
        writeln!(
            file,
            "{}",
            json!({"type":"context.append_loop_event","event":{"type":"content.part","part":{"type":"text","text":"a1"}},"time":2})
        )
        .expect("line should write");
        writeln!(
            file,
            "{}",
            json!({"type":"turn.prompt","input":[{"type":"text","text":"u2"}],"origin":{"kind":"user"},"time":3})
        )
        .expect("line should write");
        file.flush().expect("flush should succeed");

        let page = read_kimi_messages_from_jsonl(&path, 1, 2).expect("page should load");
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].content, "a1");
        assert_eq!(page[1].content, "u2");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn delete_kimi_session_removes_session_directory() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        let dir = write_session(temp.path(), "wd_demo_abc", "session_1");
        assert!(dir.exists());

        delete_kimi_session_in_dir(temp.path(), "session_1").expect("delete should succeed");

        assert!(!dir.exists());
    }

    #[test]
    fn find_kimi_session_dir_matches_across_project_dirs() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        write_session(temp.path(), "wd_one_aaa", "session_1");
        write_session(temp.path(), "wd_two_bbb", "session_2");

        let found = find_kimi_session_dir_in(temp.path(), "session_2").expect("dir should be found");
        assert!(found.ends_with("wd_two_bbb/session_2"));
    }
}

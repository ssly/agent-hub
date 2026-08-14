use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::paths::join_relative;
use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "qwen";

pub fn count_qwen_sessions() -> Result<usize, String> {
    let sessions = collect_qwen_sessions()?;
    Ok(sessions.len())
}

pub fn list_qwen_sessions_all() -> Result<Vec<SessionSummary>, String> {
    collect_qwen_sessions()
}

pub fn get_qwen_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let root = qwen_projects_dir()?;
    let file = find_qwen_session_file_in(&root, session_id)?;
    read_qwen_messages_from_jsonl(&file, offset, limit)
}

pub fn delete_qwen_session(session_id: &str) -> Result<(), String> {
    let root = qwen_projects_dir()?;
    delete_qwen_session_in_dir(&root, session_id)
}

/// Streaming scan that keeps only the latest user/assistant message, for the
/// resume preview. Never collects the full transcript into memory.
pub fn last_qwen_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let root = qwen_projects_dir()?;
    let file_path = find_qwen_session_file_in(&root, session_id)?;
    let file = fs::File::open(&file_path).map_err(|err| err.to_string())?;
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
        let Some(message) = parse_qwen_chat_record(&value) else {
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

pub fn search_qwen_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let root = qwen_projects_dir()?;
    let sessions = collect_qwen_sessions()?;
    let mut results = Vec::new();
    for session in sessions {
        let Ok(file) = find_qwen_session_file_in(&root, &session.id) else {
            continue;
        };
        if let Ok(messages) = read_qwen_messages_from_jsonl(&file, 0, 999999) {
            for msg in messages {
                if msg.matches_query(query_lower) {
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

/// Qwen Code stores sessions as single JSONL files under
/// `~/.qwen/projects/<sanitized-cwd>/chats/<sessionId>.jsonl`. Each line is a
/// ChatRecord (`{uuid, parentUuid, sessionId, timestamp, type, cwd, gitBranch,
/// message, usageMetadata, model}`). Archived sessions live in
/// `chats/archive/` and are skipped, matching how the CLI hides them.
fn collect_qwen_sessions() -> Result<Vec<SessionSummary>, String> {
    let root = qwen_projects_dir()?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for chat_path in list_qwen_chat_paths(&root) {
        if let Some(summary) = read_qwen_session(&chat_path) {
            sessions.push(summary);
        }
    }

    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(sessions)
}

/// Enumerate `projects/<sanitized-cwd>/chats/<sessionId>.jsonl` candidates.
/// Only direct children of `chats/` are taken — the `archive/` subdirectory
/// holds archived sessions and is skipped.
fn list_qwen_chat_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let project_entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return paths,
    };
    for project_entry in project_entries.flatten() {
        let chats_dir = project_entry.path().join("chats");
        let chat_entries = match fs::read_dir(&chats_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for chat_entry in chat_entries.flatten() {
            let path = chat_entry.path();
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                paths.push(path);
            }
        }
    }
    paths
}

fn read_qwen_session(path: &Path) -> Option<SessionSummary> {
    let id = path
        .file_stem()
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
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut started_at = 0i64;
    let mut updated_at = 0i64;
    let mut first_user_text: Option<String> = None;
    let mut project_path: Option<String> = None;
    let mut model: Option<String> = None;

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(parse_rfc3339_to_ms)
            .unwrap_or(0);
        if timestamp > 0 {
            if started_at == 0 {
                started_at = timestamp;
            }
            updated_at = timestamp;
        }
        if project_path.is_none() {
            project_path = value
                .get("cwd")
                .and_then(|v| v.as_str())
                .filter(|cwd| !cwd.trim().is_empty())
                .map(|cwd| cwd.to_string());
        }
        if model.is_none() {
            model = value
                .get("model")
                .and_then(|v| v.as_str())
                .filter(|name| !name.trim().is_empty())
                .map(|name| name.to_string());
        }
        if first_user_text.is_none()
            && value.get("type").and_then(|v| v.as_str()) == Some("user")
        {
            first_user_text = value.get("message").and_then(extract_message_text);
        }
    }

    let title = first_user_text
        .map(|text| truncate_chars(text, 80))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| id.clone());

    Some(SessionSummary {
        id,
        title,
        project_path: project_path.unwrap_or_default(),
        model,
        started_at: if started_at > 0 { started_at } else { fallback_ts },
        updated_at: if updated_at > 0 { updated_at } else { fallback_ts },
        message_count: None,
        tokens_used: None,
        platform_id: PLATFORM_ID.to_string(),
        source: None,
    })
}

/// Locate `projects/<sanitized-cwd>/chats/<session-id>.jsonl`. The file name
/// is the session id, so a name match is the only lookup needed.
fn find_qwen_session_file_in(root: &Path, session_id: &str) -> Result<PathBuf, String> {
    if !root.exists() {
        return Err("Qwen session directory not found: ~/.qwen/projects".to_string());
    }

    let project_entries = fs::read_dir(root).map_err(|err| err.to_string())?;
    for entry in project_entries.flatten() {
        let direct = entry.path().join("chats").join(format!("{}.jsonl", session_id));
        if direct.is_file() {
            return Ok(direct);
        }
    }

    Err(format!("Qwen session not found for id: {}", session_id))
}

fn delete_qwen_session_in_dir(root: &Path, session_id: &str) -> Result<(), String> {
    let file = find_qwen_session_file_in(root, session_id)?;
    fs::remove_file(&file)
        .map_err(|err| format!("Failed to delete Qwen session {}: {}", file.display(), err))
}

fn read_qwen_messages_from_jsonl(
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
        let Some(message) = parse_qwen_chat_record(&value) else {
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

/// A ChatRecord maps to a chat message only for `type == "user"` or
/// `"assistant"`; `tool_result` and `system` records are skipped. The text
/// comes from the GenAI Content object `message.parts[].text`; timestamps are
/// the ISO 8601 `timestamp` field.
fn parse_qwen_chat_record(value: &Value) -> Option<SessionMessage> {
    let kind = value.get("type").and_then(|v| v.as_str())?;
    let role = match kind {
        "user" => "user",
        "assistant" => "assistant",
        _ => return None,
    };
    let (content, thinking) = extract_message_parts(value.get("message")?)?;
    if content.is_empty() && thinking.is_none() {
        return None;
    }
    Some(
        SessionMessage::new(
            role,
            content,
            value
                .get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(parse_rfc3339_to_ms)
                .unwrap_or(0),
        )
        .with_thinking(thinking),
    )
}

fn extract_message_text(message: &Value) -> Option<String> {
    extract_message_parts(message).and_then(|(content, thinking)| {
        if !content.is_empty() {
            Some(content)
        } else {
            thinking
        }
    })
}

/// Split Gemini-style `message.parts` into the visible reply and thought
/// parts. A part is thinking when `thought` is true, `type` is thought /
/// thinking, or the payload lives in a string `thought` field.
fn extract_message_parts(message: &Value) -> Option<(String, Option<String>)> {
    let parts = message.get("parts")?.as_array()?;
    let mut texts = Vec::new();
    let mut thoughts = Vec::new();
    for part in parts {
        let kind = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let is_thought = part.get("thought").and_then(|v| v.as_bool()) == Some(true)
            || kind.eq_ignore_ascii_case("thought")
            || kind.eq_ignore_ascii_case("thinking");
        let text = part
            .get("text")
            .and_then(|v| v.as_str())
            .or_else(|| part.get("thought").and_then(|v| v.as_str()))
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let Some(text) = text else {
            continue;
        };
        if is_thought {
            thoughts.push(text.to_string());
        } else {
            texts.push(text.to_string());
        }
    }
    if texts.is_empty() && thoughts.is_empty() {
        None
    } else {
        let thinking = if thoughts.is_empty() {
            None
        } else {
            Some(thoughts.join("\n"))
        };
        Some((texts.join("\n"), thinking))
    }
}

/// Qwen Code derives the project directory name from the working directory:
/// every non-alphanumeric character becomes `-` (e.g. `/Users/foo/bar` →
/// `-Users-foo-bar`). On Windows the path is lowercased first.
#[allow(dead_code)]
pub(crate) fn sanitize_cwd(cwd: &str) -> String {
    #[cfg(target_os = "windows")]
    let cwd = cwd.to_lowercase();
    #[cfg(not(target_os = "windows"))]
    let cwd = cwd.to_string();
    cwd.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn truncate_chars(value: String, max_chars: usize) -> String {
    let mut result = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            break;
        }
        result.push(ch);
    }
    if value.chars().count() > max_chars {
        format!("{}...", result)
    } else {
        result
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

fn qwen_projects_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    Ok(join_relative(home, ".qwen/projects"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn user_record(text: &str, timestamp: &str) -> Value {
        json!({
            "uuid": "u-1",
            "sessionId": "sess-1",
            "timestamp": timestamp,
            "type": "user",
            "cwd": "/tmp/work",
            "gitBranch": "main",
            "message": {"role": "user", "parts": [{"text": text}]},
            "model": "qwen3-coder-plus"
        })
    }

    fn assistant_record(text: &str, timestamp: &str) -> Value {
        json!({
            "uuid": "a-1",
            "sessionId": "sess-1",
            "timestamp": timestamp,
            "type": "assistant",
            "message": {"role": "assistant", "parts": [{"text": text}]},
            "usageMetadata": {"totalTokenCount": 42}
        })
    }

    fn write_session(root: &Path, project_dir: &str, session_id: &str) -> PathBuf {
        let chats_dir = root.join(project_dir).join("chats");
        fs::create_dir_all(&chats_dir).expect("chats dir should create");
        let path = chats_dir.join(format!("{}.jsonl", session_id));
        let mut file = fs::File::create(&path).expect("chat file should create");
        writeln!(
            file,
            "{}",
            user_record("你好", "2026-07-17T15:03:19.201Z")
        )
        .expect("line should write");
        writeln!(
            file,
            "{}",
            assistant_record("你好！", "2026-07-17T15:04:20.663Z")
        )
        .expect("line should write");
        path
    }

    #[test]
    fn sanitize_cwd_replaces_non_alphanumeric_with_dash() {
        assert_eq!(sanitize_cwd("/Users/foo/bar"), "-Users-foo-bar");
        assert_eq!(sanitize_cwd("a.b_c d"), "a-b-c-d");
        assert_eq!(sanitize_cwd("already-clean"), "already-clean");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn sanitize_cwd_lowercases_on_windows() {
        assert_eq!(sanitize_cwd(r"C:\Users\Foo"), r"c--users-foo");
    }

    #[test]
    fn read_qwen_session_maps_required_fields() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        let path = write_session(temp.path(), "-tmp-work", "sess-1");

        let summary = read_qwen_session(&path).expect("session should parse");
        assert_eq!(summary.id, "sess-1");
        assert_eq!(summary.title, "你好");
        assert_eq!(summary.project_path, "/tmp/work");
        assert_eq!(summary.model.as_deref(), Some("qwen3-coder-plus"));
        assert_eq!(summary.platform_id, "qwen");
        assert!(summary.started_at > 0);
        assert!(summary.updated_at > summary.started_at);
    }

    #[test]
    fn read_qwen_session_falls_back_to_id_as_title() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        let chats_dir = temp.path().join("-tmp-work/chats");
        fs::create_dir_all(&chats_dir).expect("chats dir should create");
        let path = chats_dir.join("sess-2.jsonl");
        fs::write(
            &path,
            json!({"type": "system", "timestamp": "2026-07-17T15:03:19.201Z"}).to_string(),
        )
        .expect("chat file should write");

        let summary = read_qwen_session(&path).expect("session should parse");
        assert_eq!(summary.title, "sess-2");
        assert_eq!(summary.project_path, "");
    }

    #[test]
    fn list_qwen_chat_paths_skips_archive_subdirectory() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        write_session(temp.path(), "-tmp-work", "sess-1");
        let archive_dir = temp.path().join("-tmp-work/chats/archive");
        fs::create_dir_all(&archive_dir).expect("archive dir should create");
        fs::write(archive_dir.join("sess-archived.jsonl"), "{}").expect("archive should write");

        let paths = list_qwen_chat_paths(temp.path());
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("chats/sess-1.jsonl"));
    }

    #[test]
    fn parse_qwen_chat_record_maps_user_and_assistant() {
        let user = parse_qwen_chat_record(&user_record("你好", "2026-07-17T15:03:19.201Z"))
            .expect("user record should parse");
        assert_eq!(user.role, "user");
        assert_eq!(user.content, "你好");
        assert!(user.timestamp > 0);

        let assistant =
            parse_qwen_chat_record(&assistant_record("你好！", "2026-07-17T15:04:20.663Z"))
                .expect("assistant record should parse");
        assert_eq!(assistant.role, "assistant");
        assert_eq!(assistant.content, "你好！");
    }

    #[test]
    fn parse_qwen_chat_record_splits_thought_parts() {
        let record = json!({
            "type": "assistant",
            "timestamp": "2026-07-17T15:04:20.663Z",
            "message": {
                "role": "assistant",
                "parts": [
                    {"text": "The user said hello.", "thought": true},
                    {"text": "你好！"}
                ]
            }
        });
        let message = parse_qwen_chat_record(&record).expect("assistant should parse");
        assert_eq!(message.content, "你好！");
        assert_eq!(message.thinking.as_deref(), Some("The user said hello."));
    }

    #[test]
    fn parse_qwen_chat_record_skips_tool_result_and_system() {
        let tool_result = json!({
            "type": "tool_result",
            "timestamp": "2026-07-17T15:03:20.000Z",
            "message": {"role": "tool", "parts": [{"text": "output"}]}
        });
        let system = json!({
            "type": "system",
            "timestamp": "2026-07-17T15:03:21.000Z",
            "message": {"role": "system", "parts": [{"text": "note"}]}
        });

        assert!(parse_qwen_chat_record(&tool_result).is_none());
        assert!(parse_qwen_chat_record(&system).is_none());
    }

    #[test]
    fn read_qwen_messages_respects_offset_limit() {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "agent-hub-qwen-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        );
        path.push(unique);

        let mut file = fs::File::create(&path).expect("temp file should create");
        writeln!(file, "{}", user_record("u1", "2026-07-17T15:03:19.201Z"))
            .expect("line should write");
        writeln!(
            file,
            "{}",
            assistant_record("a1", "2026-07-17T15:04:20.663Z")
        )
        .expect("line should write");
        writeln!(file, "{}", user_record("u2", "2026-07-17T15:05:21.000Z"))
            .expect("line should write");
        file.flush().expect("flush should succeed");

        let page = read_qwen_messages_from_jsonl(&path, 1, 2).expect("page should load");
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].content, "a1");
        assert_eq!(page[1].content, "u2");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn delete_qwen_session_removes_session_file() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        let path = write_session(temp.path(), "-tmp-work", "sess-1");
        assert!(path.exists());

        delete_qwen_session_in_dir(temp.path(), "sess-1").expect("delete should succeed");

        assert!(!path.exists());
    }

    #[test]
    fn find_qwen_session_file_matches_across_project_dirs() {
        let temp = tempfile::tempdir().expect("temp dir should create");
        write_session(temp.path(), "-tmp-one", "sess-1");
        write_session(temp.path(), "-tmp-two", "sess-2");

        let found =
            find_qwen_session_file_in(temp.path(), "sess-2").expect("file should be found");
        assert!(found.ends_with("-tmp-two/chats/sess-2.jsonl"));
    }
}

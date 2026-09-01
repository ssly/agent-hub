use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::paths::join_relative;
use crate::session::models::{take_pending_thinking, SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "omp";

pub fn count_omp_sessions() -> Result<usize, String> {
    Ok(list_omp_session_files()?.len())
}

pub fn list_omp_sessions_all() -> Result<Vec<SessionSummary>, String> {
    let mut sessions = Vec::new();
    for path in list_omp_session_files()? {
        if let Some(summary) = read_session_summary(&path) {
            sessions.push(summary);
        }
    }
    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(sessions)
}

#[cfg(test)]
fn get_omp_messages_in(
    sessions_root: PathBuf,
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let path = find_session_file_in(sessions_root, session_id)?;
    let transcript = parse_session_file(&path)?;
    Ok(page_messages(&transcript.messages, offset, limit))
}

pub fn get_omp_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let path = find_session_file_in(omp_sessions_dir()?, session_id)?;
    let transcript = parse_session_file(&path)?;
    Ok(page_messages(&transcript.messages, offset, limit))
}

#[cfg(test)]
fn delete_omp_session_in(sessions_root: PathBuf, session_id: &str) -> Result<(), String> {
    let path = find_session_file_in(sessions_root, session_id)?;
    fs::remove_file(&path)
        .map_err(|err| format!("Failed to delete Oh My Pi session {}: {}", session_id, err))
}

pub fn delete_omp_session(session_id: &str) -> Result<(), String> {
    let path = find_session_file_in(omp_sessions_dir()?, session_id)?;
    fs::remove_file(&path)
        .map_err(|err| format!("Failed to delete Oh My Pi session {}: {}", session_id, err))
}

/// Streaming-free resume preview: reuse the parsed active branch and keep the
/// last user/assistant messages.
pub fn last_omp_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let path = find_session_file_in(omp_sessions_dir()?, session_id)?;
    let transcript = parse_session_file(&path)?;
    let last_user = transcript
        .messages
        .iter()
        .rev()
        .find(|msg| msg.role == "user")
        .cloned();
    let last_assistant = transcript
        .messages
        .iter()
        .rev()
        .find(|msg| msg.role == "assistant")
        .cloned();
    Ok((last_user, last_assistant))
}

pub fn search_omp_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let mut results = Vec::new();
    for path in list_omp_session_files()? {
        let Some(summary) = read_session_summary(&path) else {
            continue;
        };
        let transcript = parse_session_file(&path);
        let messages = match transcript {
            Ok(transcript) => transcript.messages,
            Err(_) => continue,
        };
        for msg in messages {
            if msg.matches_query(query_lower) {
                results.push(crate::session::SessionSearchResult {
                    session_id: summary.id.clone(),
                    session_title: summary.title.clone(),
                    project_path: summary.project_path.clone(),
                    platform_id: PLATFORM_ID.to_string(),
                    message: msg,
                });
            }
        }
    }
    Ok(results)
}

/// Enumerate `~/.omp/agent/sessions/<encoded-cwd>/<ts>_<sessionId>.jsonl`.
/// Legacy hashed buckets (17.2.5–17.2.8) use the same two-level shape, so
/// both layouts are covered by the same scan.
fn list_omp_session_files() -> Result<Vec<PathBuf>, String> {
    list_omp_session_files_in(&omp_sessions_dir()?)
}

fn list_omp_session_files_in(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    let project_entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) => return Err(err.to_string()),
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
            let path = session_entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn find_session_file_in(sessions_root: PathBuf, session_id: &str) -> Result<PathBuf, String> {
    for path in list_omp_session_files_in(&sessions_root)? {
        // Fast path: files are named `<timestamp>_<sessionId>.jsonl`.
        if path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.ends_with(session_id))
            .unwrap_or(false)
        {
            return Ok(path);
        }
    }
    // Fallback: header `id` may differ from the file name.
    for path in list_omp_session_files_in(&sessions_root)? {
        if let Some(header) = read_session_header(&path) {
            if header.get("id").and_then(|v| v.as_str()) == Some(session_id) {
                return Ok(path);
            }
        }
    }
    Err(format!("Oh My Pi session not found for id: {}", session_id))
}

fn read_session_summary(path: &Path) -> Option<SessionSummary> {
    let fallback_updated_at = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(system_time_to_ms)
        .unwrap_or(0);
    let header = read_session_header(path)?;
    let transcript = parse_session_file(path).ok()?;
    let id = header
        .get("id")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.rsplit('_').next().unwrap_or(stem).to_string())
        })?;
    if id.is_empty() {
        return None;
    }
    let title = transcript
        .title
        .clone()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let project_path = header
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .unwrap_or_default();
    Some(SessionSummary {
        id,
        title,
        project_path,
        model: transcript.model,
        started_at: transcript.started_at,
        updated_at: fallback_updated_at,
        message_count: Some(transcript.messages.len() as u32),
        tokens_used: transcript.tokens_used,
        platform_id: PLATFORM_ID.to_string(),
        source: Some("terminal".to_string()),
    })
}

/// The first line of a current file is a fixed 256-byte `type: "title"` slot
/// (padded, so trailing whitespace is tolerated), followed by the
/// `type: "session"` header. Older files may start with the header directly.
fn read_session_header(path: &Path) -> Option<Value> {
    let mut file = fs::File::open(path).ok()?;
    let mut prefix = [0u8; 1024];
    let read = std::io::Read::read(&mut file, &mut prefix).ok()?;
    let text = String::from_utf8_lossy(&prefix[..read]);
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        match value.get("type").and_then(|v| v.as_str()) {
            Some("title") => continue,
            Some("session") => return Some(value),
            _ => continue,
        }
    }
    None
}

struct SessionTranscript {
    title: Option<String>,
    started_at: i64,
    model: Option<String>,
    tokens_used: Option<u64>,
    messages: Vec<SessionMessage>,
}

/// Parse one session file into its **active branch** transcript. Entries form
/// an append-only tree; the leaf is derived (last entry in file), and the
/// active branch walks `parentId` up to root, then reverses.
fn parse_session_file(path: &Path) -> Result<SessionTranscript, String> {
    let file = fs::File::open(path).map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(file);

    let mut title: Option<String> = None;
    let mut started_at: i64 = 0;
    let mut model: Option<String> = None;
    let mut tokens_used_total: u64 = 0;
    let mut saw_usage = false;

    // entry id -> (parent id, index into entries)
    let mut parents: HashMap<String, Option<String>> = HashMap::new();
    let mut entry_ids: Vec<String> = Vec::new();
    let mut messages_by_index: HashMap<usize, SessionMessage> = HashMap::new();
    let mut pending_thinking = String::new();

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => continue,
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        match value.get("type").and_then(|v| v.as_str()) {
            Some("title") => {
                // The fixed-width slot is the live title source.
                if let Some(slot_title) = value
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                {
                    title = Some(slot_title);
                }
            }
            Some("session") => {
                started_at = value
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(parse_rfc3339_to_ms)
                    .unwrap_or(0);
                if title.is_none() {
                    title = value
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty());
                }
            }
            Some("title_change") => {
                // Append-only rename audit; the latest rename wins unless the
                // title slot already carries a user title.
                if let Some(rename) = value
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                {
                    title = Some(rename);
                }
            }
            Some("message") => {
                let entry_id = value
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string());
                let parent_id = value
                    .get("parentId")
                    .map(|v| v.as_str().map(|s| s.to_string()));
                let parent = match parent_id {
                    Some(parent) => parent,
                    None => None,
                };
                if let Some(entry_id) = &entry_id {
                    parents.insert(entry_id.clone(), parent.clone());
                    entry_ids.push(entry_id.clone());
                }
                let index = entry_ids.len().saturating_sub(1);
                let message_value = value.get("message");
                let Some(message_value) = message_value else {
                    continue;
                };
                let role = message_value
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let timestamp = message_value
                    .get("timestamp")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(started_at);
                match role {
                    "user" => {
                        let content = extract_text_content(&message_value["content"]);
                        if content.trim().is_empty() {
                            continue;
                        }
                        pending_thinking.clear();
                        messages_by_index
                            .insert(index, SessionMessage::new("user", content, timestamp));
                    }
                    "assistant" => {
                        if model.is_none() {
                            model = message_value
                                .get("model")
                                .and_then(|v| v.as_str())
                                .map(|v| v.trim().to_string())
                                .filter(|v| !v.is_empty());
                        }
                        let (content, thinking) = extract_assistant_content(message_value);
                        if let Some(usage_total) = message_value
                            .get("usage")
                            .and_then(|usage| usage.get("totalTokens"))
                            .and_then(|v| v.as_u64())
                        {
                            tokens_used_total += usage_total;
                            saw_usage = true;
                        }
                        if content.trim().is_empty() && thinking.is_none() {
                            continue;
                        }
                        let mut message = SessionMessage::new("assistant", content, timestamp);
                        message.thinking =
                            thinking.or_else(|| take_pending_thinking(&mut pending_thinking));
                        messages_by_index.insert(index, message);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // Derive the leaf: the last entry in the file. Walk parents to root.
    let mut branch_indices: Vec<usize> = Vec::new();
    if let Some(leaf) = entry_ids.last() {
        let mut current: Option<&String> = Some(leaf);
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        while let Some(id) = current {
            if !visited.insert(id.clone()) {
                break; // corrupt cycle guard
            }
            if let Some(index) = entry_ids.iter().position(|entry| entry == id) {
                branch_indices.push(index);
            }
            current = parents.get(id).and_then(|parent| parent.as_ref());
        }
        branch_indices.reverse();
    }

    let mut messages = Vec::with_capacity(branch_indices.len());
    for index in branch_indices {
        if let Some(message) = messages_by_index.remove(&index) {
            messages.push(message);
        }
    }

    Ok(SessionTranscript {
        title,
        started_at,
        model,
        tokens_used: if saw_usage {
            Some(tokens_used_total)
        } else {
            None
        },
        messages,
    })
}

fn page_messages(messages: &[SessionMessage], offset: usize, limit: usize) -> Vec<SessionMessage> {
    let page_limit = limit.max(1);
    messages
        .iter()
        .skip(offset)
        .take(page_limit)
        .cloned()
        .collect()
}

/// User content: a plain string or an array of `text`/`image` blocks. Only
/// text parts are shown; images (base64) are skipped.
fn extract_text_content(content: &Value) -> String {
    match content {
        Value::String(text) => text.trim().to_string(),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if item.get("type").and_then(|v| v.as_str()) != Some("text") {
                    continue;
                }
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

/// Assistant content: text blocks become the body, thinking blocks are
/// returned separately, toolCall blocks are skipped (tool traffic, not prose).
fn extract_assistant_content(message: &Value) -> (String, Option<String>) {
    let mut parts = Vec::new();
    let mut thinking_parts = Vec::new();
    if let Some(Value::Array(items)) = message.get("content") {
        for item in items {
            match item.get("type").and_then(|v| v.as_str()) {
                Some("text") => {
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            parts.push(trimmed.to_string());
                        }
                    }
                }
                Some("thinking") => {
                    if let Some(text) = item.get("thinking").and_then(|v| v.as_str()) {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            thinking_parts.push(trimmed.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let thinking = if thinking_parts.is_empty() {
        None
    } else {
        Some(thinking_parts.join("\n"))
    };
    (parts.join("\n"), thinking)
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

/// `~/.omp/agent` (sessions live one level below). `PI_CODING_AGENT_DIR`
/// overrides the agent directory, mirroring omp itself.
fn omp_agent_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
        if !dir.trim().is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    Ok(join_relative(home, ".omp/agent"))
}

fn omp_sessions_dir() -> Result<PathBuf, String> {
    Ok(omp_agent_dir()?.join("sessions"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    /// Write a realistic session file: 256-byte title slot + header + a
    /// branched tree of message entries. Returns the file path.
    fn write_session(
        root: &Path,
        project_dir: &str,
        file_name: &str,
        entries: &[Value],
    ) -> PathBuf {
        let dir = root.join(project_dir);
        fs::create_dir_all(&dir).expect("project dir should create");
        let path = dir.join(file_name);
        let mut file = fs::File::create(&path).expect("session file should create");

        let title_slot = json!({"type": "title", "title": "Oh My Pi 调研", "titleSource": "auto"});
        let mut slot_line = title_slot.to_string();
        // Pad the slot to 256 bytes like omp does, spaces survive JSON parse.
        while slot_line.len() < 255 {
            slot_line.push(' ');
        }
        writeln!(file, "{}", slot_line).expect("title slot");

        writeln!(
            file,
            "{}",
            json!({
                "type": "session",
                "version": 3,
                "id": file_name.trim_end_matches(".jsonl").rsplit('_').next().unwrap(),
                "timestamp": "2026-08-28T08:00:00.000Z",
                "cwd": "/tmp/omp-work",
                "title": "Oh My Pi 调研",
                "titleSource": "auto"
            })
        )
        .expect("header");

        for entry in entries {
            writeln!(file, "{}", entry).expect("entry");
        }
        path
    }

    fn message_entry(id: &str, parent: Option<&str>, message: Value) -> Value {
        json!({
            "type": "message",
            "id": id,
            "parentId": parent,
            "timestamp": "2026-08-28T08:01:00.000Z",
            "message": message
        })
    }

    #[test]
    fn parse_active_branch_skips_abandoned_side_branch() {
        let temp = tempfile::tempdir().expect("temp dir");
        let entries = vec![
            message_entry(
                "aaaaaaaa",
                None,
                json!({"role": "user", "content": "第一条提问", "timestamp": 1}),
            ),
            message_entry(
                "bbbbbbbb",
                Some("aaaaaaaa"),
                json!({
                    "role": "assistant",
                    "content": [
                        {"type": "thinking", "thinking": "先想想"},
                        {"type": "text", "text": "第一条回答"}
                    ],
                    "model": "glm-5",
                    "usage": {"totalTokens": 100},
                    "timestamp": 2
                }),
            ),
            // Abandoned side branch (user retried): must not appear.
            message_entry(
                "cccccccc",
                Some("aaaaaaaa"),
                json!({"role": "user", "content": "换个问法", "timestamp": 3}),
            ),
            message_entry(
                "dddddddd",
                Some("bbbbbbbb"),
                json!({"role": "user", "content": "继续深入", "timestamp": 4}),
            ),
            message_entry(
                "eeeeeeee",
                Some("dddddddd"),
                json!({
                    "role": "assistant",
                    "content": [{"type": "text", "text": "深入的回答"}],
                    "model": "glm-5",
                    "usage": {"totalTokens": 40},
                    "timestamp": 5
                }),
            ),
        ];
        let path = write_session(
            temp.path(),
            "-tmp-omp-work",
            "20260828_omp-1.jsonl",
            &entries,
        );

        let transcript = parse_session_file(&path).expect("transcript should parse");
        let texts: Vec<&str> = transcript
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect();
        assert_eq!(
            texts,
            vec!["第一条提问", "第一条回答", "继续深入", "深入的回答"]
        );
        assert_eq!(transcript.model.as_deref(), Some("glm-5"));
        assert_eq!(transcript.tokens_used, Some(140));
        // 2026-08-28T08:00:00.000Z
        assert_eq!(transcript.started_at, 1787904000000);
        // thinking attaches to its assistant message
        assert_eq!(transcript.messages[1].thinking.as_deref(), Some("先想想"));
    }

    #[test]
    fn summary_prefers_title_slot_and_counts_active_branch() {
        let temp = tempfile::tempdir().expect("temp dir");
        let entries = vec![
            message_entry(
                "aaaaaaaa",
                None,
                json!({"role": "user", "content": "你好", "timestamp": 1}),
            ),
            message_entry(
                "bbbbbbbb",
                Some("aaaaaaaa"),
                json!({
                    "role": "assistant",
                    "content": [{"type": "text", "text": "嗨"}],
                    "usage": {"totalTokens": 10},
                    "timestamp": 2
                }),
            ),
            message_entry(
                "cccccccc",
                Some("aaaaaaaa"),
                json!({"role": "user", "content": "弃线", "timestamp": 3}),
            ),
        ];
        let path = write_session(
            temp.path(),
            "-tmp-omp-work",
            "20260828_omp-2.jsonl",
            &entries,
        );
        fs::write(&path, fs::read_to_string(&path).unwrap()).unwrap();

        let summary = read_session_summary(&path).expect("summary");
        assert_eq!(summary.id, "omp-2");
        assert_eq!(summary.title, "Oh My Pi 调研");
        assert_eq!(summary.project_path, "/tmp/omp-work");
        assert_eq!(summary.message_count, Some(2));
        assert_eq!(summary.platform_id, "omp");
        assert_eq!(summary.source.as_deref(), Some("terminal"));
    }

    #[test]
    fn get_messages_respects_offset_limit() {
        let temp = tempfile::tempdir().expect("temp dir");
        let entries = vec![
            message_entry(
                "aaaaaaaa",
                None,
                json!({"role": "user", "content": "u1", "timestamp": 1}),
            ),
            message_entry(
                "bbbbbbbb",
                Some("aaaaaaaa"),
                json!({"role": "assistant", "content": [{"type": "text", "text": "a1"}], "timestamp": 2}),
            ),
            message_entry(
                "cccccccc",
                Some("bbbbbbbb"),
                json!({"role": "user", "content": "u2", "timestamp": 3}),
            ),
        ];
        write_session(
            temp.path(),
            "-tmp-omp-work",
            "20260828_omp-3.jsonl",
            &entries,
        );

        let page = get_omp_messages_in(temp.path().to_path_buf(), "omp-3", 1, 2).expect("page");
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].content, "a1");
        assert_eq!(page[1].content, "u2");
    }

    #[test]
    fn user_content_string_and_array_both_parse() {
        assert_eq!(extract_text_content(&json!("plain")), "plain");
        assert_eq!(
            extract_text_content(&json!([
                {"type": "text", "text": "hello "},
                {"type": "image", "data": "xx", "mimeType": "image/png"},
                {"type": "text", "text": "world"}
            ])),
            "hello\nworld"
        );
    }

    #[test]
    fn assistant_tool_calls_are_skipped_but_text_kept() {
        let (content, thinking) = extract_assistant_content(&json!({
            "content": [
                {"type": "thinking", "thinking": "why"},
                {"type": "toolCall", "id": "c1", "name": "bash", "arguments": {}},
                {"type": "text", "text": "done"}
            ]
        }));
        assert_eq!(content, "done");
        assert_eq!(thinking.as_deref(), Some("why"));
    }

    #[test]
    fn delete_removes_only_target_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let entries = vec![];
        let kept = write_session(temp.path(), "-tmp-a", "20260828_omp-keep.jsonl", &entries);
        let gone = write_session(temp.path(), "-tmp-b", "20260828_omp-gone.jsonl", &entries);

        delete_omp_session_in(temp.path().to_path_buf(), "omp-gone")
            .expect("delete should succeed");
        assert!(!gone.exists());
        assert!(kept.exists());
    }

    #[test]
    fn find_session_file_falls_back_to_header_id() {
        let temp = tempfile::tempdir().expect("temp dir");
        // File name disagrees with the header id (renamed/migrated bucket).
        let path = write_session(temp.path(), "-tmp-x", "20260828_weird-name.jsonl", &[]);
        let content = fs::read_to_string(&path)
            .unwrap()
            .replace("\"id\":\"weird-name\"", "\"id\":\"real-uuid-123\"");
        fs::write(&path, content).unwrap();

        let found = find_session_file_in(temp.path().to_path_buf(), "real-uuid-123")
            .expect("should find by header id");
        assert_eq!(found, path);
    }
}

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::paths::join_relative;
use crate::session::models::{
    push_pending_thinking, take_pending_thinking, SessionMessage, SessionSummary,
};

const PLATFORM_ID: &str = "workbuddy";

pub fn count_workbuddy_sessions() -> Result<usize, String> {
    let sessions = collect_workbuddy_sessions()?;
    Ok(sessions.len())
}

pub fn list_workbuddy_sessions_all() -> Result<Vec<SessionSummary>, String> {
    collect_workbuddy_sessions()
}

pub fn get_workbuddy_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let path = find_workbuddy_session_file(session_id)?;
    read_workbuddy_messages_from_jsonl(&path, offset, limit)
}

pub fn delete_workbuddy_session(session_id: &str) -> Result<(), String> {
    let path = find_workbuddy_session_file(session_id)?;
    if path.is_file() {
        fs::remove_file(&path).map_err(|err| err.to_string())?;
        // If parent directory is named after the session or is empty, clean it up.
        if let Some(parent) = path.parent() {
            if parent.file_name().and_then(|n| n.to_str()) == Some(session_id) {
                let _ = fs::remove_dir_all(parent);
            }
        }
        Ok(())
    } else if path.is_dir() {
        fs::remove_dir_all(&path).map_err(|err| err.to_string())
    } else {
        Err(format!("Session file not found: {}", session_id))
    }
}

/// Streaming scan that keeps only the latest user/assistant message, for the
/// resume preview. Never collects the full transcript into memory.
pub fn last_workbuddy_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let path = find_workbuddy_session_file(session_id)?;
    let file = fs::File::open(path).map_err(|err| err.to_string())?;
    let reader = BufReader::new(file);
    let mut last_user = None;
    let mut last_assistant = None;

    let mut pending_thinking = String::new();

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if let Some(msg) = parse_workbuddy_message_line(&value, &mut pending_thinking) {
            if msg.role == "user" {
                last_user = Some(msg);
            } else {
                last_assistant = Some(msg);
            }
        }
    }

    Ok((last_user, last_assistant))
}

pub fn search_workbuddy_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let sessions = collect_workbuddy_sessions()?;
    let mut results = Vec::new();
    for session in sessions {
        let Ok(path) = find_workbuddy_session_file(&session.id) else {
            continue;
        };
        if let Ok(messages) = read_workbuddy_messages_from_jsonl(&path, 0, 999999) {
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

fn workbuddy_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(join_relative(home.clone(), ".workbuddy/sessions"));
        roots.push(join_relative(home.clone(), ".workbuddy/projects"));
        roots.push(join_relative(home.clone(), ".codebuddy/sessions"));
        roots.push(join_relative(home.clone(), ".codebuddy/projects"));
    }
    roots
}

fn find_workbuddy_session_file(session_id: &str) -> Result<PathBuf, String> {
    for root in workbuddy_roots() {
        if !root.exists() {
            continue;
        }

        // Direct file: <root>/<sessionId>.jsonl
        let direct = root.join(format!("{}.jsonl", session_id));
        if direct.is_file() {
            return Ok(direct);
        }

        // Subdirectory: <root>/<sessionId>/transcript.jsonl or <root>/<sessionId>/session.jsonl
        let sub_transcript = root.join(session_id).join("transcript.jsonl");
        if sub_transcript.is_file() {
            return Ok(sub_transcript);
        }
        let sub_session = root.join(session_id).join("session.jsonl");
        if sub_session.is_file() {
            return Ok(sub_session);
        }

        // Scanned project directories: <root>/<project>/<sessionId>.jsonl
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let in_project = path.join(format!("{}.jsonl", session_id));
                    if in_project.is_file() {
                        return Ok(in_project);
                    }
                    let in_project_sub = path.join(session_id).join("session.jsonl");
                    if in_project_sub.is_file() {
                        return Ok(in_project_sub);
                    }
                }
            }
        }
    }

    Err(format!("WorkBuddy session not found: {}", session_id))
}

fn collect_workbuddy_sessions() -> Result<Vec<SessionSummary>, String> {
    let mut sessions = Vec::new();

    for root in workbuddy_roots() {
        if !root.exists() {
            continue;
        }
        scan_sessions_dir(&root, &mut sessions);
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions.dedup_by(|a, b| a.id == b.id);
    Ok(sessions)
}

fn scan_sessions_dir(dir: &Path, sessions: &mut Vec<SessionSummary>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            if let Some(summary) = parse_session_file(&path) {
                sessions.push(summary);
            }
        } else if path.is_dir() {
            let session_jsonl = path.join("session.jsonl");
            let transcript_jsonl = path.join("transcript.jsonl");
            if session_jsonl.is_file() {
                if let Some(summary) = parse_session_file(&session_jsonl) {
                    sessions.push(summary);
                }
            } else if transcript_jsonl.is_file() {
                if let Some(summary) = parse_session_file(&transcript_jsonl) {
                    sessions.push(summary);
                }
            } else {
                // Nested one level (e.g. project subdirectories)
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.is_file()
                            && sub_path.extension().and_then(|ext| ext.to_str()) == Some("jsonl")
                        {
                            if let Some(summary) = parse_session_file(&sub_path) {
                                sessions.push(summary);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn parse_session_file(path: &Path) -> Option<SessionSummary> {
    let file = fs::File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let file_mtime = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let default_id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("unknown")
        .to_string();

    let reader = BufReader::new(file);
    let mut id = default_id;
    let mut title = None;
    let mut project_path = String::new();
    let mut model = None;
    let mut started_at = file_mtime;
    let mut updated_at = file_mtime;
    let mut message_count = 0u32;
    let mut tokens_used = 0u64;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let val: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Header line if present
        if val.get("type").and_then(|v| v.as_str()) == Some("session") {
            if let Some(sid) = val.get("id").and_then(|v| v.as_str()) {
                id = sid.to_string();
            }
            if let Some(cwd) = val.get("cwd").and_then(|v| v.as_str()) {
                project_path = cwd.to_string();
            }
            if let Some(m) = val.get("model").and_then(|v| v.as_str()) {
                model = Some(m.to_string());
            }
            if let Some(ts) = val.get("createdAt").and_then(|v| v.as_i64()) {
                started_at = ts;
            }
            continue;
        }

        // Check for session ID in line
        if let Some(sid) = val.get("sessionId").and_then(|v| v.as_str()) {
            if id == "transcript" || id == "session" {
                id = sid.to_string();
            }
        }
        if project_path.is_empty() {
            if let Some(cwd) = val.get("cwd").and_then(|v| v.as_str()) {
                project_path = cwd.to_string();
            }
        }
        if model.is_none() {
            if let Some(m) = val.get("model").and_then(|v| v.as_str()) {
                model = Some(m.to_string());
            }
        }

        // Count messages & tokens
        let role = val
            .get("role")
            .or_else(|| val.get("type"))
            .and_then(|v| v.as_str());

        if let Some(role_str) = role {
            if role_str == "user" || role_str == "assistant" {
                message_count += 1;
                if role_str == "user" && title.is_none() {
                    if let Some(content) = extract_content(&val) {
                        let (body, _) = crate::session::models::split_injected_context(&content);
                        let single_line = body.lines().next().unwrap_or("").trim();
                        if !single_line.is_empty() {
                            title = Some(single_line.to_string());
                        }
                    }
                }
            }
        }

        if let Some(usage) = val.get("usage").or_else(|| val.get("usageMetadata")) {
            if let Some(total) = usage
                .get("total_tokens")
                .or_else(|| usage.get("totalTokens"))
                .and_then(|v| v.as_u64())
            {
                tokens_used += total;
            }
        }

        if let Some(ts) = val
            .get("timestamp")
            .or_else(|| val.get("time"))
            .and_then(|v| v.as_i64())
        {
            if ts > updated_at {
                updated_at = ts;
            }
            if started_at == 0 || ts < started_at {
                started_at = ts;
            }
        }
    }

    Some(SessionSummary {
        id,
        title: title.unwrap_or_else(|| "WorkBuddy Session".to_string()),
        project_path,
        model,
        started_at,
        updated_at,
        message_count: if message_count > 0 {
            Some(message_count)
        } else {
            None
        },
        tokens_used: if tokens_used > 0 {
            Some(tokens_used)
        } else {
            None
        },
        platform_id: PLATFORM_ID.to_string(),
        source: Some("terminal".to_string()),
    })
}

fn read_workbuddy_messages_from_jsonl(
    path: &Path,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let file = fs::File::open(path).map_err(|err| err.to_string())?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut pending_thinking = String::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let val: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(msg) = parse_workbuddy_message_line(&val, &mut pending_thinking) {
            messages.push(msg);
        }
    }

    let total = messages.len();
    if offset >= total {
        return Ok(Vec::new());
    }
    let end = (offset + limit).min(total);
    Ok(messages[offset..end].to_vec())
}

fn parse_workbuddy_message_line(
    val: &Value,
    pending_thinking: &mut String,
) -> Option<SessionMessage> {
    let role = val
        .get("role")
        .or_else(|| val.get("type"))
        .and_then(|v| v.as_str())?;

    let timestamp = val
        .get("timestamp")
        .or_else(|| val.get("time"))
        .or_else(|| val.get("createdAt"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // Extract thinking / reasoning if present
    if let Some(reasoning) = val
        .get("reasoning_content")
        .or_else(|| val.get("thinking"))
        .or_else(|| val.get("thought"))
        .and_then(|v| v.as_str())
    {
        push_pending_thinking(pending_thinking, reasoning);
    }

    if role == "user" {
        let content = extract_content(val)?;
        let msg = SessionMessage::new("user", content, timestamp);
        return Some(msg);
    }

    if role == "assistant" {
        let content = extract_content(val).unwrap_or_default();
        let thinking = take_pending_thinking(pending_thinking);
        let msg = SessionMessage::new("assistant", content, timestamp).with_thinking(thinking);
        return Some(msg);
    }

    None
}

fn extract_content(val: &Value) -> Option<String> {
    if let Some(text) = val.get("content").and_then(|v| v.as_str()) {
        return Some(text.to_string());
    }
    if let Some(text) = val.get("text").and_then(|v| v.as_str()) {
        return Some(text.to_string());
    }
    if let Some(msg) = val.get("message") {
        if let Some(text) = msg.get("content").and_then(|v| v.as_str()) {
            return Some(text.to_string());
        }
        if let Some(parts) = msg.get("content").and_then(|v| v.as_array()) {
            let mut combined = String::new();
            for part in parts {
                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                    combined.push_str(t);
                }
            }
            if !combined.is_empty() {
                return Some(combined);
            }
        }
    }
    if let Some(parts) = val.get("content").and_then(|v| v.as_array()) {
        let mut combined = String::new();
        for part in parts {
            if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                combined.push_str(t);
            }
        }
        if !combined.is_empty() {
            return Some(combined);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn parses_workbuddy_jsonl_messages() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"type":"session","id":"wb-101","cwd":"/workspace/demo","model":"hunyuan-t1","createdAt":1700000000000}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"role":"user","content":"Hello WorkBuddy","timestamp":1700000001000}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"role":"assistant","reasoning_content":"Thinking steps...","content":"Hello! How can I help you today?","timestamp":1700000005000,"usage":{{"total_tokens":150}}}}"#
        )
        .unwrap();

        let summary = parse_session_file(file.path()).expect("should parse summary");
        assert_eq!(summary.id, "wb-101");
        assert_eq!(summary.title, "Hello WorkBuddy");
        assert_eq!(summary.project_path, "/workspace/demo");
        assert_eq!(summary.model.as_deref(), Some("hunyuan-t1"));
        assert_eq!(summary.message_count, Some(2));
        assert_eq!(summary.tokens_used, Some(150));

        let msgs = read_workbuddy_messages_from_jsonl(file.path(), 0, 10).expect("read messages");
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content, "Hello WorkBuddy");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].content, "Hello! How can I help you today?");
        assert_eq!(msgs[1].thinking.as_deref(), Some("Thinking steps..."));
    }

    #[test]
    fn reminder_user_message_uses_user_query_as_title_and_body() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"type":"session","id":"wb-102","cwd":"/Users/liuyang/WorkBuddy/automation","createdAt":1700000000000}}"#
        )
        .unwrap();
        let prompt = r#"<system-reminder data-role="user-context"><user_info>OS Version: darwin</user_info></system-reminder>
<user_query>历史上的今天发生过什么有趣的事？</user_query>"#;
        writeln!(
            file,
            r#"{{"role":"user","content":{},"timestamp":1700000001000}}"#,
            serde_json::to_string(prompt).unwrap()
        )
        .unwrap();

        let summary = parse_session_file(file.path()).expect("should parse summary");
        assert_eq!(summary.title, "历史上的今天发生过什么有趣的事？");

        let msgs = read_workbuddy_messages_from_jsonl(file.path(), 0, 10).expect("read messages");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "历史上的今天发生过什么有趣的事？");
        assert!(msgs[0]
            .system
            .as_deref()
            .unwrap()
            .contains("data-role=\"user-context\""));
    }
}

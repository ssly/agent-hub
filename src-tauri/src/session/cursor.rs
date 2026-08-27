//! Cursor Agent / CLI session adapter.
//!
//! New Cursor agent sessions (CLI `agent` / `cursor-agent`, and the IDE
//! agent window that shares the same protocol) live at:
//! `~/.cursor/chats/<workspace-hash>/<sessionId>/`
//!   - `meta.json` — title, cwd, createdAtMs / updatedAtMs
//!   - `prompt_history.json` — user prompts only
//!   - `store.db` — opaque blob store (not used here)
//!
//! A readable transcript, when present, is the observe copy:
//! `~/.cursor/projects/<project>/agent-transcripts/<id>/<id>.jsonl`
//!
//! Classic Composer chats in VS Code `state.vscdb` are a different store and
//! are not listed here.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "cursor";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorMeta {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    created_at_ms: i64,
    #[serde(default)]
    updated_at_ms: i64,
    #[serde(default)]
    has_conversation: bool,
}

struct CursorSessionFile {
    id: String,
    dir: PathBuf,
    meta: CursorMeta,
}

fn cursor_home() -> PathBuf {
    crate::paths::home_dir().join(".cursor")
}

fn chats_root() -> PathBuf {
    cursor_home().join("chats")
}

fn projects_root() -> PathBuf {
    cursor_home().join("projects")
}

fn ms_to_secs(ms: i64) -> i64 {
    if ms > 1_000_000_000_000 {
        ms / 1000
    } else {
        ms
    }
}

fn list_cursor_session_files() -> Vec<CursorSessionFile> {
    let root = chats_root();
    if !root.is_dir() {
        return Vec::new();
    }
    let mut files = Vec::new();
    let Ok(workspaces) = fs::read_dir(&root) else {
        return files;
    };
    for workspace in workspaces.flatten() {
        let workspace_path = workspace.path();
        if !workspace_path.is_dir() {
            continue;
        }
        let Ok(sessions) = fs::read_dir(&workspace_path) else {
            continue;
        };
        for session in sessions.flatten() {
            let dir = session.path();
            if !dir.is_dir() {
                continue;
            }
            let meta_path = dir.join("meta.json");
            let Ok(text) = fs::read_to_string(&meta_path) else {
                continue;
            };
            let Ok(meta) = serde_json::from_str::<CursorMeta>(&text) else {
                continue;
            };
            if !meta.has_conversation {
                continue;
            }
            let id = dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }
            files.push(CursorSessionFile { id, dir, meta });
        }
    }
    files
}

fn find_session_file(session_id: &str) -> Option<CursorSessionFile> {
    list_cursor_session_files()
        .into_iter()
        .find(|file| file.id == session_id)
}

fn find_transcript(session_id: &str) -> Option<PathBuf> {
    let root = projects_root();
    if !root.is_dir() {
        return None;
    }
    let Ok(projects) = fs::read_dir(&root) else {
        return None;
    };
    for project in projects.flatten() {
        let path = project
            .path()
            .join("agent-transcripts")
            .join(session_id)
            .join(format!("{session_id}.jsonl"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn title_from_meta(meta: &CursorMeta, id: &str) -> String {
    meta.title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| id.to_string())
}

fn collect_cursor_sessions() -> Result<Vec<SessionSummary>, String> {
    let mut sessions: Vec<SessionSummary> = list_cursor_session_files()
        .into_iter()
        .map(|file| {
            let message_count = find_transcript(&file.id)
                .and_then(|path| count_transcript_messages(&path))
                .or_else(|| count_prompt_history(&file.dir));
            SessionSummary {
                id: file.id.clone(),
                title: title_from_meta(&file.meta, &file.id),
                project_path: file.meta.cwd.clone().unwrap_or_default(),
                model: None,
                started_at: ms_to_secs(file.meta.created_at_ms),
                updated_at: ms_to_secs(file.meta.updated_at_ms),
                message_count,
                tokens_used: None,
                platform_id: PLATFORM_ID.to_string(),
                source: Some("terminal".to_string()),
            }
        })
        .collect();
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}

fn count_transcript_messages(path: &Path) -> Option<u32> {
    let file = fs::File::open(path).ok()?;
    let mut count = 0u32;
    for line in BufReader::new(file).lines().flatten() {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if parse_transcript_line(&value).is_some() {
            count += 1;
        }
    }
    Some(count)
}

fn count_prompt_history(dir: &Path) -> Option<u32> {
    let text = fs::read_to_string(dir.join("prompt_history.json")).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value.as_array().map(|items| items.len() as u32)
}

fn content_text(content: &Value) -> String {
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    let Some(blocks) = content.as_array() else {
        return String::new();
    };
    blocks
        .iter()
        .filter_map(|block| {
            if block.get("type").and_then(Value::as_str) == Some("text") {
                block.get("text").and_then(Value::as_str)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_transcript_line(value: &Value) -> Option<SessionMessage> {
    let role = value.get("role").and_then(Value::as_str)?;
    if role != "user" && role != "assistant" {
        return None;
    }
    let content = value
        .get("message")
        .and_then(|message| message.get("content"))
        .or_else(|| value.get("content"))?;
    let text = content_text(content);
    if text.trim().is_empty() {
        return None;
    }
    Some(SessionMessage::new(role, text, 0))
}

fn read_messages_from_jsonl(
    path: &Path,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let file = fs::File::open(path).map_err(|err| err.to_string())?;
    let mut messages = Vec::new();
    let mut matched = 0usize;
    let page_limit = limit.max(1);
    for line in BufReader::new(file).lines().flatten() {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(message) = parse_transcript_line(&value) else {
            continue;
        };
        if matched >= offset && messages.len() < page_limit {
            messages.push(message);
        }
        matched += 1;
        if messages.len() >= page_limit && matched > offset {
            // keep counting only if we already filled the page
        }
    }
    Ok(messages)
}

fn read_prompt_history(dir: &Path) -> Vec<SessionMessage> {
    let Ok(text) = fs::read_to_string(dir.join("prompt_history.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str())
        .filter(|text| !text.trim().is_empty())
        .map(|text| SessionMessage::new("user", text, 0))
        .collect()
}

fn load_messages(session_id: &str) -> Result<Vec<SessionMessage>, String> {
    if let Some(path) = find_transcript(session_id) {
        return read_messages_from_jsonl(&path, 0, usize::MAX);
    }
    let file = find_session_file(session_id)
        .ok_or_else(|| format!("Cursor session not found for id: {session_id}"))?;
    Ok(read_prompt_history(&file.dir))
}

pub fn count_cursor_sessions() -> Result<usize, String> {
    Ok(list_cursor_session_files().len())
}

pub fn list_cursor_sessions_all() -> Result<Vec<SessionSummary>, String> {
    collect_cursor_sessions()
}

pub fn get_cursor_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let messages = load_messages(session_id)?;
    Ok(messages.into_iter().skip(offset).take(limit.max(1)).collect())
}

pub fn last_cursor_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let messages = load_messages(session_id)?;
    let last_user = messages.iter().rev().find(|msg| msg.role == "user").cloned();
    let last_assistant = messages
        .iter()
        .rev()
        .find(|msg| msg.role == "assistant")
        .cloned();
    Ok((last_user, last_assistant))
}

pub fn search_cursor_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let sessions = collect_cursor_sessions()?;
    let mut results = Vec::new();
    for session in sessions {
        let Ok(messages) = load_messages(&session.id) else {
            continue;
        };
        for message in messages {
            if message.matches_query(query_lower) {
                results.push(crate::session::SessionSearchResult {
                    session_id: session.id.clone(),
                    session_title: session.title.clone(),
                    project_path: session.project_path.clone(),
                    platform_id: PLATFORM_ID.to_string(),
                    message,
                });
            }
        }
    }
    Ok(results)
}

pub fn delete_cursor_session(session_id: &str) -> Result<(), String> {
    let file = find_session_file(session_id)
        .ok_or_else(|| format!("Cursor session not found for id: {session_id}"))?;
    fs::remove_dir_all(&file.dir).map_err(|err| {
        format!(
            "Failed to delete Cursor session {}: {err}",
            file.dir.display()
        )
    })?;
    if let Some(path) = find_transcript(session_id) {
        if let Some(dir) = path.parent() {
            let _ = fs::remove_dir_all(dir);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_agent_transcript_line() {
        let line = serde_json::json!({
            "role": "user",
            "message": {"content": [{"type": "text", "text": "<user_query>\nhello\n</user_query>"}]}
        });
        let message = parse_transcript_line(&line).expect("user line");
        assert_eq!(message.role, "user");
        assert_eq!(message.content, "hello");
    }

    #[test]
    fn skips_turn_ended() {
        let line = serde_json::json!({"type": "turn_ended", "status": "success"});
        assert!(parse_transcript_line(&line).is_none());
    }

    #[test]
    fn ms_to_secs_converts_millis() {
        assert_eq!(ms_to_secs(1_787_845_175_316), 1_787_845_175);
        assert_eq!(ms_to_secs(1_787_845_175), 1_787_845_175);
    }

    #[test]
    fn cursor_sessions_real_data_smoke_test() {
        let files = list_cursor_session_files();
        if files.is_empty() {
            return;
        }
        let sessions = list_cursor_sessions_all().expect("cursor scan should not fail");
        assert!(!sessions.is_empty());
        let first = &sessions[0];
        let _ = get_cursor_messages(&first.id, 0, 20);
    }
}

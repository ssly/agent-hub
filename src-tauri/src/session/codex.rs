use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, Row};
use serde_json::Value;

use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "codex-cli";

pub fn count_codex_sessions() -> Result<usize, String> {
    let db_path = codex_db_path()?;
    if !db_path.exists() {
        return Ok(0);
    }

    let conn = open_codex_db_readonly(&db_path)?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM threads WHERE archived = 0",
            [],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;
    usize::try_from(count).map_err(|err| err.to_string())
}

pub fn list_codex_sessions(offset: usize, limit: usize) -> Result<(usize, Vec<SessionSummary>), String> {
    let db_path = codex_db_path()?;
    if !db_path.exists() {
        return Ok((0, Vec::new()));
    }

    let conn = open_codex_db_readonly(&db_path)?;
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM threads WHERE archived = 0",
            [],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;
    let total = usize::try_from(total).map_err(|err| err.to_string())?;
    let page_limit = i64::try_from(limit.max(1)).unwrap_or(i64::MAX);
    let page_offset = i64::try_from(offset).map_err(|err| err.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, title, cwd, model, tokens_used, created_at, updated_at, first_user_message \
             FROM threads WHERE archived = 0 ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([page_limit, page_offset], parse_codex_summary_row)
        .map_err(|err| err.to_string())?;

    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row.map_err(|err| err.to_string())?);
    }
    Ok((total, sessions))
}

pub fn get_codex_messages(session_id: &str, offset: usize, limit: usize) -> Result<Vec<SessionMessage>, String> {
    let db_path = codex_db_path()?;
    let conn = open_codex_db_readonly(&db_path)?;
    let rollout_path: String = conn
        .query_row(
            "SELECT rollout_path FROM threads WHERE id = ?1 LIMIT 1",
            [session_id],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;

    let file = fs::File::open(&rollout_path).map_err(|err| err.to_string())?;
    let reader = BufReader::new(file);

    let mut messages = Vec::new();
    let mut matched = 0usize;
    let page_limit = limit.max(1);

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let data: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(message) = parse_codex_rollout_message(&data) else {
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

fn parse_codex_rollout_message(value: &Value) -> Option<SessionMessage> {
    let line_type = value.get("type").and_then(|v| v.as_str())?;
    if line_type == "event_msg" {
        let payload = value.get("payload")?;
        if payload.get("type").and_then(|v| v.as_str())? != "user_message" {
            return None;
        }
        let content = payload.get("message").and_then(|v| v.as_str())?.trim();
        if content.is_empty() {
            return None;
        }
        return Some(SessionMessage {
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: value
                .get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(parse_rfc3339_to_ms)
                .unwrap_or(0),
        });
    }

    if line_type == "response_item" {
        let payload = value.get("payload")?;
        if payload.get("type").and_then(|v| v.as_str())? != "message" {
            return None;
        }
        if payload.get("role").and_then(|v| v.as_str())? != "assistant" {
            return None;
        }
        let content = payload
            .get("content")
            .and_then(extract_output_text_content)?;
        if content.trim().is_empty() {
            return None;
        }
        return Some(SessionMessage {
            role: "assistant".to_string(),
            content,
            timestamp: value
                .get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(parse_rfc3339_to_ms)
                .unwrap_or(0),
        });
    }

    None
}

fn parse_codex_summary_row(row: &Row<'_>) -> Result<SessionSummary, rusqlite::Error> {
    let id: String = row.get(0)?;
    let title: String = row.get(1)?;
    let project_path: String = row.get(2)?;
    let model: Option<String> = row.get(3)?;
    let tokens_used: u64 = row.get(4)?;
    let created_at: i64 = row.get(5)?;
    let updated_at: i64 = row.get(6)?;
    let first_user_message: String = row.get(7)?;
    let title = if title.trim().is_empty() {
        truncate_chars(first_user_message, 80)
    } else {
        title
    };

    Ok(SessionSummary {
        id,
        title,
        project_path,
        model,
        started_at: created_at.saturating_mul(1000),
        updated_at: updated_at.saturating_mul(1000),
        message_count: None,
        tokens_used: Some(tokens_used),
        platform_id: PLATFORM_ID.to_string(),
    })
}

fn codex_db_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    Ok(home.join(".codex/state_5.sqlite"))
}

fn open_codex_db_readonly(path: &PathBuf) -> Result<Connection, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    for attempt in 0..2 {
        match Connection::open_with_flags(path, flags) {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                if attempt == 0 {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
                return Err(error.to_string());
            }
        }
    }
    Err(format!("Unable to open Codex database: {}", path.display()))
}

fn parse_rfc3339_to_ms(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.timestamp_millis())
}

fn extract_output_text_content(content: &Value) -> Option<String> {
    let Value::Array(items) = content else {
        return None;
    };
    let mut parts = Vec::new();
    for item in items {
        if item.get("type").and_then(|v| v.as_str()) != Some("output_text") {
            continue;
        }
        let Some(text) = item.get("text").and_then(|value| value.as_str()) else {
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

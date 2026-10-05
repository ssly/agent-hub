//! OpenCode session adapter.
//!
//! OpenCode (anomalyco / SST) stores its sessions in a local SQLite database at
//! `~/.local/share/opencode/opencode.db`.
//!
//! Schemas supported:
//! - Modern OpenCode (v2): `session_v2` and `session_message` tables.
//!   - `session_v2`: columns `id`, `title`, `directory`, `model`, `time_created`, `time_updated`, etc.
//!   - `session_message`: columns `id`, `session_id`, `type` ("user" | "assistant" | "idle"), `seq`, `data` (JSON).
//! - Legacy OpenCode (v1): `session`, `message`, and `part` tables.

use crate::session::models::{SessionMessage, SessionSearchResult, SessionSummary};
use rusqlite::{Connection, OpenFlags, Row};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const PLATFORM_ID: &str = "opencode";

fn opencode_db_path() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".local").join("share").join("opencode").join("opencode.db"))
        .ok_or_else(|| "home directory is unavailable".to_string())
}

fn open_opencode_db_readonly(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| format!("failed to open OpenCode database {}: {}", path.display(), err))
}

fn open_opencode_db_readwrite(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| format!("failed to open OpenCode database {}: {}", path.display(), err))
}

fn has_table(conn: &Connection, table_name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1",
        [table_name],
        |_| Ok(()),
    )
    .is_ok()
}

pub fn count_opencode_sessions() -> Result<usize, String> {
    let db_path = opencode_db_path()?;
    if !db_path.exists() {
        return Ok(0);
    }

    let conn = open_opencode_db_readonly(&db_path)?;
    let count: i64 = if has_table(&conn, "session_v2") && has_table(&conn, "session") {
        conn.query_row(
            "SELECT COUNT(DISTINCT id) FROM ( \
             SELECT id FROM session_v2 WHERE time_archived IS NULL \
             UNION \
             SELECT id FROM session WHERE time_archived IS NULL \
             )",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
    } else if has_table(&conn, "session_v2") {
        conn.query_row(
            "SELECT COUNT(*) FROM session_v2 WHERE time_archived IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
    } else if has_table(&conn, "session") {
        conn.query_row(
            "SELECT COUNT(*) FROM session WHERE time_archived IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
    } else {
        0
    };
    usize::try_from(count).map_err(|err| err.to_string())
}

fn map_session_summary_row(row: &Row) -> rusqlite::Result<SessionSummary> {
    let id: String = row.get(0)?;
    let title: Option<String> = row.get(1)?;
    let project_path: Option<String> = row.get(2)?;
    let model_raw: Option<String> = row.get(3)?;
    let created_at: Option<i64> = row.get(4)?;
    let updated_at: Option<i64> = row.get(5)?;
    let tokens_val: Option<i64> = row.get(6)?;
    let message_count: Option<i64> = row.get(7)?;

    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| id.clone());
    let project_path = project_path.unwrap_or_default();
    let created_at = created_at.unwrap_or(0);
    let updated_at = updated_at.unwrap_or(created_at);

    let model = model_raw.as_deref().map(opencode_display_model);
    let tokens_used = tokens_val
        .filter(|&v| v > 0)
        .and_then(|v| u64::try_from(v).ok());
    let message_count = message_count
        .filter(|&v| v > 0)
        .and_then(|v| u32::try_from(v).ok());

    Ok(SessionSummary {
        id,
        title,
        project_path,
        model,
        started_at: created_at,
        updated_at,
        message_count,
        tokens_used,
        platform_id: PLATFORM_ID.to_string(),
        source: None,
    })
}

pub fn list_opencode_sessions_all() -> Result<Vec<SessionSummary>, String> {
    let db_path = opencode_db_path()?;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = open_opencode_db_readonly(&db_path)?;
    let mut sessions = Vec::new();
    let mut seen_ids = HashSet::new();

    // 1. Query modern session_v2 table if available
    if has_table(&conn, "session_v2") {
        let msg_subquery = if has_table(&conn, "session_message") {
            "SELECT session_id, COUNT(*) as cnt FROM session_message WHERE type IN ('user', 'assistant') GROUP BY session_id"
        } else {
            "SELECT session_id, COUNT(*) as cnt FROM message GROUP BY session_id"
        };
        let query = format!(
            "SELECT s.id, s.title, s.directory, s.model, s.time_created, s.time_updated, \
             s.tokens_input + s.tokens_output + s.tokens_reasoning, \
             COALESCE(m.cnt, 0) \
             FROM session_v2 s \
             LEFT JOIN ({msg_subquery}) m ON m.session_id = s.id \
             WHERE s.time_archived IS NULL \
             ORDER BY s.time_updated DESC"
        );
        if let Ok(mut stmt) = conn.prepare(&query) {
            let rows = stmt
                .query_map([], |row| map_session_summary_row(row))
                .map_err(|err| err.to_string())?;
            for row in rows {
                let summary = row.map_err(|err| err.to_string())?;
                seen_ids.insert(summary.id.clone());
                sessions.push(summary);
            }
        }
    }

    // 2. Query legacy session table for backwards compatibility
    if has_table(&conn, "session") {
        let query = "SELECT s.id, s.title, s.directory, s.model, s.time_created, s.time_updated, \
                     s.tokens_input + s.tokens_output + s.tokens_reasoning, \
                     COALESCE(m.cnt, 0) \
                     FROM session s \
                     LEFT JOIN ( \
                         SELECT session_id, COUNT(*) as cnt FROM message GROUP BY session_id \
                     ) m ON m.session_id = s.id \
                     WHERE s.time_archived IS NULL \
                     ORDER BY s.time_updated DESC";
        if let Ok(mut stmt) = conn.prepare(query) {
            let rows = stmt
                .query_map([], |row| map_session_summary_row(row))
                .map_err(|err| err.to_string())?;
            for row in rows {
                let summary = row.map_err(|err| err.to_string())?;
                if seen_ids.insert(summary.id.clone()) {
                    sessions.push(summary);
                }
            }
        }
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}

fn opencode_display_model(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            if let Some(id) = val.get("id").and_then(|v| v.as_str()) {
                return id.to_string();
            }
            if let Some(model_id) = val.get("modelID").and_then(|v| v.as_str()) {
                return model_id.to_string();
            }
        }
    }
    raw.rsplit('/').next().unwrap_or(raw).to_string()
}

pub fn get_opencode_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let db_path = opencode_db_path()?;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = open_opencode_db_readonly(&db_path)?;

    // Check if session_message (v2) has rows for this session
    if has_table(&conn, "session_message") {
        let mut check_stmt = conn
            .prepare("SELECT COUNT(*) FROM session_message WHERE session_id = ?1 AND type IN ('user', 'assistant')")
            .map_err(|err| err.to_string())?;
        let v2_count: i64 = check_stmt
            .query_row([session_id], |row| row.get(0))
            .unwrap_or(0);

        if v2_count > 0 {
            return get_opencode_v2_messages(&conn, session_id, offset, limit);
        }
    }

    // Fallback to legacy v1 message + part tables
    get_opencode_v1_messages(&conn, session_id, offset, limit)
}

fn get_opencode_v2_messages(
    conn: &Connection,
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type, time_created, data FROM session_message \
             WHERE session_id = ?1 AND type IN ('user', 'assistant') \
             ORDER BY seq ASC, time_created ASC, id ASC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            ))
        })
        .map_err(|err| err.to_string())?;

    let mut messages = Vec::new();
    let mut matched = 0usize;
    let page_limit = limit.max(1);

    for row in rows {
        let (_msg_id, msg_type, time_created, data_raw) = row.map_err(|err| err.to_string())?;
        let data: Value = match serde_json::from_str(&data_raw) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if msg_type == "user" {
            let text = data
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if text.is_empty() {
                continue;
            }
            if matched >= offset {
                messages.push(SessionMessage::new("user", text.to_string(), time_created));
                if messages.len() >= page_limit {
                    break;
                }
            }
            matched += 1;
        } else if msg_type == "assistant" {
            let mut texts = Vec::new();
            let mut reasonings = Vec::new();

            if let Some(content_array) = data.get("content").and_then(|v| v.as_array()) {
                for block in content_array {
                    let block_type = block.get("type").and_then(|v| v.as_str());
                    let block_text = block.get("text").and_then(|v| v.as_str()).unwrap_or("").trim();
                    if block_text.is_empty() {
                        continue;
                    }
                    if block_type == Some("reasoning") {
                        reasonings.push(block_text.to_string());
                    } else if block_type == Some("text") {
                        texts.push(block_text.to_string());
                    }
                }
            }

            let content = texts.join("\n\n");
            let thinking = if reasonings.is_empty() {
                None
            } else {
                Some(reasonings.join("\n\n"))
            };

            if content.is_empty() && thinking.is_none() {
                continue;
            }

            if matched >= offset {
                let mut msg = SessionMessage::new("assistant", content, time_created);
                msg.thinking = thinking;
                messages.push(msg);
                if messages.len() >= page_limit {
                    break;
                }
            }
            matched += 1;
        }
    }

    Ok(messages)
}

fn get_opencode_v1_messages(
    conn: &Connection,
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    if !has_table(conn, "message") || !has_table(conn, "part") {
        return Ok(Vec::new());
    }

    let mut msg_stmt = conn
        .prepare(
            "SELECT id, time_created, data FROM message \
             WHERE session_id = ?1 ORDER BY time_created, id",
        )
        .map_err(|err| err.to_string())?;

    let mut part_stmt = conn
        .prepare("SELECT data FROM part WHERE message_id = ?1 ORDER BY time_created, id")
        .map_err(|err| err.to_string())?;

    let rows = msg_stmt
        .query_map([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        })
        .map_err(|err| err.to_string())?;

    let mut messages = Vec::new();
    let mut matched = 0usize;
    let page_limit = limit.max(1);

    for row in rows {
        let (message_id, time_created, data) = row.map_err(|err| err.to_string())?;
        let data: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let role = match data.get("role").and_then(|v| v.as_str()) {
            Some("user") => "user",
            Some("assistant") => "assistant",
            _ => continue,
        };

        let (content, thinking) = read_opencode_parts(&mut part_stmt, &message_id)?;
        if content.is_empty() && thinking.is_none() {
            continue;
        }

        if matched >= offset {
            let mut msg = SessionMessage::new(role, content, time_created);
            msg.thinking = thinking;
            messages.push(msg);
            if messages.len() >= page_limit {
                break;
            }
        }
        matched += 1;
    }

    Ok(messages)
}

fn read_opencode_parts(
    part_stmt: &mut rusqlite::Statement,
    message_id: &str,
) -> Result<(String, Option<String>), String> {
    let parts = part_stmt
        .query_map([message_id], |row| row.get::<_, Option<String>>(0))
        .map_err(|err| err.to_string())?;

    let mut texts = Vec::new();
    let mut reasonings = Vec::new();

    for part in parts {
        let data: Value = match part.ok().flatten().and_then(|raw| serde_json::from_str(&raw).ok()) {
            Some(value) => value,
            None => continue,
        };

        match data.get("type").and_then(|v| v.as_str()) {
            Some("text") => {
                if let Some(text) = data.get("text").and_then(|v| v.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        texts.push(trimmed.to_string());
                    }
                }
            }
            Some("reasoning") => {
                if let Some(text) = data.get("text").and_then(|v| v.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        reasonings.push(trimmed.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    let content = texts.join("\n\n");
    let thinking = if reasonings.is_empty() {
        None
    } else {
        Some(reasonings.join("\n\n"))
    };

    Ok((content, thinking))
}

pub fn last_opencode_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let messages = get_opencode_messages(session_id, 0, usize::MAX)?;
    let last_user = messages.iter().rev().find(|m| m.role == "user").cloned();
    let last_assistant = messages.iter().rev().find(|m| m.role == "assistant").cloned();
    Ok((last_user, last_assistant))
}

pub fn delete_opencode_session(session_id: &str) -> Result<(), String> {
    let db_path = opencode_db_path()?;
    if !db_path.exists() {
        return Err(format!("OpenCode database not found at {}", db_path.display()));
    }

    let conn = open_opencode_db_readwrite(&db_path)?;
    let mut deleted = 0;

    // Clean up v2 tables if present
    if has_table(&conn, "session_message") {
        let _ = conn.execute("DELETE FROM session_message WHERE session_id = ?1", [session_id]);
    }
    if has_table(&conn, "session_v2") {
        if let Ok(affected) = conn.execute("DELETE FROM session_v2 WHERE id = ?1", [session_id]) {
            deleted += affected;
        }
    }

    // Clean up v1 tables if present
    if has_table(&conn, "part") {
        let _ = conn.execute("DELETE FROM part WHERE session_id = ?1", [session_id]);
    }
    if has_table(&conn, "message") {
        let _ = conn.execute("DELETE FROM message WHERE session_id = ?1", [session_id]);
    }
    if has_table(&conn, "session") {
        if let Ok(affected) = conn.execute("DELETE FROM session WHERE id = ?1", [session_id]) {
            deleted += affected;
        }
    }

    if deleted == 0 {
        return Err(format!("session '{}' not found", session_id));
    }
    Ok(())
}

pub fn search_opencode_messages(
    query_lower: &str,
) -> Result<Vec<SessionSearchResult>, String> {
    let sessions = list_opencode_sessions_all()?;
    let db_path = opencode_db_path()?;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    for session in sessions {
        let Ok(messages) = get_opencode_messages(&session.id, 0, usize::MAX) else {
            continue;
        };
        for msg in messages {
            if msg.content.to_lowercase().contains(query_lower) {
                results.push(SessionSearchResult {
                    session_id: session.id.clone(),
                    session_title: session.title.clone(),
                    project_path: session.project_path.clone(),
                    platform_id: PLATFORM_ID.to_string(),
                    message: msg,
                });
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_session_summary_row_handles_null_title_and_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE test_session (
                id TEXT NOT NULL,
                title TEXT,
                directory TEXT,
                model TEXT,
                time_created INTEGER,
                time_updated INTEGER,
                tokens INTEGER,
                cnt INTEGER
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO test_session (id, title, directory, model, time_created, time_updated, tokens, cnt)
             VALUES ('ses_null_title', NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
            [],
        )
        .unwrap();

        let mut stmt = conn
            .prepare("SELECT id, title, directory, model, time_created, time_updated, tokens, cnt FROM test_session")
            .unwrap();
        let session = stmt
            .query_row([], |row| map_session_summary_row(row))
            .expect("should successfully parse row with NULL title and columns");

        assert_eq!(session.id, "ses_null_title");
        assert_eq!(session.title, "ses_null_title");
        assert_eq!(session.project_path, "");
        assert_eq!(session.started_at, 0);
        assert_eq!(session.tokens_used, None);
        assert_eq!(session.message_count, None);
    }
}


use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
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

pub fn list_codex_sessions_all() -> Result<Vec<SessionSummary>, String> {
    let db_path = codex_db_path()?;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = open_codex_db_readonly(&db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, cwd, model, tokens_used, created_at, updated_at, first_user_message \
             FROM threads WHERE archived = 0 ORDER BY updated_at DESC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], parse_codex_summary_row)
        .map_err(|err| err.to_string())?;

    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row.map_err(|err| err.to_string())?);
    }
    Ok(sessions)
}

pub fn get_codex_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
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

pub fn delete_codex_session(session_id: &str) -> Result<(), String> {
    let db_path = codex_db_path()?;
    delete_codex_session_in_db(&db_path, session_id)
}

pub fn delete_codex_sessions(session_ids: &[String]) -> Result<usize, String> {
    let db_path = codex_db_path()?;
    delete_codex_sessions_in_db(&db_path, session_ids)
}

fn delete_codex_sessions_in_db(path: &Path, session_ids: &[String]) -> Result<usize, String> {
    if !path.exists() {
        return Err(format!(
            "Codex session database not found: {}",
            path.display()
        ));
    }
    if session_ids.is_empty() {
        return Ok(0);
    }

    let conn = open_codex_db_readwrite(path)?;
    // One write-lock acquisition marks all requested threads archived in a single
    // statement, instead of reopening a readwrite connection per thread (each of
    // which would re-fight the CLI's write lock under contention). rows-affected ==
    // count of rows flipped archived 0 -> 1.
    use rusqlite::params_from_iter;
    let placeholders = vec!["?"; session_ids.len()];
    let sql = format!(
        "UPDATE threads SET archived = 1 WHERE archived = 0 AND id IN ({})",
        placeholders.join(", ")
    );
    let changed = conn
        .execute(&sql, params_from_iter(session_ids.iter().map(|s| s.as_str())))
        .map_err(|err| err.to_string())?;
    Ok(changed)
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

fn open_codex_db_readonly(path: &Path) -> Result<Connection, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    open_codex_db_with_flags(path, flags)
}

fn open_codex_db_readwrite(path: &Path) -> Result<Connection, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    open_codex_db_with_flags(path, flags)
}

fn open_codex_db_with_flags(path: &Path, flags: OpenFlags) -> Result<Connection, String> {
    for attempt in 0..3 {
        match Connection::open_with_flags(path, flags) {
            Ok(connection) => {
                let _ = connection.busy_timeout(Duration::from_millis(1500));
                return Ok(connection);
            }
            Err(error) => {
                if attempt < 2 {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
                return Err(error.to_string());
            }
        }
    }
    Err(format!("Unable to open Codex database: {}", path.display()))
}

fn delete_codex_session_in_db(path: &Path, session_id: &str) -> Result<(), String> {
    if !path.exists() {
        return Err(format!(
            "Codex session database not found: {}",
            path.display()
        ));
    }

    let conn = open_codex_db_readwrite(path)?;
    let changed = conn
        .execute(
            "UPDATE threads SET archived = 1 WHERE id = ?1 AND archived = 0",
            [session_id],
        )
        .map_err(|err| err.to_string())?;
    if changed > 0 {
        return Ok(());
    }

    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM threads WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;
    if exists > 0 {
        return Err(format!("Codex session already archived: {}", session_id));
    }
    Err(format!("Codex session not found: {}", session_id))
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

pub fn search_codex_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let db_path = codex_db_path()?;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = open_codex_db_readonly(&db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, cwd, model, tokens_used, created_at, updated_at, first_user_message, rollout_path \
             FROM threads WHERE archived = 0 ORDER BY updated_at DESC",
        )
        .map_err(|err| err.to_string())?;

    struct ThreadInfo {
        summary: SessionSummary,
        rollout_path: String,
    }

    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let project_path: String = row.get(2)?;
            let model: Option<String> = row.get(3)?;
            let tokens_used: u64 = row.get(4)?;
            let created_at: i64 = row.get(5)?;
            let updated_at: i64 = row.get(6)?;
            let first_user_message: String = row.get(7)?;
            let rollout_path: String = row.get(8)?;

            let title = if title.trim().is_empty() {
                truncate_chars(first_user_message, 80)
            } else {
                title
            };

            Ok(ThreadInfo {
                summary: SessionSummary {
                    id,
                    title,
                    project_path,
                    model,
                    started_at: created_at.saturating_mul(1000),
                    updated_at: updated_at.saturating_mul(1000),
                    message_count: None,
                    tokens_used: Some(tokens_used),
                    platform_id: PLATFORM_ID.to_string(),
                },
                rollout_path,
            })
        })
        .map_err(|err| err.to_string())?;

    let mut results = Vec::new();
    for row in rows {
        let thread = match row {
            Ok(t) => t,
            Err(_) => continue,
        };

        if thread.rollout_path.is_empty() {
            continue;
        }

        let Ok(file) = fs::File::open(&thread.rollout_path) else {
            continue;
        };
        let reader = BufReader::new(file);

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
            if message.content.to_lowercase().contains(query_lower) {
                results.push(crate::session::SessionSearchResult {
                    session_id: thread.summary.id.clone(),
                    session_title: thread.summary.title.clone(),
                    project_path: thread.summary.project_path.clone(),
                    platform_id: PLATFORM_ID.to_string(),
                    message,
                });
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_codex_db() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().expect("temp dir should create");
        let db_path = dir.path().join("state_5.sqlite");
        let conn = Connection::open(&db_path).expect("sqlite db should create");
        conn.execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                cwd TEXT NOT NULL,
                model TEXT,
                tokens_used INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                first_user_message TEXT NOT NULL DEFAULT '',
                rollout_path TEXT NOT NULL DEFAULT '',
                archived INTEGER NOT NULL DEFAULT 0
            );",
        )
        .expect("threads table should create");
        (dir, db_path)
    }

    #[test]
    fn delete_codex_session_sets_archived_flag() {
        let (_dir, db_path) = create_test_codex_db();
        let conn = Connection::open(&db_path).expect("db should open");
        conn.execute(
            "INSERT INTO threads (id, title, cwd, model, tokens_used, created_at, updated_at, first_user_message, rollout_path, archived)
             VALUES (?1, 't', '/tmp', NULL, 0, 1, 1, '', '', 0)",
            ["thread-1"],
        )
        .expect("thread should insert");
        drop(conn);

        delete_codex_session_in_db(&db_path, "thread-1").expect("delete should succeed");

        let conn = Connection::open(&db_path).expect("db should reopen");
        let archived: i64 = conn
            .query_row(
                "SELECT archived FROM threads WHERE id = ?1",
                ["thread-1"],
                |row| row.get(0),
            )
            .expect("archived should load");
        assert_eq!(archived, 1);
    }

    #[test]
    fn delete_codex_session_reports_missing_session() {
        let (_dir, db_path) = create_test_codex_db();
        let err = delete_codex_session_in_db(&db_path, "missing")
            .expect_err("missing session should fail");
        assert!(err.contains("not found"));
    }

    #[test]
    fn delete_codex_session_reports_missing_database() {
        let mut missing = std::env::temp_dir();
        let unique = format!(
            "agent-hub-codex-missing-{}.sqlite",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        );
        missing.push(unique);
        let err = delete_codex_session_in_db(&missing, "thread-1")
            .expect_err("missing database should fail");
        assert!(err.contains("database not found"));
    }

    #[test]
    fn delete_codex_sessions_archives_all_in_one_statement() {
        let (_dir, db_path) = create_test_codex_db();
        let conn = Connection::open(&db_path).expect("db should open");
        for id in ["t1", "t2", "t3"] {
            conn.execute(
                "INSERT INTO threads (id, title, cwd, model, tokens_used, created_at, updated_at, first_user_message, rollout_path, archived)
                 VALUES (?1, 't', '/tmp', NULL, 0, 1, 1, '', '', 0)",
                [id],
            )
            .expect("insert");
        }
        drop(conn);

        let n = delete_codex_sessions_in_db(
            &db_path,
            &["t1".to_string(), "t2".to_string(), "t3".to_string()],
        )
        .expect("batch delete should succeed");
        assert_eq!(n, 3);

        let conn = Connection::open(&db_path).expect("db should reopen");
        let archived: i64 = conn
            .query_row(
                "SELECT SUM(archived) FROM threads WHERE id IN ('t1','t2','t3')",
                [],
                |row| row.get(0),
            )
            .expect("sum");
        assert_eq!(archived, 3);
    }

    #[test]
    fn delete_codex_sessions_reports_missing_database() {
        let mut missing = std::env::temp_dir();
        missing.push(format!(
            "agent-hub-codex-batch-missing-{}.sqlite",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let err = delete_codex_sessions_in_db(&missing, &["x".to_string()])
            .expect_err("missing db should fail");
        assert!(err.contains("database not found"));
    }

    #[test]
    fn delete_codex_sessions_empty_is_ok_zero() {
        let (_dir, db_path) = create_test_codex_db();
        let n = delete_codex_sessions_in_db(&db_path, &[]).expect("empty batch ok");
        assert_eq!(n, 0);
    }
}

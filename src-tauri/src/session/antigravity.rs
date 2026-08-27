//! Antigravity (agy CLI / Antigravity 2.0) session browser.
//!
//! Index: `~/.gemini/antigravity-cli/conversation_summaries.db` (covers both
//! CLI and IDE via `app_data_dir`). Message text is read from the human-readable
//! transcript at `<app>/brain/<id>/.system_generated/logs/transcript.jsonl`
//! rather than the protobuf trajectory SQLite (which is hard to decode).

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::paths::join_relative;
use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "antigravity";

pub fn count_antigravity_sessions() -> Result<usize, String> {
    let db_path = summaries_db_path()?;
    if !db_path.exists() {
        return Ok(0);
    }
    let conn = open_readonly(&db_path)?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM conversation_summaries", [], |row| {
            row.get(0)
        })
        .map_err(|err| err.to_string())?;
    usize::try_from(count).map_err(|err| err.to_string())
}

pub fn list_antigravity_sessions_all() -> Result<Vec<SessionSummary>, String> {
    let db_path = summaries_db_path()?;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = open_readonly(&db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT conversation_id, title, preview, step_count, last_modified_time, \
             workspace_uris, app_data_dir, last_user_input_time \
             FROM conversation_summaries \
             ORDER BY last_modified_time DESC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let preview: String = row.get(2)?;
            let step_count: i64 = row.get(3)?;
            let last_modified: String = row.get(4)?;
            let workspace_uris: String = row.get(5)?;
            let app_data_dir: String = row.get(6)?;
            let last_user_input: String = row.get(7)?;
            Ok((
                id,
                title,
                preview,
                step_count,
                last_modified,
                workspace_uris,
                app_data_dir,
                last_user_input,
            ))
        })
        .map_err(|err| err.to_string())?;

    let mut sessions = Vec::new();
    for row in rows {
        let (
            id,
            title,
            preview,
            step_count,
            last_modified,
            workspace_uris,
            app_data_dir,
            last_user,
        ) = row.map_err(|err| err.to_string())?;
        if id.trim().is_empty() {
            continue;
        }

        let display_title = first_nonempty(&[&title, &preview]).unwrap_or_else(|| id.clone());
        let project_path = first_workspace_path(&workspace_uris).unwrap_or_default();
        let updated_at = parse_sqlite_datetime(&last_modified)
            .or_else(|| parse_sqlite_datetime(&last_user))
            .unwrap_or(0);
        let started_at = parse_sqlite_datetime(&last_user).unwrap_or(updated_at);
        // Product surface from the summaries index (official app_data_dir):
        // antigravity-cli → terminal CLI, antigravity → desktop 2.0,
        // antigravity-ide → IDE. Same strings the monitor capture uses.
        let source = match app_data_dir.as_str() {
            "antigravity-cli" => Some("terminal".to_string()),
            "antigravity-ide" => Some("antigravity-ide".to_string()),
            "antigravity" => Some("antigravity".to_string()),
            other if !other.trim().is_empty() => Some(other.to_string()),
            _ => None,
        };
        let message_count = u32::try_from(step_count.max(0)).ok();

        sessions.push(SessionSummary {
            id,
            title: display_title,
            project_path,
            model: None,
            started_at,
            updated_at,
            message_count,
            tokens_used: None,
            platform_id: PLATFORM_ID.to_string(),
            source,
        });
    }
    Ok(sessions)
}

pub fn get_antigravity_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let path = resolve_transcript_path(session_id)?;
    read_messages_from_transcript(&path, offset, limit)
}

pub fn last_antigravity_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let path = resolve_transcript_path(session_id)?;
    let file = fs::File::open(&path).map_err(|err| err.to_string())?;
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
        let Some(message) = parse_transcript_line(&value) else {
            continue;
        };
        if message.role == "user" {
            last_user = Some(message);
        } else if message.role == "assistant" {
            last_assistant = Some(message);
        }
    }
    Ok((last_user, last_assistant))
}

pub fn search_antigravity_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let sessions = list_antigravity_sessions_all()?;
    let mut results = Vec::new();
    for session in sessions {
        let Ok(path) = resolve_transcript_path(&session.id) else {
            continue;
        };
        let Ok(messages) = read_messages_from_transcript(&path, 0, usize::MAX) else {
            continue;
        };
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
    Ok(results)
}

pub fn delete_antigravity_session(session_id: &str) -> Result<(), String> {
    let id = session_id.trim();
    if id.is_empty() {
        return Err("session id is empty".to_string());
    }

    let app_dir = lookup_app_data_dir(id).unwrap_or_else(|| "antigravity-cli".to_string());
    let home = crate::paths::home_dir();
    let base = join_relative(home, &format!(".gemini/{app_dir}"));

    // Conversation files: {id}.db / .pb + sqlite sidecars.
    let conversations = base.join("conversations");
    if conversations.is_dir() {
        if let Ok(entries) = fs::read_dir(&conversations) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name == id
                    || name.starts_with(&format!("{id}."))
                    || name.starts_with(&format!("{id}-"))
                {
                    let path = entry.path();
                    if path.is_dir() {
                        let _ = fs::remove_dir_all(&path);
                    } else {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }

    // Brain tree for this conversation.
    let brain = base.join("brain").join(id);
    if brain.exists() {
        let _ = fs::remove_dir_all(&brain);
    }

    // Drop the index row (best-effort).
    let db_path = summaries_db_path()?;
    if db_path.exists() {
        if let Ok(conn) = Connection::open(&db_path) {
            let _ = conn.execute(
                "DELETE FROM conversation_summaries WHERE conversation_id = ?1",
                [id],
            );
        }
    }

    Ok(())
}

fn summaries_db_path() -> Result<PathBuf, String> {
    Ok(join_relative(
        crate::paths::home_dir(),
        ".gemini/antigravity-cli/conversation_summaries.db",
    ))
}

fn open_readonly(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| format!("unable to open {}: {err}", path.display()))
}

fn lookup_app_data_dir(session_id: &str) -> Option<String> {
    let db_path = summaries_db_path().ok()?;
    if !db_path.exists() {
        return None;
    }
    let conn = open_readonly(&db_path).ok()?;
    conn.query_row(
        "SELECT app_data_dir FROM conversation_summaries WHERE conversation_id = ?1",
        [session_id],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .filter(|value| !value.trim().is_empty())
}

/// Prefer the app_data_dir from the index; fall back to probing both trees.
fn resolve_transcript_path(session_id: &str) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if let Some(app) = lookup_app_data_dir(session_id) {
        candidates.push(app);
    }
    for app in ["antigravity-cli", "antigravity"] {
        if !candidates.iter().any(|existing| existing == app) {
            candidates.push(app.to_string());
        }
    }
    for app in candidates {
        let path = join_relative(
            crate::paths::home_dir(),
            &format!(".gemini/{app}/brain/{session_id}/.system_generated/logs/transcript.jsonl"),
        );
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(format!(
        "Antigravity transcript not found for session {session_id}"
    ))
}

fn read_messages_from_transcript(
    path: &Path,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let file = fs::File::open(path).map_err(|err| err.to_string())?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(message) = parse_transcript_line(&value) {
            messages.push(message);
        }
    }
    Ok(messages.into_iter().skip(offset).take(limit).collect())
}

fn parse_transcript_line(value: &Value) -> Option<SessionMessage> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let role = match kind {
        "USER_INPUT" => "user",
        "PLANNER_RESPONSE" => "assistant",
        _ => return None,
    };
    let raw = value.get("content").and_then(Value::as_str).unwrap_or("");
    let content = if role == "user" {
        unwrap_user_request(raw)
    } else {
        raw.trim().to_string()
    };
    if content.is_empty() {
        return None;
    }
    let timestamp = value
        .get("created_at")
        .and_then(Value::as_str)
        .and_then(parse_iso8601_millis)
        .unwrap_or(0);
    Some(SessionMessage::new(role, content, timestamp))
}

/// Strip `<USER_REQUEST>…</USER_REQUEST>` and drop metadata blocks Antigravity
/// appends after the user text.
pub fn unwrap_user_request(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed
        .strip_prefix("<USER_REQUEST>")
        .and_then(|rest| rest.split("</USER_REQUEST>").next())
    {
        return inner.trim().to_string();
    }
    // Fall back: cut at the first known metadata tag.
    for marker in [
        "<ADDITIONAL_METADATA>",
        "<USER_SETTINGS_CHANGE>",
        "</USER_REQUEST>",
    ] {
        if let Some(idx) = trimmed.find(marker) {
            return trimmed[..idx].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn first_workspace_path(workspace_uris_json: &str) -> Option<String> {
    let trimmed = workspace_uris_json.trim();
    if trimmed.is_empty() {
        return None;
    }
    let value: Value = serde_json::from_str(trimmed).ok()?;
    let uri = value
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_str)?;
    file_uri_to_path(uri)
}

fn file_uri_to_path(uri: &str) -> Option<String> {
    // Shared normalizer strips file://, percent-decodes, and fixes Windows
    // shapes (`/D:/…`, `file:///C:/…`) for session path filters/UI.
    crate::paths::normalize_project_path_display(uri)
}

fn first_nonempty(parts: &[&str]) -> Option<String> {
    parts
        .iter()
        .map(|part| part.trim())
        .find(|part| !part.is_empty())
        .map(ToOwned::to_owned)
}

/// Parse SQLite datetime strings like `2026-07-28 13:42:00+00:00` or ISO-ish
/// values into epoch milliseconds. Invalid / sentinel zeros become None.
fn parse_sqlite_datetime(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with("0001-01-01") {
        return None;
    }
    parse_iso8601_millis(trimmed).or_else(|| {
        // `YYYY-MM-DD HH:MM:SS+00:00` → replace space with T for chrono-less parse.
        let normalized = if trimmed.contains(' ') && !trimmed.contains('T') {
            trimmed.replacen(' ', "T", 1)
        } else {
            trimmed.to_string()
        };
        parse_iso8601_millis(&normalized)
    })
}

fn parse_iso8601_millis(value: &str) -> Option<i64> {
    // Prefer time crate if available; otherwise a minimal parser for the
    // shapes Antigravity writes (`2026-07-28T05:07:16Z` and offset variants).
    let trimmed = value.trim();
    // Use `chrono` is not in deps — implement a tiny RFC3339-ish parser via
    // the standard library isn't available for dates. Rely on `time`? Check
    // Cargo.toml… not listed. Use a pragmatic approach: call `date` parsing
    // via manual components for the common form.
    let (date, rest) = trimmed
        .split_once('T')
        .or_else(|| trimmed.split_once(' '))?;
    let mut date_parts = date.split('-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;

    let rest = rest.trim_end_matches('Z');
    let (time_part, offset_minutes) = parse_time_and_offset(rest)?;
    let mut time_parts = time_part.split(':');
    let hour: u32 = time_parts.next()?.parse().ok()?;
    let minute: u32 = time_parts.next()?.parse().ok()?;
    let second_frac = time_parts.next().unwrap_or("0");
    let second: u32 = second_frac.split('.').next()?.parse().ok()?;

    // Days from civil date to Unix epoch using Howard Hinnant's algorithm.
    let days = days_from_civil(year, month, day)?;
    let secs = i64::from(days) * 86_400
        + i64::from(hour) * 3_600
        + i64::from(minute) * 60
        + i64::from(second)
        - i64::from(offset_minutes) * 60;
    Some(secs * 1000)
}

fn parse_time_and_offset(rest: &str) -> Option<(&str, i32)> {
    // Forms: `05:07:16`, `05:07:16+08:00`, `05:07:16-07:00`, `05:07:16+00:00`
    if let Some(idx) = rest.rfind('+') {
        let (time, off) = rest.split_at(idx);
        return Some((time, parse_offset(off)?));
    }
    // Minus offset: only when there's a `-` after the hour portion (index > 2).
    if let Some(idx) = rest.rfind('-') {
        if idx > 2 {
            let (time, off) = rest.split_at(idx);
            return Some((time, parse_offset(off)?));
        }
    }
    Some((rest, 0))
}

fn parse_offset(offset: &str) -> Option<i32> {
    let sign = if offset.starts_with('-') {
        -1
    } else if offset.starts_with('+') {
        1
    } else {
        return None;
    };
    let body = &offset[1..];
    let mut parts = body.split(':');
    let hours: i32 = parts.next()?.parse().ok()?;
    let minutes: i32 = parts.next().unwrap_or("0").parse().ok()?;
    Some(sign * (hours * 60 + minutes))
}

/// Days since Unix epoch for a Gregorian civil date (Howard Hinnant).
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i32> {
    if !(1..=12).contains(&month) || day == 0 || day > 31 {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp as i32 + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_user_request_strips_tags_and_metadata() {
        let raw = "<USER_REQUEST>\nhello world\n</USER_REQUEST>\n<ADDITIONAL_METADATA>\nx\n</ADDITIONAL_METADATA>";
        assert_eq!(unwrap_user_request(raw), "hello world");
    }

    #[test]
    fn parse_iso_z_and_offset() {
        let z = parse_iso8601_millis("2026-07-28T05:07:16Z").expect("z");
        let offset = parse_iso8601_millis("2026-07-28T13:07:16+08:00").expect("offset");
        assert_eq!(z, offset);
    }

    #[test]
    fn file_uri_decodes_percent() {
        let path = file_uri_to_path(
            "file:///Users/liuyang/Library/Mobile%20Documents/iCloud~md~obsidian/Documents/LIUS",
        )
        .expect("path");
        assert!(path.contains("Mobile Documents"));
    }

    #[test]
    fn file_uri_windows_drive_normalized() {
        assert_eq!(
            file_uri_to_path("file:///D:/Task").as_deref(),
            Some(r"D:\Task")
        );
        assert_eq!(
            file_uri_to_path("file:///c:/Users/x").as_deref(),
            Some(r"C:\Users\x")
        );
    }

    #[test]
    fn real_summaries_smoke() {
        let sessions = list_antigravity_sessions_all().expect("list");
        // Machine may have no Antigravity install — empty is fine.
        if sessions.is_empty() {
            return;
        }
        assert!(!sessions[0].id.is_empty());
    }
}

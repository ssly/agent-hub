use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::DateTime;
use serde_json::Value;

use crate::paths::join_relative;
use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "claude-code";
const METADATA_HEAD_LINES: usize = 100;

#[derive(Clone)]
struct ClaudeSessionCandidate {
    path: PathBuf,
    project_path: String,
    updated_at: i64,
}

pub fn list_claude_sessions_all() -> Result<Vec<SessionSummary>, String> {
    let candidates = collect_claude_session_candidates()?;
    let mut sessions = Vec::new();
    for candidate in candidates {
        if let Ok(session) = extract_session_summary(
            &candidate.path,
            &candidate.project_path,
            Some(candidate.updated_at),
        ) {
            sessions.push(session);
        }
    }
    Ok(sessions)
}

pub fn count_claude_sessions() -> Result<usize, String> {
    let mut count = 0usize;
    for_each_claude_session_file(|_, _| {
        count += 1;
    })?;
    Ok(count)
}

pub fn get_claude_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let path = find_session_file(session_id)
        .ok_or_else(|| format!("Claude session not found: {}", session_id))?;
    read_claude_messages_from_file(&path, offset, limit)
}

pub fn delete_claude_session(session_id: &str) -> Result<(), String> {
    let projects_dir = claude_projects_dir()?;
    let ignored_project_prefixes = claude_ignored_project_prefixes()?;
    delete_claude_session_in_projects_dir(&projects_dir, session_id, &ignored_project_prefixes)
}

fn extract_session_summary(
    path: &Path,
    project_path: &str,
    known_updated_at: Option<i64>,
) -> Result<SessionSummary, String> {
    let session_id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("Invalid Claude session filename: {}", path.display()))?
        .to_string();
    let file = fs::File::open(path).map_err(|err| err.to_string())?;
    let reader = BufReader::new(file);

    let mut title: Option<String> = None;
    let mut first_user_message: Option<String> = None;
    let mut model: Option<String> = None;
    let mut started_at: Option<i64> = None;
    let mut project_path_from_record: Option<String> = None;

    for line in reader.lines().take(METADATA_HEAD_LINES) {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let data: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if started_at.is_none() {
            started_at = data
                .get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(parse_rfc3339_to_ms);
        }
        if project_path_from_record.is_none() {
            project_path_from_record = data
                .get("cwd")
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }

        let record_type = data.get("type").and_then(|value| value.as_str());
        if record_type == Some("custom-title") && title.is_none() {
            title = data
                .get("customTitle")
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }

        if record_type == Some("user") && first_user_message.is_none() {
            first_user_message = data
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(extract_text_content);
        }

        if record_type == Some("assistant") && model.is_none() {
            model = data
                .get("message")
                .and_then(|message| message.get("model"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string());
        }
    }

    let updated_at = known_updated_at.unwrap_or_else(|| {
        fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(system_time_to_ms)
            .unwrap_or(0)
    });
    let started_at = started_at.unwrap_or(updated_at);
    let title = title
        .or_else(|| {
            first_user_message
                .clone()
                .map(|value| truncate_chars(value, 80))
        })
        .unwrap_or_else(|| session_id.clone());
    let final_project_path = project_path_from_record.unwrap_or_else(|| project_path.to_string());

    Ok(SessionSummary {
        id: session_id,
        title,
        project_path: final_project_path,
        model,
        started_at,
        updated_at,
        message_count: None,
        tokens_used: None,
        platform_id: PLATFORM_ID.to_string(),
    })
}

fn read_claude_messages_from_file(
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
        let data: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(message) = parse_claude_message_line(&data) else {
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

fn parse_claude_message_line(data: &Value) -> Option<SessionMessage> {
    let record_type = data.get("type").and_then(|value| value.as_str())?;
    if record_type != "user" && record_type != "assistant" {
        return None;
    }
    let content = data
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(extract_text_content)?;
    let timestamp = data
        .get("timestamp")
        .and_then(|value| value.as_str())
        .and_then(parse_rfc3339_to_ms)
        .unwrap_or(0);
    Some(SessionMessage {
        role: record_type.to_string(),
        content,
        timestamp,
    })
}

/// Streaming scan that keeps only the latest user/assistant message, for the
/// resume preview. Never collects the full transcript into memory.
pub fn last_claude_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let path = find_session_file(session_id)
        .ok_or_else(|| format!("Claude session not found: {}", session_id))?;
    let file = fs::File::open(path).map_err(|err| err.to_string())?;
    let reader = BufReader::new(file);
    let mut last_user = None;
    let mut last_assistant = None;

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let data: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(message) = parse_claude_message_line(&data) else {
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

fn find_session_file(session_id: &str) -> Option<PathBuf> {
    let projects_dir = claude_projects_dir().ok()?;
    let ignored_project_prefixes = claude_ignored_project_prefixes().ok()?;
    find_session_file_in_projects_dir(&projects_dir, &ignored_project_prefixes, session_id)
}

fn find_session_file_in_projects_dir(
    projects_dir: &Path,
    ignored_project_prefixes: &[String],
    session_id: &str,
) -> Option<PathBuf> {
    let mut found = None;
    let _ =
        for_each_claude_session_file_in_dir(projects_dir, ignored_project_prefixes, |path, _| {
            if found.is_none()
                && path.file_stem().and_then(|stem| stem.to_str()) == Some(session_id)
            {
                found = Some(path.to_path_buf());
            }
        });
    found
}

fn claude_projects_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    Ok(join_relative(home, ".claude/projects"))
}

fn collect_claude_session_candidates() -> Result<Vec<ClaudeSessionCandidate>, String> {
    let mut candidates = Vec::new();
    for_each_claude_session_file(|path, project_path| {
        let updated_at = fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(system_time_to_ms)
            .unwrap_or(0);
        candidates.push(ClaudeSessionCandidate {
            path: path.to_path_buf(),
            project_path,
            updated_at,
        });
    })?;
    candidates.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(candidates)
}

fn for_each_claude_session_file<F>(visitor: F) -> Result<(), String>
where
    F: FnMut(&Path, String),
{
    let projects_dir = claude_projects_dir()?;
    let ignored_project_prefixes = claude_ignored_project_prefixes()?;
    for_each_claude_session_file_in_dir(&projects_dir, &ignored_project_prefixes, visitor)
}

fn for_each_claude_session_file_in_dir<F>(
    projects_dir: &Path,
    ignored_project_prefixes: &[String],
    mut visitor: F,
) -> Result<(), String>
where
    F: FnMut(&Path, String),
{
    if !projects_dir.exists() {
        return Ok(());
    }

    let project_entries = fs::read_dir(&projects_dir).map_err(|err| err.to_string())?;
    for project_entry in project_entries {
        let project_entry = match project_entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !project_entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            continue;
        }

        let project_path = project_entry.file_name().to_string_lossy().to_string();
        if is_ignored_project_dir(&project_path, &ignored_project_prefixes) {
            continue;
        }
        let session_entries = match fs::read_dir(project_entry.path()) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for session_entry in session_entries {
            let session_entry = match session_entry {
                Ok(value) => value,
                Err(_) => continue,
            };
            if !session_entry
                .file_type()
                .map(|file_type| file_type.is_file())
                .unwrap_or(false)
            {
                continue;
            }
            let path = session_entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            visitor(&path, project_path.clone());
        }
    }
    Ok(())
}

fn delete_claude_session_in_projects_dir(
    projects_dir: &Path,
    session_id: &str,
    ignored_project_prefixes: &[String],
) -> Result<(), String> {
    let session_path =
        find_session_file_in_projects_dir(projects_dir, ignored_project_prefixes, session_id)
            .ok_or_else(|| format!("Claude session not found: {}", session_id))?;
    fs::remove_file(&session_path)
        .map_err(|err| format!("Failed to delete Claude session {}: {}", session_id, err))?;
    // Claude Code keeps per-session artifacts (e.g. subagent transcripts) in a
    // sibling directory named after the session id; remove it too so nothing
    // orphaned is left behind.
    let session_dir = session_path.with_extension("");
    if session_dir.is_dir() {
        fs::remove_dir_all(&session_dir).map_err(|err| {
            format!(
                "Failed to delete Claude session directory {}: {}",
                session_dir.display(),
                err
            )
        })?;
    }
    Ok(())
}

fn claude_ignored_project_prefixes() -> Result<Vec<String>, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    let claude_mem = home.join(".claude-mem");
    Ok(vec![encode_claude_project_dir_name(
        claude_mem.to_string_lossy().as_ref(),
    )])
}

fn encode_claude_project_dir_name(path: &str) -> String {
    path.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn is_ignored_project_dir(project_dir: &str, ignored_prefixes: &[String]) -> bool {
    ignored_prefixes.iter().any(|prefix| {
        project_dir == prefix
            || project_dir
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('-'))
    })
}

fn parse_rfc3339_to_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.timestamp_millis())
}

fn system_time_to_ms(value: std::time::SystemTime) -> Option<i64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

fn extract_text_content(content: &Value) -> Option<String> {
    match content {
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(text) = item.get("text").and_then(|value| value.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
                } else if let Some(text) = item.get("content").and_then(|value| value.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
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

pub fn search_claude_messages(
    query_lower: &str,
) -> Result<Vec<crate::session::SessionSearchResult>, String> {
    let candidates = collect_claude_session_candidates()?;
    let mut results = Vec::new();
    for candidate in candidates {
        let session = match extract_session_summary(
            &candidate.path,
            &candidate.project_path,
            Some(candidate.updated_at),
        ) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Ok(messages) = read_claude_messages_from_file(&candidate.path, 0, 999999) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn encode_claude_project_dir_name_replaces_non_alphanumeric() {
        let encoded = encode_claude_project_dir_name("/Users/alice/.claude-mem/observer-sessions");
        assert_eq!(encoded, "-Users-alice--claude-mem-observer-sessions");
    }

    #[test]
    fn ignored_project_dir_matches_prefix_and_subpaths() {
        let prefix = encode_claude_project_dir_name("/Users/alice/.claude-mem");
        let ignored = vec![prefix.clone()];

        assert!(is_ignored_project_dir(&prefix, &ignored));
        assert!(is_ignored_project_dir(
            &format!("{}-observer-sessions", prefix),
            &ignored
        ));
        assert!(!is_ignored_project_dir(
            "-Users-alice-Documents-code-agent-hub",
            &ignored
        ));
    }

    #[test]
    fn delete_claude_session_removes_jsonl_file() {
        let dir = tempdir().expect("temp dir should create");
        let project_dir = dir.path().join("project-a");
        fs::create_dir_all(&project_dir).expect("project dir should create");
        let session_path = project_dir.join("session-1.jsonl");
        fs::write(&session_path, "{\"type\":\"user\"}\n").expect("session file should write");

        delete_claude_session_in_projects_dir(dir.path(), "session-1", &[])
            .expect("delete should succeed");

        assert!(!session_path.exists());
    }

    #[test]
    fn delete_claude_session_removes_session_directory() {
        let dir = tempdir().expect("temp dir should create");
        let project_dir = dir.path().join("project-a");
        fs::create_dir_all(&project_dir).expect("project dir should create");
        let session_path = project_dir.join("session-1.jsonl");
        fs::write(&session_path, "{\"type\":\"user\"}\n").expect("session file should write");
        let session_dir = project_dir.join("session-1");
        fs::create_dir_all(session_dir.join("subagents")).expect("subagents dir should create");
        fs::write(session_dir.join("subagents/agent.jsonl"), "{}\n").expect("agent should write");

        delete_claude_session_in_projects_dir(dir.path(), "session-1", &[])
            .expect("delete should succeed");

        assert!(!session_path.exists());
        assert!(!session_dir.exists());
    }

    #[test]
    fn delete_claude_session_returns_not_found() {
        let dir = tempdir().expect("temp dir should create");
        let err = delete_claude_session_in_projects_dir(dir.path(), "missing", &[])
            .expect_err("missing session should fail");
        assert!(err.contains("not found"));
    }
}

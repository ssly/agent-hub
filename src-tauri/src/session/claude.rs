use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::DateTime;
use serde_json::Value;

use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "claude-code";
const METADATA_HEAD_LINES: usize = 100;

#[derive(Clone)]
struct ClaudeSessionCandidate {
    path: PathBuf,
    project_path: String,
    updated_at: i64,
}

pub fn list_claude_sessions(offset: usize, limit: usize) -> Result<(usize, Vec<SessionSummary>), String> {
    let candidates = collect_claude_session_candidates()?;
    let total = candidates.len();
    let page_limit = limit.max(1);
    let mut sessions = Vec::new();
    for candidate in candidates.into_iter().skip(offset).take(page_limit) {
        if let Ok(session) = extract_session_summary(&candidate.path, &candidate.project_path, Some(candidate.updated_at)) {
            sessions.push(session);
        }
    }
    Ok((total, sessions))
}

pub fn count_claude_sessions() -> Result<usize, String> {
    let mut count = 0usize;
    for_each_claude_session_file(|_, _| {
        count += 1;
    })?;
    Ok(count)
}

pub fn get_claude_messages(session_id: &str, offset: usize, limit: usize) -> Result<Vec<SessionMessage>, String> {
    let path = find_session_file(session_id)
        .ok_or_else(|| format!("Claude session not found: {}", session_id))?;
    read_claude_messages_from_file(&path, offset, limit)
}

fn extract_session_summary(path: &Path, project_path: &str, known_updated_at: Option<i64>) -> Result<SessionSummary, String> {
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
        .or_else(|| first_user_message.clone().map(|value| truncate_chars(value, 80)))
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

fn read_claude_messages_from_file(path: &Path, offset: usize, limit: usize) -> Result<Vec<SessionMessage>, String> {
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
        let Some(record_type) = data.get("type").and_then(|value| value.as_str()) else {
            continue;
        };
        if record_type != "user" && record_type != "assistant" {
            continue;
        }
        let Some(content) = data
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(extract_text_content) else {
            continue;
        };

        if matched >= offset {
            let timestamp = data
                .get("timestamp")
                .and_then(|value| value.as_str())
                .and_then(parse_rfc3339_to_ms)
                .unwrap_or(0);
            messages.push(SessionMessage {
                role: record_type.to_string(),
                content,
                timestamp,
            });
            if messages.len() >= page_limit {
                break;
            }
        }
        matched += 1;
    }

    Ok(messages)
}

fn find_session_file(session_id: &str) -> Option<PathBuf> {
    let mut found = None;
    let _ = for_each_claude_session_file(|path, _| {
        if found.is_none() && path.file_stem().and_then(|stem| stem.to_str()) == Some(session_id) {
            found = Some(path.to_path_buf());
        }
    });
    found
}

fn claude_projects_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    Ok(home.join(".claude/projects"))
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

fn for_each_claude_session_file<F>(mut visitor: F) -> Result<(), String>
where
    F: FnMut(&Path, String),
{
    let projects_dir = claude_projects_dir()?;
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

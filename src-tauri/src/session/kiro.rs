use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "kiro";

pub fn count_kiro_sessions() -> Result<usize, String> {
    let sessions = collect_kiro_sessions()?;
    Ok(sessions.len())
}

pub fn list_kiro_sessions_all() -> Result<Vec<SessionSummary>, String> {
    collect_kiro_sessions()
}

pub fn get_kiro_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let path = find_kiro_session_jsonl(session_id)?;
    read_kiro_messages_from_jsonl(&path, offset, limit)
}

pub fn delete_kiro_session(session_id: &str) -> Result<(), String> {
    let root = kiro_sessions_dir()?;
    delete_kiro_session_in_dir(&root, session_id)
}

fn collect_kiro_sessions() -> Result<Vec<SessionSummary>, String> {
    let root = kiro_sessions_dir()?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    let entries = fs::read_dir(&root).map_err(|err| err.to_string())?;
    for entry in entries {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };

        if !entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            continue;
        }

        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let fallback_id = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        if fallback_id.is_empty() {
            continue;
        }

        let fallback_updated_at = fs::metadata(&path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(system_time_to_ms)
            .unwrap_or(0);

        let content = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&content) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if let Some(summary) = parse_kiro_summary(&value, &fallback_id, fallback_updated_at) {
            sessions.push(summary);
        }
    }

    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(sessions)
}

fn parse_kiro_summary(
    value: &Value,
    fallback_id: &str,
    fallback_updated_at: i64,
) -> Option<SessionSummary> {
    let id = value
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| fallback_id.to_string());

    if id.is_empty() {
        return None;
    }

    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| id.clone());

    let project_path = value
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .unwrap_or_default();

    let model = value
        .get("session_state")
        .and_then(|v| v.get("rts_model_state"))
        .and_then(|v| v.get("model_info"))
        .and_then(|v| v.get("model_id"))
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let started_at = value
        .get("created_at")
        .and_then(|v| v.as_str())
        .and_then(parse_rfc3339_to_ms)
        .unwrap_or(fallback_updated_at);

    let updated_at = value
        .get("updated_at")
        .and_then(|v| v.as_str())
        .and_then(parse_rfc3339_to_ms)
        .unwrap_or(fallback_updated_at);

    Some(SessionSummary {
        id,
        title,
        project_path,
        model,
        started_at,
        updated_at,
        message_count: None,
        tokens_used: None,
        platform_id: PLATFORM_ID.to_string(),
    })
}

fn find_kiro_session_jsonl(session_id: &str) -> Result<PathBuf, String> {
    let root = kiro_sessions_dir()?;
    if !root.exists() {
        return Err("Kiro session directory not found: ~/.kiro/sessions/cli".to_string());
    }

    let direct = root.join(format!("{}.jsonl", session_id));
    if direct.exists() {
        return Ok(direct);
    }

    // Fallback search for possible future naming differences.
    let entries = fs::read_dir(&root).map_err(|err| err.to_string())?;
    for entry in entries {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if stem == session_id {
            return Ok(path);
        }
    }

    Err(format!(
        "Kiro session jsonl not found for id: {}",
        session_id
    ))
}

fn delete_kiro_session_in_dir(root: &Path, session_id: &str) -> Result<(), String> {
    if !root.exists() {
        return Err("Kiro session directory not found: ~/.kiro/sessions/cli".to_string());
    }

    let artifacts = collect_kiro_session_artifacts(root, session_id)?;
    if artifacts.is_empty() {
        return Err(format!("Kiro session not found: {}", session_id));
    }

    for path in artifacts {
        if !path.exists() {
            continue;
        }
        fs::remove_file(&path).map_err(|err| {
            format!(
                "Failed to delete Kiro session artifact {}: {}",
                path.display(),
                err
            )
        })?;
    }
    Ok(())
}

fn collect_kiro_session_artifacts(root: &Path, session_id: &str) -> Result<Vec<PathBuf>, String> {
    let mut artifacts = Vec::new();
    let direct_json = root.join(format!("{}.json", session_id));
    if direct_json.exists() {
        artifacts.push(direct_json);
    }
    let direct_jsonl = root.join(format!("{}.jsonl", session_id));
    if direct_jsonl.exists() {
        artifacts.push(direct_jsonl);
    }

    let entries = fs::read_dir(root).map_err(|err| err.to_string())?;
    for entry in entries {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let fallback_id = match path.file_stem().and_then(|name| name.to_str()) {
            Some(value) if !value.is_empty() => value.to_string(),
            _ => continue,
        };
        let content = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&content) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(summary) = parse_kiro_summary(&value, &fallback_id, 0) else {
            continue;
        };
        if summary.id != session_id {
            continue;
        }
        artifacts.push(path.clone());
        artifacts.push(root.join(format!("{}.jsonl", fallback_id)));
        artifacts.push(root.join(format!("{}.jsonl", summary.id)));
    }

    artifacts.sort();
    artifacts.dedup();
    Ok(artifacts)
}

fn read_kiro_messages_from_jsonl(
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
        let Some(message) = parse_kiro_message_line(&value) else {
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

fn parse_kiro_message_line(value: &Value) -> Option<SessionMessage> {
    let kind = value.get("kind").and_then(|v| v.as_str())?;
    let role = match kind {
        "Prompt" => "user",
        "AssistantMessage" => "assistant",
        _ => return None,
    };

    let content = extract_kiro_text_content(value.get("data")?.get("content")?)?;

    let raw_ts = value
        .get("data")
        .and_then(|v| v.get("meta"))
        .and_then(|v| v.get("timestamp"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let timestamp = normalize_epoch_millis(raw_ts);

    Some(SessionMessage {
        role: role.to_string(),
        content,
        timestamp,
    })
}

fn extract_kiro_text_content(content: &Value) -> Option<String> {
    let Value::Array(items) = content else {
        return None;
    };

    let mut parts = Vec::new();
    for item in items {
        if item.get("kind").and_then(|v| v.as_str()) != Some("text") {
            continue;
        }
        let Some(text) = item.get("data").and_then(|v| v.as_str()) else {
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

fn normalize_epoch_millis(value: i64) -> i64 {
    if value <= 0 {
        return 0;
    }
    if value < 100_000_000_000 {
        value.saturating_mul(1000)
    } else {
        value
    }
}

fn parse_rfc3339_to_ms(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

fn system_time_to_ms(time: std::time::SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

fn kiro_sessions_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to resolve HOME directory".to_string())?;
    Ok(home.join(".kiro/sessions/cli"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn parse_kiro_summary_maps_required_fields() {
        let value = json!({
            "session_id": "abc-123",
            "cwd": "/tmp/work",
            "created_at": "2026-05-02T00:08:27.428216Z",
            "updated_at": "2026-05-02T00:08:45.134022Z",
            "title": "test-title",
            "session_state": {
                "rts_model_state": {
                    "model_info": {
                        "model_id": "auto"
                    }
                }
            }
        });

        let summary = parse_kiro_summary(&value, "fallback", 0).expect("summary should parse");
        assert_eq!(summary.id, "abc-123");
        assert_eq!(summary.title, "test-title");
        assert_eq!(summary.project_path, "/tmp/work");
        assert_eq!(summary.model.as_deref(), Some("auto"));
        assert_eq!(summary.platform_id, "kiro");
        assert!(summary.started_at > 0);
        assert!(summary.updated_at > 0);
    }

    #[test]
    fn parse_kiro_summary_fallbacks_when_fields_missing() {
        let value = json!({
            "session_id": "",
            "title": ""
        });

        let summary = parse_kiro_summary(&value, "fallback-id", 123).expect("summary should parse");
        assert_eq!(summary.id, "fallback-id");
        assert_eq!(summary.title, "fallback-id");
        assert_eq!(summary.started_at, 123);
        assert_eq!(summary.updated_at, 123);
    }

    #[test]
    fn parse_kiro_message_line_maps_prompt_and_assistant() {
        let prompt = json!({
            "kind": "Prompt",
            "data": {
                "meta": {"timestamp": 1777680512},
                "content": [{"kind": "text", "data": "hello"}]
            }
        });
        let assistant = json!({
            "kind": "AssistantMessage",
            "data": {
                "content": [{"kind": "text", "data": "world"}]
            }
        });

        let user_msg = parse_kiro_message_line(&prompt).expect("prompt should parse");
        assert_eq!(user_msg.role, "user");
        assert_eq!(user_msg.content, "hello");
        assert_eq!(user_msg.timestamp, 1_777_680_512_000);

        let assistant_msg = parse_kiro_message_line(&assistant).expect("assistant should parse");
        assert_eq!(assistant_msg.role, "assistant");
        assert_eq!(assistant_msg.content, "world");
        assert_eq!(assistant_msg.timestamp, 0);
    }

    #[test]
    fn read_kiro_messages_respects_offset_limit() {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "agent-hub-kiro-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        );
        path.push(unique);

        let mut file = fs::File::create(&path).expect("temp file should create");
        writeln!(
            file,
            "{}",
            json!({"kind":"Prompt","data":{"content":[{"kind":"text","data":"u1"}]}})
        )
        .expect("line should write");
        writeln!(
            file,
            "{}",
            json!({"kind":"AssistantMessage","data":{"content":[{"kind":"text","data":"a1"}]}})
        )
        .expect("line should write");
        writeln!(
            file,
            "{}",
            json!({"kind":"Prompt","data":{"content":[{"kind":"text","data":"u2"}]}})
        )
        .expect("line should write");
        file.flush().expect("flush should succeed");

        let page = read_kiro_messages_from_jsonl(&path, 1, 2).expect("page should load");
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].content, "a1");
        assert_eq!(page[1].content, "u2");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn delete_kiro_session_removes_metadata_and_transcript() {
        let dir = tempfile::tempdir().expect("temp dir should create");
        let metadata_path = dir.path().join("meta-id.json");
        let transcript_path = dir.path().join("meta-id.jsonl");
        fs::write(
            &metadata_path,
            json!({
                "session_id": "session-1",
                "title": "test"
            })
            .to_string(),
        )
        .expect("metadata should write");
        fs::write(&transcript_path, "{}\n").expect("transcript should write");

        delete_kiro_session_in_dir(dir.path(), "session-1").expect("delete should succeed");

        assert!(!metadata_path.exists());
        assert!(!transcript_path.exists());
    }

    #[test]
    fn delete_kiro_session_returns_not_found() {
        let dir = tempfile::tempdir().expect("temp dir should create");
        let err = delete_kiro_session_in_dir(dir.path(), "missing")
            .expect_err("missing session should fail");
        assert!(err.contains("not found"));
    }
}

//! Kiro CLI session monitoring via plain file watching.
//!
//! Stable kiro-cli 2.x does not load global hook files (`~/.kiro/hooks/` is an
//! IDE 1.0.182+ / CLI 3.0 feature), so the file watcher is the base layer
//! that works on every version. If the global hook does fire (IDE / v3), its
//! events merge into the same session rows — both channels feed the shared
//! aggregation pipeline.
//!
//! The watcher observes `~/.kiro/sessions/cli/`:
//!
//! - `<uuid>.jsonl` is an append-only event stream (`Prompt` /
//!   `AssistantMessage` lines with second-level timestamps); new lines are
//!   tailed with per-file offsets and turned into monitor events.
//! - `<uuid>.json` metadata provides the session cwd.
//! - `<uuid>.lock` holds the live CLI pid; the running/ended status is
//!   derived from it lazily in `service.rs` (no polling here).
//!
//! The on-disk layout is not officially documented, so every parse failure
//! is logged and skipped — a format change degrades monitoring silently but
//! must never break the app.

use super::types::{AgentKind, HookEvent, SessionSource};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;
use uuid::Uuid;

/// Only sessions with activity inside this window are picked up by the
/// initial scan; older transcripts are left to the session browser.
const INITIAL_SCAN_MAX_AGE_MILLIS: i64 = 24 * 60 * 60 * 1000;

type Offsets = Arc<Mutex<HashMap<PathBuf, u64>>>;

pub struct KiroWatcher {
    _watcher: RecommendedWatcher,
}

impl KiroWatcher {
    /// Returns None when the sessions directory does not exist (Kiro CLI not
    /// installed / never used) or the watcher cannot be created.
    pub fn new(
        sessions_dir: PathBuf,
        on_event: impl Fn(HookEvent) + Send + Sync + 'static,
    ) -> Option<Self> {
        if !sessions_dir.is_dir() {
            return None;
        }

        let offsets: Offsets = Arc::new(Mutex::new(HashMap::new()));
        initial_scan(&sessions_dir, &offsets, &on_event);

        let watch_dir = sessions_dir.clone();
        let watch_offsets = offsets.clone();
        let mut watcher = match RecommendedWatcher::new(
            move |result: Result<notify::Event, notify::Error>| {
                let Ok(event) = result else {
                    return;
                };
                if !event.kind.is_create() && !event.kind.is_modify() {
                    return;
                }
                for path in event.paths {
                    if path.extension().and_then(|extension| extension.to_str()) == Some("jsonl") {
                        process_jsonl(&watch_dir, &path, &watch_offsets, &on_event);
                    }
                }
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => watcher,
            Err(error) => {
                log::warn!("Unable to create Kiro session watcher: {error}");
                return None;
            }
        };

        if let Err(error) = watcher.watch(&sessions_dir, RecursiveMode::NonRecursive) {
            log::warn!(
                "Unable to watch Kiro sessions directory {}: {error}",
                sessions_dir.display()
            );
            return None;
        }
        Some(Self {
            _watcher: watcher,
        })
    }
}

/// Seed the monitor from recently active transcripts and record their end
/// offsets so only new lines are processed from now on.
fn initial_scan(sessions_dir: &Path, offsets: &Offsets, on_event: &dyn Fn(HookEvent)) {
    let Ok(entries) = fs::read_dir(sessions_dir) else {
        return;
    };
    let cutoff = now_millis() - INITIAL_SCAN_MAX_AGE_MILLIS;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(system_time_to_millis)
            .unwrap_or(0);
        if modified < cutoff {
            continue;
        }
        let Ok(mut file) = fs::File::open(&path) else {
            continue;
        };
        let mut last_prompt: Option<(String, i64)> = None;
        let mut last_reply: Option<(String, i64)> = None;
        let reader = BufReader::new(&mut file);
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            match parse_stream_line(&line) {
                Some(StreamEvent::Prompt { text, occurred_at }) => {
                    last_prompt = Some((text, occurred_at));
                }
                Some(StreamEvent::Reply { text, occurred_at }) => {
                    last_reply = Some((text, occurred_at));
                }
                None => {}
            }
        }
        let end_offset = file.stream_position().unwrap_or(0);
        offsets
            .lock()
            .map(|mut map| map.insert(path.clone(), end_offset))
            .ok();

        let Some(session_id) = session_id_for(&path) else {
            continue;
        };
        let cwd = read_session_cwd(sessions_dir, &session_id);
        if let Some((text, occurred_at)) = last_prompt {
            on_event(build_event(
                &session_id,
                "UserPromptSubmit",
                Some(text),
                None,
                cwd.clone(),
                occurred_at,
            ));
        }
        if let Some((text, occurred_at)) = last_reply {
            on_event(build_event(
                &session_id,
                "Stop",
                None,
                Some(text),
                cwd,
                occurred_at,
            ));
        }
    }
}

/// Append-only tail: read lines added since the recorded offset. A trailing
/// partial line (writer still flushing) is left for the next event.
fn process_jsonl(
    sessions_dir: &Path,
    path: &Path,
    offsets: &Offsets,
    on_event: &dyn Fn(HookEvent),
) {
    let Some(session_id) = session_id_for(path) else {
        return;
    };
    let start = offsets
        .lock()
        .ok()
        .and_then(|map| map.get(path).copied())
        .unwrap_or(0);
    let Ok(mut file) = fs::File::open(path) else {
        return;
    };
    let file_len = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    // The file was truncated or rotated — restart from the beginning.
    let start = if file_len < start { 0 } else { start };
    if file.seek(SeekFrom::Start(start)).is_err() {
        return;
    }

    let cwd = read_session_cwd(sessions_dir, &session_id);
    let mut position = start;
    let mut reader = BufReader::new(file);
    loop {
        let mut line = String::new();
        let read = match reader.read_line(&mut line) {
            Ok(read) => read,
            Err(_) => break,
        };
        if read == 0 || !line.ends_with('\n') {
            break;
        }
        position += read as u64;
        match parse_stream_line(&line) {
            Some(StreamEvent::Prompt { text, occurred_at }) => on_event(build_event(
                &session_id,
                "UserPromptSubmit",
                Some(text),
                None,
                cwd.clone(),
                occurred_at,
            )),
            Some(StreamEvent::Reply { text, occurred_at }) => on_event(build_event(
                &session_id,
                "Stop",
                None,
                Some(text),
                cwd.clone(),
                occurred_at,
            )),
            None => {}
        }
    }
    offsets
        .lock()
        .map(|mut map| map.insert(path.to_path_buf(), position))
        .ok();
}

enum StreamEvent {
    Prompt { text: String, occurred_at: i64 },
    Reply { text: String, occurred_at: i64 },
}

fn parse_stream_line(line: &str) -> Option<StreamEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let kind = value.get("kind").and_then(|v| v.as_str())?;
    let text = extract_text_content(value.get("data")?.get("content")?)?;
    let occurred_at = value
        .get("data")
        .and_then(|v| v.get("meta"))
        .and_then(|v| v.get("timestamp"))
        .and_then(|v| v.as_i64())
        .map(normalize_epoch_millis)
        .filter(|timestamp| *timestamp > 0)
        .unwrap_or_else(now_millis);
    match kind {
        "Prompt" => Some(StreamEvent::Prompt {
            text,
            occurred_at,
        }),
        "AssistantMessage" => Some(StreamEvent::Reply {
            text,
            occurred_at,
        }),
        _ => None,
    }
}

fn extract_text_content(content: &serde_json::Value) -> Option<String> {
    let serde_json::Value::Array(items) = content else {
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

fn build_event(
    session_id: &str,
    hook_event_name: &str,
    user_prompt: Option<String>,
    assistant_reply: Option<String>,
    cwd: Option<String>,
    occurred_at: i64,
) -> HookEvent {
    let event_id = Uuid::new_v4().to_string();
    HookEvent {
        turn_id: event_id.clone(),
        event_id,
        agent: AgentKind::Kiro,
        hook_event_name: hook_event_name.to_string(),
        session_id: session_id.to_string(),
        source: SessionSource::Terminal,
        cwd,
        user_prompt,
        assistant_reply,
        occurred_at,
    }
}

fn session_id_for(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .map(ToOwned::to_owned)
}

fn read_session_cwd(sessions_dir: &Path, session_id: &str) -> Option<String> {
    let metadata = sessions_dir.join(format!("{session_id}.json"));
    let content = fs::read_to_string(metadata).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|cwd| !cwd.is_empty())
        .map(ToOwned::to_owned)
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

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

fn system_time_to_millis(time: std::time::SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs::OpenOptions;
    use std::io::Write;

    #[test]
    fn parses_prompt_and_reply_lines() {
        let prompt = json!({
            "kind": "Prompt",
            "data": {
                "meta": {"timestamp": 1777680512},
                "content": [{"kind": "text", "data": "hello"}]
            }
        })
        .to_string();
        let Some(StreamEvent::Prompt { text, occurred_at }) = parse_stream_line(&prompt) else {
            panic!("prompt line should parse");
        };
        assert_eq!(text, "hello");
        assert_eq!(occurred_at, 1_777_680_512_000);

        let reply = json!({
            "kind": "AssistantMessage",
            "data": {"content": [{"kind": "text", "data": "world"}]}
        })
        .to_string();
        let Some(StreamEvent::Reply { text, .. }) = parse_stream_line(&reply) else {
            panic!("reply line should parse");
        };
        assert_eq!(text, "world");

        // Unrelated record kinds and malformed JSON are ignored.
        assert!(parse_stream_line("{\"kind\":\"ToolResults\",\"data\":{}}").is_none());
        assert!(parse_stream_line("not json").is_none());
    }

    #[test]
    fn process_jsonl_tails_incrementally_and_keeps_partial_line() {
        let directory = tempfile::tempdir().expect("temp dir should create");
        let path = directory.path().join("session-1.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"kind\":\"Prompt\",\"data\":{\"content\":[{\"kind\":\"text\",\"data\":\"u1\"}]}}\n",
                "{\"kind\":\"AssistantMessage\",\"data\":{\"content\":[{\"kind\":\"text\",\"data\":\"a1\"}]}}\n"
            ),
        )
        .expect("initial content should write");

        let offsets: Offsets = Arc::new(Mutex::new(HashMap::new()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let collected = events.clone();
        let sink = move |event: HookEvent| collected.lock().unwrap().push(event);

        process_jsonl(directory.path(), &path, &offsets, &sink);
        assert_eq!(events.lock().unwrap().len(), 2);

        // No new complete lines — a second pass emits nothing.
        process_jsonl(directory.path(), &path, &offsets, &sink);
        assert_eq!(events.lock().unwrap().len(), 2);

        // Append a partial line (no newline): nothing emitted, offset kept.
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        write!(
            file,
            "{{\"kind\":\"Prompt\",\"data\":{{\"content\":[{{\"kind\":\"text\",\"data\":\"u2\"}}]}}}}"
        )
        .unwrap();
        file.flush().unwrap();
        process_jsonl(directory.path(), &path, &offsets, &sink);
        assert_eq!(events.lock().unwrap().len(), 2);

        // Completing the line makes it visible on the next pass.
        writeln!(file).unwrap();
        file.flush().unwrap();
        process_jsonl(directory.path(), &path, &offsets, &sink);
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[2].session_id, "session-1");
        assert_eq!(events[2].agent, AgentKind::Kiro);
        assert_eq!(events[2].user_prompt.as_deref(), Some("u2"));
    }
}

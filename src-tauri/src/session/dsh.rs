//! DeepSeek Harness (dsh CLI) session adapter.
//!
//! Session storage (verified against @deepseek-ai/dsh 0.1.0-rc.6):
//! - `~/.dsh/sessions/<projectKey>/<session-id>/session.jsonl.zstd`
//!   (plain `.jsonl` when compression is disabled). The artifact is a
//!   concatenated-zstd-frame container: each durable append batch is one
//!   frame, so the file grows frame by frame and a crashed write only ever
//!   leaves a torn final frame.
//! - Line 1 is the immutable header: `{"type":"session","version":0,
//!   "id":…,"createdAt":<ms>,"cwd":…,"origin":"subagent"?,…}`.
//! - Every following line is one storage record: either a session event
//!   envelope `{type, seq, time, data}` or a packed chunk row
//!   (`text-chunks` / `reasoning-chunks` / `tool-call-chunks`) that only
//!   stores raw stream deltas — assembled text always arrives via
//!   `assistant/message`, so chunk rows are skipped here.
//! - Transcript-worthy messages are `user/message` (data.source.kind ==
//!   "user" only — plugin injections / goal rounds carry other kinds) and
//!   `assistant/message`. Compaction replacement copies carry
//!   `surfaceOp: {op:"replace"}` and must not be re-shown (their originals
//!   stay in the log).
//!
//! The session index used for listing lives in
//! `~/.dsh/storages/session_projcache.json` (`tables.sessions.<id>.rows.*`:
//! title, sessionStats, tokenUsage, sessionListMetadata) and
//! `~/.dsh/storages/workspace.json` (workspace → sessionIds). Titles and
//! stats come from projcache; the log header supplies cwd/createdAt.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session::models::{SessionMessage, SessionSummary};

const PLATFORM_ID: &str = "dsh";
const DS_HOME_ENV: &str = "DSH_HOME";
const LOG_ZSTD: &str = "session.jsonl.zstd";
const LOG_PLAIN: &str = "session.jsonl";
/// Chunk rows only carry raw stream deltas; skipping them is safe.
const CHUNK_ROW_TYPES: [&str; 3] = ["text-chunks", "reasoning-chunks", "tool-call-chunks"];

/// Decoded session artifact: immutable header + the event stream.
#[derive(Debug, Clone, Default)]
pub struct DshLog {
    pub header: DshHeader,
    pub events: Vec<DshEvent>,
}

#[derive(Debug, Clone, Default)]
pub struct DshHeader {
    pub id: String,
    pub created_at: i64,
    pub cwd: Option<String>,
    /// Subagent child sessions (`origin: "subagent"`) are presentation
    /// metadata for the harness itself; Agent Hub skips them in both the
    /// browser and the monitor to avoid phantom rows.
    pub is_subagent: bool,
}

#[derive(Debug, Clone)]
pub struct DshEvent {
    pub event_type: String,
    pub time: i64,
    pub data: Value,
}

/// Harness home: `$DSH_HOME` when set, else `~/.dsh`.
pub fn dsh_home() -> Option<PathBuf> {
    if let Ok(home) = std::env::var(DS_HOME_ENV) {
        if !home.trim().is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    Some(crate::paths::home_dir().join(".dsh"))
}

pub fn dsh_sessions_root() -> Option<PathBuf> {
    dsh_home().map(|home| home.join("sessions"))
}

fn dsh_projcache_path() -> Option<PathBuf> {
    dsh_home().map(|home| home.join("storages").join("session_projcache.json"))
}

fn dsh_workspace_path() -> Option<PathBuf> {
    dsh_home().map(|home| home.join("storages").join("workspace.json"))
}

/// One session artifact found on disk: log path + the session id encoded in
/// the directory name (ids are `session-<uuid>`; the directory name is the
/// path-escaped id, safe to use as-is for known ids).
#[derive(Debug, Clone)]
pub struct DshSessionFile {
    pub log_path: PathBuf,
    pub session_id: String,
}

/// Walk `~/.dsh/sessions/*/*/` and collect every session log.
pub fn list_dsh_session_files() -> Vec<DshSessionFile> {
    let Some(root) = dsh_sessions_root() else {
        return Vec::new();
    };
    let Ok(project_entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for project_entry in project_entries.flatten() {
        let project_dir = project_entry.path();
        if !project_dir.is_dir() {
            continue;
        }
        let Ok(session_entries) = fs::read_dir(&project_dir) else {
            continue;
        };
        for session_entry in session_entries.flatten() {
            let session_dir = session_entry.path();
            if !session_dir.is_dir() {
                continue;
            }
            let zstd = session_dir.join(LOG_ZSTD);
            let plain = session_dir.join(LOG_PLAIN);
            let log_path = if zstd.exists() {
                zstd
            } else if plain.exists() {
                plain
            } else {
                continue;
            };
            let session_id = session_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            files.push(DshSessionFile {
                log_path,
                session_id,
            });
        }
    }
    files
}

pub fn find_dsh_session_file(session_id: &str) -> Option<PathBuf> {
    list_dsh_session_files()
        .into_iter()
        .find(|file| file.session_id == session_id)
        .map(|file| file.log_path)
}

/// Decode one session artifact. `session.jsonl.zstd` is a concatenated-frame
/// zstd container: every durable append batch is its own frame, so the file
/// is a sequence of independent frames. ruzstd's `StreamingDecoder` only
/// handles a single frame, so we decode frame by frame, walking the frame
/// boundaries ourselves. A torn final frame (crashed writer) stops the walk
/// and keeps the decodable prefix — the same recovery semantics DSH's own
/// scanner applies.
pub fn decode_dsh_log(path: &Path) -> Result<DshLog, String> {
    let raw = fs::read(path).map_err(|e| format!("无法读取会话文件 {}: {e}", path.display()))?;
    let text = if path.extension().and_then(|e| e.to_str()) == Some("zstd") {
        let out = zstd_decompress_frames(&raw)?;
        String::from_utf8(out).map_err(|e| format!("会话日志不是 UTF-8: {e}"))?
    } else {
        String::from_utf8(raw).map_err(|e| format!("会话日志不是 UTF-8: {e}"))?
    };

    let mut log = DshLog::default();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parsed: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(record_type) = parsed.get("type").and_then(|v| v.as_str()) else {
            continue;
        };
        if index == 0 && record_type == "session" {
            log.header = DshHeader {
                id: parsed
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                created_at: parsed
                    .get("createdAt")
                    .and_then(|v| v.as_i64())
                    .unwrap_or_default(),
                cwd: parsed
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty()),
                is_subagent: parsed
                    .get("origin")
                    .and_then(|v| v.as_str())
                    .is_some_and(|origin| origin == "subagent"),
            };
            continue;
        }
        if CHUNK_ROW_TYPES.contains(&record_type) {
            continue;
        }
        log.events.push(DshEvent {
            event_type: record_type.to_string(),
            time: parsed.get("time").and_then(|v| v.as_i64()).unwrap_or_default(),
            data: parsed.get("data").cloned().unwrap_or(Value::Null),
        });
    }
    Ok(log)
}

const ZSTD_FRAME_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// Decode every complete zstd frame in `raw`, in order. `raw` must start at
/// a frame boundary (the container's first magic). Stops at the first
/// incomplete/unsupported frame — the tail a crashed writer left behind —
/// and returns everything decoded before it.
fn zstd_decompress_frames(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(raw.len().saturating_mul(8).min(128 << 20));
    let mut pos = 0usize;
    while pos < raw.len() {
        if pos + ZSTD_FRAME_MAGIC.len() > raw.len() || raw[pos..pos + ZSTD_FRAME_MAGIC.len()] != ZSTD_FRAME_MAGIC {
            break;
        }
        let mut cursor = std::io::Cursor::new(&raw[pos..]);
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(&mut cursor)
            .map_err(|e| format!("zstd 解码器初始化失败: {e}"))?;
        match decoder.read_to_end(&mut out) {
            Ok(_) => {
                let consumed = cursor.position() as usize;
                if consumed == 0 {
                    break;
                }
                pos += consumed;
            }
            Err(_) => {
                // Torn final frame (or a frame we cannot decode): stop here
                // and keep the prefix — the next poll/read repairs naturally
                // once DSH extends or truncates the file.
                break;
            }
        }
    }
    Ok(out)
}

/// Load the projcache index once per call (small file, parsed on demand).
pub fn load_dsh_projcache() -> Value {
    dsh_projcache_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}

fn projcache_session<'a>(cache: &'a Value, session_id: &str) -> Option<&'a Value> {
    cache
        .get("tables")?
        .get("sessions")?
        .get(session_id)
}

fn projcache_row<'a>(session: &'a Value, row: &str) -> Option<&'a Value> {
    session.get("rows")?.get(row)?.get("val")
}

fn system_time_to_ms(time: std::time::SystemTime) -> Option<i64> {
    time.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
}

// ---------------------------------------------------------------------------
// Session browser contract
// ---------------------------------------------------------------------------

pub fn count_dsh_sessions() -> Result<usize, String> {
    Ok(list_dsh_session_files().len())
}

pub fn list_dsh_sessions_all() -> Result<Vec<SessionSummary>, String> {
    let cache = load_dsh_projcache();
    let files = list_dsh_session_files();
    let mut sessions = Vec::with_capacity(files.len());
    for file in files {
        let Ok(log) = decode_dsh_log(&file.log_path) else {
            continue;
        };
        if log.header.is_subagent {
            continue;
        }
        let pc = projcache_session(&cache, &file.session_id);
        let title = pc
            .and_then(|s| projcache_row(s, "title"))
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                first_user_text(&log)
                    .map(|text| truncate_chars(&text, 80))
                    .unwrap_or_else(|| file.session_id.clone())
            });
        let updated_at = pc
            .and_then(|s| projcache_row(s, "sessionListMetadata"))
            .and_then(|v| v.get("lastPromptAt"))
            .and_then(|v| v.as_i64())
            .or_else(|| {
                fs::metadata(&file.log_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(system_time_to_ms)
            })
            .unwrap_or_default();
        let message_count = pc
            .and_then(|s| projcache_row(s, "sessionStats"))
            .and_then(|v| v.get("turns"))
            .and_then(|v| v.as_u64())
            .map(|turns| turns.min(u32::MAX as u64) as u32);
        let tokens_used = pc
            .and_then(|s| projcache_row(s, "tokenUsage"))
            .and_then(|v| v.get("totals"))
            .and_then(|totals| {
                let sum = |key: &str| totals.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
                Some(
                    sum("uncachedInputTokens")
                        + sum("cacheReadTokens")
                        + sum("cacheWriteTokens")
                        + sum("outputTokens"),
                )
            })
            .filter(|total| *total > 0);

        let project_path = log.header.cwd.clone().unwrap_or_default();
        let model = first_assistant_model(&log);
        let started_at = log.header.created_at;
        // Prefer the header's own id (path-escaped ids round-trip for known
        // `session-<uuid>` names); fall back to the directory name.
        let session_id = if log.header.id.is_empty() {
            file.session_id.clone()
        } else {
            log.header.id.clone()
        };

        sessions.push(SessionSummary {
            id: session_id,
            title,
            project_path,
            model,
            started_at,
            updated_at,
            message_count,
            tokens_used,
            platform_id: PLATFORM_ID.to_string(),
            source: None,
        });
    }
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}

/// Events folded into transcript messages: user prompts (real user source
/// only) and assistant replies. Replace-copy events (compaction) and packed
/// chunk rows never enter the transcript.
pub fn collect_dsh_messages(log: &DshLog) -> Vec<SessionMessage> {
    let mut messages = Vec::new();
    for event in &log.events {
        if let Some(surface_op) = event.data.get("surfaceOp") {
            if surface_op.get("op").and_then(|v| v.as_str()) == Some("replace") {
                continue;
            }
        }
        match event.event_type.as_str() {
            "user/message" => {
                let is_real_user = event
                    .data
                    .get("source")
                    .and_then(|s| s.get("kind"))
                    .and_then(|v| v.as_str())
                    == Some("user");
                if !is_real_user {
                    continue;
                }
                let text = extract_text_blocks(event.data.get("content"));
                if text.trim().is_empty() {
                    continue;
                }
                messages.push(SessionMessage::new("user", text, event.time));
            }
            "assistant/message" => {
                let message = event.data.get("message").unwrap_or(&event.data);
                let (text, thinking) = extract_text_and_reasoning(message.get("content"));
                if text.trim().is_empty() && thinking.trim().is_empty() {
                    continue;
                }
                let msg = SessionMessage::new("assistant", text, event.time);
                messages.push(if thinking.trim().is_empty() {
                    msg
                } else {
                    msg.with_thinking(Some(thinking))
                });
            }
            _ => {}
        }
    }
    messages
}

pub fn get_dsh_messages(
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    let path = find_dsh_session_file(session_id)
        .ok_or_else(|| format!("DeepSeek Harness session not found: {}", session_id))?;
    let log = decode_dsh_log(&path)?;
    let messages = collect_dsh_messages(&log);
    Ok(messages
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>())
}

pub fn last_dsh_messages(
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    let messages = get_dsh_messages(session_id, 0, usize::MAX)?;
    let last_user = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();
    let last_assistant = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .cloned();
    Ok((last_user, last_assistant))
}

pub fn search_dsh_messages(query_lower: &str) -> Result<Vec<crate::session::models::SessionSearchResult>, String> {
    let cache = load_dsh_projcache();
    let mut results = Vec::new();
    for file in list_dsh_session_files() {
        let Ok(log) = decode_dsh_log(&file.log_path) else {
            continue;
        };
        if log.header.is_subagent {
            continue;
        }
        let title = projcache_session(&cache, &file.session_id)
            .and_then(|s| projcache_row(s, "title"))
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| file.session_id.clone());
        for message in collect_dsh_messages(&log) {
            if message.matches_query(query_lower) {
                results.push(crate::session::models::SessionSearchResult {
                    session_id: file.session_id.clone(),
                    session_title: title.clone(),
                    project_path: log.header.cwd.clone().unwrap_or_default(),
                    platform_id: PLATFORM_ID.to_string(),
                    message,
                });
            }
        }
    }
    results.sort_by(|a, b| b.message.timestamp.cmp(&a.message.timestamp));
    Ok(results)
}

/// Delete a session: remove its directory, then drop the id from DSH's own
/// indexes (`workspace.json` sessionIds and `session_projcache.json` table)
/// so the harness UI does not keep pointing at a dead entry.
pub fn delete_dsh_session(session_id: &str) -> Result<(), String> {
    let file = find_dsh_session_file(session_id)
        .ok_or_else(|| format!("DeepSeek Harness session not found: {}", session_id))?;
    let session_dir = file
        .parent()
        .ok_or_else(|| "invalid session path".to_string())?;
    fs::remove_dir_all(session_dir).map_err(|e| format!("无法删除会话目录: {e}"))?;
    // Best effort: the session is gone from disk even if index cleanup fails.
    let _ = remove_id_from_workspace_index(session_id);
    let _ = remove_id_from_projcache(session_id);
    Ok(())
}

fn remove_id_from_workspace_index(session_id: &str) -> Result<(), String> {
    let Some(path) = dsh_workspace_path() else {
        return Ok(());
    };
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut doc: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let mut changed = false;
    if let Some(workspaces) = doc.get_mut("tables").and_then(|t| t.get_mut("workspaces")) {
        if let Some(map) = workspaces.as_object_mut() {
            for (_id, workspace) in map.iter_mut() {
                if let Some(ids) = workspace.get_mut("sessionIds").and_then(|v| v.as_array_mut()) {
                    let before = ids.len();
                    ids.retain(|v| v.as_str() != Some(session_id));
                    changed |= ids.len() != before;
                }
            }
        }
    }
    if !changed {
        return Ok(());
    }
    write_json_atomic(&path, &doc)
}

fn remove_id_from_projcache(session_id: &str) -> Result<(), String> {
    let Some(path) = dsh_projcache_path() else {
        return Ok(());
    };
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut doc: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let removed = doc
        .get_mut("tables")
        .and_then(|t| t.get_mut("sessions"))
        .and_then(|s| s.as_object_mut())
        .map(|map| map.remove(session_id).is_some())
        .unwrap_or(false);
    if !removed {
        return Ok(());
    }
    write_json_atomic(&path, &doc)
}

fn write_json_atomic(path: &Path, doc: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    let parent = path.parent().ok_or("no parent")?;
    let temp = parent.join(format!(".agent-hub-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temp, text).map_err(|e| e.to_string())?;
    crate::paths::replace_file(&temp, path).map_err(|e| format!("replace failed: {e}"))
}

// ---------------------------------------------------------------------------
// Extraction helpers
// ---------------------------------------------------------------------------

fn first_user_text(log: &DshLog) -> Option<String> {
    log.events
        .iter()
        .find(|event| {
            event.event_type == "user/message"
                && event
                    .data
                    .get("source")
                    .and_then(|s| s.get("kind"))
                    .and_then(|v| v.as_str())
                    == Some("user")
        })
        .and_then(|event| {
            let text = extract_text_blocks(event.data.get("content"));
            (!text.trim().is_empty()).then_some(text)
        })
}

fn first_assistant_model(log: &DshLog) -> Option<String> {
    log.events
        .iter()
        .find(|event| event.event_type == "assistant/message")
        .and_then(|event| {
            let message = event.data.get("message").unwrap_or(&event.data);
            let source = message.get("source")?;
            let provider = source.get("provider").and_then(|v| v.as_str())?;
            let model = source.get("model").and_then(|v| v.as_str())?;
            Some(format!("{provider} {model}"))
        })
}

/// Join every `text` content block; images and tool blocks carry no visible
/// prose to render.
fn extract_text_blocks(content: Option<&Value>) -> String {
    let Some(blocks) = content.and_then(|c| c.as_array()) else {
        return String::new();
    };
    blocks
        .iter()
        .filter(|block| block.get("type").and_then(|v| v.as_str()) == Some("text"))
        .filter_map(|block| block.get("text").and_then(|v| v.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Visible reply = `text` blocks; reasoning = `reasoning` blocks (kept
/// separate so the UI can render a thinking section like other platforms).
fn extract_text_and_reasoning(content: Option<&Value>) -> (String, String) {
    let Some(blocks) = content.and_then(|c| c.as_array()) else {
        return (String::new(), String::new());
    };
    let mut text = Vec::new();
    let mut reasoning = Vec::new();
    for block in blocks {
        let Some(block_type) = block.get("type").and_then(|v| v.as_str()) else {
            continue;
        };
        match block_type {
            "text" => {
                if let Some(value) = block.get("text").and_then(|v| v.as_str()) {
                    text.push(value);
                }
            }
            "reasoning" => {
                if let Some(value) = block.get("text").and_then(|v| v.as_str()) {
                    reasoning.push(value);
                }
            }
            _ => {}
        }
    }
    (text.join("\n"), reasoning.join("\n"))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max_chars).collect();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsh_sessions_real_data_smoke_test() {
        let files = list_dsh_session_files();
        if files.is_empty() {
            return;
        }
        let sessions = list_dsh_sessions_all().expect("dsh scan should not fail");
        if sessions.is_empty() {
            return;
        }
        // Real session logs must decode past the header frame into actual
        // transcript messages — a single-frame decode regression shows up
        // here as zero events/messages.
        let mut decoded_any = false;
        for file in &files {
            let log = decode_dsh_log(&file.log_path).expect("dsh log should decode");
            decoded_any |= !log.events.is_empty();
        }
        assert!(decoded_any, "real dsh session logs decoded zero events");
        let first = &sessions[0];
        let page = get_dsh_messages(&first.id, 0, 50);
        if let Ok(messages) = page {
            assert!(messages.len() <= 50);
            assert!(
                !messages.is_empty(),
                "session {} should expose transcript messages",
                first.id
            );
        }
    }

    #[test]
    fn parse_header_and_events_from_jsonl_text() {
        let text = r#"{"type":"session","version":0,"id":"session-a","createdAt":1000,"cwd":"/tmp/proj"}
{"type":"user/message","seq":0,"time":2000,"data":{"source":{"kind":"user"},"content":[{"type":"text","text":"hello"}]}}
{"type":"assistant/message","seq":1,"time":3000,"data":{"message":{"content":[{"type":"text","text":"hi"},{"type":"reasoning","text":"think"}]}}}
{"type":"text-chunks","seq0":2,"time0":3000,"data":{"turn":1,"step":1,"index":0,"dt":[],"texts":["x"]}}
"#;
        let mut log = DshLog::default();
        for (index, line) in text.lines().enumerate() {
            let parsed: Value = serde_json::from_str(line).unwrap();
            let record_type = parsed.get("type").and_then(|v| v.as_str()).unwrap();
            if index == 0 {
                log.header.id = "session-a".into();
                log.header.created_at = 1000;
                log.header.cwd = Some("/tmp/proj".into());
                continue;
            }
            if CHUNK_ROW_TYPES.contains(&record_type) {
                continue;
            }
            log.events.push(DshEvent {
                event_type: record_type.into(),
                time: parsed.get("time").and_then(|v| v.as_i64()).unwrap_or_default(),
                data: parsed.get("data").cloned().unwrap_or(Value::Null),
            });
        }
        let messages = collect_dsh_messages(&log);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "hi");
        assert_eq!(messages[1].thinking.as_deref(), Some("think"));
        assert_eq!(first_user_text(&log).as_deref(), Some("hello"));
    }

    #[test]
    fn plugin_injected_user_message_is_not_a_transcript_row() {
        let log = DshLog {
            header: DshHeader::default(),
            events: vec![DshEvent {
                event_type: "user/message".into(),
                time: 1,
                data: serde_json::json!({
                    "source": {"kind": "plugin", "plugin": "fs-observation"},
                    "content": [{"type": "text", "text": "internal notice"}]
                }),
            }],
        };
        assert!(collect_dsh_messages(&log).is_empty());
    }

    #[test]
    fn replacement_surface_copy_is_skipped_in_transcript() {
        let log = DshLog {
            header: DshHeader::default(),
            events: vec![DshEvent {
                event_type: "user/message".into(),
                time: 1,
                data: serde_json::json!({
                    "source": {"kind": "user"},
                    "content": [{"type": "text", "text": "compacted copy"}],
                    "surfaceOp": {"op": "replace", "start": 0, "end": 0}
                }),
            }],
        };
        assert!(collect_dsh_messages(&log).is_empty());
    }

    #[test]
    fn truncate_chars_caps_long_titles() {
        assert_eq!(truncate_chars("short", 80), "short");
        let long = "长".repeat(100);
        assert_eq!(truncate_chars(&long, 80).chars().count(), 83); // 80 + "..."
    }
}

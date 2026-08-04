use super::types::{AgentKind, HookEvent, SessionSource};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const CODEX_HOOK_ARG: &str = "--agent-hub-codex-hook";
pub const CLAUDE_HOOK_ARG: &str = "--agent-hub-claude-hook";
pub const CURSOR_HOOK_ARG: &str = "--agent-hub-cursor-hook";
pub const GROK_HOOK_ARG: &str = "--agent-hub-grok-hook";
pub const KIMI_HOOK_ARG: &str = "--agent-hub-kimi-hook";
pub const ZCODE_HOOK_ARG: &str = "--agent-hub-zcode-hook";
const MAX_HOOK_INPUT_BYTES: u64 = 256 * 1024;
const MAX_IGNORED_SESSIONS: usize = 200;

/// Prompt signatures of Codex desktop's internal background turns. The
/// desktop app runs its own hidden turns (ambient-suggestion generation, the
/// safety/compliance reviewer for those suggestions, memory consolidation),
/// and they fire UserPromptSubmit/Stop hooks just like real user turns. They
/// are never persisted to the Codex threads DB, so prompt matching is the
/// only way to recognize them. Automation prompts are user-configured and
/// intentionally NOT filtered here.
pub fn is_internal_system_prompt(prompt: &str) -> bool {
    let trimmed = prompt.trim_start();
    // Ambient-suggestion generator ("# Overview\n\nGenerate 0 to 3 hyperpersonalized…").
    if trimmed.starts_with("# Overview") && trimmed.contains("hyperpersonalized suggestions") {
        return true;
    }
    // Safety/compliance reviewer for ambient suggestions.
    if trimmed.contains("Codex ambient suggestions") {
        return true;
    }
    // Memory consolidation agent ("## Memory Writing Agent: Phase 2 …").
    if trimmed.starts_with("## Memory Writing Agent") {
        return true;
    }
    false
}

pub fn try_capture_hook_event() -> bool {
    let agent = if std::env::args().any(|arg| arg == CODEX_HOOK_ARG) {
        AgentKind::Codex
    } else if std::env::args().any(|arg| arg == CLAUDE_HOOK_ARG) {
        AgentKind::Claude
    } else if std::env::args().any(|arg| arg == CURSOR_HOOK_ARG) {
        AgentKind::Cursor
    } else if std::env::args().any(|arg| arg == GROK_HOOK_ARG) {
        AgentKind::Grok
    } else if std::env::args().any(|arg| arg == KIMI_HOOK_ARG) {
        AgentKind::Kimi
    } else if std::env::args().any(|arg| arg == ZCODE_HOOK_ARG) {
        AgentKind::Zcode
    } else {
        return false;
    };

    if let Err(error) = capture_stdin_event(agent) {
        // Hooks must never block or fail an agent turn. This process
        // intentionally exits successfully even when the local event inbox is
        // unavailable.
        eprintln!("agent-hub {} hook capture skipped: {error}", agent.as_str());
    }
    true
}

fn capture_stdin_event(agent: AgentKind) -> Result<(), String> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(MAX_HOOK_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("unable to read hook input: {error}"))?;
    if bytes.len() as u64 > MAX_HOOK_INPUT_BYTES {
        return Err("hook input exceeded 256 KiB".to_string());
    }

    let input: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid hook JSON: {error}"))?;
    // Codex/Claude/Kimi/Zcode wrap events in snake_case, Grok in camelCase.
    let event_name = string_field_any(&input, &["hook_event_name", "hookEventName"])
        .ok_or_else(|| "hook_event_name is missing".to_string())?;
    let Some(event_name) = canonical_event_name(&event_name) else {
        return Ok(());
    };

    let event_id = Uuid::new_v4().to_string();
    let session_id = string_field_any(&input, &["session_id", "sessionId", "conversation_id"])
        .or_else(|| std::env::var("CODEX_THREAD_ID").ok())
        .unwrap_or_else(|| format!("unknown-{event_id}"));
    let user_prompt = prompt_field(&input);

    // Codex desktop only: drop internal desktop turns (ambient suggestions,
    // safety reviewer, memory consolidation). Their Stop event carries no
    // prompt, so remember the session id and drop the matching Stop when it
    // arrives. Claude Code has no such hidden turns.
    if agent == AgentKind::Codex {
        if event_name == "UserPromptSubmit" {
            if let Some(prompt) = user_prompt.as_deref() {
                if is_internal_system_prompt(prompt) {
                    let _ = mark_session_ignored(&session_id);
                    return Ok(());
                }
            }
        }
        if event_name == "Stop" && take_ignored_session(&session_id) {
            return Ok(());
        }
    }

    // Codex provides `turn_id`, Claude Code provides `prompt_id` (v2.1.196+),
    // and Cursor provides `generation_id` for one user-message lifecycle.
    let turn_id = resolve_turn_id(&input, &event_id);
    let source = match agent {
        AgentKind::Codex => detect_source(),
        AgentKind::Cursor => SessionSource::Cursor,
        // Claude Desktop is intentionally out of scope; the Claude Code hook
        // only fires for terminal sessions. Grok Build and Kimi Code are
        // terminal CLIs, and the Zcode hook payload carries no client
        // discriminator.
        _ => SessionSource::Terminal,
    };
    let event = HookEvent {
        event_id: event_id.clone(),
        agent,
        hook_event_name: event_name.to_string(),
        session_id,
        turn_id,
        source,
        cwd: cwd_field(&input),
        user_prompt,
        assistant_reply: assistant_reply_field(agent, &input),
        occurred_at: now_millis(),
    };

    let inbox = monitor_root()?.join("inbox");
    fs::create_dir_all(&inbox).map_err(|error| format!("unable to create event inbox: {error}"))?;
    let final_path = inbox.join(format!("{}-{event_id}.json", event.occurred_at));
    let temp_path = inbox.join(format!(".{event_id}.tmp"));
    let payload = serde_json::to_vec(&event)
        .map_err(|error| format!("unable to serialize hook event: {error}"))?;

    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .map_err(|error| format!("unable to create event file: {error}"))?;
    file.write_all(&payload)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("unable to persist hook event: {error}"))?;
    fs::rename(&temp_path, &final_path)
        .map_err(|error| format!("unable to publish hook event: {error}"))?;
    Ok(())
}

/// Normalize the hook event value across agents: Codex/Claude/Kimi use
/// PascalCase, Grok uses snake_case, and Cursor uses camelCase lifecycle
/// names. Returns None for events we ignore.
/// Kimi Code fires Interrupt (never Stop) when the user aborts a turn, and
/// Claude/Grok/Kimi fire StopFailure when a turn dies on an API error — both
/// are normalized to Stop because the turn is over either way.
fn canonical_event_name(name: &str) -> Option<&'static str> {
    match name {
        "UserPromptSubmit" | "user_prompt_submit" => Some("UserPromptSubmit"),
        "beforeSubmitPrompt" => Some("UserPromptSubmit"),
        "afterAgentResponse" => Some("AssistantResponse"),
        "Stop" | "stop" | "Interrupt" | "interrupt" | "StopFailure" | "stop_failure" => {
            Some("Stop")
        }
        _ => None,
    }
}

fn string_field_any(input: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| string_field(input, key))
}

/// Extract the user prompt from the hook payload. Codex/Claude/Grok send a
/// plain string; Kimi Code sends an array of content parts
/// (`[{type: "text", text: "…"}, …]`), whose text parts are joined here.
fn prompt_field(input: &serde_json::Value) -> Option<String> {
    for key in ["prompt", "promptText"] {
        let Some(value) = input.get(key) else {
            continue;
        };
        if let Some(text) = string_field(input, key) {
            return Some(text);
        }
        if let Some(parts) = value.as_array() {
            let text = parts
                .iter()
                .filter(|part| part.get("type").and_then(serde_json::Value::as_str) == Some("text"))
                .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn resolve_turn_id(input: &serde_json::Value, fallback: &str) -> String {
    string_field(input, "turn_id")
        .or_else(|| string_field(input, "prompt_id"))
        .or_else(|| string_field(input, "generation_id"))
        .unwrap_or_else(|| fallback.to_string())
}

fn cwd_field(input: &serde_json::Value) -> Option<String> {
    string_field(input, "cwd").or_else(|| {
        input
            .get("workspace_roots")
            .and_then(serde_json::Value::as_array)
            .and_then(|roots| roots.first())
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn assistant_reply_field(agent: AgentKind, input: &serde_json::Value) -> Option<String> {
    let reply = string_field_any(input, &["last_assistant_message", "lastAssistantMessage"]);
    if agent == AgentKind::Cursor {
        reply.or_else(|| string_field(input, "text"))
    } else {
        reply
    }
}

fn string_field(input: &serde_json::Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn detect_source() -> SessionSource {
    let originator = std::env::var("CODEX_INTERNAL_ORIGINATOR_OVERRIDE")
        .unwrap_or_default()
        .to_ascii_lowercase();
    source_from_originator(&originator)
}

fn source_from_originator(originator: &str) -> SessionSource {
    if originator.contains("desktop") || originator.contains("chatgpt") {
        SessionSource::Chatgpt
    } else {
        SessionSource::Terminal
    }
}

fn monitor_root() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".agent-hub").join("session-monitor"))
        .ok_or_else(|| "home directory is unavailable".to_string())
}

fn ignored_sessions_path() -> Result<PathBuf, String> {
    Ok(monitor_root()?.join("ignored-sessions.json"))
}

fn load_ignored_sessions() -> Vec<String> {
    let Ok(path) = ignored_sessions_path() else {
        return Vec::new();
    };
    fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_ignored_sessions(ids: &[String]) -> Result<(), String> {
    let path = ignored_sessions_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("unable to create ignored-session directory: {error}"))?;
    }
    let temp_path = path.with_file_name(format!(".ignored-sessions-{}.tmp", Uuid::new_v4()));
    let payload = serde_json::to_vec(ids)
        .map_err(|error| format!("unable to serialize ignored sessions: {error}"))?;
    fs::write(&temp_path, payload)
        .map_err(|error| format!("unable to persist ignored sessions: {error}"))?;
    crate::paths::replace_file(&temp_path, &path)
        .map_err(|error| format!("unable to persist ignored sessions: {error}"))
}

/// Remember a session whose events must be dropped (internal desktop turn).
fn mark_session_ignored(session_id: &str) -> Result<(), String> {
    let mut ids = load_ignored_sessions();
    if !ids.iter().any(|id| id == session_id) {
        ids.push(session_id.to_string());
    }
    if ids.len() > MAX_IGNORED_SESSIONS {
        let overflow = ids.len() - MAX_IGNORED_SESSIONS;
        ids.drain(..overflow);
    }
    save_ignored_sessions(&ids)
}

/// Remove and return whether the session was marked as ignored.
fn take_ignored_session(session_id: &str) -> bool {
    let mut ids = load_ignored_sessions();
    let Some(position) = ids.iter().position(|id| id == session_id) else {
        return false;
    };
    ids.remove(position);
    let _ = save_ignored_sessions(&ids);
    true
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_field_reads_plain_string() {
        let input = serde_json::json!({"prompt": "  帮我修一下这个 bug  "});
        assert_eq!(prompt_field(&input).as_deref(), Some("帮我修一下这个 bug"));
    }

    #[test]
    fn prompt_field_joins_kimi_content_parts() {
        let input = serde_json::json!({
            "prompt": [
                {"type": "text", "text": "第一段"},
                {"type": "image", "source": {"kind": "url", "url": "https://x/y.png"}},
                {"type": "text", "text": "第二段"}
            ]
        });
        assert_eq!(prompt_field(&input).as_deref(), Some("第一段\n第二段"));
    }

    #[test]
    fn prompt_field_returns_none_for_empty_or_missing_prompt() {
        assert_eq!(prompt_field(&serde_json::json!({})), None);
        assert_eq!(prompt_field(&serde_json::json!({"prompt": "  "})), None);
        assert_eq!(prompt_field(&serde_json::json!({"prompt": []})), None);
        assert_eq!(
            prompt_field(&serde_json::json!({"prompt": [{"type": "image", "source": {}}]})),
            None
        );
    }

    #[test]
    fn source_distinguishes_desktop_and_terminal_originators() {
        assert_eq!(
            source_from_originator("codex desktop"),
            SessionSource::Chatgpt
        );
        assert_eq!(source_from_originator("chatgpt"), SessionSource::Chatgpt);
        assert_eq!(source_from_originator("codex cli"), SessionSource::Terminal);
    }

    #[test]
    fn event_names_are_normalized_across_agents() {
        assert_eq!(
            canonical_event_name("UserPromptSubmit"),
            Some("UserPromptSubmit")
        );
        assert_eq!(
            canonical_event_name("user_prompt_submit"),
            Some("UserPromptSubmit")
        );
        assert_eq!(
            canonical_event_name("beforeSubmitPrompt"),
            Some("UserPromptSubmit")
        );
        assert_eq!(
            canonical_event_name("afterAgentResponse"),
            Some("AssistantResponse")
        );
        assert_eq!(canonical_event_name("Stop"), Some("Stop"));
        assert_eq!(canonical_event_name("stop"), Some("Stop"));
        // Kimi Code fires Interrupt instead of Stop on user abort (Esc/Ctrl+C);
        // Claude/Grok/Kimi fire StopFailure when a turn dies on an API error.
        assert_eq!(canonical_event_name("Interrupt"), Some("Stop"));
        assert_eq!(canonical_event_name("interrupt"), Some("Stop"));
        assert_eq!(canonical_event_name("StopFailure"), Some("Stop"));
        assert_eq!(canonical_event_name("stop_failure"), Some("Stop"));
        assert_eq!(canonical_event_name("SessionStart"), None);
        assert_eq!(canonical_event_name("session_start"), None);
        assert_eq!(canonical_event_name("Notification"), None);
        assert_eq!(canonical_event_name("SubagentStop"), None);
    }

    #[test]
    fn turn_id_falls_back_to_prompt_id_then_event_id() {
        let with_turn = serde_json::json!({"turn_id": "turn-1", "prompt_id": "prompt-1"});
        assert_eq!(resolve_turn_id(&with_turn, "event-1"), "turn-1");
        let claude_style = serde_json::json!({"prompt_id": "prompt-1"});
        assert_eq!(resolve_turn_id(&claude_style, "event-1"), "prompt-1");
        let cursor_style = serde_json::json!({"generation_id": "generation-1"});
        assert_eq!(resolve_turn_id(&cursor_style, "event-1"), "generation-1");
        let bare = serde_json::json!({});
        assert_eq!(resolve_turn_id(&bare, "event-1"), "event-1");
    }

    #[test]
    fn cursor_payload_fields_are_extracted() {
        let input = serde_json::json!({
            "conversation_id": "conversation-1",
            "generation_id": "generation-1",
            "workspace_roots": ["/tmp/cursor-project"],
            "text": "Cursor reply"
        });
        assert_eq!(
            string_field_any(&input, &["session_id", "sessionId", "conversation_id"]).as_deref(),
            Some("conversation-1")
        );
        assert_eq!(cwd_field(&input).as_deref(), Some("/tmp/cursor-project"));
        assert_eq!(
            assistant_reply_field(AgentKind::Cursor, &input).as_deref(),
            Some("Cursor reply")
        );
        assert_eq!(assistant_reply_field(AgentKind::Claude, &input), None);
    }

    #[test]
    fn zcode_payload_fields_are_extracted() {
        // Zcode wraps hook payloads in snake_case (with camelCase aliases):
        // session_id is `sess_<uuid>`, Stop carries last_assistant_message.
        let prompt_input = serde_json::json!({
            "session_id": "sess-9f2c",
            "hook_event_name": "UserPromptSubmit",
            "cwd": "/Users/demo/projects/zcode-app",
            "transcript_path": "/Users/demo/.zcode/cli/sessions/sess-9f2c.jsonl",
            "prompt": "把设置页改成暗色主题"
        });
        assert_eq!(
            string_field_any(&prompt_input, &["session_id", "sessionId", "conversation_id"])
                .as_deref(),
            Some("sess-9f2c")
        );
        assert_eq!(prompt_field(&prompt_input).as_deref(), Some("把设置页改成暗色主题"));
        assert_eq!(cwd_field(&prompt_input).as_deref(), Some("/Users/demo/projects/zcode-app"));
        assert_eq!(resolve_turn_id(&prompt_input, "event-1"), "event-1");

        let stop_input = serde_json::json!({
            "session_id": "sess-9f2c",
            "hook_event_name": "Stop",
            "last_assistant_message": "已切换为暗色主题。",
            "stop_hook_active": false
        });
        assert_eq!(
            assistant_reply_field(AgentKind::Zcode, &stop_input).as_deref(),
            Some("已切换为暗色主题。")
        );

        // camelCase aliases read the same values.
        let aliased = serde_json::json!({
            "sessionId": "sess-7a1b",
            "hookEventName": "Stop",
            "lastAssistantMessage": "done"
        });
        assert_eq!(
            string_field_any(&aliased, &["session_id", "sessionId", "conversation_id"])
                .as_deref(),
            Some("sess-7a1b")
        );
        assert_eq!(
            assistant_reply_field(AgentKind::Zcode, &aliased).as_deref(),
            Some("done")
        );
    }

    #[test]
    fn internal_prompt_matches_ambient_suggestion_generator() {
        let prompt = "# Overview\n\nGenerate 0 to 3 hyperpersonalized suggestions for what this user can do with Codex in this local project: /tmp/x";
        assert!(is_internal_system_prompt(prompt));
    }

    #[test]
    fn internal_prompt_matches_ambient_safety_reviewer() {
        let prompt = "You are an expert at upholding safety and compliance standards for Codex ambient suggestions.\n\nI will present…";
        assert!(is_internal_system_prompt(prompt));
    }

    #[test]
    fn internal_prompt_matches_memory_writing_agent() {
        let prompt =
            "## Memory Writing Agent: Phase 2 (Consolidation)\n\nYou are a Memory Writing Agent.";
        assert!(is_internal_system_prompt(prompt));
    }

    #[test]
    fn internal_prompt_keeps_automations_and_real_user_prompts() {
        // Automations are user-configured scheduled tasks — they stay visible.
        let automation = "Automation: 吾日三省\nAutomation ID: automation\nAutomation memory: $CODEX_HOME/automations/automation/memory.md\n\n扮演一名心理咨询师…";
        assert!(!is_internal_system_prompt(automation));
        assert!(!is_internal_system_prompt("帮我修一下这个 bug"));
        assert!(!is_internal_system_prompt(
            "# Overview of my project\n\n请写一份项目概览"
        ));
    }
}

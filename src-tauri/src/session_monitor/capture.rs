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
pub const ANTIGRAVITY_HOOK_ARG: &str = "--agent-hub-antigravity-hook";
pub const KIRO_HOOK_ARG: &str = "--agent-hub-kiro-hook";
/// Runaway-stdin guard, not a payload policy: Kimi embeds pasted images as
/// base64 in the prompt content parts, so a legitimate UserPromptSubmit can
/// reach several MiB. The monitor only extracts the text parts anyway.
const MAX_HOOK_INPUT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_IGNORED_SESSIONS: usize = 200;
/// Kimi sub-agent markers older than this are treated as stale: the matching
/// SubagentStop presumably never arrived (killed process), so they must not
/// suppress the main turn's Stop forever.
const KIMI_SUBAGENT_MARKER_TTL_MILLIS: i64 = 60 * 60 * 1000;

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
        AgentKind::ZCode
    } else if std::env::args().any(|arg| arg == ANTIGRAVITY_HOOK_ARG) {
        AgentKind::Antigravity
    } else if std::env::args().any(|arg| arg == KIRO_HOOK_ARG) {
        AgentKind::Kiro
    } else {
        return false;
    };

    if let Err(error) = capture_stdin_event(agent) {
        // Hooks must never block or fail an agent turn. This process
        // intentionally exits successfully even when the local event inbox is
        // unavailable. Always leave a breadcrumb so Windows silent failures
        // (empty stdin from GUI-subsystem spawn) are diagnosable.
        eprintln!("agent-hub {} hook capture skipped: {error}", agent.as_str());
        log_capture_error(agent, &error);
    }
    true
}

fn capture_stdin_event(agent: AgentKind) -> Result<(), String> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(MAX_HOOK_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("unable to read hook input: {error}"))?;
    // Kiro IDE Prompt Submit may put the user text in USER_PROMPT and still
    // send a JSON envelope (or an empty body on older builds). Allow empty
    // stdin only for Kiro when env context is enough to build an event.
    if bytes.is_empty() && agent != AgentKind::Kiro {
        return Err(
            "hook stdin was empty (Windows GUI-subsystem / shell spawn often causes this; \
             reinstall hooks after upgrading Agent Hub so the .cmd runner is used)"
                .to_string(),
        );
    }
    if bytes.len() as u64 > MAX_HOOK_INPUT_BYTES {
        return Err(format!(
            "hook input exceeded {} MiB",
            MAX_HOOK_INPUT_BYTES / (1024 * 1024)
        ));
    }
    if !bytes.is_empty() {
        debug_dump_payload(agent, &bytes);
    }

    let mut input: serde_json::Value = if bytes.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid hook JSON: {error}"))?
    };
    // IDE docs: user prompt also available as USER_PROMPT for shell actions.
    if agent == AgentKind::Kiro {
        merge_kiro_env_fields(&mut input);
    }

    // Provenance guard: Grok CLI also executes hooks configured for other
    // agents (~/.claude/settings.json, ~/.cursor/hooks.json) but feeds them
    // its OWN payloads, which would plant phantom rows in those agents'
    // monitors. Reject payloads that cannot belong to the agent whose hook
    // arg invoked us.
    if !payload_matches_agent(agent, &input) {
        return Ok(());
    }

    let event_id = Uuid::new_v4().to_string();
    let session_id = string_field_any(
        &input,
        &[
            "session_id",
            "sessionId",
            "conversation_id",
            "conversationId",
        ],
    )
    .or_else(|| std::env::var("CODEX_THREAD_ID").ok())
    .unwrap_or_else(|| format!("unknown-{event_id}"));

    // Kimi Code fires the plain Stop event when a SUB-AGENT's model turn ends
    // (payload identical to the main turn's Stop, always BEFORE SubagentStop —
    // verified against kimi-code 2.x). Without filtering, every Agent-tool
    // call flips the monitor row to "ended" while the main turn is still
    // running. SubagentStart/SubagentStop maintain marker files per session;
    // a Stop that arrives while any live marker exists belongs to a sub-agent
    // and is dropped. Interrupt/StopFailure always pass through: they concern
    // the whole turn, and dropping them could strand the row as "running".
    if agent == AgentKind::Kimi {
        let raw_name = string_field_any(&input, &["hook_event_name", "hookEventName"])
            .unwrap_or_default();
        match raw_name.as_str() {
            "SubagentStart" => return mark_kimi_subagent(&session_id),
            "SubagentStop" => return clear_kimi_subagent(&session_id),
            "Stop" | "stop" if kimi_subagent_active(&session_id) => return Ok(()),
            _ => {}
        }
    }

    // Grok subagents are independent child sessions (verified against grok
    // 0.2.x): a child fires its own user_prompt_submit under its own
    // sessionId but never a stop — without filtering, every Task-tool call
    // plants a permanently "running" phantom row showing the internal task
    // prompt. SubagentStart names the child session in subagentId; every
    // event from an ignored session is dropped.
    if agent == AgentKind::Grok {
        let raw_name = string_field_any(&input, &["hook_event_name", "hookEventName"])
            .unwrap_or_default();
        match raw_name.as_str() {
            "subagent_start" | "SubagentStart" => {
                if let Some(child_id) = string_field_any(&input, &["subagent_id", "subagentId"]) {
                    let _ = mark_session_ignored(&child_id);
                }
                return Ok(());
            }
            "subagent_stop" | "SubagentStop" => return Ok(()),
            // Grok appends a second Stop with `reason: "shutdown"` when the
            // session closes. For sessions the hooks saw it only re-marks the
            // row ended; for sessions whose turns predate the hooks (e.g.
            // internal grok-build-plan sessions) it is the ONLY event and
            // would plant a prompt-less noise row. Drop it either way.
            "stop" | "Stop" if is_grok_session_close(&input) => return Ok(()),
            _ => {}
        }
        if is_session_ignored(&session_id) {
            return Ok(());
        }
    }

    // Codex/Claude/Kimi/ZCode wrap events in snake_case, Grok in camelCase.
    // Antigravity payloads omit hookEventName — infer from field shape.
    let raw_event_name = string_field_any(&input, &["hook_event_name", "hookEventName"])
        .or_else(|| infer_antigravity_event_name(agent, &input));
    let event_name = raw_event_name.ok_or_else(|| "hook_event_name is missing".to_string())?;
    let Some(event_name) = canonical_event_name(&event_name) else {
        return Ok(());
    };

    let mut user_prompt = prompt_field(&input).map(|prompt| {
        if agent == AgentKind::Grok {
            unwrap_grok_user_query(&prompt)
        } else {
            prompt
        }
    });
    // Antigravity PreInvocation/Stop never carry the user text; pull the last
    // USER_INPUT from the conversation transcript when available.
    if agent == AgentKind::Antigravity && user_prompt.is_none() {
        if let Some(path) = string_field_any(&input, &["transcriptPath", "transcript_path"]) {
            user_prompt = last_transcript_user_prompt(&path);
        }
    }

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
        // Shared hooks.json; product is encoded in transcriptPath /
        // artifactDirectoryPath (antigravity-cli | antigravity | antigravity-ide).
        AgentKind::Antigravity => detect_antigravity_source(&input),
        // Claude Desktop is intentionally out of scope; the Claude Code hook
        // only fires for terminal sessions. Grok Build and Kimi Code are
        // terminal CLIs, and the ZCode hook payload carries no client
        // discriminator.
        _ => SessionSource::Terminal,
    };
    let mut assistant_reply = assistant_reply_field(agent, &input);
    if agent == AgentKind::Antigravity && event_name == "Stop" && assistant_reply.is_none() {
        if let Some(path) = string_field_any(&input, &["transcriptPath", "transcript_path"]) {
            assistant_reply = last_transcript_assistant_reply(&path);
        }
    }

    let event = HookEvent {
        event_id: event_id.clone(),
        agent,
        hook_event_name: event_name.to_string(),
        session_id,
        turn_id,
        source,
        cwd: cwd_field(&input),
        user_prompt,
        assistant_reply,
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
        "UserPromptSubmit"
        | "user_prompt_submit"
        | "userPromptSubmit"
        | "PreInvocation"
        | "pre_invocation" => Some("UserPromptSubmit"),
        "beforeSubmitPrompt" => Some("UserPromptSubmit"),
        "afterAgentResponse" => Some("AssistantResponse"),
        // Kiro docs also call this "Agent Stop"; CLI payloads may use camelCase.
        "Stop"
        | "stop"
        | "agentStop"
        | "AgentStop"
        | "Interrupt"
        | "interrupt"
        | "StopFailure"
        | "stop_failure" => Some("Stop"),
        _ => None,
    }
}

/// Fill missing Kiro fields from process environment (IDE shell hooks).
fn merge_kiro_env_fields(input: &mut serde_json::Value) {
    let Some(object) = input.as_object_mut() else {
        return;
    };
    if !object.contains_key("prompt") && !object.contains_key("promptText") {
        if let Ok(prompt) = std::env::var("USER_PROMPT") {
            if !prompt.trim().is_empty() {
                object.insert("prompt".to_string(), serde_json::Value::String(prompt));
            }
        }
    }
    if !object.contains_key("session_id")
        && !object.contains_key("sessionId")
        && !object.contains_key("conversation_id")
    {
        for key in ["KIRO_SESSION_ID", "SESSION_ID"] {
            if let Ok(session_id) = std::env::var(key) {
                if !session_id.trim().is_empty() {
                    object.insert(
                        "session_id".to_string(),
                        serde_json::Value::String(session_id),
                    );
                    break;
                }
            }
        }
    }
    if !object.contains_key("hook_event_name") && !object.contains_key("hookEventName") {
        if let Ok(event) = std::env::var("HOOK_EVENT_NAME")
            .or_else(|_| std::env::var("KIRO_HOOK_EVENT"))
        {
            if !event.trim().is_empty() {
                object.insert("hook_event_name".to_string(), serde_json::Value::String(event));
            }
        } else if object.get("prompt").is_some() {
            // Prompt Submit with env-only text and no event name.
            object.insert(
                "hook_event_name".to_string(),
                serde_json::Value::String("UserPromptSubmit".to_string()),
            );
        }
    }
    if !object.contains_key("cwd") {
        if let Ok(cwd) = std::env::var("PWD") {
            object.insert("cwd".to_string(), serde_json::Value::String(cwd));
        } else if let Ok(cwd) = std::env::current_dir() {
            object.insert(
                "cwd".to_string(),
                serde_json::Value::String(cwd.display().to_string()),
            );
        }
    }
}

/// Antigravity stdin payloads have no hookEventName. Infer from documented
/// field shapes: Stop carries `terminationReason`/`fullyIdle`; PreInvocation
/// carries `invocationNum`.
fn infer_antigravity_event_name(agent: AgentKind, input: &serde_json::Value) -> Option<String> {
    if agent != AgentKind::Antigravity {
        return None;
    }
    if input.get("terminationReason").is_some()
        || input.get("termination_reason").is_some()
        || input.get("fullyIdle").is_some()
        || input.get("fully_idle").is_some()
    {
        return Some("Stop".to_string());
    }
    if input.get("invocationNum").is_some() || input.get("invocation_num").is_some() {
        return Some("PreInvocation".to_string());
    }
    // Fallback: conversation lifecycle without tool fields → treat as start.
    if string_field_any(input, &["conversationId", "conversation_id"]).is_some() {
        return Some("PreInvocation".to_string());
    }
    None
}

fn string_field_any(input: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| string_field(input, key))
}

/// Whether the payload can genuinely come from `agent`. Guards against CLIs
/// that execute other agents' hook configs with their own payload shape
/// (Grok reads ~/.claude/settings.json and ~/.cursor/hooks.json).
fn payload_matches_agent(agent: AgentKind, input: &serde_json::Value) -> bool {
    match agent {
        // Claude Code always wraps hook payloads in snake_case. A camelCase-
        // only hookEventName means another CLI invoked our handler.
        AgentKind::Claude => string_field(input, "hook_event_name").is_some(),
        // Every genuine Cursor hook payload carries conversation_id.
        AgentKind::Cursor => string_field(input, "conversation_id").is_some(),
        _ => true,
    }
}

/// Grok wraps the submitted text in `<user_query>` tags in hook payloads;
/// strip the wrapper so the monitor shows the raw user text.
fn unwrap_grok_user_query(prompt: &str) -> String {
    let inner = prompt
        .trim()
        .strip_prefix("<user_query>")
        .and_then(|rest| rest.strip_suffix("</user_query>"))
        .map(str::trim)
        .filter(|text| !text.is_empty());
    inner.unwrap_or(prompt).to_owned()
}

/// Grok fires a second Stop carrying `reason: "shutdown"` when the session
/// closes (the per-turn Stop already ended the row).
fn is_grok_session_close(input: &serde_json::Value) -> bool {
    string_field_any(input, &["reason"]).as_deref() == Some("shutdown")
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
    let raw = string_field(input, "cwd")
        .or_else(|| {
            input
                .get("workspace_roots")
                .and_then(serde_json::Value::as_array)
                .and_then(|roots| roots.first())
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            // Antigravity: workspacePaths is an array of absolute dirs.
            input
                .get("workspacePaths")
                .or_else(|| input.get("workspace_paths"))
                .and_then(serde_json::Value::as_array)
                .and_then(|roots| roots.first())
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })?;
    crate::paths::normalize_project_path_display(&raw)
}

fn last_transcript_user_prompt(path: &str) -> Option<String> {
    last_transcript_role(path, "USER_INPUT").map(|text| {
        crate::session::antigravity::unwrap_user_request(&text)
    })
}

fn last_transcript_assistant_reply(path: &str) -> Option<String> {
    last_transcript_role(path, "PLANNER_RESPONSE")
}

fn last_transcript_role(path: &str, want_type: &str) -> Option<String> {
    use std::io::{BufRead, BufReader};
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last = None;
    for line in reader.lines().flatten() {
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some(want_type) {
            continue;
        }
        if let Some(content) = value
            .get("content")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            last = Some(content.to_string());
        }
    }
    last
}

fn assistant_reply_field(agent: AgentKind, input: &serde_json::Value) -> Option<String> {
    // Kiro CLI 2.x agent-embedded hooks send `assistant_response` on stop;
    // IDE / CLI 3.0 / KAS v2 use last_assistant_message (snake or camel).
    let reply = string_field_any(
        input,
        &[
            "last_assistant_message",
            "lastAssistantMessage",
            "assistant_response",
            "assistantResponse",
        ],
    );
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

/// Map Antigravity product data dir to a SessionSource. Check the longer
/// product names first so `antigravity-cli` is not mistaken for bare
/// `antigravity`.
fn detect_antigravity_source(input: &serde_json::Value) -> SessionSource {
    let path = string_field_any(
        input,
        &[
            "transcriptPath",
            "transcript_path",
            "artifactDirectoryPath",
            "artifact_directory_path",
        ],
    )
    .unwrap_or_default()
    .replace('\\', "/")
    .to_ascii_lowercase();
    antigravity_source_from_path(&path)
}

fn antigravity_source_from_path(path: &str) -> SessionSource {
    if path.contains("antigravity-cli") {
        SessionSource::Terminal
    } else if path.contains("antigravity-ide") {
        SessionSource::AntigravityIde
    } else if path.contains("antigravity") {
        SessionSource::Antigravity
    } else {
        // Unknown path: prefer the desktop product (majority of local data).
        SessionSource::Antigravity
    }
}

fn monitor_root() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".agent-hub").join("session-monitor"))
        .ok_or_else(|| "home directory is unavailable".to_string())
}

// --- Kimi sub-agent Stop filtering -----------------------------------------
// One marker file per in-flight sub-agent, under
// `~/.agent-hub/session-monitor/kimi-subagents/<session-id>/<millis>-<uuid>`.
// File create/remove avoids the read-modify-write races a counter file would
// have when parallel sub-agents finish at the same time.

fn kimi_subagent_dir(root: &std::path::Path, session_id: &str) -> PathBuf {
    let safe: String = session_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    root.join("kimi-subagents").join(safe)
}

fn mark_kimi_subagent(session_id: &str) -> Result<(), String> {
    mark_kimi_subagent_in(&monitor_root()?, session_id)
}

fn mark_kimi_subagent_in(root: &std::path::Path, session_id: &str) -> Result<(), String> {
    let dir = kimi_subagent_dir(root, session_id);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("unable to create sub-agent marker directory: {error}"))?;
    let marker = dir.join(format!("{}-{}", now_millis(), Uuid::new_v4()));
    fs::write(&marker, []).map_err(|error| format!("unable to create sub-agent marker: {error}"))?;
    Ok(())
}

fn clear_kimi_subagent(session_id: &str) -> Result<(), String> {
    clear_kimi_subagent_in(&monitor_root()?, session_id)
}

fn clear_kimi_subagent_in(root: &std::path::Path, session_id: &str) -> Result<(), String> {
    let dir = kimi_subagent_dir(root, session_id);
    let mut markers = list_kimi_markers(&dir);
    markers.sort();
    if let Some(oldest) = markers.first() {
        let _ = fs::remove_file(oldest);
    }
    Ok(())
}

/// True while any non-stale sub-agent marker exists for the session. Stale
/// markers (SubagentStop lost to a kill/crash) are pruned here so a main-turn
/// Stop is suppressed for at most KIMI_SUBAGENT_MARKER_TTL_MILLIS.
fn kimi_subagent_active(session_id: &str) -> bool {
    monitor_root()
        .map(|root| kimi_subagent_active_in(&root, session_id))
        .unwrap_or(false)
}

fn kimi_subagent_active_in(root: &std::path::Path, session_id: &str) -> bool {
    let dir = kimi_subagent_dir(root, session_id);
    let cutoff = now_millis() - KIMI_SUBAGENT_MARKER_TTL_MILLIS;
    let mut active = false;
    for marker in list_kimi_markers(&dir) {
        let created = marker
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.split('-').next())
            .and_then(|millis| millis.parse::<i64>().ok());
        match created {
            Some(millis) if millis >= cutoff => active = true,
            _ => {
                let _ = fs::remove_file(&marker);
            }
        }
    }
    active
}

fn list_kimi_markers(dir: &std::path::Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .collect()
        })
        .unwrap_or_default()
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

/// Remember a session whose events must be dropped. Two users: Codex desktop
/// internal turns (removed again by the matching Stop, see
/// take_ignored_session) and Grok sub-agent child sessions (kept until the
/// cap evicts them — a child session never fires a stop we could hook).
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

/// Return whether the session was marked as ignored, without removing it.
fn is_session_ignored(session_id: &str) -> bool {
    load_ignored_sessions().iter().any(|id| id == session_id)
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

/// Append one line with a single write_all. write_fmt (writeln!) chunks large
/// lines into multiple write(2) syscalls, and concurrent hook processes (Grok
/// fires both its own hook and the Claude hook at session close) interleave
/// those chunks and corrupt the log.
fn append_line(path: &std::path::Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let mut bytes = Vec::with_capacity(line.len() + 1);
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
        let _ = file.write_all(&bytes);
    }
}

/// Always-on breadcrumb for capture failures (empty stdin, bad JSON, IO).
/// Lives next to hook-debug.jsonl so Windows users can diagnose silent hooks
/// without enabling AGENT_HUB_HOOK_DEBUG.
fn log_capture_error(agent: AgentKind, error: &str) {
    let Ok(path) = monitor_root().map(|root| root.join("hook-capture-error.log")) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    append_line(
        &path,
        &format!("{} agent={} error={}", now_millis(), agent.as_str(), error),
    );
}

/// Development aid: append every raw hook payload to
/// `~/.agent-hub/session-monitor/hook-debug.jsonl`. Active in debug builds or
/// when AGENT_HUB_HOOK_DEBUG is set, so release builds never write it unless
/// explicitly requested. Payloads may contain user prompts — local debugging
/// only.
fn debug_dump_payload(agent: AgentKind, bytes: &[u8]) {
    let enabled = cfg!(debug_assertions)
        || std::env::var_os("AGENT_HUB_HOOK_DEBUG").is_some_and(|value| !value.is_empty());
    if !enabled {
        return;
    }
    let Ok(path) = monitor_root().map(|root| root.join("hook-debug.jsonl")) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let raw: serde_json::Value = serde_json::from_slice(bytes).unwrap_or_default();
    // Huge payloads (base64 screenshots in Kimi prompts) would bury the log;
    // keep a bounded preview instead of the full JSON.
    const MAX_DUMP_BYTES: usize = 64 * 1024;
    let line = if bytes.len() <= MAX_DUMP_BYTES {
        serde_json::json!({
            "ts": now_millis(),
            "agent": agent.as_str(),
            "raw": raw,
        })
        .to_string()
    } else {
        serde_json::json!({
            "ts": now_millis(),
            "agent": agent.as_str(),
            "raw_bytes": bytes.len(),
            "raw_preview": String::from_utf8_lossy(&bytes[..MAX_DUMP_BYTES]),
        })
        .to_string()
    };
    append_line(&path, &line);
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
    fn prompt_field_skips_kimi_image_url_parts() {
        // Real Kimi payload when the user pastes a screenshot: the image rides
        // along as a base64 image_url part, the text part still carries the
        // question (this payload shape is also why the stdin cap is 8 MiB).
        let input = serde_json::json!({
            "prompt": [
                {"type": "image_url", "imageUrl": {"url": "data:image/png;base64,AAAA"}},
                {"type": "text", "text": "这张图里哪里不对"}
            ]
        });
        assert_eq!(prompt_field(&input).as_deref(), Some("这张图里哪里不对"));
    }

    #[test]
    fn grok_session_close_detection() {
        assert!(is_grok_session_close(&serde_json::json!({"reason": "shutdown"})));
        assert!(!is_grok_session_close(&serde_json::json!({})));
        assert!(!is_grok_session_close(&serde_json::json!({"reason": "complete"})));
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
    fn antigravity_path_distinguishes_cli_desktop_and_ide() {
        assert_eq!(
            antigravity_source_from_path(
                "/Users/x/.gemini/antigravity-cli/brain/id/.system_generated/logs/transcript.jsonl"
            ),
            SessionSource::Terminal
        );
        assert_eq!(
            antigravity_source_from_path(
                "/Users/x/.gemini/antigravity/brain/id/.system_generated/logs/transcript.jsonl"
            ),
            SessionSource::Antigravity
        );
        assert_eq!(
            antigravity_source_from_path(
                "/Users/x/.gemini/antigravity-ide/brain/id/.system_generated/logs/transcript.jsonl"
            ),
            SessionSource::AntigravityIde
        );
        // Longer product names must win over the bare "antigravity" segment.
        assert_eq!(
            antigravity_source_from_path("C:\\Users\\x\\.gemini\\antigravity-cli\\brain\\x"),
            SessionSource::Terminal
        );
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
    fn provenance_guard_rejects_foreign_payloads() {
        // Genuine Claude Code payload: snake_case hook_event_name.
        let claude = serde_json::json!({"hook_event_name": "Stop", "session_id": "s1"});
        assert!(payload_matches_agent(AgentKind::Claude, &claude));
        // Grok running our Claude hook with its own camelCase payload.
        let grok = serde_json::json!({"hookEventName": "stop", "sessionId": "s2"});
        assert!(!payload_matches_agent(AgentKind::Claude, &grok));
        // Genuine Cursor payload carries conversation_id; a Grok payload
        // invoked through ~/.cursor/hooks.json does not.
        let cursor = serde_json::json!({"hook_event_name": "stop", "conversation_id": "c1"});
        assert!(payload_matches_agent(AgentKind::Cursor, &cursor));
        assert!(!payload_matches_agent(AgentKind::Cursor, &grok));
        // Other agents are not cross-invoked and pass unconditionally.
        assert!(payload_matches_agent(AgentKind::Kimi, &grok));
        assert!(payload_matches_agent(AgentKind::Grok, &grok));
        assert!(payload_matches_agent(AgentKind::Codex, &grok));
    }

    #[test]
    fn grok_user_query_wrapper_is_stripped() {
        assert_eq!(
            unwrap_grok_user_query("<user_query>\n帮我修一下这个 bug\n</user_query>"),
            "帮我修一下这个 bug"
        );
        // Untagged prompts (and empty wrappers) pass through untouched.
        assert_eq!(unwrap_grok_user_query("plain prompt"), "plain prompt");
        assert_eq!(
            unwrap_grok_user_query("<user_query>\n</user_query>"),
            "<user_query>\n</user_query>"
        );
    }

    #[test]
    fn zcode_payload_fields_are_extracted() {
        // ZCode wraps hook payloads in snake_case (with camelCase aliases):
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
            assistant_reply_field(AgentKind::ZCode, &stop_input).as_deref(),
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
            assistant_reply_field(AgentKind::ZCode, &aliased).as_deref(),
            Some("done")
        );

        // Kiro CLI 2.x agent-embedded stop payload.
        let kiro_stop = serde_json::json!({
            "hook_event_name": "stop",
            "cwd": "/tmp",
            "assistant_response": "pong"
        });
        assert_eq!(
            assistant_reply_field(AgentKind::Kiro, &kiro_stop).as_deref(),
            Some("pong")
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

    #[test]
    fn kimi_subagent_markers_track_parallel_subagents() {
        let root = std::env::temp_dir().join(format!("agent-hub-test-{}", Uuid::new_v4()));
        let session = "session_test";
        assert!(!kimi_subagent_active_in(&root, session));

        // Two parallel sub-agents: the first SubagentStop must not clear the
        // second sub-agent's marker.
        mark_kimi_subagent_in(&root, session).unwrap();
        mark_kimi_subagent_in(&root, session).unwrap();
        assert!(kimi_subagent_active_in(&root, session));
        clear_kimi_subagent_in(&root, session).unwrap();
        assert!(kimi_subagent_active_in(&root, session));
        clear_kimi_subagent_in(&root, session).unwrap();
        assert!(!kimi_subagent_active_in(&root, session));

        // SubagentStop without a matching start is a no-op.
        clear_kimi_subagent_in(&root, session).unwrap();
        assert!(!kimi_subagent_active_in(&root, session));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn kimi_subagent_markers_isolate_sessions_and_prune_stale() {
        let root = std::env::temp_dir().join(format!("agent-hub-test-{}", Uuid::new_v4()));
        mark_kimi_subagent_in(&root, "session_a").unwrap();
        assert!(kimi_subagent_active_in(&root, "session_a"));
        // A marker must never leak into another session's check.
        assert!(!kimi_subagent_active_in(&root, "session_b"));

        // A marker older than the TTL (lost SubagentStop) is pruned and no
        // longer suppresses the main turn's Stop.
        let dir = kimi_subagent_dir(&root, "session_stale");
        fs::create_dir_all(&dir).unwrap();
        let stale = dir.join(format!("{}-{}", now_millis() - 2 * KIMI_SUBAGENT_MARKER_TTL_MILLIS, Uuid::new_v4()));
        fs::write(&stale, []).unwrap();
        assert!(!kimi_subagent_active_in(&root, "session_stale"));
        assert!(!stale.exists());

        let _ = fs::remove_dir_all(&root);
    }
}

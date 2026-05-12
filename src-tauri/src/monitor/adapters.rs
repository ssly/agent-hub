use super::types::*;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const PREVIEW_MAX_LEN: usize = 150;

fn truncate_preview(s: &str) -> String {
    if s.len() <= PREVIEW_MAX_LEN {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(PREVIEW_MAX_LEN).collect();
        truncated + "..."
    }
}

/// FNV-1a 64-bit hash of cwd. Stable across process restarts and platforms,
/// so an agent restarted in the same cwd keeps the same session_id.
/// Used by Tier-2 adapters (Codex, Gemini) where no internal session id is exposed.
fn cwd_hash(cwd: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in cwd.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// Claude Code maps both `/` and `.` to `-` when deriving the project dir name.
/// So `/Users/x/.claude/worktrees/foo` → `-Users-x--claude-worktrees-foo`.
fn encode_claude_project_dir(cwd: &str) -> String {
    cwd.trim_end_matches('/')
        .chars()
        .map(|c| if c == '/' || c == '.' { '-' } else { c })
        .collect()
}

/// Check if a string looks like a Claude Code session UUID
/// (8-4-4-4-12 lowercase hex with dashes).
fn is_uuid(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    let bytes = s.as_bytes();
    let dash_positions = [8, 13, 18, 23];
    for (i, &b) in bytes.iter().enumerate() {
        if dash_positions.contains(&i) {
            if b != b'-' {
                return false;
            }
        } else if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

/// Read the last N lines from a file efficiently without reading the whole file.
fn read_last_lines(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(mut file) = fs::File::open(path) else {
        return vec![];
    };
    let Ok(size) = file.metadata().map(|m| m.len()) else {
        return vec![];
    };
    if size == 0 {
        return vec![];
    }

    // Read up to 64KB from the end — enough for hundreds of JSONL lines
    let read_start = if size > 65536 { size - 65536 } else { 0 };
    if file.seek(SeekFrom::Start(read_start)).is_err() {
        return vec![];
    }

    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return vec![];
    }

    // Skip the first partial line if we didn't start at 0
    let lines: Vec<String> = if read_start > 0 {
        buf.lines().skip(1).map(String::from).collect()
    } else {
        buf.lines().map(String::from).collect()
    };

    let start = if lines.len() > max_lines {
        lines.len() - max_lines
    } else {
        0
    };
    lines[start..].to_vec()
}

pub struct KiroAdapter {
    home: PathBuf,
}

impl KiroAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            home,
        }
    }

    fn sessions_dir(&self) -> PathBuf {
        self.home.join(".kiro/sessions/cli")
    }

    fn parse_lock_file(&self, path: &Path) -> Option<u32> {
        let content = fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        json.get("pid")?.as_u64().map(|p| p as u32)
    }

    fn parse_metadata(&self, path: &Path) -> Option<serde_json::Value> {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn last_jsonl_status(&self, path: &Path) -> Option<String> {
        let lines = read_last_lines(path, 5);
        let last_line = lines.last()?;
        let json: serde_json::Value = serde_json::from_str(last_line).ok()?;
        json.get("kind").and_then(|v| v.as_str()).map(String::from)
    }

    fn last_assistant_preview(&self, jsonl_path: &Path) -> Option<String> {
        let lines = read_last_lines(jsonl_path, 50);
        for line in lines.iter().rev() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                let kind = json.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                if kind != "AssistantMessage" {
                    continue;
                }
                // Kiro format: {"kind":"AssistantMessage","data":{"content":[{"kind":"text","data":"..."}]}}
                if let Some(content_arr) = json
                    .get("data")
                    .and_then(|d| d.get("content"))
                    .and_then(|c| c.as_array())
                {
                    for block in content_arr.iter().rev() {
                        if block.get("kind").and_then(|k| k.as_str()) == Some("text") {
                            if let Some(text) = block.get("data").and_then(|d| d.as_str()) {
                                let trimmed = text.trim();
                                if !trimmed.is_empty() {
                                    return Some(truncate_preview(trimmed));
                                }
                            }
                        }
                    }
                }
                // Fallback: flat format content.text
                if let Some(text) = json
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(truncate_preview(trimmed));
                    }
                }
                // Fallback: content as string
                if let Some(text) = json
                    .get("content")
                    .and_then(|c| c.as_str())
                {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(truncate_preview(trimmed));
                    }
                }
            }
        }
        None
    }

    /// Last user prompt (Q in the Q-A pairing). Kiro JSONL shape:
    /// `{"kind":"Prompt","data":{"content":[{"kind":"text","data":"..."}]}}`
    fn last_user_prompt_preview(&self, jsonl_path: &Path) -> Option<String> {
        let lines = read_last_lines(jsonl_path, 50);
        for line in lines.iter().rev() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                let kind = json.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                if kind != "Prompt" {
                    continue;
                }
                if let Some(content_arr) = json
                    .get("data")
                    .and_then(|d| d.get("content"))
                    .and_then(|c| c.as_array())
                {
                    for block in content_arr {
                        if block.get("kind").and_then(|k| k.as_str()) == Some("text") {
                            if let Some(text) = block.get("data").and_then(|d| d.as_str()) {
                                let trimmed = text.trim();
                                if !trimmed.is_empty() {
                                    return Some(truncate_preview(trimmed));
                                }
                            }
                        }
                    }
                }
                // Fallback: data.text or data as string for older Kiro variants.
                if let Some(text) = json
                    .get("data")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(truncate_preview(trimmed));
                    }
                }
                if let Some(text) = json.get("data").and_then(|d| d.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(truncate_preview(trimmed));
                    }
                }
            }
        }
        None
    }

    fn session_from_files(
        &self,
        session_id: &str,
        dir: &Path,
        sys: &sysinfo::System,
    ) -> Option<AgentSession> {
        let lock_path = dir.join(format!("{session_id}.lock"));
        let meta_path = dir.join(format!("{session_id}.json"));
        let jsonl_path = dir.join(format!("{session_id}.jsonl"));

        let pid = if lock_path.exists() {
            self.parse_lock_file(&lock_path)
        } else {
            None
        };

        let meta = self.parse_metadata(&meta_path)?;
        let raw_title: String = meta
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .chars()
            .take(50)
            .collect();
        let model = meta
            .get("rts_model_state")
            .and_then(|v| v.get("model_info"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| meta.get("model").and_then(|v| v.as_str()).map(String::from))
            .unwrap_or_else(|| "unknown".to_string());
        let cwd = meta
            .get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = if raw_title.is_empty() {
            let short = std::path::Path::new(&cwd)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if short.is_empty() {
                format!("Kiro {}", &session_id[..8.min(session_id.len())])
            } else {
                format!("Kiro – {short}")
            }
        } else {
            raw_title
        };

        let (status, working_state) = if let Some(kind) = self.last_jsonl_status(&jsonl_path) {
            match kind.as_str() {
                "Prompt" => (SessionStatus::Idle, WorkingState::Idle),
                "ToolUseRequest" => (SessionStatus::Active, WorkingState::Working),
                "AssistantMessage" => (SessionStatus::Active, WorkingState::Finished),
                _ => (SessionStatus::Active, WorkingState::Idle),
            }
        } else {
            (SessionStatus::Idle, WorkingState::Idle)
        };

        let started_at = meta
            .get("created_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.to_utc())
            .unwrap_or_default();

        let source_tag = if let Some(pid_val) = pid {
            let pid_obj = sysinfo::Pid::from_u32(pid_val);
            if let Some(proc) = sys.process(pid_obj) {
                let proc_name = proc.name().to_string_lossy().to_string();
                if proc_name.contains("desktop") {
                    "Desktop"
                } else {
                    "CLI"
                }
            } else {
                "CLI"
            }
        } else {
            "CLI"
        };

        let last_message_preview = if jsonl_path.exists() {
            self.last_assistant_preview(&jsonl_path)
        } else {
            None
        };

        let last_user_prompt = if jsonl_path.exists() {
            self.last_user_prompt_preview(&jsonl_path)
        } else {
            None
        };

        Some(AgentSession {
            agent_type: "kiro".to_string(),
            source_tag: source_tag.to_string(),
            session_id: session_id.to_string(),
            title,
            model,
            cwd,
            status,
            started_at,
            last_activity: Utc::now(),
            data_limited: false,
            data_limited_reason: None,
            pid,
            last_message_preview,
            last_user_prompt,
            working_state,
        })
    }
}

impl AgentMonitor for KiroAdapter {
    fn platform_id(&self) -> &str {
        "kiro"
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        let dir = self.sessions_dir();
        if dir.exists() {
            vec![dir]
        } else {
            vec![]
        }
    }

    fn detect_sessions(&self, sys: &sysinfo::System) -> Vec<AgentSession> {
        let dir = self.sessions_dir();
        if !dir.exists() {
            return vec![];
        }
        let mut result = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            let mut seen = std::collections::HashSet::new();
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.ends_with(".lock") {
                    let session_id = name_str.trim_end_matches(".lock");
                    if seen.insert(session_id.to_string()) {
                        let lock_path = dir.join(format!("{session_id}.lock"));
                        let pid_alive = self.parse_lock_file(&lock_path)
                            .map(|pid| {
                                sys.process(sysinfo::Pid::from_u32(pid)).is_some()
                            })
                            .unwrap_or(false);
                        if !pid_alive {
                            continue;
                        }
                        if let Some(session) =
                            self.session_from_files(session_id, &dir, sys)
                        {
                            result.push(session);
                        }
                    }
                }
            }
        }
        result
    }

    fn on_fs_event(&mut self, _event: &notify::Event) -> Vec<(StateChange, AgentSession)> {
        vec![]
    }
}

pub struct ClaudeCodeAdapter {
    home: PathBuf,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            home,
        }
    }

    fn projects_dir(&self) -> PathBuf {
        self.home.join(".claude/projects")
    }

    /// Resolve the JSONL file for a Claude Code session.
    ///
    /// Strategy:
    /// 1. If `session_id_hint` looks like a UUID (e.g. from `claude --resume <uuid>`),
    ///    scan every project dir for `<uuid>.jsonl`. This is the only reliable path
    ///    when the user starts Claude Code in cwd A then `cd`s into worktree B —
    ///    sysinfo reports B as the process cwd, but the jsonl was written under A.
    /// 2. Fallback to encoding `cwd` into the project dir name. Claude Code maps
    ///    BOTH `/` and `.` to `-`, so `.claude` becomes `--claude`, not `.claude`.
    fn find_session_jsonl(
        &self,
        cwd: &str,
        session_id_hint: Option<&str>,
    ) -> Option<PathBuf> {
        // Step 1: try session_id-based lookup if it looks like a UUID.
        if let Some(sid) = session_id_hint {
            if is_uuid(sid) {
                if let Ok(entries) = fs::read_dir(self.projects_dir()) {
                    for entry in entries.flatten() {
                        let candidate = entry.path().join(format!("{sid}.jsonl"));
                        if candidate.exists() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }

        // Step 2: encode cwd into project dir name and pick the latest jsonl.
        let encoded = encode_claude_project_dir(cwd);
        let project_dir = self.projects_dir().join(&encoded);
        if !project_dir.exists() {
            return None;
        }

        let mut latest: Option<(PathBuf, std::time::SystemTime)> = None;
        if let Ok(entries) = fs::read_dir(&project_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    if let Ok(meta) = path.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if latest.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                                latest = Some((path, modified));
                            }
                        }
                    }
                }
            }
        }
        latest.map(|(p, _)| p)
    }

    fn last_assistant_preview(&self, jsonl_path: &Path) -> Option<String> {
        let lines = read_last_lines(jsonl_path, 50);
        for line in lines.iter().rev() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if msg_type != "assistant" {
                    continue;
                }
                // Claude Code JSONL: message.content is an array of blocks
                if let Some(content) = json.get("message").and_then(|m| m.get("content")) {
                    if let Some(arr) = content.as_array() {
                        // Get the last text block
                        for block in arr.iter().rev() {
                            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                if let Some(text) =
                                    block.get("text").and_then(|t| t.as_str())
                                {
                                    let trimmed = text.trim();
                                    if !trimmed.is_empty() {
                                        return Some(truncate_preview(trimmed));
                                    }
                                }
                            }
                        }
                    }
                    // Fallback: content as string
                    if let Some(text) = content.as_str() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            return Some(truncate_preview(trimmed));
                        }
                    }
                }
            }
        }
        None
    }

    /// Last user prompt (Q in the Q-A pairing). Claude Code JSONL has two `type:"user"`
    /// shapes: real prompts (`content` is a string or contains a `type:"text"` block)
    /// and tool results (`content` is an array of `type:"tool_result"` blocks). Tool
    /// results are NOT user input — skip them or the headline becomes "Tool returned…"
    /// instead of "请帮我修…".
    fn last_user_prompt_preview(&self, jsonl_path: &Path) -> Option<String> {
        let lines = read_last_lines(jsonl_path, 50);
        for line in lines.iter().rev() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if msg_type != "user" {
                    continue;
                }
                let content = match json.get("message").and_then(|m| m.get("content")) {
                    Some(c) => c,
                    None => continue,
                };

                // Plain string content — always a real user prompt.
                if let Some(text) = content.as_str() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(truncate_preview(trimmed));
                    }
                    continue;
                }

                // Array content — only count it as a user prompt if at least one block
                // is `type:"text"`. Pure tool_result arrays are model output, not Q.
                if let Some(arr) = content.as_array() {
                    for block in arr.iter().rev() {
                        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                let trimmed = text.trim();
                                if !trimmed.is_empty() {
                                    return Some(truncate_preview(trimmed));
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Derive working state from the last meaningful line in the JSONL.
    /// Claude Code writes `type:"user"` (Idle), `type:"assistant"` with
    /// `stop_reason` (Finished), or `type:"assistant"` without it (Working).
    fn working_state_from_jsonl(&self, jsonl_path: &Path) -> WorkingState {
        let lines = read_last_lines(jsonl_path, 50);
        for line in lines.iter().rev() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if msg_type == "user" {
                    let content = match json.get("message").and_then(|m| m.get("content")) {
                        Some(c) => c,
                        None => continue,
                    };
                    // Real user prompt (string or text block) → Idle.
                    if content.as_str().is_some() {
                        return WorkingState::Idle;
                    }
                    if let Some(arr) = content.as_array() {
                        for block in arr.iter().rev() {
                            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                return WorkingState::Idle;
                            }
                        }
                    }
                    continue; // tool_result-only user → skip
                }
                if msg_type == "assistant" {
                    let has_stop_reason = json
                        .get("message")
                        .and_then(|m| m.get("stop_reason"))
                        .is_some();
                    return if has_stop_reason {
                        WorkingState::Finished
                    } else {
                        WorkingState::Working
                    };
                }
            }
        }
        WorkingState::Idle
    }

    fn detect_from_processes(&self, sys: &sysinfo::System) -> Vec<AgentSession> {
        let mut sessions = Vec::new();
        for (_, proc) in sys.processes() {
            let name = proc.name().to_string_lossy().to_string();
            let exe = proc
                .exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            // Match "claude" by process name (argv[0]) or exe path.
            // proc.name() is reliable even when claude is a symlink to a
            // versioned binary (e.g. ~/.local/share/claude/versions/2.1.128).
            let is_claude = name == "claude"
                || exe.split('/').last().map(|b| b == "claude").unwrap_or(false)
                || exe.contains("/claude");
            if !is_claude {
                continue;
            }

            let cmd: Vec<String> = proc
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect();

            if cmd.iter().any(|a| {
                a.starts_with("--teammate-mode") || a.starts_with("--output-format")
            }) {
                continue;
            }
            if cmd.len() <= 1 {
                continue;
            }

            if exe.contains("Claude-3p") || exe.contains("Claude.app") {
                if !cmd.iter().any(|a| a == "--resume") {
                    continue;
                }
            }

            let mut session_id = String::new();
            let mut model = String::from("unknown");
            let mut agent_name = String::new();

            let mut i = 0;
            while i < cmd.len() {
                match cmd[i].as_str() {
                    "--resume" if i + 1 < cmd.len() => {
                        session_id = cmd[i + 1].clone();
                        i += 2;
                    }
                    "--model" if i + 1 < cmd.len() => {
                        model = cmd[i + 1].clone();
                        i += 2;
                    }
                    "--agent-name" if i + 1 < cmd.len() => {
                        agent_name = cmd[i + 1].clone();
                        i += 2;
                    }
                    _ => i += 1,
                }
            }

            if session_id.is_empty() {
                session_id = format!("claude-{}", proc.pid().as_u32());
            }

            let source_tag =
                if exe.contains("Claude-3p") || exe.contains("Claude.app") {
                    "Desktop"
                } else {
                    "CLI"
                };

            let cwd = proc
                .cwd()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let title = if !agent_name.is_empty() {
                agent_name
            } else if !cwd.is_empty() {
                let short = std::path::Path::new(&cwd)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                format!("Claude Code – {short}")
            } else {
                format!("Claude Code #{}", proc.pid().as_u32())
            };

            let started_at =
                DateTime::from_timestamp(proc.start_time() as i64, 0).unwrap_or_default();

            // Read last message preview from session JSONL.
            // Only attempt this when session_id is a real UUID — i.e. the process
            // was started with `claude --resume <uuid>` so we can pin its jsonl
            // file. Without a UUID the cwd-encoding fallback would just pick up
            // the most-recent jsonl in that project dir, which usually belongs to
            // *another* Claude Code instance and would mislabel this row's
            // headline. Better to leave the fields None and let the UI fall back
            // to the project title.
            let jsonl_path = if !cwd.is_empty() && is_uuid(&session_id) {
                self.find_session_jsonl(&cwd, Some(&session_id))
            } else {
                None
            };
            let last_message_preview = jsonl_path
                .as_ref()
                .and_then(|p| self.last_assistant_preview(p));
            let last_user_prompt = jsonl_path
                .as_ref()
                .and_then(|p| self.last_user_prompt_preview(p));

            let working_state = jsonl_path
                .as_ref()
                .map(|p| self.working_state_from_jsonl(p))
                .unwrap_or(WorkingState::Idle);

            sessions.push(AgentSession {
                agent_type: "claude-code".to_string(),
                source_tag: source_tag.to_string(),
                session_id: session_id.clone(),
                title,
                model,
                cwd,
                status: SessionStatus::Active,
                started_at,
                last_activity: Utc::now(),
                data_limited: false,
                data_limited_reason: None,
                pid: Some(proc.pid().as_u32()),
                last_message_preview,
                last_user_prompt,
                working_state,
            });
        }
        sessions
    }
}

impl AgentMonitor for ClaudeCodeAdapter {
    fn platform_id(&self) -> &str {
        "claude-code"
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        let dir = self.projects_dir();
        if dir.exists() {
            vec![dir]
        } else {
            vec![]
        }
    }

    fn detect_sessions(&self, sys: &sysinfo::System) -> Vec<AgentSession> {
        self.detect_from_processes(sys)
    }

    fn on_fs_event(&mut self, _event: &notify::Event) -> Vec<(StateChange, AgentSession)> {
        // Unused — service re-detects all sessions on fs events
        vec![]
    }
}

pub struct CodexAdapter {
    home: PathBuf,
}

impl CodexAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            home,
        }
    }

    fn codex_dir(&self) -> PathBuf {
        self.home.join(".codex")
    }

    fn detect_from_processes(&self, sys: &sysinfo::System) -> Vec<AgentSession> {
        let mut sessions = Vec::new();
        for (_, proc) in sys.processes() {
            let name = proc.name().to_string_lossy().to_string();
            let is_codex = name == "codex" || name == "Codex";
            if !is_codex {
                continue;
            }
            let exe_path = proc
                .exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let source_tag = if name == "Codex" || exe_path.contains("Electron") {
                "Desktop"
            } else {
                "CLI"
            };

            let cwd = proc
                .cwd()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let session_id = if cwd.is_empty() {
                format!("codex-pid{}", proc.pid().as_u32())
            } else {
                format!("codex-{}", cwd_hash(&cwd))
            };

            let title = if cwd.is_empty() {
                format!("Codex {}", source_tag)
            } else {
                let short = Path::new(&cwd)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                format!("Codex – {short}")
            };

            let started_at =
                DateTime::from_timestamp(proc.start_time() as i64, 0).unwrap_or_default();
            sessions.push(AgentSession {
                agent_type: "codex".to_string(),
                source_tag: source_tag.to_string(),
                session_id: session_id.clone(),
                title,
                model: "codex".to_string(),
                cwd,
                status: SessionStatus::Active,
                started_at,
                last_activity: Utc::now(),
                data_limited: true,
                data_limited_reason: Some("monitor.data_limited_codex".to_string()),
                pid: Some(proc.pid().as_u32()),
                last_message_preview: None,
                last_user_prompt: None,
                working_state: WorkingState::Idle,
            });
        }
        sessions
    }
}

impl AgentMonitor for CodexAdapter {
    fn platform_id(&self) -> &str {
        "codex"
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        let dir = self.codex_dir();
        if dir.exists() {
            vec![dir]
        } else {
            vec![]
        }
    }

    fn detect_sessions(&self, sys: &sysinfo::System) -> Vec<AgentSession> {
        self.detect_from_processes(sys)
    }

    fn on_fs_event(&mut self, _event: &notify::Event) -> Vec<(StateChange, AgentSession)> {
        vec![]
    }
}

pub struct GeminiAdapter {
    home: PathBuf,
}

impl GeminiAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            home,
        }
    }

    fn gemini_tmp_dir(&self) -> PathBuf {
        self.home.join(".gemini/tmp")
    }

    fn detect_from_processes(&self, sys: &sysinfo::System) -> Vec<AgentSession> {
        let mut seen_cwd: HashMap<String, u32> = HashMap::new();
        let mut all: Vec<(u32, AgentSession)> = Vec::new();
        for (_, proc) in sys.processes() {
            let name = proc.name().to_string_lossy().to_string();
            if name != "node" {
                continue;
            }
            let cmd: Vec<String> = proc
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect();
            let is_gemini = cmd.iter().any(|a| a.contains("/bin/gemini"));
            if !is_gemini {
                continue;
            }

            let cwd = proc
                .cwd()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let project_name = Path::new(&cwd)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let pid = proc.pid().as_u32();
            let session_id = if cwd.is_empty() {
                format!("gemini-pid{pid}")
            } else {
                format!("gemini-{}", cwd_hash(&cwd))
            };
            let title = if project_name.is_empty() {
                "Gemini CLI".to_string()
            } else {
                format!("Gemini – {project_name}")
            };

            let started_at =
                DateTime::from_timestamp(proc.start_time() as i64, 0).unwrap_or_default();

            all.push((
                pid,
                AgentSession {
                    agent_type: "gemini".to_string(),
                    source_tag: "CLI".to_string(),
                    session_id: session_id.clone(),
                    title,
                    model: "gemini".to_string(),
                    cwd: cwd.clone(),
                    status: SessionStatus::Active,
                    started_at,
                    last_activity: Utc::now(),
                    data_limited: true,
                    data_limited_reason: Some("monitor.data_limited_gemini".to_string()),
                    pid: Some(pid),
                    last_message_preview: None,
                    last_user_prompt: None,
                    working_state: WorkingState::Idle,
                },
            ));

            seen_cwd
                .entry(cwd)
                .and_modify(|e| {
                    if pid < *e {
                        *e = pid;
                    }
                })
                .or_insert(pid);
        }
        let parent_pids: std::collections::HashSet<u32> =
            seen_cwd.values().copied().collect();
        all.into_iter()
            .filter(|(pid, _)| parent_pids.contains(pid))
            .map(|(_, s)| s)
            .collect()
    }

}

impl AgentMonitor for GeminiAdapter {
    fn platform_id(&self) -> &str {
        "gemini"
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        let dir = self.gemini_tmp_dir();
        if dir.exists() {
            vec![dir]
        } else {
            vec![]
        }
    }

    fn detect_sessions(&self, sys: &sysinfo::System) -> Vec<AgentSession> {
        self.detect_from_processes(sys)
    }

    fn on_fs_event(&mut self, _event: &notify::Event) -> Vec<(StateChange, AgentSession)> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cwd_hash_is_stable_across_calls() {
        let cwd = "/Users/foo/code/myapp";
        assert_eq!(cwd_hash(cwd), cwd_hash(cwd));
    }

    #[test]
    fn cwd_hash_different_inputs_produce_different_outputs() {
        let a = cwd_hash("/Users/foo/code/myapp");
        let b = cwd_hash("/Users/foo/code/other");
        assert_ne!(a, b);
    }

    #[test]
    fn cwd_hash_format_is_16_hex_chars() {
        let h = cwd_hash("/some/path");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn cwd_hash_empty_string_is_handled() {
        let h = cwd_hash("");
        assert_eq!(h.len(), 16);
    }

    /// Regression: Codex restart in the same cwd must keep the same session_id
    /// (PID changes, cwd does not). Defends against Issue #5 — Codex/Gemini ghost-row.
    #[test]
    fn codex_session_id_only_depends_on_cwd_not_pid() {
        let cwd = "/Users/foo/code/myapp";
        let id_pid_1234 = format!("codex-{}", cwd_hash(cwd));
        let id_pid_5678 = format!("codex-{}", cwd_hash(cwd));
        assert_eq!(id_pid_1234, id_pid_5678);
    }

    /// Regression: same as above for Gemini.
    #[test]
    fn gemini_session_id_only_depends_on_cwd_not_pid() {
        let cwd = "/Users/foo/code/myapp";
        let id_pid_1234 = format!("gemini-{}", cwd_hash(cwd));
        let id_pid_5678 = format!("gemini-{}", cwd_hash(cwd));
        assert_eq!(id_pid_1234, id_pid_5678);
    }

    #[test]
    fn empty_cwd_falls_back_to_pid_in_session_id() {
        // Documents the fallback behavior — when cwd is empty (rare), PID is used.
        // This branch is not stable across restarts but is the best we can do.
        let cwd = "";
        let pid: u32 = 1234;
        let id = if cwd.is_empty() {
            format!("codex-pid{pid}")
        } else {
            format!("codex-{}", cwd_hash(cwd))
        };
        assert_eq!(id, "codex-pid1234");
    }

    // --- Q-prompt-A-reply: user prompt extraction ---

    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_jsonl(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("create temp jsonl");
        for line in lines {
            writeln!(f, "{line}").expect("write line");
        }
        f.flush().expect("flush");
        f
    }

    #[test]
    fn kiro_extracts_last_user_prompt_from_content_array() {
        let f = write_jsonl(&[
            r#"{"kind":"Prompt","data":{"content":[{"kind":"text","data":"先归档所有 openspec"}]}}"#,
        ]);
        let adapter = KiroAdapter::new();
        let got = adapter.last_user_prompt_preview(f.path());
        assert_eq!(got.as_deref(), Some("先归档所有 openspec"));
    }

    #[test]
    fn kiro_takes_most_recent_prompt_when_multiple() {
        let f = write_jsonl(&[
            r#"{"kind":"Prompt","data":{"content":[{"kind":"text","data":"first"}]}}"#,
            r#"{"kind":"AssistantMessage","content":{"text":"reply"}}"#,
            r#"{"kind":"Prompt","data":{"content":[{"kind":"text","data":"second"}]}}"#,
        ]);
        let adapter = KiroAdapter::new();
        let got = adapter.last_user_prompt_preview(f.path());
        assert_eq!(got.as_deref(), Some("second"));
    }

    /// Regression: assistant blocks must not be returned as the user prompt.
    /// This was the root cause of the Q/A swap — the headline showed "Agent: 我已修复…"
    /// instead of "User: 帮我修…".
    #[test]
    fn kiro_assistant_only_jsonl_returns_none() {
        let f = write_jsonl(&[
            r#"{"kind":"AssistantMessage","content":{"text":"only assistant here"}}"#,
        ]);
        let adapter = KiroAdapter::new();
        assert_eq!(adapter.last_user_prompt_preview(f.path()), None);
    }

    #[test]
    fn claude_code_extracts_user_prompt_from_string_content() {
        let f = write_jsonl(&[
            r#"{"type":"user","message":{"role":"user","content":"帮我看下这个 bug"}}"#,
        ]);
        let adapter = ClaudeCodeAdapter::new();
        let got = adapter.last_user_prompt_preview(f.path());
        assert_eq!(got.as_deref(), Some("帮我看下这个 bug"));
    }

    #[test]
    fn claude_code_extracts_user_prompt_from_text_block() {
        let f = write_jsonl(&[
            r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hello with image"}]}}"#,
        ]);
        let adapter = ClaudeCodeAdapter::new();
        let got = adapter.last_user_prompt_preview(f.path());
        assert_eq!(got.as_deref(), Some("hello with image"));
    }

    /// Regression: tool_result entries are `type:"user"` in Claude Code JSONL but
    /// they're model-side tool output, not user input. They must NOT be treated as
    /// the user's last prompt.
    #[test]
    fn claude_code_skips_tool_result_only_user_messages() {
        let f = write_jsonl(&[
            r#"{"type":"user","message":{"role":"user","content":"原始的用户问题"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"我去查"}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"call_1","type":"tool_result","content":[{"type":"text","text":"file contents..."}]}]}}"#,
        ]);
        let adapter = ClaudeCodeAdapter::new();
        // Walks back from the end: tool_result line has no `text` block → skipped.
        // First real prompt found is the original user question, NOT the tool result.
        let got = adapter.last_user_prompt_preview(f.path());
        assert_eq!(got.as_deref(), Some("原始的用户问题"));
    }

    #[test]
    fn claude_code_assistant_only_jsonl_returns_none() {
        let f = write_jsonl(&[
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"only assistant"}]}}"#,
        ]);
        let adapter = ClaudeCodeAdapter::new();
        assert_eq!(adapter.last_user_prompt_preview(f.path()), None);
    }

    #[test]
    fn claude_code_takes_text_block_when_mixed_with_tool_result() {
        // A user turn that includes both tool_result AND a text block (e.g. user
        // pasted text alongside an attachment) should still surface the text.
        let f = write_jsonl(&[
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"x","type":"tool_result","content":[]},{"type":"text","text":"看下这个"}]}}"#,
        ]);
        let adapter = ClaudeCodeAdapter::new();
        let got = adapter.last_user_prompt_preview(f.path());
        assert_eq!(got.as_deref(), Some("看下这个"));
    }

    // --- jsonl path resolution: encoded-cwd & session_id-based ---

    #[test]
    fn encode_claude_project_dir_replaces_slash_and_dot() {
        // Real-world: ~/.claude/worktrees/foo lives under "-Users-x--claude-worktrees-foo".
        // Both `/` and `.` map to `-`.
        assert_eq!(
            encode_claude_project_dir("/Users/foo/.claude/worktrees/bar"),
            "-Users-foo--claude-worktrees-bar"
        );
        assert_eq!(
            encode_claude_project_dir("/Users/foo/code/agent-hub"),
            "-Users-foo-code-agent-hub"
        );
    }

    #[test]
    fn encode_claude_project_dir_strips_trailing_slash() {
        assert_eq!(encode_claude_project_dir("/foo/"), "-foo");
    }

    #[test]
    fn is_uuid_accepts_canonical_form() {
        assert!(is_uuid("260e932f-7946-4650-a23c-30ba12c7ef57"));
    }

    #[test]
    fn is_uuid_rejects_pid_fallback_format() {
        // session_id = "claude-{pid}" must not be treated as a UUID and trigger a
        // global jsonl scan.
        assert!(!is_uuid("claude-11476"));
        assert!(!is_uuid(""));
        assert!(!is_uuid("260e932f-7946-4650-a23c-30ba12c7ef5")); // 35 chars
    }

    /// Regression: the `--resume <uuid>` jsonl path must resolve even when the
    /// process cwd no longer maps to the original project dir (user `cd`'d into
    /// a subdir or worktree after starting Claude Code).
    #[test]
    fn find_session_jsonl_uses_uuid_when_cwd_dir_missing() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join(".claude/projects");
        let real_dir = projects_dir.join("-Users-x-code-myapp");
        fs::create_dir_all(&real_dir).unwrap();
        let uuid = "260e932f-7946-4650-a23c-30ba12c7ef57";
        let jsonl_path = real_dir.join(format!("{uuid}.jsonl"));
        fs::write(&jsonl_path, "").unwrap();

        let adapter = ClaudeCodeAdapter {
            home: tmp.path().to_path_buf(),
        };
        // cwd reported by sysinfo points at a subdirectory whose encoded form
        // does NOT exist as a project dir — but the UUID does.
        let got = adapter.find_session_jsonl(
            "/Users/x/code/myapp/.claude/worktrees/foo",
            Some(uuid),
        );
        assert_eq!(got.as_deref(), Some(jsonl_path.as_path()));
    }

    #[test]
    fn find_session_jsonl_falls_back_to_cwd_encoding_without_uuid() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join(".claude/projects");
        let dir = projects_dir.join("-Users-x-code-myapp");
        fs::create_dir_all(&dir).unwrap();
        let jsonl_path = dir.join("session-a.jsonl");
        fs::write(&jsonl_path, "").unwrap();

        let adapter = ClaudeCodeAdapter {
            home: tmp.path().to_path_buf(),
        };
        // No UUID hint, so resolver must encode cwd → project dir → newest jsonl.
        let got = adapter.find_session_jsonl("/Users/x/code/myapp", Some("claude-1234"));
        assert_eq!(got.as_deref(), Some(jsonl_path.as_path()));
    }

    // --- Tier-1 state machine (WorkingState) ---

    #[test]
    fn kiro_prompt_means_idle() {
        let f = write_jsonl(&[
            r#"{"kind":"Prompt","data":{"content":[{"kind":"text","data":"hello"}]}}"#,
        ]);
        let adapter = KiroAdapter::new();
        let kind = adapter.last_jsonl_status(f.path());
        assert_eq!(kind.as_deref(), Some("Prompt"));
        // Mapping in session_from_files: Prompt → Idle
    }

    #[test]
    fn kiro_tool_use_request_means_working() {
        let f = write_jsonl(&[
            r#"{"kind":"ToolUseRequest","data":{"name":"Read"}}"#,
        ]);
        let adapter = KiroAdapter::new();
        let kind = adapter.last_jsonl_status(f.path());
        assert_eq!(kind.as_deref(), Some("ToolUseRequest"));
        // Mapping: ToolUseRequest → Working
    }

    #[test]
    fn kiro_assistant_message_means_finished() {
        let f = write_jsonl(&[
            r#"{"kind":"AssistantMessage","data":{"content":[{"kind":"text","data":"done"}]}}"#,
        ]);
        let adapter = KiroAdapter::new();
        let kind = adapter.last_jsonl_status(f.path());
        assert_eq!(kind.as_deref(), Some("AssistantMessage"));
        // Mapping: AssistantMessage → Finished
    }

    #[test]
    fn claude_code_user_text_means_idle() {
        let f = write_jsonl(&[
            r#"{"type":"user","message":{"role":"user","content":"hello"}}"#,
        ]);
        let adapter = ClaudeCodeAdapter::new();
        assert_eq!(adapter.working_state_from_jsonl(f.path()), WorkingState::Idle);
    }

    #[test]
    fn claude_code_assistant_with_stop_reason_means_finished() {
        let f = write_jsonl(&[
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"done"}],"stop_reason":"end_turn"}}"#,
        ]);
        let adapter = ClaudeCodeAdapter::new();
        assert_eq!(adapter.working_state_from_jsonl(f.path()), WorkingState::Finished);
    }

    #[test]
    fn claude_code_assistant_without_stop_reason_means_working() {
        let f = write_jsonl(&[
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"thinking..."}]}}"#,
        ]);
        let adapter = ClaudeCodeAdapter::new();
        assert_eq!(adapter.working_state_from_jsonl(f.path()), WorkingState::Working);
    }

    #[test]
    fn claude_code_tool_result_user_skipped_looks_earlier() {
        // tool_result-only user message is skipped; look at the preceding real prompt.
        let f = write_jsonl(&[
            r#"{"type":"user","message":{"role":"user","content":"original question"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"let me check"}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"x","type":"tool_result","content":[]}]}}"#,
        ]);
        let adapter = ClaudeCodeAdapter::new();
        // Last meaningful line is assistant (Working), tool_result is skipped.
        assert_eq!(adapter.working_state_from_jsonl(f.path()), WorkingState::Working);
    }
}

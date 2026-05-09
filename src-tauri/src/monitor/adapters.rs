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
                // Try content.text field first
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
                // Fallback: content might be a string directly
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

        let status = if let Some(kind) = self.last_jsonl_status(&jsonl_path) {
            match kind.as_str() {
                "Prompt" => SessionStatus::Idle,
                _ => SessionStatus::Active,
            }
        } else {
            SessionStatus::Idle
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

    fn find_latest_session_jsonl(&self, cwd: &str) -> Option<PathBuf> {
        // Claude Code stores sessions under ~/.claude/projects/{encoded_path}/
        // The encoded path replaces / with -
        let encoded = cwd.trim_end_matches('/').replace('/', "-");
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

    fn detect_from_processes(&self, sys: &sysinfo::System) -> Vec<AgentSession> {
        let mut sessions = Vec::new();
        for (_, proc) in sys.processes() {
            let exe = proc
                .exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let exe_basename = std::path::Path::new(&exe)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if exe_basename != "claude" {
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

            // Read last message preview from session JSONL
            let last_message_preview = if !cwd.is_empty() {
                self.find_latest_session_jsonl(&cwd)
                    .and_then(|p| self.last_assistant_preview(&p))
            } else {
                None
            };

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

            let session_id = format!("codex-{}", proc.pid().as_u32());

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
            let session_id = format!("gemini-{}", pid);
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

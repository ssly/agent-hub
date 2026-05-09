use super::types::*;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct KiroAdapter {
    home: PathBuf,
    sessions: HashMap<String, AgentSession>,
}

impl KiroAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            home,
            sessions: HashMap::new(),
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
        let content = fs::read_to_string(path).ok()?;
        let last_line = content.lines().last()?;
        let json: serde_json::Value = serde_json::from_str(last_line).ok()?;
        json.get("kind").and_then(|v| v.as_str()).map(String::from)
    }

    fn session_from_files(&self, session_id: &str, dir: &Path) -> Option<AgentSession> {
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
                _ => SessionStatus::Active, // AssistantMessage, ToolUse, etc.
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

        // Check if PID belongs to CLI or Desktop process
        let source_tag = if let Some(pid_val) = pid {
            let mut sys = sysinfo::System::new_all();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
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

    fn detect_sessions(&self) -> Vec<AgentSession> {
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
                        // Verify PID is still alive
                        let lock_path = dir.join(format!("{session_id}.lock"));
                        let pid_alive = self.parse_lock_file(&lock_path)
                            .map(|pid| {
                                let mut sys = sysinfo::System::new_all();
                                sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                                sys.process(sysinfo::Pid::from_u32(pid)).is_some()
                            })
                            .unwrap_or(false);
                        if !pid_alive {
                            continue;
                        }
                        if let Some(session) = self.session_from_files(session_id, &dir) {
                            result.push(session);
                        }
                    }
                }
            }
        }
        result
    }

    fn on_fs_event(&mut self, event: &notify::Event) -> Vec<(StateChange, AgentSession)> {
        let mut changes = Vec::new();
        for path in &event.paths {
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            let dir = path.parent().unwrap_or(Path::new(""));

            if file_name.ends_with(".lock") {
                let session_id = file_name.trim_end_matches(".lock");
                match event.kind {
                    notify::EventKind::Create(_) => {
                        if let Some(session) = self.session_from_files(session_id, dir) {
                            self.sessions
                                .insert(session_id.to_string(), session.clone());
                            changes.push((StateChange::Added, session));
                        }
                    }
                    notify::EventKind::Remove(_) => {
                        if let Some(mut session) = self.sessions.remove(session_id) {
                            session.status = SessionStatus::Ended;
                            changes.push((StateChange::Updated, session));
                        }
                    }
                    _ => {}
                }
            } else if file_name.ends_with(".jsonl") || file_name.ends_with(".json") {
                let session_id = file_name
                    .trim_end_matches(".jsonl")
                    .trim_end_matches(".json");
                if let Some(session) = self.session_from_files(session_id, dir) {
                    let change = if self.sessions.contains_key(session_id) {
                        StateChange::Updated
                    } else {
                        StateChange::Added
                    };
                    self.sessions
                        .insert(session_id.to_string(), session.clone());
                    changes.push((change, session));
                }
            }
        }
        changes
    }
}

pub struct ClaudeCodeAdapter {
    home: PathBuf,
    sessions: HashMap<String, AgentSession>,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            home,
            sessions: HashMap::new(),
        }
    }

    fn projects_dir(&self) -> PathBuf {
        self.home.join(".claude/projects")
    }

    fn detect_from_processes(&self) -> Vec<AgentSession> {
        let mut sessions = Vec::new();
        let mut sys = sysinfo::System::new_all();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        for (_, proc) in sys.processes() {
            let exe = proc.exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            // Only match processes whose exe basename is exactly "claude"
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

            // Skip internal/team processes
            if cmd.iter().any(|a| a.starts_with("--teammate-mode")
                || a.starts_with("--output-format"))
            {
                continue;
            }
            // Skip processes with no meaningful arguments (sub-processes)
            if cmd.len() <= 1 {
                continue;
            }

            // Skip Claude Desktop internal agent processes
            if exe.contains("Claude-3p") || exe.contains("Claude.app") {
                // Only show if it has --resume (user-facing Desktop session)
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

            let source_tag = if exe.contains("Claude-3p")
                || exe.contains("Claude.app")
            {
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

            // Use actual process start time
            let started_at = DateTime::from_timestamp(proc.start_time() as i64, 0).unwrap_or_default();

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

    fn detect_sessions(&self) -> Vec<AgentSession> {
        self.detect_from_processes()
    }

    fn on_fs_event(&mut self, _event: &notify::Event) -> Vec<(StateChange, AgentSession)> {
        // Claude Code relies on process detection; fs events just trigger refresh
        let current = self.detect_from_processes();
        let mut changes = Vec::new();
        let current_ids: std::collections::HashSet<_> =
            current.iter().map(|s| s.session_id.clone()).collect();

        for (id, session) in &self.sessions {
            if !current_ids.contains(id) {
                let mut ended = session.clone();
                ended.status = SessionStatus::Ended;
                changes.push((StateChange::Updated, ended));
            }
        }

        for session in current {
            let change = if self.sessions.contains_key(&session.session_id) {
                StateChange::Updated
            } else {
                StateChange::Added
            };
            self.sessions
                .insert(session.session_id.clone(), session.clone());
            changes.push((change, session));
        }

        // Remove ended sessions from tracking
        self.sessions.retain(|id, s| {
            current_ids.contains(id) || s.status != SessionStatus::Ended
        });

        changes
    }
}

pub struct CodexAdapter {
    home: PathBuf,
    sessions: HashMap<String, AgentSession>,
}

impl CodexAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            home,
            sessions: HashMap::new(),
        }
    }

    fn codex_dir(&self) -> PathBuf {
        self.home.join(".codex")
    }

    fn detect_from_processes(&self) -> Vec<AgentSession> {
        let mut sessions = Vec::new();
        let mut sys = sysinfo::System::new_all();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        for (_, proc) in sys.processes() {
            let name = proc.name().to_string_lossy().to_string();
            let is_codex = name == "codex" || name == "Codex";
            if !is_codex {
                continue;
            }
            let exe_path = proc.exe()
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

            let started_at = DateTime::from_timestamp(proc.start_time() as i64, 0).unwrap_or_default();
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

    fn detect_sessions(&self) -> Vec<AgentSession> {
        self.detect_from_processes()
    }

    fn on_fs_event(&mut self, _event: &notify::Event) -> Vec<(StateChange, AgentSession)> {
        let current = self.detect_from_processes();
        let mut changes = Vec::new();
        let current_ids: std::collections::HashSet<_> =
            current.iter().map(|s| s.session_id.clone()).collect();

        for (id, session) in &self.sessions {
            if !current_ids.contains(id) {
                let mut ended = session.clone();
                ended.status = SessionStatus::Ended;
                changes.push((StateChange::Updated, ended));
            }
        }

        for session in current {
            let change = if self.sessions.contains_key(&session.session_id) {
                StateChange::Updated
            } else {
                StateChange::Added
            };
            self.sessions
                .insert(session.session_id.clone(), session.clone());
            changes.push((change, session));
        }

        self.sessions.retain(|id, s| {
            current_ids.contains(id) || s.status != SessionStatus::Ended
        });

        changes
    }
}

pub struct GeminiAdapter {
    home: PathBuf,
    sessions: HashMap<String, AgentSession>,
}

impl GeminiAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            home,
            sessions: HashMap::new(),
        }
    }

    fn gemini_tmp_dir(&self) -> PathBuf {
        self.home.join(".gemini/tmp")
    }

    fn detect_from_processes(&self) -> Vec<AgentSession> {
        let mut seen_cwd: HashMap<String, u32> = HashMap::new(); // cwd -> smallest PID
        let mut all: Vec<(u32, AgentSession)> = Vec::new();
        let mut sys = sysinfo::System::new_all();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
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

            let started_at = DateTime::from_timestamp(proc.start_time() as i64, 0).unwrap_or_default();

            all.push((pid, AgentSession {
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
                data_limited_reason: Some(
                    "monitor.data_limited_gemini".to_string(),
                ),
                pid: Some(pid),
            }));

            // Track lowest PID per cwd (parent process)
            seen_cwd.entry(cwd).and_modify(|e| {
                if pid < *e { *e = pid; }
            }).or_insert(pid);
        }
        // Deduplicate: keep only the parent process (lowest PID) per cwd
        let parent_pids: std::collections::HashSet<u32> = seen_cwd.values().copied().collect();
        all.into_iter()
            .filter(|(pid, _)| parent_pids.contains(pid))
            .map(|(_, s)| s)
            .collect()
    }

    fn read_logs_json(&self, project: &str) -> Option<String> {
        let path = self.gemini_tmp_dir().join(project).join("logs.json");
        let content = fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        // Try to get the last user message as title
        if let Some(messages) = json.as_array() {
            for msg in messages.iter().rev() {
                if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                    return msg
                        .get("parts")
                        .and_then(|p| p.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|t| t.as_str())
                        .map(|s| s.chars().take(50).collect());
                }
            }
        }
        None
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

    fn detect_sessions(&self) -> Vec<AgentSession> {
        self.detect_from_processes()
    }

    fn on_fs_event(&mut self, event: &notify::Event) -> Vec<(StateChange, AgentSession)> {
        // Try to enrich session data from file events
        for path in &event.paths {
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            if file_name == "logs.json" {
                if let Some(project) = path.parent().and_then(|p| p.file_name()) {
                    let project_str = project.to_string_lossy();
                    if let Some(title) = self.read_logs_json(&project_str) {
                        for (_, session) in self.sessions.iter_mut() {
                            if session.cwd.contains(&*project_str) {
                                session.title = title.clone();
                                session.last_activity = Utc::now();
                            }
                        }
                    }
                }
            }
        }

        let current = self.detect_from_processes();
        let mut changes = Vec::new();
        let current_ids: std::collections::HashSet<_> =
            current.iter().map(|s| s.session_id.clone()).collect();

        for (id, session) in &self.sessions {
            if !current_ids.contains(id) {
                let mut ended = session.clone();
                ended.status = SessionStatus::Ended;
                changes.push((StateChange::Updated, ended));
            }
        }

        for session in current {
            let change = if self.sessions.contains_key(&session.session_id) {
                StateChange::Updated
            } else {
                StateChange::Added
            };
            self.sessions
                .insert(session.session_id.clone(), session.clone());
            changes.push((change, session));
        }

        self.sessions.retain(|id, s| {
            current_ids.contains(id) || s.status != SessionStatus::Ended
        });

        changes
    }
}

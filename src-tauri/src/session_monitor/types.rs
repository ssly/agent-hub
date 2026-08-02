use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Codex,
    Claude,
    Kiro,
    Grok,
    Kimi,
}

impl AgentKind {
    pub const ALL: [AgentKind; 5] = [
        Self::Codex,
        Self::Claude,
        Self::Kiro,
        Self::Grok,
        Self::Kimi,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Kiro => "kiro",
            Self::Grok => "grok",
            Self::Kimi => "kimi",
        }
    }

    pub fn state_file_name(self) -> &'static str {
        match self {
            Self::Codex => "codex-state.json",
            Self::Claude => "claude-state.json",
            Self::Kiro => "kiro-state.json",
            Self::Grok => "grok-state.json",
            Self::Kimi => "kimi-state.json",
        }
    }

    pub fn changed_event(self) -> &'static str {
        match self {
            Self::Codex => "session-monitor:codex-changed",
            Self::Claude => "session-monitor:claude-changed",
            Self::Kiro => "session-monitor:kiro-changed",
            Self::Grok => "session-monitor:grok-changed",
            Self::Kimi => "session-monitor:kimi-changed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionSource {
    Terminal,
    Chatgpt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeStatus {
    Running,
    Ended,
}

/// Events captured before the `agent` field existed are Codex events.
fn default_agent_kind() -> AgentKind {
    AgentKind::Codex
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEvent {
    pub event_id: String,
    #[serde(default = "default_agent_kind")]
    pub agent: AgentKind,
    pub hook_event_name: String,
    pub session_id: String,
    pub turn_id: String,
    pub source: SessionSource,
    pub cwd: Option<String>,
    pub user_prompt: Option<String>,
    pub assistant_reply: Option<String>,
    pub occurred_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub session_id: String,
    pub turn_id: String,
    pub source: SessionSource,
    pub status: RuntimeStatus,
    pub cwd: Option<String>,
    pub user_prompt: Option<String>,
    pub assistant_reply: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSnapshot {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub sessions: Vec<SessionState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStatus {
    pub installed: bool,
    pub config_path: String,
    pub command: String,
    pub managed_handler_count: usize,
    pub issue: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroMonitorStatus {
    pub available: bool,
    pub sessions_dir: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookDiffLine {
    pub tag: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookChangePreview {
    pub action: String,
    pub config_path: String,
    pub command: String,
    pub before_hash: String,
    pub diff_lines: Vec<HookDiffLine>,
    pub added: usize,
    pub removed: usize,
    pub changed: bool,
}

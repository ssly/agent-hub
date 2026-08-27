use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Codex,
    Claude,
    Cursor,
    Antigravity,
    Grok,
    Kimi,
    Qwen,
    ZCode,
    Workbuddy,
    Kiro,
    Dsh,
}

impl AgentKind {
    /// Same relative order as `platform/registry.rs` builtin platforms
    /// (monitor subset: no Shared).
    pub const ALL: [AgentKind; 11] = [
        Self::Codex,
        Self::Claude,
        Self::Cursor,
        Self::Antigravity,
        Self::Grok,
        Self::Kimi,
        Self::Qwen,
        Self::ZCode,
        Self::Workbuddy,
        Self::Kiro,
        Self::Dsh,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
            Self::Antigravity => "antigravity",
            Self::Grok => "grok",
            Self::Kimi => "kimi",
            Self::Qwen => "qwen",
            Self::ZCode => "zcode",
            Self::Workbuddy => "workbuddy",
            Self::Kiro => "kiro",
            Self::Dsh => "dsh",
        }
    }

    pub fn state_file_name(self) -> &'static str {
        match self {
            Self::Codex => "codex-state.json",
            Self::Claude => "claude-state.json",
            Self::Cursor => "cursor-state.json",
            Self::Antigravity => "antigravity-state.json",
            Self::Grok => "grok-state.json",
            Self::Kimi => "kimi-state.json",
            Self::Qwen => "qwen-state.json",
            Self::ZCode => "zcode-state.json",
            Self::Workbuddy => "workbuddy-state.json",
            Self::Kiro => "kiro-state.json",
            Self::Dsh => "dsh-state.json",
        }
    }

    pub fn changed_event(self) -> &'static str {
        match self {
            Self::Codex => "session-monitor:codex-changed",
            Self::Claude => "session-monitor:claude-changed",
            Self::Cursor => "session-monitor:cursor-changed",
            Self::Antigravity => "session-monitor:antigravity-changed",
            Self::Grok => "session-monitor:grok-changed",
            Self::Kimi => "session-monitor:kimi-changed",
            Self::Qwen => "session-monitor:qwen-changed",
            Self::ZCode => "session-monitor:zcode-changed",
            Self::Workbuddy => "session-monitor:workbuddy-changed",
            Self::Kiro => "session-monitor:kiro-changed",
            Self::Dsh => "session-monitor:dsh-changed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionSource {
    Terminal,
    Chatgpt,
    Cursor,
    /// Antigravity 2.0 desktop app (`~/.gemini/antigravity/`).
    Antigravity,
    /// Antigravity IDE surface (`~/.gemini/antigravity-ide/`).
    #[serde(rename = "antigravity-ide")]
    AntigravityIde,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeStatus {
    Running,
    /// Turn is blocked on a user permission/approval prompt.
    Waiting,
    /// Turn died on an API/tool error (`StopFailure`, Cursor `stop` + `error`).
    Failed,
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

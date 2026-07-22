use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookEvent {
    pub event_id: String,
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
pub struct CodexSessionState {
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
pub struct CodexMonitorSnapshot {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub sessions: Vec<CodexSessionState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookStatus {
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
pub struct CodexHookChangePreview {
    pub action: String,
    pub config_path: String,
    pub command: String,
    pub before_hash: String,
    pub diff_lines: Vec<HookDiffLine>,
    pub added: usize,
    pub removed: usize,
    pub changed: bool,
}

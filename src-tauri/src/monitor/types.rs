use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Idle,
    Completed,
    Ended,
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Active => write!(f, "active"),
            SessionStatus::Idle => write!(f, "idle"),
            SessionStatus::Completed => write!(f, "completed"),
            SessionStatus::Ended => write!(f, "ended"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StateChange {
    Added,
    Updated,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub agent_type: String,
    pub source_tag: String,
    pub session_id: String,
    pub title: String,
    pub model: String,
    pub cwd: String,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub data_limited: bool,
    pub data_limited_reason: Option<String>,
    #[allow(dead_code)]
    pub pid: Option<u32>,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub notification_enabled: bool,
    #[serde(default = "default_cooldown")]
    pub notification_cooldown_secs: u64,
}

fn default_cooldown() -> u64 {
    30
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            notification_enabled: false,
            notification_cooldown_secs: 30,
        }
    }
}

pub struct MonitorState {
    pub sessions: HashMap<String, AgentSession>,
    pub config: MonitorConfig,
    #[allow(dead_code)]
    pub last_notified: HashMap<String, DateTime<Utc>>,
}

impl MonitorState {
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            config,
            last_notified: HashMap::new(),
        }
    }
}

pub trait AgentMonitor: Send + Sync {
    fn platform_id(&self) -> &str;
    fn watch_paths(&self) -> Vec<PathBuf>;
    fn detect_sessions(&self, sys: &sysinfo::System) -> Vec<AgentSession>;
    #[allow(dead_code)]
    fn on_fs_event(&mut self, event: &notify::Event) -> Vec<(StateChange, AgentSession)>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionPlatform {
    pub id: String,
    pub display_name: String,
    pub session_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub project_path: String,
    pub model: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
    pub message_count: Option<u32>,
    pub tokens_used: Option<u64>,
    pub platform_id: String,
    /// Provenance of the client that created the session, when the storage
    /// format records it: "terminal" (CLI) or "chatgpt" (ChatGPT desktop /
    /// IDE client). None means undetectable — UIs fall back to the bare
    /// platform name.
    pub source: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionListPage {
    pub paths: Vec<String>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub sessions: Vec<SessionSummary>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    /// Model reasoning / chain-of-thought, when the storage format keeps it
    /// separate from the visible reply. None means the adapter had nothing
    /// to show (or the platform never persisted it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

impl SessionMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>, timestamp: i64) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            timestamp,
            thinking: None,
        }
    }

    pub fn with_thinking(mut self, thinking: Option<String>) -> Self {
        self.thinking = thinking.filter(|text| !text.trim().is_empty());
        self
    }

    pub fn matches_query(&self, query_lower: &str) -> bool {
        self.content.to_lowercase().contains(query_lower)
            || self
                .thinking
                .as_ref()
                .is_some_and(|thinking| thinking.to_lowercase().contains(query_lower))
    }
}

pub fn push_pending_thinking(pending: &mut String, chunk: &str) {
    let trimmed = chunk.trim();
    if trimmed.is_empty() {
        return;
    }
    if !pending.is_empty() {
        pending.push('\n');
    }
    pending.push_str(trimmed);
}

pub fn take_pending_thinking(pending: &mut String) -> Option<String> {
    let trimmed = pending.trim();
    if trimmed.is_empty() {
        pending.clear();
        None
    } else {
        let text = trimmed.to_string();
        pending.clear();
        Some(text)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionTerminalOption {
    pub id: String,
    pub display_name: String,
    pub available: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionResumePreview {
    pub command: String,
    pub last_user_message: Option<String>,
    pub last_assistant_message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionSearchResult {
    pub session_id: String,
    pub session_title: String,
    pub project_path: String,
    pub platform_id: String,
    pub message: SessionMessage,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchDeleteFailure {
    pub session_id: String,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchDeleteResult {
    pub deleted: usize,
    pub failed: Vec<BatchDeleteFailure>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionExportResult {
    pub path: String,
    pub session_count: usize,
    pub message_count: usize,
}

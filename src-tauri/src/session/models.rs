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
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionTerminalOption {
    pub id: String,
    pub display_name: String,
    pub available: bool,
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


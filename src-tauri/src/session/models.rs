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
    /// Injected `<system-reminder>` blocks (workspace identity, memory
    /// rules, hook context). Kept separate so the UI can collapse them
    /// like thinking and leave the real prompt as `content`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

impl SessionMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>, timestamp: i64) -> Self {
        let (content, system) = split_injected_context(&content.into());
        Self {
            role: role.into(),
            content,
            timestamp,
            thinking: None,
            system,
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
            || self
                .system
                .as_ref()
                .is_some_and(|system| system.to_lowercase().contains(query_lower))
    }
}

/// Pull official `<system-reminder>` injections out of a stored message and
/// unwrap `<user_query>` so the visible body is the user's actual words.
/// WorkBuddy / CodeBuddy and Claude Code both persist these tags inside
/// user-role records; they are not something the user typed.
pub fn split_injected_context(text: &str) -> (String, Option<String>) {
    let (without_reminders, system) = take_tagged_blocks(text, "system-reminder");
    let body = extract_tagged_inner(&without_reminders, "user_query")
        .unwrap_or_else(|| without_reminders.trim().to_string());
    (body, system)
}

fn take_tagged_blocks(text: &str, tag: &str) -> (String, Option<String>) {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut rest = String::new();
    let mut blocks = Vec::new();
    let mut remaining = text;
    while let Some(start) = find_ignore_ascii_case(remaining, &open) {
        rest.push_str(&remaining[..start]);
        let after_name = &remaining[start + open.len()..];
        let Some(gt) = after_name.find('>') else {
            rest.push_str(&remaining[start..]);
            remaining = "";
            break;
        };
        let inner_start = start + open.len() + gt + 1;
        if let Some(end_rel) = find_ignore_ascii_case(&remaining[inner_start..], &close) {
            let end = inner_start + end_rel + close.len();
            let block = remaining[start..end].trim();
            if !block.is_empty() {
                blocks.push(block.to_string());
            }
            remaining = &remaining[end..];
        } else {
            let block = remaining[start..].trim();
            if !block.is_empty() {
                blocks.push(block.to_string());
            }
            remaining = "";
            break;
        }
    }
    rest.push_str(remaining);
    let system = if blocks.is_empty() {
        None
    } else {
        Some(blocks.join("\n\n"))
    };
    (rest, system)
}

fn extract_tagged_inner(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = find_ignore_ascii_case(text, &open)?;
    let after_name = &text[start + open.len()..];
    let gt = after_name.find('>')?;
    let inner = &after_name[gt + 1..];
    let end = find_ignore_ascii_case(inner, &close)?;
    Some(inner[..end].trim().to_string())
}

fn find_ignore_ascii_case(hay: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_system_reminder_and_unwraps_user_query() {
        let raw = r#"<system-reminder data-role="user-context">
<user_info>
OS Version: darwin
</user_info>
</system-reminder>
<user_query>历史上的今天发生过什么有趣的事？</user_query>"#;
        let (body, system) = split_injected_context(raw);
        assert_eq!(body, "历史上的今天发生过什么有趣的事？");
        assert!(system.unwrap().contains("data-role=\"user-context\""));
    }

    #[test]
    fn session_message_new_hides_injected_context() {
        let msg = SessionMessage::new(
            "user",
            "<system-reminder>hook context</system-reminder>\n<user_query>hello</user_query>",
            1,
        );
        assert_eq!(msg.content, "hello");
        assert_eq!(
            msg.system.as_deref(),
            Some("<system-reminder>hook context</system-reminder>")
        );
    }

    #[test]
    fn leaves_plain_user_text_alone() {
        let (body, system) = split_injected_context("普通用户问题");
        assert_eq!(body, "普通用户问题");
        assert!(system.is_none());
    }
}

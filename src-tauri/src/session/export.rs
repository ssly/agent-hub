use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use super::models::{SessionExportResult, SessionMessage, SessionSummary};

const MESSAGE_PAGE_SIZE: usize = 500;

struct ExportConversation {
    summary: SessionSummary,
    messages: Vec<SessionMessage>,
}

struct Labels {
    document_title: &'static str,
    exported_count: &'static str,
    search_placeholder: &'static str,
    user: &'static str,
    assistant: &'static str,
    thinking: &'static str,
    system: &'static str,
    empty: &'static str,
    project: &'static str,
    model: &'static str,
    messages: &'static str,
    no_results: &'static str,
}

pub(super) fn export_sessions_html(
    platform_id: &str,
    session_ids: &[String],
    output_path: &str,
    locale: &str,
) -> Result<SessionExportResult, String> {
    if session_ids.is_empty() {
        return Err("No sessions selected".to_string());
    }

    let requested = session_ids.iter().cloned().collect::<HashSet<_>>();
    let all_sessions = super::list_sessions_all(platform_id)?;
    let mut conversations = Vec::new();
    for summary in all_sessions {
        if !requested.contains(&summary.id) {
            continue;
        }
        let messages = load_all_messages(platform_id, &summary.id)?;
        conversations.push(ExportConversation { summary, messages });
    }

    if conversations.len() != requested.len() {
        let found = conversations
            .iter()
            .map(|conversation| conversation.summary.id.as_str())
            .collect::<HashSet<_>>();
        let missing = requested
            .iter()
            .filter(|id| !found.contains(id.as_str()))
            .take(5)
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!("Session not found: {}", missing.join(", ")));
    }

    let output = normalize_output_path(output_path)?;
    let html = build_html(platform_id, locale, &conversations);
    fs::write(&output, html.as_bytes())
        .map_err(|error| format!("Failed to write {}: {}", output.display(), error))?;

    Ok(SessionExportResult {
        path: output.display().to_string(),
        session_count: conversations.len(),
        message_count: conversations
            .iter()
            .map(|conversation| conversation.messages.len())
            .sum(),
    })
}

fn load_all_messages(platform_id: &str, session_id: &str) -> Result<Vec<SessionMessage>, String> {
    let mut messages = Vec::new();
    loop {
        let page = super::get_session_messages(
            platform_id,
            session_id,
            messages.len(),
            MESSAGE_PAGE_SIZE,
        )?;
        let page_len = page.len();
        messages.extend(page);
        if page_len < MESSAGE_PAGE_SIZE {
            break;
        }
    }
    Ok(messages)
}

fn normalize_output_path(output_path: &str) -> Result<PathBuf, String> {
    let trimmed = output_path.trim();
    if trimmed.is_empty() {
        return Err("Export path is empty".to_string());
    }
    let mut output = PathBuf::from(trimmed);
    if output.extension().is_none() {
        output.set_extension("html");
    }
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        if !parent.is_dir() {
            return Err(format!(
                "Export directory does not exist: {}",
                parent.display()
            ));
        }
    }
    Ok(output)
}

fn build_html(platform_id: &str, locale: &str, conversations: &[ExportConversation]) -> String {
    let labels = labels(locale);
    let platform_name = match platform_id {
        "claude-code" => "Claude Code",
        "codex" => "Codex",
        "cursor" => "Cursor",
        "antigravity" => "Antigravity",
        "kiro" => "Kiro",
        "grok" => "Grok Build",
        "kimi" => "Kimi Code",
        "qwen" => "Qwen Code",
        "zcode" => "ZCode",
        "dsh" => "DeepSeek Harness",
        "omp" => "Oh My Pi",
        _ => platform_id,
    };
    let locale_tag = if locale.to_ascii_lowercase().starts_with("zh") {
        "zh-CN"
    } else {
        "en"
    };
    let total_messages = conversations
        .iter()
        .map(|conversation| conversation.messages.len())
        .sum::<usize>();

    let mut html = String::with_capacity(32_768 + total_messages.saturating_mul(500));
    write!(
        html,
        r#"<!doctype html>
<html lang="{locale_tag}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:">
  <title>{title}</title>
  <style>
    :root {{ color-scheme: light; --paper:#f4f1eb; --panel:#fffefa; --ink:#1e2a32; --muted:#6c7478; --line:#dedbd4; --accent:#356f7b; --accent-soft:#e5f0f1; --user:#fff7df; --assistant:#edf4f5; --shadow:0 18px 54px rgba(30,42,50,.10); }}
    * {{ box-sizing:border-box; }}
    html,body {{ margin:0; min-height:100%; background:var(--paper); color:var(--ink); font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif; }}
    button,input {{ font:inherit; }}
    .layout {{ min-height:100vh; display:grid; grid-template-columns:310px minmax(0,1fr); }}
    .sidebar {{ position:sticky; top:0; height:100vh; display:flex; flex-direction:column; padding:24px 18px; background:#17313a; color:#fff; overflow:hidden; }}
    .brand {{ margin:0; font-size:12px; font-weight:800; letter-spacing:.18em; color:#9fc5ca; }}
    .sidebar h1 {{ margin:10px 0 6px; font-size:24px; line-height:1.25; }}
    .summary {{ margin:0 0 18px; color:#b9c9cc; font-size:13px; }}
    .search {{ width:100%; margin-bottom:14px; padding:10px 12px; border:1px solid #45616a; border-radius:10px; outline:none; color:#fff; background:#23434d; }}
    .search:focus {{ border-color:#94c3c9; box-shadow:0 0 0 3px rgba(148,195,201,.15); }}
    .session-nav {{ display:flex; flex-direction:column; gap:7px; overflow:auto; padding-right:3px; }}
    .session-link {{ width:100%; padding:11px 12px; border:0; border-radius:10px; color:#cddadd; background:transparent; text-align:left; cursor:pointer; }}
    .session-link:hover {{ background:#264851; }}
    .session-link.active {{ color:#fff; background:#356f7b; }}
    .session-link strong {{ display:block; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:13px; }}
    .session-link span {{ display:block; margin-top:4px; color:inherit; opacity:.7; font-size:11px; }}
    .no-results {{ display:none; padding:18px 10px; color:#b9c9cc; font-size:13px; }}
    .content {{ width:min(1020px,100%); margin:0 auto; padding:42px 44px 80px; }}
    .session-view {{ display:none; }}
    .session-view.active {{ display:block; }}
    .session-header {{ margin-bottom:28px; padding:26px 28px; border:1px solid var(--line); border-radius:18px; background:var(--panel); box-shadow:var(--shadow); }}
    .eyebrow {{ margin:0 0 8px; color:var(--accent); font-size:12px; font-weight:800; letter-spacing:.12em; text-transform:uppercase; }}
    .session-header h2 {{ margin:0 0 14px; font-size:30px; line-height:1.25; overflow-wrap:anywhere; }}
    .meta {{ display:flex; flex-wrap:wrap; gap:8px; }}
    .meta span {{ padding:5px 9px; border-radius:999px; background:#f0efeb; color:var(--muted); font-size:12px; }}
    .conversation {{ display:flex; flex-direction:column; gap:18px; }}
    .message {{ max-width:88%; }}
    .message.user {{ align-self:flex-end; }}
    .message.assistant {{ align-self:flex-start; }}
    .message-head {{ display:flex; align-items:center; gap:8px; margin:0 10px 6px; color:var(--muted); font-size:12px; }}
    .message.user .message-head {{ justify-content:flex-end; }}
    .role {{ font-weight:800; color:var(--ink); }}
    .bubble {{ padding:18px 20px; border:1px solid var(--line); border-radius:17px; background:var(--assistant); box-shadow:0 8px 24px rgba(30,42,50,.06); overflow-wrap:anywhere; }}
    .user .bubble {{ background:var(--user); border-top-right-radius:5px; }}
    .assistant .bubble {{ border-top-left-radius:5px; }}
    .thinking {{ margin:0 0 12px; padding:8px 10px; border-radius:10px; background:#eef2f2; color:var(--muted); }}
    .thinking summary {{ cursor:pointer; font-size:12px; font-weight:700; user-select:none; }}
    .thinking pre {{ margin:8px 0 0; padding:0; overflow:visible; border-radius:0; color:var(--muted); background:transparent; font:12.5px/1.65 inherit; white-space:pre-wrap; }}
    
    /* Markdown Body Typography */
    .md-body {{ line-height:1.68; font-size:14px; color:inherit; }}
    .md-body > *:first-child {{ margin-top:0; }}
    .md-body > *:last-child {{ margin-bottom:0; }}
    .md-body p {{ margin:0.5em 0; line-height:1.68; }}
    .md-body h1, .md-body h2, .md-body h3, .md-body h4, .md-body h5, .md-body h6 {{
      margin:1.1em 0 0.35em;
      font-weight:700;
      line-height:1.3;
      color:var(--ink);
    }}
    .md-body h1 {{ font-size:1.35em; border-bottom:1px solid var(--line); padding-bottom:0.25em; }}
    .md-body h2 {{ font-size:1.2em; border-bottom:1px solid var(--line); padding-bottom:0.2em; }}
    .md-body h3 {{ font-size:1.08em; }}
    .md-body h4 {{ font-size:1em; }}
    .md-body code {{
      font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace;
      font-size:0.88em;
      padding:2px 5px;
      border-radius:4px;
      background:rgba(0,0,0,0.06);
      color:#b33917;
    }}
    .user .md-body code {{
      background:rgba(0,0,0,0.05);
      color:#9c2e10;
    }}
    .md-body pre {{
      margin:0.8em 0;
      padding:14px 16px;
      overflow-x:auto;
      border-radius:10px;
      background:#172b33;
      color:#e8eef0;
      font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace;
      font-size:12.5px;
      line-height:1.6;
    }}
    .md-body pre code {{
      padding:0;
      background:transparent;
      color:inherit;
      font-size:inherit;
    }}
    .md-body ul, .md-body ol {{
      margin:0.5em 0;
      padding-left:1.4em;
    }}
    .md-body li {{
      margin:0.25em 0;
      line-height:1.6;
    }}
    .md-body blockquote {{
      margin:0.6em 0;
      padding:4px 12px;
      border-left:3px solid var(--accent);
      background:rgba(53,111,123,0.06);
      border-radius:0 4px 4px 0;
      color:var(--muted);
    }}
    .md-body blockquote > *:first-child {{ margin-top:0; }}
    .md-body blockquote > *:last-child {{ margin-bottom:0; }}
    .md-body table {{
      width:100%;
      margin:0.8em 0;
      border-collapse:collapse;
      font-size:13px;
    }}
    .md-body th, .md-body td {{
      padding:6px 10px;
      border:1px solid var(--line);
      text-align:left;
    }}
    .md-body th {{
      background:rgba(0,0,0,0.04);
      font-weight:600;
    }}
    .md-body hr {{
      margin:1.2em 0;
      border:0;
      border-top:1px solid var(--line);
    }}
    .md-body a {{
      color:var(--accent);
      text-decoration:underline;
      text-underline-offset:2px;
    }}
    .empty {{ padding:28px; border:1px dashed var(--line); border-radius:14px; color:var(--muted); text-align:center; }}
    [hidden] {{ display:none !important; }}
    @media (max-width:760px) {{ .layout {{ display:block; }} .sidebar {{ position:relative; height:auto; max-height:44vh; }} .content {{ padding:24px 14px 60px; }} .session-header {{ padding:20px; }} .session-header h2 {{ font-size:24px; }} .message {{ max-width:96%; }} }}
    @media print {{ .layout {{ display:block; }} .sidebar {{ display:none; }} .content {{ width:100%; padding:0; }} .session-view {{ display:block !important; page-break-after:always; }} .session-header,.bubble {{ box-shadow:none; }} }}
  </style>
</head>
<body>
<div class="layout">
  <aside class="sidebar">
    <p class="brand">Agent Hub</p>
    <h1>{title}</h1>
    <p class="summary">{platform} · {count} {exported_count} · {messages} {message_label}</p>
    <input id="search" class="search" type="search" placeholder="{search_placeholder}" autocomplete="off">
    <nav id="session-nav" class="session-nav" aria-label="Sessions">
"#,
        title = escape_html(labels.document_title),
        platform = escape_html(platform_name),
        count = conversations.len(),
        exported_count = escape_html(labels.exported_count),
        messages = total_messages,
        message_label = escape_html(labels.messages),
        search_placeholder = escape_attribute(labels.search_placeholder),
    )
    .expect("writing to String cannot fail");

    for (index, conversation) in conversations.iter().enumerate() {
        let active = if index == 0 { " active" } else { "" };
        let project = project_name(&conversation.summary.project_path);
        writeln!(
            html,
            "      <button class=\"session-link{active}\" data-target=\"session-{index}\"><strong>{title}</strong><span>{project} · {count} {messages}</span></button>",
            title = escape_html(&conversation.summary.title),
            project = escape_html(&project),
            count = conversation.messages.len(),
            messages = escape_html(labels.messages),
        )
        .expect("writing to String cannot fail");
    }
    write!(
        html,
        "      <div id=\"no-results\" class=\"no-results\">{}</div>\n    </nav>\n  </aside>\n  <main class=\"content\">\n",
        escape_html(labels.no_results)
    )
    .expect("writing to String cannot fail");

    for (index, conversation) in conversations.iter().enumerate() {
        render_conversation(&mut html, index, conversation, platform_name, &labels);
    }

    let locale_json = serde_json::to_string(locale_tag).unwrap_or_else(|_| "\"en\"".to_string());
    write!(
        html,
        r#"  </main>
</div>
<script>
(() => {{
  const locale = {locale_json};
  const links = [...document.querySelectorAll('.session-link')];
  const views = [...document.querySelectorAll('.session-view')];
  const activate = (id) => {{
    links.forEach(link => link.classList.toggle('active', link.dataset.target === id));
    views.forEach(view => view.classList.toggle('active', view.id === id));
    window.scrollTo({{ top: 0, behavior: 'smooth' }});
  }};
  links.forEach(link => link.addEventListener('click', () => activate(link.dataset.target)));
  document.querySelectorAll('time[data-ts]').forEach(time => {{
    const value = Number(time.dataset.ts);
    time.textContent = value > 0 ? new Date(value).toLocaleString(locale, {{ dateStyle:'medium', timeStyle:'short' }}) : '';
  }});
  const search = document.getElementById('search');
  const noResults = document.getElementById('no-results');
  search.addEventListener('input', () => {{
    const query = search.value.trim().toLocaleLowerCase();
    let firstVisible = null;
    links.forEach(link => {{
      const view = document.getElementById(link.dataset.target);
      const haystack = `${{link.textContent}} ${{view.textContent}}`.toLocaleLowerCase();
      const visible = !query || haystack.includes(query);
      link.hidden = !visible;
      if (visible && !firstVisible) firstVisible = link;
    }});
    noResults.style.display = firstVisible ? 'none' : 'block';
    const active = links.find(link => link.classList.contains('active') && !link.hidden);
    if (!active && firstVisible) activate(firstVisible.dataset.target);
  }});
}})();
</script>
</body>
</html>
"#,
    )
    .expect("writing to String cannot fail");
    html
}

use pulldown_cmark::{html, Event, Options, Parser};

struct DisplayMessage {
    role: String,
    timestamp: i64,
    system: Option<String>,
    thinking: Option<String>,
    content: String,
}

fn split_injected_context(text: &str) -> (String, Option<String>) {
    let mut body = text.to_string();
    let mut system_parts = Vec::new();

    // Extract <system-reminder>...</system-reminder>
    while let Some(start) = body.find("<system-reminder") {
        if let Some(end_tag_start) = body[start..].find("</system-reminder>") {
            let full_end = start + end_tag_start + "</system-reminder>".len();
            let block = &body[start..full_end];
            if let Some(inner_start) = block.find('>') {
                let inner = &block[inner_start + 1..block.len() - "</system-reminder>".len()];
                let trimmed = inner.trim();
                if !trimmed.is_empty() {
                    system_parts.push(trimmed.to_string());
                }
            }
            body.replace_range(start..full_end, "");
        } else {
            break;
        }
    }

    // Extract <user_query>...</user_query>
    if let Some(start) = body.find("<user_query") {
        if let Some(end_tag_start) = body[start..].find("</user_query>") {
            let full_end = start + end_tag_start + "</user_query>".len();
            let block = &body[start..full_end];
            if let Some(inner_start) = block.find('>') {
                let inner = &block[inner_start + 1..block.len() - "</user_query>".len()];
                let inner_trimmed = inner.trim().to_string();
                body.replace_range(start..full_end, &inner_trimmed);
            }
        }
    }

    let cleaned_body = body.trim().to_string();
    let combined_system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };
    (cleaned_body, combined_system)
}

fn group_conversation_messages(messages: &[SessionMessage]) -> Vec<DisplayMessage> {
    let mut groups: Vec<DisplayMessage> = Vec::new();
    for msg in messages {
        let (content, extracted_system) = split_injected_context(&msg.content);
        let combined_system = match (&msg.system, extracted_system) {
            (Some(s), Some(ext)) => Some(format!("{}\n\n{}", s.trim(), ext.trim())),
            (Some(s), None) => Some(s.clone()),
            (None, Some(ext)) => Some(ext),
            (None, None) => None,
        };

        if msg.role == "assistant" {
            if let Some(last) = groups.last_mut() {
                if last.role == "assistant" {
                    last.timestamp = msg.timestamp;
                    if let Some(th) = &msg.thinking {
                        let prev = last.thinking.take().unwrap_or_default();
                        last.thinking = Some(if prev.is_empty() {
                            th.clone()
                        } else {
                            format!("{}\n\n{}", prev, th)
                        });
                    }
                    if let Some(sys) = combined_system {
                        let prev = last.system.take().unwrap_or_default();
                        last.system = Some(if prev.is_empty() {
                            sys
                        } else {
                            format!("{}\n\n{}", prev, sys)
                        });
                    }
                    if !content.is_empty() {
                        if last.content.is_empty() {
                            last.content = content;
                        } else {
                            last.content.push_str("\n\n");
                            last.content.push_str(&content);
                        }
                    }
                    continue;
                }
            }
        }

        groups.push(DisplayMessage {
            role: msg.role.clone(),
            timestamp: msg.timestamp,
            system: combined_system,
            thinking: msg.thinking.clone(),
            content,
        });
    }
    groups
}

fn render_conversation(
    html: &mut String,
    index: usize,
    conversation: &ExportConversation,
    platform_name: &str,
    labels: &Labels,
) {
    let active = if index == 0 { " active" } else { "" };
    let project = project_name(&conversation.summary.project_path);
    let model = conversation.summary.model.as_deref().unwrap_or("—");
    write!(
        html,
        "    <section id=\"session-{index}\" class=\"session-view{active}\">\n      <header class=\"session-header\"><p class=\"eyebrow\">{platform}</p><h2>{title}</h2><div class=\"meta\"><span>{project_label}: {project}</span><span>{model_label}: {model}</span><span>{count} {messages}</span><span><time data-ts=\"{updated}\"></time></span></div></header>\n      <div class=\"conversation\">\n",
        platform = escape_html(platform_name),
        title = escape_html(&conversation.summary.title),
        project_label = escape_html(labels.project),
        project = escape_html(&project),
        model_label = escape_html(labels.model),
        model = escape_html(model),
        count = conversation.messages.len(),
        messages = escape_html(labels.messages),
        updated = conversation.summary.updated_at,
    )
    .expect("writing to String cannot fail");

    let display_messages = group_conversation_messages(&conversation.messages);
    if display_messages.is_empty() {
        writeln!(
            html,
            "        <div class=\"empty\">{}</div>",
            escape_html(labels.empty)
        )
        .expect("writing to String cannot fail");
    }
    for message in &display_messages {
        let role_class = if message.role == "user" {
            "user"
        } else {
            "assistant"
        };
        let role_label = if message.role == "user" {
            labels.user
        } else {
            labels.assistant
        };
        let thinking_html = message
            .thinking
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| {
                format!(
                    "<details class=\"thinking\"><summary>{}</summary><pre>{}</pre></details>",
                    escape_html(labels.thinking),
                    escape_html(text)
                )
            })
            .unwrap_or_default();
        let system_html = message
            .system
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| {
                format!(
                    "<details class=\"thinking\"><summary>{}</summary><pre>{}</pre></details>",
                    escape_html(labels.system),
                    escape_html(text)
                )
            })
            .unwrap_or_default();
        writeln!(
            html,
            "        <article class=\"message {role_class}\"><div class=\"message-head\"><span class=\"role\">{role}</span><time data-ts=\"{timestamp}\"></time></div><div class=\"bubble\">{system}{thinking}{content}</div></article>",
            role = escape_html(role_label),
            timestamp = message.timestamp,
            system = system_html,
            thinking = thinking_html,
            content = render_message_content(&message.content),
        )
        .expect("writing to String cannot fail");
    }
    html.push_str("      </div>\n    </section>\n");
}

fn render_message_content(content: &str) -> String {
    if content.trim().is_empty() {
        return String::new();
    }
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(content, options).map(|event| match event {
        Event::Html(text) => Event::Text(text),
        Event::InlineHtml(text) => Event::Text(text),
        other => other,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    format!("<div class=\"md-body\">{}</div>", html_output)
}

fn project_name(project_path: &str) -> String {
    let trimmed = project_path.trim();
    if trimmed.is_empty() {
        return "—".to_string();
    }
    Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attribute(value: &str) -> String {
    escape_html(value).replace('`', "&#96;")
}

fn labels(locale: &str) -> Labels {
    if locale.to_ascii_lowercase().starts_with("zh") {
        Labels {
            document_title: "会话记录",
            exported_count: "个会话",
            search_placeholder: "搜索会话和内容…",
            user: "用户",
            assistant: "Agent",
            thinking: "思维链",
            system: "系统提示",
            empty: "这个会话没有可展示的文本消息。",
            project: "项目",
            model: "模型",
            messages: "条消息",
            no_results: "没有匹配的会话",
        }
    } else {
        Labels {
            document_title: "Session Transcript",
            exported_count: "sessions",
            search_placeholder: "Search sessions and messages…",
            user: "User",
            assistant: "Agent",
            thinking: "Thinking",
            system: "System",
            empty: "This session has no displayable text messages.",
            project: "Project",
            model: "Model",
            messages: "messages",
            no_results: "No matching sessions",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_conversation(content: &str) -> ExportConversation {
        ExportConversation {
            summary: SessionSummary {
                id: "session-1".to_string(),
                title: "HTML <review>".to_string(),
                project_path: "/tmp/agent-hub".to_string(),
                model: Some("gpt-test".to_string()),
                started_at: 1,
                updated_at: 2,
                message_count: None,
                tokens_used: None,
                platform_id: "codex".to_string(),
                source: None,
            },
            messages: vec![SessionMessage::new("assistant", content, 3)],
        }
    }

    #[test]
    fn message_content_escapes_html_and_formats_fenced_code() {
        let rendered =
            render_message_content("hello <script>alert(1)</script>\n```rust\nlet x = 1 < 2;\n```");
        assert!(!rendered.contains("<script>"));
        assert!(rendered.contains("&lt;script&gt;"));
        assert!(rendered
            .contains("<pre><code class=\"language-rust\">let x = 1 &lt; 2;\n</code></pre>"));
    }

    #[test]
    fn message_content_formats_markdown_elements() {
        let rendered = render_message_content(
            "# Title\n\n**bold** and *italic*\n\n- item 1\n- item 2\n\n> quote",
        );
        assert!(rendered.contains("<h1>Title</h1>"));
        assert!(rendered.contains("<strong>bold</strong>"));
        assert!(rendered.contains("<em>italic</em>"));
        assert!(rendered.contains("<ul>"));
        assert!(rendered.contains("<li>item 1</li>"));
        assert!(rendered.contains("<blockquote>"));
    }

    #[test]
    fn export_html_is_single_file_searchable_and_escapes_titles() {
        let conversations = vec![sample_conversation("A clear answer")];
        let rendered = build_html("codex", "zh-CN", &conversations);
        assert!(rendered.starts_with("<!doctype html>"));
        assert!(rendered.contains("id=\"search\""));
        assert!(rendered.contains("HTML &lt;review&gt;"));
        assert!(rendered.contains("A clear answer"));
        assert!(rendered.contains("Content-Security-Policy"));
    }

    #[test]
    fn export_html_renders_thinking_collapsed() {
        let mut conversation = sample_conversation("可见回复");
        conversation.messages[0].thinking = Some("先想清楚再回答".to_string());
        let rendered = build_html("qwen", "zh-CN", &[conversation]);
        assert!(rendered.contains("<details class=\"thinking\">"));
        assert!(rendered.contains("<summary>思维链</summary>"));
        assert!(rendered.contains("先想清楚再回答"));
        assert!(rendered.contains("可见回复"));
    }

    #[test]
    fn output_path_gets_html_extension_when_missing() {
        let path = normalize_output_path("session-export").expect("path should be valid");
        assert_eq!(path, PathBuf::from("session-export.html"));
    }
}

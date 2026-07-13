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
        "kiro" => "Kiro",
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
    .text {{ line-height:1.72; white-space:pre-wrap; }}
    pre {{ margin:14px 0; padding:15px 16px; overflow:auto; border-radius:11px; color:#e8eef0; background:#172b33; font:12px/1.65 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace; white-space:pre; }}
    .empty {{ padding:28px; border:1px dashed var(--line); border-radius:14px; color:var(--muted); text-align:center; }}
    [hidden] {{ display:none !important; }}
    @media (max-width:760px) {{ .layout {{ display:block; }} .sidebar {{ position:relative; height:auto; max-height:44vh; }} .content {{ padding:24px 14px 60px; }} .session-header {{ padding:20px; }} .session-header h2 {{ font-size:24px; }} .message {{ max-width:96%; }} }}
    @media print {{ .layout {{ display:block; }} .sidebar {{ display:none; }} .content {{ width:100%; padding:0; }} .session-view {{ display:block !important; page-break-after:always; }} .session-header,.bubble {{ box-shadow:none; }} }}
  </style>
</head>
<body>
<div class="layout">
  <aside class="sidebar">
    <p class="brand">AGENT HUB</p>
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

    if conversation.messages.is_empty() {
        writeln!(
            html,
            "        <div class=\"empty\">{}</div>",
            escape_html(labels.empty)
        )
        .expect("writing to String cannot fail");
    }
    for message in &conversation.messages {
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
        writeln!(
            html,
            "        <article class=\"message {role_class}\"><div class=\"message-head\"><span class=\"role\">{role}</span><time data-ts=\"{timestamp}\"></time></div><div class=\"bubble\">{content}</div></article>",
            role = escape_html(role_label),
            timestamp = message.timestamp,
            content = render_message_content(&message.content),
        )
        .expect("writing to String cannot fail");
    }
    html.push_str("      </div>\n    </section>\n");
}

fn render_message_content(content: &str) -> String {
    let mut output = String::new();
    let mut text = String::new();
    let mut code = String::new();
    let mut in_code = false;

    for line in content.lines() {
        if line.trim_start().starts_with("```") {
            if in_code {
                write!(
                    output,
                    "<pre><code>{}</code></pre>",
                    escape_html(code.trim_end())
                )
                .expect("writing to String cannot fail");
                code.clear();
            } else if !text.is_empty() {
                write!(
                    output,
                    "<div class=\"text\">{}</div>",
                    escape_html(text.trim_end())
                )
                .expect("writing to String cannot fail");
                text.clear();
            }
            in_code = !in_code;
            continue;
        }
        let target = if in_code { &mut code } else { &mut text };
        target.push_str(line);
        target.push('\n');
    }

    if in_code || !code.is_empty() {
        write!(
            output,
            "<pre><code>{}</code></pre>",
            escape_html(code.trim_end())
        )
        .expect("writing to String cannot fail");
    }
    if !text.is_empty() || output.is_empty() {
        write!(
            output,
            "<div class=\"text\">{}</div>",
            escape_html(text.trim_end())
        )
        .expect("writing to String cannot fail");
    }
    output
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
            },
            messages: vec![SessionMessage {
                role: "assistant".to_string(),
                content: content.to_string(),
                timestamp: 3,
            }],
        }
    }

    #[test]
    fn message_content_escapes_html_and_formats_fenced_code() {
        let rendered =
            render_message_content("hello <script>alert(1)</script>\n```rust\nlet x = 1 < 2;\n```");
        assert!(!rendered.contains("<script>"));
        assert!(rendered.contains("&lt;script&gt;"));
        assert!(rendered.contains("<pre><code>let x = 1 &lt; 2;</code></pre>"));
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
    fn output_path_gets_html_extension_when_missing() {
        let path = normalize_output_path("session-export").expect("path should be valid");
        assert_eq!(path, PathBuf::from("session-export.html"));
    }
}

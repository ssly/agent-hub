pub(crate) mod antigravity;
mod claude;
mod codex;
mod cursor;
pub(crate) mod dsh;
mod export;
mod grok;
mod kimi;
mod kiro;
mod models;
mod qwen;
mod workbuddy;
mod zcode;

#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::process::Command;

pub use models::{
    BatchDeleteFailure, BatchDeleteResult, SessionExportResult, SessionListPage, SessionMessage,
    SessionPlatform, SessionResumePreview, SessionSearchResult, SessionTerminalOption,
};

#[cfg(target_os = "windows")]
use crate::win_console::suppress_console;

const MAX_SESSION_PAGE_SIZE: usize = 1000;
const PATH_FILTER_ALL: &str = "all";
const PATH_FILTER_UNKNOWN: &str = "unknown";

pub fn list_session_platforms(path_filter: Option<&str>) -> Result<Vec<SessionPlatform>, String> {
    let is_filtered = match path_filter {
        Some(filter) => {
            let t = filter.trim();
            !t.is_empty() && t != PATH_FILTER_ALL
        }
        None => false,
    };

    if !is_filtered {
        let mut platforms = Vec::new();

        let codex_count = codex::count_codex_sessions()?;
        if codex_count > 0 {
            platforms.push(SessionPlatform {
                id: "codex".to_string(),
                display_name: "Codex".to_string(),
                session_count: codex_count,
            });
        }

        // Order mirrors platform/registry.rs (session subset only).
        let claude_count = claude::count_claude_sessions()?;
        if claude_count > 0 {
            platforms.push(SessionPlatform {
                id: "claude-code".to_string(),
                display_name: "Claude Code".to_string(),
                session_count: claude_count,
            });
        }

        let cursor_count = cursor::count_cursor_sessions()?;
        if cursor_count > 0 {
            platforms.push(SessionPlatform {
                id: "cursor".to_string(),
                display_name: "Cursor".to_string(),
                session_count: cursor_count,
            });
        }

        let antigravity_count = antigravity::count_antigravity_sessions()?;
        if antigravity_count > 0 {
            platforms.push(SessionPlatform {
                id: "antigravity".to_string(),
                display_name: "Antigravity".to_string(),
                session_count: antigravity_count,
            });
        }

        let grok_count = grok::count_grok_sessions()?;
        if grok_count > 0 {
            platforms.push(SessionPlatform {
                id: "grok".to_string(),
                display_name: "Grok Build".to_string(),
                session_count: grok_count,
            });
        }

        let kimi_count = kimi::count_kimi_sessions()?;
        if kimi_count > 0 {
            platforms.push(SessionPlatform {
                id: "kimi".to_string(),
                display_name: "Kimi Code".to_string(),
                session_count: kimi_count,
            });
        }

        let qwen_count = qwen::count_qwen_sessions()?;
        if qwen_count > 0 {
            platforms.push(SessionPlatform {
                id: "qwen".to_string(),
                display_name: "Qwen Code".to_string(),
                session_count: qwen_count,
            });
        }

        let zcode_count = zcode::count_zcode_sessions()?;
        if zcode_count > 0 {
            platforms.push(SessionPlatform {
                id: "zcode".to_string(),
                display_name: "ZCode".to_string(),
                session_count: zcode_count,
            });
        }

        let workbuddy_count = workbuddy::count_workbuddy_sessions()?;
        if workbuddy_count > 0 {
            platforms.push(SessionPlatform {
                id: "workbuddy".to_string(),
                display_name: "WorkBuddy".to_string(),
                session_count: workbuddy_count,
            });
        }

        let kiro_count = kiro::count_kiro_sessions()?;
        if kiro_count > 0 {
            platforms.push(SessionPlatform {
                id: "kiro".to_string(),
                display_name: "Kiro".to_string(),
                session_count: kiro_count,
            });
        }

        let dsh_count = dsh::count_dsh_sessions()?;
        if dsh_count > 0 {
            platforms.push(SessionPlatform {
                id: "dsh".to_string(),
                display_name: "DeepSeek Harness".to_string(),
                session_count: dsh_count,
            });
        }

        return Ok(platforms);
    }

    let filter = path_filter.unwrap().trim();
    let mut platforms = Vec::new();

    let check_platform = |id: &str,
                          display_name: &str,
                          count_fn: fn() -> Result<usize, String>|
     -> Result<Option<SessionPlatform>, String> {
        let total = count_fn()?;
        if total == 0 {
            return Ok(None);
        }
        let all_sessions = list_sessions_all(id)?;
        let matching = filter_sessions_by_path(all_sessions, filter).len();
        Ok(Some(SessionPlatform {
            id: id.to_string(),
            display_name: display_name.to_string(),
            session_count: matching,
        }))
    };

    if let Some(p) = check_platform("codex", "Codex", codex::count_codex_sessions)? {
        platforms.push(p);
    }
    if let Some(p) = check_platform("claude-code", "Claude Code", claude::count_claude_sessions)? {
        platforms.push(p);
    }
    if let Some(p) = check_platform("cursor", "Cursor", cursor::count_cursor_sessions)? {
        platforms.push(p);
    }
    if let Some(p) = check_platform(
        "antigravity",
        "Antigravity",
        antigravity::count_antigravity_sessions,
    )? {
        platforms.push(p);
    }
    if let Some(p) = check_platform("grok", "Grok Build", grok::count_grok_sessions)? {
        platforms.push(p);
    }
    if let Some(p) = check_platform("kimi", "Kimi Code", kimi::count_kimi_sessions)? {
        platforms.push(p);
    }
    if let Some(p) = check_platform("qwen", "Qwen Code", qwen::count_qwen_sessions)? {
        platforms.push(p);
    }
    if let Some(p) = check_platform("zcode", "ZCode", zcode::count_zcode_sessions)? {
        platforms.push(p);
    }
    if let Some(p) = check_platform(
        "workbuddy",
        "WorkBuddy",
        workbuddy::count_workbuddy_sessions,
    )? {
        platforms.push(p);
    }
    if let Some(p) = check_platform("kiro", "Kiro", kiro::count_kiro_sessions)? {
        platforms.push(p);
    }
    if let Some(p) = check_platform("dsh", "DeepSeek Harness", dsh::count_dsh_sessions)? {
        platforms.push(p);
    }

    Ok(platforms)
}

pub fn list_sessions(
    platform_id: &str,
    path_filter: &str,
    offset: usize,
    limit: usize,
) -> Result<SessionListPage, String> {
    let page_limit = limit.clamp(1, MAX_SESSION_PAGE_SIZE);
    let all_sessions = list_sessions_all(platform_id)?;
    let paths = build_path_options(&all_sessions);
    let filtered_sessions = filter_sessions_by_path(all_sessions, path_filter);
    let total = filtered_sessions.len();
    let sessions = filtered_sessions
        .into_iter()
        .skip(offset)
        .take(page_limit)
        .collect::<Vec<_>>();
    let has_more = offset.saturating_add(sessions.len()) < total;
    Ok(SessionListPage {
        paths,
        total,
        offset,
        limit: page_limit,
        has_more,
        sessions,
    })
}

fn list_sessions_all(platform_id: &str) -> Result<Vec<models::SessionSummary>, String> {
    let mut sessions = match platform_id {
        "claude-code" => claude::list_claude_sessions_all(),
        "codex" => codex::list_codex_sessions_all(),
        "cursor" => cursor::list_cursor_sessions_all(),
        "antigravity" => antigravity::list_antigravity_sessions_all(),
        "kiro" => kiro::list_kiro_sessions_all(),
        "grok" => grok::list_grok_sessions_all(),
        "kimi" => kimi::list_kimi_sessions_all(),
        "qwen" => qwen::list_qwen_sessions_all(),
        "zcode" => zcode::list_zcode_sessions_all(),
        "workbuddy" => workbuddy::list_workbuddy_sessions_all(),
        "dsh" => dsh::list_dsh_sessions_all(),
        _ => Err(format!("Unsupported platform: {}", platform_id)),
    }?;
    // Normalize once for path filters, cards, and resume `cd` so every agent
    // surface shows the same Windows-friendly shape.
    for session in &mut sessions {
        session.project_path = normalize_project_path(&session.project_path).unwrap_or_default();
    }
    Ok(sessions)
}

pub fn export_sessions_html(
    platform_id: &str,
    session_ids: &[String],
    output_path: &str,
    locale: &str,
) -> Result<SessionExportResult, String> {
    export::export_sessions_html(platform_id, session_ids, output_path, locale)
}

fn normalize_project_path(value: &str) -> Option<String> {
    crate::paths::normalize_project_path_display(value)
}

fn build_path_options(sessions: &[models::SessionSummary]) -> Vec<String> {
    let mut options = sessions
        .iter()
        .filter_map(|session| normalize_project_path(&session.project_path))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    options.insert(0, PATH_FILTER_UNKNOWN.to_string());
    options.insert(0, PATH_FILTER_ALL.to_string());
    options
}

fn filter_sessions_by_path(
    sessions: Vec<models::SessionSummary>,
    path_filter: &str,
) -> Vec<models::SessionSummary> {
    let filter = path_filter.trim();
    if filter.is_empty() || filter == PATH_FILTER_ALL {
        return sessions;
    }
    if filter == PATH_FILTER_UNKNOWN {
        return sessions
            .into_iter()
            .filter(|session| normalize_project_path(&session.project_path).is_none())
            .collect();
    }
    sessions
        .into_iter()
        .filter(|session| {
            if let Some(p) = normalize_project_path(&session.project_path) {
                crate::paths::paths_match(&p, filter)
            } else {
                false
            }
        })
        .collect()
}

pub fn list_session_terminals() -> Vec<SessionTerminalOption> {
    #[cfg(target_os = "macos")]
    {
        vec![
            SessionTerminalOption {
                id: "warp".to_string(),
                display_name: "Warp".to_string(),
                available: is_terminal_available("warp"),
            },
            SessionTerminalOption {
                id: "terminal-default".to_string(),
                display_name: "Terminal".to_string(),
                available: is_terminal_available("terminal-default"),
            },
            SessionTerminalOption {
                id: "iterm".to_string(),
                display_name: "iTerm".to_string(),
                available: is_terminal_available("iterm"),
            },
            SessionTerminalOption {
                id: "ghostty".to_string(),
                display_name: "Ghostty".to_string(),
                available: is_terminal_available("ghostty"),
            },
        ]
    }
    #[cfg(target_os = "windows")]
    {
        vec![
            SessionTerminalOption {
                id: "terminal-default".to_string(),
                display_name: "Terminal".to_string(),
                available: is_terminal_available("terminal-default"),
            },
            SessionTerminalOption {
                id: "powershell".to_string(),
                display_name: "PowerShell".to_string(),
                available: is_terminal_available("powershell"),
            },
            SessionTerminalOption {
                id: "windows-terminal".to_string(),
                display_name: "Windows Terminal".to_string(),
                available: is_terminal_available("windows-terminal"),
            },
        ]
    }
    #[cfg(target_os = "linux")]
    {
        vec![SessionTerminalOption {
            id: "terminal-default".to_string(),
            display_name: "Terminal".to_string(),
            available: is_terminal_available("terminal-default"),
        }]
    }
}

pub fn resume_session(
    platform_id: &str,
    session_id: &str,
    project_path: &str,
    terminal_id: &str,
) -> Result<String, String> {
    // Auto-launch writes a .bat that cmd.exe runs, so Windows still needs `&`.
    // The copy-paste preview uses PowerShell `;` — see paste_resume_sep().
    let full_command =
        build_chained_resume_command(platform_id, session_id, project_path, launch_resume_sep())?;

    launch_terminal_with_command(terminal_id, &full_command)?;
    Ok(full_command)
}

/// Paste-ready `cd <project><sep><cli> resume <id>`. Unix shells get `&&`;
/// Windows gets PowerShell's statement separator `;` — cmd-style `&` is the
/// call operator in PowerShell and cannot chain commands.
fn build_full_resume_command(
    platform_id: &str,
    session_id: &str,
    project_path: &str,
) -> Result<String, String> {
    build_chained_resume_command(platform_id, session_id, project_path, paste_resume_sep())
}

fn paste_resume_sep() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "; "
    }
    #[cfg(not(target_os = "windows"))]
    {
        " && "
    }
}

fn launch_resume_sep() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        " & "
    }
    #[cfg(not(target_os = "windows"))]
    {
        " && "
    }
}

fn build_chained_resume_command(
    platform_id: &str,
    session_id: &str,
    project_path: &str,
    sep: &str,
) -> Result<String, String> {
    let resume_command = build_resume_command(platform_id, session_id)?;
    if project_path.trim().is_empty() {
        Ok(resume_command)
    } else {
        Ok(format!(
            "cd {}{}{}",
            shell_quote(project_path),
            sep,
            resume_command
        ))
    }
}

const RESUME_PREVIEW_MAX_CHARS: usize = 300;

/// Data behind the resume modal: the paste-ready command plus the session's
/// last user/assistant message (condensed to one line, capped in length).
/// A missing transcript never blocks the command — messages degrade to None.
pub fn get_session_resume_preview(
    platform_id: &str,
    session_id: &str,
    project_path: &str,
) -> Result<SessionResumePreview, String> {
    let command = build_full_resume_command(platform_id, session_id, project_path)?;
    let (last_user, last_assistant) =
        last_session_messages(platform_id, session_id).unwrap_or((None, None));
    Ok(SessionResumePreview {
        command,
        last_user_message: last_user.map(|msg| condense_resume_preview(&msg.content)),
        last_assistant_message: last_assistant.map(|msg| condense_resume_preview(&msg.content)),
    })
}

fn last_session_messages(
    platform_id: &str,
    session_id: &str,
) -> Result<(Option<SessionMessage>, Option<SessionMessage>), String> {
    match platform_id {
        "claude-code" => claude::last_claude_messages(session_id),
        "codex" => codex::last_codex_messages(session_id),
        "cursor" => cursor::last_cursor_messages(session_id),
        "antigravity" => antigravity::last_antigravity_messages(session_id),
        "kiro" => kiro::last_kiro_messages(session_id),
        "grok" => grok::last_grok_messages(session_id),
        "kimi" => kimi::last_kimi_messages(session_id),
        "qwen" => qwen::last_qwen_messages(session_id),
        "workbuddy" => workbuddy::last_workbuddy_messages(session_id),
        "dsh" => dsh::last_dsh_messages(session_id),
        _ => Err(format!("Unsupported platform: {}", platform_id)),
    }
}

fn condense_resume_preview(content: &str) -> String {
    let condensed = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut result = String::new();
    for (index, ch) in condensed.chars().enumerate() {
        if index >= RESUME_PREVIEW_MAX_CHARS {
            break;
        }
        result.push(ch);
    }
    if condensed.chars().count() > RESUME_PREVIEW_MAX_CHARS {
        format!("{}...", result)
    } else {
        result
    }
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(not(target_os = "macos"))]
fn shell_quote(value: &str) -> String {
    // Windows (cmd/PowerShell): wrap in double quotes; inner double quotes are
    // escaped by doubling. Backslashes stay single — doubling them would
    // produce a non-standard path like `C:\\Users\\x`.
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Whether a CLI name resolves on PATH. Platform-aware so Windows never
/// shells out through `sh` (Git for Windows ships `sh.exe`; spawning it
/// without CREATE_NO_WINDOW flashes a blank console).
fn command_exists(command: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        // `where` without a console window. Check bare name and `.exe`.
        return executable_available(command)
            || executable_available(&format!("{command}.exe"))
            || executable_available(&format!("{command}.cmd"))
            || executable_available(&format!("{command}.bat"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh")
            .arg("-lc")
            .arg(format!("command -v {} >/dev/null 2>&1", command))
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

/// Windows: check whether an executable is on PATH via `where`, without flashing
/// a console window. `CREATE_NO_WINDOW` keeps the probe silent; the exit code it
/// returns is unchanged, so detection results are identical to before.
#[cfg(target_os = "windows")]
fn executable_available(exe: &str) -> bool {
    let mut cmd = Command::new("where");
    cmd.arg(exe)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    suppress_console(&mut cmd)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_osascript_lines(lines: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new("osascript");
    for line in lines {
        cmd.arg("-e").arg(line);
    }
    let status = cmd.status().map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("osascript exited with status: {}", status))
    }
}

fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Resolve a macOS `.app` bundle. Prefer system `/Applications`, then fall
/// back to the user's `~/Applications`. Returns `None` only when neither
/// location has the app.
#[cfg(target_os = "macos")]
fn mac_app_bundle(app_name: &str) -> Option<PathBuf> {
    let system = PathBuf::from("/Applications").join(app_name);
    if system.exists() {
        return Some(system);
    }
    let user = crate::paths::home_dir().join("Applications").join(app_name);
    if user.exists() {
        return Some(user);
    }
    None
}

#[cfg(target_os = "macos")]
fn is_terminal_available(terminal_id: &str) -> bool {
    match terminal_id {
        "terminal-default" => true,
        "iterm" => {
            mac_app_bundle("iTerm.app").is_some()
                || mac_app_bundle("iTerm2.app").is_some()
                || command_exists("iterm2")
        }
        "ghostty" => command_exists("ghostty") || mac_app_bundle("Ghostty.app").is_some(),
        "warp" => command_exists("warp") || mac_app_bundle("Warp.app").is_some(),
        _ => false,
    }
}

#[cfg(target_os = "windows")]
fn is_terminal_available(terminal_id: &str) -> bool {
    match terminal_id {
        "terminal-default" => true, // default console always available
        // Prefer pwsh (PowerShell 7+), fall back to powershell (Windows PowerShell 5)
        "powershell" => executable_available("pwsh.exe") || executable_available("powershell.exe"),
        "windows-terminal" => executable_available("wt.exe"),
        _ => false,
    }
}

#[cfg(target_os = "linux")]
fn is_terminal_available(terminal_id: &str) -> bool {
    terminal_id == "terminal-default"
}

#[cfg(target_os = "macos")]
fn launch_terminal_with_command(terminal_id: &str, command: &str) -> Result<(), String> {
    match terminal_id {
        "terminal-default" => {
            let escaped = applescript_escape(command);
            run_osascript_lines(&[
                &format!("tell application \"Terminal\" to do script \"{}\"", escaped),
                "tell application \"Terminal\" to activate",
            ])
        }
        "iterm" => {
            let escaped = applescript_escape(command);
            run_osascript_lines(&[
                "tell application id \"com.googlecode.iterm2\"",
                "set newWindow to (create window with default profile)",
                &format!(
                    "tell current session of newWindow to write text \"{}\"",
                    escaped
                ),
                "activate",
                "end tell",
            ])
        }
        "ghostty" => {
            let bin = if command_exists("ghostty") {
                "ghostty".to_string()
            } else if let Some(app) = mac_app_bundle("Ghostty.app") {
                let path = app.join("Contents/MacOS/ghostty");
                if !path.exists() {
                    return Err("Ghostty is not installed.".to_string());
                }
                path.to_string_lossy().into_owned()
            } else {
                return Err("Ghostty is not installed.".to_string());
            };
            Command::new(bin)
                .arg("-e")
                .arg("zsh")
                .arg("-lc")
                .arg(command)
                .spawn()
                .map_err(|err| err.to_string())?;
            Ok(())
        }
        "warp" => {
            if mac_app_bundle("Warp.app").is_none() && !command_exists("warp") {
                return Err("Warp is not installed.".to_string());
            }
            let escaped = applescript_escape(command);
            run_osascript_lines(&[
                "tell application \"Warp\" to activate",
                "delay 0.12",
                &format!(
                    "tell application \"System Events\" to keystroke \"{}\"",
                    escaped
                ),
                "tell application \"System Events\" to key code 36",
            ])
        }
        _ => Err(format!("Unsupported terminal: {}", terminal_id)),
    }
}

#[cfg(target_os = "windows")]
fn launch_terminal_with_command(terminal_id: &str, command: &str) -> Result<(), String> {
    // Write command to a .bat file to avoid cmd.exe escaping issues
    let bat_path = std::env::temp_dir().join(format!(
        "agent-hub-resume-{}.bat",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let escaped = command.replace('%', "%%").replace('"', "\"\"");
    // chcp 65001: project paths with non-ASCII characters (e.g. Chinese) are
    // written here as UTF-8, but cmd parses .bat files in the OEM/ANSI
    // codepage — switching to UTF-8 first keeps `cd "D:\代码\proj"` working.
    // CRLF line endings because cmd is picky about bare LF in batch files.
    let bat_content = format!("@echo off\r\nchcp 65001 >nul\r\n{}\r\npause\r\n", escaped);
    std::fs::write(&bat_path, &bat_content).map_err(|e| e.to_string())?;

    match terminal_id {
        "windows-terminal" => {
            Command::new("wt.exe")
                .arg("cmd.exe")
                .arg("/k")
                .arg(&bat_path)
                .spawn()
                .map_err(|e| format!("Failed to launch terminal: {}", e))?;
            Ok(())
        }
        "powershell" => {
            // Prefer pwsh (PowerShell 7+), fall back to powershell (5).
            // Outer PowerShell is a launcher only — suppress its own console;
            // Start-Process still opens the visible resume terminal the user asked for.
            let ps = if executable_available("pwsh.exe") {
                "pwsh.exe"
            } else {
                "powershell.exe"
            };
            let ps_script = format!(
                "Start-Process cmd.exe -ArgumentList '/k','{}'",
                bat_path.to_string_lossy().replace('\'', "''")
            );
            let mut cmd = Command::new(ps);
            cmd.arg("-NoProfile").arg("-Command").arg(&ps_script);
            suppress_console(&mut cmd)
                .spawn()
                .map_err(|e| format!("Failed to launch terminal: {}", e))?;
            Ok(())
        }
        _ => {
            // Default: use PowerShell Start-Process so the outer PS host stays
            // invisible (CREATE_NO_WINDOW + -WindowStyle Hidden). The resume
            // cmd window itself is intentional and still shown.
            let bat_str = bat_path.to_string_lossy().replace('\'', "''");
            let ps_script = format!("Start-Process cmd.exe -ArgumentList '/k','{}'", bat_str);
            let mut cmd = Command::new("powershell.exe");
            cmd.arg("-NoProfile")
                .arg("-WindowStyle")
                .arg("Hidden")
                .arg("-Command")
                .arg(&ps_script);
            suppress_console(&mut cmd)
                .spawn()
                .map_err(|e| format!("Failed to launch terminal: {}", e))?;
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn launch_terminal_with_command(_terminal_id: &str, _command: &str) -> Result<(), String> {
    Err("Session resume terminal launcher is not yet supported on Linux.".to_string())
}

pub fn get_session_messages(
    platform_id: &str,
    session_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionMessage>, String> {
    match platform_id {
        "claude-code" => claude::get_claude_messages(session_id, offset, limit),
        "codex" => codex::get_codex_messages(session_id, offset, limit),
        "cursor" => cursor::get_cursor_messages(session_id, offset, limit),
        "antigravity" => antigravity::get_antigravity_messages(session_id, offset, limit),
        "kiro" => kiro::get_kiro_messages(session_id, offset, limit),
        "grok" => grok::get_grok_messages(session_id, offset, limit),
        "kimi" => kimi::get_kimi_messages(session_id, offset, limit),
        "qwen" => qwen::get_qwen_messages(session_id, offset, limit),
        "zcode" => zcode::get_zcode_messages(session_id, offset, limit),
        "workbuddy" => workbuddy::get_workbuddy_messages(session_id, offset, limit),
        "dsh" => dsh::get_dsh_messages(session_id, offset, limit),
        _ => Err(format!("Unsupported platform: {}", platform_id)),
    }
}

pub fn search_session_messages(
    platform_id: &str,
    query: &str,
) -> Result<Vec<SessionSearchResult>, String> {
    let query_lower = query.to_lowercase();
    match platform_id {
        "claude-code" => claude::search_claude_messages(&query_lower),
        "codex" => codex::search_codex_messages(&query_lower),
        "cursor" => cursor::search_cursor_messages(&query_lower),
        "antigravity" => antigravity::search_antigravity_messages(&query_lower),
        "kiro" => kiro::search_kiro_messages(&query_lower),
        "grok" => grok::search_grok_messages(&query_lower),
        "kimi" => kimi::search_kimi_messages(&query_lower),
        "qwen" => qwen::search_qwen_messages(&query_lower),
        "zcode" => zcode::search_zcode_messages(&query_lower),
        "workbuddy" => workbuddy::search_workbuddy_messages(&query_lower),
        "dsh" => dsh::search_dsh_messages(&query_lower),
        _ => Err(format!("Unsupported platform: {}", platform_id)),
    }
}

pub fn delete_session(platform_id: &str, session_id: &str) -> Result<(), String> {
    match platform_id {
        "claude-code" => claude::delete_claude_session(session_id),
        "codex" => codex::delete_codex_session(session_id),
        "cursor" => cursor::delete_cursor_session(session_id),
        "antigravity" => antigravity::delete_antigravity_session(session_id),
        "kiro" => kiro::delete_kiro_session(session_id),
        "grok" => grok::delete_grok_session(session_id),
        "kimi" => kimi::delete_kimi_session(session_id),
        "qwen" => qwen::delete_qwen_session(session_id),
        "zcode" => zcode::delete_zcode_session(session_id),
        "workbuddy" => workbuddy::delete_workbuddy_session(session_id),
        "dsh" => dsh::delete_dsh_session(session_id),
        _ => Err(format!("Unsupported platform: {}", platform_id)),
    }
}

/// Best-effort batch delete. One session failing never aborts the others — every
/// id is attempted and its outcome recorded. Codex uses a single batched UPDATE
/// (one write-lock acquisition) instead of reopening a connection per thread.
/// Returns the outcome directly (not a `Result`): an all-failed batch is still a
/// legitimate result the UI must render.
pub fn delete_sessions(platform_id: &str, session_ids: &[String]) -> BatchDeleteResult {
    let mut deleted: usize = 0;
    let mut failed: Vec<BatchDeleteFailure> = Vec::new();

    if session_ids.is_empty() {
        return BatchDeleteResult { deleted, failed };
    }

    if platform_id == "codex" {
        match codex::delete_codex_sessions(session_ids) {
            Ok(changed) => {
                deleted = changed;
                // rows-affected only gives the count flipped 0 -> 1; we cannot tell
                // WHICH ids were already archived or absent. Report the shortfall as
                // generic failures so the UI count ("deleted N, failed M") stays honest.
                let shortfall = session_ids.len().saturating_sub(changed);
                while failed.len() < shortfall {
                    failed.push(BatchDeleteFailure {
                        session_id: String::new(),
                        error: "Codex thread already archived or not found".to_string(),
                    });
                }
                return BatchDeleteResult { deleted, failed };
            }
            Err(err) => {
                // Whole batch failed (DB lock exhausted / missing DB). Do NOT fall
                // through to the per-item loop — it would reopen a readwrite connection
                // per id and each would re-fail under contention. Report every id.
                for id in session_ids {
                    failed.push(BatchDeleteFailure {
                        session_id: id.clone(),
                        error: err.clone(),
                    });
                }
                return BatchDeleteResult { deleted, failed };
            }
        }
    }

    // Claude / Kiro (and any unknown platform): per-item best-effort.
    for id in session_ids {
        match delete_session(platform_id, id) {
            Ok(()) => deleted += 1,
            Err(err) => failed.push(BatchDeleteFailure {
                session_id: id.clone(),
                error: err,
            }),
        }
    }
    BatchDeleteResult { deleted, failed }
}

fn build_resume_command(platform_id: &str, session_id: &str) -> Result<String, String> {
    // Always emit the paste-ready command. Agent Hub's GUI PATH is often
    // thinner than the user's terminal (especially on Windows), so probing
    // here would hide the command the user can still run in PowerShell.
    match platform_id {
        "claude-code" => Ok(format!("claude --resume {}", shell_quote(session_id))),
        "codex" => Ok(format!("codex resume {}", shell_quote(session_id))),
        "kiro" => Ok(format!(
            "kiro-cli chat --resume-id {}",
            shell_quote(session_id)
        )),
        "grok" => Ok(format!("grok --resume {}", shell_quote(session_id))),
        "kimi" => Ok(format!("kimi --session {}", shell_quote(session_id))),
        "qwen" => Ok(format!("qwen --resume {}", shell_quote(session_id))),
        "cursor" => Ok(format!("agent --resume={}", shell_quote(session_id))),
        "workbuddy" => {
            let bin = if command_exists("codebuddy") {
                "codebuddy"
            } else if command_exists("workbuddy") {
                "workbuddy"
            } else {
                "codebuddy"
            };
            Ok(format!("{} -r {}", bin, shell_quote(session_id)))
        }
        "antigravity" => Ok(format!(
            "agy --conversation={}",
            shell_quote(session_id)
        )),
        // ZCode is an Electron desktop app: sessions have no terminal resume
        // command. The resume modal surfaces this error instead of a command.
        "zcode" => Err(
            "ZCode is a desktop application and does not support terminal session resume."
                .to_string(),
        ),
        // DeepSeek Harness sessions are resumed from the dsh GUI (or a
        // headless profile), not through a stable CLI resume flag. Point the
        // user at `dsh web` instead of fabricating a broken command.
        "dsh" => Err(
            "DeepSeek Harness 会话由 dsh 界面管理，没有终端恢复命令。运行 `dsh web` 后在会话列表里点击继续该会话。"
                .to_string(),
        ),
        _ => Err(format!("Unsupported platform: {}", platform_id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_sessions_real_data_smoke_test() {
        let sessions = claude::list_claude_sessions_all().expect("claude scan should not fail");
        if sessions.is_empty() {
            return;
        }
        let first = &sessions[0];
        let page = claude::get_claude_messages(&first.id, 0, 50);
        if let Ok(messages) = page {
            assert!(messages.len() <= 50);
        }
    }

    #[test]
    fn codex_sessions_real_data_smoke_test() {
        let sessions = codex::list_codex_sessions_all().expect("codex scan should not fail");
        if sessions.is_empty() {
            return;
        }
        let first = &sessions[0];
        let page = codex::get_codex_messages(&first.id, 0, 50);
        if let Ok(messages) = page {
            assert!(messages.len() <= 50);
        }
    }

    #[test]
    fn pagination_advances_without_crashing() {
        let platforms = list_session_platforms(None).expect("session platforms should list");
        let Some(platform) = platforms.first() else {
            return;
        };
        let first_page =
            list_sessions(&platform.id, PATH_FILTER_ALL, 0, 50).expect("session list should load");
        let Some(session) = first_page.sessions.first() else {
            return;
        };
        // Tolerate load errors: other tests in this process temporarily override
        // HOME, and the resolver may see that value mid-flight. This test guards
        // pagination behavior, not transcript availability.
        let Ok(page1) = get_session_messages(&platform.id, &session.id, 0, 50) else {
            return;
        };
        let Ok(page2) = get_session_messages(&platform.id, &session.id, 50, 50) else {
            return;
        };
        assert!(first_page.limit <= 50);
        assert!(page1.len() <= 50);
        assert!(page2.len() <= 50);
        assert!(first_page.paths.iter().any(|path| path == PATH_FILTER_ALL));
        assert!(first_page
            .paths
            .iter()
            .any(|path| path == PATH_FILTER_UNKNOWN));
    }

    #[test]
    fn kiro_sessions_real_data_smoke_test() {
        let sessions = kiro::list_kiro_sessions_all().expect("kiro scan should not fail");
        if sessions.is_empty() {
            return;
        }
        let first = &sessions[0];
        let page = kiro::get_kiro_messages(&first.id, 0, 50);
        if let Ok(messages) = page {
            assert!(messages.len() <= 50);
        }
    }

    #[test]
    fn build_resume_command_for_kiro_contains_resume_id() {
        let command = build_resume_command("kiro", "abc-123").expect("command should build");
        assert!(command.contains("kiro-cli chat --resume-id"));
        assert!(command.contains(&shell_quote("abc-123")));
    }

    #[test]
    fn grok_sessions_real_data_smoke_test() {
        let sessions = grok::list_grok_sessions_all().expect("grok scan should not fail");
        if sessions.is_empty() {
            return;
        }
        let first = &sessions[0];
        let page = grok::get_grok_messages(&first.id, 0, 50);
        if let Ok(messages) = page {
            assert!(messages.len() <= 50);
        }
    }

    #[test]
    fn build_resume_command_for_grok_contains_resume_flag() {
        let command = build_resume_command("grok", "abc-123").expect("command should build");
        assert!(command.contains("grok --resume"));
        assert!(command.contains(&shell_quote("abc-123")));
    }

    #[test]
    fn kimi_sessions_real_data_smoke_test() {
        let sessions = kimi::list_kimi_sessions_all().expect("kimi scan should not fail");
        if sessions.is_empty() {
            return;
        }
        let first = &sessions[0];
        let page = kimi::get_kimi_messages(&first.id, 0, 50);
        if let Ok(messages) = page {
            assert!(messages.len() <= 50);
        }
    }

    #[test]
    fn build_resume_command_for_kimi_contains_session_flag() {
        let command = build_resume_command("kimi", "abc-123").expect("command should build");
        assert!(command.contains("kimi --session"));
        assert!(command.contains(&shell_quote("abc-123")));
    }

    #[test]
    fn qwen_sessions_real_data_smoke_test() {
        let sessions = qwen::list_qwen_sessions_all().expect("qwen scan should not fail");
        if sessions.is_empty() {
            return;
        }
        let first = &sessions[0];
        let page = qwen::get_qwen_messages(&first.id, 0, 50);
        if let Ok(messages) = page {
            assert!(messages.len() <= 50);
        }
    }

    #[test]
    fn build_resume_command_for_qwen_contains_resume_flag() {
        let command = build_resume_command("qwen", "abc-123").expect("command should build");
        assert!(command.contains("qwen --resume"));
        assert!(command.contains(&shell_quote("abc-123")));
    }

    #[test]
    fn build_resume_command_for_antigravity_and_workbuddy_without_path_probe() {
        let agy = build_resume_command("antigravity", "abc-123").expect("agy command should build");
        assert!(agy.contains("agy --conversation="));
        assert!(agy.contains(&shell_quote("abc-123")));

        let wb =
            build_resume_command("workbuddy", "abc-123").expect("workbuddy command should build");
        assert!(wb.contains(" -r "));
        assert!(wb.contains(&shell_quote("abc-123")));
        assert!(wb.starts_with("codebuddy ") || wb.starts_with("workbuddy "));
    }

    #[test]
    fn full_resume_command_chains_cd_with_shell_separator() {
        // claude-code does not probe PATH, so this stays deterministic.
        let command = build_full_resume_command("claude-code", "abc-123", "/tmp/proj")
            .expect("command should build");
        assert!(command.starts_with("cd "));
        assert!(command.contains("claude --resume"));
        #[cfg(target_os = "windows")]
        {
            assert!(
                command.contains("; claude --resume "),
                "Windows paste command must use PowerShell `;`, got {command}"
            );
            assert!(
                !command.contains(" & "),
                "cmd-style `&` is the PowerShell call operator and cannot chain, got {command}"
            );
        }
        #[cfg(not(target_os = "windows"))]
        {
            assert!(
                command.contains(" && claude --resume "),
                "Unix paste command must use `&&`, got {command}"
            );
        }
    }

    #[test]
    fn launch_resume_command_keeps_cmd_separator_on_windows() {
        let command = build_chained_resume_command(
            "claude-code",
            "abc-123",
            r"D:\Coding\proj",
            launch_resume_sep(),
        )
        .expect("command should build");
        #[cfg(target_os = "windows")]
        {
            assert!(command.contains(" & claude --resume "));
        }
        #[cfg(not(target_os = "windows"))]
        {
            assert!(command.contains(" && claude --resume "));
        }
    }

    #[test]
    fn zcode_sessions_real_data_smoke_test() {
        let sessions = zcode::list_zcode_sessions_all().expect("zcode scan should not fail");
        if sessions.is_empty() {
            return;
        }
        let first = &sessions[0];
        let page = zcode::get_zcode_messages(&first.id, 0, 50);
        if let Ok(messages) = page {
            assert!(messages.len() <= 50);
        }
    }

    #[test]
    fn build_resume_command_for_zcode_reports_desktop_app() {
        let err =
            build_resume_command("zcode", "sess_abc").expect_err("zcode resume should be rejected");
        assert!(err.contains("desktop application"));
    }

    #[test]
    fn resume_preview_smoke_test() {
        let platforms = list_session_platforms(None).expect("platforms should list");
        for platform in platforms {
            let Ok(page) = list_sessions(&platform.id, PATH_FILTER_ALL, 0, 1) else {
                continue;
            };
            let Some(session) = page.sessions.first() else {
                continue;
            };
            // ZCode/DSH have no terminal resume. Every other platform must
            // still produce a paste-ready command even if the CLI is missing.
            let preview =
                get_session_resume_preview(&platform.id, &session.id, &session.project_path);
            if platform.id == "zcode" || platform.id == "dsh" {
                assert!(
                    preview.is_err(),
                    "{} should reject terminal resume",
                    platform.id
                );
                continue;
            }
            let preview = preview.expect("resume preview should build");
            assert!(!preview.command.is_empty());
        }
    }

    #[test]
    fn delete_session_rejects_unknown_platform() {
        let err = delete_session("unknown-platform", "session-1")
            .expect_err("unknown platform should be rejected");
        assert!(err.contains("Unsupported platform"));
    }

    #[test]
    fn delete_sessions_best_effort_reports_all_failed_on_unknown_platform() {
        let result = delete_sessions(
            "does-not-exist",
            &["a".to_string(), "b".to_string(), "c".to_string()],
        );
        assert_eq!(result.deleted, 0);
        assert_eq!(result.failed.len(), 3);
        // Confirms one failure did not abort the loop (best-effort).
        let ids: Vec<&str> = result
            .failed
            .iter()
            .map(|f| f.session_id.as_str())
            .collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
    }

    #[test]
    fn delete_sessions_empty_input_is_empty_result() {
        let result = delete_sessions("claude-code", &[]);
        assert_eq!(result.deleted, 0);
        assert!(result.failed.is_empty());
    }

    #[test]
    fn filter_sessions_by_path_supports_all_unknown_and_exact_match() {
        let sessions = vec![
            models::SessionSummary {
                id: "1".to_string(),
                title: "a".to_string(),
                project_path: "/tmp/a".to_string(),
                model: None,
                started_at: 0,
                updated_at: 0,
                message_count: None,
                tokens_used: None,
                platform_id: "x".to_string(),
                source: None,
            },
            models::SessionSummary {
                id: "2".to_string(),
                title: "b".to_string(),
                project_path: "  ".to_string(),
                model: None,
                started_at: 0,
                updated_at: 0,
                message_count: None,
                tokens_used: None,
                platform_id: "x".to_string(),
                source: None,
            },
        ];

        let all = filter_sessions_by_path(sessions.clone(), PATH_FILTER_ALL);
        assert_eq!(all.len(), 2);
        let unknown = filter_sessions_by_path(sessions.clone(), PATH_FILTER_UNKNOWN);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].id, "2");
        let exact = filter_sessions_by_path(sessions.clone(), "/tmp/a");
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].id, "1");
        let trailing = filter_sessions_by_path(sessions, "/tmp/a/");
        assert_eq!(trailing.len(), 1);
        assert_eq!(trailing[0].id, "1");
    }

    #[test]
    fn normalize_project_path_windows_shapes() {
        assert_eq!(
            normalize_project_path(r"\\?\C:\Users\liuyang\.codex\worktrees\x").as_deref(),
            Some(r"C:\Users\liuyang\.codex\worktrees\x")
        );
        assert_eq!(
            normalize_project_path("/D:/Coding/mng-master-web").as_deref(),
            Some(r"D:\Coding\mng-master-web")
        );
        assert_eq!(
            normalize_project_path("file:///D:/feishu-bot-go").as_deref(),
            Some(r"D:\feishu-bot-go")
        );
    }
}

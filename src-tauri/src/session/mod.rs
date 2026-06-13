mod claude;
mod codex;
mod kiro;
mod models;

use std::path::Path;
use std::process::Command;

pub use models::{SessionListPage, SessionMessage, SessionPlatform, SessionTerminalOption, SessionSearchResult};

const MAX_SESSION_PAGE_SIZE: usize = 200;
const PATH_FILTER_ALL: &str = "all";
const PATH_FILTER_UNKNOWN: &str = "unknown";

pub fn list_session_platforms() -> Result<Vec<SessionPlatform>, String> {
    let mut platforms = Vec::new();

    let claude_count = claude::count_claude_sessions()?;
    if claude_count > 0 {
        platforms.push(SessionPlatform {
            id: "claude-code".to_string(),
            display_name: "Claude Code".to_string(),
            session_count: claude_count,
        });
    }

    let codex_count = codex::count_codex_sessions()?;
    if codex_count > 0 {
        platforms.push(SessionPlatform {
            id: "codex-cli".to_string(),
            display_name: "Codex CLI".to_string(),
            session_count: codex_count,
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

    Ok(platforms)
}

pub fn list_sessions(
    platform_id: &str,
    path_filter: &str,
    offset: usize,
    limit: usize,
) -> Result<SessionListPage, String> {
    let page_limit = limit.clamp(1, MAX_SESSION_PAGE_SIZE);
    let all_sessions = match platform_id {
        "claude-code" => claude::list_claude_sessions_all()?,
        "codex-cli" => codex::list_codex_sessions_all()?,
        "kiro" => kiro::list_kiro_sessions_all()?,
        _ => return Err(format!("Unsupported platform: {}", platform_id)),
    };
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

fn normalize_project_path(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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
        .filter(|session| normalize_project_path(&session.project_path).as_deref() == Some(filter))
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
    let resume_command = build_resume_command(platform_id, session_id)?;

    let full_command = if project_path.trim().is_empty() {
        resume_command
    } else {
        #[cfg(target_os = "macos")]
        let sep = " && ";
        #[cfg(not(target_os = "macos"))]
        let sep = " & ";
        format!("cd {}{}{}", shell_quote(project_path), sep, resume_command)
    };

    launch_terminal_with_command(terminal_id, &full_command)?;
    Ok(full_command)
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(not(target_os = "macos"))]
fn shell_quote(value: &str) -> String {
    // Windows: wrap in double quotes, escape inner double quotes with backslash
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn command_exists(command: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {} >/dev/null 2>&1", command))
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

#[cfg(target_os = "macos")]
fn is_terminal_available(terminal_id: &str) -> bool {
    match terminal_id {
        "terminal-default" => true,
        "iterm" => {
            Path::new("/Applications/iTerm.app").exists()
                || Path::new("/Applications/iTerm2.app").exists()
                || command_exists("iterm2")
        }
        "ghostty" => {
            command_exists("ghostty")
                || Path::new("/Applications/Ghostty.app").exists()
                || Path::new("/Applications/Ghostty.app/Contents/MacOS/ghostty").exists()
        }
        "warp" => command_exists("warp") || Path::new("/Applications/Warp.app").exists(),
        _ => false,
    }
}

#[cfg(target_os = "windows")]
fn is_terminal_available(terminal_id: &str) -> bool {
    match terminal_id {
        "terminal-default" => true, // default console always available
        "powershell" => {
            // Prefer pwsh (PowerShell 7+), fall back to powershell (Windows PowerShell 5)
            Command::new("where")
                .arg("pwsh.exe")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
                || Command::new("where")
                    .arg("powershell.exe")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
        }
        "windows-terminal" => {
            Command::new("where")
                .arg("wt.exe")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
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
            } else {
                "/Applications/Ghostty.app/Contents/MacOS/ghostty".to_string()
            };
            if !Path::new(&bin).exists() && bin != "ghostty" {
                return Err("Ghostty is not installed.".to_string());
            }
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
            if !Path::new("/Applications/Warp.app").exists() {
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
    let bat_content = format!("@echo off\n{}\npause\n", escaped);
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
            // Prefer pwsh (PowerShell 7+), fall back to powershell (5)
            let ps = if Command::new("where")
                .arg("pwsh.exe")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                "pwsh.exe"
            } else {
                "powershell.exe"
            };
            let ps_script = format!(
                "Start-Process cmd.exe -ArgumentList '/k','{}'",
                bat_path.to_string_lossy().replace('\'', "''")
            );
            Command::new(ps)
                .arg("-Command")
                .arg(&ps_script)
                .spawn()
                .map_err(|e| format!("Failed to launch terminal: {}", e))?;
            Ok(())
        }
        _ => {
            // Default: use PowerShell Start-Process to avoid the cmd.exe flash
            let bat_str = bat_path.to_string_lossy().replace('\'', "''");
            let ps_script =
                format!("Start-Process cmd.exe -ArgumentList '/k','{}'", bat_str);
            Command::new("powershell.exe")
                .arg("-WindowStyle")
                .arg("Hidden")
                .arg("-Command")
                .arg(&ps_script)
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
        "codex-cli" => codex::get_codex_messages(session_id, offset, limit),
        "kiro" => kiro::get_kiro_messages(session_id, offset, limit),
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
        "codex-cli" => codex::search_codex_messages(&query_lower),
        "kiro" => kiro::search_kiro_messages(&query_lower),
        _ => Err(format!("Unsupported platform: {}", platform_id)),
    }
}

pub fn delete_session(platform_id: &str, session_id: &str) -> Result<(), String> {
    match platform_id {
        "claude-code" => claude::delete_claude_session(session_id),
        "codex-cli" => codex::delete_codex_session(session_id),
        "kiro" => kiro::delete_kiro_session(session_id),
        _ => Err(format!("Unsupported platform: {}", platform_id)),
    }
}

fn build_resume_command(platform_id: &str, session_id: &str) -> Result<String, String> {
    match platform_id {
        "claude-code" => Ok(format!("claude --resume {}", shell_quote(session_id))),
        "codex-cli" => Ok(format!("codex resume {}", shell_quote(session_id))),
        "kiro" => {
            if !command_exists("kiro-cli") {
                return Err("Kiro CLI is not available on PATH.".to_string());
            }
            Ok(format!(
                "kiro-cli chat --resume-id {}",
                shell_quote(session_id)
            ))
        }
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
        let platforms = list_session_platforms().expect("session platforms should list");
        let Some(platform) = platforms.first() else {
            return;
        };
        let first_page =
            list_sessions(&platform.id, PATH_FILTER_ALL, 0, 50).expect("session list should load");
        let Some(session) = first_page.sessions.first() else {
            return;
        };
        let page1 =
            get_session_messages(&platform.id, &session.id, 0, 50).expect("page1 should load");
        let page2 =
            get_session_messages(&platform.id, &session.id, 50, 50).expect("page2 should load");
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
        if !command_exists("kiro-cli") {
            return;
        }
        let command = build_resume_command("kiro", "abc-123").expect("command should build");
        assert!(command.contains("kiro-cli chat --resume-id"));
        assert!(command.contains("'abc-123'"));
    }

    #[test]
    fn delete_session_rejects_unknown_platform() {
        let err = delete_session("unknown-platform", "session-1")
            .expect_err("unknown platform should be rejected");
        assert!(err.contains("Unsupported platform"));
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
            },
        ];

        let all = filter_sessions_by_path(sessions.clone(), PATH_FILTER_ALL);
        assert_eq!(all.len(), 2);
        let unknown = filter_sessions_by_path(sessions.clone(), PATH_FILTER_UNKNOWN);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].id, "2");
        let exact = filter_sessions_by_path(sessions, "/tmp/a");
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].id, "1");
    }
}

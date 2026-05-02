mod claude;
mod codex;
mod kiro;
mod models;

use std::path::Path;
use std::process::Command;

pub use models::{SessionListPage, SessionMessage, SessionPlatform, SessionTerminalOption};

const MAX_SESSION_PAGE_SIZE: usize = 200;

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
    offset: usize,
    limit: usize,
) -> Result<SessionListPage, String> {
    let page_limit = limit.clamp(1, MAX_SESSION_PAGE_SIZE);
    let (total, sessions) = match platform_id {
        "claude-code" => claude::list_claude_sessions(offset, page_limit)?,
        "codex-cli" => codex::list_codex_sessions(offset, page_limit)?,
        "kiro" => kiro::list_kiro_sessions(offset, page_limit)?,
        _ => return Err(format!("Unsupported platform: {}", platform_id)),
    };
    let has_more = offset.saturating_add(sessions.len()) < total;
    Ok(SessionListPage {
        total,
        offset,
        limit: page_limit,
        has_more,
        sessions,
    })
}

pub fn list_session_terminals() -> Vec<SessionTerminalOption> {
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
        format!("cd {} && {}", shell_quote(project_path), resume_command)
    };

    launch_terminal_with_command(terminal_id, &full_command)?;
    Ok(full_command)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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

#[cfg(not(target_os = "macos"))]
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

#[cfg(not(target_os = "macos"))]
fn launch_terminal_with_command(_terminal_id: &str, _command: &str) -> Result<(), String> {
    Err("Session resume terminal launcher is currently macOS-only.".to_string())
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
        let (_total, sessions) =
            claude::list_claude_sessions(0, 50).expect("claude scan should not fail");
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
        let (_total, sessions) =
            codex::list_codex_sessions(0, 50).expect("codex scan should not fail");
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
        let first_page = list_sessions(&platform.id, 0, 50).expect("session list should load");
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
    }

    #[test]
    fn kiro_sessions_real_data_smoke_test() {
        let (_total, sessions) =
            kiro::list_kiro_sessions(0, 50).expect("kiro scan should not fail");
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
}

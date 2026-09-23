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
mod omp;
mod opencode;
mod qwen;
mod workbuddy;
mod zcode;

#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::process::Command;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

pub use models::{
    AgentSessionStats, BatchDeleteFailure, BatchDeleteResult, SessionExportResult, SessionListPage,
    SessionMessage, SessionMessageStats, SessionPlatform, SessionResumePreview,
    SessionSearchResult, SessionStatsReport, SessionTerminalOption,
};

#[cfg(target_os = "windows")]
use crate::win_console::suppress_console;

const MAX_SESSION_PAGE_SIZE: usize = 1000;
const PATH_FILTER_ALL: &str = "all";
const PATH_FILTER_UNKNOWN: &str = "unknown";

/// Sessions-capable platforms in sidebar order (mirrors platform/registry.rs).
/// `count_sessions` is the cheap existence probe shared by the sidebar badges
/// and the statistics report.
struct SessionPlatformSpec {
    id: &'static str,
    display_name: &'static str,
    count_sessions: fn() -> Result<usize, String>,
}

const SESSION_PLATFORM_SPECS: &[SessionPlatformSpec] = &[
    SessionPlatformSpec { id: "codex", display_name: "Codex", count_sessions: codex::count_codex_sessions },
    SessionPlatformSpec { id: "claude-code", display_name: "Claude Code", count_sessions: claude::count_claude_sessions },
    SessionPlatformSpec { id: "cursor", display_name: "Cursor", count_sessions: cursor::count_cursor_sessions },
    SessionPlatformSpec { id: "antigravity", display_name: "Antigravity", count_sessions: antigravity::count_antigravity_sessions },
    SessionPlatformSpec { id: "grok", display_name: "Grok Build", count_sessions: grok::count_grok_sessions },
    SessionPlatformSpec { id: "kimi", display_name: "Kimi Code", count_sessions: kimi::count_kimi_sessions },
    SessionPlatformSpec { id: "qwen", display_name: "Qwen Code", count_sessions: qwen::count_qwen_sessions },
    SessionPlatformSpec { id: "zcode", display_name: "ZCode", count_sessions: zcode::count_zcode_sessions },
    SessionPlatformSpec { id: "workbuddy", display_name: "WorkBuddy", count_sessions: workbuddy::count_workbuddy_sessions },
    SessionPlatformSpec { id: "kiro", display_name: "Kiro", count_sessions: kiro::count_kiro_sessions },
    SessionPlatformSpec { id: "dsh", display_name: "DeepSeek Harness", count_sessions: dsh::count_dsh_sessions },
    SessionPlatformSpec { id: "omp", display_name: "Oh My Pi", count_sessions: omp::count_omp_sessions },
    SessionPlatformSpec { id: "opencode", display_name: "OpenCode", count_sessions: opencode::count_opencode_sessions },
];

/// Normalized path filter: `None` means "every directory".
fn path_filter_value(path_filter: Option<&str>) -> Option<&str> {
    path_filter
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != PATH_FILTER_ALL)
}

pub fn list_session_platforms(path_filter: Option<&str>) -> Result<Vec<SessionPlatform>, String> {
    let filter = path_filter_value(path_filter);
    let mut platforms = Vec::new();

    for spec in SESSION_PLATFORM_SPECS {
        let total = (spec.count_sessions)()?;
        if total == 0 {
            continue;
        }
        // Unfiltered: the probe already answered. Filtered: the count that
        // matters is the one inside the selected directory, which needs the
        // full list (a platform may legitimately end up at zero and still be
        // listed, so the user sees the scope emptied it).
        let session_count = match filter {
            None => total,
            Some(path) => filter_sessions_by_path(list_sessions_all(spec.id)?, path).len(),
        };
        platforms.push(SessionPlatform {
            id: spec.id.to_string(),
            display_name: spec.display_name.to_string(),
            session_count,
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
        "omp" => omp::list_omp_sessions_all(),
        "opencode" => opencode::list_opencode_sessions_all(),
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

// ---------------------------------------------------------------------------
// Statistics (Sessions → All)
// ---------------------------------------------------------------------------
// Counting means walking every transcript in the window, which is the one
// genuinely expensive read in this module. Two things keep it affordable:
// the window is applied to the session list *before* any file is opened, and
// counts are memoized per session keyed by `updated_at`, so switching between
// windows only counts what changed.

/// Transcript page size while counting. Pages bound peak memory (one page of
/// message bodies) while keeping the re-read cost per session at one pass for
/// everything but the very largest transcripts.
const STATS_PAGE_SIZE: usize = 1000;
/// Upper bound of parallel transcript readers. Sessions are independent files
/// (or rows), so this is a plain work-stealing split of the session list.
const STATS_MAX_WORKERS: usize = 6;

struct CachedMessageStats {
    /// Session `updated_at` the tally was computed from — a session that grew
    /// since then has a different value and is recounted.
    updated_at: i64,
    stats: SessionMessageStats,
}

fn message_stats_cache() -> &'static Mutex<HashMap<String, CachedMessageStats>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedMessageStats>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Session ids are only unique per platform, so the cache key carries both.
fn stats_cache_key(platform_id: &str, session_id: &str) -> String {
    format!("{platform_id}\u{1f}{session_id}")
}

/// Some storages record seconds (Cursor), the rest milliseconds. Window
/// comparisons must normalize first.
fn session_time_millis(value: i64) -> i64 {
    if value > 0 && value < 1_000_000_000_000 {
        value.saturating_mul(1000)
    } else {
        value
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

/// Fold one page of roles into the running tally. `in_assistant_turn` carries
/// the "still inside the same assistant turn" state across pages, so a turn
/// split by paging is still counted once.
fn fold_role_counts<'a>(
    roles: impl IntoIterator<Item = &'a str>,
    stats: &mut SessionMessageStats,
    in_assistant_turn: &mut bool,
) {
    for role in roles {
        if role == "user" {
            stats.user = stats.user.saturating_add(1);
            *in_assistant_turn = false;
        } else if !*in_assistant_turn {
            stats.assistant = stats.assistant.saturating_add(1);
            *in_assistant_turn = true;
        }
    }
}

/// Tally one session by paging through the same records the message panel
/// renders, so the footer always agrees with the bubbles on screen: every
/// user record is one message, and consecutive assistant records form one
/// turn (tool round-trips inside a turn are not separate replies).
pub fn count_session_messages(
    platform_id: &str,
    session_id: &str,
) -> Result<SessionMessageStats, String> {
    let mut stats = SessionMessageStats::default();
    let mut offset = 0usize;
    let mut in_assistant_turn = false;

    loop {
        let page = get_session_messages(platform_id, session_id, offset, STATS_PAGE_SIZE)?;
        if page.is_empty() {
            break;
        }
        let received = page.len();
        fold_role_counts(
            page.iter().map(|message| message.role.as_str()),
            &mut stats,
            &mut in_assistant_turn,
        );
        offset = offset.saturating_add(received);
        if received < STATS_PAGE_SIZE {
            break;
        }
    }

    stats.total = stats.user.saturating_add(stats.assistant);
    Ok(stats)
}

fn cached_session_message_stats(
    platform_id: &str,
    session: &models::SessionSummary,
) -> Result<SessionMessageStats, String> {
    let key = stats_cache_key(platform_id, &session.id);
    if let Ok(cache) = message_stats_cache().lock() {
        if let Some(entry) = cache.get(&key) {
            if entry.updated_at == session.updated_at {
                return Ok(entry.stats);
            }
        }
    }

    let stats = count_session_messages(platform_id, &session.id)?;
    if let Ok(mut cache) = message_stats_cache().lock() {
        cache.insert(
            key,
            CachedMessageStats {
                updated_at: session.updated_at,
                stats,
            },
        );
    }
    Ok(stats)
}

/// Drop cache entries for sessions that no longer exist. Keyed per platform so
/// a scan of one platform never evicts another's tallies.
fn prune_stats_cache(platform_id: &str, sessions: &[models::SessionSummary]) {
    let live = sessions
        .iter()
        .map(|session| session.id.as_str())
        .collect::<HashSet<_>>();
    if let Ok(mut cache) = message_stats_cache().lock() {
        cache.retain(|key, _| match key.split_once('\u{1f}') {
            Some((platform, session_id)) => platform != platform_id || live.contains(session_id),
            None => false,
        });
    }
}

fn stats_worker_count(sessions: usize) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(2);
    cpus.min(STATS_MAX_WORKERS).min(sessions.max(1)).max(1)
}

/// Sum the tallies of every session of one Agent. Sessions are counted on a
/// few threads at once; a session that cannot be read (deleted mid-scan,
/// corrupt transcript) is skipped and reported through the failure count
/// instead of failing the whole report.
fn agent_message_stats(
    platform_id: &str,
    sessions: &[models::SessionSummary],
) -> (SessionMessageStats, u32, i64) {
    if sessions.is_empty() {
        return (SessionMessageStats::default(), 0, 0);
    }

    let next = AtomicUsize::new(0);
    let partials = std::thread::scope(|scope| {
        let handles = (0..stats_worker_count(sessions.len()))
            .map(|_| {
                scope.spawn(|| {
                    let mut stats = SessionMessageStats::default();
                    let mut failed = 0u32;
                    let mut last_active_at = 0i64;
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(session) = sessions.get(index) else {
                            break;
                        };
                        last_active_at = last_active_at.max(session_time_millis(session.updated_at));
                        match cached_session_message_stats(platform_id, session) {
                            Ok(counted) => stats.add(counted),
                            Err(_) => failed = failed.saturating_add(1),
                        }
                    }
                    (stats, failed, last_active_at)
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_default())
            .collect::<Vec<(SessionMessageStats, u32, i64)>>()
    });

    partials
        .into_iter()
        .fold(
            (SessionMessageStats::default(), 0u32, 0i64),
            |(mut stats, failed, last_active_at), (part_stats, part_failed, part_last)| {
                stats.add(part_stats);
                (stats, failed.saturating_add(part_failed), last_active_at.max(part_last))
            },
        )
}

/// Per-Agent session/message statistics for the trailing `days` window,
/// optionally scoped to one workspace directory. A session counts as inside
/// the window when its latest activity is; its messages are then counted in
/// full (transcripts have no per-record window semantics that survive every
/// storage format — several adapters record no usable per-message timestamp).
pub fn session_stats_report(
    days: u32,
    path_filter: Option<&str>,
) -> Result<SessionStatsReport, String> {
    let days = days.clamp(1, 3650);
    let generated_at = now_millis();
    let since = generated_at.saturating_sub(i64::from(days).saturating_mul(86_400_000));
    let filter = path_filter_value(path_filter);

    let mut agents = Vec::new();
    let mut totals = SessionMessageStats::default();
    let mut session_count = 0u32;
    let mut inactive_agents = 0u32;

    for spec in SESSION_PLATFORM_SPECS {
        let mut sessions = match list_sessions_all(spec.id) {
            Ok(sessions) => sessions,
            Err(error) => {
                // One unreadable storage (locked DB, missing directory) must
                // not sink the other Agents' statistics.
                log::warn!("session stats: listing {} failed: {}", spec.id, error);
                continue;
            }
        };
        if sessions.is_empty() {
            continue;
        }
        prune_stats_cache(spec.id, &sessions);
        if let Some(path) = filter {
            sessions = filter_sessions_by_path(sessions, path);
        }
        sessions.retain(|session| session_time_millis(session.updated_at) >= since);
        if sessions.is_empty() {
            inactive_agents = inactive_agents.saturating_add(1);
            continue;
        }

        let counted = u32::try_from(sessions.len()).unwrap_or(u32::MAX);
        let (messages, failed_sessions, last_active_at) = agent_message_stats(spec.id, &sessions);
        session_count = session_count.saturating_add(counted);
        totals.add(messages);
        agents.push(AgentSessionStats {
            platform_id: spec.id.to_string(),
            display_name: spec.display_name.to_string(),
            session_count: counted,
            messages,
            last_active_at,
            failed_sessions,
        });
    }

    agents.sort_by(|left, right| {
        right
            .messages
            .total
            .cmp(&left.messages.total)
            .then(right.session_count.cmp(&left.session_count))
            .then(left.display_name.cmp(&right.display_name))
    });

    Ok(SessionStatsReport {
        days,
        since,
        generated_at,
        session_count,
        totals,
        agents,
        inactive_agents,
    })
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
        "omp" => omp::last_omp_messages(session_id),
        "opencode" => opencode::last_opencode_messages(session_id),
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
        "omp" => omp::get_omp_messages(session_id, offset, limit),
        "opencode" => opencode::get_opencode_messages(session_id, offset, limit),
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
        "omp" => omp::search_omp_messages(&query_lower),
        "opencode" => opencode::search_opencode_messages(&query_lower),
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
        "omp" => omp::delete_omp_session(session_id),
        "opencode" => opencode::delete_opencode_session(session_id),
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
        "omp" => Ok(format!("omp -r {}", shell_quote(session_id))),
        "opencode" => Ok(format!("opencode -s {}", shell_quote(session_id))),
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

    #[test]
    fn role_counts_merge_assistant_turns_but_not_user_messages() {
        // Mirrors what the panel renders: 2 user bubbles, 2 assistant turns
        // (the middle pair is one turn split by a tool round-trip).
        let roles = ["user", "assistant", "assistant", "user", "assistant"];
        let mut stats = SessionMessageStats::default();
        let mut in_assistant_turn = false;
        fold_role_counts(roles, &mut stats, &mut in_assistant_turn);
        stats.total = stats.user + stats.assistant;
        assert_eq!(
            (stats.total, stats.user, stats.assistant),
            (4, 2, 2),
            "consecutive assistant records must fold into one turn"
        );
    }

    #[test]
    fn role_counts_carry_the_turn_across_pages() {
        // A turn split by the page boundary must not be counted twice.
        let mut stats = SessionMessageStats::default();
        let mut in_assistant_turn = false;
        fold_role_counts(["user", "assistant"], &mut stats, &mut in_assistant_turn);
        fold_role_counts(["assistant", "user"], &mut stats, &mut in_assistant_turn);
        stats.total = stats.user + stats.assistant;
        assert_eq!((stats.total, stats.user, stats.assistant), (3, 2, 1));
    }

    #[test]
    fn empty_transcript_counts_as_zero() {
        let mut stats = SessionMessageStats::default();
        let mut in_assistant_turn = false;
        fold_role_counts([], &mut stats, &mut in_assistant_turn);
        assert_eq!((stats.total, stats.user, stats.assistant), (0, 0, 0));
    }

    #[test]
    fn session_time_accepts_seconds_and_millis() {
        assert_eq!(session_time_millis(1_787_845_175), 1_787_845_175_000);
        assert_eq!(session_time_millis(1_787_845_175_316), 1_787_845_175_316);
        assert_eq!(session_time_millis(0), 0);
    }

    #[test]
    fn path_filter_all_and_blank_mean_every_directory() {
        assert_eq!(path_filter_value(None), None);
        assert_eq!(path_filter_value(Some("all")), None);
        assert_eq!(path_filter_value(Some("  ")), None);
        assert_eq!(path_filter_value(Some(" /tmp/a ")), Some("/tmp/a"));
    }

    /// The tally pages a transcript 1000 records at a time; the message panel
    /// pages the same transcript 30 at a time. Both must fold to the same
    /// numbers — that is what keeps the footer in step with the bubbles on
    /// screen, including assistant turns straddling a page boundary.
    #[test]
    fn message_stats_match_the_panel_paging() {
        let Ok(platforms) = list_session_platforms(None) else {
            return;
        };
        for platform in platforms.iter().take(4) {
            let Ok(sessions) = list_sessions_all(&platform.id) else {
                continue;
            };
            let Some(session) = sessions.first() else {
                continue;
            };
            let Ok(stats) = count_session_messages(&platform.id, &session.id) else {
                continue;
            };

            let mut manual = SessionMessageStats::default();
            let mut in_assistant_turn = false;
            let mut offset = 0usize;
            loop {
                let Ok(page) = get_session_messages(&platform.id, &session.id, offset, 30) else {
                    break;
                };
                if page.is_empty() {
                    break;
                }
                let received = page.len();
                fold_role_counts(
                    page.iter().map(|message| message.role.as_str()),
                    &mut manual,
                    &mut in_assistant_turn,
                );
                offset += received;
                if received < 30 {
                    break;
                }
            }
            manual.total = manual.user + manual.assistant;

            assert_eq!(
                (stats.total, stats.user, stats.assistant),
                (manual.total, manual.user, manual.assistant),
                "platform {} session {}",
                platform.id,
                session.id
            );
        }
    }

    #[test]
    fn build_resume_command_for_opencode_contains_session_flag() {
        let command = build_resume_command("opencode", "ses_test123").expect("resume command");
        assert!(command.contains("opencode -s"));
        assert!(command.contains("ses_test123"));
    }

    #[test]
    fn opencode_sessions_real_data_smoke_test() {
        let Ok(sessions) = list_sessions_all("opencode") else {
            return;
        };
        for session in sessions.iter().take(3) {
            let messages = get_session_messages("opencode", &session.id, 0, 10).unwrap_or_default();
            for msg in messages {
                assert!(!msg.role.is_empty());
                assert!(!msg.content.is_empty() || msg.thinking.is_some());
            }
        }
    }
}

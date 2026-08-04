use crate::platform::Platform;
use crate::session;
use crate::session_monitor::{AgentKind, HookAction, MonitorSnapshot};
use crate::skill::Skill;
use crate::state::SafeState;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use base64::Engine as _;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use futures_util::StreamExt;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use http::header::{HeaderValue, ACCEPT, CONTENT_LENGTH, CONTENT_RANGE, RANGE};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use minisign_verify::{PublicKey, Signature};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use reqwest::{ClientBuilder, StatusCode, Url};
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::time::Duration;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::{ipc::Channel, Manager, ResourceId, Runtime, Webview};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri_plugin_updater::Update;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
const UPDATE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Error returned when a download is cancelled by the user.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const CANCELLED_ERROR: &str = "__cancelled__";

// --- View types ---

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlatformView {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub skill_dir: String,
}

impl From<&Platform> for PlatformView {
    fn from(p: &Platform) -> Self {
        Self {
            id: p.id.clone(),
            display_name: p.display_name.clone(),
            description: p.description.clone(),
            skill_dir: p.skill_dir.display().to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillSummary {
    pub name: String,
    pub folder: String,
    pub version: Option<String>,
    pub description: String,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
    pub total_size: u64,
    pub modified_at: Option<u64>,
}

impl From<&Skill> for SkillSummary {
    fn from(s: &Skill) -> Self {
        Self {
            name: s.name.clone(),
            folder: s.folder.clone(),
            version: s.version.clone(),
            description: s.description.clone(),
            is_symlink: s.is_symlink,
            symlink_target: s.symlink_target.as_ref().map(|p| p.display().to_string()),
            total_size: s.total_size,
            modified_at: s
                .modified_at
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillDetail {
    pub name: String,
    pub folder: String,
    pub version: Option<String>,
    pub description: String,
    pub platform_id: String,
    pub path: String,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
    pub files: Vec<String>,
    pub total_size: u64,
    pub body: String,
    pub modified_at: Option<u64>,
}

impl From<&Skill> for SkillDetail {
    fn from(s: &Skill) -> Self {
        Self {
            name: s.name.clone(),
            folder: s.folder.clone(),
            version: s.version.clone(),
            description: s.description.clone(),
            platform_id: s.platform_id.clone(),
            path: s.path.display().to_string(),
            is_symlink: s.is_symlink,
            symlink_target: s.symlink_target.as_ref().map(|p| p.display().to_string()),
            files: s.files.iter().map(|f| f.display().to_string()).collect(),
            total_size: s.total_size,
            body: s.body.clone(),
            modified_at: s
                .modified_at
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub platform_id: String,
    pub platform_name: String,
    pub skill_name: String,
    pub folder: String,
    pub description: String,
}

// --- Error ---

#[derive(Debug, Clone, serde::Serialize)]
pub enum CommandError {
    NotFound(String),
    SyncError(String),
    General(String),
}

// --- Helpers ---

fn find_skill<'a>(platform: &'a Platform, skill_name: &str, folder: &str) -> Option<&'a Skill> {
    platform
        .skills
        .iter()
        .find(|sk| sk.name == skill_name && sk.folder == folder)
}

fn resolve_workspace_dir(workspace_dir: Option<&str>) -> Result<Option<PathBuf>, CommandError> {
    let Some(raw) = workspace_dir.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let path = PathBuf::from(raw);
    if !path.is_dir() {
        return Err(CommandError::NotFound(format!(
            "Workspace directory does not exist: {}",
            path.display()
        )));
    }
    path.canonicalize()
        .map(Some)
        .map_err(|error| CommandError::General(format!("Unable to resolve workspace: {error}")))
}

fn scoped_platform(
    state: &mut crate::state::AppState,
    platform_id: &str,
    workspace_dir: Option<&str>,
) -> Result<Platform, CommandError> {
    let platform = state
        .platforms
        .iter_mut()
        .find(|platform| platform.id == platform_id)
        .ok_or_else(|| CommandError::NotFound(format!("Platform {platform_id} not found")))?;

    if let Some(workspace) = resolve_workspace_dir(workspace_dir)? {
        let mut scoped = platform.clone();
        scoped.skill_dir = crate::platform::workspace_skill_dir(platform_id, &workspace)
            .ok_or_else(|| {
                CommandError::NotFound(
                    "Workspace skills are not supported for this platform".into(),
                )
            })?;
        crate::platform::invalidate_platform_skills(&mut scoped);
        crate::platform::load_platform_skills(&mut scoped);
        Ok(scoped)
    } else {
        crate::platform::load_platform_skills(platform);
        Ok(platform.clone())
    }
}

// --- Commands ---

#[tauri::command]
pub fn list_platforms(state: tauri::State<'_, SafeState>) -> Vec<PlatformView> {
    let s = state.lock().unwrap();
    s.platforms.iter().map(PlatformView::from).collect()
}

#[tauri::command]
pub fn get_platform_skills(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    workspace_dir: Option<String>,
) -> Vec<SkillSummary> {
    let mut s = state.lock().unwrap();
    scoped_platform(&mut s, &platform_id, workspace_dir.as_deref())
        .map(|platform| platform.skills.iter().map(SkillSummary::from).collect())
        .unwrap_or_default()
}

#[tauri::command]
pub fn refresh_platform_skills(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    workspace_dir: Option<String>,
) -> Vec<SkillSummary> {
    let mut s = state.lock().unwrap();
    if workspace_dir
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        return scoped_platform(&mut s, &platform_id, workspace_dir.as_deref())
            .map(|platform| platform.skills.iter().map(SkillSummary::from).collect())
            .unwrap_or_default();
    }
    if let Some(p) = s.platforms.iter_mut().find(|p| p.id == platform_id) {
        crate::platform::invalidate_platform_skills(p);
        crate::platform::load_platform_skills(p);
        p.skills.iter().map(SkillSummary::from).collect()
    } else {
        Vec::new()
    }
}

#[tauri::command]
pub fn get_skill_detail(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    skill_name: String,
    folder: String,
    workspace_dir: Option<String>,
) -> Result<SkillDetail, CommandError> {
    let mut s = state.lock().unwrap();
    let platform = scoped_platform(&mut s, &platform_id, workspace_dir.as_deref())?;
    let skill = find_skill(&platform, &skill_name, &folder)
        .ok_or_else(|| CommandError::NotFound(format!("Skill {} not found", skill_name)))?;
    Ok(SkillDetail::from(skill))
}

/// Reveal a skill's directory in the OS file manager:
///   - Windows: `explorer.exe <path>` (selects the folder)
///   - macOS:   `open <path>`
///   - Linux:   `xdg-open <path>`
#[tauri::command]
pub fn open_skill_folder(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    skill_name: String,
    folder: String,
    workspace_dir: Option<String>,
) -> Result<(), CommandError> {
    let mut s = state.lock().unwrap();
    let platform = scoped_platform(&mut s, &platform_id, workspace_dir.as_deref())?;
    let skill = find_skill(&platform, &skill_name, &folder)
        .ok_or_else(|| CommandError::NotFound(format!("Skill {} not found", skill_name)))?;

    let path = &skill.path;
    if !path.exists() {
        return Err(CommandError::General(format!(
            "Skill path does not exist: {}",
            path.display()
        )));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map_err(|e| CommandError::General(format!("Failed to open folder: {e}")))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| CommandError::General(format!("Failed to open folder: {e}")))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| CommandError::General(format!("Failed to open folder: {e}")))?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_diff_candidates(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    skill_name: String,
    folder: String,
) -> Vec<PlatformView> {
    let mut s = state.lock().unwrap();
    crate::platform::ensure_all_skills_loaded(&mut s.platforms);
    s.platforms
        .iter()
        .filter(|p| {
            p.id != platform_id
                && p.skills
                    .iter()
                    .any(|sk| sk.name == skill_name && sk.folder == folder)
        })
        .map(PlatformView::from)
        .collect()
}

#[tauri::command]
pub fn diff_skills_cmd(
    state: tauri::State<'_, SafeState>,
    source_platform_id: String,
    target_platform_id: String,
    skill_name: String,
    folder: String,
) -> Result<crate::diff::DiffResult, CommandError> {
    let mut s = state.lock().unwrap();
    for id in [&source_platform_id, &target_platform_id] {
        if let Some(p) = s.platforms.iter_mut().find(|p| &p.id == id) {
            crate::platform::load_platform_skills(p);
        }
    }
    let source_platform = s
        .platforms
        .iter()
        .find(|p| p.id == source_platform_id)
        .ok_or_else(|| CommandError::NotFound("Source platform not found".into()))?;
    let target_platform = s
        .platforms
        .iter()
        .find(|p| p.id == target_platform_id)
        .ok_or_else(|| CommandError::NotFound("Target platform not found".into()))?;
    let source_skill = find_skill(source_platform, &skill_name, &folder)
        .cloned()
        .ok_or_else(|| CommandError::NotFound("Source skill not found".into()))?;
    let target_skill = find_skill(target_platform, &skill_name, &folder)
        .cloned()
        .ok_or_else(|| CommandError::NotFound("Target skill not found".into()))?;
    Ok(crate::diff::diff_skills(&source_skill, &target_skill))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncTarget {
    pub id: String,
    pub display_name: String,
    pub has_skill: bool,
}

#[tauri::command]
pub fn get_sync_targets(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    skill_name: String,
    folder: String,
) -> Vec<SyncTarget> {
    let mut s = state.lock().unwrap();
    crate::platform::ensure_all_skills_loaded(&mut s.platforms);
    s.platforms
        .iter()
        .filter(|p| p.id != platform_id)
        .map(|p| SyncTarget {
            id: p.id.clone(),
            display_name: p.display_name.clone(),
            has_skill: p
                .skills
                .iter()
                .any(|sk| sk.name == skill_name && sk.folder == folder),
        })
        .collect()
}

#[tauri::command]
pub fn sync_skill_cmd(
    state: tauri::State<'_, SafeState>,
    source_platform_id: String,
    target_platform_id: String,
    skill_name: String,
    folder: String,
    overwrite: bool,
) -> Result<String, CommandError> {
    let (source_skill, target_platform) = {
        let mut s = state.lock().unwrap();
        if let Some(p) = s.platforms.iter_mut().find(|p| p.id == source_platform_id) {
            crate::platform::load_platform_skills(p);
        }
        let source_platform = s
            .platforms
            .iter()
            .find(|p| p.id == source_platform_id)
            .ok_or_else(|| CommandError::NotFound("Source platform not found".into()))?;
        let target = s
            .platforms
            .iter()
            .find(|p| p.id == target_platform_id)
            .ok_or_else(|| CommandError::NotFound("Target platform not found".into()))?;
        let skill = find_skill(source_platform, &skill_name, &folder)
            .cloned()
            .ok_or_else(|| CommandError::NotFound("Source skill not found".into()))?;
        (skill, target.clone())
    };

    let result = if overwrite {
        crate::sync::sync_overwrite(&source_skill, &target_platform)
    } else {
        crate::sync::sync_skill(&source_skill, &target_platform)
    };

    match result {
        Ok(()) => {
            let mut s = state.lock().unwrap();
            if let Some(p) = s.platforms.iter_mut().find(|p| p.id == target_platform_id) {
                crate::platform::invalidate_platform_skills(p);
            }
            Ok("ok".to_string())
        }
        Err(e) => Err(CommandError::SyncError(e.to_string())),
    }
}

#[tauri::command]
pub fn sync_folder_cmd(
    state: tauri::State<'_, SafeState>,
    source_platform_id: String,
    target_platform_id: String,
    folder: String,
) -> Result<serde_json::Value, CommandError> {
    let results = {
        let mut s = state.lock().unwrap();
        if let Some(p) = s.platforms.iter_mut().find(|p| p.id == source_platform_id) {
            crate::platform::load_platform_skills(p);
        }
        let source_platform = s
            .platforms
            .iter()
            .find(|p| p.id == source_platform_id)
            .ok_or_else(|| CommandError::NotFound("Source platform not found".into()))?;
        let target_platform = s
            .platforms
            .iter()
            .find(|p| p.id == target_platform_id)
            .ok_or_else(|| CommandError::NotFound("Target platform not found".into()))?;

        let skills: Vec<_> = source_platform
            .skills
            .iter()
            .filter(|sk| sk.folder == folder)
            .cloned()
            .collect();

        let mut synced = 0;
        let mut errors = Vec::new();
        for skill in &skills {
            match crate::sync::sync_overwrite(skill, target_platform) {
                Ok(()) => synced += 1,
                Err(e) => errors.push(format!("{}: {}", skill.name, e)),
            }
        }
        serde_json::json!({ "synced": synced, "errors": errors, "total": skills.len() })
    };

    let mut s = state.lock().unwrap();
    if let Some(p) = s.platforms.iter_mut().find(|p| p.id == target_platform_id) {
        crate::platform::invalidate_platform_skills(p);
    }
    Ok(results)
}

#[tauri::command]
pub fn refresh_platforms(state: tauri::State<'_, SafeState>) -> Vec<PlatformView> {
    let mut s = state.lock().unwrap();
    s.platforms = crate::platform::discover_platforms(&s.config);
    s.platforms.iter().map(PlatformView::from).collect()
}

#[tauri::command]
pub fn get_locale(state: tauri::State<'_, SafeState>) -> String {
    let s = state.lock().unwrap();
    s.locale.tag().to_string()
}

#[tauri::command]
pub fn set_locale(
    app: tauri::AppHandle,
    state: tauri::State<'_, SafeState>,
    locale: String,
) -> String {
    let tag = {
        let mut s = state.lock().unwrap();
        s.locale = match locale.as_str() {
            "zh-CN" | "zh" => crate::i18n::Locale::ZhCn,
            _ => crate::i18n::Locale::En,
        };
        s.config.general.language = s.locale.tag().to_string();
        let _ = s.config.save();
        s.locale.tag().to_string()
    };
    // Keep the native tray right-click menu in sync with the UI language.
    crate::tray::apply_locale(&app, &tag);
    tag
}

#[tauri::command]
pub fn search_skills(
    state: tauri::State<'_, SafeState>,
    query: String,
    workspace_dir: Option<String>,
) -> Vec<SearchResult> {
    let q = query.to_lowercase();
    let mut s = state.lock().unwrap();
    if workspace_dir
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        let platform_ids: Vec<String> = s
            .platforms
            .iter()
            .map(|platform| platform.id.clone())
            .collect();
        let mut results = Vec::new();
        for platform_id in platform_ids {
            let Ok(platform) = scoped_platform(&mut s, &platform_id, workspace_dir.as_deref())
            else {
                continue;
            };
            for skill in &platform.skills {
                if skill.name.to_lowercase().contains(&q)
                    || skill.description.to_lowercase().contains(&q)
                {
                    results.push(SearchResult {
                        platform_id: platform.id.clone(),
                        platform_name: platform.display_name.clone(),
                        skill_name: skill.name.clone(),
                        folder: skill.folder.clone(),
                        description: skill.description.clone(),
                    });
                }
            }
        }
        return results;
    }
    crate::platform::ensure_all_skills_loaded(&mut s.platforms);
    let mut results = Vec::new();
    for platform in &s.platforms {
        for skill in &platform.skills {
            if skill.name.to_lowercase().contains(&q)
                || skill.description.to_lowercase().contains(&q)
            {
                results.push(SearchResult {
                    platform_id: platform.id.clone(),
                    platform_name: platform.display_name.clone(),
                    skill_name: skill.name.clone(),
                    folder: skill.folder.clone(),
                    description: skill.description.clone(),
                });
            }
        }
    }
    results
}

#[tauri::command]
pub fn delete_skill_cmd(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    skill_name: String,
    folder: String,
) -> Result<String, CommandError> {
    let skill_path = {
        let mut s = state.lock().unwrap();
        let platform = s
            .platforms
            .iter_mut()
            .find(|p| p.id == platform_id)
            .ok_or_else(|| CommandError::NotFound("Platform not found".into()))?;
        crate::platform::load_platform_skills(platform);
        let skill = find_skill(platform, &skill_name, &folder)
            .ok_or_else(|| CommandError::NotFound(format!("Skill {} not found", skill_name)))?;
        skill.path.clone()
    };
    crate::trash::move_skill_to_trash(&platform_id, &skill_name, &folder, &skill_path)
        .map_err(|e| CommandError::SyncError(e))?;
    let mut s = state.lock().unwrap();
    if let Some(p) = s.platforms.iter_mut().find(|p| p.id == platform_id) {
        crate::platform::invalidate_platform_skills(p);
    }
    Ok("ok".to_string())
}

#[tauri::command]
pub fn read_skill_file(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    skill_name: String,
    folder: String,
    file_path: String,
    workspace_dir: Option<String>,
) -> Result<String, CommandError> {
    let mut s = state.lock().unwrap();
    let platform = scoped_platform(&mut s, &platform_id, workspace_dir.as_deref())?;
    let skill = find_skill(&platform, &skill_name, &folder)
        .ok_or_else(|| CommandError::NotFound(format!("Skill {} not found", skill_name)))?;
    let full_path = skill.path.join(&file_path);
    if !full_path.exists() {
        return Err(CommandError::NotFound(format!(
            "File {} not found",
            file_path
        )));
    }
    let canonical_skill = std::fs::canonicalize(&skill.path).unwrap_or_else(|_| skill.path.clone());
    let canonical_file = std::fs::canonicalize(&full_path).unwrap_or_else(|_| full_path.clone());
    if !canonical_file.starts_with(&canonical_skill) {
        return Err(CommandError::NotFound("Path traversal not allowed".into()));
    }
    std::fs::read_to_string(&full_path).map_err(|e| CommandError::NotFound(e.to_string()))
}

// --- Session Commands ---

#[tauri::command(async)]
pub fn list_session_platforms() -> Result<Vec<session::SessionPlatform>, CommandError> {
    session::list_session_platforms().map_err(CommandError::SyncError)
}

#[tauri::command(async)]
pub fn list_sessions(
    platform_id: String,
    path_filter: String,
    offset: u32,
    limit: u32,
) -> Result<session::SessionListPage, CommandError> {
    session::list_sessions(&platform_id, &path_filter, offset as usize, limit as usize)
        .map_err(CommandError::SyncError)
}

#[tauri::command(async)]
pub fn list_session_terminals() -> Vec<session::SessionTerminalOption> {
    session::list_session_terminals()
}

#[tauri::command(async)]
pub fn resume_session(
    platform_id: String,
    session_id: String,
    project_path: String,
    terminal_id: String,
) -> Result<String, CommandError> {
    session::resume_session(&platform_id, &session_id, &project_path, &terminal_id)
        .map_err(CommandError::SyncError)
}

#[tauri::command(async)]
pub fn get_session_resume_preview(
    platform_id: String,
    session_id: String,
    project_path: String,
) -> Result<session::SessionResumePreview, CommandError> {
    session::get_session_resume_preview(&platform_id, &session_id, &project_path)
        .map_err(CommandError::SyncError)
}

#[tauri::command(async)]
pub fn get_session_messages(
    platform_id: String,
    session_id: String,
    offset: u32,
    limit: u32,
) -> Result<Vec<session::SessionMessage>, CommandError> {
    session::get_session_messages(&platform_id, &session_id, offset as usize, limit as usize)
        .map_err(CommandError::SyncError)
}

#[tauri::command(async)]
pub fn search_session_messages(
    platform_id: String,
    query: String,
) -> Result<Vec<session::SessionSearchResult>, CommandError> {
    session::search_session_messages(&platform_id, &query).map_err(CommandError::SyncError)
}

#[tauri::command(async)]
pub fn delete_session(platform_id: String, session_id: String) -> Result<String, CommandError> {
    session::delete_session(&platform_id, &session_id).map_err(CommandError::SyncError)?;
    Ok("ok".to_string())
}

#[tauri::command(async)]
pub fn delete_sessions(
    platform_id: String,
    session_ids: Vec<String>,
) -> Result<session::BatchDeleteResult, CommandError> {
    Ok(session::delete_sessions(&platform_id, &session_ids))
}

#[tauri::command(async)]
pub fn export_sessions_html(
    platform_id: String,
    session_ids: Vec<String>,
    output_path: String,
    locale: String,
) -> Result<session::SessionExportResult, CommandError> {
    session::export_sessions_html(&platform_id, &session_ids, &output_path, &locale)
        .map_err(CommandError::SyncError)
}

// --- Trash Commands ---

#[derive(Debug, Clone, serde::Serialize)]
pub struct TrashItemView {
    pub id: String,
    pub item_type: String,
    pub name: String,
    pub platform_id: String,
    pub folder: Option<String>,
    pub deleted_at: u64,
}

impl From<&crate::trash::TrashItem> for TrashItemView {
    fn from(item: &crate::trash::TrashItem) -> Self {
        Self {
            id: item.id.clone(),
            item_type: match item.item_type {
                crate::trash::TrashItemType::Skill => "skill".to_string(),
                crate::trash::TrashItemType::Mcp => "mcp".to_string(),
            },
            name: item.name.clone(),
            platform_id: item.platform_id.clone(),
            folder: item.folder.clone(),
            deleted_at: item.deleted_at,
        }
    }
}

#[tauri::command]
pub fn list_trash_cmd() -> Vec<TrashItemView> {
    crate::trash::list_trash()
        .iter()
        .map(TrashItemView::from)
        .collect()
}

#[tauri::command]
pub fn restore_trash_item_cmd(
    state: tauri::State<'_, SafeState>,
    id: String,
) -> Result<String, CommandError> {
    crate::trash::restore_item(&id).map_err(|e| CommandError::SyncError(e))?;
    let mut s = state.lock().unwrap();
    for p in s.platforms.iter_mut() {
        crate::platform::invalidate_platform_skills(p);
    }
    Ok("ok".to_string())
}

#[tauri::command]
pub fn permanently_delete_trash_item_cmd(id: String) -> Result<String, CommandError> {
    crate::trash::permanently_delete_item(&id).map_err(|e| CommandError::SyncError(e))?;
    Ok("ok".to_string())
}

#[tauri::command]
pub fn empty_trash_cmd() -> Result<String, CommandError> {
    crate::trash::empty_trash().map_err(|e| CommandError::SyncError(e))?;
    Ok("ok".to_string())
}

// --- MCP Commands ---

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpPlatformView {
    pub id: String,
    pub display_name: String,
    pub config_path: String,
    pub format: String,
    pub server_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpServerView {
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpServerDetail {
    pub name: String,
    pub config_text: String,
    pub format: String,
}

fn server_summary(config: &serde_json::Value) -> String {
    let command = config.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let args = config
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    if args.is_empty() {
        command.to_string()
    } else {
        format!("{} {}", command, args)
    }
}

#[tauri::command]
pub fn list_mcp_platforms(workspace_dir: Option<String>) -> Vec<McpPlatformView> {
    let workspace = resolve_workspace_dir(workspace_dir.as_deref())
        .ok()
        .flatten();
    crate::mcp::builtin_mcp_platforms()
        .into_iter()
        .filter(|def| workspace.is_some() || def.presence_path.exists())
        .map(|global_def| {
            let def = workspace
                .as_deref()
                .and_then(|root| crate::mcp::find_workspace_mcp_platform(&global_def.id, root))
                .unwrap_or(global_def);
            let servers = if let Some(root) = workspace.as_deref() {
                crate::mcp::read_workspace_mcp_servers(&def.id, root).unwrap_or_default()
            } else {
                crate::mcp::read_mcp_servers(&def.id).unwrap_or_default()
            };
            McpPlatformView {
                id: def.id,
                display_name: def.display_name,
                config_path: def.config_path.display().to_string(),
                format: match def.format {
                    crate::mcp::McpFormat::Json => "json",
                    crate::mcp::McpFormat::Toml => "toml",
                }
                .to_string(),
                server_count: servers.len(),
            }
        })
        .collect()
}

#[tauri::command]
pub fn get_mcp_servers(
    platform_id: String,
    workspace_dir: Option<String>,
) -> Result<Vec<McpServerView>, CommandError> {
    let workspace = resolve_workspace_dir(workspace_dir.as_deref())?;
    let servers = if let Some(root) = workspace.as_deref() {
        crate::mcp::read_workspace_mcp_servers(&platform_id, root)
    } else {
        crate::mcp::read_mcp_servers(&platform_id)
    }
    .map_err(CommandError::NotFound)?;
    Ok(servers
        .into_iter()
        .map(|s| McpServerView {
            name: s.name,
            summary: server_summary(&s.config),
        })
        .collect())
}

#[tauri::command]
pub fn get_mcp_server(
    platform_id: String,
    name: String,
    workspace_dir: Option<String>,
) -> Result<McpServerDetail, CommandError> {
    let workspace = resolve_workspace_dir(workspace_dir.as_deref())?;
    let server = if let Some(root) = workspace.as_deref() {
        crate::mcp::read_workspace_mcp_server(&platform_id, root, &name)
    } else {
        crate::mcp::read_mcp_server(&platform_id, &name)
    }
    .map_err(CommandError::NotFound)?;
    let def = workspace
        .as_deref()
        .and_then(|root| crate::mcp::find_workspace_mcp_platform(&platform_id, root))
        .or_else(|| crate::mcp::find_mcp_platform(&platform_id))
        .ok_or_else(|| CommandError::NotFound("Platform not found".into()))?;
    let config_text =
        crate::mcp::config_to_display(&server.config, def.format, &def.mcp_key, &server.name);
    Ok(McpServerDetail {
        name: server.name,
        config_text,
        format: match def.format {
            crate::mcp::McpFormat::Json => "json",
            crate::mcp::McpFormat::Toml => "toml",
        }
        .to_string(),
    })
}

#[tauri::command]
pub fn save_mcp_server_cmd(
    platform_id: String,
    name: String,
    config_json: String,
) -> Result<String, CommandError> {
    let config: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| CommandError::SyncError(format!("Invalid JSON: {}", e)))?;
    crate::mcp::save_mcp_server(&platform_id, &name, config)
        .map_err(|e| CommandError::SyncError(e))?;
    Ok("ok".to_string())
}

#[tauri::command]
pub fn delete_mcp_server_cmd(platform_id: String, name: String) -> Result<String, CommandError> {
    // Save config to trash before deleting
    if let Ok(server) = crate::mcp::read_mcp_server(&platform_id, &name) {
        let _ = crate::trash::move_mcp_to_trash(&platform_id, &name, server.config);
    }
    crate::mcp::delete_mcp_server(&platform_id, &name).map_err(|e| CommandError::SyncError(e))?;
    Ok("ok".to_string())
}

#[tauri::command]
pub fn import_mcp_server_cmd(
    platform_id: String,
    name: String,
    config_text: String,
) -> Result<String, CommandError> {
    crate::mcp::import_mcp_server(&platform_id, &name, &config_text)
        .map_err(|e| CommandError::SyncError(e))?;
    Ok("ok".to_string())
}

// --- App Info ---

#[derive(Debug, Clone, serde::Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
pub enum ResumableDownloadEvent {
    Started {
        content_length: Option<u64>,
        total: Option<u64>,
        resumed_from: u64,
        downloaded: u64,
    },
    Progress {
        chunk_length: usize,
        total: Option<u64>,
        downloaded: u64,
    },
    Finished {
        total: Option<u64>,
        downloaded: u64,
        used_resume: bool,
    },
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Signal the resumable updater to abort the current download. The download
/// loop checks this flag each iteration and returns early with a sentinel
/// error so the frontend knows the cancellation was intentional.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
pub fn cancel_update_download(
    cancel_flag: tauri::State<'_, crate::UpdateCancelFlag>,
) -> Result<(), CommandError> {
    cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command(async)]
pub async fn download_and_install_update_resumable<R: Runtime>(
    app: tauri::AppHandle<R>,
    webview: Webview<R>,
    rid: ResourceId,
    on_event: Channel<ResumableDownloadEvent>,
    use_mirror: bool,
    clear_cache: bool,
    cancel_flag: tauri::State<'_, crate::UpdateCancelFlag>,
) -> Result<(), CommandError> {
    // Reset the cancel flag before starting a new download.
    cancel_flag.store(false, std::sync::atomic::Ordering::Relaxed);

    let update = webview
        .resources_table()
        .get::<Update>(rid)
        .map_err(|err| CommandError::SyncError(err.to_string()))?;
    let update = (*update).clone();

    let pubkey = updater_pubkey(&app)?;
    let cache_dir = update_cache_dir(&app)?;
    fs::create_dir_all(&cache_dir).map_err(|err| CommandError::SyncError(err.to_string()))?;

    let download_url = update_download_url(&update.download_url, use_mirror)?;
    let cache_key = update_cache_key(&update, &download_url);
    let partial_path = cache_dir.join(format!("{}-{}.part", update.version, cache_key));
    if clear_cache {
        let _ = fs::remove_file(&partial_path);
    }
    let mut resumed_from = fs::metadata(&partial_path)
        .map(|meta| meta.len())
        .unwrap_or(0);

    let mut headers = update.headers.clone();
    if !headers.contains_key(ACCEPT) {
        headers.insert(ACCEPT, HeaderValue::from_static("application/octet-stream"));
    }

    let mut response = send_update_request(&update, &download_url, &headers, resumed_from).await?;

    if resumed_from > 0 {
        match response.status() {
            StatusCode::PARTIAL_CONTENT => {}
            StatusCode::OK | StatusCode::RANGE_NOT_SATISFIABLE => {
                resumed_from = 0;
                let _ = fs::remove_file(&partial_path);
                response = send_update_request(&update, &download_url, &headers, 0).await?;
            }
            status if !status.is_success() => {
                return Err(CommandError::SyncError(format!(
                    "Download request failed with status: {}",
                    status
                )));
            }
            _ => {}
        }
    }

    if !response.status().is_success() {
        return Err(CommandError::SyncError(format!(
            "Download request failed with status: {}",
            response.status()
        )));
    }

    let content_length = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let total = parse_total_from_content_range(response.headers().get(CONTENT_RANGE))
        .or_else(|| content_length.map(|len| len.saturating_add(resumed_from)));

    let mut file = if resumed_from > 0 && response.status() == StatusCode::PARTIAL_CONTENT {
        OpenOptions::new()
            .append(true)
            .open(&partial_path)
            .map_err(|err| CommandError::SyncError(err.to_string()))?
    } else {
        resumed_from = 0;
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&partial_path)
            .map_err(|err| CommandError::SyncError(err.to_string()))?
    };

    let mut downloaded = resumed_from;
    let _ = on_event.send(ResumableDownloadEvent::Started {
        content_length,
        total,
        resumed_from,
        downloaded,
    });

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        // Check the cancellation flag each iteration so the user can abort
        // mid-download (e.g. when switching to a China mirror).
        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            drop(file);
            let _ = fs::remove_file(&partial_path);
            return Err(CommandError::SyncError(CANCELLED_ERROR.to_string()));
        }
        let chunk = chunk.map_err(|err| CommandError::SyncError(err.to_string()))?;
        file.write_all(&chunk)
            .map_err(|err| CommandError::SyncError(err.to_string()))?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        let _ = on_event.send(ResumableDownloadEvent::Progress {
            chunk_length: chunk.len(),
            total,
            downloaded,
        });
    }
    file.flush()
        .map_err(|err| CommandError::SyncError(err.to_string()))?;

    let bytes = fs::read(&partial_path).map_err(|err| CommandError::SyncError(err.to_string()))?;
    verify_update_signature(&bytes, &update.signature, &pubkey)?;
    let _ = on_event.send(ResumableDownloadEvent::Finished {
        total,
        downloaded,
        used_resume: resumed_from > 0,
    });
    update
        .install(&bytes)
        .map_err(|err| CommandError::SyncError(err.to_string()))?;
    let _ = fs::remove_file(&partial_path);
    Ok(())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[tauri::command(async)]
pub async fn download_and_install_update_resumable(
    rid: u32,
    on_event: serde_json::Value,
    use_mirror: bool,
    clear_cache: bool,
) -> Result<(), CommandError> {
    let _ = (rid, on_event, use_mirror, clear_cache);
    Err(CommandError::SyncError(
        "Resumable updater is not available on this platform.".to_string(),
    ))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[tauri::command]
pub fn cancel_update_download() -> Result<(), CommandError> {
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn updater_pubkey<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<String, CommandError> {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(|pubkey| pubkey.as_str())
        .map(|value| value.to_string())
        .ok_or_else(|| {
            CommandError::SyncError("Updater pubkey is missing from config.".to_string())
        })
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn update_cache_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, CommandError> {
    let base = app
        .path()
        .app_cache_dir()
        .ok()
        .or_else(|| dirs::cache_dir().map(|path| path.join("agent-hub")))
        .ok_or_else(|| CommandError::SyncError("Unable to resolve cache directory.".to_string()))?;
    Ok(base.join("updater"))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn update_cache_key(update: &Update, download_url: &Url) -> String {
    update_cache_fingerprint(
        download_url.as_str(),
        &update.signature,
        &update.version,
        &update.target,
    )
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn update_cache_fingerprint(
    download_url: &str,
    signature: &str,
    version: &str,
    target: &str,
) -> String {
    let mut hasher = DefaultHasher::new();
    download_url.hash(&mut hasher);
    signature.hash(&mut hasher);
    version.hash(&mut hasher);
    target.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn build_resumable_update_request(update: &Update) -> Result<reqwest::Client, CommandError> {
    let timeout = update
        .timeout
        .map(|timeout| timeout.min(UPDATE_DOWNLOAD_TIMEOUT))
        .unwrap_or(UPDATE_DOWNLOAD_TIMEOUT);
    let mut builder = ClientBuilder::new()
        .user_agent("agent-hub-resumable-updater")
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout);
    if update.no_proxy {
        builder = builder.no_proxy();
    } else if let Some(proxy) = update.proxy.clone() {
        builder = builder.proxy(
            reqwest::Proxy::all(proxy.as_str())
                .map_err(|err| CommandError::SyncError(err.to_string()))?,
        );
    }
    builder
        .build()
        .map_err(|err| CommandError::SyncError(err.to_string()))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn update_download_url(download_url: &Url, use_mirror: bool) -> Result<Url, CommandError> {
    const GITHUB_ORIGIN: &str = "https://github.com/";
    const GITHUB_MIRROR: &str = "https://gh-proxy.com/";

    if !use_mirror {
        return Ok(download_url.clone());
    }
    if !download_url.as_str().starts_with(GITHUB_ORIGIN) {
        return Err(CommandError::SyncError(
            "The domestic mirror is only available for GitHub downloads.".to_string(),
        ));
    }
    Url::parse(&format!("{GITHUB_MIRROR}{download_url}"))
        .map_err(|err| CommandError::SyncError(err.to_string()))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn send_update_request(
    update: &Update,
    download_url: &Url,
    headers: &http::HeaderMap,
    resumed_from: u64,
) -> Result<reqwest::Response, CommandError> {
    let mut request = build_resumable_update_request(update)?
        .get(download_url.clone())
        .headers(headers.clone());
    if resumed_from > 0 {
        request = request.header(RANGE, format!("bytes={resumed_from}-"));
    }

    let response = request
        .send()
        .await
        .map_err(|err| CommandError::SyncError(format!("{download_url}: {err}")))?;
    if response.status().is_success() || response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
        Ok(response)
    } else {
        Err(CommandError::SyncError(format!(
            "{download_url} returned HTTP {}",
            response.status()
        )))
    }
}

#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
mod updater_tests {
    use super::{
        update_cache_fingerprint, update_download_url, ResumableDownloadEvent,
        UPDATE_DOWNLOAD_TIMEOUT,
    };
    use reqwest::Url;

    #[test]
    fn github_download_uses_direct_url_by_default() {
        let download_url =
            Url::parse("https://github.com/ssly/agent-hub/releases/download/v0.11.0/app.msi")
                .unwrap();

        let url = update_download_url(&download_url, false).unwrap();

        assert_eq!(url, download_url);
    }

    #[test]
    fn github_download_uses_mirror_only_when_requested() {
        let download_url =
            Url::parse("https://github.com/ssly/agent-hub/releases/download/v0.11.0/app.msi")
                .unwrap();

        let url = update_download_url(&download_url, true).unwrap();

        assert_eq!(
            url.as_str(),
            "https://gh-proxy.com/https://github.com/ssly/agent-hub/releases/download/v0.11.0/app.msi"
        );
    }

    #[test]
    fn mirror_rejects_non_github_downloads() {
        let download_url = Url::parse("https://downloads.example.com/app.msi").unwrap();

        let error = update_download_url(&download_url, true).unwrap_err();

        assert!(matches!(
            error,
            super::CommandError::SyncError(message)
                if message.contains("only available for GitHub")
        ));
    }

    #[test]
    fn direct_and_mirror_downloads_use_different_cache_keys() {
        let direct = "https://github.com/ssly/agent-hub/releases/download/v0.11.0/app.msi";
        let mirror =
            "https://gh-proxy.com/https://github.com/ssly/agent-hub/releases/download/v0.11.0/app.msi";

        let direct_key = update_cache_fingerprint(direct, "sig", "0.11.0", "darwin-aarch64");
        let mirror_key = update_cache_fingerprint(mirror, "sig", "0.11.0", "darwin-aarch64");

        assert_ne!(direct_key, mirror_key);
    }

    #[test]
    fn progress_events_use_lowercase_names_and_camel_case_fields() {
        let event = ResumableDownloadEvent::Progress {
            chunk_length: 128,
            total: Some(1024),
            downloaded: 512,
        };

        let value = serde_json::to_value(event).unwrap();

        assert_eq!(value["event"], "progress");
        assert_eq!(value["data"]["chunkLength"], 128);
        assert_eq!(value["data"]["downloaded"], 512);
    }

    #[test]
    fn update_download_timeout_is_five_minutes() {
        assert_eq!(UPDATE_DOWNLOAD_TIMEOUT.as_secs(), 5 * 60);
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn parse_total_from_content_range(value: Option<&http::HeaderValue>) -> Option<u64> {
    let value = value?.to_str().ok()?;
    let slash = value.rsplit('/').next()?;
    slash.parse::<u64>().ok()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn verify_update_signature(
    data: &[u8],
    release_signature: &str,
    pub_key: &str,
) -> Result<(), CommandError> {
    let pub_key_decoded = base64_to_string(pub_key)?;
    let public_key = PublicKey::decode(&pub_key_decoded)
        .map_err(|err| CommandError::SyncError(err.to_string()))?;
    let signature_decoded = base64_to_string(release_signature)?;
    let signature = Signature::decode(&signature_decoded)
        .map_err(|err| CommandError::SyncError(err.to_string()))?;
    public_key
        .verify(data, &signature, true)
        .map_err(|err| CommandError::SyncError(err.to_string()))?;
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn base64_to_string(value: &str) -> Result<String, CommandError> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|err| CommandError::SyncError(err.to_string()))?;
    std::str::from_utf8(&decoded)
        .map(|s| s.to_string())
        .map_err(|err| CommandError::SyncError(err.to_string()))
}

// --- MCP Sync Commands ---

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpSyncTarget {
    pub id: String,
    pub display_name: String,
    pub has_server: bool,
    pub format: String,
}

#[tauri::command]
pub fn get_mcp_sync_targets(platform_id: String, server_name: String) -> Vec<McpSyncTarget> {
    crate::mcp::builtin_mcp_platforms()
        .into_iter()
        // Only JSON platforms can be sync targets. TOML platforms (Codex) use a
        // different config structure and don't support cross-format MCP sync.
        .filter(|def| matches!(def.format, crate::mcp::McpFormat::Json))
        .filter(|def| def.presence_path.exists())
        .filter(|def| def.id != platform_id)
        .map(|def| {
            let has_server = crate::mcp::read_mcp_server(&def.id, &server_name).is_ok();
            McpSyncTarget {
                id: def.id,
                display_name: def.display_name,
                has_server,
                format: match def.format {
                    crate::mcp::McpFormat::Json => "json",
                    crate::mcp::McpFormat::Toml => "toml",
                }
                .to_string(),
            }
        })
        .collect()
}

#[tauri::command]
pub fn preview_mcp_change_cmd(
    platform_id: String,
    server_name: String,
    config_text: Option<String>,
) -> Result<crate::mcp::McpSyncPreview, CommandError> {
    if let Some(text) = config_text {
        // Add/import preview
        let def = crate::mcp::find_mcp_platform(&platform_id)
            .ok_or_else(|| CommandError::NotFound("Platform not found".into()))?;
        let config = crate::mcp::parse_server_config_input_with_format(
            &text,
            &def.mcp_key,
            &server_name,
            def.format,
        )
        .map_err(|e| CommandError::SyncError(e))?;
        crate::mcp::preview_import_mcp_server(&platform_id, &server_name, &config)
            .map_err(|e| CommandError::SyncError(e))
    } else {
        // Delete preview
        crate::mcp::preview_delete_mcp_server(&platform_id, &server_name)
            .map_err(|e| CommandError::SyncError(e))
    }
}

#[tauri::command]
pub fn preview_mcp_sync_cmd(
    source_platform_id: String,
    target_platform_id: String,
    server_name: String,
) -> Result<crate::mcp::McpSyncPreview, CommandError> {
    crate::mcp::preview_mcp_sync(&source_platform_id, &target_platform_id, &server_name)
        .map_err(|e| CommandError::SyncError(e))
}

#[tauri::command]
pub fn sync_mcp_server_cmd(
    source_platform_id: String,
    target_platform_id: String,
    server_name: String,
) -> Result<String, CommandError> {
    let server = crate::mcp::read_mcp_server(&source_platform_id, &server_name)
        .map_err(|e| CommandError::NotFound(e))?;
    let written = crate::mcp::sync_mcp_server(&server.config, &target_platform_id, &server_name)
        .map_err(|e| CommandError::SyncError(e))?;
    Ok(if written { "ok" } else { "no-op" }.to_string())
}

// --- Monitor Commands ---

#[allow(dead_code)]
pub type MonitorStateHandle = std::sync::Arc<crate::monitor::service::MonitorService<tauri::Wry>>;

#[tauri::command]
#[allow(dead_code)]
pub fn get_active_sessions(
    monitor: tauri::State<'_, MonitorStateHandle>,
) -> Vec<crate::monitor::types::AgentSession> {
    monitor.ensure_scanned();
    monitor.get_sessions()
}

#[tauri::command]
#[allow(dead_code)]
pub fn get_monitor_config(
    monitor: tauri::State<'_, MonitorStateHandle>,
) -> crate::monitor::types::MonitorConfig {
    monitor.get_config()
}

#[tauri::command]
#[allow(dead_code)]
pub fn set_monitor_config(
    monitor: tauri::State<'_, MonitorStateHandle>,
    state: tauri::State<'_, SafeState>,
    notification_enabled: Option<bool>,
    notification_cooldown_secs: Option<u64>,
) -> Result<crate::monitor::types::MonitorConfig, CommandError> {
    let mut config = monitor.get_config();
    if let Some(v) = notification_enabled {
        config.notification_enabled = v;
    }
    if let Some(v) = notification_cooldown_secs {
        config.notification_cooldown_secs = v;
    }
    monitor.set_config(config.clone());

    // Persist to config.toml
    let mut app_state = state.lock().unwrap();
    app_state.config.monitor = config.clone();
    app_state
        .config
        .save()
        .map_err(|e| CommandError::General(e))?;
    Ok(config)
}

#[tauri::command]
#[allow(dead_code)]
pub fn set_monitor_polling(monitor: tauri::State<'_, MonitorStateHandle>, enabled: bool) {
    monitor
        .polling_enabled
        .store(enabled, std::sync::atomic::Ordering::Relaxed);
}

/// Force an immediate re-scan and emit. The 5s polling thread runs on its own
/// cadence and the FS watcher only fires on file events; this command lets the
/// UI request a fresh snapshot when the user opens the Monitor tab or hits a
/// refresh button, without waiting for the next poll tick.
#[tauri::command]
#[allow(dead_code)]
pub fn force_poll_monitor(monitor: tauri::State<'_, MonitorStateHandle>) {
    monitor.poll();
}

#[tauri::command]
#[allow(dead_code)]
pub fn configure_hooks(
    monitor: tauri::State<'_, MonitorStateHandle>,
    agent_type: String,
) -> Result<(), CommandError> {
    monitor
        .configure_hooks(&agent_type)
        .map_err(CommandError::General)
}

#[tauri::command]
#[allow(dead_code)]
pub fn remove_hooks(
    monitor: tauri::State<'_, MonitorStateHandle>,
    agent_type: String,
) -> Result<(), CommandError> {
    monitor
        .remove_hooks(&agent_type)
        .map_err(CommandError::General)
}

#[tauri::command]
#[allow(dead_code)]
pub fn get_hooks_status(
    monitor: tauri::State<'_, MonitorStateHandle>,
) -> std::collections::HashMap<String, bool> {
    monitor.hooks_status()
}

// --- Session Monitor Commands ---

fn parse_hook_action(action: &str) -> Result<HookAction, CommandError> {
    HookAction::parse(action).map_err(CommandError::General)
}

#[tauri::command]
pub fn get_codex_session_monitor_snapshot(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
) -> MonitorSnapshot {
    monitor.snapshot(AgentKind::Codex)
}

#[tauri::command]
pub fn delete_codex_session_monitor_session(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
    session_id: String,
) -> Result<(), CommandError> {
    monitor
        .remove_session(AgentKind::Codex, &session_id)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_codex_hook_status() -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::get_hook_status(AgentKind::Codex).map_err(CommandError::General)
}

#[tauri::command]
pub fn preview_codex_hook_change(
    action: String,
) -> Result<crate::session_monitor::HookChangePreview, CommandError> {
    crate::session_monitor::preview_hook_change(AgentKind::Codex, parse_hook_action(&action)?)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn apply_codex_hook_change(
    action: String,
    expected_before_hash: String,
) -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::apply_hook_change(
        AgentKind::Codex,
        parse_hook_action(&action)?,
        &expected_before_hash,
    )
    .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_claude_session_monitor_snapshot(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
) -> MonitorSnapshot {
    monitor.snapshot(AgentKind::Claude)
}

#[tauri::command]
pub fn delete_claude_session_monitor_session(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
    session_id: String,
) -> Result<(), CommandError> {
    monitor
        .remove_session(AgentKind::Claude, &session_id)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_claude_hook_status() -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::get_hook_status(AgentKind::Claude).map_err(CommandError::General)
}

#[tauri::command]
pub fn preview_claude_hook_change(
    action: String,
) -> Result<crate::session_monitor::HookChangePreview, CommandError> {
    crate::session_monitor::preview_hook_change(AgentKind::Claude, parse_hook_action(&action)?)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn apply_claude_hook_change(
    action: String,
    expected_before_hash: String,
) -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::apply_hook_change(
        AgentKind::Claude,
        parse_hook_action(&action)?,
        &expected_before_hash,
    )
    .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_cursor_session_monitor_snapshot(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
) -> MonitorSnapshot {
    monitor.snapshot(AgentKind::Cursor)
}

#[tauri::command]
pub fn delete_cursor_session_monitor_session(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
    session_id: String,
) -> Result<(), CommandError> {
    monitor
        .remove_session(AgentKind::Cursor, &session_id)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_cursor_hook_status() -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::get_hook_status(AgentKind::Cursor).map_err(CommandError::General)
}

#[tauri::command]
pub fn preview_cursor_hook_change(
    action: String,
) -> Result<crate::session_monitor::HookChangePreview, CommandError> {
    crate::session_monitor::preview_hook_change(AgentKind::Cursor, parse_hook_action(&action)?)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn apply_cursor_hook_change(
    action: String,
    expected_before_hash: String,
) -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::apply_hook_change(
        AgentKind::Cursor,
        parse_hook_action(&action)?,
        &expected_before_hash,
    )
    .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_grok_session_monitor_snapshot(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
) -> MonitorSnapshot {
    monitor.snapshot(AgentKind::Grok)
}

#[tauri::command]
pub fn delete_grok_session_monitor_session(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
    session_id: String,
) -> Result<(), CommandError> {
    monitor
        .remove_session(AgentKind::Grok, &session_id)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_grok_hook_status() -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::get_hook_status(AgentKind::Grok).map_err(CommandError::General)
}

#[tauri::command]
pub fn preview_grok_hook_change(
    action: String,
) -> Result<crate::session_monitor::HookChangePreview, CommandError> {
    crate::session_monitor::preview_hook_change(AgentKind::Grok, parse_hook_action(&action)?)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn apply_grok_hook_change(
    action: String,
    expected_before_hash: String,
) -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::apply_hook_change(
        AgentKind::Grok,
        parse_hook_action(&action)?,
        &expected_before_hash,
    )
    .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_kimi_session_monitor_snapshot(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
) -> MonitorSnapshot {
    monitor.snapshot(AgentKind::Kimi)
}

#[tauri::command]
pub fn delete_kimi_session_monitor_session(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
    session_id: String,
) -> Result<(), CommandError> {
    monitor
        .remove_session(AgentKind::Kimi, &session_id)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_kimi_hook_status() -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::get_hook_status(AgentKind::Kimi).map_err(CommandError::General)
}

#[tauri::command]
pub fn preview_kimi_hook_change(
    action: String,
) -> Result<crate::session_monitor::HookChangePreview, CommandError> {
    crate::session_monitor::preview_hook_change(AgentKind::Kimi, parse_hook_action(&action)?)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn apply_kimi_hook_change(
    action: String,
    expected_before_hash: String,
) -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::apply_hook_change(
        AgentKind::Kimi,
        parse_hook_action(&action)?,
        &expected_before_hash,
    )
    .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_zcode_session_monitor_snapshot(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
) -> MonitorSnapshot {
    monitor.snapshot(AgentKind::Zcode)
}

#[tauri::command]
pub fn delete_zcode_session_monitor_session(
    monitor: tauri::State<'_, crate::session_monitor::ServiceHandle>,
    session_id: String,
) -> Result<(), CommandError> {
    monitor
        .remove_session(AgentKind::Zcode, &session_id)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn get_zcode_hook_status() -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::get_hook_status(AgentKind::Zcode).map_err(CommandError::General)
}

#[tauri::command]
pub fn preview_zcode_hook_change(
    action: String,
) -> Result<crate::session_monitor::HookChangePreview, CommandError> {
    crate::session_monitor::preview_hook_change(AgentKind::Zcode, parse_hook_action(&action)?)
        .map_err(CommandError::General)
}

#[tauri::command]
pub fn apply_zcode_hook_change(
    action: String,
    expected_before_hash: String,
) -> Result<crate::session_monitor::HookStatus, CommandError> {
    crate::session_monitor::apply_hook_change(
        AgentKind::Zcode,
        parse_hook_action(&action)?,
        &expected_before_hash,
    )
    .map_err(CommandError::General)
}

/// Zcode 插件市场只读列表。目录不存在（未安装 Zcode）时返回空列表，不报错。
#[tauri::command]
pub fn get_zcode_plugins() -> Vec<crate::zcode_plugin::ZcodePluginView> {
    crate::zcode_plugin::list_zcode_plugins()
}

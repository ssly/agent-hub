use crate::platform::Platform;
use crate::session;
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
use reqwest::{ClientBuilder, StatusCode};
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::{ipc::Channel, Manager, ResourceId, Runtime, Webview};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri_plugin_updater::Update;

// --- View types ---

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlatformView {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub skill_dir: String,
    pub skill_count: usize,
}

impl From<&Platform> for PlatformView {
    fn from(p: &Platform) -> Self {
        Self {
            id: p.id.clone(),
            display_name: p.display_name.clone(),
            description: p.description.clone(),
            skill_dir: p.skill_dir.display().to_string(),
            skill_count: p.skills.len(),
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
) -> Vec<SkillSummary> {
    let s = state.lock().unwrap();
    s.platforms
        .iter()
        .find(|p| p.id == platform_id)
        .map(|p| p.skills.iter().map(SkillSummary::from).collect())
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_skill_detail(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    skill_name: String,
    folder: String,
) -> Result<SkillDetail, CommandError> {
    let s = state.lock().unwrap();
    let platform = s
        .platforms
        .iter()
        .find(|p| p.id == platform_id)
        .ok_or_else(|| CommandError::NotFound(format!("Platform {} not found", platform_id)))?;
    let skill = find_skill(platform, &skill_name, &folder)
        .ok_or_else(|| CommandError::NotFound(format!("Skill {} not found", skill_name)))?;
    Ok(SkillDetail::from(skill))
}

#[tauri::command]
pub fn get_diff_candidates(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    skill_name: String,
    folder: String,
) -> Vec<PlatformView> {
    let s = state.lock().unwrap();
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
    let s = state.lock().unwrap();
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
    let s = state.lock().unwrap();
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
        let s = state.lock().unwrap();
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
            s.platforms = crate::platform::discover_platforms(&s.config);
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
        let s = state.lock().unwrap();
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
    s.platforms = crate::platform::discover_platforms(&s.config);
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
pub fn set_locale(state: tauri::State<'_, SafeState>, locale: String) -> String {
    let mut s = state.lock().unwrap();
    s.locale = match locale.as_str() {
        "zh-CN" | "zh" => crate::i18n::Locale::ZhCn,
        _ => crate::i18n::Locale::En,
    };
    s.config.general.language = s.locale.tag().to_string();
    let _ = s.config.save();
    s.locale.tag().to_string()
}

#[tauri::command]
pub fn search_skills(state: tauri::State<'_, SafeState>, query: String) -> Vec<SearchResult> {
    let q = query.to_lowercase();
    let s = state.lock().unwrap();
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
        let s = state.lock().unwrap();
        let platform = s
            .platforms
            .iter()
            .find(|p| p.id == platform_id)
            .ok_or_else(|| CommandError::NotFound("Platform not found".into()))?;
        let skill = find_skill(platform, &skill_name, &folder)
            .ok_or_else(|| CommandError::NotFound(format!("Skill {} not found", skill_name)))?;
        skill.path.clone()
    };
    crate::trash::move_skill_to_trash(&platform_id, &skill_name, &folder, &skill_path)
        .map_err(|e| CommandError::SyncError(e))?;
    let mut s = state.lock().unwrap();
    s.platforms = crate::platform::discover_platforms(&s.config);
    Ok("ok".to_string())
}

#[tauri::command]
pub fn read_skill_file(
    state: tauri::State<'_, SafeState>,
    platform_id: String,
    skill_name: String,
    folder: String,
    file_path: String,
) -> Result<String, CommandError> {
    let s = state.lock().unwrap();
    let platform = s
        .platforms
        .iter()
        .find(|p| p.id == platform_id)
        .ok_or_else(|| CommandError::NotFound(format!("Platform {} not found", platform_id)))?;
    let skill = find_skill(platform, &skill_name, &folder)
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
pub fn delete_session(platform_id: String, session_id: String) -> Result<String, CommandError> {
    session::delete_session(&platform_id, &session_id).map_err(CommandError::SyncError)?;
    Ok("ok".to_string())
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct InvalidSkillView {
    pub path: String,
    pub platform_id: String,
    pub platform_name: String,
    pub reason: String,
}

impl From<&crate::skill::InvalidSkill> for InvalidSkillView {
    fn from(s: &crate::skill::InvalidSkill) -> Self {
        Self {
            path: s.path.clone(),
            platform_id: s.platform_id.clone(),
            platform_name: s.platform_name.clone(),
            reason: s.reason.clone(),
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
pub fn scan_invalid_skills_cmd(state: tauri::State<'_, SafeState>) -> Vec<InvalidSkillView> {
    let s = state.lock().unwrap();
    let mut invalid: Vec<InvalidSkillView> = Vec::new();
    for platform in &s.platforms {
        let items = crate::skill::scan_invalid_skills(
            &platform.skill_dir,
            &platform.id,
            &platform.display_name,
        );
        invalid.extend(items.iter().map(InvalidSkillView::from));
    }
    invalid
}

#[tauri::command]
pub fn restore_trash_item_cmd(
    state: tauri::State<'_, SafeState>,
    id: String,
) -> Result<String, CommandError> {
    crate::trash::restore_item(&id).map_err(|e| CommandError::SyncError(e))?;
    let mut s = state.lock().unwrap();
    s.platforms = crate::platform::discover_platforms(&s.config);
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
pub fn list_mcp_platforms() -> Vec<McpPlatformView> {
    crate::mcp::builtin_mcp_platforms()
        .into_iter()
        .filter(|def| def.presence_path.exists())
        .map(|def| {
            let servers = crate::mcp::read_mcp_servers(&def.id).unwrap_or_default();
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
pub fn get_mcp_servers(platform_id: String) -> Result<Vec<McpServerView>, CommandError> {
    let servers =
        crate::mcp::read_mcp_servers(&platform_id).map_err(|e| CommandError::NotFound(e))?;
    Ok(servers
        .into_iter()
        .map(|s| McpServerView {
            name: s.name,
            summary: server_summary(&s.config),
        })
        .collect())
}

#[tauri::command]
pub fn get_mcp_server(platform_id: String, name: String) -> Result<McpServerDetail, CommandError> {
    let server =
        crate::mcp::read_mcp_server(&platform_id, &name).map_err(|e| CommandError::NotFound(e))?;
    let def = crate::mcp::find_mcp_platform(&platform_id)
        .ok_or_else(|| CommandError::NotFound("Platform not found".into()))?;
    let config_text = crate::mcp::config_to_display(&server.config, def.format);
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

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command(async)]
pub async fn download_and_install_update_resumable<R: Runtime>(
    app: tauri::AppHandle<R>,
    webview: Webview<R>,
    rid: ResourceId,
    on_event: Channel<ResumableDownloadEvent>,
) -> Result<(), CommandError> {
    let update = webview
        .resources_table()
        .get::<Update>(rid)
        .map_err(|err| CommandError::SyncError(err.to_string()))?;
    let update = (*update).clone();

    let pubkey = updater_pubkey(&app)?;
    let cache_dir = update_cache_dir(&app)?;
    fs::create_dir_all(&cache_dir).map_err(|err| CommandError::SyncError(err.to_string()))?;

    let cache_key = update_cache_key(&update);
    let partial_path = cache_dir.join(format!("{}-{}.part", update.version, cache_key));
    let mut resumed_from = fs::metadata(&partial_path)
        .map(|meta| meta.len())
        .unwrap_or(0);

    let mut headers = update.headers.clone();
    if !headers.contains_key(ACCEPT) {
        headers.insert(ACCEPT, HeaderValue::from_static("application/octet-stream"));
    }

    let mut request = build_resumable_update_request(&update)?
        .get(update.download_url.clone())
        .headers(headers.clone());

    if resumed_from > 0 {
        request = request.header(RANGE, format!("bytes={}-", resumed_from));
    }

    let mut response = request
        .send()
        .await
        .map_err(|err| CommandError::SyncError(err.to_string()))?;

    if resumed_from > 0 {
        match response.status() {
            StatusCode::PARTIAL_CONTENT => {}
            StatusCode::OK | StatusCode::RANGE_NOT_SATISFIABLE => {
                resumed_from = 0;
                let _ = fs::remove_file(&partial_path);
                response = build_resumable_update_request(&update)?
                    .get(update.download_url.clone())
                    .headers(headers.clone())
                    .send()
                    .await
                    .map_err(|err| CommandError::SyncError(err.to_string()))?;
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
) -> Result<(), CommandError> {
    let _ = (rid, on_event);
    Err(CommandError::SyncError(
        "Resumable updater is not available on this platform.".to_string(),
    ))
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
fn update_cache_key(update: &Update) -> String {
    let mut hasher = DefaultHasher::new();
    update.download_url.as_str().hash(&mut hasher);
    update.signature.hash(&mut hasher);
    update.version.hash(&mut hasher);
    update.target.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn build_resumable_update_request(update: &Update) -> Result<reqwest::Client, CommandError> {
    let mut builder = ClientBuilder::new().user_agent("agent-hub-resumable-updater");
    if let Some(timeout) = update.timeout {
        builder = builder.timeout(timeout);
    }
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

pub type MonitorStateHandle = std::sync::Arc<crate::monitor::service::MonitorService<tauri::Wry>>;

#[tauri::command]
pub fn get_active_sessions(
    monitor: tauri::State<'_, MonitorStateHandle>,
) -> Vec<crate::monitor::types::AgentSession> {
    monitor.ensure_scanned();
    monitor.get_sessions()
}

#[tauri::command]
pub fn get_monitor_config(
    monitor: tauri::State<'_, MonitorStateHandle>,
) -> crate::monitor::types::MonitorConfig {
    monitor.get_config()
}

#[tauri::command]
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
    app_state.config.save().map_err(|e| CommandError::General(e))?;
    Ok(config)
}

#[tauri::command]
pub fn set_monitor_polling(
    monitor: tauri::State<'_, MonitorStateHandle>,
    enabled: bool,
) {
    monitor.polling_enabled
        .store(enabled, std::sync::atomic::Ordering::Relaxed);
}

/// Force an immediate re-scan and emit. The 5s polling thread runs on its own
/// cadence and the FS watcher only fires on file events; this command lets the
/// UI request a fresh snapshot when the user opens the Monitor tab or hits a
/// refresh button, without waiting for the next poll tick.
#[tauri::command]
pub fn force_poll_monitor(monitor: tauri::State<'_, MonitorStateHandle>) {
    monitor.poll();
}

#[tauri::command]
pub fn configure_hooks(
    monitor: tauri::State<'_, MonitorStateHandle>,
    agent_type: String,
) -> Result<(), CommandError> {
    monitor.configure_hooks(&agent_type).map_err(CommandError::General)
}

#[tauri::command]
pub fn remove_hooks(
    monitor: tauri::State<'_, MonitorStateHandle>,
    agent_type: String,
) -> Result<(), CommandError> {
    monitor.remove_hooks(&agent_type).map_err(CommandError::General)
}

#[tauri::command]
pub fn get_hooks_status(
    monitor: tauri::State<'_, MonitorStateHandle>,
) -> std::collections::HashMap<String, bool> {
    monitor.hooks_status()
}

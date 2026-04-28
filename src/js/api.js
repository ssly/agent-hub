function invoke(cmd, args) {
    return window.__TAURI_INTERNALS__.invoke(cmd, args);
}

export async function listPlatforms() {
    return invoke('list_platforms');
}

export async function getPlatformSkills(platformId) {
    return invoke('get_platform_skills', { platformId });
}

export async function getSkillDetail(platformId, skillName, folder) {
    return invoke('get_skill_detail', { platformId, skillName, folder });
}

export async function getDiffCandidates(platformId, skillName, folder) {
    return invoke('get_diff_candidates', { platformId, skillName, folder });
}

export async function diffSkills(sourcePlatformId, targetPlatformId, skillName, folder) {
    return invoke('diff_skills_cmd', { sourcePlatformId, targetPlatformId, skillName, folder });
}

export async function getSyncTargets(platformId, skillName, folder) {
    return invoke('get_sync_targets', { platformId, skillName, folder });
}

export async function syncSkill(sourcePlatformId, targetPlatformId, skillName, folder, overwrite) {
    return invoke('sync_skill_cmd', { sourcePlatformId, targetPlatformId, skillName, folder, overwrite });
}

export async function syncFolder(sourcePlatformId, targetPlatformId, folder) {
    return invoke('sync_folder_cmd', { sourcePlatformId, targetPlatformId, folder });
}

export async function refreshPlatforms() {
    return invoke('refresh_platforms');
}

export async function getLocale() {
    return invoke('get_locale');
}

export async function setLocale(locale) {
    return invoke('set_locale', { locale });
}

export async function searchSkills(query) {
    return invoke('search_skills', { query });
}

export async function readSkillFile(platformId, skillName, folder, filePath) {
    return invoke('read_skill_file', { platformId, skillName, folder, filePath });
}

export async function deleteSkill(platformId, skillName, folder) {
    return invoke('delete_skill_cmd', { platformId, skillName, folder });
}

// MCP
export async function listMcpPlatforms() {
    return invoke('list_mcp_platforms');
}

export async function getMcpServers(platformId) {
    return invoke('get_mcp_servers', { platformId });
}

export async function getMcpServer(platformId, name) {
    return invoke('get_mcp_server', { platformId, name });
}

export async function saveMcpServer(platformId, name, configJson) {
    return invoke('save_mcp_server_cmd', { platformId, name, configJson });
}

export async function deleteMcpServer(platformId, name) {
    return invoke('delete_mcp_server_cmd', { platformId, name });
}

export async function importMcpServer(platformId, name, configText) {
    return invoke('import_mcp_server_cmd', { platformId, name, configText });
}

// MCP Sync
export async function getMcpSyncTargets(platformId, serverName) {
    return invoke('get_mcp_sync_targets', { platformId, serverName });
}

export async function previewMcpSync(sourcePlatformId, targetPlatformId, serverName) {
    return invoke('preview_mcp_sync_cmd', { sourcePlatformId, targetPlatformId, serverName });
}

export async function syncMcpServer(sourcePlatformId, targetPlatformId, serverName) {
    return invoke('sync_mcp_server_cmd', { sourcePlatformId, targetPlatformId, serverName });
}

export async function getAppVersion() {
    return invoke('get_app_version');
}

// Trash
export async function listTrash() {
    return invoke('list_trash_cmd');
}

export async function restoreTrashItem(id) {
    return invoke('restore_trash_item_cmd', { id });
}

export async function permanentlyDeleteTrashItem(id) {
    return invoke('permanently_delete_trash_item_cmd', { id });
}

export async function emptyTrash() {
    return invoke('empty_trash_cmd');
}

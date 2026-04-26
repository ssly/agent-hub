function invoke(cmd, args) {
    return window.__TAURI_INTERNALS__.invoke(cmd, args);
}

export async function listPlatforms() {
    return invoke('list_platforms');
}

export async function getPlatformSkills(platformId) {
    return invoke('get_platform_skills', { platformId });
}

export async function getSkillDetail(platformId, skillName) {
    return invoke('get_skill_detail', { platformId, skillName });
}

export async function getDiffCandidates(platformId, skillName) {
    return invoke('get_diff_candidates', { platformId, skillName });
}

export async function diffSkills(sourcePlatformId, targetPlatformId, skillName) {
    return invoke('diff_skills_cmd', { sourcePlatformId, targetPlatformId, skillName });
}

export async function getSyncTargets(platformId, skillName) {
    return invoke('get_sync_targets', { platformId, skillName });
}

export async function syncSkill(sourcePlatformId, targetPlatformId, skillName, overwrite) {
    return invoke('sync_skill_cmd', { sourcePlatformId, targetPlatformId, skillName, overwrite });
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

export async function readSkillFile(platformId, skillName, filePath) {
    return invoke('read_skill_file', { platformId, skillName, filePath });
}

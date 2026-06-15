use std::path::PathBuf;

use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::model::{AuthProfile, ListSwitchResponse, ProfileMeta};

/// Resolve the storage directory for a given agent type.
fn profiles_dir(agent_type: &str) -> Result<PathBuf, String> {
    let slug = agent_slug(agent_type)?;
    let dir = dirs::home_dir()
        .ok_or("Cannot determine home directory")?
        .join(format!(".agent-hub/switch/{slug}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Resolve the live config file path for a given agent type.
fn agent_config_path(agent_type: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let path = match agent_type {
        "codex" => home.join(".codex/auth.json"),
        "claude-code" => home.join(".claude/settings.json"),
        _ => return Err(format!("unknown_agent_type:{agent_type}")),
    };
    Ok(path)
}

/// Validate & normalise the agent type, returns the directory slug.
fn agent_slug(agent_type: &str) -> Result<&'static str, String> {
    match agent_type {
        "codex" => Ok("codex"),
        "claude-code" => Ok("claude-code"),
        _ => Err(format!("unknown_agent_type:{agent_type}")),
    }
}

fn hash_file(path: &PathBuf) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(hasher.finalize().to_vec())
}

fn hash_env_field(path: &PathBuf) -> Option<Vec<u8>> {
    let content = std::fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    let env = val.get("env").cloned().unwrap_or(serde_json::Value::Null);
    let env_str = serde_json::to_string(&env).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(env_str.as_bytes());
    Some(hasher.finalize().to_vec())
}

fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn extract_key(agent_type: &str, content: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(content).ok()?;
    match agent_type {
        "codex" => val
            .get("access_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "claude-code" => val
            .get("env")
            .and_then(|env| env.get("ANTHROPIC_API_KEY"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

fn mask_key(key: &str) -> String {
    if key.len() <= 12 {
        return key.to_string();
    }
    format!("{}...{}", &key[..8], &key[key.len() - 4..])
}

fn check_duplicate_key(
    agent_type: &str,
    key: &str,
    exclude_id: Option<&str>,
) -> Result<(), String> {
    let dir = profiles_dir(agent_type)?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(exclude) = exclude_id {
            if path.file_name().map(|n| n == exclude).unwrap_or(false) {
                continue;
            }
        }
        let config_path = path.join("config.json");
        if !config_path.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Some(existing_key) = extract_key(agent_type, &content) {
            if existing_key == key {
                return Err("duplicate_key".to_string());
            }
        }
    }
    Ok(())
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_switch_profiles(agent_type: String) -> Result<ListSwitchResponse, String> {
    let dir = profiles_dir(&agent_type)?;
    let active_hash = if agent_type == "claude-code" {
        agent_config_path(&agent_type).ok().and_then(|p| hash_env_field(&p))
    } else {
        agent_config_path(&agent_type).ok().and_then(|p| hash_file(&p))
    };

    let config_path = agent_config_path(&agent_type).ok();
    let current_key = config_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|c| extract_key(&agent_type, &c))
        .map(|k| mask_key(&k));

    let mut profiles: Vec<AuthProfile> = Vec::new();

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => {
            return Ok(ListSwitchResponse {
                profiles: Vec::new(),
                current_key,
            })
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join("meta.json");
        let cfg_path = path.join("config.json");
        let Ok(meta_str) = std::fs::read_to_string(&meta_path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<ProfileMeta>(&meta_str) else {
            continue;
        };
        if !cfg_path.exists() {
            continue;
        }
        let profile_hash = if agent_type == "claude-code" {
            hash_env_field(&cfg_path)
        } else {
            hash_file(&cfg_path)
        };
        let is_active = match (&active_hash, &profile_hash) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        let profile_key = std::fs::read_to_string(&cfg_path)
            .ok()
            .and_then(|c| extract_key(&agent_type, &c))
            .map(|k| mask_key(&k));
        profiles.push(AuthProfile {
            id: meta.id,
            note: meta.note,
            saved_at: meta.saved_at,
            is_active,
            key: profile_key,
        });
    }

    profiles.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    Ok(ListSwitchResponse {
        profiles,
        current_key,
    })
}

#[tauri::command]
pub fn save_current_auth_profile(agent_type: String, note: String) -> Result<String, String> {
    let src = agent_config_path(&agent_type)?;
    if !src.exists() {
        return Err("no_active_auth".to_string());
    }
    let content_bytes = std::fs::read(&src).map_err(|e| e.to_string())?;
    let content_str = String::from_utf8_lossy(&content_bytes).to_string();
    if let Some(key) = extract_key(&agent_type, &content_str) {
        check_duplicate_key(&agent_type, &key, None)?;
    }

    let id = Uuid::new_v4().to_string();
    let dir = profiles_dir(&agent_type)?.join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    std::fs::write(dir.join("config.json"), &content_bytes).map_err(|e| e.to_string())?;

    let meta = ProfileMeta {
        id: id.clone(),
        note,
        saved_at: now_iso(),
    };
    let meta_str = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("meta.json"), meta_str).map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn add_auth_profile(agent_type: String, content: String, note: String) -> Result<String, String> {
    serde_json::from_str::<serde_json::Value>(&content).map_err(|_| "invalid_json".to_string())?;
    if let Some(key) = extract_key(&agent_type, &content) {
        check_duplicate_key(&agent_type, &key, None)?;
    }

    let id = Uuid::new_v4().to_string();
    let dir = profiles_dir(&agent_type)?.join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    std::fs::write(dir.join("config.json"), content.as_bytes()).map_err(|e| e.to_string())?;

    let meta = ProfileMeta {
        id: id.clone(),
        note,
        saved_at: now_iso(),
    };
    let meta_str = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("meta.json"), meta_str).map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn switch_auth_profile(agent_type: String, id: String) -> Result<(), String> {
    let dir = profiles_dir(&agent_type)?.join(&id);
    let src = dir.join("config.json");
    if !src.exists() {
        return Err("profile_not_found".to_string());
    }
    let content = std::fs::read(&src).map_err(|e| e.to_string())?;

    let dest = agent_config_path(&agent_type)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    if agent_type == "claude-code" {
        let profile_str = std::fs::read_to_string(&src).map_err(|e| e.to_string())?;
        let mut profile_val: serde_json::Value =
            serde_json::from_str(&profile_str).map_err(|e| e.to_string())?;
        let env_val = profile_val
            .as_object_mut()
            .and_then(|m| m.remove("env"))
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        let current_str = if dest.exists() {
            std::fs::read_to_string(&dest).map_err(|e| e.to_string())?
        } else {
            "{}".to_string()
        };
        let mut current_val: serde_json::Value =
            serde_json::from_str(&current_str).map_err(|e| e.to_string())?;

        if let Some(obj) = current_val.as_object_mut() {
            obj.insert("env".to_string(), env_val);
        }

        let merged_str = serde_json::to_string_pretty(&current_val).map_err(|e| e.to_string())?;
        let tmp = dest.with_extension("json.tmp");
        std::fs::write(&tmp, merged_str.as_bytes()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    } else {
        let tmp = dest.with_extension("json.tmp");
        std::fs::write(&tmp, &content).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn update_auth_profile_note(agent_type: String, id: String, note: String) -> Result<(), String> {
    let dir = profiles_dir(&agent_type)?.join(&id);
    let meta_path = dir.join("meta.json");
    if !meta_path.exists() {
        return Err("profile_not_found".to_string());
    }
    let meta_str = std::fs::read_to_string(&meta_path).map_err(|e| e.to_string())?;
    let mut meta: ProfileMeta = serde_json::from_str(&meta_str).map_err(|e| e.to_string())?;
    meta.note = note;
    let new_str = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    std::fs::write(&meta_path, new_str).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_auth_profile(agent_type: String, id: String) -> Result<(), String> {
    let dir = profiles_dir(&agent_type)?.join(&id);
    if !dir.exists() {
        return Err("profile_not_found".to_string());
    }
    std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_auth_profile_content(agent_type: String, id: String) -> Result<String, String> {
    let path = profiles_dir(&agent_type)?.join(&id).join("config.json");
    if !path.exists() {
        return Err("profile_not_found".to_string());
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_auth_profile_content(agent_type: String, id: String, content: String) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&content).map_err(|_| "invalid_json".to_string())?;

    let dir = profiles_dir(&agent_type)?.join(&id);
    if !dir.exists() {
        return Err("profile_not_found".to_string());
    }
    let path = dir.join("config.json");
    let tmp = dir.join("config.json.tmp");
    std::fs::write(&tmp, content.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clear_active_auth(agent_type: String) -> Result<String, String> {
    let src = agent_config_path(&agent_type)?;
    if !src.exists() {
        return Err("no_active_auth".to_string());
    }

    let note = format!("auto-backup before clear {}", now_iso());
    let backup_id = save_current_auth_profile(agent_type.clone(), note)?;

    std::fs::remove_file(&src).map_err(|e| e.to_string())?;

    Ok(backup_id)
}

// --- Codex quota via internal endpoint (for the Switch view) ---

#[derive(serde::Deserialize)]
struct CodexAuthTokens {
    access_token: String,
    account_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct CodexAuthFile {
    tokens: Option<CodexAuthTokens>,
    account_id: Option<String>,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct UsageWindow {
    pub used_percent: u8,
    pub remaining_percent: u8,
    pub reset_after_seconds: u64,
    pub reset_at: u64,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct CodexUsageResponse {
    pub plan_type: String,
    pub primary_window: UsageWindow,
    pub secondary_window: UsageWindow,
}

#[tauri::command]
pub async fn get_codex_usage() -> Result<CodexUsageResponse, String> {
    // Resolve auth file (respect CODEX_HOME if set)
    let codex_home: PathBuf = match std::env::var("CODEX_HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            let home = dirs::home_dir()
                .ok_or_else(|| "无法确定用户主目录".to_string())?;
            home.join(".codex")
        }
    };

    let auth_path = codex_home.join("auth.json");
    if !auth_path.exists() {
        return Err("未找到 Codex 认证文件（~/.codex/auth.json）。请先在终端运行 `codex login`。".to_string());
    }

    let content = std::fs::read_to_string(&auth_path).map_err(|e| format!("读取 auth.json 失败: {}", e))?;
    let auth: CodexAuthFile = serde_json::from_str(&content).map_err(|e| format!("解析 auth.json 失败: {}", e))?;

    let tokens = auth.tokens.ok_or_else(|| "auth.json 中没有 tokens 字段".to_string())?;
    let access_token = tokens.access_token;
    let account_id = tokens.account_id
        .or(auth.account_id)
        .ok_or_else(|| "缺少 account_id，无法查询用量".to_string())?;

    // Call the (undocumented) internal usage endpoint.
    // We prefer /wham/usage as it reliably returns the JSON quota windows.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let url = "https://chatgpt.com/backend-api/wham/usage";
    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("ChatGPT-Account-Id", &account_id)
        .header("Accept", "application/json")
        .header("User-Agent", "Codex CLI")
        .header("Origin", "https://chatgpt.com")
        .header("Referer", "https://chatgpt.com/")
        .send()
        .await
        .map_err(|e| format!("请求 Codex 用量接口失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Codex 用量接口返回错误: HTTP {}", resp.status()));
    }

    let raw: serde_json::Value = resp.json().await.map_err(|e| format!("解析用量响应失败: {}", e))?;

    let rate = raw.get("rate_limit").ok_or("响应缺少 rate_limit 字段")?;
    let primary = rate.get("primary_window").ok_or("缺少 primary_window")?;
    let secondary = rate.get("secondary_window").ok_or("缺少 secondary_window")?;

    let p_used = primary.get("used_percent").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let s_used = secondary.get("used_percent").and_then(|v| v.as_u64()).unwrap_or(0) as u8;

    let p_after = primary.get("reset_after_seconds").and_then(|v| v.as_u64()).unwrap_or(0);
    let s_after = secondary.get("reset_after_seconds").and_then(|v| v.as_u64()).unwrap_or(0);

    let p_at = primary.get("reset_at").and_then(|v| v.as_u64()).unwrap_or(0);
    let s_at = secondary.get("reset_at").and_then(|v| v.as_u64()).unwrap_or(0);

    let plan = raw.get("plan_type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(CodexUsageResponse {
        plan_type: plan,
        primary_window: UsageWindow {
            used_percent: p_used,
            remaining_percent: 100u8.saturating_sub(p_used),
            reset_after_seconds: p_after,
            reset_at: p_at,
        },
        secondary_window: UsageWindow {
            used_percent: s_used,
            remaining_percent: 100u8.saturating_sub(s_used),
            reset_after_seconds: s_after,
            reset_at: s_at,
        },
    })
}

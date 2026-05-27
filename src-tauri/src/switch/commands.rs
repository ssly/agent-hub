use std::path::PathBuf;

use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::model::{AuthProfile, ProfileMeta};

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

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_switch_profiles(agent_type: String) -> Result<Vec<AuthProfile>, String> {
    let dir = profiles_dir(&agent_type)?;
    let active_hash = if agent_type == "claude-code" {
        agent_config_path(&agent_type).ok().and_then(|p| hash_env_field(&p))
    } else {
        agent_config_path(&agent_type).ok().and_then(|p| hash_file(&p))
    };

    let mut profiles: Vec<AuthProfile> = Vec::new();

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join("meta.json");
        let config_path = path.join("config.json");
        let Ok(meta_str) = std::fs::read_to_string(&meta_path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<ProfileMeta>(&meta_str) else {
            continue;
        };
        if !config_path.exists() {
            continue;
        }
        let profile_hash = if agent_type == "claude-code" {
            hash_env_field(&config_path)
        } else {
            hash_file(&config_path)
        };
        let is_active = match (&active_hash, &profile_hash) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        profiles.push(AuthProfile {
            id: meta.id,
            note: meta.note,
            saved_at: meta.saved_at,
            is_active,
        });
    }

    profiles.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    Ok(profiles)
}

#[tauri::command]
pub fn save_current_auth_profile(agent_type: String, note: String) -> Result<String, String> {
    let src = agent_config_path(&agent_type)?;
    if !src.exists() {
        return Err("no_active_auth".to_string());
    }
    let content = std::fs::read(&src).map_err(|e| e.to_string())?;

    let id = Uuid::new_v4().to_string();
    let dir = profiles_dir(&agent_type)?.join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    std::fs::write(dir.join("config.json"), &content).map_err(|e| e.to_string())?;

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

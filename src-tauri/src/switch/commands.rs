use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use chrono::Utc;
use uuid::Uuid;

use super::model::{AuthProfile, ListSwitchResponse, ProfileMeta};
use crate::paths::join_relative;

/// Minimum interval between automatic usage network queries (Accounts + tray).
const USAGE_CACHE_TTL_SECS: u64 = 600; // 10 minutes

pub(crate) struct UsageCacheEntry<T> {
    pub fetched_at: u64,
    pub data: T,
}

static CODEX_USAGE_CACHE: Mutex<Option<UsageCacheEntry<CodexTraySnapshot>>> = Mutex::new(None);
static GROK_USAGE_CACHE: Mutex<Option<UsageCacheEntry<GrokUsageResponse>>> = Mutex::new(None);
static KIMI_USAGE_CACHE: Mutex<Option<UsageCacheEntry<KimiUsageResponse>>> = Mutex::new(None);
static CLAUDE_USAGE_CACHE: Mutex<Option<UsageCacheEntry<ClaudeUsageResponse>>> = Mutex::new(None);

pub(crate) fn usage_unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn usage_cache_is_fresh(fetched_at: u64, now: u64) -> bool {
    now.saturating_sub(fetched_at) < USAGE_CACHE_TTL_SECS
}

fn refresh_usage_window_timers(window: &mut UsageWindow, now: u64) {
    window.reset_after_seconds = window.reset_at.saturating_sub(now);
}

fn refresh_usage_windows(windows: &mut [UsageWindow], now: u64) {
    for window in windows {
        refresh_usage_window_timers(window, now);
    }
}

/// Shared Codex snapshot consumed by the Accounts view and the tray popup.
#[derive(Clone, serde::Serialize, Debug)]
pub struct CodexTraySnapshot {
    pub usage: CodexUsageResponse,
    pub reset_credits: Option<CodexResetCreditsResponse>,
    pub last_query_at: u64,
}

fn apply_codex_snapshot_timers(snapshot: &mut CodexTraySnapshot, now: u64) {
    refresh_usage_windows(&mut snapshot.usage.usage_windows, now);
    if let Some(window) = snapshot.usage.primary_window.as_mut() {
        refresh_usage_window_timers(window, now);
    }
    if let Some(window) = snapshot.usage.secondary_window.as_mut() {
        refresh_usage_window_timers(window, now);
    }
}

fn apply_grok_usage_timers(usage: &mut GrokUsageResponse, now: u64) {
    refresh_usage_window_timers(&mut usage.usage_window, now);
}

fn apply_kimi_usage_timers(usage: &mut KimiUsageResponse, now: u64) {
    refresh_usage_windows(&mut usage.usage_windows, now);
    if let Some(window) = usage.window_5h.as_mut() {
        refresh_usage_window_timers(window, now);
    }
    if let Some(window) = usage.window_weekly.as_mut() {
        refresh_usage_window_timers(window, now);
    }
}

fn apply_claude_usage_timers(usage: &mut ClaudeUsageResponse, now: u64) {
    refresh_usage_windows(&mut usage.usage_windows, now);
    if let Some(window) = usage.window_5h.as_mut() {
        refresh_usage_window_timers(window, now);
    }
    if let Some(window) = usage.window_weekly.as_mut() {
        refresh_usage_window_timers(window, now);
    }
}

/// Resolve the storage directory for a given agent type.
fn profiles_dir(agent_type: &str) -> Result<PathBuf, String> {
    let slug = agent_slug(agent_type)?;
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let dir = join_relative(home, &format!(".agent-hub/switch/{slug}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Resolve the live config file path for a given agent type.
fn agent_config_path(agent_type: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let path = match agent_type {
        "claude-code" => join_relative(home, ".claude/settings.json"),
        _ => return Err(format!("unknown_agent_type:{agent_type}")),
    };
    Ok(path)
}

/// Validate & normalise the agent type, returns the directory slug.
fn agent_slug(agent_type: &str) -> Result<&'static str, String> {
    match agent_type {
        "claude-code" => Ok("claude-code"),
        _ => Err(format!("unknown_agent_type:{agent_type}")),
    }
}

fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn extract_claude_auth_token(val: &serde_json::Value) -> Option<String> {
    val.get("env")
        .and_then(|env| env.get("ANTHROPIC_AUTH_TOKEN"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_key(agent_type: &str, content: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(content).ok()?;
    match agent_type {
        "codex" => val
            .get("access_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "claude-code" => extract_claude_auth_token(&val),
        _ => None,
    }
}

fn mask_key(key: &str) -> String {
    if key.len() <= 12 {
        return key.to_string();
    }
    format!("{}...{}", &key[..8], &key[key.len() - 4..])
}

/// Decode the payload (second segment) of a JWT without signature validation.
/// Used only to read display fields (email, name) from the Codex `id_token` —
/// never for trust decisions — so skipping verification is intentional.
fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let mut parts = token.split('.');
    parts.next()?; // header
    let payload = parts.next()?;
    // JWT uses base64url *without* padding; add back padding for the decoder.
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| {
            let padded = match payload.len() % 4 {
                2 => format!("{payload}=="),
                3 => format!("{payload}="),
                _ => payload.to_string(),
            };
            base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&padded)
        })
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// The stable identity of a credential — what makes two snapshots "the same
/// account" even after the secret rotates. This is intentionally *not* the
/// secret itself, because Codex `access_token`s refresh frequently and would
/// otherwise make a just-saved account look like a stranger on every reload.
///
/// - codex → the ChatGPT `account_id` (tokens.account_id / account_id),
///   which is invariant across token refreshes.
/// - claude-code → the `ANTHROPIC_AUTH_TOKEN` itself; two Claude configs are
///   considered the same account exactly when this token matches.
fn extract_account_identity(agent_type: &str, content: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(content).ok()?;
    match agent_type {
        "codex" => val
            .get("tokens")
            .and_then(|t| t.get("account_id"))
            .or_else(|| val.get("account_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "claude-code" => extract_claude_auth_token(&val),
        _ => None,
    }
}

/// A human-readable label for the account — used as the auto-generated `note`
/// when the current account is auto-saved into the pool. Falls back to `None`
/// when no name can be derived, leaving the caller to use a default label.
///
/// - codex → the email inside the `id_token` JWT payload
///   (ChatGPT accounts are best identified by email).
/// - claude-code → no derivable label (key-only auth).
fn extract_account_name(agent_type: &str, content: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(content).ok()?;
    match agent_type {
        "codex" => val
            .get("tokens")
            .and_then(|t| t.get("id_token"))
            .and_then(|v| v.as_str())
            .and_then(decode_jwt_payload)
            .and_then(|p| {
                p.get("email")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            }),
        "claude-code" => None,
        _ => None,
    }
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

    // Read the live auth file once: derive the stable identity, the masked key
    // for the toolbar, and a human-readable name (email) for auto-save.
    let config_path = agent_config_path(&agent_type).ok();
    let live_content = config_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok());

    // Detect the live auth shape. Token mode: settings.json carries
    // env.ANTHROPIC_AUTH_TOKEN (custom account). Otherwise, when official
    // OAuth credentials exist, the identity is the oauthAccount UUID (falling
    // back to the account email) from ~/.claude.json.
    let live_token_identity = live_content
        .as_ref()
        .and_then(|c| extract_account_identity(&agent_type, c));
    let oauth_mode = agent_type == "claude-code" && live_token_identity.is_none();
    let oauth_identity = if oauth_mode {
        claude_oauth_identity()
    } else {
        None
    };
    let (active_identity, current_key) = if oauth_mode {
        (oauth_identity, None)
    } else {
        (
            live_token_identity,
            live_content
                .as_ref()
                .and_then(|c| extract_key(&agent_type, c))
                .map(|k| mask_key(&k)),
        )
    };

    // First pass: read every saved profile and derive its stable identity.
    struct RawProfile {
        meta: ProfileMeta,
        identity: Option<String>,
        key: Option<String>,
    }
    let mut raw: Vec<RawProfile> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
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
            let content = match std::fs::read_to_string(&cfg_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            // OAuth profiles carry their identity in meta.json (captured at
            // save time); token profiles derive it from the stored config.
            let identity = if meta.kind == "oauth" {
                meta.identity.clone()
            } else {
                extract_account_identity(&agent_type, &content)
            };
            let key = if meta.kind == "oauth" {
                None
            } else {
                extract_key(&agent_type, &content).map(|k| mask_key(&k))
            };
            raw.push(RawProfile {
                identity,
                key,
                meta,
            });
        }
    }

    // Auto-save: if a live account exists but no saved profile shares its
    // stable identity, persist it so the current account always appears in the
    // list (and gets selected). This never duplicates a refresh-stale snapshot
    // because the comparison is by identity, not by secret/hash.
    if let Some(identity) = &active_identity {
        let already_saved = raw.iter().any(|r| r.identity.as_deref() == Some(identity));
        if !already_saved {
            let note = if oauth_mode {
                read_claude_oauth_account()
                    .and_then(|a| a.email)
                    .unwrap_or_default()
            } else {
                live_content
                    .as_ref()
                    .and_then(|c| extract_account_name(&agent_type, c))
                    .unwrap_or_default()
            };
            if let Ok(id) = save_current_auth_profile_inner(&agent_type, note.clone(), true) {
                raw.push(RawProfile {
                    identity: active_identity.clone(),
                    key: current_key.clone(),
                    meta: ProfileMeta {
                        id,
                        note,
                        saved_at: now_iso(),
                        kind: if oauth_mode {
                            "oauth".to_string()
                        } else {
                            "token".to_string()
                        },
                        identity: if oauth_mode {
                            active_identity.clone()
                        } else {
                            None
                        },
                    },
                });
            }
        }
    }

    // Mark the active profile (if any) by identity match.
    let mut profiles: Vec<AuthProfile> = raw
        .into_iter()
        .map(|r| AuthProfile {
            is_active: match (&active_identity, &r.identity) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
            id: r.meta.id,
            note: r.meta.note,
            saved_at: r.meta.saved_at,
            key: r.key,
            kind: r.meta.kind,
        })
        .collect();

    profiles.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));

    // Claude Code: the current account always leads the list. When a saved
    // profile has the same ANTHROPIC_AUTH_TOKEN it is already marked active;
    // otherwise the auto-save above just inserted it. Either way it renders
    // as the first card so the top of the page reflects the live account.
    if agent_type == "claude-code" {
        if let Some(pos) = profiles.iter().position(|p| p.is_active) {
            let active = profiles.remove(pos);
            profiles.insert(0, active);
        }
    }

    Ok(ListSwitchResponse {
        profiles,
        current_key,
    })
}

/// Persist the live auth file as a new profile entry. The inner implementation
/// used both by the user-facing `save_current_auth_profile` command and by the
/// auto-save logic in `list_switch_profiles` (which passes `allow_duplicate` to
/// bypass the duplicate-key guard — the current account may legitimately equal
/// an existing profile after a token refresh and we never want auto-save to
/// surface a "duplicate" error to the user).
fn save_current_auth_profile_inner(
    agent_type: &str,
    note: String,
    allow_duplicate: bool,
) -> Result<String, String> {
    let src = agent_config_path(agent_type)?;
    // Claude Code OAuth mode: settings.json has no env token but official
    // /login credentials exist — save the credential JSON instead of the
    // settings file.
    if agent_type == "claude-code" {
        let has_token = std::fs::read_to_string(&src)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
            .and_then(|v| extract_claude_auth_token(&v))
            .is_some();
        if !has_token {
            return save_claude_oauth_profile(agent_type, note, allow_duplicate);
        }
    }
    if !src.exists() {
        return Err("no_active_auth".to_string());
    }
    let content_bytes = std::fs::read(&src).map_err(|e| e.to_string())?;
    let content_str = String::from_utf8_lossy(&content_bytes).to_string();
    if !allow_duplicate {
        if let Some(key) = extract_key(agent_type, &content_str) {
            check_duplicate_key(agent_type, &key, None)?;
        }
    }

    let id = Uuid::new_v4().to_string();
    let dir = profiles_dir(agent_type)?.join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    std::fs::write(dir.join("config.json"), &content_bytes).map_err(|e| e.to_string())?;

    let meta = ProfileMeta {
        id: id.clone(),
        note,
        saved_at: now_iso(),
        kind: "token".to_string(),
        identity: None,
    };
    let meta_str = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("meta.json"), meta_str).map_err(|e| e.to_string())?;

    Ok(id)
}

/// Persist the current official Claude Code OAuth login as a profile. The raw
/// credential JSON (keychain item or `.credentials.json`) is stored verbatim as
/// the profile's `config.json`; the stable identity (oauthAccount UUID, else
/// email) goes into `meta.json` so the active profile can be matched later
/// even after the access token rotates.
fn save_claude_oauth_profile(
    agent_type: &str,
    note: String,
    allow_duplicate: bool,
) -> Result<String, String> {
    let credentials = read_claude_oauth_credentials_raw().ok_or_else(|| {
        "未找到 Claude Code 登录凭证。请先在 Claude Code 中运行 /login。".to_string()
    })?;
    let identity = claude_oauth_identity();
    let note = if note.trim().is_empty() {
        read_claude_oauth_account()
            .and_then(|a| a.email)
            .unwrap_or(note)
    } else {
        note
    };

    if !allow_duplicate {
        if let Some(identity) = &identity {
            let dir = profiles_dir(agent_type)?;
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let meta_path = entry.path().join("meta.json");
                    let Ok(meta_str) = std::fs::read_to_string(&meta_path) else {
                        continue;
                    };
                    let Ok(meta) = serde_json::from_str::<ProfileMeta>(&meta_str) else {
                        continue;
                    };
                    if meta.kind == "oauth" && meta.identity.as_deref() == Some(identity) {
                        return Err("duplicate_key".to_string());
                    }
                }
            }
        }
    }

    let id = Uuid::new_v4().to_string();
    let dir = profiles_dir(agent_type)?.join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    std::fs::write(dir.join("config.json"), credentials.as_bytes()).map_err(|e| e.to_string())?;

    let meta = ProfileMeta {
        id: id.clone(),
        note,
        saved_at: now_iso(),
        kind: "oauth".to_string(),
        identity,
    };
    let meta_str = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("meta.json"), meta_str).map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn save_current_auth_profile(agent_type: String, note: String) -> Result<String, String> {
    save_current_auth_profile_inner(&agent_type, note, false)
}

#[tauri::command]
pub fn add_auth_profile(
    agent_type: String,
    content: String,
    note: String,
) -> Result<String, String> {
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
        kind: "token".to_string(),
        identity: None,
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
        let kind = std::fs::read_to_string(dir.join("meta.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<ProfileMeta>(&s).ok())
            .map(|m| m.kind)
            .unwrap_or_else(|| "token".to_string());

        if kind == "oauth" {
            // Write the saved credential JSON back to where Claude Code reads
            // it (macOS keychain, or `.credentials.json` as the cross-platform
            // fallback), then strip env.ANTHROPIC_AUTH_TOKEN from settings.json
            // so the official OAuth login takes effect.
            let credentials = std::fs::read_to_string(&src).map_err(|e| e.to_string())?;
            write_claude_oauth_credentials(&credentials)?;
            if dest.exists() {
                let current_str = std::fs::read_to_string(&dest).map_err(|e| e.to_string())?;
                let stripped = remove_claude_env_token(&current_str)?;
                let tmp = dest.with_extension("json.tmp");
                std::fs::write(&tmp, stripped.as_bytes()).map_err(|e| e.to_string())?;
                crate::paths::replace_file(&tmp, &dest).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }

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
        crate::paths::replace_file(&tmp, &dest).map_err(|e| e.to_string())?;
    } else {
        let tmp = dest.with_extension("json.tmp");
        std::fs::write(&tmp, &content).map_err(|e| e.to_string())?;
        crate::paths::replace_file(&tmp, &dest).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn update_auth_profile_note(
    agent_type: String,
    id: String,
    note: String,
) -> Result<(), String> {
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
pub fn update_auth_profile_content(
    agent_type: String,
    id: String,
    content: String,
) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&content).map_err(|_| "invalid_json".to_string())?;

    let dir = profiles_dir(&agent_type)?.join(&id);
    if !dir.exists() {
        return Err("profile_not_found".to_string());
    }
    let path = dir.join("config.json");
    let tmp = dir.join("config.json.tmp");
    std::fs::write(&tmp, content.as_bytes()).map_err(|e| e.to_string())?;
    crate::paths::replace_file(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clear_active_auth(agent_type: String) -> Result<String, String> {
    if agent_type == "claude-code" && claude_oauth_active() {
        return Err("官方登录账号请先在 Claude Code 中 /logout".to_string());
    }
    let src = agent_config_path(&agent_type)?;
    if !src.exists() {
        return Err("no_active_auth".to_string());
    }

    let note = format!("auto-backup before clear {}", now_iso());
    let backup_id = save_current_auth_profile(agent_type.clone(), note)?;

    std::fs::remove_file(&src).map_err(|e| e.to_string())?;

    Ok(backup_id)
}

/// Delete the live auth file (e.g. ~/.codex/auth.json) **without** backing it
/// up to the account pool first. Used by the "Clear Account" button so the user
/// can sign out of {agent} while keeping every saved profile in the pool intact.
///
/// Unlike `clear_active_auth`, this never touches `~/.agent-hub/switch/<agent>/`.
#[tauri::command]
pub fn delete_active_auth(agent_type: String) -> Result<(), String> {
    // Official OAuth logins live in the macOS keychain / .credentials.json —
    // never delete those from here; the user must /logout in Claude Code.
    if agent_type == "claude-code" && claude_oauth_active() {
        return Err("官方登录账号请先在 Claude Code 中 /logout".to_string());
    }
    // Validate the agent type up front so we surface a clear error rather than
    // silently deleting an unrelated file.
    let path = agent_config_path(&agent_type)?;
    if !path.exists() {
        return Err("no_active_auth".to_string());
    }
    std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    Ok(())
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

/// Resolved Codex auth: the bearer token + account id needed to call the
/// (undocumented) ChatGPT backend-api WHAM endpoints from a Codex login.
struct CodexAuth {
    access_token: String,
    account_id: String,
    account_name: Option<String>,
}

/// Read `~/.codex/auth.json` (honoring `CODEX_HOME`) and pull out the token +
/// account id. Shared by the usage and reset-credits commands so both use the
/// exact same auth source and error messages.
fn resolve_codex_auth() -> Result<CodexAuth, String> {
    let codex_home: PathBuf = match std::env::var("CODEX_HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            let home = dirs::home_dir().ok_or_else(|| "无法确定用户主目录".to_string())?;
            home.join(".codex")
        }
    };

    let auth_path = codex_home.join("auth.json");
    if !auth_path.exists() {
        return Err(
            "未找到 Codex 认证文件（~/.codex/auth.json）。请先在终端运行 `codex login`。"
                .to_string(),
        );
    }

    let content =
        std::fs::read_to_string(&auth_path).map_err(|e| format!("读取 auth.json 失败: {}", e))?;
    let account_name = extract_account_name("codex", &content);
    let auth: CodexAuthFile =
        serde_json::from_str(&content).map_err(|e| format!("解析 auth.json 失败: {}", e))?;

    let tokens = auth
        .tokens
        .ok_or_else(|| "auth.json 中没有 tokens 字段".to_string())?;
    let access_token = tokens.access_token;
    let account_id = tokens
        .account_id
        .or(auth.account_id)
        .ok_or_else(|| "缺少 account_id，无法查询用量".to_string())?;

    Ok(CodexAuth {
        access_token,
        account_id,
        account_name,
    })
}

/// Build a reqwest client + a configured GET request to a WHAM endpoint.
/// Headers mimic the official Codex CLI so the request is indistinguishable
/// from the client's own rate-limit fetches.
async fn wham_get(url: &str, auth: &CodexAuth) -> Result<reqwest::Response, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    client
        .get(url)
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .header("ChatGPT-Account-Id", &auth.account_id)
        .header("Accept", "application/json")
        .header("User-Agent", "Codex CLI")
        .header("Origin", "https://chatgpt.com")
        .header("Referer", "https://chatgpt.com/")
        .send()
        .await
        .map_err(|e| format!("请求 Codex 接口失败: {}", e))
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct UsageWindow {
    pub used_percent: u8,
    pub remaining_percent: u8,
    pub reset_after_seconds: u64,
    pub reset_at: u64,
    /// Duration of the rate-limit window in seconds.
    /// Common values are 18000 (5h), 604800 (7d), and 2592000 (30d).
    /// Accounts may return any subset in either primary or secondary.
    /// Lets the front-end label the window dynamically instead of hard-coding 5h/7d.
    pub window_seconds: u64,
}

/// "Rate-limit reset" credits — the one-click window reset button on the
/// ChatGPT web UI draws from this pool. `available_count` is how many resets
/// the account still has left.
#[derive(serde::Serialize, Debug, Clone, Default)]
pub struct ResetCredits {
    pub available_count: u32,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct CodexUsageResponse {
    /// Email derived from the current Codex CLI login's ID token. This is
    /// display-only and never used as an authentication or switching key.
    pub account_name: Option<String>,
    pub plan_type: String,
    /// Every usable quota window returned by the API, sorted shortest first.
    /// Keep the named fields below for the existing Accounts view while tray
    /// consumers can render 5h/7d/30d without assuming there are only two.
    pub usage_windows: Vec<UsageWindow>,
    pub primary_window: Option<UsageWindow>,
    pub secondary_window: Option<UsageWindow>,
    pub reset_credits: Option<ResetCredits>,
}

#[tauri::command]
pub async fn get_codex_usage() -> Result<CodexUsageResponse, String> {
    let auth = resolve_codex_auth()?;

    // Call the (undocumented) internal usage endpoint.
    // We prefer /wham/usage as it reliably returns the JSON quota windows.
    let resp = wham_get("https://chatgpt.com/backend-api/wham/usage", &auth).await?;

    if !resp.status().is_success() {
        return Err(format!("Codex 用量接口返回错误: HTTP {}", resp.status()));
    }

    let raw: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析用量响应失败: {}", e))?;

    let plan = raw
        .get("plan_type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // The rate_limit payload may be absent entirely (e.g. API-key auth without a
    // subscription) — treat that as "no usage data" rather than a hard error.
    let reset_credits = map_reset_credits(raw.get("rate_limit_reset_credits"));

    let rate = match raw.get("rate_limit") {
        Some(v) => v,
        None => {
            return Ok(CodexUsageResponse {
                account_name: auth.account_name,
                plan_type: plan,
                usage_windows: Vec::new(),
                primary_window: None,
                secondary_window: None,
                reset_credits,
            });
        }
    };

    // Windows are optional and their position is not a stable label. Determine
    // 5h/7d/30d from each window's duration instead of primary/secondary.
    let primary = map_usage_window(rate.get("primary_window"));
    let secondary = map_usage_window(rate.get("secondary_window"));
    let usage_windows = collect_usage_windows(rate);

    Ok(CodexUsageResponse {
        account_name: auth.account_name,
        plan_type: plan,
        usage_windows,
        primary_window: primary,
        secondary_window: secondary,
        reset_credits,
    })
}

// --- Codex rate-limit reset credits (validity period) ---

/// One banked reset credit from `/wham/rate-limit-reset-credits`.
/// `expires_at` is an ISO-8601 UTC timestamp; we surface it so the front-end
/// can show a countdown to when the credit expires (each credit is valid ~30d).
#[derive(serde::Serialize, Debug, Clone)]
pub struct ResetCreditEntry {
    pub status: String,
    /// ISO-8601 timestamp, e.g. "2026-07-31T20:03:43.074555Z".
    pub expires_at: Option<String>,
    /// ISO-8601 timestamp of when the credit was granted.
    pub granted_at: Option<String>,
    /// Human-readable title, e.g. "Full reset (Weekly + 5 hr)".
    pub title: Option<String>,
}

#[derive(serde::Serialize, Debug, Clone, Default)]
pub struct CodexResetCreditsResponse {
    pub available_count: u32,
    /// Soonest-expiring `available` credit (precomputed for the front-end).
    pub next_expires_at: Option<String>,
    /// All banked credits (available + redeemed), newest first.
    pub credits: Vec<ResetCreditEntry>,
}

/// Fetch the separate reset-credits list endpoint. Unlike `/wham/usage` (which
/// only carries `available_count`), this one returns per-credit `expires_at`,
/// so we can display how long the credits are valid for. Borrowed from the
/// open-source codex_quota.py / CodexBar pattern.
#[tauri::command]
pub async fn get_codex_reset_credits() -> Result<CodexResetCreditsResponse, String> {
    let auth = resolve_codex_auth()?;
    let resp = wham_get(
        "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits",
        &auth,
    )
    .await?;

    if !resp.status().is_success() {
        return Err(format!("重置券接口返回错误: HTTP {}", resp.status()));
    }

    let raw: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析重置券响应失败: {}", e))?;

    let available_count = raw
        .get("available_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let mut credits: Vec<ResetCreditEntry> = Vec::new();
    if let Some(arr) = raw.get("credits").and_then(|v| v.as_array()) {
        for c in arr {
            credits.push(ResetCreditEntry {
                status: c
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                expires_at: c
                    .get("expires_at")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                granted_at: c
                    .get("granted_at")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                title: c
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        }
    }

    // Soonest-expiring *available* credit → the front-end's "X 天后到期".
    // Sort available credits by expires_at ascending and take the first.
    let next_expires_at = credits
        .iter()
        .filter(|c| c.status == "available" && c.expires_at.is_some())
        .min_by_key(|c| c.expires_at.clone().unwrap_or_default())
        .and_then(|c| c.expires_at.clone());

    Ok(CodexResetCreditsResponse {
        available_count,
        next_expires_at,
        credits,
    })
}

/// Parse the `rate_limit_reset_credits` node. Returns `None` when the node is
/// absent or null — this is expected for plans that don't grant reset credits.
fn map_reset_credits(node: Option<&serde_json::Value>) -> Option<ResetCredits> {
    let obj = node?.as_object()?;
    let available_count = obj
        .get("available_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    Some(ResetCredits { available_count })
}

/// Parse a single rate-limit window from the raw API value.
/// Returns `None` when the window is missing or null — this is expected for
/// Free accounts (no 5h/7d windows) and must not be treated as an error.
fn map_usage_window(node: Option<&serde_json::Value>) -> Option<UsageWindow> {
    let win = node?.as_object()?;
    let used = win
        .get("used_percent")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u8;
    let after = win
        .get("reset_after_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let at = win.get("reset_at").and_then(|v| v.as_u64()).unwrap_or(0);
    // OpenAI exposes the window length as either "limit_window_seconds" (current)
    // or "window_seconds" (older payloads). Fall back to 0 when absent.
    let window_seconds = win
        .get("limit_window_seconds")
        .or_else(|| win.get("window_seconds"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some(UsageWindow {
        used_percent: used,
        remaining_percent: 100u8.saturating_sub(used),
        reset_after_seconds: after,
        reset_at: at,
        window_seconds,
    })
}

/// Collect every window-shaped object below `rate_limit`. Current payloads use
/// `primary_window` and `secondary_window`; walking the object also preserves a
/// future `monthly_window`/array instead of silently dropping the third row.
fn collect_usage_windows(rate: &serde_json::Value) -> Vec<UsageWindow> {
    fn visit(node: &serde_json::Value, windows: &mut Vec<UsageWindow>) {
        if let Some(window) =
            map_usage_window(Some(node)).filter(|window| window.window_seconds > 0)
        {
            windows.push(window);
            return;
        }

        match node {
            serde_json::Value::Object(object) => {
                object.values().for_each(|value| visit(value, windows));
            }
            serde_json::Value::Array(array) => {
                array.iter().for_each(|value| visit(value, windows));
            }
            _ => {}
        }
    }

    let mut windows = Vec::new();
    visit(rate, &mut windows);
    windows.sort_by_key(|window| window.window_seconds);
    windows.dedup_by_key(|window| window.window_seconds);
    windows
}

// --- Grok Build quota via the Grok CLI billing endpoint ---------------------
//
// Grok CLI requests `/v1/billing?format=credits`, which returns the unified
// weekly credit window (`creditUsagePercent` + `currentPeriod`). The bare
// `/v1/billing` endpoint still exists but serves a legacy monthly-credit
// payload (monthlyLimit/used) that no longer matches the Recap UI.

const GROK_BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";

#[derive(serde::Serialize, Debug, Clone)]
pub struct GrokUsageResponse {
    /// Display label derived from the current Grok CLI login when available.
    pub account_name: Option<String>,
    pub plan_type: String,
    /// `weekly` for SuperGrok / unified billing (`format=credits`);
    /// `monthly` for the legacy monthly-credit payload.
    pub period_type: String,
    pub usage_window: UsageWindow,
    /// Raw credit values when the billing endpoint provides them.
    pub limit_value: Option<f64>,
    pub used_value: Option<f64>,
    pub prepaid_balance: Option<f64>,
    pub on_demand_cap: Option<f64>,
    pub on_demand_used: Option<f64>,
    pub on_demand_enabled: Option<bool>,
    /// Unix seconds when this live billing response was fetched.
    pub fetched_at: u64,
}

struct GrokAuth {
    bearer: String,
    account_name: Option<String>,
}

fn grok_home_dir() -> Result<PathBuf, String> {
    match std::env::var("GROK_HOME") {
        Ok(dir) => Ok(PathBuf::from(dir)),
        Err(_) => dirs::home_dir()
            .map(|home| home.join(".grok"))
            .ok_or_else(|| "无法确定用户主目录".to_string()),
    }
}

/// Grok stores one or more login records below top-level keys in auth.json.
/// We only read the first record with a non-empty session key; Agent Hub never
/// refreshes, rewrites, or switches these credentials.
fn resolve_grok_auth(grok_home: &std::path::Path) -> Result<GrokAuth, String> {
    let auth_path = grok_home.join("auth.json");
    if !auth_path.exists() {
        return Err(
            "未找到 Grok Build 认证文件（~/.grok/auth.json）。请先在 Grok CLI 中登录。".to_string(),
        );
    }

    let content = std::fs::read_to_string(&auth_path)
        .map_err(|e| format!("读取 Grok auth.json 失败: {e}"))?;
    let raw: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("解析 Grok auth.json 失败: {e}"))?;

    let direct = raw
        .get("key")
        .and_then(|key| key.as_str())
        .filter(|key| !key.trim().is_empty())
        .map(|key| (None, &raw, key));
    let nested = raw.as_object().and_then(|object| {
        object.iter().find_map(|(label, value)| {
            value
                .get("key")
                .and_then(|key| key.as_str())
                .filter(|key| !key.trim().is_empty())
                .map(|key| (Some(label.as_str()), value, key))
        })
    });
    let (record_label, record, bearer) = direct
        .or(nested)
        .ok_or_else(|| "Grok auth.json 中没有可用的登录凭据".to_string())?;

    let account_name = ["email", "user_email", "name"]
        .iter()
        .find_map(|field| record.get(field).and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            record_label
                .filter(|label| label.contains('@'))
                .map(str::to_string)
        });

    Ok(GrokAuth {
        bearer: bearer.to_string(),
        account_name,
    })
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct UsageProviderAvailability {
    pub codex: bool,
    pub grok_build: bool,
    pub kimi_code: bool,
    pub claude_code: bool,
}

/// Report which quota providers have usable local credentials without making
/// a network request. The tray uses this to select the Accounts view's current
/// provider and to fall back only when that provider is genuinely signed out.
#[tauri::command]
pub fn get_usage_provider_availability() -> UsageProviderAvailability {
    let codex = resolve_codex_auth().is_ok();
    let grok_build = grok_home_dir()
        .and_then(|home| resolve_grok_auth(&home))
        .is_ok();
    let kimi_code = resolve_kimi_credential().is_ok();
    let claude_code = resolve_claude_oauth().is_ok();

    UsageProviderAvailability {
        codex,
        grok_build,
        kimi_code,
        claude_code,
    }
}

fn json_number(node: Option<&serde_json::Value>) -> Option<f64> {
    let node = node?;
    if let Some(value) = node.as_f64() {
        return Some(value);
    }
    if let Some(value) = node.as_str().and_then(|value| value.parse::<f64>().ok()) {
        return Some(value);
    }
    node.get("val").and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
    })
}

fn json_timestamp(node: Option<&serde_json::Value>) -> Option<u64> {
    let node = node?;
    node.as_u64().or_else(|| {
        node.as_str().and_then(|value| {
            chrono::DateTime::parse_from_rfc3339(value)
                .ok()
                .and_then(|date| u64::try_from(date.timestamp()).ok())
        })
    })
}

fn map_grok_usage(
    raw: &serde_json::Value,
    account_name: Option<String>,
    fetched_at: u64,
) -> Result<GrokUsageResponse, String> {
    // Live /v1/billing responses expose the config directly. Some older
    // shapes wrap it under config / ctx.config.
    let payload = raw
        .get("config")
        .or_else(|| raw.get("ctx").and_then(|ctx| ctx.get("config")))
        .unwrap_or(raw);
    let period = payload
        .get("currentPeriod")
        .or_else(|| payload.get("current_period"));
    let period_start = json_timestamp(
        period.and_then(|value| value.get("start").or_else(|| value.get("startsAt"))),
    )
    .or_else(|| json_timestamp(payload.get("billingPeriodStart")));
    let period_end =
        json_timestamp(period.and_then(|value| value.get("end").or_else(|| value.get("endsAt"))))
            .or_else(|| json_timestamp(payload.get("billingPeriodEnd")));
    let reset_at = period_end.ok_or_else(|| "Grok billing 响应缺少额度重置时间".to_string())?;
    let now = u64::try_from(Utc::now().timestamp()).unwrap_or(0);
    let window_seconds = match (period_start, period_end) {
        (Some(start), Some(end)) => end.saturating_sub(start),
        _ => 0,
    };
    let limit_value = json_number(
        payload
            .get("monthlyLimit")
            .or_else(|| payload.get("monthly_limit")),
    );
    let used_value = json_number(payload.get("used"));
    let legacy_used_percent = json_number(
        payload
            .get("creditUsagePercent")
            .or_else(|| payload.get("credit_usage_percent")),
    );
    let used_percent = legacy_used_percent.or_else(|| match (used_value, limit_value) {
        (Some(used), Some(limit)) if limit > 0.0 => Some((used / limit) * 100.0),
        _ => None,
    });
    let used = used_percent
        .ok_or_else(|| "Grok billing 响应缺少可识别的额度数据".to_string())?
        .round()
        .clamp(0.0, 100.0) as u8;

    let period_type = period
        .and_then(|value| value.get("type"))
        .and_then(|value| value.as_str())
        .map(|value| {
            if value.to_ascii_uppercase().contains("WEEK") {
                "weekly"
            } else {
                "monthly"
            }
        })
        .unwrap_or_else(|| {
            if limit_value.is_some() {
                "monthly"
            } else {
                "weekly"
            }
        })
        .to_string();

    let plan_type = raw
        .get("subscriptionTier")
        .or_else(|| raw.get("subscription_tier"))
        .or_else(|| raw.get("ctx").and_then(|ctx| ctx.get("subscriptionTier")))
        .or_else(|| payload.get("subscriptionTier"))
        .and_then(|value| value.as_str())
        .unwrap_or("Grok")
        .to_string();

    Ok(GrokUsageResponse {
        account_name,
        plan_type,
        period_type,
        usage_window: UsageWindow {
            used_percent: used,
            remaining_percent: 100u8.saturating_sub(used),
            reset_after_seconds: reset_at.saturating_sub(now),
            reset_at,
            window_seconds,
        },
        limit_value,
        used_value,
        prepaid_balance: json_number(payload.get("prepaidBalance")),
        on_demand_cap: json_number(payload.get("onDemandCap")),
        on_demand_used: json_number(payload.get("onDemandUsed")),
        on_demand_enabled: raw
            .get("onDemandEnabled")
            .or_else(|| raw.get("ctx").and_then(|ctx| ctx.get("onDemandEnabled")))
            .and_then(|value| value.as_bool()),
        fetched_at,
    })
}

/// Outcome of a live Grok billing request (no CLI-log fallback).
enum GrokFetchOutcome {
    Ok(GrokUsageResponse),
    /// Auth missing, network timeout, HTTP error — caller surfaces the error.
    Transport(String),
    /// Body arrived but JSON/schema mapping failed — keep previous usage.
    Parse(String),
}

/// Always hits Grok's live billing API. Never reads CLI log caches.
async fn fetch_grok_usage() -> GrokFetchOutcome {
    let grok_home = match grok_home_dir() {
        Ok(path) => path,
        Err(error) => return GrokFetchOutcome::Transport(error),
    };
    let auth = match resolve_grok_auth(&grok_home) {
        Ok(auth) => auth,
        Err(error) => return GrokFetchOutcome::Transport(error),
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => return GrokFetchOutcome::Transport(error.to_string()),
    };
    let response = match client
        .get(GROK_BILLING_URL)
        .header("Authorization", format!("Bearer {}", auth.bearer))
        .header("Accept", "application/json")
        .header("User-Agent", "Grok CLI")
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return GrokFetchOutcome::Transport(if error.is_timeout() {
                "请求 Grok 用量接口超时（10s）。请检查网络后重试。".to_string()
            } else {
                format!("请求 Grok 用量接口失败: {error}")
            });
        }
    };
    if !response.status().is_success() {
        return GrokFetchOutcome::Transport(format!(
            "Grok 用量接口返回错误: HTTP {}",
            response.status()
        ));
    }
    let raw = match response.json::<serde_json::Value>().await {
        Ok(raw) => raw,
        Err(error) => {
            return GrokFetchOutcome::Parse(format!("解析 Grok 用量响应失败: {error}"));
        }
    };
    match map_grok_usage(
        &raw,
        auth.account_name,
        u64::try_from(Utc::now().timestamp()).unwrap_or(0),
    ) {
        Ok(usage) => GrokFetchOutcome::Ok(usage),
        Err(error) => GrokFetchOutcome::Parse(error),
    }
}

fn clone_cached_grok_usage(now: u64) -> Option<GrokUsageResponse> {
    let guard = GROK_USAGE_CACHE.lock().ok()?;
    let entry = guard.as_ref()?;
    let mut usage = entry.data.clone();
    apply_grok_usage_timers(&mut usage, now);
    Some(usage)
}

/// Grok Build usage. Shared by the Accounts view and tray popup.
///
/// Only live `/v1/billing` data is written. A short in-process TTL avoids
/// hammering the API on rapid tray reopen. On parse failures the previous
/// successful snapshot is kept and returned; transport errors surface as Err
/// without clearing that snapshot. Manual refresh should pass `force: true`.
#[tauri::command]
pub async fn get_grok_usage(force: Option<bool>) -> Result<GrokUsageResponse, String> {
    let force = force.unwrap_or(false);
    let now = usage_unix_now();
    if !force {
        if let Ok(guard) = GROK_USAGE_CACHE.lock() {
            if let Some(entry) = guard.as_ref() {
                if usage_cache_is_fresh(entry.fetched_at, now) {
                    let mut usage = entry.data.clone();
                    apply_grok_usage_timers(&mut usage, now);
                    return Ok(usage);
                }
            }
        }
    }

    match fetch_grok_usage().await {
        GrokFetchOutcome::Ok(usage) => {
            if let Ok(mut guard) = GROK_USAGE_CACHE.lock() {
                *guard = Some(UsageCacheEntry {
                    fetched_at: usage.fetched_at.max(now),
                    data: usage.clone(),
                });
            }
            Ok(usage)
        }
        GrokFetchOutcome::Parse(error) => {
            // Keep the last good numbers; only report the parse problem when
            // there is nothing previous to show.
            if let Some(previous) = clone_cached_grok_usage(now) {
                return Ok(previous);
            }
            Err(error)
        }
        GrokFetchOutcome::Transport(error) => Err(error),
    }
}

// --- Kimi Code quota via the Kimi CLI usages endpoint -----------------------
//
// Authentication model: the kimi CLI stores a long-lived Coding Plan API key
// (`sk-kimi-…`) at `~/.kimi-code/config.toml`. We read that key and call the
// usages endpoint read-only. This is the only officially sanctioned path for
// third-party tools per Kimi's docs — OAuth tokens are explicitly scoped to
// the kimi CLI itself, so we deliberately do NOT touch the keychain or any
// OAuth credential file. See https://www.kimi.com/zh-cn/help/kimi-code/third-party-agents

const KIMI_USAGES_URL: &str = "https://api.kimi.com/coding/v1/usages";

#[derive(serde::Serialize, Debug, Clone)]
pub struct KimiUsageResponse {
    /// Display label derived from the JWT email claim, if the key embeds one.
    pub account_name: Option<String>,
    /// How this key authenticates — `METHOD_API_KEY` for the long-lived
    /// `sk-kimi-…` Coding Plan key, `METHOD_OAUTH` for a CLI OAuth login.
    /// Surfaced as a badge so the user knows which credential path is in use.
    pub auth_method: String,
    /// The 5-hour rolling rate-limit window. Always present when the API
    /// returns a usable response.
    pub window_5h: Option<UsageWindow>,
    /// The weekly quota window (resets every 7 days from subscription date).
    pub window_weekly: Option<UsageWindow>,
    /// Raw weekly limit/used values, for "used / limit" display.
    pub weekly_limit: Option<u64>,
    pub weekly_used: Option<u64>,
    /// Windows in ascending order, for any UI that iterates generically.
    pub usage_windows: Vec<UsageWindow>,
    pub fetched_at: u64,
}

/// A resolved Kimi Coding Plan API key (`sk-kimi-…`) plus the account label
/// derived from its JWT payload (if present). Long-lived, never refreshed.
struct KimiCredential {
    api_key: String,
    account_name: Option<String>,
}

impl KimiCredential {
    fn is_usable(&self) -> bool {
        !self.api_key.trim().is_empty()
    }
}

fn kimi_home_dir() -> Result<PathBuf, String> {
    match std::env::var("KIMI_CODE_HOME") {
        Ok(dir) => Ok(PathBuf::from(dir)),
        Err(_) => dirs::home_dir()
            .map(|home| home.join(".kimi-code"))
            .ok_or_else(|| "无法确定用户主目录".to_string()),
    }
}

/// Decode the JWT payload (without signature verification) and pull the email
/// claim out for display. Some `sk-kimi-…` keys embed the user email; if not
/// present we just fall back to a generic account label downstream.
fn kimi_account_name_from_jwt(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    parts.next()?;
    let payload = parts.next()?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(payload))
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    value
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Parse `~/.kimi-code/config.toml` and pull the first provider's API key.
/// The kimi CLI stores a long-lived `sk-kimi-…` Coding Plan key here:
///   [providers.kimi-for-coding]
///   api_key = "sk-kimi-…"
fn read_kimi_credential_config(home: &std::path::Path) -> Option<KimiCredential> {
    let path = home.join("config.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let doc: toml::Value = toml::from_str(&content).ok()?;
    let providers = doc.get("providers")?.as_table()?;
    for (_, provider) in providers {
        let Some(key) = provider.get("api_key").and_then(toml::Value::as_str) else {
            continue;
        };
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Some(KimiCredential {
                api_key: trimmed.to_string(),
                account_name: kimi_account_name_from_jwt(trimmed),
            });
        }
    }
    None
}

/// Resolve the Kimi Coding Plan API key from `~/.kimi-code/config.toml`.
/// This is the only supported credential source: the OAuth tokens the kimi CLI
/// may store elsewhere are explicitly scoped to the CLI itself per Kimi's ToS,
/// so we deliberately do not read the keychain or any OAuth credential file.
fn resolve_kimi_credential() -> Result<KimiCredential, String> {
    let home = kimi_home_dir()?;
    if let Some(cred) = read_kimi_credential_config(&home).filter(KimiCredential::is_usable) {
        return Ok(cred);
    }
    Err(
        "未找到 Kimi Code 凭据（~/.kimi-code/config.toml 中的 [providers.*].api_key）。请先在 Kimi CLI 中登录。"
            .to_string(),
    )
}

/// Normalise a limit/remaining node — Kimi returns them as JSON strings.
fn kimi_quota_value(node: Option<&serde_json::Value>) -> Option<u64> {
    let node = node?;
    if let Some(n) = node.as_u64() {
        return Some(n);
    }
    if let Some(n) = node.as_i64() {
        return u64::try_from(n).ok();
    }
    if let Some(n) = node.as_f64() {
        return Some(n.round() as u64);
    }
    node.as_str().and_then(|text| {
        text.parse::<u64>()
            .ok()
            .or_else(|| text.parse::<f64>().ok().map(|v| v.round() as u64))
    })
}

fn kimi_reset_unix(node: Option<&serde_json::Value>) -> Option<u64> {
    let node = node?;
    json_timestamp(Some(node))
}

/// Derive `window_seconds` from a Kimi `window` descriptor: `{duration, timeUnit}`.
/// The API emits both plain (`"HOUR"`, `"MINUTE"`) and enum-prefixed
/// (`"TIME_UNIT_HOUR"`, `"TIME_UNIT_MINUTE"`) unit strings depending on build;
/// we accept both.
fn kimi_window_seconds(window: Option<&serde_json::Value>) -> Option<u64> {
    let window = window?;
    let duration = window.get("duration").and_then(|v| v.as_u64())?;
    let unit = window
        .get("timeUnit")
        .or_else(|| window.get("time_unit"))
        .and_then(|v| v.as_str())?;
    // Normalise to the trailing unit word, e.g. "TIME_UNIT_MINUTE" -> "MINUTE".
    let normalized = unit.to_ascii_uppercase();
    let unit_word = normalized
        .rsplit('_')
        .next()
        .filter(|tail| !tail.is_empty())
        .unwrap_or(&normalized);
    let multiplier = match unit_word {
        "MINUTE" => 60,
        "HOUR" => 3_600,
        "DAY" => 86_400,
        "MONTH" => 2_592_000,
        _ => return None,
    };
    Some(duration.saturating_mul(multiplier))
}

/// Build a usage window from a `{limit, remaining, resetTime}` detail object.
/// A parsed Kimi window with its raw limit/used values, so callers can show
/// "used 43 / 100" alongside the percent ring.
struct KimiParsedWindow {
    window: UsageWindow,
    limit: Option<u64>,
    used: Option<u64>,
}

fn kimi_build_window(
    detail: &serde_json::Value,
    window_seconds: u64,
    now: u64,
) -> Option<KimiParsedWindow> {
    let limit = kimi_quota_value(detail.get("limit"))?;
    // `used` is sometimes missing (5h sub-window); derive it from limit-remaining.
    let used = kimi_quota_value(detail.get("used"))
        .or_else(|| kimi_quota_value(detail.get("remaining")).map(|r| limit.saturating_sub(r)));
    let used_pct = used.unwrap_or(0);
    let used_percent = if limit > 0 {
        ((used_pct as f64 / limit as f64) * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8
    } else {
        0
    };
    let reset_at = kimi_reset_unix(detail.get("resetTime"))
        .or_else(|| kimi_reset_unix(detail.get("reset_at")))
        .unwrap_or(0);
    Some(KimiParsedWindow {
        window: UsageWindow {
            used_percent,
            remaining_percent: 100u8.saturating_sub(used_percent),
            reset_after_seconds: reset_at.saturating_sub(now),
            reset_at,
            window_seconds,
        },
        limit: Some(limit),
        used,
    })
}

fn map_kimi_usage(
    raw: &serde_json::Value,
    account_name: Option<String>,
    fetched_at: u64,
) -> Result<KimiUsageResponse, String> {
    let now = u64::try_from(Utc::now().timestamp()).unwrap_or(0);
    let mut window_weekly: Option<KimiParsedWindow> = None;
    let mut window_5h: Option<KimiParsedWindow> = None;

    // Top-level `usage` is the rolling weekly window.
    if let Some(usage) = raw.get("usage") {
        window_weekly = kimi_build_window(usage, 604_800, now);
    }

    // `limits[]` carries the 5-hour rolling window (and possibly others).
    if let Some(limits) = raw.get("limits").and_then(|v| v.as_array()) {
        for entry in limits {
            let Some(seconds) = kimi_window_seconds(entry.get("window")) else {
                continue;
            };
            let Some(detail) = entry
                .get("detail")
                .or_else(|| entry.get("rateLimit"))
                .or_else(|| entry.get("rate_limit"))
            else {
                continue;
            };
            let parsed = kimi_build_window(detail, seconds, now);
            // Pick the ~5h window (15000–25000s) as the primary rate-limit card.
            if window_5h.is_none() && (14_000..=25_000).contains(&seconds) {
                window_5h = parsed;
            }
        }
    }

    let mut windows: Vec<UsageWindow> = Vec::new();
    if let Some(ref w) = window_5h {
        windows.push(w.window.clone());
    }
    if let Some(ref w) = window_weekly {
        windows.push(w.window.clone());
    }
    windows.sort_by_key(|w| w.window_seconds);
    windows.dedup_by_key(|w| w.window_seconds);

    if windows.is_empty() {
        return Err("Kimi 用量响应中没有可识别的窗口数据".to_string());
    }

    let auth_method = raw
        .get("authentication")
        .and_then(|a| a.get("method"))
        .and_then(|v| v.as_str())
        .unwrap_or("METHOD_API_KEY")
        .to_string();

    let weekly_limit = window_weekly.as_ref().and_then(|p| p.limit);
    let weekly_used = window_weekly.as_ref().and_then(|p| p.used);

    Ok(KimiUsageResponse {
        account_name,
        auth_method,
        window_5h: window_5h.map(|p| p.window),
        window_weekly: window_weekly.map(|p| p.window),
        weekly_limit,
        weekly_used,
        usage_windows: windows,
        fetched_at,
    })
}

async fn fetch_kimi_usage() -> Result<KimiUsageResponse, String> {
    let cred = resolve_kimi_credential()?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("构建 Kimi 用量请求失败: {e}"))?;

    let response = client
        .get(KIMI_USAGES_URL)
        .header("Authorization", format!("Bearer {}", cred.api_key))
        .header("Accept", "application/json")
        .header("User-Agent", "Kimi CLI")
        .send()
        .await
        .map_err(|e| format!("请求 Kimi 用量接口失败: {e}"))?;
    let status = response.status();

    if !status.is_success() {
        return Err(format!("Kimi 用量接口返回错误: HTTP {status}"));
    }

    let raw: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 Kimi 用量响应失败: {e}"))?;
    map_kimi_usage(
        &raw,
        cred.account_name,
        u64::try_from(Utc::now().timestamp()).unwrap_or(0),
    )
}

/// Kimi Code usage. Shared by the Accounts view and tray popup.
///
/// When `force` is false/omitted, returns the in-memory cache if it is younger
/// than 10 minutes. Manual refresh buttons should pass `force: true`.
#[tauri::command]
pub async fn get_kimi_usage(force: Option<bool>) -> Result<KimiUsageResponse, String> {
    let force = force.unwrap_or(false);
    let now = usage_unix_now();
    if !force {
        if let Ok(guard) = KIMI_USAGE_CACHE.lock() {
            if let Some(entry) = guard.as_ref() {
                if usage_cache_is_fresh(entry.fetched_at, now) {
                    let mut usage = entry.data.clone();
                    apply_kimi_usage_timers(&mut usage, now);
                    return Ok(usage);
                }
            }
        }
    }

    let usage = fetch_kimi_usage().await?;
    if let Ok(mut guard) = KIMI_USAGE_CACHE.lock() {
        *guard = Some(UsageCacheEntry {
            fetched_at: usage.fetched_at.max(now),
            data: usage.clone(),
        });
    }
    Ok(usage)
}

// --- Claude Code quota via the official OAuth login ---------------------------
//
// Authentication model: Claude Code `/login` stores OAuth subscription
// credentials in the macOS login keychain (service `Claude Code-credentials`)
// or, on Linux/Windows and as a fallback, in
// `<CLAUDE_CONFIG_DIR|~/.claude>/.credentials.json`. We only ever *read* these
// (except for the explicit account-switch write-back) and never call the
// refresh endpoint: when the ~8h access token expires we ask the user to open
// Claude Code once so it refreshes itself, avoiding any refresh-token
// rotation race with the official client.

const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const CLAUDE_KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

#[derive(serde::Serialize, Debug, Clone)]
pub struct ClaudeUsageResponse {
    /// Email from `~/.claude.json` `oauthAccount.emailAddress`, display-only.
    pub account_name: Option<String>,
    /// `subscriptionType` from the credential JSON (`max`/`pro`/…), or
    /// "unknown" when the credential does not carry one.
    pub plan_type: String,
    /// The 5-hour rate-limit window (`five_hour`).
    pub window_5h: Option<UsageWindow>,
    /// The weekly quota window (`seven_day`).
    pub window_weekly: Option<UsageWindow>,
    /// Windows in ascending order, for any UI that iterates generically.
    pub usage_windows: Vec<UsageWindow>,
    pub fetched_at: u64,
}

/// Resolved Claude Code OAuth credential: the bearer token plus its expiry
/// (unix milliseconds; 0 = unknown, e.g. from the env var) and display fields.
struct ClaudeOauth {
    access_token: String,
    expires_at_ms: u64,
    subscription_type: Option<String>,
    account_name: Option<String>,
}

/// Identity fields of the official login, from `~/.claude.json` `oauthAccount`.
struct ClaudeOauthAccount {
    account_uuid: Option<String>,
    email: Option<String>,
}

/// Read `~/.claude.json` and pull the top-level `oauthAccount` identity block.
/// Returns `None` when the file or the block is absent (never logged in via
/// the official flow).
fn read_claude_oauth_account() -> Option<ClaudeOauthAccount> {
    let home = dirs::home_dir()?;
    let content = std::fs::read_to_string(home.join(".claude.json")).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    let account = val.get("oauthAccount")?;
    let non_empty = |key: &str| {
        account
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Some(ClaudeOauthAccount {
        account_uuid: non_empty("accountUuid"),
        email: non_empty("emailAddress"),
    })
}

/// The Claude Code config directory: `CLAUDE_CONFIG_DIR` or `~/.claude`.
fn claude_config_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    dirs::home_dir().map(|home| home.join(".claude"))
}

/// Parse the credential JSON shared by the keychain item and
/// `.credentials.json`: `{ "claudeAiOauth": { accessToken, expiresAt, … } }`.
/// Pure so it can be unit-tested; returns (access_token, expires_at_ms,
/// subscription_type).
fn parse_claude_credentials(content: &str) -> Result<(String, u64, Option<String>), String> {
    let val: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("解析 Claude 凭证失败: {e}"))?;
    let oauth = val
        .get("claudeAiOauth")
        .ok_or_else(|| "Claude 凭证缺少 claudeAiOauth 字段".to_string())?;
    let access_token = oauth
        .get("accessToken")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Claude 凭证缺少 accessToken".to_string())?
        .to_string();
    let expires_at_ms = oauth
        .get("expiresAt")
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
        .unwrap_or(0);
    let subscription_type = oauth
        .get("subscriptionType")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok((access_token, expires_at_ms, subscription_type))
}

/// Read the credential JSON from the macOS login keychain via the `security`
/// CLI. May trigger a system authorization prompt; any failure (non-zero exit,
/// timeout, empty output) falls back to the file-based credential.
#[cfg(target_os = "macos")]
fn read_keychain_credentials() -> Option<String> {
    use std::io::Read as _;
    use std::process::{Command, Stdio};

    let mut child = Command::new("security")
        .args(["find-generic-password", "-s", CLAUDE_KEYCHAIN_SERVICE, "-w"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut out = String::new();
                child.stdout.take()?.read_to_string(&mut out).ok()?;
                let trimmed = out.trim();
                if trimmed.is_empty() {
                    return None;
                }
                return Some(trimmed.to_string());
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn read_keychain_credentials() -> Option<String> {
    None
}

/// Read the raw credential JSON from keychain or `.credentials.json`, without
/// the `CLAUDE_CODE_OAUTH_TOKEN` env override (which has no full JSON to save).
/// Used by the account-switch save path.
fn read_claude_oauth_credentials_raw() -> Option<String> {
    if let Some(raw) = read_keychain_credentials() {
        return Some(raw);
    }
    let path = claude_config_dir()?.join(".credentials.json");
    std::fs::read_to_string(path).ok()
}

/// Resolve the current official Claude Code OAuth credential, read-only.
/// Priority: `CLAUDE_CODE_OAUTH_TOKEN` env var → macOS keychain →
/// `.credentials.json`.
fn resolve_claude_oauth() -> Result<ClaudeOauth, String> {
    let account_name = read_claude_oauth_account().and_then(|a| a.email);

    if let Ok(token) = std::env::var("CLAUDE_CODE_OAUTH_TOKEN") {
        let token = token.trim();
        if !token.is_empty() {
            return Ok(ClaudeOauth {
                access_token: token.to_string(),
                expires_at_ms: 0,
                subscription_type: None,
                account_name,
            });
        }
    }

    if let Some(raw) = read_claude_oauth_credentials_raw() {
        let (access_token, expires_at_ms, subscription_type) = parse_claude_credentials(&raw)?;
        return Ok(ClaudeOauth {
            access_token,
            expires_at_ms,
            subscription_type,
            account_name,
        });
    }

    Err("未找到 Claude Code 登录凭证。请先在 Claude Code 中运行 /login。".to_string())
}

/// Stable identity of the current official OAuth login: the
/// `oauthAccount.accountUuid`, falling back to the email. Returns `None` when
/// no usable OAuth credential exists (so token mode / signed-out stays
/// distinguishable).
fn claude_oauth_identity() -> Option<String> {
    resolve_claude_oauth().ok()?;
    read_claude_oauth_account().and_then(|a| a.account_uuid.or(a.email))
}

/// Whether the live Claude Code auth is an official OAuth login: settings.json
/// carries no env token, but OAuth credentials exist. Used to guard the
/// destructive clear/delete paths.
fn claude_oauth_active() -> bool {
    let has_token = agent_config_path("claude-code")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|v| extract_claude_auth_token(&v))
        .is_some();
    !has_token && resolve_claude_oauth().is_ok()
}

/// Remove `env.ANTHROPIC_AUTH_TOKEN` from a settings.json document, preserving
/// every other field. A now-empty `env` object is dropped entirely.
fn remove_claude_env_token(content: &str) -> Result<String, String> {
    let mut val: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("解析 settings.json 失败: {e}"))?;
    if let Some(obj) = val.as_object_mut() {
        let drop_env = match obj.get_mut("env").and_then(|e| e.as_object_mut()) {
            Some(env) => {
                env.remove("ANTHROPIC_AUTH_TOKEN");
                env.is_empty()
            }
            None => false,
        };
        if drop_env {
            obj.remove("env");
        }
    }
    serde_json::to_string_pretty(&val).map_err(|e| e.to_string())
}

/// Write a credential JSON back to the macOS keychain (`security
/// add-generic-password -U` upserts the Claude Code item). Returns false on
/// any failure so the caller falls back to the file-based credential.
#[cfg(target_os = "macos")]
fn write_keychain_credentials(json: &str) -> bool {
    use std::process::{Command, Stdio};

    let user = std::env::var("USER")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "claude".to_string());
    Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            CLAUDE_KEYCHAIN_SERVICE,
            "-a",
            &user,
            "-w",
            json,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn write_keychain_credentials(_json: &str) -> bool {
    false
}

/// Persist an OAuth credential JSON so Claude Code picks it up: macOS keychain
/// first, atomically-written `.credentials.json` (mode 0600) otherwise. This
/// is the only place Agent Hub ever *writes* Claude OAuth credentials.
fn write_claude_oauth_credentials(json: &str) -> Result<(), String> {
    // Light validation: never write back something Claude Code cannot parse.
    parse_claude_credentials(json)?;

    if write_keychain_credentials(json) {
        return Ok(());
    }

    let dir = claude_config_dir().ok_or_else(|| "无法确定用户主目录".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(".credentials.json");
    let tmp = dir.join(".credentials.json.tmp");
    std::fs::write(&tmp, json.as_bytes()).map_err(|e| format!("写入 Claude 凭证失败: {e}"))?;
    crate::paths::replace_file(&tmp, &path).map_err(|e| format!("写入 Claude 凭证失败: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Normalize a `utilization` value to a 0–100 integer percent. The API emits
/// 0–100 floats; values ≤ 1 are defensively treated as a 0–1 ratio.
fn claude_utilization_percent(node: Option<&serde_json::Value>) -> Option<u8> {
    let raw = node?.as_f64()?;
    let percent = if raw <= 1.0 { raw * 100.0 } else { raw };
    Some(percent.round().clamp(0.0, 100.0) as u8)
}

/// Parse an ISO-8601 `resets_at` timestamp into unix seconds.
fn claude_reset_unix(node: Option<&serde_json::Value>) -> Option<u64> {
    let text = node?.as_str()?;
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .and_then(|date| u64::try_from(date.timestamp()).ok())
}

/// Build a usage window from a `{utilization, resets_at}` node. Returns `None`
/// for absent/null windows (e.g. plans without a weekly limit).
fn claude_build_window(
    node: Option<&serde_json::Value>,
    window_seconds: u64,
    now: u64,
) -> Option<UsageWindow> {
    let obj = node?.as_object()?;
    let used_percent = claude_utilization_percent(obj.get("utilization"))?;
    let reset_at = claude_reset_unix(obj.get("resets_at")).unwrap_or(0);
    Some(UsageWindow {
        used_percent,
        remaining_percent: 100u8.saturating_sub(used_percent),
        reset_after_seconds: reset_at.saturating_sub(now),
        reset_at,
        window_seconds,
    })
}

fn map_claude_usage(
    raw: &serde_json::Value,
    account_name: Option<String>,
    plan_type: String,
    fetched_at: u64,
) -> Result<ClaudeUsageResponse, String> {
    let now = u64::try_from(Utc::now().timestamp()).unwrap_or(0);
    // five_hour → 5h window, seven_day → weekly window. Other windows
    // (seven_day_sonnet, seven_day_opus, …) are intentionally ignored.
    let window_5h = claude_build_window(raw.get("five_hour"), 18_000, now);
    let window_weekly = claude_build_window(raw.get("seven_day"), 604_800, now);

    let mut windows: Vec<UsageWindow> = Vec::new();
    if let Some(ref w) = window_5h {
        windows.push(w.clone());
    }
    if let Some(ref w) = window_weekly {
        windows.push(w.clone());
    }
    windows.sort_by_key(|w| w.window_seconds);
    windows.dedup_by_key(|w| w.window_seconds);

    if windows.is_empty() {
        return Err("Claude 用量响应中没有可识别的窗口数据".to_string());
    }

    Ok(ClaudeUsageResponse {
        account_name,
        plan_type,
        window_5h,
        window_weekly,
        usage_windows: windows,
        fetched_at,
    })
}

async fn fetch_claude_usage() -> Result<ClaudeUsageResponse, String> {
    let oauth = resolve_claude_oauth()?;

    // Read-only token policy: never refresh. When the access token has
    // expired, Claude Code itself refreshes it the next time it runs.
    if oauth.expires_at_ms > 0 {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if oauth.expires_at_ms <= now_ms {
            return Err("登录态已过期，请打开一次 Claude Code 刷新后重试".to_string());
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("构建 Claude 用量请求失败: {e}"))?;

    let response = client
        .get(CLAUDE_USAGE_URL)
        .header("Authorization", format!("Bearer {}", oauth.access_token))
        .header("anthropic-beta", "oauth-2025-04-20")
        // The endpoint 401s without a Claude Code user-agent.
        .header("User-Agent", "claude-code/2.1.80")
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("请求 Claude 用量接口失败: {e}"))?;
    let status = response.status();

    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err("Claude 用量接口请求过于频繁（HTTP 429），请稍后再试".to_string());
    }
    if !status.is_success() {
        return Err(format!("Claude 用量接口返回错误: HTTP {status}"));
    }

    let raw: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 Claude 用量响应失败: {e}"))?;

    map_claude_usage(
        &raw,
        oauth.account_name,
        oauth
            .subscription_type
            .unwrap_or_else(|| "unknown".to_string()),
        u64::try_from(Utc::now().timestamp()).unwrap_or(0),
    )
}

/// Claude Code usage from the official OAuth login. Shared by the Accounts
/// view and tray popup.
///
/// When `force` is false/omitted, returns the in-memory cache if it is younger
/// than 10 minutes. Manual refresh buttons should pass `force: true`.
#[tauri::command]
pub async fn get_claude_usage(force: Option<bool>) -> Result<ClaudeUsageResponse, String> {
    let force = force.unwrap_or(false);
    let now = usage_unix_now();
    if !force {
        if let Ok(guard) = CLAUDE_USAGE_CACHE.lock() {
            if let Some(entry) = guard.as_ref() {
                if usage_cache_is_fresh(entry.fetched_at, now) {
                    let mut usage = entry.data.clone();
                    apply_claude_usage_timers(&mut usage, now);
                    return Ok(usage);
                }
            }
        }
    }

    let usage = fetch_claude_usage().await?;
    if let Ok(mut guard) = CLAUDE_USAGE_CACHE.lock() {
        *guard = Some(UsageCacheEntry {
            fetched_at: usage.fetched_at.max(now),
            data: usage.clone(),
        });
    }
    Ok(usage)
}

async fn fetch_codex_tray_snapshot() -> Result<CodexTraySnapshot, String> {
    let (usage_result, credits_result) =
        futures_util::future::join(get_codex_usage(), get_codex_reset_credits()).await;
    let usage = usage_result?;
    let now = usage_unix_now();
    Ok(CodexTraySnapshot {
        usage,
        reset_credits: credits_result.ok(),
        last_query_at: now,
    })
}

/// Shared Codex usage snapshot for the Accounts view and tray popup.
///
/// When `force` is false/omitted, returns the in-memory cache if it is younger
/// than 10 minutes. Manual refresh buttons should pass `force: true`.
#[tauri::command]
pub async fn get_codex_tray_usage(force: Option<bool>) -> Result<CodexTraySnapshot, String> {
    let force = force.unwrap_or(false);
    let now = usage_unix_now();
    if !force {
        if let Ok(guard) = CODEX_USAGE_CACHE.lock() {
            if let Some(entry) = guard.as_ref() {
                if usage_cache_is_fresh(entry.fetched_at, now) {
                    let mut snapshot = entry.data.clone();
                    apply_codex_snapshot_timers(&mut snapshot, now);
                    return Ok(snapshot);
                }
            }
        }
    }

    let snapshot = fetch_codex_tray_snapshot().await?;
    if let Ok(mut guard) = CODEX_USAGE_CACHE.lock() {
        *guard = Some(UsageCacheEntry {
            fetched_at: snapshot.last_query_at,
            data: snapshot.clone(),
        });
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal JWT: header.payload.signature, where the payload is
    // base64url-no-pad of {"email":"user@example.com","name":"Test User"}.
    fn jwt(email: &str) -> String {
        let payload = format!(r#"{{"email":"{email}","name":"Test User"}}"#);
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload);
        format!("header.{b64}.signature")
    }

    #[test]
    fn decode_jwt_payload_reads_email() {
        let tok = jwt("someone@gmail.com");
        let p = decode_jwt_payload(&tok).expect("payload decodes");
        assert_eq!(p["email"].as_str(), Some("someone@gmail.com"));
        assert_eq!(p["name"].as_str(), Some("Test User"));
    }

    #[test]
    fn decode_jwt_payload_rejects_garbage() {
        assert!(decode_jwt_payload("not-a-jwt").is_none());
        assert!(decode_jwt_payload("only.onepart").is_none());
        assert!(decode_jwt_payload("").is_none());
    }

    #[test]
    fn collect_usage_windows_keeps_5h_7d_and_30d_when_present() {
        let window = |used_percent: u64, seconds: u64| {
            serde_json::json!({
                "used_percent": used_percent,
                "reset_after_seconds": seconds / 2,
                "reset_at": 123,
                "limit_window_seconds": seconds
            })
        };
        let rate = serde_json::json!({
            "primary_window": window(39, 18_000),
            "secondary_window": window(61, 604_800),
            "additional_windows": [window(12, 2_592_000)],
            "ignored": { "used_percent": 99 }
        });

        let windows = collect_usage_windows(&rate);
        assert_eq!(windows.len(), 3);
        assert_eq!(
            windows
                .iter()
                .map(|window| window.window_seconds)
                .collect::<Vec<_>>(),
            vec![18_000, 604_800, 2_592_000]
        );
    }

    #[test]
    fn map_grok_usage_reads_weekly_billing_payload() {
        // Shape returned by /v1/billing?format=credits (and CLI logs).
        let raw = serde_json::json!({
            "config": {
                "creditUsagePercent": 4.0,
                "currentPeriod": {
                    "type": "USAGE_PERIOD_TYPE_WEEKLY",
                    "start": "2099-07-10T00:00:00Z",
                    "end": "2099-07-17T00:00:00Z"
                },
                "prepaidBalance": { "val": 0 },
                "onDemandCap": { "val": 0 },
                "onDemandUsed": { "val": 0 },
                "isUnifiedBillingUser": true,
                "billingPeriodStart": "2099-07-10T00:00:00Z",
                "billingPeriodEnd": "2099-07-17T00:00:00Z"
            },
            "subscriptionTier": "SuperGrok"
        });

        let usage = map_grok_usage(&raw, Some("user@example.com".to_string()), 1)
            .expect("maps Grok credits billing response");
        assert_eq!(usage.plan_type, "SuperGrok");
        assert_eq!(usage.period_type, "weekly");
        assert_eq!(usage.usage_window.used_percent, 4);
        assert_eq!(usage.usage_window.remaining_percent, 96);
        assert_eq!(usage.usage_window.window_seconds, 604_800);
        assert_eq!(usage.used_value, None);
        assert_eq!(usage.limit_value, None);
        assert_eq!(usage.prepaid_balance, Some(0.0));
        assert_eq!(usage.on_demand_cap, Some(0.0));
    }

    #[test]
    fn usage_cache_ttl_is_ten_minutes() {
        assert_eq!(USAGE_CACHE_TTL_SECS, 600);
        let t0 = 1_000_000u64;
        assert!(usage_cache_is_fresh(t0, t0 + 599));
        assert!(!usage_cache_is_fresh(t0, t0 + 600));
    }

    #[test]
    fn refresh_usage_window_timers_uses_reset_at() {
        let mut window = UsageWindow {
            used_percent: 10,
            remaining_percent: 90,
            reset_after_seconds: 9999,
            reset_at: 1_000_500,
            window_seconds: 18_000,
        };
        refresh_usage_window_timers(&mut window, 1_000_000);
        assert_eq!(window.reset_after_seconds, 500);
    }

    #[test]
    fn map_grok_usage_reads_legacy_monthly_billing_payload() {
        // Bare /v1/billing still returns this monthly-credit shape; keep
        // parsing support so older live payloads do not break the UI.
        let raw = serde_json::json!({
            "config": {
                "monthlyLimit": { "val": 15000 },
                "used": { "val": 728 },
                "onDemandCap": { "val": 0 },
                "billingPeriodStart": "2099-07-01T00:00:00+00:00",
                "billingPeriodEnd": "2099-08-01T00:00:00+00:00"
            }
        });

        let usage =
            map_grok_usage(&raw, None, 1).expect("maps legacy Grok monthly billing response");
        assert_eq!(usage.plan_type, "Grok");
        assert_eq!(usage.period_type, "monthly");
        assert_eq!(usage.limit_value, Some(15_000.0));
        assert_eq!(usage.used_value, Some(728.0));
        assert_eq!(usage.usage_window.used_percent, 5);
        assert_eq!(usage.usage_window.remaining_percent, 95);
        assert_eq!(usage.usage_window.window_seconds, 2_678_400);
    }

    const CODEX_AUTH: &str = r#"{
        "tokens": {
            "id_token": "PLACEHOLDER",
            "access_token": "sk-1234567890abcdef",
            "refresh_token": "rt",
            "account_id": "acct-abc-123"
        },
        "account_id": "acct-abc-123"
    }"#;

    #[test]
    fn codex_identity_uses_account_id() {
        let id = extract_account_identity("codex", CODEX_AUTH);
        assert_eq!(id.as_deref(), Some("acct-abc-123"));
    }

    #[test]
    fn codex_profile_pool_is_disabled() {
        assert_eq!(agent_slug("codex").unwrap_err(), "unknown_agent_type:codex");
    }

    #[test]
    fn codex_identity_prefers_tokens_account_id() {
        let content = r#"{"tokens":{"account_id":"from-tokens"},"account_id":"from-top"}"#;
        let id = extract_account_identity("codex", content);
        assert_eq!(id.as_deref(), Some("from-tokens"));
    }

    #[test]
    fn codex_identity_falls_back_to_top_level_account_id() {
        let content = r#"{"account_id":"from-top"}"#;
        let id = extract_account_identity("codex", content);
        assert_eq!(id.as_deref(), Some("from-top"));
    }

    #[test]
    fn codex_name_reads_email_from_id_token() {
        let content = CODEX_AUTH.replace("PLACEHOLDER", &jwt("worker@corp.com"));
        let name = extract_account_name("codex", &content);
        assert_eq!(name.as_deref(), Some("worker@corp.com"));
    }

    #[test]
    fn codex_name_none_without_id_token() {
        let content = r#"{"tokens":{"account_id":"x"},"account_id":"x"}"#;
        assert!(extract_account_name("codex", content).is_none());
    }

    #[test]
    fn claude_identity_is_the_auth_token() {
        let content = r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-ant-xyz123"}}"#;
        let id = extract_account_identity("claude-code", content);
        assert_eq!(id.as_deref(), Some("sk-ant-xyz123"));
        assert_eq!(
            extract_key("claude-code", content).as_deref(),
            Some("sk-ant-xyz123")
        );
    }

    #[test]
    fn claude_identity_ignores_api_key_only_configs() {
        let content = r#"{"env":{"ANTHROPIC_API_KEY":"sk-ant-xyz123"}}"#;
        assert!(extract_account_identity("claude-code", content).is_none());
        assert!(extract_key("claude-code", content).is_none());
    }

    #[test]
    fn claude_name_is_never_derived() {
        let content = r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-ant-xyz123"}}"#;
        assert!(extract_account_name("claude-code", content).is_none());
    }

    #[test]
    fn extract_identity_invalid_json_returns_none() {
        assert!(extract_account_identity("codex", "not json").is_none());
        assert!(extract_account_identity("claude-code", "not json").is_none());
    }

    // --- Kimi config.toml + window-unit parsing ---

    #[test]
    fn kimi_config_toml_reads_first_provider_api_key() {
        let dir =
            std::env::temp_dir().join(format!("agent-hub-kimi-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            r#"
default_model = "kimi-for-coding/k3"

[providers.kimi-for-coding]
type = "anthropic"
api_key = "sk-kimi-test-abc123"
base_url = "https://api.kimi.com/coding"
"#,
        )
        .unwrap();

        let cred = read_kimi_credential_config(&dir).expect("should parse config.toml");
        assert_eq!(cred.api_key, "sk-kimi-test-abc123");
        assert!(cred.is_usable());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn kimi_config_toml_returns_none_without_api_key() {
        let dir =
            std::env::temp_dir().join(format!("agent-hub-kimi-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            r#"
[providers.kimi-for-coding]
type = "anthropic"
base_url = "https://api.kimi.com/coding"
"#,
        )
        .unwrap();

        assert!(read_kimi_credential_config(&dir).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn kimi_config_toml_skips_providers_without_api_keys() {
        let dir =
            std::env::temp_dir().join(format!("agent-hub-kimi-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            r#"
[providers.other]
type = "openai"

[providers.kimi-for-coding]
type = "anthropic"
api_key = "sk-kimi-test-abc123"
"#,
        )
        .unwrap();

        let cred = read_kimi_credential_config(&dir).expect("should find a later provider key");
        assert_eq!(cred.api_key, "sk-kimi-test-abc123");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Both plain (`"MINUTE"`) and enum-prefixed (`"TIME_UNIT_MINUTE"`) units
    /// must normalise to the same multiplier. The live API emits the enum form.
    #[test]
    fn kimi_window_seconds_handles_enum_prefixed_units() {
        let enum_form = serde_json::json!({"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"});
        let plain_form = serde_json::json!({"duration": 300, "timeUnit": "MINUTE"});
        let hour_form = serde_json::json!({"duration": 5, "timeUnit": "TIME_UNIT_HOUR"});

        assert_eq!(kimi_window_seconds(Some(&enum_form)), Some(300 * 60)); // 5h
        assert_eq!(kimi_window_seconds(Some(&plain_form)), Some(300 * 60));
        assert_eq!(kimi_window_seconds(Some(&hour_form)), Some(5 * 3600)); // 5h
    }

    #[test]
    fn kimi_map_usage_extracts_weekly_and_5h_windows() {
        let now: u64 = 1_700_000_000;
        let raw = serde_json::json!({
            "user": {"membership": {"level": "LEVEL_INTERMEDIATE"}},
            "usage": {
                "limit": "100",
                "used": "43",
                "remaining": "57",
                "resetTime": "2026-07-24T07:52:20.292236Z"
            },
            "limits": [{
                "window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                "detail": {
                    "limit": "100",
                    "used": "5",
                    "remaining": "95",
                    "resetTime": "2026-07-22T02:52:20.292236Z"
                }
            }]
        });

        let resp = map_kimi_usage(&raw, None, now).expect("should map");
        assert_eq!(resp.auth_method, "METHOD_API_KEY");
        assert_eq!(resp.usage_windows.len(), 2);
        // Sorted ascending: 5h (18000) before weekly (604800).
        assert_eq!(resp.usage_windows[0].window_seconds, 18_000);
        assert_eq!(resp.usage_windows[0].used_percent, 5);
        assert_eq!(resp.usage_windows[1].window_seconds, 604_800);
        assert_eq!(resp.usage_windows[1].used_percent, 43);
        // Named windows mirror the array.
        let w5 = resp.window_5h.expect("5h window should be populated");
        assert_eq!(w5.window_seconds, 18_000);
        assert_eq!(w5.used_percent, 5);
        let ww = resp
            .window_weekly
            .expect("weekly window should be populated");
        assert_eq!(ww.window_seconds, 604_800);
        assert_eq!(ww.used_percent, 43);
        assert_eq!(resp.weekly_limit, Some(100));
        assert_eq!(resp.weekly_used, Some(43));
    }

    /// Live end-to-end check against the real `~/.kimi-code/config.toml` login.
    /// Gated behind `#[ignore]` so CI doesn't run it; invoke locally with
    /// `cargo test kimi_live_usage_query -- --ignored --nocapture` after signing
    /// in via `kimi`. Verifies the full resolve-credential → GET → map chain.
    #[test]
    #[ignore]
    fn kimi_live_usage_query() {
        let cred = resolve_kimi_credential().expect("should find a Kimi credential");
        assert!(
            cred.api_key.starts_with("sk-"),
            "expected the config.toml API key to start with sk-"
        );

        let usage = tauri::async_runtime::block_on(get_kimi_usage(Some(true)))
            .expect("live usage query should succeed with config.toml credentials");
        assert!(
            !usage.usage_windows.is_empty(),
            "should return at least one window"
        );
        println!(
            "auth = {}, 5h = {:?}, weekly = {:?}",
            usage.auth_method,
            usage
                .window_5h
                .as_ref()
                .map(|w| (w.used_percent, w.remaining_percent)),
            usage
                .window_weekly
                .as_ref()
                .map(|w| (w.used_percent, w.remaining_percent)),
        );
    }

    // --- Claude Code OAuth credentials + usage mapping ---

    const CLAUDE_CREDENTIALS: &str = r#"{
        "claudeAiOauth": {
            "accessToken": "sk-ant-oat01-test-token",
            "refreshToken": "sk-ant-ort01-test-refresh",
            "expiresAt": 1893456000000,
            "scopes": ["user:inference", "user:profile"],
            "subscriptionType": "max"
        }
    }"#;

    #[test]
    fn claude_credentials_parse_nested_oauth_block() {
        let (token, expires_at_ms, subscription) =
            parse_claude_credentials(CLAUDE_CREDENTIALS).expect("should parse");
        assert_eq!(token, "sk-ant-oat01-test-token");
        assert_eq!(expires_at_ms, 1_893_456_000_000);
        assert_eq!(subscription.as_deref(), Some("max"));
    }

    #[test]
    fn claude_credentials_missing_fields_error() {
        // Missing the claudeAiOauth wrapper entirely.
        assert!(parse_claude_credentials(r#"{"other": true}"#).is_err());
        // Wrapper present but no accessToken.
        assert!(parse_claude_credentials(r#"{"claudeAiOauth": {"expiresAt": 1}}"#).is_err());
        // Empty accessToken is rejected too.
        assert!(parse_claude_credentials(r#"{"claudeAiOauth": {"accessToken": "  "}}"#).is_err());
        // Not JSON at all.
        assert!(parse_claude_credentials("not json").is_err());
    }

    #[test]
    fn claude_utilization_accepts_ratio_and_percent() {
        // 0–1 ratio form is scaled to percent.
        assert_eq!(
            claude_utilization_percent(Some(&serde_json::json!(0.25))),
            Some(25)
        );
        // 0–100 float form is used as-is.
        assert_eq!(
            claude_utilization_percent(Some(&serde_json::json!(25.0))),
            Some(25)
        );
        assert_eq!(
            claude_utilization_percent(Some(&serde_json::json!(99.6))),
            Some(100)
        );
        // Non-numeric nodes yield None.
        assert!(claude_utilization_percent(Some(&serde_json::json!("25"))).is_none());
        assert!(claude_utilization_percent(None).is_none());
    }

    #[test]
    fn claude_reset_unix_parses_iso8601() {
        let ts = claude_reset_unix(Some(&serde_json::json!("2030-01-01T00:00:00Z")))
            .expect("should parse ISO-8601");
        assert_eq!(ts, 1_893_456_000);
        // Fractional seconds and offsets are accepted too.
        let with_fraction =
            claude_reset_unix(Some(&serde_json::json!("2030-01-01T00:00:00.123456+00:00")));
        assert_eq!(with_fraction, Some(1_893_456_000));
        assert!(claude_reset_unix(Some(&serde_json::json!("not a date"))).is_none());
        assert!(claude_reset_unix(Some(&serde_json::json!(123))).is_none());
    }

    #[test]
    fn claude_map_usage_builds_5h_and_weekly_windows() {
        let raw = serde_json::json!({
            "five_hour": {"utilization": 25.0, "resets_at": "2030-01-01T00:00:00Z"},
            "seven_day": {"utilization": 0.5, "resets_at": "2030-01-08T00:00:00Z"},
            "seven_day_sonnet": {"utilization": 10.0, "resets_at": "2030-01-08T00:00:00Z"}
        });

        let resp = map_claude_usage(&raw, Some("user@example.com".to_string()), "max".into(), 1)
            .expect("should map");
        assert_eq!(resp.plan_type, "max");
        assert_eq!(resp.account_name.as_deref(), Some("user@example.com"));
        assert_eq!(resp.usage_windows.len(), 2);
        // Sorted ascending: 5h before weekly; sonnet window ignored.
        assert_eq!(resp.usage_windows[0].window_seconds, 18_000);
        assert_eq!(resp.usage_windows[0].used_percent, 25);
        assert_eq!(resp.usage_windows[0].remaining_percent, 75);
        assert_eq!(resp.usage_windows[0].reset_at, 1_893_456_000);
        assert_eq!(resp.usage_windows[1].window_seconds, 604_800);
        // Ratio form 0.5 → 50%.
        assert_eq!(resp.usage_windows[1].used_percent, 50);
        assert!(resp.window_5h.is_some());
        assert!(resp.window_weekly.is_some());
    }

    #[test]
    fn claude_map_usage_skips_null_windows() {
        let raw = serde_json::json!({
            "five_hour": {"utilization": 10.0, "resets_at": "2030-01-01T00:00:00Z"},
            "seven_day": null
        });
        let resp = map_claude_usage(&raw, None, "pro".into(), 1).expect("should map");
        assert!(resp.window_5h.is_some());
        assert!(resp.window_weekly.is_none());
        assert_eq!(resp.usage_windows.len(), 1);

        // No usable windows at all is an error.
        let empty = serde_json::json!({"five_hour": null, "seven_day": null});
        assert!(map_claude_usage(&empty, None, "pro".into(), 1).is_err());
    }

    // --- Claude Code switch: profile kind + env token removal ---

    #[test]
    fn profile_meta_defaults_kind_to_token() {
        // Profiles saved before OAuth support have no kind field.
        let meta: ProfileMeta =
            serde_json::from_str(r#"{"id":"abc","note":"work","saved_at":"2026-01-01T00:00:00Z"}"#)
                .expect("old meta.json should still parse");
        assert_eq!(meta.kind, "token");
        assert_eq!(meta.identity, None);

        let oauth_meta: ProfileMeta = serde_json::from_str(
            r#"{"id":"abc","note":"me@example.com","saved_at":"2026-01-01T00:00:00Z","kind":"oauth","identity":"uuid-1"}"#,
        )
        .expect("oauth meta should parse");
        assert_eq!(oauth_meta.kind, "oauth");
        assert_eq!(oauth_meta.identity.as_deref(), Some("uuid-1"));
    }

    #[test]
    fn oauth_profile_content_has_no_token_identity() {
        // A stored OAuth credential JSON must not be mistaken for a token
        // profile: identity comes from meta.json instead.
        assert!(extract_account_identity("claude-code", CLAUDE_CREDENTIALS).is_none());
        assert!(extract_key("claude-code", CLAUDE_CREDENTIALS).is_none());
    }

    #[test]
    fn remove_claude_env_token_preserves_other_fields() {
        let settings = r#"{
            "model": "opus",
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "sk-ant-xyz123",
                "ANTHROPIC_BASE_URL": "https://proxy.example.com"
            },
            "hooks": {"stop": []}
        }"#;
        let stripped = remove_claude_env_token(settings).expect("should strip");
        let val: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert!(val
            .get("env")
            .and_then(|e| e.get("ANTHROPIC_AUTH_TOKEN"))
            .is_none());
        assert_eq!(
            val.get("env")
                .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
                .and_then(|v| v.as_str()),
            Some("https://proxy.example.com")
        );
        assert_eq!(val.get("model").and_then(|v| v.as_str()), Some("opus"));
        assert!(val.get("hooks").is_some());
    }

    #[test]
    fn remove_claude_env_token_drops_empty_env() {
        let settings = r#"{"env": {"ANTHROPIC_AUTH_TOKEN": "sk-ant-xyz123"}}"#;
        let stripped = remove_claude_env_token(settings).expect("should strip");
        let val: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert!(val.get("env").is_none());
    }

    #[test]
    fn remove_claude_env_token_noop_without_token() {
        let settings = r#"{"model": "opus"}"#;
        let stripped = remove_claude_env_token(settings).expect("should strip");
        let val: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(val.get("model").and_then(|v| v.as_str()), Some("opus"));
        assert!(remove_claude_env_token("not json").is_err());
    }
}

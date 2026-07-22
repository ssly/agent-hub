use std::path::PathBuf;

use base64::Engine as _;
use chrono::Utc;
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
    let active_identity = live_content
        .as_ref()
        .and_then(|c| extract_account_identity(&agent_type, c));
    let current_key = live_content
        .as_ref()
        .and_then(|c| extract_key(&agent_type, c))
        .map(|k| mask_key(&k));

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
            raw.push(RawProfile {
                identity: extract_account_identity(&agent_type, &content),
                key: extract_key(&agent_type, &content).map(|k| mask_key(&k)),
                meta,
            });
        }
    }

    // Auto-save: if a live account exists but no saved profile shares its
    // stable identity, persist it so the current account always appears in the
    // list (and gets selected). This never duplicates a refresh-stale snapshot
    // because the comparison is by identity, not by secret/hash.
    if let (Some(identity), Some(_)) = (&active_identity, &live_content) {
        let already_saved = raw.iter().any(|r| r.identity.as_deref() == Some(identity));
        if !already_saved {
            let note = extract_account_name(&agent_type, live_content.as_ref().unwrap())
                .unwrap_or_default();
            if let Ok(id) = save_current_auth_profile_inner(&agent_type, note.clone(), true) {
                raw.push(RawProfile {
                    identity: active_identity.clone(),
                    key: current_key.clone(),
                    meta: ProfileMeta {
                        id,
                        note,
                        saved_at: now_iso(),
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

/// Delete the live auth file (e.g. ~/.codex/auth.json) **without** backing it
/// up to the account pool first. Used by the "Clear Account" button so the user
/// can sign out of {agent} while keeping every saved profile in the pool intact.
///
/// Unlike `clear_active_auth`, this never touches `~/.agent-hub/switch/<agent>/`.
#[tauri::command]
pub fn delete_active_auth(agent_type: String) -> Result<(), String> {
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

const GROK_BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing";

#[derive(serde::Serialize, Debug, Clone)]
pub struct GrokUsageResponse {
    /// Display label derived from the current Grok CLI login when available.
    pub account_name: Option<String>,
    pub plan_type: String,
    /// `monthly` for current Grok Build plans, `weekly` for older SuperGrok payloads.
    pub period_type: String,
    pub usage_window: UsageWindow,
    /// Raw credit values when the billing endpoint provides them.
    pub limit_value: Option<f64>,
    pub used_value: Option<f64>,
    pub prepaid_balance: Option<f64>,
    pub on_demand_cap: Option<f64>,
    pub on_demand_used: Option<f64>,
    pub on_demand_enabled: Option<bool>,
    /// `live` comes from /v1/billing; `cache` comes from Grok's own structured log.
    pub source: String,
    pub fetched_at: u64,
    /// Cached data is stale when its billing period has already ended.
    pub stale: bool,
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
    let kimi_code = resolve_kimi_auth().is_ok();

    UsageProviderAvailability {
        codex,
        grok_build,
        kimi_code,
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
    source: &str,
    fetched_at: u64,
) -> Result<GrokUsageResponse, String> {
    // Live responses expose the config directly. Structured logs wrap it in
    // ctx.config, and some CLI builds use a top-level config wrapper.
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
        source: source.to_string(),
        fetched_at,
        stale: source == "cache" && reset_at <= now,
    })
}

fn read_cached_grok_usage(
    grok_home: &std::path::Path,
    account_name: Option<String>,
) -> Result<GrokUsageResponse, String> {
    let log_path = grok_home.join("logs/unified.jsonl");
    let content =
        std::fs::read_to_string(&log_path).map_err(|e| format!("读取 Grok 用量缓存失败: {e}"))?;

    for line in content.lines().rev() {
        let Ok(raw) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if raw.get("msg").and_then(|value| value.as_str())
            != Some("billing: fetched credits config")
        {
            continue;
        }
        let fetched_at =
            json_timestamp(raw.get("ts").or_else(|| raw.get("timestamp"))).unwrap_or_default();
        if let Ok(usage) = map_grok_usage(&raw, account_name.clone(), "cache", fetched_at) {
            return Ok(usage);
        }
    }

    Err("Grok 日志中没有可用的额度缓存".to_string())
}

#[tauri::command]
pub async fn get_grok_usage() -> Result<GrokUsageResponse, String> {
    let grok_home = grok_home_dir()?;
    let auth = resolve_grok_auth(&grok_home);
    let account_name = auth
        .as_ref()
        .ok()
        .and_then(|value| value.account_name.clone());

    let live_result = match auth {
        Ok(auth) => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .map_err(|e| e.to_string())?;
            match client
                .get(GROK_BILLING_URL)
                .header("Authorization", format!("Bearer {}", auth.bearer))
                .header("Accept", "application/json")
                .header("User-Agent", "Grok CLI")
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    match response.json::<serde_json::Value>().await {
                        Ok(raw) => map_grok_usage(
                            &raw,
                            auth.account_name,
                            "live",
                            u64::try_from(Utc::now().timestamp()).unwrap_or(0),
                        ),
                        Err(error) => Err(format!("解析 Grok 用量响应失败: {error}")),
                    }
                }
                Ok(response) => Err(format!("Grok 用量接口返回错误: HTTP {}", response.status())),
                Err(error) => Err(format!("请求 Grok 用量接口失败: {error}")),
            }
        }
        Err(error) => Err(error),
    };

    match live_result {
        Ok(usage) => Ok(usage),
        Err(live_error) => read_cached_grok_usage(&grok_home, account_name)
            .map_err(|cache_error| format!("{live_error}；{cache_error}")),
    }
}

// --- Kimi Code quota via the Kimi CLI usages endpoint -----------------------

const KIMI_USAGES_URL: &str = "https://api.kimi.com/coding/v1/usages";
const KIMI_TOKEN_URL: &str = "https://auth.kimi.com/api/oauth/token";
const KIMI_OAUTH_CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";
const KIMI_KEYCHAIN_SERVICE: &str = "kimi-code";
const KIMI_KEYCHAIN_KEY: &str = "oauth/kimi-code";

#[derive(serde::Serialize, Debug, Clone)]
pub struct KimiUsageResponse {
    /// Display label derived from the JWT email claim or credential file name.
    pub account_name: Option<String>,
    /// Membership tier reported by the usages endpoint, falls back to "Kimi Code".
    pub plan_type: String,
    /// Rolling windows (typically 5h primary + weekly). Sorted ascending.
    pub usage_windows: Vec<UsageWindow>,
    /// Raw weekly limit/used values when the endpoint exposes them.
    pub limit_value: Option<f64>,
    pub used_value: Option<f64>,
    /// Only the live endpoint is queried for Kimi.
    pub source: String,
    pub fetched_at: u64,
}

/// The shape of the Kimi OAuth credential file at
/// `~/.kimi/credentials/kimi-code.json` and the OS keychain entry.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
struct KimiCredentialRaw {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_type: Option<String>,
}

impl KimiCredentialRaw {
    fn is_usable(&self) -> bool {
        !self.access_token.trim().is_empty()
    }

    /// `expires_at` shows up as a unix timestamp, an ISO string, or is missing
    /// entirely. Normalise to unix seconds when possible.
    fn expires_unix(&self) -> Option<u64> {
        let value = self.expires_at.as_ref()?;
        if let Some(n) = value.as_u64() {
            return Some(n);
        }
        if let Some(n) = value.as_i64() {
            return u64::try_from(n).ok();
        }
        value.as_str().and_then(|text| {
            text.parse::<u64>()
                .ok()
                .or_else(|| {
                    chrono::DateTime::parse_from_rfc3339(text)
                        .ok()
                        .and_then(|dt| u64::try_from(dt.timestamp()).ok())
                })
        })
    }
}

struct KimiAuth {
    access_token: String,
    /// None when the source is a long-lived API key (no refresh flow).
    refresh_token: Option<String>,
    expires_at: Option<u64>,
    account_name: Option<String>,
}

/// Where the credential came from so we can write refreshed tokens back home.
#[derive(Clone, Copy, PartialEq)]
enum KimiCredentialSource {
    Keychain,
    File,
}

struct KimiResolvedCredential {
    raw: KimiCredentialRaw,
    source: KimiCredentialSource,
}

fn kimi_home_dir() -> Result<PathBuf, String> {
    match std::env::var("KIMI_CODE_HOME") {
        Ok(dir) => Ok(PathBuf::from(dir)),
        Err(_) => dirs::home_dir()
            .map(|home| home.join(".kimi"))
            .ok_or_else(|| "无法确定用户主目录".to_string()),
    }
}

/// Decode the JWT payload (without signature verification) and pull the email
/// claim out for display. Kimi access tokens carry the user email there.
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

fn read_kimi_credential_file(home: &std::path::Path) -> Option<KimiCredentialRaw> {
    let path = home.join("credentials/kimi-code.json");
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str::<KimiCredentialRaw>(&content).ok()
}

fn read_kimi_credential_keychain() -> Option<KimiCredentialRaw> {
    let entry = keyring::Entry::new(KIMI_KEYCHAIN_SERVICE, KIMI_KEYCHAIN_KEY).ok()?;
    let value = entry.get_password().ok()?;
    serde_json::from_str::<KimiCredentialRaw>(&value).ok()
}

/// Try the OS keychain first (Kimi CLI default), fall back to the credential
/// file. We never rewrite, refresh, or switch these credentials from the
/// Accounts view — only the usage endpoint consumes them.
fn resolve_kimi_credential() -> Result<KimiResolvedCredential, String> {
    if let Some(raw) = read_kimi_credential_keychain().filter(KimiCredentialRaw::is_usable) {
        return Ok(KimiResolvedCredential {
            raw,
            source: KimiCredentialSource::Keychain,
        });
    }
    let home = kimi_home_dir()?;
    if let Some(raw) = read_kimi_credential_file(&home).filter(KimiCredentialRaw::is_usable) {
        return Ok(KimiResolvedCredential {
            raw,
            source: KimiCredentialSource::File,
        });
    }
    Err(
        "未找到 Kimi Code 凭据（keychain 或 ~/.kimi/credentials/kimi-code.json）。请先在 Kimi CLI 中登录。"
            .to_string(),
    )
}

fn resolve_kimi_auth() -> Result<KimiAuth, String> {
    let resolved = resolve_kimi_credential()?;
    let access_token = resolved.raw.access_token.clone();
    let refresh_token = {
        let trimmed = resolved.raw.refresh_token.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };
    let expires_at = resolved.raw.expires_unix();
    let account_name = kimi_account_name_from_jwt(&access_token);
    Ok(KimiAuth {
        access_token,
        refresh_token,
        expires_at,
        account_name,
    })
}

fn write_kimi_credential(resolved: &KimiResolvedCredential) {
    let serialized = match serde_json::to_string(&resolved.raw) {
        Ok(text) => text,
        Err(_) => return,
    };
    match resolved.source {
        KimiCredentialSource::Keychain => {
            if let Ok(entry) = keyring::Entry::new(KIMI_KEYCHAIN_SERVICE, KIMI_KEYCHAIN_KEY) {
                let _ = entry.set_password(&serialized);
            }
        }
        KimiCredentialSource::File => {
            if let Ok(home) = kimi_home_dir() {
                let path = home.join("credentials/kimi-code.json");
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let tmp = path.with_extension("json.tmp");
                if std::fs::write(&tmp, &serialized).is_ok() {
                    let _ = std::fs::rename(&tmp, &path);
                }
            }
        }
    }
}

/// Exchange a refresh token for a new access token. Public client, no secret.
async fn refresh_kimi_token(
    refresh_token: &str,
    source: KimiCredentialSource,
) -> Result<KimiAuth, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("构建 Kimi OAuth 请求失败: {e}"))?;
    let form = [
        ("grant_type", "refresh_token"),
        ("client_id", KIMI_OAUTH_CLIENT_ID),
        ("refresh_token", refresh_token),
    ];
    let response = client
        .post(KIMI_TOKEN_URL)
        .form(&form)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("请求 Kimi OAuth 刷新失败: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Kimi OAuth 刷新失败: HTTP {status}"));
    }
    let raw: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 Kimi OAuth 响应失败: {e}"))?;
    let access_token = raw
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "Kimi OAuth 响应缺少 access_token".to_string())?;
    let new_refresh = raw
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| refresh_token.to_string());
    let expires_in = raw.get("expires_in").and_then(|v| v.as_u64()).unwrap_or(0);
    let expires_at = if expires_in > 0 {
        Some(u64::try_from(Utc::now().timestamp()).unwrap_or(0) + expires_in)
    } else {
        None
    };

    // Persist refreshed credentials back to the original store so subsequent
    // queries don't trigger another refresh round-trip.
    let mut next_raw = KimiCredentialRaw {
        access_token: access_token.clone(),
        refresh_token: new_refresh.clone(),
        ..Default::default()
    };
    next_raw.scope = Some("kimi-code".to_string());
    next_raw.token_type = Some("Bearer".to_string());
    if let Some(ts) = expires_at {
        next_raw.expires_at = Some(serde_json::Value::from(ts));
    }
    write_kimi_credential(&KimiResolvedCredential {
        raw: next_raw,
        source,
    });

    let account_name = kimi_account_name_from_jwt(&access_token);
    Ok(KimiAuth {
        access_token,
        refresh_token: Some(new_refresh),
        expires_at,
        account_name,
    })
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
/// HOUR/MINUTE/DAY/MONTH are the units observed in the wild.
fn kimi_window_seconds(window: Option<&serde_json::Value>) -> Option<u64> {
    let window = window?;
    let duration = window.get("duration").and_then(|v| v.as_u64())?;
    let unit = window
        .get("timeUnit")
        .or_else(|| window.get("time_unit"))
        .and_then(|v| v.as_str())?;
    let multiplier = match unit.to_ascii_uppercase().as_str() {
        "MINUTE" => 60,
        "HOUR" => 3_600,
        "DAY" => 86_400,
        "MONTH" => 2_592_000,
        _ => return None,
    };
    Some(duration.saturating_mul(multiplier))
}

/// Build a usage window from a `{limit, remaining, resetTime}` detail object.
fn kimi_build_window(
    detail: &serde_json::Value,
    window_seconds: u64,
    now: u64,
) -> Option<UsageWindow> {
    let limit = kimi_quota_value(detail.get("limit"))?;
    let remaining = kimi_quota_value(detail.get("remaining"))?;
    let used = limit.saturating_sub(remaining);
    let used_percent = if limit > 0 {
        ((used as f64 / limit as f64) * 100.0).round().clamp(0.0, 100.0) as u8
    } else {
        0
    };
    let reset_at = kimi_reset_unix(detail.get("resetTime"))
        .or_else(|| kimi_reset_unix(detail.get("reset_at")))
        .unwrap_or(0);
    Some(UsageWindow {
        used_percent,
        remaining_percent: 100u8.saturating_sub(used_percent),
        reset_after_seconds: reset_at.saturating_sub(now),
        reset_at,
        window_seconds,
    })
}

fn map_kimi_usage(
    raw: &serde_json::Value,
    account_name: Option<String>,
    fetched_at: u64,
) -> Result<KimiUsageResponse, String> {
    let now = u64::try_from(Utc::now().timestamp()).unwrap_or(0);
    let mut windows: Vec<UsageWindow> = Vec::new();
    let mut weekly_limit: Option<f64> = None;
    let mut weekly_used: Option<f64> = None;

    // Top-level `usage` is the rolling weekly window.
    if let Some(usage) = raw.get("usage") {
        if let Some(window) = kimi_build_window(usage, 604_800, now) {
            if let (Some(limit), Some(remaining)) = (
                kimi_quota_value(usage.get("limit")),
                kimi_quota_value(usage.get("remaining")),
            ) {
                weekly_limit = Some(limit as f64);
                weekly_used = Some(limit.saturating_sub(remaining) as f64);
            }
            windows.push(window);
        }
    }

    // `limits[]` carries additional windows (typically the 5h rolling window).
    if let Some(limits) = raw.get("limits").and_then(|v| v.as_array()) {
        for entry in limits {
            let window_seconds = kimi_window_seconds(entry.get("window"));
            let detail = entry
                .get("detail")
                .or_else(|| entry.get("rateLimit"))
                .or_else(|| entry.get("rate_limit"));
            if let (Some(seconds), Some(detail)) = (window_seconds, detail) {
                if let Some(window) = kimi_build_window(detail, seconds, now) {
                    windows.push(window);
                }
            }
        }
    }

    windows.sort_by_key(|w| w.window_seconds);
    windows.dedup_by_key(|w| w.window_seconds);

    let plan_type = raw
        .get("user")
        .and_then(|u| u.get("membership"))
        .and_then(|m| m.get("level"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| "Kimi Code".to_string());

    if windows.is_empty() {
        return Err("Kimi 用量响应中没有可识别的窗口数据".to_string());
    }

    Ok(KimiUsageResponse {
        account_name,
        plan_type,
        usage_windows: windows,
        limit_value: weekly_limit,
        used_value: weekly_used,
        source: "live".to_string(),
        fetched_at,
    })
}

#[tauri::command]
pub async fn get_kimi_usage() -> Result<KimiUsageResponse, String> {
    let resolved = resolve_kimi_credential()?;
    let refresh_token = {
        let trimmed = resolved.raw.refresh_token.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };
    let mut auth = KimiAuth {
        access_token: resolved.raw.access_token.clone(),
        refresh_token,
        expires_at: resolved.raw.expires_unix(),
        account_name: kimi_account_name_from_jwt(&resolved.raw.access_token),
    };
    let source = resolved.source;
    let now = u64::try_from(Utc::now().timestamp()).unwrap_or(0);

    // Pre-emptively refresh when the token is within 60s of expiry.
    if let Some(expires_at) = auth.expires_at {
        if expires_at <= now + 60 {
            if let Some(refresh) = auth.refresh_token.clone() {
                if let Ok(refreshed) = refresh_kimi_token(&refresh, source).await {
                    auth = refreshed;
                }
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("构建 Kimi 用量请求失败: {e}"))?;

    async fn fetch_usages(
        client: &reqwest::Client,
        token: &str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        client
            .get(KIMI_USAGES_URL)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json")
            .header("User-Agent", "Kimi CLI")
            .send()
            .await
    }

    let response = fetch_usages(&client, &auth.access_token)
        .await
        .map_err(|e| format!("请求 Kimi 用量接口失败: {e}"))?;
    let status = response.status();

    // On auth failure, attempt a single refresh then retry.
    if status.as_u16() == 401 || status.as_u16() == 403 {
        if let Some(refresh) = auth.refresh_token.clone() {
            if let Ok(refreshed) = refresh_kimi_token(&refresh, source).await {
                auth = refreshed;
                let retry = fetch_usages(&client, &auth.access_token)
                    .await
                    .map_err(|e| format!("请求 Kimi 用量接口失败: {e}"))?;
                if !retry.status().is_success() {
                    return Err(format!(
                        "Kimi 用量接口返回错误: HTTP {}",
                        retry.status()
                    ));
                }
                let raw: serde_json::Value = retry
                    .json()
                    .await
                    .map_err(|e| format!("解析 Kimi 用量响应失败: {e}"))?;
                return map_kimi_usage(
                    &raw,
                    auth.account_name,
                    u64::try_from(Utc::now().timestamp()).unwrap_or(0),
                );
            }
        }
    }

    if !status.is_success() {
        return Err(format!("Kimi 用量接口返回错误: HTTP {status}"));
    }

    let raw: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 Kimi 用量响应失败: {e}"))?;
    map_kimi_usage(
        &raw,
        auth.account_name,
        u64::try_from(Utc::now().timestamp()).unwrap_or(0),
    )
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
        let raw = serde_json::json!({
            "creditUsagePercent": 12.4,
            "currentPeriod": {
                "type": "USAGE_PERIOD_TYPE_WEEKLY",
                "start": "2099-07-10T00:00:00Z",
                "end": "2099-07-17T00:00:00Z"
            },
            "prepaidBalance": { "val": 3.5 },
            "onDemandCap": { "val": "20" },
            "onDemandUsed": { "val": 2 },
            "subscriptionTier": "SuperGrok"
        });

        let usage = map_grok_usage(&raw, Some("user@example.com".to_string()), "live", 1)
            .expect("maps Grok billing response");
        assert_eq!(usage.plan_type, "SuperGrok");
        assert_eq!(usage.period_type, "weekly");
        assert_eq!(usage.usage_window.used_percent, 12);
        assert_eq!(usage.usage_window.remaining_percent, 88);
        assert_eq!(usage.usage_window.window_seconds, 604_800);
        assert_eq!(usage.prepaid_balance, Some(3.5));
        assert_eq!(usage.on_demand_cap, Some(20.0));
        assert!(!usage.stale);
    }

    #[test]
    fn map_grok_usage_reads_current_monthly_billing_payload() {
        let raw = serde_json::json!({
            "config": {
                "monthlyLimit": { "val": 15000 },
                "used": { "val": 728 },
                "onDemandCap": { "val": 0 },
                "billingPeriodStart": "2099-07-01T00:00:00+00:00",
                "billingPeriodEnd": "2099-08-01T00:00:00+00:00"
            }
        });

        let usage = map_grok_usage(&raw, None, "live", 1)
            .expect("maps current Grok monthly billing response");
        assert_eq!(usage.plan_type, "Grok");
        assert_eq!(usage.period_type, "monthly");
        assert_eq!(usage.limit_value, Some(15_000.0));
        assert_eq!(usage.used_value, Some(728.0));
        assert_eq!(usage.usage_window.used_percent, 5);
        assert_eq!(usage.usage_window.remaining_percent, 95);
        assert_eq!(usage.usage_window.window_seconds, 2_678_400);
        assert_eq!(usage.source, "live");
        assert!(!usage.stale);
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
}

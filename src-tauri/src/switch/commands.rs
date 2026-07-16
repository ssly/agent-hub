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
/// - claude-code → the `ANTHROPIC_API_KEY` itself, which is a long-lived key
///   that only changes when the user actually switches accounts.
fn extract_account_identity(agent_type: &str, content: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(content).ok()?;
    match agent_type {
        "codex" => val
            .get("tokens")
            .and_then(|t| t.get("account_id"))
            .or_else(|| val.get("account_id"))
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

/// Stable identity used to scope the tray cache to the currently active
/// Codex account. Reading this never performs a network request.
pub(crate) fn current_codex_account_id() -> Result<String, String> {
    Ok(resolve_codex_auth()?.account_id)
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

    Ok(CodexUsageResponse {
        plan_type: plan,
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
    fn claude_identity_is_the_api_key() {
        let content = r#"{"env":{"ANTHROPIC_API_KEY":"sk-ant-xyz123"}}"#;
        let id = extract_account_identity("claude-code", content);
        assert_eq!(id.as_deref(), Some("sk-ant-xyz123"));
    }

    #[test]
    fn claude_name_is_never_derived() {
        let content = r#"{"env":{"ANTHROPIC_API_KEY":"sk-ant-xyz123"}}"#;
        assert!(extract_account_name("claude-code", content).is_none());
    }

    #[test]
    fn extract_identity_invalid_json_returns_none() {
        assert!(extract_account_identity("codex", "not json").is_none());
        assert!(extract_account_identity("claude-code", "not json").is_none());
    }
}

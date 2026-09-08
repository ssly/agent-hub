//! Kiro usage query module.
//!
//! Queries Kiro plan limits and credit usage via the official Control Plane API
//! (`GET https://management.us-east-1.kiro.dev/Get-Usage-Limits`).
//!
//! Credentials are read locally and safely from existing Kiro CLI / IDE storage:
//! 1. Kiro CLI SQLite storage: `~/Library/Application Support/kiro-cli/data.sqlite3`
//!    (or OS equivalent path) in the `auth_kv` table.
//! 2. SSO file cache: `~/.aws/sso/cache/kiro-auth-token-cli.json` or `kiro-auth-token.json`.
//! 3. Kiro IDE storage: `~/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/profile.json`.
//! 4. Environment variable: `KIRO_API_KEY`.
//!
//! If a Social login token has expired, it is refreshed silently via
//! `https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};

use super::commands::{
    usage_cache_is_fresh, usage_unix_now, UsageCacheEntry, UsageWindow,
};
use crate::paths::home_dir;

const KIRO_USAGE_URL: &str = "https://management.us-east-1.kiro.dev/Get-Usage-Limits";
const SOCIAL_REFRESH_URL: &str = "https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken";
const DEFAULT_WINDOW_SECONDS: u64 = 2592000; // 30 days (monthly billing cycle)

static KIRO_USAGE_CACHE: Mutex<Option<UsageCacheEntry<KiroUsageResponse>>> = Mutex::new(None);

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct KiroFreeTrial {
    pub current_usage: f64,
    pub usage_limit: f64,
    pub status: String,
    pub expiry: Option<u64>,
}

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct KiroAddOnCredit {
    pub used: f64,
    pub total: f64,
    pub expires_at: Option<u64>,
    pub is_active: bool,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct KiroUsageResponse {
    pub account_name: Option<String>,
    pub plan_type: String,
    pub usage_window: UsageWindow,
    pub usage_windows: Vec<UsageWindow>,
    pub credits_used: f64,
    pub credits_limit: f64,
    pub free_trial: Option<KiroFreeTrial>,
    pub add_on_credits: Vec<KiroAddOnCredit>,
    pub fetched_at: u64,
}

#[derive(Debug, Clone)]
struct KiroAuth {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
    profile_arn: Option<String>,
    auth_method: String,
    user_id: Option<String>,
}

/// Locate the `data.sqlite3` file used by Kiro CLI across platforms.
fn kiro_sqlite_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let home = home_dir();

    // macOS
    paths.push(
        home.join("Library")
            .join("Application Support")
            .join("kiro-cli")
            .join("data.sqlite3"),
    );
    // Linux
    paths.push(
        home.join(".local")
            .join("share")
            .join("kiro-cli")
            .join("data.sqlite3"),
    );
    paths.push(home.join(".config").join("kiro-cli").join("data.sqlite3"));

    // Windows / generic dirs fallback
    if let Some(app_data) = dirs::data_dir() {
        paths.push(app_data.join("kiro-cli").join("data.sqlite3"));
    }

    paths
}

fn parse_expiry_timestamp(val: &serde_json::Value) -> Option<u64> {
    if let Some(ts) = val.as_u64() {
        return Some(ts);
    }
    if let Some(ts_f) = val.as_f64() {
        return Some(ts_f as u64);
    }
    if let Some(s) = val.as_str() {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return Some(dt.timestamp().max(0) as u64);
        }
        if let Ok(dt) = s.parse::<DateTime<Utc>>() {
            return Some(dt.timestamp().max(0) as u64);
        }
    }
    None
}

/// Read Kiro auth credentials from SQLite `auth_kv`.
fn read_sqlite_auth(db_path: &Path) -> Option<KiroAuth> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;

    // Preferred key order: social token -> odic (Builder ID) -> external idp
    let keys = [
        ("kirocli:social:token", "social"),
        ("kirocli:odic:token", "odic"),
        ("kirocli:external-idp:token", "external_idp"),
    ];

    for (key, method) in keys {
        let mut stmt = conn
            .prepare("SELECT value FROM auth_kv WHERE key = ?1")
            .ok()?;
        let value_res: Result<String, _> = stmt.query_row([key], |row| row.get(0));
        if let Ok(raw_json) = value_res {
            if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&raw_json) {
                let access_token = doc
                    .get("access_token")
                    .or_else(|| doc.get("accessToken"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())?;

                let refresh_token = doc
                    .get("refresh_token")
                    .or_else(|| doc.get("refreshToken"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string);

                let expires_at = doc
                    .get("expires_at")
                    .or_else(|| doc.get("expiresAt"))
                    .and_then(parse_expiry_timestamp);

                let profile_arn = doc
                    .get("profile_arn")
                    .or_else(|| doc.get("profileArn"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string);

                let user_id = doc
                    .get("user_id")
                    .or_else(|| doc.get("userId"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string);

                return Some(KiroAuth {
                    access_token: access_token.to_string(),
                    refresh_token,
                    expires_at,
                    profile_arn,
                    auth_method: method.to_string(),
                    user_id,
                });
            }
        }
    }

    None
}

/// Fallback: read cached auth JSON from `~/.aws/sso/cache/`.
fn read_file_auth() -> Option<KiroAuth> {
    let home = home_dir();
    let candidates = [
        home.join(".aws")
            .join("sso")
            .join("cache")
            .join("kiro-auth-token-cli.json"),
        home.join(".aws")
            .join("sso")
            .join("cache")
            .join("kiro-auth-token.json"),
        home.join("Library")
            .join("Application Support")
            .join("Kiro")
            .join("User")
            .join("globalStorage")
            .join("kiro.kiroagent")
            .join("profile.json"),
    ];

    for path in candidates {
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        let doc: serde_json::Value = serde_json::from_str(&content).ok()?;

        let access_token = doc
            .get("accessToken")
            .or_else(|| doc.get("access_token"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())?;

        let refresh_token = doc
            .get("refreshToken")
            .or_else(|| doc.get("refresh_token"))
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let expires_at = doc
            .get("expiresAt")
            .or_else(|| doc.get("expires_at"))
            .and_then(parse_expiry_timestamp);

        let profile_arn = doc
            .get("profileArn")
            .or_else(|| doc.get("profile_arn"))
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let auth_method = doc
            .get("authMethod")
            .or_else(|| doc.get("auth_method"))
            .and_then(|v| v.as_str())
            .unwrap_or("social")
            .to_string();

        return Some(KiroAuth {
            access_token: access_token.to_string(),
            refresh_token,
            expires_at,
            profile_arn,
            auth_method,
            user_id: None,
        });
    }

    // Fallback: check environment variable
    if let Ok(key) = std::env::var("KIRO_API_KEY") {
        let key = key.trim();
        if !key.is_empty() {
            return Some(KiroAuth {
                access_token: key.to_string(),
                refresh_token: None,
                expires_at: None,
                profile_arn: None,
                auth_method: "api_key".to_string(),
                user_id: None,
            });
        }
    }

    None
}

/// Discover existing Kiro credentials without network requests.
fn resolve_kiro_auth() -> Result<KiroAuth, String> {
    for path in kiro_sqlite_paths() {
        if path.exists() {
            if let Some(auth) = read_sqlite_auth(&path) {
                return Ok(auth);
            }
        }
    }

    if let Some(auth) = read_file_auth() {
        return Ok(auth);
    }

    Err("未找到 Kiro 认证凭证。请先在终端运行 kiro-cli 登录。".to_string())
}

/// Check if Kiro credentials are present.
pub fn is_available() -> bool {
    resolve_kiro_auth().is_ok()
}

/// Refresh a Social (Google) token via Kiro desktop auth service.
async fn refresh_social_token(
    refresh_token: &str,
) -> Result<(String, Option<String>, Option<String>), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("无法构建 HTTP 客户端: {e}"))?;

    let payload = serde_json::json!({
        "refreshToken": refresh_token
    });

    let resp = client
        .post(SOCIAL_REFRESH_URL)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("刷新 Kiro 凭据失败: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("刷新 Kiro 凭据返回状态码: HTTP {status}"));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析刷新凭据响应失败: {e}"))?;

    let new_access = data
        .get("accessToken")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "刷新响应缺少 accessToken".to_string())?
        .to_string();

    let new_refresh = data
        .get("refreshToken")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let profile_arn = data
        .get("profileArn")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok((new_access, new_refresh, profile_arn))
}

/// Map raw JSON from `Get-Usage-Limits` to `KiroUsageResponse`.
pub fn map_kiro_usage(raw: &serde_json::Value, account_label: Option<String>, now: u64) -> KiroUsageResponse {
    let sub_info = raw.get("subscriptionInfo");
    let plan_type = sub_info
        .and_then(|s| s.get("subscriptionTitle"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("KIRO FREE")
        .to_string();

    let reset_at = raw
        .get("nextDateReset")
        .and_then(|v| v.as_f64())
        .map(|f| f as u64)
        .unwrap_or(now + DEFAULT_WINDOW_SECONDS);

    let mut credits_used = 0.0;
    let mut credits_limit = 0.0;
    let mut free_trial = None;
    let mut add_on_credits = Vec::new();

    if let Some(list) = raw.get("usageBreakdownList").and_then(|v| v.as_array()) {
        if let Some(item) = list.iter().find(|i| {
            i.get("resourceType")
                .and_then(|v| v.as_str())
                .map(|t| t == "CREDIT")
                .unwrap_or(false)
        }).or_else(|| list.first()) {
            credits_used = item
                .get("currentUsageWithPrecision")
                .and_then(|v| v.as_f64())
                .or_else(|| item.get("currentUsage").and_then(|v| v.as_f64()))
                .unwrap_or(0.0);

            credits_limit = item
                .get("usageLimitWithPrecision")
                .and_then(|v| v.as_f64())
                .or_else(|| item.get("usageLimit").and_then(|v| v.as_f64()))
                .unwrap_or(0.0);

            if let Some(ft) = item.get("freeTrialInfo") {
                let ft_used = ft
                    .get("currentUsageWithPrecision")
                    .and_then(|v| v.as_f64())
                    .or_else(|| ft.get("currentUsage").and_then(|v| v.as_f64()))
                    .unwrap_or(0.0);
                let ft_limit = ft
                    .get("usageLimitWithPrecision")
                    .and_then(|v| v.as_f64())
                    .or_else(|| ft.get("usageLimit").and_then(|v| v.as_f64()))
                    .unwrap_or(0.0);
                let ft_status = ft
                    .get("freeTrialStatus")
                    .and_then(|v| v.as_str())
                    .unwrap_or("EXPIRED")
                    .to_string();
                let ft_expiry = ft
                    .get("freeTrialExpiry")
                    .and_then(|v| v.as_f64())
                    .map(|f| f as u64);

                free_trial = Some(KiroFreeTrial {
                    current_usage: ft_used,
                    usage_limit: ft_limit,
                    status: ft_status,
                    expiry: ft_expiry,
                });
            }

            if let Some(packs) = item.get("overageCredits").and_then(|v| v.as_array()) {
                for pack in packs {
                    let used = pack.get("currentUsage").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let total = pack.get("usageLimit").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let exp = pack.get("expiresAt").and_then(|v| v.as_f64()).map(|f| f as u64);
                    add_on_credits.push(KiroAddOnCredit {
                        used,
                        total,
                        expires_at: exp,
                        is_active: used < total,
                    });
                }
            }
        }
    }

    let used_percent = if credits_limit > 0.0 {
        ((credits_used / credits_limit) * 100.0).clamp(0.0, 100.0) as u8
    } else {
        0
    };
    let remaining_percent = 100u8.saturating_sub(used_percent);
    let reset_after_seconds = reset_at.saturating_sub(now);

    let usage_window = UsageWindow {
        used_percent,
        remaining_percent,
        reset_after_seconds,
        reset_at,
        window_seconds: DEFAULT_WINDOW_SECONDS,
    };

    let user_id = raw
        .get("userInfo")
        .and_then(|u| u.get("userId"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let account_name = account_label.or(user_id);

    KiroUsageResponse {
        account_name,
        plan_type,
        usage_window: usage_window.clone(),
        usage_windows: vec![usage_window],
        credits_used,
        credits_limit,
        free_trial,
        add_on_credits,
        fetched_at: now,
    }
}

/// Fetch live usage from Kiro control plane endpoint.
async fn fetch_kiro_usage() -> Result<KiroUsageResponse, String> {
    let now = usage_unix_now();
    let mut auth = resolve_kiro_auth()?;

    // Check if token requires refresh (expires within 180s)
    let is_expired = auth
        .expires_at
        .map(|exp| now + 180 >= exp)
        .unwrap_or(false);

    if is_expired {
        if let Some(ref refresh_tok) = auth.refresh_token {
            if auth.auth_method == "social" {
                if let Ok((new_access, new_refresh, profile_arn)) =
                    refresh_social_token(refresh_tok).await
                {
                    auth.access_token = new_access;
                    if let Some(r) = new_refresh {
                        auth.refresh_token = Some(r);
                    }
                    if let Some(p) = profile_arn {
                        auth.profile_arn = Some(p);
                    }
                }
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("构建 Kiro 客户端失败: {e}"))?;

    let mut query_params = vec![("origin", "KIRO_CLI")];
    if let Some(ref arn) = auth.profile_arn {
        query_params.push(("profileArn", arn.as_str()));
    }

    let mut request = client
        .get(KIRO_USAGE_URL)
        .query(&query_params)
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .header("x-amzn-kiro-client-attribution", "KiroCLI")
        .header("Accept", "application/json");

    if auth.auth_method == "odic" || auth.auth_method == "IdC" {
        request = request.header("TokenType", "SSO_OIDC");
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("请求 Kiro 用量接口失败: {e}"))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(format!("Kiro 认证失败或 Token 已过期（HTTP {status}）。请在终端运行 kiro-cli 重新登录。"));
    }
    if !status.is_success() {
        return Err(format!("Kiro 用量接口返回错误: HTTP {status}"));
    }

    let raw: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 Kiro 用量响应失败: {e}"))?;

    Ok(map_kiro_usage(&raw, auth.user_id, now))
}

/// Fetch Kiro usage with caching (10m TTL).
#[tauri::command]
pub async fn get_kiro_usage(force: Option<bool>) -> Result<KiroUsageResponse, String> {
    let force = force.unwrap_or(false);
    let now = usage_unix_now();

    if !force {
        if let Ok(guard) = KIRO_USAGE_CACHE.lock() {
            if let Some(entry) = guard.as_ref() {
                if usage_cache_is_fresh(entry.fetched_at, now) {
                    let mut cached = entry.data.clone();
                    cached.usage_window.reset_after_seconds =
                        cached.usage_window.reset_at.saturating_sub(now);
                    for w in &mut cached.usage_windows {
                        w.reset_after_seconds = w.reset_at.saturating_sub(now);
                    }
                    return Ok(cached);
                }
            }
        }
    }

    let usage = fetch_kiro_usage().await?;
    if let Ok(mut guard) = KIRO_USAGE_CACHE.lock() {
        *guard = Some(UsageCacheEntry {
            fetched_at: usage.fetched_at.max(now),
            data: usage.clone(),
        });
    }

    Ok(usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_official_kiro_usage_payload() {
        let raw = serde_json::json!({
            "nextDateReset": 1790812800.0,
            "subscriptionInfo": {
                "subscriptionTitle": "KIRO FREE",
                "type": "Q_DEVELOPER_STANDALONE_FREE",
                "upgradeCapability": "UPGRADE_CAPABLE",
                "overageCapability": "OVERAGE_INCAPABLE"
            },
            "usageBreakdownList": [
                {
                    "resourceType": "CREDIT",
                    "displayName": "Credit",
                    "displayNamePlural": "Credits",
                    "currentUsageWithPrecision": 10.0,
                    "usageLimitWithPrecision": 50.0,
                    "freeTrialInfo": {
                        "currentUsageWithPrecision": 3.0,
                        "usageLimitWithPrecision": 500.0,
                        "freeTrialStatus": "EXPIRED"
                    }
                }
            ],
            "userInfo": {
                "userId": "user-test-123"
            }
        });

        let mapped = map_kiro_usage(&raw, None, 1790000000);
        assert_eq!(mapped.plan_type, "KIRO FREE");
        assert_eq!(mapped.credits_used, 10.0);
        assert_eq!(mapped.credits_limit, 50.0);
        assert_eq!(mapped.usage_window.used_percent, 20);
        assert_eq!(mapped.usage_window.remaining_percent, 80);
        assert_eq!(mapped.account_name.as_deref(), Some("user-test-123"));
        assert!(mapped.free_trial.is_some());
    }
}

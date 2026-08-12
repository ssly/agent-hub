//! DeepSeek balance queries via the official platform API.
//!
//! Unlike the CLI-backed providers there is no local DeepSeek agent install to
//! read credentials from: the user pastes a platform API key
//! (platform.deepseek.com → API keys) and we store it locally at
//! `~/.agent-hub/deepseek.json` (0600 on unix). The key never leaves the
//! machine except as the Bearer token of the official balance endpoint.
//!
//! `GET https://api.deepseek.com/user/balance` is a control-plane endpoint —
//! it consumes no tokens (only chat/completion calls are billed), so polling
//! it on the shared usage cadence is free.

use std::path::PathBuf;
use std::sync::Mutex;

use super::commands::{usage_cache_is_fresh, usage_unix_now, UsageCacheEntry};
use crate::paths::join_relative;

const DEEPSEEK_BALANCE_URL: &str = "https://api.deepseek.com/user/balance";

static DEEPSEEK_USAGE_CACHE: Mutex<Option<UsageCacheEntry<DeepSeekUsageResponse>>> =
    Mutex::new(None);

/// One currency bucket of the account balance. Amounts stay strings exactly
/// as the API returns them ("12.34") — the UI formats them for display.
#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct DeepSeekBalanceInfo {
    /// `CNY` or `USD`.
    pub currency: String,
    /// Total spendable balance = granted + topped-up.
    pub total_balance: String,
    /// Not-yet-expired granted (free) balance.
    pub granted_balance: String,
    /// Topped-up (paid) balance.
    pub topped_up_balance: String,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct DeepSeekUsageResponse {
    /// Whether the balance is sufficient for API calls.
    pub is_available: bool,
    pub balances: Vec<DeepSeekBalanceInfo>,
    pub fetched_at: u64,
}

/// Key presence plus a masked preview — the full key is never sent to the
/// frontend after saving.
#[derive(serde::Serialize, Debug, Clone)]
pub struct DeepSeekSettings {
    pub has_key: bool,
    pub masked_key: Option<String>,
}

fn deepseek_key_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法确定用户主目录".to_string())?;
    Ok(join_relative(home, ".agent-hub/deepseek.json"))
}

fn read_api_key() -> Option<String> {
    let path = deepseek_key_path().ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value
        .get("api_key")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `sk-abc…wxyz` — enough to recognise which key is configured, useless on
/// its own. Keys are ASCII; slice via chars to stay panic-free regardless.
fn mask_api_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 10 {
        return "****".to_string();
    }
    let head: String = chars.iter().take(6).collect();
    let tail: String = chars.iter().skip(chars.len() - 4).collect();
    format!("{head}…{tail}")
}

fn clear_usage_cache() {
    if let Ok(mut guard) = DEEPSEEK_USAGE_CACHE.lock() {
        *guard = None;
    }
}

#[tauri::command]
pub fn get_deepseek_settings() -> DeepSeekSettings {
    match read_api_key() {
        Some(key) => DeepSeekSettings {
            has_key: true,
            masked_key: Some(mask_api_key(&key)),
        },
        None => DeepSeekSettings {
            has_key: false,
            masked_key: None,
        },
    }
}

/// Save (or, when empty, clear) the DeepSeek API key. Stored locally only,
/// with owner-only permissions on unix. Saving a new key drops the cached
/// balance so the next query hits the API with the new credential.
#[tauri::command]
pub fn save_deepseek_api_key(api_key: String) -> Result<DeepSeekSettings, String> {
    let trimmed = api_key.trim();
    let path = deepseek_key_path()?;
    if trimmed.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("无法删除 DeepSeek API Key 文件: {e}"))?;
        }
        clear_usage_cache();
        return Ok(DeepSeekSettings {
            has_key: false,
            masked_key: None,
        });
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("无法创建配置目录: {e}"))?;
    }
    let content = serde_json::json!({ "api_key": trimmed }).to_string();
    std::fs::write(&path, content).map_err(|e| format!("无法保存 DeepSeek API Key: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    clear_usage_cache();
    Ok(get_deepseek_settings())
}

fn map_deepseek_usage(raw: &serde_json::Value, fetched_at: u64) -> DeepSeekUsageResponse {
    let balances = raw
        .get("balance_infos")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| DeepSeekBalanceInfo {
                    currency: item
                        .get("currency")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    total_balance: item
                        .get("total_balance")
                        .and_then(|v| v.as_str())
                        .unwrap_or("0")
                        .to_string(),
                    granted_balance: item
                        .get("granted_balance")
                        .and_then(|v| v.as_str())
                        .unwrap_or("0")
                        .to_string(),
                    topped_up_balance: item
                        .get("topped_up_balance")
                        .and_then(|v| v.as_str())
                        .unwrap_or("0")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    DeepSeekUsageResponse {
        is_available: raw
            .get("is_available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        balances,
        fetched_at,
    }
}

async fn fetch_deepseek_usage() -> Result<DeepSeekUsageResponse, String> {
    let api_key = read_api_key()
        .ok_or_else(|| "未配置 DeepSeek API Key，请先在上方设置中保存 Key。".to_string())?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("构建 DeepSeek 余额请求失败: {e}"))?;

    let response = client
        .get(DEEPSEEK_BALANCE_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("请求 DeepSeek 余额接口失败: {e}"))?;
    let status = response.status();

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(format!("DeepSeek API Key 无效或已过期（HTTP {status}）。"));
    }
    if !status.is_success() {
        return Err(format!("DeepSeek 余额接口返回错误: HTTP {status}"));
    }

    let raw: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 DeepSeek 余额响应失败: {e}"))?;
    Ok(map_deepseek_usage(&raw, usage_unix_now()))
}

/// DeepSeek balance. Same caching contract as the other providers: a
/// 10-minute in-memory TTL unless `force` is true (manual refresh buttons).
#[tauri::command]
pub async fn get_deepseek_usage(force: Option<bool>) -> Result<DeepSeekUsageResponse, String> {
    let force = force.unwrap_or(false);
    let now = usage_unix_now();
    if !force {
        if let Ok(guard) = DEEPSEEK_USAGE_CACHE.lock() {
            if let Some(entry) = guard.as_ref() {
                if usage_cache_is_fresh(entry.fetched_at, now) {
                    return Ok(entry.data.clone());
                }
            }
        }
    }

    let usage = fetch_deepseek_usage().await?;
    if let Ok(mut guard) = DEEPSEEK_USAGE_CACHE.lock() {
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
    fn maps_official_balance_payload() {
        let raw = serde_json::json!({
            "is_available": true,
            "balance_infos": [
                {
                    "currency": "CNY",
                    "total_balance": "12.50",
                    "granted_balance": "2.50",
                    "topped_up_balance": "10.00"
                },
                {
                    "currency": "USD",
                    "total_balance": "0.00",
                    "granted_balance": "0.00",
                    "topped_up_balance": "0.00"
                }
            ]
        });
        let usage = map_deepseek_usage(&raw, 123);
        assert!(usage.is_available);
        assert_eq!(usage.fetched_at, 123);
        assert_eq!(usage.balances.len(), 2);
        assert_eq!(
            usage.balances[0],
            DeepSeekBalanceInfo {
                currency: "CNY".to_string(),
                total_balance: "12.50".to_string(),
                granted_balance: "2.50".to_string(),
                topped_up_balance: "10.00".to_string(),
            }
        );
        assert_eq!(usage.balances[1].currency, "USD");
    }

    #[test]
    fn maps_empty_or_partial_payload_to_defaults() {
        let usage = map_deepseek_usage(&serde_json::json!({}), 7);
        assert!(!usage.is_available);
        assert!(usage.balances.is_empty());
    }

    #[test]
    fn masks_key_with_head_and_tail() {
        assert_eq!(mask_api_key("sk-abcdef1234567890"), "sk-abc…7890");
        assert_eq!(mask_api_key("short"), "****");
        assert_eq!(mask_api_key(""), "****");
    }
}

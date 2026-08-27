//! DeepSeek balance queries via the official platform API.
//!
//! The key is read automatically from DeepSeek Harness's own credential
//! layering — no manual entry in Agent Hub:
//!   1. process env `DEEPSEEK_API_KEY`
//!   2. `$DSH_HOME|~/.dsh/.credentials.yaml` (`refs.DEEPSEEK_API_KEY`,
//!      with a fallback to the pre-release flat `DEEPSEEK_API_KEY` root)
//!   3. `$DSH_HOME|~/.dsh/.env` (dotenv line `DEEPSEEK_API_KEY=…`)
//! The full key never leaves the machine except as the Bearer token of the
//! official balance endpoint.
//!
//! `GET https://api.deepseek.com/user/balance` is a control-plane endpoint —
//! it consumes no tokens (only chat/completion calls are billed), so polling
//! it on the shared usage cadence is free.

use std::path::PathBuf;
use std::sync::Mutex;

use super::commands::{usage_cache_is_fresh, usage_unix_now, UsageCacheEntry};

const DEEPSEEK_BALANCE_URL: &str = "https://api.deepseek.com/user/balance";
const DS_HOME_ENV: &str = "DSH_HOME";

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

/// Key presence only — the full key (and its store) never leaves the
/// backend; the UI needs nothing but "is a credential available".
#[derive(serde::Serialize, Debug, Clone)]
pub struct DeepSeekSettings {
    pub has_key: bool,
}

/// Harness home: `$DSH_HOME` when set, else `~/.dsh` (same rule dsh itself
/// uses).
fn dsh_home() -> Option<PathBuf> {
    if let Ok(home) = std::env::var(DS_HOME_ENV) {
        if !home.trim().is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    Some(crate::paths::home_dir().join(".dsh"))
}

/// `DEEPSEEK_API_KEY` from the harness credentials document
/// (`$DSH_HOME/.credentials.yaml`).
///
/// Current dsh writes a versioned layout (`version: 1` + `refs:` map).
/// Pre-release builds wrote a flat string map; those files still exist on
/// disk until the next harness boot migrates them, so both shapes are read.
fn read_harness_credentials_key() -> Option<String> {
    let path = dsh_home()?.join(".credentials.yaml");
    let content = std::fs::read_to_string(path).ok()?;
    parse_credentials_key(&content)
}

fn yaml_nonempty_string(value: &serde_yaml::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn parse_credentials_key(content: &str) -> Option<String> {
    let doc: serde_yaml::Value = serde_yaml::from_str(content).ok()?;
    doc.get("refs")
        .and_then(|refs| refs.get("DEEPSEEK_API_KEY"))
        .and_then(yaml_nonempty_string)
        .or_else(|| doc.get("DEEPSEEK_API_KEY").and_then(yaml_nonempty_string))
}

/// `DEEPSEEK_API_KEY=` line from the harness env fallback (`$DSH_HOME/.env`).
/// Simple dotenv scan — comments and `export` prefixes tolerated, quotes
/// stripped.
fn read_harness_dotenv_key() -> Option<String> {
    let path = dsh_home()?.join(".env");
    let content = std::fs::read_to_string(path).ok()?;
    parse_dotenv_key(&content)
}

fn parse_dotenv_key(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() != "DEEPSEEK_API_KEY" {
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn read_env_api_key() -> Option<String> {
    std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Layered resolution: env > harness credentials > harness .env.
fn read_api_key() -> Option<String> {
    read_env_api_key()
        .or_else(read_harness_credentials_key)
        .or_else(read_harness_dotenv_key)
}

#[tauri::command]
pub fn get_deepseek_settings() -> DeepSeekSettings {
    DeepSeekSettings {
        has_key: read_api_key().is_some(),
    }
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
    let api_key = read_api_key().ok_or_else(|| {
        "未找到 DeepSeek API Key：未检测到 DeepSeek Harness 的凭证（~/.dsh/.credentials.yaml 或环境变量）。请先在 dsh 中登录。".to_string()
    })?;

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
    fn parses_dotenv_key_variants() {
        assert_eq!(
            parse_dotenv_key("DEEPSEEK_API_KEY=sk-a\n"),
            Some("sk-a".into())
        );
        assert_eq!(
            parse_dotenv_key("export DEEPSEEK_API_KEY=\"sk-b\"\n"),
            Some("sk-b".into())
        );
        assert_eq!(
            parse_dotenv_key("  DEEPSEEK_API_KEY = 'sk-c'  \n"),
            Some("sk-c".into())
        );
        assert_eq!(parse_dotenv_key("# comment\nOTHER_KEY=x\n"), None);
        assert_eq!(parse_dotenv_key("DEEPSEEK_API_KEY=\n"), None);
    }

    #[test]
    fn parses_versioned_credentials_refs() {
        let content =
            "version: 1\nrefs:\n  DEEPSEEK_API_KEY: sk-versioned\n  OPENAI_API_KEY: sk-other\n";
        assert_eq!(parse_credentials_key(content), Some("sk-versioned".into()));
    }

    #[test]
    fn parses_legacy_flat_credentials() {
        let content = "DEEPSEEK_API_KEY: sk-flat\nOPENAI_API_KEY: sk-other\n";
        assert_eq!(parse_credentials_key(content), Some("sk-flat".into()));
    }

    #[test]
    fn versioned_credentials_without_deepseek_key_are_absent() {
        let content = "version: 1\nrefs:\n  OPENAI_API_KEY: sk-other\n";
        assert_eq!(parse_credentials_key(content), None);
    }

    #[test]
    fn empty_or_malformed_credentials_are_absent() {
        assert_eq!(parse_credentials_key(""), None);
        assert_eq!(parse_credentials_key("version: 1\nrefs: {}\n"), None);
        assert_eq!(
            parse_credentials_key("version: 1\nrefs:\n  DEEPSEEK_API_KEY: \"\"\n"),
            None
        );
        assert_eq!(parse_credentials_key("not: [valid"), None);
    }
}

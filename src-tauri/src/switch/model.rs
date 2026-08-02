use serde::{Deserialize, Serialize};

/// Profiles saved before OAuth support have no `kind` field; they are all
/// token-based (settings.json `env.ANTHROPIC_AUTH_TOKEN`) profiles.
fn default_profile_kind() -> String {
    "token".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProfile {
    pub id: String,
    pub note: String,
    pub saved_at: String,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// `token` = custom account via settings.json env token;
    /// `oauth` = official Claude Code /login subscription account.
    #[serde(default = "default_profile_kind")]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub id: String,
    pub note: String,
    pub saved_at: String,
    /// `token` | `oauth` — see [`AuthProfile::kind`].
    #[serde(default = "default_profile_kind")]
    pub kind: String,
    /// Stable account identity used to match the active profile. Only set for
    /// `oauth` profiles (the `oauthAccount.accountUuid` captured at save time,
    /// falling back to the account email); token profiles derive their identity
    /// from the stored config content instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSwitchResponse {
    pub profiles: Vec<AuthProfile>,
    pub current_key: Option<String>,
}

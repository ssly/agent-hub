use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default, serde::Serialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub platforms: Vec<CustomPlatform>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub language: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { language: "auto".to_string() }
    }
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct CustomPlatform {
    pub id: String,
    pub display_name: String,
    pub skill_dir: String,
}

impl Config {
    pub fn load() -> Self {
        let config_path = dirs::home_dir()
            .map(|h| h.join(".agent-hub/config.toml"))
            .unwrap_or_else(|| PathBuf::from(".agent-hub/config.toml"));

        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        Config::default()
    }

    pub fn resolved_language(&self) -> Option<&str> {
        if self.general.language == "auto" { None } else { Some(&self.general.language) }
    }
}

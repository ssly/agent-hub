use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default, serde::Serialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub platforms: Vec<CustomPlatform>,
    #[serde(default)]
    pub monitor: crate::monitor::types::MonitorConfig,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub language: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            language: "auto".to_string(),
        }
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
        let config = Config::default();
        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &config_path,
            toml::to_string_pretty(&config).unwrap_or_default(),
        );
        config
    }

    pub fn save(&self) -> Result<(), String> {
        let config_path = dirs::home_dir()
            .map(|h| h.join(".agent-hub/config.toml"))
            .ok_or("Cannot determine home directory")?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&config_path, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn resolved_language(&self) -> Option<&str> {
        if self.general.language == "auto" {
            None
        } else {
            Some(&self.general.language)
        }
    }
}

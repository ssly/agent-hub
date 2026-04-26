use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Skill {
    pub name: String,
    pub version: Option<String>,
    pub description: String,
    pub platform_id: String,
    #[serde(skip)]
    pub path: PathBuf,
    #[serde(skip)]
    pub skill_file: PathBuf,
    #[serde(skip)]
    pub content: String,
    #[serde(skip)]
    pub body: String,
    #[serde(skip)]
    pub metadata: HashMap<String, serde_yaml::Value>,
    pub is_symlink: bool,
    pub symlink_target: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    #[serde(skip)]
    pub modified_at: Option<SystemTime>,
    pub total_size: u64,
}

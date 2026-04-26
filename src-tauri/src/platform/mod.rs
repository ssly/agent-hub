mod discovery;
mod registry;

pub use discovery::discover_platforms;
pub use registry::PlatformDef;

use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Platform {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub skill_dir: PathBuf,
    pub installed: bool,
    #[serde(skip)]
    pub skills: Vec<crate::skill::Skill>,
}

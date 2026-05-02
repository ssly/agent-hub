use super::{Platform, PlatformDef};
use crate::config::Config;
use crate::platform::registry::builtin_platforms;
use crate::skill::scan_skills;

pub fn discover_platforms(config: &Config) -> Vec<Platform> {
    let mut defs = builtin_platforms();

    for custom in &config.platforms {
        let expanded = shellexpand_home(&custom.skill_dir);
        let presence_path = expanded
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| expanded.clone());
        defs.push(PlatformDef {
            id: custom.id.clone(),
            display_name: custom.display_name.clone(),
            description: String::new(),
            presence_path,
            skill_dir: expanded,
        });
    }

    let mut platforms: Vec<Platform> = defs
        .into_iter()
        .filter(|d| d.presence_path.exists())
        .map(|d| {
            let skills = if d.skill_dir.exists() {
                scan_skills(&d.skill_dir, &d.id)
            } else {
                Vec::new()
            };
            Platform {
                id: d.id,
                display_name: d.display_name,
                description: d.description,
                skill_dir: d.skill_dir,
                installed: true,
                skills,
            }
        })
        .collect();

    platforms.sort_by(|a, b| b.skills.len().cmp(&a.skills.len()));
    platforms
}

fn shellexpand_home(path: &str) -> std::path::PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        dirs::home_dir()
            .map(|h| h.join(stripped))
            .unwrap_or_else(|| std::path::PathBuf::from(path))
    } else {
        std::path::PathBuf::from(path)
    }
}

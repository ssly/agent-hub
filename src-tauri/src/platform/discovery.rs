use super::{Platform, PlatformDef};
use crate::config::Config;
use crate::platform::registry::{builtin_platforms, join_relative};
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

    defs.into_iter()
        .filter(|d| d.presence_path.exists())
        .map(|d| Platform {
            id: d.id,
            display_name: d.display_name,
            description: d.description,
            skill_dir: d.skill_dir,
            installed: true,
            skills_loaded: false,
            skills: Vec::new(),
        })
        .collect()
}

pub fn load_platform_skills(platform: &mut Platform) {
    if platform.skills_loaded {
        return;
    }
    platform.skills = if platform.skill_dir.exists() {
        scan_skills(&platform.skill_dir, &platform.id)
    } else {
        Vec::new()
    };
    platform.skills_loaded = true;
}

pub fn ensure_all_skills_loaded(platforms: &mut [Platform]) {
    for p in platforms.iter_mut() {
        load_platform_skills(p);
    }
}

pub fn invalidate_platform_skills(platform: &mut Platform) {
    platform.skills_loaded = false;
    platform.skills.clear();
}

fn shellexpand_home(path: &str) -> std::path::PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        // `stripped` may itself contain '/' or '\' (e.g. ".claude/skills");
        // join each segment separately to avoid mixed-separator paths on Windows.
        dirs::home_dir()
            .map(|h| join_relative(h, stripped))
            .unwrap_or_else(|| std::path::PathBuf::from(path))
    } else {
        // A literal path (possibly already containing native separators).
        std::path::PathBuf::from(path)
    }
}

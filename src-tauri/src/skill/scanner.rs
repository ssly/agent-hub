use std::fs;
use std::path::Path;

use crate::skill::parser::parse_skill;
use crate::skill::Skill;

pub fn scan_skills(skill_dir: &Path, platform_id: &str) -> Vec<Skill> {
    let Ok(entries) = fs::read_dir(skill_dir) else { return Vec::new(); };
    let mut skills: Vec<Skill> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() || path.is_symlink() { parse_skill(&path, platform_id) } else { None }
        })
        .collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

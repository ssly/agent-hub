use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::skill::parser::parse_skill;
use crate::skill::Skill;

pub fn scan_skills(skill_dir: &Path, platform_id: &str) -> Vec<Skill> {
    let mut skills: Vec<Skill> = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    if let Ok(canonical) = skill_dir.canonicalize() {
        visited.insert(canonical);
    }
    scan_recursive(skill_dir, skill_dir, platform_id, &mut skills, &mut visited);
    skills.sort_by(|a, b| a.folder.cmp(&b.folder).then(a.name.cmp(&b.name)));
    skills
}

fn scan_recursive(
    current_dir: &Path,
    root_dir: &Path,
    platform_id: &str,
    skills: &mut Vec<Skill>,
    visited: &mut HashSet<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(current_dir) else {
        return;
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() && !path.is_symlink() {
            continue;
        }

        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if visited.contains(&canonical) {
            continue;
        }

        if path.join("SKILL.md").exists() {
            let folder = path
                .parent()
                .and_then(|p| p.strip_prefix(root_dir).ok())
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();

            if let Some(mut skill) = parse_skill(&path, platform_id) {
                skill.folder = folder;
                skills.push(skill);
            }
        } else {
            visited.insert(canonical);
            scan_recursive(&path, root_dir, platform_id, skills, visited);
        }
    }
}

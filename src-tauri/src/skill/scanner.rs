use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::skill::parser::{parse_flat_skill, parse_skill};
use crate::skill::Skill;

/// Legacy signature kept for API stability: plain directory-bundle scan
/// (SKILL.md folders only, no flat Markdown files).
#[allow(dead_code)]
pub fn scan_skills(skill_dir: &Path, platform_id: &str) -> Vec<Skill> {
    scan_skills_ext(skill_dir, platform_id, false)
}

/// Scan a platform skills directory. `allow_flat_md` additionally picks up
/// flat Markdown skills (`*.md` directly inside the root, SKILL.md excluded) —
/// DeepSeek Harness loads this layout beside directory-bundle skills.
pub fn scan_skills_ext(skill_dir: &Path, platform_id: &str, allow_flat_md: bool) -> Vec<Skill> {
    let mut skills: Vec<Skill> = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    if let Ok(canonical) = skill_dir.canonicalize() {
        visited.insert(canonical);
    }
    scan_recursive(skill_dir, skill_dir, platform_id, &mut skills, &mut visited);
    if allow_flat_md {
        scan_flat_files(skill_dir, platform_id, &mut skills);
    }
    skills.sort_by(|a, b| a.folder.cmp(&b.folder).then(a.name.cmp(&b.name)));
    skills
}

/// Flat skills live only directly under the root — files deeper inside a
/// directory bundle (or a random README) are never promoted to skills.
fn scan_flat_files(root_dir: &Path, platform_id: &str, skills: &mut Vec<Skill>) {
    let Ok(entries) = fs::read_dir(root_dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || path.is_symlink() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
        {
            continue;
        }
        if let Some(skill) = parse_flat_skill(&path, platform_id) {
            skills.push(skill);
        }
    }
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

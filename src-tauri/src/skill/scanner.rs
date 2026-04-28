use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::skill::parser::parse_skill;
use crate::skill::Skill;

#[derive(Debug, Clone, serde::Serialize)]
pub struct InvalidSkill {
    pub path: String,
    pub platform_id: String,
    pub platform_name: String,
    pub reason: String,
}

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

fn scan_recursive(current_dir: &Path, root_dir: &Path, platform_id: &str, skills: &mut Vec<Skill>, visited: &mut HashSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(current_dir) else { return };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() && !path.is_symlink() { continue; }

        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if visited.contains(&canonical) { continue; }

        if path.join("SKILL.md").exists() {
            let folder = path.parent()
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

// --- Invalid skill detection ---

pub fn scan_invalid_skills(skill_dir: &Path, platform_id: &str, platform_name: &str) -> Vec<InvalidSkill> {
    let mut invalid: Vec<InvalidSkill> = Vec::new();
    scan_invalid_recursive(skill_dir, skill_dir, platform_id, platform_name, &mut invalid);
    invalid
}

fn scan_invalid_recursive(current_dir: &Path, root_dir: &Path, platform_id: &str, platform_name: &str, invalid: &mut Vec<InvalidSkill>) {
    let Ok(entries) = fs::read_dir(current_dir) else { return };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() && !path.is_symlink() { continue; }

        if path.join("SKILL.md").exists() {
            validate_skill(&path, platform_id, platform_name, invalid);
        } else {
            // 没有 SKILL.md，但目录下有文件 -> 可能是缺少 SKILL.md 的 skill
            let has_files = fs::read_dir(&path)
                .map(|mut e| e.any(|f| f.map(|fe| fe.file_type().map(|ft| ft.is_file()).unwrap_or(false)).unwrap_or(false)))
                .unwrap_or(false);
            let is_special = path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.') || n == "node_modules" || n == "target")
                .unwrap_or(false);

            if has_files && !is_special {
                invalid.push(InvalidSkill {
                    path: path.display().to_string(),
                    platform_id: platform_id.to_string(),
                    platform_name: platform_name.to_string(),
                    reason: "缺少 SKILL.md".to_string(),
                });
            }

            scan_invalid_recursive(&path, root_dir, platform_id, platform_name, invalid);
        }
    }
}

fn validate_skill(path: &Path, platform_id: &str, platform_name: &str, invalid: &mut Vec<InvalidSkill>) {
    let skill_file = path.join("SKILL.md");
    let content = match fs::read_to_string(&skill_file) {
        Ok(c) => c,
        Err(e) => {
            invalid.push(InvalidSkill {
                path: path.display().to_string(),
                platform_id: platform_id.to_string(),
                platform_name: platform_name.to_string(),
                reason: format!("无法读取 SKILL.md: {}", e),
            });
            return;
        }
    };

    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        invalid.push(InvalidSkill {
            path: path.display().to_string(),
            platform_id: platform_id.to_string(),
            platform_name: platform_name.to_string(),
            reason: "SKILL.md 缺少 frontmatter（应以 --- 开头）".to_string(),
        });
        return;
    }

    let rest = &trimmed[3..];
    let Some(end) = rest.find("---") else {
        invalid.push(InvalidSkill {
            path: path.display().to_string(),
            platform_id: platform_id.to_string(),
            platform_name: platform_name.to_string(),
            reason: "SKILL.md frontmatter 未闭合（缺少结束的 ---）".to_string(),
        });
        return;
    };

    let yaml_str = &rest[..end];
    let body = rest[end + 3..].trim();

    let metadata: std::collections::HashMap<String, serde_yaml::Value> = match serde_yaml::from_str(yaml_str) {
        Ok(m) => m,
        Err(e) => {
            invalid.push(InvalidSkill {
                path: path.display().to_string(),
                platform_id: platform_id.to_string(),
                platform_name: platform_name.to_string(),
                reason: format!("Frontmatter YAML 解析失败: {}", e),
            });
            return;
        }
    };

    let mut reasons = Vec::new();
    if metadata.get("name").and_then(|v| v.as_str()).map(|s| s.trim().is_empty()).unwrap_or(true) {
        reasons.push("缺少 name 字段");
    }
    if body.is_empty() {
        reasons.push("内容为空");
    }

    if !reasons.is_empty() {
        invalid.push(InvalidSkill {
            path: path.display().to_string(),
            platform_id: platform_id.to_string(),
            platform_name: platform_name.to_string(),
            reason: reasons.join(", "),
        });
    }
}

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::skill::model::Skill;

pub fn parse_skill(skill_dir: &Path, platform_id: &str) -> Option<Skill> {
    let skill_file = skill_dir.join("SKILL.md");
    if !skill_file.exists() {
        return None;
    }

    let content = fs::read_to_string(&skill_file).ok()?;
    let (metadata, body) = parse_frontmatter(&content);

    let name = metadata
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            skill_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        })
        .to_string();

    let version = metadata
        .get("version")
        .and_then(|v| v.as_str())
        .map(String::from);
    let description = metadata
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_string();

    let is_symlink = skill_dir.is_symlink();
    let symlink_target = if is_symlink {
        fs::read_link(skill_dir).ok()
    } else {
        None
    };
    let files = list_files(skill_dir);
    let modified_at = fs::metadata(skill_dir).ok().and_then(|m| m.modified().ok());
    let total_size = calc_total_size(skill_dir);

    Some(Skill {
        name,
        folder: String::new(),
        version,
        description,
        platform_id: platform_id.to_string(),
        path: skill_dir.to_path_buf(),
        skill_file,
        content,
        body,
        metadata,
        is_symlink,
        symlink_target,
        files,
        modified_at,
        total_size,
    })
}

/// Parse a "flat" skill: one Markdown file (e.g. `<name>.md`) directly inside
/// the skills directory. DeepSeek Harness supports this layout beside
/// directory-bundle skills (SKILL.md folders). The file's own stem is the
/// fallback name when frontmatter carries no `name`.
pub fn parse_flat_skill(md_file: &Path, platform_id: &str) -> Option<Skill> {
    let content = fs::read_to_string(md_file).ok()?;
    let (metadata, body) = parse_frontmatter(&content);

    let file_stem = md_file
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let name = metadata
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(file_stem)
        .to_string();

    let version = metadata
        .get("version")
        .and_then(|v| v.as_str())
        .map(String::from);
    let description = metadata
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_string();

    let is_symlink = md_file.is_symlink();
    let symlink_target = if is_symlink {
        fs::read_link(md_file).ok()
    } else {
        None
    };
    let files = md_file
        .file_name()
        .map(PathBuf::from)
        .into_iter()
        .collect::<Vec<_>>();
    let modified_at = fs::metadata(md_file).ok().and_then(|m| m.modified().ok());
    let total_size = fs::metadata(md_file).map(|m| m.len()).unwrap_or(0);

    Some(Skill {
        name,
        folder: String::new(),
        version,
        description,
        platform_id: platform_id.to_string(),
        path: md_file.to_path_buf(),
        skill_file: md_file.to_path_buf(),
        content,
        body,
        metadata,
        is_symlink,
        symlink_target,
        files,
        modified_at,
        total_size,
    })
}

fn parse_frontmatter(content: &str) -> (HashMap<String, serde_yaml::Value>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (HashMap::new(), content.to_string());
    }
    let rest = &trimmed[3..];
    if let Some(end) = rest.find("---") {
        let yaml_str = &rest[..end];
        let body = rest[end + 3..].trim().to_string();
        return (serde_yaml::from_str(yaml_str).unwrap_or_default(), body);
    }
    (HashMap::new(), content.to_string())
}

fn list_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Ok(relative) = entry.path().strip_prefix(dir) {
                files.push(relative.to_path_buf());
            }
        }
    }
    files.sort();
    files
}

fn calc_total_size(dir: &Path) -> u64 {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

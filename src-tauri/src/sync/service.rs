use std::fs;
use std::path::Path;

use crate::platform::Platform;
use crate::skill::Skill;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    Symlink,
    Copy,
}

impl Default for SyncMode {
    fn default() -> Self {
        SyncMode::Copy
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SyncError {
    TargetExists(String),
    IoError(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::TargetExists(path) => write!(f, "Target already exists: {}", path),
            SyncError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

fn target_dir(source: &Skill, target_platform: &Platform) -> std::path::PathBuf {
    if source.folder.is_empty() {
        target_platform.skill_dir.join(&source.name)
    } else {
        target_platform
            .skill_dir
            .join(&source.folder)
            .join(&source.name)
    }
}

fn ensure_parent(dir: &Path) -> Result<(), SyncError> {
    if let Some(parent) = dir.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| SyncError::IoError(e.to_string()))?;
        }
    }
    Ok(())
}

pub fn sync_skill(
    source: &Skill,
    target_platform: &Platform,
    mode: SyncMode,
) -> Result<(), SyncError> {
    let target_dir = target_dir(source, target_platform);
    if target_dir.symlink_metadata().is_ok() {
        return Err(SyncError::TargetExists(target_dir.display().to_string()));
    }
    ensure_parent(&target_dir)?;

    // Resolve the canonical source (源头): if source is already a symlink or junction,
    // trace through to the physical origin folder so we link/copy directly from the origin.
    let canonical_source = source
        .path
        .canonicalize()
        .unwrap_or_else(|_| source.path.clone());
    let canonical_source = crate::paths::clean_canonical_path(&canonical_source);

    match mode {
        SyncMode::Symlink => {
            crate::paths::create_symlink_auto(&canonical_source, &target_dir)
                .map_err(|e| SyncError::IoError(format!("symlink failed: {}", e)))?;
        }
        SyncMode::Copy => {
            copy_dir_recursive(&canonical_source, &target_dir)?;
        }
    }
    Ok(())
}

pub fn sync_overwrite(
    source: &Skill,
    target_platform: &Platform,
    mode: SyncMode,
) -> Result<(), SyncError> {
    let target_dir = target_dir(source, target_platform);
    ensure_parent(&target_dir)?;

    if target_dir.symlink_metadata().is_ok() {
        if target_dir.is_symlink() {
            crate::paths::remove_dir_link(&target_dir)
                .map_err(|e| SyncError::IoError(e.to_string()))?;
        } else if target_dir.is_dir() {
            fs::remove_dir_all(&target_dir).map_err(|e| SyncError::IoError(e.to_string()))?;
        } else {
            fs::remove_file(&target_dir).map_err(|e| SyncError::IoError(e.to_string()))?;
        }
    }

    // Resolve canonical source and execute sync
    let canonical_source = source
        .path
        .canonicalize()
        .unwrap_or_else(|_| source.path.clone());
    let canonical_source = crate::paths::clean_canonical_path(&canonical_source);

    match mode {
        SyncMode::Symlink => {
            crate::paths::create_symlink_auto(&canonical_source, &target_dir)
                .map_err(|e| SyncError::IoError(format!("symlink failed: {}", e)))?;
        }
        SyncMode::Copy => {
            copy_dir_recursive(&canonical_source, &target_dir)?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), SyncError> {
    let resolved_source = std::path::Path::canonicalize(source)
        .map_err(|e| SyncError::IoError(format!("Failed to resolve path: {}", e)))?;
    let resolved_source = crate::paths::clean_canonical_path(&resolved_source);

    if resolved_source.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| SyncError::IoError(e.to_string()))?;
        }
        fs::copy(&resolved_source, target).map_err(|e| SyncError::IoError(e.to_string()))?;
        return Ok(());
    }

    copy_dir_all(&resolved_source, target).map_err(|e| SyncError::IoError(e.to_string()))
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&from, &to)?;
        } else if file_type.is_symlink() {
            // Dereference internal symlinks so the copy is an independent physical replica
            let canonical = from.canonicalize().unwrap_or_else(|_| from.clone());
            if canonical.is_dir() {
                copy_dir_all(&canonical, &to)?;
            } else if canonical.is_file() {
                fs::copy(&canonical, &to)?;
            }
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn mock_platform(id: &str, dir: PathBuf) -> Platform {
        Platform {
            id: id.to_string(),
            display_name: id.to_string(),
            description: String::new(),
            skill_dir: dir,
            installed: true,
            skills_loaded: true,
            skills: Vec::new(),
        }
    }

    fn mock_skill(name: &str, path: PathBuf) -> Skill {
        Skill {
            name: name.to_string(),
            folder: String::new(),
            version: None,
            description: String::new(),
            platform_id: "test".to_string(),
            path: path.clone(),
            skill_file: path.join("SKILL.md"),
            content: String::new(),
            body: String::new(),
            metadata: HashMap::new(),
            is_symlink: false,
            symlink_target: None,
            files: Vec::new(),
            modified_at: None,
            total_size: 0,
        }
    }

    #[test]
    fn test_sync_copy_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let src_platform_dir = tmp.path().join("platform_a");
        let tgt_platform_dir = tmp.path().join("platform_b");
        let skill_src = src_platform_dir.join("test-skill");
        fs::create_dir_all(&skill_src).unwrap();
        fs::write(skill_src.join("SKILL.md"), b"# Test Skill").unwrap();

        let source_skill = mock_skill("test-skill", skill_src);
        let target_platform = mock_platform("b", tgt_platform_dir.clone());

        sync_skill(&source_skill, &target_platform, SyncMode::Copy).unwrap();

        let tgt_skill = tgt_platform_dir.join("test-skill");
        assert!(tgt_skill.exists());
        assert!(!tgt_skill.is_symlink());
        assert_eq!(
            fs::read_to_string(tgt_skill.join("SKILL.md")).unwrap(),
            "# Test Skill"
        );
    }

    #[test]
    fn test_sync_symlink_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let src_platform_dir = tmp.path().join("platform_a");
        let tgt_platform_dir = tmp.path().join("platform_b");
        let skill_src = src_platform_dir.join("test-skill");
        fs::create_dir_all(&skill_src).unwrap();
        fs::write(skill_src.join("SKILL.md"), b"# Test Skill").unwrap();

        let source_skill = mock_skill("test-skill", skill_src);
        let target_platform = mock_platform("b", tgt_platform_dir.clone());

        sync_skill(&source_skill, &target_platform, SyncMode::Symlink).unwrap();

        let tgt_skill = tgt_platform_dir.join("test-skill");
        assert!(tgt_skill.exists());
        assert!(tgt_skill.is_symlink());
        assert_eq!(
            fs::read_to_string(tgt_skill.join("SKILL.md")).unwrap(),
            "# Test Skill"
        );
    }

    #[test]
    fn test_sync_symlink_traces_to_canonical_origin() {
        let tmp = tempfile::tempdir().unwrap();
        let origin_dir = tmp.path().join("shared_origin").join("test-skill");
        fs::create_dir_all(&origin_dir).unwrap();
        fs::write(origin_dir.join("SKILL.md"), b"# Original Content").unwrap();

        // Platform A has a symlink pointing to shared_origin
        let platform_a_dir = tmp.path().join("platform_a");
        fs::create_dir_all(&platform_a_dir).unwrap();
        let platform_a_link = platform_a_dir.join("test-skill");
        crate::paths::create_dir_link(&origin_dir, &platform_a_link).unwrap();

        // Now sync from Platform A to Platform B with Symlink mode
        let mut source_skill = mock_skill("test-skill", platform_a_link.clone());
        source_skill.is_symlink = true;
        source_skill.symlink_target = Some(origin_dir.clone());

        let platform_b_dir = tmp.path().join("platform_b");
        let platform_b = mock_platform("b", platform_b_dir.clone());

        sync_skill(&source_skill, &platform_b, SyncMode::Symlink).unwrap();

        let platform_b_link = platform_b_dir.join("test-skill");
        assert!(platform_b_link.exists());
        assert!(platform_b_link.is_symlink());

        // Platform B's link must point directly to origin_dir (canonical source),
        // so even if Platform A's intermediate link is removed, Platform B still works!
        crate::paths::remove_dir_link(&platform_a_link).unwrap();
        assert!(!platform_a_link.exists());

        assert!(platform_b_link.exists());
        assert_eq!(
            fs::read_to_string(platform_b_link.join("SKILL.md")).unwrap(),
            "# Original Content"
        );
    }

    #[test]
    fn test_sync_copy_traces_to_canonical_origin() {
        let tmp = tempfile::tempdir().unwrap();
        let origin_dir = tmp.path().join("shared_origin").join("test-skill");
        fs::create_dir_all(&origin_dir).unwrap();
        fs::write(origin_dir.join("SKILL.md"), b"# Original Content").unwrap();

        // Platform A has a symlink pointing to shared_origin
        let platform_a_dir = tmp.path().join("platform_a");
        fs::create_dir_all(&platform_a_dir).unwrap();
        let platform_a_link = platform_a_dir.join("test-skill");
        crate::paths::create_dir_link(&origin_dir, &platform_a_link).unwrap();

        // Now sync from Platform A to Platform B with Copy mode
        let mut source_skill = mock_skill("test-skill", platform_a_link.clone());
        source_skill.is_symlink = true;
        source_skill.symlink_target = Some(origin_dir.clone());

        let platform_b_dir = tmp.path().join("platform_b");
        let platform_b = mock_platform("b", platform_b_dir.clone());

        sync_skill(&source_skill, &platform_b, SyncMode::Copy).unwrap();

        let platform_b_copy = platform_b_dir.join("test-skill");
        assert!(platform_b_copy.exists());
        // Must be an independent physical directory, not a symlink!
        assert!(!platform_b_copy.is_symlink());

        // Platform A's intermediate link is removed
        crate::paths::remove_dir_link(&platform_a_link).unwrap();
        assert!(!platform_a_link.exists());

        // Platform B's copy must be fully intact
        assert!(platform_b_copy.exists());
        assert_eq!(
            fs::read_to_string(platform_b_copy.join("SKILL.md")).unwrap(),
            "# Original Content"
        );

        // Modifying Platform B must NOT modify origin_dir (isolated copy)
        fs::write(platform_b_copy.join("SKILL.md"), b"# Modified Copy").unwrap();
        assert_eq!(
            fs::read_to_string(origin_dir.join("SKILL.md")).unwrap(),
            "# Original Content"
        );
    }
}

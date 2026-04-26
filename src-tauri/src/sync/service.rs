use std::fs;
use std::path::Path;

use crate::platform::Platform;
use crate::skill::Skill;

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

pub fn sync_skill(source: &Skill, target_platform: &Platform) -> Result<(), SyncError> {
    let target_dir = target_platform.skill_dir.join(&source.name);
    if target_dir.exists() { return Err(SyncError::TargetExists(target_dir.display().to_string())); }
    copy_dir_recursive(&source.path, &target_dir)
}

pub fn sync_overwrite(source: &Skill, target_platform: &Platform) -> Result<(), SyncError> {
    let target_dir = target_platform.skill_dir.join(&source.name);
    if target_dir.exists() {
        if target_dir.is_symlink() { fs::remove_file(&target_dir).map_err(|e| SyncError::IoError(e.to_string()))?; }
        else { fs::remove_dir_all(&target_dir).map_err(|e| SyncError::IoError(e.to_string()))?; }
    }
    copy_dir_recursive(&source.path, &target_dir)
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), SyncError> {
    let options = fs_extra::dir::CopyOptions::new().content_only(false).copy_inside(false);
    fs_extra::dir::copy(source, target.parent().unwrap(), &options)
        .map_err(|e| SyncError::IoError(e.to_string()))?;
    Ok(())
}

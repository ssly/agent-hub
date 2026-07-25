//! Cross-platform filesystem path helpers.
//!
//! All platform-specific path handling lives here so the rest of the codebase
//! stays platform-agnostic:
//!
//! - [`join_relative`] — join multi-segment relative paths without mixing
//!   `/` and `\` in displayed paths on Windows.
//! - [`home_dir`] — home directory with a platform-appropriate fallback.
//! - [`replace_file`] — atomic replace that also works on Windows, where
//!   `fs::rename` refuses to overwrite an existing target.

use std::path::{Path, PathBuf};

/// Join `base` with a relative path that may use either `/` or `\` as
/// separator. We split on *both* and join each non-empty segment separately,
/// so on Windows we don't end up with mixed-separator paths like
/// `C:\Users\xxx\.claude/skills` (which `base.join(".claude/skills")` would
/// produce, because the whole string is treated as a single component).
pub fn join_relative(base: PathBuf, rel: &str) -> PathBuf {
    let mut out = base;
    for seg in rel.split(['/', '\\']) {
        if !seg.is_empty() {
            out = out.join(seg);
        }
    }
    out
}

/// Home directory with a platform-appropriate last-resort fallback:
/// `/` on macOS/Linux, the root of the current drive on Windows.
pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(std::path::MAIN_SEPARATOR_STR))
}

/// Replace the regular file `target` with `from`, atomically where the
/// platform allows it.
///
/// On macOS/Linux this is a plain `rename`, which overwrites `target`. On
/// Windows `rename` fails when `target` exists, so the existing file is first
/// moved aside as a backup and restored if the replacement fails.
pub fn replace_file(from: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(not(target_os = "windows"))]
    {
        std::fs::rename(from, target)
    }
    #[cfg(target_os = "windows")]
    {
        if !target.exists() {
            return std::fs::rename(from, target);
        }
        let mut backup_name = target
            .file_name()
            .map(|name| name.to_os_string())
            .unwrap_or_default();
        backup_name.push(format!(".bak-{}", uuid::Uuid::new_v4()));
        let backup = target.with_file_name(backup_name);
        std::fs::rename(target, &backup)?;
        match std::fs::rename(from, target) {
            Ok(()) => {
                let _ = std::fs::remove_file(&backup);
                Ok(())
            }
            Err(error) => {
                let _ = std::fs::rename(&backup, target);
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_relative_splits_both_separators() {
        let base = PathBuf::from("/base");
        assert_eq!(
            join_relative(base.clone(), ".claude/skills"),
            PathBuf::from("/base").join(".claude").join("skills")
        );
        assert_eq!(
            join_relative(base.clone(), r"AppData\Roaming\npm"),
            PathBuf::from("/base").join("AppData").join("Roaming").join("npm")
        );
        // Mixed separators and empty segments collapse cleanly.
        assert_eq!(
            join_relative(base, "a//b\\c"),
            PathBuf::from("/base").join("a").join("b").join("c")
        );
    }

    #[test]
    fn home_dir_is_not_empty() {
        assert!(!home_dir().as_os_str().is_empty());
    }

    #[test]
    fn replace_file_writes_new_target() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("new");
        let target = dir.path().join("target");
        std::fs::write(&from, b"new-content").unwrap();
        replace_file(&from, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new-content");
        assert!(!from.exists());
    }

    #[test]
    fn replace_file_overwrites_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("new");
        let target = dir.path().join("target");
        std::fs::write(&target, b"old-content").unwrap();
        std::fs::write(&from, b"new-content").unwrap();
        replace_file(&from, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new-content");
        // No backup files left behind.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}

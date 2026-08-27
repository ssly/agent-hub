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

/// Normalize a project/cwd path for session UI, path filters, and resume `cd`.
///
/// Agents persist paths in platform-specific shapes. On Windows that often
/// includes extended-length prefixes (`\\?\`), `file://` URIs, or POSIX-style
/// drive roots (`/D:/Coding`). This returns a stable display form or `None`
/// when the value is empty/whitespace only.
///
/// Unix absolute paths without a drive letter are left largely intact so macOS
/// session paths keep working.
pub fn normalize_project_path_display(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_uri = strip_file_uri(trimmed);
    let decoded = percent_decode_path(&without_uri);
    let normalized = normalize_windows_path_shape(&decoded);
    let cleaned = normalized.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

/// Compare two project paths for equality after normalization.
///
/// Handles differences in trailing slashes, URI schemes (`file://`), percent-encoding,
/// Windows backslashes / drive roots, and case-insensitivity on Windows.
pub fn paths_match(a: &str, b: &str) -> bool {
    let norm_a = normalize_project_path_display(a).unwrap_or_else(|| a.trim().to_string());
    let norm_b = normalize_project_path_display(b).unwrap_or_else(|| b.trim().to_string());
    if norm_a.is_empty() || norm_b.is_empty() {
        return norm_a == norm_b;
    }
    let clean_a = norm_a.trim_end_matches(['/', '\\']);
    let clean_b = norm_b.trim_end_matches(['/', '\\']);
    #[cfg(target_os = "windows")]
    {
        clean_a.eq_ignore_ascii_case(clean_b)
    }
    #[cfg(not(target_os = "windows"))]
    {
        clean_a == clean_b
    }
}

fn strip_file_uri(input: &str) -> String {
    let Some(after_scheme) = input
        .strip_prefix("file://")
        .or_else(|| input.strip_prefix("FILE://"))
    else {
        return input.to_string();
    };
    // file:///C:/x → /C:/x ; file://localhost/C:/x → /C:/x after localhost strip
    let after_host = after_scheme
        .strip_prefix("localhost")
        .or_else(|| after_scheme.strip_prefix("LOCALHOST"))
        .unwrap_or(after_scheme);
    after_host.to_string()
}

fn percent_decode_path(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn normalize_windows_path_shape(input: &str) -> String {
    let mut s = input.to_string();

    // Extended-length / device prefixes from Win32 APIs (Codex cwd etc.).
    // \\?\C:\Users\…  //?/C:/Users/…  \?\C:\Users\… (sometimes shown that way)
    const EXTENDED: &[&str] = &[r"\\?\", r"//?/", r"\?\", r"/?/"];
    for prefix in EXTENDED {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
            break;
        }
    }
    // \\?\UNC\server\share → \\server\share
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        s = format!(r"\\{rest}");
    } else if let Some(rest) = s.strip_prefix("//?/UNC/") {
        s = format!(r"\\{}", rest.replace('/', "\\"));
    }

    // POSIX drive root: /D:/Coding or /D:\Coding → D:/… or D:\…
    if looks_like_posix_drive_root(&s) {
        s = s[1..].to_string();
    }

    // MSYS/Git-Bash: /c/Users/… → C:\Users\…
    if looks_like_msys_drive_path(&s) {
        let drive = (s.as_bytes()[1] as char).to_ascii_uppercase();
        let rest = s[3..].replace('/', "\\");
        return format!("{drive}:\\{rest}");
    }

    if is_windows_path_like(&s) {
        s = s.replace('/', "\\");
        // Uppercase drive letter: c:\… → C:\…
        if s.len() >= 2 && s.as_bytes()[1] == b':' {
            let mut chars: Vec<char> = s.chars().collect();
            chars[0] = chars[0].to_ascii_uppercase();
            s = chars.into_iter().collect();
        }
        s = collapse_win_backslashes(&s);
    }

    s
}

fn looks_like_posix_drive_root(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':'
}

fn looks_like_msys_drive_path(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b'/'
}

fn is_windows_path_like(s: &str) -> bool {
    let b = s.as_bytes();
    // C:\ or C:/
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return true;
    }
    // UNC \\server\share
    if s.starts_with(r"\\") || s.starts_with("//") {
        return true;
    }
    false
}

/// Collapse duplicate `\` except keep a leading UNC `\\`.
fn collapse_win_backslashes(s: &str) -> String {
    let unc = s.starts_with(r"\\");
    let body = if unc { &s[2..] } else { s };
    let mut out = String::with_capacity(s.len());
    if unc {
        out.push_str(r"\\");
    }
    let mut prev_sep = false;
    for ch in body.chars() {
        if ch == '\\' {
            if !prev_sep {
                out.push('\\');
            }
            prev_sep = true;
        } else {
            out.push(ch);
            prev_sep = false;
        }
    }
    out
}

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
    fn normalize_strips_extended_prefix() {
        assert_eq!(
            normalize_project_path_display(r"\\?\C:\Users\liuyang\.codex\worktrees\x"),
            Some(r"C:\Users\liuyang\.codex\worktrees\x".to_string())
        );
        assert_eq!(
            normalize_project_path_display(r"\?\C:\Users\liuyang"),
            Some(r"C:\Users\liuyang".to_string())
        );
    }

    #[test]
    fn normalize_posix_drive_and_file_uri() {
        assert_eq!(
            normalize_project_path_display("/D:/Coding/portal-master-web"),
            Some(r"D:\Coding\portal-master-web".to_string())
        );
        assert_eq!(
            normalize_project_path_display("file:///D:/Task"),
            Some(r"D:\Task".to_string())
        );
        assert_eq!(
            normalize_project_path_display("file:///C:/Users/liuyang/Documents"),
            Some(r"C:\Users\liuyang\Documents".to_string())
        );
        assert_eq!(
            normalize_project_path_display(r"c:\Users\liuyang\Downloads"),
            Some(r"C:\Users\liuyang\Downloads".to_string())
        );
    }

    #[test]
    fn normalize_msys_and_unix_unchanged() {
        assert_eq!(
            normalize_project_path_display("/c/Users/demo/proj"),
            Some(r"C:\Users\demo\proj".to_string())
        );
        assert_eq!(
            normalize_project_path_display("/Users/demo/projects/app"),
            Some("/Users/demo/projects/app".to_string())
        );
        assert_eq!(normalize_project_path_display("   "), None);
    }

    #[test]
    fn paths_match_handles_slashes_and_schemes() {
        assert!(paths_match("/Users/demo/app", "/Users/demo/app"));
        assert!(paths_match("/Users/demo/app/", "/Users/demo/app"));
        assert!(paths_match("file:///Users/demo/app", "/Users/demo/app/"));
        assert!(paths_match(
            "file:///Users/demo/my%20app",
            "/Users/demo/my app"
        ));
        assert!(!paths_match("/Users/demo/app1", "/Users/demo/app2"));
    }

    #[test]
    fn join_relative_splits_both_separators() {
        let base = PathBuf::from("/base");
        assert_eq!(
            join_relative(base.clone(), ".claude/skills"),
            PathBuf::from("/base").join(".claude").join("skills")
        );
        assert_eq!(
            join_relative(base.clone(), r"AppData\Roaming\npm"),
            PathBuf::from("/base")
                .join("AppData")
                .join("Roaming")
                .join("npm")
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

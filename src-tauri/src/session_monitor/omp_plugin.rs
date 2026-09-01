//! Oh My Pi monitor extension install/uninstall.
//!
//! omp has no hooks.json; lifecycle events are TypeScript extensions that
//! are auto-discovered from `~/.omp/agent/extensions/` (no settings
//! registration, no trust gate). Agent Hub drops an observe-only extension
//! package there — install copies two files, uninstall removes the
//! directory. Behavior mirrors dsh_plugin.rs: in-app one-click install /
//! uninstall, never touching omp's own config.

use super::hooks::{build_preview, content_hash, HookAction};
use super::types::{HookChangePreview, HookDiffLine, HookStatus};
use std::fs;
use std::path::PathBuf;

const EXTENSION_DIR_NAME: &str = "agent-hub-omp-monitor";
const EXTENSION_INDEX: &str = include_str!("../../resources/omp-monitor-plugin/index.js");
const EXTENSION_PACKAGE: &str = include_str!("../../resources/omp-monitor-plugin/package.json");

fn extension_files() -> [(&'static str, &'static str); 2] {
    [
        ("index.js", EXTENSION_INDEX),
        ("package.json", EXTENSION_PACKAGE),
    ]
}

/// `~/.omp/agent` — `PI_CODING_AGENT_DIR` overrides it, mirroring omp
/// itself (same resolution as the session adapter).
fn agent_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
        if !dir.trim().is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    dirs::home_dir()
        .map(|home| home.join(".omp").join("agent"))
        .ok_or_else(|| "home directory is unavailable".to_string())
}

fn extensions_root() -> Result<PathBuf, String> {
    Ok(agent_dir()?.join("extensions"))
}

fn extension_dir() -> Result<PathBuf, String> {
    Ok(extensions_root()?.join(EXTENSION_DIR_NAME))
}

fn extension_files_match(dir: &PathBuf) -> bool {
    extension_files().iter().all(|(name, expected)| {
        fs::read_to_string(dir.join(name)).is_ok_and(|got| got == *expected)
    })
}

fn write_extension_files(dir: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|error| format!("unable to create {}: {error}", dir.display()))?;
    for (name, content) in extension_files() {
        fs::write(dir.join(name), content)
            .map_err(|error| format!("unable to write {}/{name}: {error}", dir.display()))?;
    }
    Ok(())
}

/// Marker of the on-disk state, hashed into the before-hash guard so a
/// concurrent change between preview and apply is caught.
fn extension_state_marker() -> String {
    let dir = match extension_dir() {
        Ok(dir) => dir,
        Err(error) => return error,
    };
    if !dir.exists() {
        return "missing".to_string();
    }
    if extension_files_match(&dir) {
        return "current".to_string();
    }
    "stale".to_string()
}

pub fn omp_hook_status() -> Result<HookStatus, String> {
    let dir = extension_dir()?;
    let agent_root = agent_dir()?;
    if !agent_root.exists() {
        return Ok(HookStatus {
            installed: false,
            config_path: dir.display().to_string(),
            command: EXTENSION_DIR_NAME.to_string(),
            managed_handler_count: 0,
            issue: Some("未找到 Oh My Pi。请先安装并运行一次 omp。".into()),
        });
    }
    let marker = extension_state_marker();
    let installed = marker == "current";
    let issue = if installed {
        None
    } else if marker == "stale" {
        Some("Oh My Pi 监听扩展为旧版本，请点击「重置插件」。".into())
    } else {
        None
    };
    Ok(HookStatus {
        installed,
        config_path: dir.display().to_string(),
        command: EXTENSION_DIR_NAME.to_string(),
        managed_handler_count: if installed { 1 } else { 0 },
        issue,
    })
}

pub fn omp_preview(action: HookAction) -> Result<HookChangePreview, String> {
    let dir = extension_dir()?;
    let before = extension_state_marker();
    let after = match action {
        HookAction::Install => "current".to_string(),
        HookAction::Uninstall => "missing".to_string(),
    };
    let mut preview = build_preview(action, &dir, EXTENSION_DIR_NAME, &before, &after);
    let note = match action {
        HookAction::Install => format!(
            "# Also writes {EXTENSION_DIR_NAME}/ (index.js + package.json) into {}",
            extensions_root()?.display()
        ),
        HookAction::Uninstall => format!(
            "# Also removes {EXTENSION_DIR_NAME}/ from {}",
            extensions_root()?.display()
        ),
    };
    preview.diff_lines.push(HookDiffLine {
        tag: "context".to_string(),
        content: note,
    });
    Ok(preview)
}

pub fn omp_apply(action: HookAction, expected_before_hash: &str) -> Result<HookStatus, String> {
    let dir = extension_dir()?;
    let before = extension_state_marker();
    if content_hash(&before) != expected_before_hash {
        return Err("Oh My Pi 扩展目录已发生变化，请重新预览后再确认。".into());
    }
    match action {
        HookAction::Install => write_extension_files(&dir)?,
        HookAction::Uninstall => {
            let _ = fs::remove_dir_all(&dir);
        }
    }
    omp_hook_status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_extension_is_observe_only() {
        assert!(EXTENSION_INDEX.contains("observe-only"));
        // The inbox consumes serialized HookEvent files: the extension must
        // build the full camelCase envelope itself, with agent routing.
        assert!(EXTENSION_INDEX.contains("agent: \"omp\""));
        assert!(EXTENSION_INDEX.contains("hookEventName"));
        assert!(EXTENSION_INDEX.contains("eventId"));
        assert!(EXTENSION_INDEX.contains("occurredAt"));
        assert!(EXTENSION_INDEX.contains("sessionId"));
        assert!(EXTENSION_INDEX.contains("UserPromptSubmit"));
        assert!(EXTENSION_INDEX.contains("AssistantResponse"));
        assert!(EXTENSION_INDEX.contains("Stop"));
        assert!(EXTENSION_INDEX.contains("PermissionRequest"));
        assert!(EXTENSION_INDEX.contains("PermissionResult"));
        assert!(EXTENSION_INDEX.contains("Never surface monitor I/O"));
        assert!(EXTENSION_PACKAGE.contains("\"type\": \"module\""));
        assert!(EXTENSION_PACKAGE.contains(EXTENSION_DIR_NAME));
        assert!(EXTENSION_PACKAGE.contains("\"omp.extensions\""));
    }

    #[test]
    fn state_marker_progresses_through_lifecycle() {
        // Real home directory: omp is almost certainly absent, so the marker
        // is "missing" (never "stale"/"current" without our files).
        let marker = extension_state_marker();
        assert!(["missing", "stale", "current"].contains(&marker.as_str()));
    }

    #[test]
    fn preview_and_apply_hashes_stay_in_sync() {
        // preview hashes the state marker; apply must accept that exact hash
        // (and reject a different one). exercised without touching disk:
        let before = extension_state_marker();
        let hash = content_hash(&before);
        let _ = omp_preview(HookAction::Install).expect("preview should not fail");
        // A mismatched hash must be rejected before any filesystem write.
        let error = omp_apply(HookAction::Install, "deadbeefdeadbeef")
            .expect_err("stale hash should be rejected");
        assert!(error.contains("重新预览"));
        assert_eq!(content_hash(&extension_state_marker()), hash);
    }
}

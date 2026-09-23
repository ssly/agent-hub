//! OpenCode monitor extension install/uninstall.
//!
//! OpenCode has no hooks.json; lifecycle events are plugins discovered from
//! `~/.config/opencode/plugins/`. Agent Hub drops an observe-only extension
//! package there — install copies two files, uninstall removes the
//! directory. Behavior mirrors omp_plugin.rs: in-app one-click install /
//! uninstall, never touching OpenCode's own config.

use super::hooks::{build_preview, content_hash, HookAction};
use super::types::{HookChangePreview, HookDiffLine, HookStatus};
use std::fs;
use std::path::PathBuf;

const EXTENSION_DIR_NAME: &str = "agent-hub-opencode-monitor";
const EXTENSION_INDEX: &str = include_str!("../../resources/opencode-monitor-plugin/index.js");
const EXTENSION_PACKAGE: &str = include_str!("../../resources/opencode-monitor-plugin/package.json");

fn extension_files() -> [(&'static str, &'static str); 2] {
    [
        ("index.js", EXTENSION_INDEX),
        ("package.json", EXTENSION_PACKAGE),
    ]
}

fn config_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".config").join("opencode"))
        .ok_or_else(|| "home directory is unavailable".to_string())
}

fn plugins_root() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("plugins"))
}

fn extension_dir() -> Result<PathBuf, String> {
    Ok(plugins_root()?.join(EXTENSION_DIR_NAME))
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

pub fn opencode_hook_status() -> Result<HookStatus, String> {
    let dir = extension_dir()?;
    let config_root = config_dir()?;
    let data_root = dirs::home_dir()
        .map(|home| home.join(".local").join("share").join("opencode"))
        .unwrap_or_else(|| config_root.clone());

    if !config_root.exists() && !data_root.exists() {
        return Ok(HookStatus {
            installed: false,
            config_path: dir.display().to_string(),
            command: EXTENSION_DIR_NAME.to_string(),
            managed_handler_count: 0,
            issue: Some("未找到 OpenCode。请先安装并运行一次 opencode。".into()),
        });
    }
    let marker = extension_state_marker();
    let installed = marker == "current";
    let issue = if installed {
        None
    } else if marker == "stale" {
        Some("OpenCode 监听扩展为旧版本，请点击「重置插件」。".into())
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

pub fn opencode_preview(action: HookAction) -> Result<HookChangePreview, String> {
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
            plugins_root()?.display()
        ),
        HookAction::Uninstall => format!(
            "# Also removes {EXTENSION_DIR_NAME}/ from {}",
            plugins_root()?.display()
        ),
    };
    preview.diff_lines.push(HookDiffLine {
        tag: "context".to_string(),
        content: note,
    });
    Ok(preview)
}

pub fn opencode_apply(action: HookAction, expected_before_hash: &str) -> Result<HookStatus, String> {
    let dir = extension_dir()?;
    let before = extension_state_marker();
    if content_hash(&before) != expected_before_hash {
        return Err("OpenCode 扩展目录已发生变化，请重新预览后再确认。".into());
    }
    match action {
        HookAction::Install => write_extension_files(&dir)?,
        HookAction::Uninstall => {
            let _ = fs::remove_dir_all(&dir);
        }
    }
    opencode_hook_status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_extension_is_observe_only() {
        assert!(EXTENSION_INDEX.contains("observe-only"));
        assert!(EXTENSION_INDEX.contains("agent: \"opencode\""));
        assert!(EXTENSION_INDEX.contains("hookEventName"));
        assert!(EXTENSION_INDEX.contains("eventId"));
        assert!(EXTENSION_INDEX.contains("occurredAt"));
        assert!(EXTENSION_INDEX.contains("sessionId"));
        assert!(EXTENSION_INDEX.contains("UserPromptSubmit"));
        assert!(EXTENSION_INDEX.contains("Stop"));
        assert!(EXTENSION_INDEX.contains("PermissionRequest"));
        assert!(EXTENSION_INDEX.contains("PermissionResult"));
        assert!(EXTENSION_INDEX.contains("Never surface monitor I/O"));
        assert!(EXTENSION_PACKAGE.contains("\"type\": \"module\""));
        assert!(EXTENSION_PACKAGE.contains(EXTENSION_DIR_NAME));
    }

    #[test]
    fn state_marker_progresses_through_lifecycle() {
        let marker = extension_state_marker();
        assert!(["missing", "stale", "current"].contains(&marker.as_str()));
    }

    #[test]
    fn preview_and_apply_hashes_stay_in_sync() {
        let before = extension_state_marker();
        let hash = content_hash(&before);
        let _ = opencode_preview(HookAction::Install).expect("preview should not fail");
        let error = opencode_apply(HookAction::Install, "deadbeefdeadbeef")
            .expect_err("stale hash should be rejected");
        assert!(error.contains("重新预览"));
        assert_eq!(content_hash(&extension_state_marker()), hash);
    }
}

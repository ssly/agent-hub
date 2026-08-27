//! DeepSeek Harness monitor plugin install/uninstall.
//!
//! DSH has no user-level hooks.json. Agent Hub drops an observe-only Cordis
//! plugin into each profile's node_modules and inserts a managed row into
//! that profile's `cordis.patch.yml`. The Web UI hot-watches the patch;
//! the user never has to open dsh Settings.

use super::hooks::{build_preview, content_hash, HookAction};
use super::types::{HookChangePreview, HookDiffLine, HookStatus};
use crate::session::dsh::dsh_home;
use serde_yaml::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const PLUGIN_ID: &str = "agent-hub-monitor";
const PLUGIN_NAME: &str = "agent-hub-dsh-monitor";
const PLUGIN_INDEX: &str = include_str!("../../resources/dsh-monitor-plugin/index.js");
const PLUGIN_PACKAGE: &str = include_str!("../../resources/dsh-monitor-plugin/package.json");
const PLUGIN_BUNDLE_PATCH: &str =
    include_str!("../../resources/dsh-monitor-plugin/cordis.patch.yml");

fn plugin_files() -> [(&'static str, &'static str); 3] {
    [
        ("index.js", PLUGIN_INDEX),
        ("package.json", PLUGIN_PACKAGE),
        ("cordis.patch.yml", PLUGIN_BUNDLE_PATCH),
    ]
}

fn profiles_root() -> Result<PathBuf, String> {
    dsh_home()
        .map(|home| home.join("profiles"))
        .ok_or_else(|| "home directory is unavailable".to_string())
}

/// Profile directories that look like a dsh profile (have package.json).
pub fn dsh_profile_dirs() -> Result<Vec<PathBuf>, String> {
    let root = profiles_root()?;
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut dirs = Vec::new();
    let entries = fs::read_dir(&root)
        .map_err(|error| format!("unable to read {}: {error}", root.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "node_modules" || name.starts_with('.') {
            continue;
        }
        if path.join("package.json").is_file() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn preferred_profile_dir() -> Result<Option<PathBuf>, String> {
    let dirs = dsh_profile_dirs()?;
    if dirs.is_empty() {
        return Ok(None);
    }
    let web = dirs.iter().find(|dir| {
        dir.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "web")
    });
    Ok(Some(web.cloned().unwrap_or_else(|| dirs[0].clone())))
}

fn patch_path(profile: &Path) -> PathBuf {
    profile.join("cordis.patch.yml")
}

fn plugin_dir(profile: &Path) -> PathBuf {
    profile.join("node_modules").join(PLUGIN_NAME)
}

/// Hoisted fallback used by Node's parent-walk from
/// `$DSH_HOME/profiles/node_modules/@deepseek-ai/...` (bare `import(name)`
/// from the Cordis loader) and from a profile directory.
fn hoisted_plugin_dir() -> Result<PathBuf, String> {
    Ok(profiles_root()?.join("node_modules").join(PLUGIN_NAME))
}

fn plugin_installed_on_disk(profile: &Path) -> bool {
    plugin_files_match(&plugin_dir(profile))
        || hoisted_plugin_dir().is_ok_and(|dir| plugin_files_match(&dir))
}

fn read_existing(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("unable to read {}: {error}", path.display())),
    }
}

fn our_insert_row() -> Value {
    serde_yaml::from_str(&format!("id: {PLUGIN_ID}\nname: {PLUGIN_NAME}\n"))
        .expect("static insert row")
}

fn is_our_row(value: &Value) -> bool {
    value
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| id == PLUGIN_ID)
}

fn entry_inserts_ours(entry: &Value) -> bool {
    entry
        .get("insert")
        .and_then(Value::as_sequence)
        .is_some_and(|rows| rows.iter().any(is_our_row))
}

fn parse_patch(content: &str) -> Result<Vec<Value>, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_yaml::from_str(content)
        .map_err(|error| format!("cordis.patch.yml 不是有效 YAML：{error}"))?;
    match value {
        Value::Sequence(items) => Ok(items),
        Value::Null => Ok(Vec::new()),
        _ => Err("cordis.patch.yml 的根节点必须是 YAML 数组，已停止变更以保护原配置。".into()),
    }
}

pub fn render_patch(action: HookAction, before: &str) -> Result<String, String> {
    let mut items = parse_patch(before)?;
    items.retain(|entry| !entry_inserts_ours(entry));
    if action == HookAction::Install {
        items.push(Value::Mapping({
            let mut map = serde_yaml::Mapping::new();
            map.insert(
                Value::String("insert".into()),
                Value::Sequence(vec![our_insert_row()]),
            );
            map
        }));
    }
    if items.is_empty() {
        return Ok("# Your patch layer for this dsh profile.\n[]\n".into());
    }
    let mut rendered = serde_yaml::to_string(&Value::Sequence(items))
        .map_err(|error| format!("unable to serialize cordis.patch.yml: {error}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn plugin_files_match(dir: &Path) -> bool {
    plugin_files().iter().all(|(name, expected)| {
        fs::read_to_string(dir.join(name)).is_ok_and(|got| got == *expected)
    })
}

fn write_plugin_files(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|error| format!("unable to create {}: {error}", dir.display()))?;
    for (name, content) in plugin_files() {
        fs::write(dir.join(name), content)
            .map_err(|error| format!("unable to write {}/{name}: {error}", dir.display()))?;
    }
    Ok(())
}

fn remove_plugin_files(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

pub fn dsh_hook_status() -> Result<HookStatus, String> {
    let Some(profile) = preferred_profile_dir()? else {
        return Ok(HookStatus {
            installed: false,
            config_path: profiles_root()?
                .join("web")
                .join("cordis.patch.yml")
                .display()
                .to_string(),
            command: PLUGIN_NAME.to_string(),
            managed_handler_count: 0,
            issue: Some(
                "未找到 DeepSeek Harness profile。请先启动一次 dsh（例如 `dsh web`）。".into(),
            ),
        });
    };
    let path = patch_path(&profile);
    let content = read_existing(&path)?;
    let items = parse_patch(&content).unwrap_or_default();
    let in_patch = items.iter().any(entry_inserts_ours);
    let files_ok = plugin_installed_on_disk(&profile);
    let installed = in_patch && files_ok;
    let issue = if installed {
        None
    } else if in_patch
        || plugin_dir(&profile).exists()
        || hoisted_plugin_dir().is_ok_and(|dir| dir.exists())
    {
        Some("DeepSeek Harness 监听插件为旧版本，请点击「重置插件」。".into())
    } else {
        None
    };
    Ok(HookStatus {
        installed,
        config_path: path.display().to_string(),
        command: PLUGIN_NAME.to_string(),
        managed_handler_count: if in_patch { 1 } else { 0 },
        issue,
    })
}

pub fn dsh_preview(action: HookAction) -> Result<HookChangePreview, String> {
    let Some(profile) = preferred_profile_dir()? else {
        return Err("未找到 DeepSeek Harness profile。请先启动一次 dsh。".into());
    };
    let path = patch_path(&profile);
    let before = read_existing(&path)?;
    let after = render_patch(action, &before)?;
    let mut preview = build_preview(action, &path, PLUGIN_NAME, &before, &after);
    let note = match action {
        HookAction::Install => format!(
            "# Also copies {PLUGIN_NAME} into {}/node_modules/ and ~/.dsh/profiles/node_modules/",
            profile.display()
        ),
        HookAction::Uninstall => format!(
            "# Also removes {PLUGIN_NAME} from {}/node_modules/ and ~/.dsh/profiles/node_modules/",
            profile.display()
        ),
    };
    preview.diff_lines.push(HookDiffLine {
        tag: "context".to_string(),
        content: note,
    });
    Ok(preview)
}

pub fn dsh_apply(action: HookAction, expected_before_hash: &str) -> Result<HookStatus, String> {
    let dirs = dsh_profile_dirs()?;
    if dirs.is_empty() {
        return Err("未找到 DeepSeek Harness profile。请先启动一次 dsh。".into());
    }
    let primary = preferred_profile_dir()?.expect("non-empty profile list");
    let path = patch_path(&primary);
    let before = read_existing(&path)?;
    if content_hash(&before) != expected_before_hash {
        return Err("DeepSeek Harness 配置文件已发生变化，请重新预览后再确认。".into());
    }

    match action {
        HookAction::Install => write_plugin_files(&hoisted_plugin_dir()?)?,
        HookAction::Uninstall => {
            if let Ok(dir) = hoisted_plugin_dir() {
                remove_plugin_files(&dir);
            }
        }
    }

    for profile in &dirs {
        let patch = patch_path(profile);
        let current = read_existing(&patch)?;
        let after = render_patch(action, &current)?;
        if current != after {
            atomic_write_text(&patch, &after)?;
        }
        match action {
            HookAction::Install => write_plugin_files(&plugin_dir(profile))?,
            HookAction::Uninstall => remove_plugin_files(&plugin_dir(profile)),
        }
    }
    dsh_hook_status()
}

fn atomic_write_text(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("unable to create {}: {error}", parent.display()))?;
    }
    let temp_path = path.with_extension("yml.agent-hub-tmp");
    {
        let mut file = fs::File::create(&temp_path)
            .map_err(|error| format!("unable to create {}: {error}", temp_path.display()))?;
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("unable to write {}: {error}", temp_path.display()))?;
    }
    if let Err(error) = crate::paths::replace_file(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("unable to replace {}: {error}", path.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_into_empty_patch_writes_managed_insert() {
        let after = render_patch(HookAction::Install, "[]\n").unwrap();
        assert!(after.contains("agent-hub-monitor"));
        assert!(after.contains(PLUGIN_NAME));
        let items = parse_patch(&after).unwrap();
        assert_eq!(items.len(), 1);
        assert!(entry_inserts_ours(&items[0]));
    }

    #[test]
    fn install_preserves_foreign_inserts() {
        let before = "- insert:\n    - id: other\n      name: someone-else\n";
        let after = render_patch(HookAction::Install, before).unwrap();
        assert!(after.contains("someone-else"));
        assert!(after.contains(PLUGIN_ID));
        let items = parse_patch(&after).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn uninstall_removes_only_managed_insert() {
        let installed = render_patch(HookAction::Install, "[]\n").unwrap();
        let after = render_patch(HookAction::Uninstall, &installed).unwrap();
        assert!(!after.contains(PLUGIN_ID));
        assert!(after.contains("[]"));
    }

    #[test]
    fn install_is_idempotent() {
        let once = render_patch(HookAction::Install, "[]\n").unwrap();
        let twice = render_patch(HookAction::Install, &once).unwrap();
        let items = parse_patch(&twice).unwrap();
        assert_eq!(items.iter().filter(|e| entry_inserts_ours(e)).count(), 1);
    }

    #[test]
    fn rejects_non_sequence_root() {
        let error = render_patch(HookAction::Install, "foo: bar\n").unwrap_err();
        assert!(error.contains("数组"));
    }

    #[test]
    fn bundled_plugin_is_observe_only() {
        assert!(PLUGIN_INDEX.contains("agent/pre-step"));
        assert!(PLUGIN_INDEX.contains("agent/turn-stopping"));
        assert!(PLUGIN_INDEX.contains("tools/pre-execute"));
        assert!(PLUGIN_INDEX.contains("PermissionRequest"));
        assert!(PLUGIN_INDEX.contains("origin === 'subagent'"));
        assert!(PLUGIN_INDEX.contains("Never surface monitor I/O"));
        assert!(PLUGIN_PACKAGE.contains("\"type\": \"module\""));
        assert!(PLUGIN_BUNDLE_PATCH.contains(PLUGIN_ID));
    }

    #[test]
    fn install_into_commented_empty_official_patch() {
        let before = "# Your patch layer for this dsh profile, applied after every bundle layer:\n\
# a top-level YAML array of loader patch entries (id-targeted config\n\
# overrides, disables, and insert lists; `!!js` expressions allowed).\n\
[]\n";
        let after = render_patch(HookAction::Install, before).unwrap();
        let items = parse_patch(&after).unwrap();
        assert_eq!(items.len(), 1);
        assert!(entry_inserts_ours(&items[0]));
    }

    #[test]
    fn plugin_files_round_trip_on_disk() {
        let dir =
            std::env::temp_dir().join(format!("agent-hub-dsh-plugin-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        write_plugin_files(&dir).unwrap();
        assert!(plugin_files_match(&dir));
        remove_plugin_files(&dir);
        assert!(!dir.exists());
    }
}

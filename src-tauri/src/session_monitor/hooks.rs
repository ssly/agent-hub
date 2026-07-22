use super::capture::HOOK_ARG;
use super::types::{CodexHookChangePreview, CodexHookStatus, HookDiffLine};
use serde_json::{json, Map, Value};
use similar::{ChangeTag, TextDiff};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
const STOP: &str = "Stop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookAction {
    Install,
    Uninstall,
}

impl HookAction {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "install" => Ok(Self::Install),
            "uninstall" => Ok(Self::Uninstall),
            _ => Err(format!("unsupported hook action: {value}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
        }
    }
}

pub fn get_hook_status() -> Result<CodexHookStatus, String> {
    let path = codex_hooks_path()?;
    let command = expected_command()?;
    if !path.exists() {
        return Ok(CodexHookStatus {
            installed: false,
            config_path: path.display().to_string(),
            command,
            managed_handler_count: 0,
            issue: None,
        });
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("unable to read {}: {error}", path.display()))?;
    let root = parse_root(&content, &path)?;
    let prompt_commands = managed_commands_for(&root, USER_PROMPT_SUBMIT);
    let stop_commands = managed_commands_for(&root, STOP);
    let managed_handler_count = managed_command_count(&root);
    let installed = managed_handler_count == 2
        && prompt_commands.len() == 1
        && stop_commands.len() == 1
        && prompt_commands[0] == command
        && stop_commands[0] == command;
    let issue = if installed || managed_handler_count == 0 {
        None
    } else {
        Some("Codex Hook 配置不完整或命令路径已变化，可重新安装进行修复。".to_string())
    };

    Ok(CodexHookStatus {
        installed,
        config_path: path.display().to_string(),
        command,
        managed_handler_count,
        issue,
    })
}

pub fn preview_hook_change(action: HookAction) -> Result<CodexHookChangePreview, String> {
    let path = codex_hooks_path()?;
    let command = expected_command()?;
    let before = read_existing(&path)?;
    build_preview(action, &path, &command, &before)
}

pub fn apply_hook_change(
    action: HookAction,
    expected_before_hash: &str,
) -> Result<CodexHookStatus, String> {
    let path = codex_hooks_path()?;
    let command = expected_command()?;
    let before = read_existing(&path)?;
    if content_hash(&before) != expected_before_hash {
        return Err("Codex Hook 配置文件已发生变化，请重新预览后再确认。".to_string());
    }

    let preview = build_preview(action, &path, &command, &before)?;
    if preview.changed {
        let after = render_after(action, &path, &command, &before)?;
        if action == HookAction::Install {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("unable to create {}: {error}", parent.display()))?;
            }
        }
        let write_path = resolve_write_target(&path)?;
        atomic_write(&write_path, after.as_bytes(), expected_before_hash)?;
    }
    get_hook_status()
}

fn build_preview(
    action: HookAction,
    path: &Path,
    command: &str,
    before: &str,
) -> Result<CodexHookChangePreview, String> {
    let after = render_after(action, path, command, before)?;
    let mut diff_lines = Vec::new();
    let mut added = 0;
    let mut removed = 0;
    for change in TextDiff::from_lines(before, &after).iter_all_changes() {
        let tag = match change.tag() {
            ChangeTag::Delete => {
                removed += 1;
                "removed"
            }
            ChangeTag::Insert => {
                added += 1;
                "added"
            }
            ChangeTag::Equal => "context",
        };
        diff_lines.push(HookDiffLine {
            tag: tag.to_string(),
            content: change.value().trim_end_matches(['\r', '\n']).to_string(),
        });
    }

    Ok(CodexHookChangePreview {
        action: action.as_str().to_string(),
        config_path: path.display().to_string(),
        command: command.to_string(),
        before_hash: content_hash(before),
        changed: before != after,
        diff_lines,
        added,
        removed,
    })
}

fn render_after(
    action: HookAction,
    path: &Path,
    command: &str,
    before: &str,
) -> Result<String, String> {
    if action == HookAction::Uninstall && before.is_empty() {
        return Ok(String::new());
    }
    let mut root = if before.is_empty() {
        Value::Object(Map::new())
    } else {
        parse_root(before, path)?
    };
    remove_managed_handlers(&mut root)?;
    if action == HookAction::Install {
        append_managed_handler(&mut root, USER_PROMPT_SUBMIT, command)?;
        append_managed_handler(&mut root, STOP, command)?;
    }
    let mut rendered = serde_json::to_string_pretty(&root)
        .map_err(|error| format!("unable to serialize Codex Hook config: {error}"))?;
    rendered.push('\n');
    Ok(rendered)
}

fn parse_root(content: &str, path: &Path) -> Result<Value, String> {
    let root: Value = serde_json::from_str(content)
        .map_err(|error| format!("{} 不是有效 JSON：{error}", path.display()))?;
    if !root.is_object() {
        return Err(format!("{} 的根节点必须是 JSON 对象", path.display()));
    }
    Ok(root)
}

fn append_managed_handler(root: &mut Value, event_name: &str, command: &str) -> Result<(), String> {
    let root_object = root
        .as_object_mut()
        .ok_or_else(|| "Codex Hook config root is not an object".to_string())?;
    let hooks = root_object
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| "hooks 字段必须是 JSON 对象，已停止安装以保护原配置。".to_string())?;
    let groups = hooks
        .entry(event_name.to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| format!("hooks.{event_name} 必须是数组，已停止安装以保护原配置。"))?;
    groups.push(json!({
        "hooks": [{
            "type": "command",
            "command": command,
            "timeout": 10
        }]
    }));
    Ok(())
}

fn remove_managed_handlers(root: &mut Value) -> Result<(), String> {
    let Some(root_object) = root.as_object_mut() else {
        return Err("Codex Hook config root is not an object".to_string());
    };
    let Some(hooks_value) = root_object.get_mut("hooks") else {
        return Ok(());
    };
    let hooks = hooks_value
        .as_object_mut()
        .ok_or_else(|| "hooks 字段必须是 JSON 对象，已停止变更以保护原配置。".to_string())?;

    for (event_name, groups_value) in hooks.iter_mut() {
        let groups = groups_value
            .as_array_mut()
            .ok_or_else(|| format!("hooks.{event_name} 必须是数组，已停止变更以保护原配置。"))?;
        for group in groups.iter_mut() {
            let Some(group_object) = group.as_object_mut() else {
                continue;
            };
            let Some(handlers_value) = group_object.get_mut("hooks") else {
                continue;
            };
            let Some(handlers) = handlers_value.as_array_mut() else {
                continue;
            };
            handlers.retain(|handler| !is_managed_handler(handler));
        }
        groups.retain(|group| {
            let Some(object) = group.as_object() else {
                return true;
            };
            let empty_handlers = object
                .get("hooks")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty);
            !(empty_handlers && object.len() == 1)
        });
    }
    Ok(())
}

fn managed_commands_for(root: &Value, event_name: &str) -> Vec<String> {
    root.get("hooks")
        .and_then(Value::as_object)
        .and_then(|hooks| hooks.get(event_name))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|group| group.get("hooks").and_then(Value::as_array))
        .flatten()
        .filter(|handler| is_managed_handler(handler))
        .filter_map(|handler| handler.get("command").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

fn managed_command_count(root: &Value) -> usize {
    root.get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|hooks| hooks.values())
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|group| group.get("hooks").and_then(Value::as_array))
        .flatten()
        .filter(|handler| is_managed_handler(handler))
        .count()
}

fn is_managed_handler(handler: &Value) -> bool {
    handler.get("type").and_then(Value::as_str) == Some("command")
        && handler
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| command.split_whitespace().any(|part| part == HOOK_ARG))
}

fn expected_command() -> Result<String, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("unable to locate Agent Hub executable: {error}"))?;
    let path = executable.to_string_lossy();
    #[cfg(target_os = "windows")]
    {
        return Ok(format!("\"{}\" {HOOK_ARG}", path.replace('"', "\\\"")));
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(format!("'{}' {HOOK_ARG}", path.replace('\'', "'\\''")))
    }
}

fn codex_hooks_path() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".codex/hooks.json"))
        .ok_or_else(|| "home directory is unavailable".to_string())
}

fn read_existing(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("unable to read {}: {error}", path.display())),
    }
}

fn resolve_write_target(path: &Path) -> Result<PathBuf, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::canonicalize(path).map_err(|error| {
                format!(
                    "unable to resolve Hook config symlink {}: {error}",
                    path.display()
                )
            })
        }
        Ok(_) => Ok(path.to_path_buf()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(format!("unable to inspect {}: {error}", path.display())),
    }
}

fn content_hash(content: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn atomic_write(path: &Path, content: &[u8], expected_before_hash: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    let temp_path = parent.join(format!(".agent-hub-hooks-{}.tmp", Uuid::new_v4()));
    let existing_permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());
    let mut temp = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .map_err(|error| format!("unable to create temporary Hook config: {error}"))?;
    if let Some(permissions) = existing_permissions {
        if let Err(error) = fs::set_permissions(&temp_path, permissions) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "unable to preserve Hook config permissions: {error}"
            ));
        }
    }
    if let Err(error) = temp.write_all(content).and_then(|_| temp.sync_all()) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("unable to persist temporary Hook config: {error}"));
    }

    // Re-check immediately before the atomic replacement. This narrows the
    // race window when Codex or another editor updates hooks.json after the
    // user reviewed the diff.
    let current = match read_existing(path) {
        Ok(current) => current,
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
    };
    if content_hash(&current) != expected_before_hash {
        let _ = fs::remove_file(&temp_path);
        return Err("Codex Hook 配置文件已发生变化，请重新预览后再确认。".to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Err(error) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("unable to replace {}: {error}", path.display()));
        }
    }
    #[cfg(target_os = "windows")]
    {
        let backup_path = parent.join(format!(".agent-hub-hooks-{}.bak", Uuid::new_v4()));
        let had_existing = path.exists();
        if had_existing {
            if let Err(error) = fs::rename(path, &backup_path) {
                let _ = fs::remove_file(&temp_path);
                return Err(format!("unable to stage existing Hook config: {error}"));
            }
        }
        if let Err(error) = fs::rename(&temp_path, path) {
            if had_existing {
                let _ = fs::rename(&backup_path, path);
            }
            let _ = fs::remove_file(&temp_path);
            return Err(format!("unable to replace {}: {error}", path.display()));
        }
        if had_existing {
            let _ = fs::remove_file(backup_path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn original_config() -> &'static str {
        r#"{
  "custom": "keep-me",
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "all",
        "hooks": [
          {
            "type": "command",
            "command": "existing-command"
          }
        ]
      }
    ]
  }
}
"#
    }

    #[test]
    fn install_preserves_unrelated_handlers_and_adds_two_managed_handlers() {
        let path = Path::new("/tmp/hooks.json");
        let after = render_after(
            HookAction::Install,
            path,
            "'/Applications/Agent Hub' --agent-hub-codex-hook",
            original_config(),
        )
        .unwrap();
        let root: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(root["custom"], "keep-me");
        assert_eq!(managed_command_count(&root), 2);
        assert!(after.contains("existing-command"));
    }

    #[test]
    fn uninstall_only_removes_owned_handlers() {
        let path = Path::new("/tmp/hooks.json");
        let installed = render_after(
            HookAction::Install,
            path,
            "agent-hub --agent-hub-codex-hook",
            original_config(),
        )
        .unwrap();
        let after = render_after(
            HookAction::Uninstall,
            path,
            "agent-hub --agent-hub-codex-hook",
            &installed,
        )
        .unwrap();
        let root: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(managed_command_count(&root), 0);
        assert!(after.contains("existing-command"));
        assert_eq!(root["custom"], "keep-me");
    }

    #[test]
    fn malformed_hooks_shape_is_rejected_without_replacement() {
        let error = render_after(
            HookAction::Install,
            Path::new("/tmp/hooks.json"),
            "agent-hub --agent-hub-codex-hook",
            r#"{"hooks":"do-not-touch"}"#,
        )
        .unwrap_err();
        assert!(error.contains("hooks 字段必须是 JSON 对象"));
    }

    #[test]
    fn atomic_write_rejects_a_stale_preview_hash() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("hooks.json");
        fs::write(&path, "changed-after-preview").unwrap();

        let error = atomic_write(&path, b"replacement", &content_hash("old-preview")).unwrap_err();

        assert!(error.contains("已发生变化"));
        assert_eq!(fs::read_to_string(path).unwrap(), "changed-after-preview");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_config_resolves_to_its_target() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("real-hooks.json");
        let link = directory.path().join("hooks.json");
        fs::write(&target, "{}").unwrap();
        symlink(&target, &link).unwrap();

        assert_eq!(
            resolve_write_target(&link).unwrap(),
            fs::canonicalize(target).unwrap()
        );
        assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    }
}

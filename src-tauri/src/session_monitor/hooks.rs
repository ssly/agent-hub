use super::capture::{CLAUDE_HOOK_ARG, CODEX_HOOK_ARG, GROK_HOOK_ARG, KIMI_HOOK_ARG};
use super::types::{AgentKind, HookChangePreview, HookDiffLine, HookStatus};
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

fn hook_arg(agent: AgentKind) -> Result<&'static str, String> {
    match agent {
        AgentKind::Codex => Ok(CODEX_HOOK_ARG),
        AgentKind::Claude => Ok(CLAUDE_HOOK_ARG),
        AgentKind::Grok => Ok(GROK_HOOK_ARG),
        AgentKind::Kimi => Ok(KIMI_HOOK_ARG),
        // Kiro is covered by read-only file watching: stable kiro-cli 2.x
        // does not load hook configs at all, and injecting agent-embedded
        // hooks would change the user's default agent.
        AgentKind::Kiro => Err("Kiro 会话监听基于文件监听，无需安装 Hook。".to_string()),
    }
}

fn config_path(agent: AgentKind) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "home directory is unavailable".to_string())?;
    match agent {
        AgentKind::Codex => Ok(home.join(".codex").join("hooks.json")),
        AgentKind::Claude => Ok(home.join(".claude").join("settings.json")),
        // Grok merges every ~/.grok/hooks/*.json (always trusted, no trust
        // gate), so Agent Hub gets its own managed file instead of editing a
        // shared one.
        AgentKind::Grok => Ok(home.join(".grok").join("hooks").join("agent-hub.json")),
        AgentKind::Kimi => Ok(home.join(".kimi-code").join("config.toml")),
        AgentKind::Kiro => Err("Kiro 会话监听基于文件监听，无需安装 Hook。".to_string()),
    }
}

fn config_label(agent: AgentKind) -> &'static str {
    match agent {
        AgentKind::Codex => "Codex Hook 配置文件",
        AgentKind::Claude => "Claude Code 配置文件",
        AgentKind::Kiro => "Kiro Hook 文件",
        AgentKind::Grok => "Grok Hook 文件",
        AgentKind::Kimi => "Kimi Code 配置文件",
    }
}

fn agent_label(agent: AgentKind) -> &'static str {
    match agent {
        AgentKind::Codex => "Codex",
        AgentKind::Claude => "Claude Code",
        AgentKind::Kiro => "Kiro",
        AgentKind::Grok => "Grok Build",
        AgentKind::Kimi => "Kimi Code",
    }
}

pub fn get_hook_status(agent: AgentKind) -> Result<HookStatus, String> {
    if agent == AgentKind::Kimi {
        return kimi_hook_status();
    }
    let path = config_path(agent)?;
    let arg = hook_arg(agent)?;
    let command = expected_command(arg)?;
    if !path.exists() {
        return Ok(HookStatus {
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
    let prompt_commands = managed_commands_for(&root, USER_PROMPT_SUBMIT, arg);
    let stop_commands = managed_commands_for(&root, STOP, arg);
    let managed_handler_count = managed_command_count(&root, arg);
    let installed = managed_handler_count == 2
        && prompt_commands.len() == 1
        && stop_commands.len() == 1
        && prompt_commands[0] == command
        && stop_commands[0] == command;
    let issue = if installed || managed_handler_count == 0 {
        // Claude Code lets users disable every hook with one switch; an
        // installed-but-disabled hook looks broken, so surface it.
        if agent == AgentKind::Claude
            && root
                .get("disableAllHooks")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            Some("Claude Code 已设置 disableAllHooks，所有 Hook 都不会执行，请先关闭该选项。".to_string())
        } else if agent == AgentKind::Codex && installed {
            // hooks.json written is not enough: Codex only runs handlers it
            // trusts. An installed-but-untrusted hook silently never fires.
            codex_trust_issue(&path, &root)
        } else {
            None
        }
    } else {
        Some(format!(
            "{} Hook 配置不完整或命令路径已变化，可重新安装进行修复。",
            agent_label(agent)
        ))
    };

    Ok(HookStatus {
        installed,
        config_path: path.display().to_string(),
        command,
        managed_handler_count,
        issue,
    })
}

pub fn preview_hook_change(
    agent: AgentKind,
    action: HookAction,
) -> Result<HookChangePreview, String> {
    let path = config_path(agent)?;
    let arg = hook_arg(agent)?;
    let command = expected_command(arg)?;
    let before = read_existing(&path)?;
    if agent == AgentKind::Kimi {
        let after = kimi_render_after(action, &command, &before);
        return Ok(build_preview(action, &path, &command, &before, &after));
    }
    let after = render_after(agent, action, &path, &command, arg, &before)?;
    Ok(build_preview(action, &path, &command, &before, &after))
}

pub fn apply_hook_change(
    agent: AgentKind,
    action: HookAction,
    expected_before_hash: &str,
) -> Result<HookStatus, String> {
    let path = config_path(agent)?;
    let arg = hook_arg(agent)?;
    let command = expected_command(arg)?;
    let before = read_existing(&path)?;
    if content_hash(&before) != expected_before_hash {
        return Err(format!(
            "{}已发生变化，请重新预览后再确认。",
            config_label(agent)
        ));
    }

    let after = if agent == AgentKind::Kimi {
        kimi_render_after(action, &command, &before)
    } else {
        render_after(agent, action, &path, &command, arg, &before)?
    };
    if before != after {
        if action == HookAction::Install {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("unable to create {}: {error}", parent.display()))?;
            }
        }
        let write_path = resolve_write_target(&path)?;
        atomic_write(agent, &write_path, after.as_bytes(), expected_before_hash)?;
    }
    get_hook_status(agent)
}

fn build_preview(
    action: HookAction,
    path: &Path,
    command: &str,
    before: &str,
    after: &str,
) -> HookChangePreview {
    let mut diff_lines = Vec::new();
    let mut added = 0;
    let mut removed = 0;
    for change in TextDiff::from_lines(before, after).iter_all_changes() {
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

    HookChangePreview {
        action: action.as_str().to_string(),
        config_path: path.display().to_string(),
        command: command.to_string(),
        before_hash: content_hash(before),
        changed: before != after,
        diff_lines,
        added,
        removed,
    }
}

fn render_after(
    agent: AgentKind,
    action: HookAction,
    path: &Path,
    command: &str,
    arg: &str,
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
    remove_managed_handlers(agent, &mut root, arg)?;
    if action == HookAction::Install {
        append_managed_handler(&mut root, USER_PROMPT_SUBMIT, command)?;
        append_managed_handler(&mut root, STOP, command)?;
    }
    let mut rendered = serde_json::to_string_pretty(&root)
        .map_err(|error| format!("unable to serialize {}: {error}", config_label(agent)))?;
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
        .ok_or_else(|| "Hook config root is not an object".to_string())?;
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

fn remove_managed_handlers(agent: AgentKind, root: &mut Value, arg: &str) -> Result<(), String> {
    let Some(root_object) = root.as_object_mut() else {
        return Err(format!("{} root is not an object", config_label(agent)));
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
            handlers.retain(|handler| !is_managed_handler(handler, arg));
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

fn managed_commands_for(root: &Value, event_name: &str, arg: &str) -> Vec<String> {
    root.get("hooks")
        .and_then(Value::as_object)
        .and_then(|hooks| hooks.get(event_name))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|group| group.get("hooks").and_then(Value::as_array))
        .flatten()
        .filter(|handler| is_managed_handler(handler, arg))
        .filter_map(|handler| handler.get("command").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

fn managed_command_count(root: &Value, arg: &str) -> usize {
    root.get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|hooks| hooks.values())
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|group| group.get("hooks").and_then(Value::as_array))
        .flatten()
        .filter(|handler| is_managed_handler(handler, arg))
        .count()
}

fn is_managed_handler(handler: &Value, arg: &str) -> bool {
    handler.get("type").and_then(Value::as_str) == Some("command")
        && handler
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| command.split_whitespace().any(|part| part == arg))
}

fn expected_command(arg: &str) -> Result<String, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("unable to locate Agent Hub executable: {error}"))?;
    let path = executable.to_string_lossy();
    #[cfg(target_os = "windows")]
    {
        return Ok(format!("\"{}\" {arg}", path.replace('"', "\\\"")));
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(format!("'{}' {arg}", path.replace('\'', "'\\''")))
    }
}

/// Codex runs user-level hooks.json handlers only after they are trusted.
/// The trust state lives in `~/.codex/config.toml` under
/// `hooks.state."<hooks.json path>:<event>:<group>:<handler>"` with a
/// `trusted_hash`; a handler our installer just wrote starts untrusted and
/// silently never fires (Codex TUI shows a startup review, ChatGPT desktop
/// has 设置 → 钩子). We cannot recompute Codex's trust hash, so this is a
/// presence check on the state entry, not a hash verification.
fn codex_trust_issue(config_path: &Path, root: &Value) -> Option<String> {
    let config_toml = dirs::home_dir()
        .and_then(|home| fs::read_to_string(home.join(".codex").join("config.toml")).ok());
    let missing = codex_untrusted_events(config_toml.as_deref(), config_path, root);
    if missing.is_empty() {
        None
    } else {
        Some(format!(
            "Codex 尚未信任 {} Hook（已写入 hooks.json，但未信任的 Hook 不会执行）。请启动一次 Codex，在启动时的 Hook 审查中选择 Trust all and continue；ChatGPT 桌面端则在 设置 → 钩子 中手动信任。",
            missing.join(" 与 ")
        ))
    }
}

fn codex_untrusted_events(
    config_toml: Option<&str>,
    config_path: &Path,
    root: &Value,
) -> Vec<&'static str> {
    let doc = config_toml.and_then(|content| toml::from_str::<toml::Value>(content).ok());
    let states = doc
        .as_ref()
        .and_then(|doc| doc.get("hooks"))
        .and_then(|hooks| hooks.get("state"))
        .and_then(toml::Value::as_table);
    let base = config_path.display().to_string();
    let mut missing = Vec::new();
    for (event, label) in [(USER_PROMPT_SUBMIT, "user_prompt_submit"), (STOP, "stop")] {
        let Some((group, handler)) = managed_handler_position(root, event) else {
            continue;
        };
        let key = format!("{base}:{label}:{group}:{handler}");
        let state = states.and_then(|table| table.get(&key));
        let trusted = state
            .and_then(|state| state.get("trusted_hash"))
            .and_then(toml::Value::as_str)
            .is_some_and(|hash| !hash.trim().is_empty());
        let enabled = state
            .and_then(|state| state.get("enabled"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        if !trusted || !enabled {
            missing.push(event);
        }
    }
    missing
}

/// Position of our managed handler inside hooks.json: Codex keys hook state
/// by the handler's group/handler index, so the trust lookup needs them.
fn managed_handler_position(root: &Value, event_name: &str) -> Option<(usize, usize)> {
    let groups = root.get("hooks")?.get(event_name)?.as_array()?;
    for (group_index, group) in groups.iter().enumerate() {
        let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
            continue;
        };
        for (handler_index, handler) in handlers.iter().enumerate() {
            if is_managed_handler(handler, CODEX_HOOK_ARG) {
                return Some((group_index, handler_index));
            }
        }
    }
    None
}

fn read_existing(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("unable to read {}: {error}", path.display())),
    }
}

// --- Kimi Code (TOML) ------------------------------------------------------
// Kimi reads `[[hooks]]` tables from ~/.kimi-code/config.toml — the user's
// main settings file — so edits are text-based: only the managed blocks
// (identified by our hook arg) are removed/appended. Every other byte —
// comments, formatting, unrelated tables — survives untouched.

fn kimi_hook_status() -> Result<HookStatus, String> {
    let path = config_path(AgentKind::Kimi)?;
    let command = expected_command(KIMI_HOOK_ARG)?;
    if !path.exists() {
        return Ok(HookStatus {
            installed: false,
            config_path: path.display().to_string(),
            command,
            managed_handler_count: 0,
            issue: None,
        });
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("unable to read {}: {error}", path.display()))?;
    let managed = kimi_managed_entries(&content)?;
    let prompt_commands: Vec<&str> = managed
        .iter()
        .filter(|(event, _)| event == USER_PROMPT_SUBMIT)
        .map(|(_, command)| command.as_str())
        .collect();
    let stop_commands: Vec<&str> = managed
        .iter()
        .filter(|(event, _)| event == STOP)
        .map(|(_, command)| command.as_str())
        .collect();
    let installed = managed.len() == 2
        && prompt_commands.len() == 1
        && stop_commands.len() == 1
        && prompt_commands[0] == command
        && stop_commands[0] == command;
    let issue = if installed || managed.is_empty() {
        None
    } else {
        Some(format!(
            "{} Hook 配置不完整或命令路径已变化，可重新安装进行修复。",
            agent_label(AgentKind::Kimi)
        ))
    };

    Ok(HookStatus {
        installed,
        config_path: path.display().to_string(),
        command,
        managed_handler_count: managed.len(),
        issue,
    })
}

/// (event, command) pairs of the `[[hooks]]` tables we manage.
fn kimi_managed_entries(content: &str) -> Result<Vec<(String, String)>, String> {
    let doc: toml::Value = toml::from_str(content)
        .map_err(|error| format!("Kimi Code 配置文件不是有效 TOML：{error}"))?;
    let Some(tables) = doc.get("hooks").and_then(toml::Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    for table in tables {
        let command = table.get("command").and_then(toml::Value::as_str).unwrap_or("");
        if !command.split_whitespace().any(|part| part == KIMI_HOOK_ARG) {
            continue;
        }
        let event = table
            .get("event")
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string();
        entries.push((event, command.to_string()));
    }
    Ok(entries)
}

fn kimi_render_after(action: HookAction, command: &str, before: &str) -> String {
    let mut cleaned = remove_kimi_managed_blocks(before);
    if action == HookAction::Uninstall {
        return cleaned;
    }
    if !cleaned.is_empty() {
        cleaned.push('\n');
    }
    cleaned.push_str(&format!(
        "[[hooks]]\nevent = \"{USER_PROMPT_SUBMIT}\"\ncommand = \"{}\"\ntimeout = 10\n\n[[hooks]]\nevent = \"{STOP}\"\ncommand = \"{}\"\ntimeout = 10\n",
        kimi_toml_escape(command),
        kimi_toml_escape(command),
    ));
    cleaned
}

/// Escape a command line for embedding in a TOML basic string.
fn kimi_toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Drop every `[[hooks]]` table block containing our hook arg, preserving the
/// rest of the file byte-for-byte.
fn remove_kimi_managed_blocks(before: &str) -> String {
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in before.split_inclusive('\n') {
        if line.trim_start().starts_with('[') && !current.is_empty() {
            segments.push(std::mem::take(&mut current));
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        segments.push(current);
    }

    let mut out = String::new();
    for segment in segments {
        let is_managed_hook_block =
            segment.trim_start().starts_with("[[hooks]]") && segment.contains(KIMI_HOOK_ARG);
        if !is_managed_hook_block {
            out.push_str(&segment);
        }
    }
    // Removing the last block can leave trailing blank lines behind; keep a
    // single trailing newline.
    let trimmed = out.trim_end();
    if trimmed.is_empty() {
        String::new()
    } else {
        let mut out = trimmed.to_string();
        out.push('\n');
        out
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

fn atomic_write(
    agent: AgentKind,
    path: &Path,
    content: &[u8],
    expected_before_hash: &str,
) -> Result<(), String> {
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
    // race window when the agent or another editor updates the config after
    // the user reviewed the diff.
    let current = match read_existing(path) {
        Ok(current) => current,
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
    };
    if content_hash(&current) != expected_before_hash {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "{}已发生变化，请重新预览后再确认。",
            config_label(agent)
        ));
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
            AgentKind::Codex,
            HookAction::Install,
            path,
            "'/Applications/Agent Hub' --agent-hub-codex-hook",
            CODEX_HOOK_ARG,
            original_config(),
        )
        .unwrap();
        let root: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(root["custom"], "keep-me");
        assert_eq!(managed_command_count(&root, CODEX_HOOK_ARG), 2);
        assert!(after.contains("existing-command"));
    }

    #[test]
    fn uninstall_only_removes_owned_handlers() {
        let path = Path::new("/tmp/hooks.json");
        let installed = render_after(
            AgentKind::Codex,
            HookAction::Install,
            path,
            "agent-hub --agent-hub-codex-hook",
            CODEX_HOOK_ARG,
            original_config(),
        )
        .unwrap();
        let after = render_after(
            AgentKind::Codex,
            HookAction::Uninstall,
            path,
            "agent-hub --agent-hub-codex-hook",
            CODEX_HOOK_ARG,
            &installed,
        )
        .unwrap();
        let root: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(managed_command_count(&root, CODEX_HOOK_ARG), 0);
        assert!(after.contains("existing-command"));
        assert_eq!(root["custom"], "keep-me");
    }

    #[test]
    fn claude_install_preserves_other_settings_and_does_not_touch_codex_handlers() {
        let before = r#"{
  "model": "claude-sonnet-5",
  "permissions": {"allow": ["Bash(git status)"]},
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {"type": "command", "command": "agent-hub --agent-hub-codex-hook"}
        ]
      }
    ]
  }
}
"#;
        let path = Path::new("/tmp/settings.json");
        let after = render_after(
            AgentKind::Claude,
            HookAction::Install,
            path,
            "agent-hub --agent-hub-claude-hook",
            CLAUDE_HOOK_ARG,
            before,
        )
        .unwrap();
        let root: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(root["model"], "claude-sonnet-5");
        assert!(root.get("permissions").is_some());
        assert_eq!(managed_command_count(&root, CLAUDE_HOOK_ARG), 2);
        // A Codex handler in the same file is not managed by the Claude
        // target and survives install/uninstall cycles.
        assert!(after.contains("--agent-hub-codex-hook"));

        let after_uninstall = render_after(
            AgentKind::Claude,
            HookAction::Uninstall,
            path,
            "agent-hub --agent-hub-claude-hook",
            CLAUDE_HOOK_ARG,
            &after,
        )
        .unwrap();
        let root: Value = serde_json::from_str(&after_uninstall).unwrap();
        assert_eq!(managed_command_count(&root, CLAUDE_HOOK_ARG), 0);
        assert!(after_uninstall.contains("--agent-hub-codex-hook"));
        assert_eq!(root["model"], "claude-sonnet-5");
    }

    #[test]
    fn kiro_hook_change_is_rejected() {
        // Kiro monitoring is file-watching only; no hook target exists.
        assert!(config_path(AgentKind::Kiro).is_err());
        assert!(hook_arg(AgentKind::Kiro).is_err());
    }

    #[test]
    fn kimi_install_appends_two_managed_blocks_and_preserves_config() {
        let before = "model = \"k2\"\n\n[[hooks]]\nevent = \"Notification\"\ncommand = \"terminal-notifier -message done\"\n";
        let after = kimi_render_after(
            HookAction::Install,
            "'/Applications/Agent Hub' --agent-hub-kimi-hook",
            before,
        );
        assert!(after.contains("model = \"k2\""));
        assert!(after.contains("terminal-notifier"));
        let entries = kimi_managed_entries(&after).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|(event, _)| event == USER_PROMPT_SUBMIT));
        assert!(entries.iter().any(|(event, _)| event == STOP));
    }

    #[test]
    fn kimi_uninstall_removes_only_managed_blocks() {
        let before = "model = \"k2\"\n\n[[hooks]]\nevent = \"Notification\"\ncommand = \"terminal-notifier -message done\"\n";
        let installed = kimi_render_after(
            HookAction::Install,
            "agent-hub --agent-hub-kimi-hook",
            before,
        );
        let after = kimi_render_after(
            HookAction::Uninstall,
            "agent-hub --agent-hub-kimi-hook",
            &installed,
        );
        assert!(after.contains("model = \"k2\""));
        assert!(after.contains("terminal-notifier"));
        assert!(!after.contains(KIMI_HOOK_ARG));
        // Only the user's own hook block survives.
        assert_eq!(after.matches("[[hooks]]").count(), 1);
    }

    #[test]
    fn kimi_managed_entries_ignores_foreign_hooks() {
        let content = "[[hooks]]\nevent = \"Notification\"\ncommand = \"notify-send done\"\n";
        assert!(kimi_managed_entries(content).unwrap().is_empty());
    }

    #[test]
    fn kimi_toml_escape_handles_backslashes_and_quotes() {
        assert_eq!(
            kimi_toml_escape("\"C:\\Tools\\Agent Hub.exe\" --agent-hub-kimi-hook"),
            "\\\"C:\\\\Tools\\\\Agent Hub.exe\\\" --agent-hub-kimi-hook"
        );
    }

    #[test]
    fn malformed_hooks_shape_is_rejected_without_replacement() {
        let error = render_after(
            AgentKind::Codex,
            HookAction::Install,
            Path::new("/tmp/hooks.json"),
            "agent-hub --agent-hub-codex-hook",
            CODEX_HOOK_ARG,
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

        let error = atomic_write(
            AgentKind::Codex,
            &path,
            b"replacement",
            &content_hash("old-preview"),
        )
        .unwrap_err();

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
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    }

    fn codex_root_with_managed_handlers() -> Value {
        let installed = render_after(
            AgentKind::Codex,
            HookAction::Install,
            Path::new("/tmp/hooks.json"),
            "agent-hub --agent-hub-codex-hook",
            CODEX_HOOK_ARG,
            "{}",
        )
        .unwrap();
        serde_json::from_str(&installed).unwrap()
    }

    #[test]
    fn untrusted_codex_hooks_are_reported() {
        let root = codex_root_with_managed_handlers();
        let config_path = Path::new("/home/u/.codex/hooks.json");
        // No config.toml at all → both events untrusted.
        assert_eq!(
            codex_untrusted_events(None, config_path, &root),
            vec![USER_PROMPT_SUBMIT, STOP]
        );
        // Only UserPromptSubmit trusted → Stop still reported.
        let config = r#"
[hooks.state."/home/u/.codex/hooks.json:user_prompt_submit:0:0"]
trusted_hash = "sha256:abc"
"#;
        assert_eq!(
            codex_untrusted_events(Some(config), config_path, &root),
            vec![STOP]
        );
        // Both trusted → clean.
        let config = r#"
[hooks.state."/home/u/.codex/hooks.json:user_prompt_submit:0:0"]
trusted_hash = "sha256:abc"

[hooks.state."/home/u/.codex/hooks.json:stop:0:0"]
trusted_hash = "sha256:def"
"#;
        assert!(codex_untrusted_events(Some(config), config_path, &root).is_empty());
        // Trusted but explicitly disabled → reported again.
        let config = r#"
[hooks.state."/home/u/.codex/hooks.json:user_prompt_submit:0:0"]
trusted_hash = "sha256:abc"

[hooks.state."/home/u/.codex/hooks.json:stop:0:0"]
trusted_hash = "sha256:def"
enabled = false
"#;
        assert_eq!(
            codex_untrusted_events(Some(config), config_path, &root),
            vec![STOP]
        );
    }

    #[test]
    fn managed_handler_position_tracks_existing_groups() {
        // A pre-existing hook group pushes our handler to index 1.
        let installed = render_after(
            AgentKind::Codex,
            HookAction::Install,
            Path::new("/tmp/hooks.json"),
            "agent-hub --agent-hub-codex-hook",
            CODEX_HOOK_ARG,
            original_config(),
        )
        .unwrap();
        let root: Value = serde_json::from_str(&installed).unwrap();
        assert_eq!(
            managed_handler_position(&root, USER_PROMPT_SUBMIT),
            Some((1, 0))
        );
        assert_eq!(managed_handler_position(&root, STOP), Some((0, 0)));
        assert_eq!(managed_handler_position(&root, "SessionStart"), None);
    }
}

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::paths::join_relative;
use crate::win_console::suppress_console;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ClaudePluginView {
    pub id: String,
    pub name: String,
    pub marketplace: String,
    pub version: String,
    pub scope: String,
    pub enabled: bool,
    pub manageable: bool,
    pub description: String,
    pub install_path: String,
    pub installed_at: Option<String>,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledPlugin {
    id: String,
    #[serde(default)]
    version: String,
    scope: String,
    enabled: bool,
    #[serde(default)]
    install_path: String,
    #[serde(default)]
    installed_at: Option<String>,
    #[serde(default)]
    last_updated: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AvailablePlugin {
    plugin_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    marketplace_name: String,
}

fn split_plugin_id(id: &str) -> (&str, &str) {
    id.rsplit_once('@').unwrap_or((id, ""))
}

fn manifest_metadata(install_path: &str) -> (Option<String>, Option<String>) {
    if install_path.is_empty() {
        return (None, None);
    }

    let manifest_path = join_relative(Path::new(install_path).to_path_buf(), ".claude-plugin/plugin.json");
    let Ok(text) = fs::read_to_string(manifest_path) else {
        return (None, None);
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return (None, None);
    };

    let name = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);
    let description = value
        .get("description")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);
    (name, description)
}

fn parse_plugin_list(stdout: &str) -> Result<Vec<ClaudePluginView>, String> {
    let value: Value = serde_json::from_str(stdout)
        .map_err(|error| format!("Claude Code 插件列表不是有效 JSON：{error}"))?;

    let (installed_value, available_value) = match &value {
        Value::Array(_) => (value.clone(), Value::Array(Vec::new())),
        Value::Object(root) => (
            root.get("installed")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
            root.get("available")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
        ),
        _ => return Err("Claude Code 插件列表返回了不支持的数据结构。".into()),
    };

    let installed: Vec<InstalledPlugin> = serde_json::from_value(installed_value)
        .map_err(|error| format!("无法解析已安装的 Claude Code 插件：{error}"))?;
    let available: Vec<AvailablePlugin> = serde_json::from_value(available_value)
        .map_err(|error| format!("无法解析 Claude Code 插件目录：{error}"))?;
    let available_by_id: HashMap<String, AvailablePlugin> = available
        .into_iter()
        .map(|plugin| (plugin.plugin_id.clone(), plugin))
        .collect();

    let mut plugins: Vec<ClaudePluginView> = installed
        .into_iter()
        .map(|plugin| {
            let (id_name, id_marketplace) = split_plugin_id(&plugin.id);
            let catalog = available_by_id.get(&plugin.id);
            let (manifest_name, manifest_description) = manifest_metadata(&plugin.install_path);
            let name = catalog
                .map(|item| item.name.trim())
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .or(manifest_name)
                .unwrap_or_else(|| id_name.to_owned());
            let marketplace = catalog
                .map(|item| item.marketplace_name.trim())
                .filter(|value| !value.is_empty())
                .unwrap_or(id_marketplace)
                .to_owned();
            let description = catalog
                .map(|item| item.description.trim())
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .or(manifest_description)
                .unwrap_or_default();

            ClaudePluginView {
                id: plugin.id,
                name,
                marketplace,
                version: plugin.version,
                manageable: plugin.scope == "user",
                scope: plugin.scope,
                enabled: plugin.enabled,
                description,
                install_path: plugin.install_path,
                installed_at: plugin.installed_at,
                last_updated: plugin.last_updated,
            }
        })
        .collect();

    plugins.sort_by(|left, right| {
        right
            .enabled
            .cmp(&left.enabled)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(plugins)
}

fn claude_executable_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("CLAUDE_CODE_BIN") {
        candidates.push(PathBuf::from(path));
    }
    candidates.push(PathBuf::from("claude"));

    if let Some(home) = dirs::home_dir() {
        candidates.push(join_relative(home.clone(), ".local/bin/claude"));
        candidates.push(join_relative(home.clone(), ".claude/local/claude"));
        #[cfg(target_os = "windows")]
        candidates.push(join_relative(home.clone(), "AppData/Roaming/npm/claude.cmd"));
    }

    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/opt/homebrew/bin/claude"));
        candidates.push(PathBuf::from("/usr/local/bin/claude"));
    }

    let mut seen = HashSet::new();
    candidates.retain(|path| seen.insert(path.clone()));
    candidates
}

fn background_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    // Windows: CREATE_NO_WINDOW so listing/toggling Claude plugins never
    // flashes a console behind the GUI.
    suppress_console(&mut command);
    command
}

fn run_claude(args: &[&str], workspace: Option<&Path>) -> Result<Output, String> {
    let mut attempted = Vec::new();
    for executable in claude_executable_candidates() {
        attempted.push(executable.display().to_string());
        let mut command = background_command(&executable);
        command.args(args);
        if let Some(directory) = workspace {
            command.current_dir(directory);
        }
        match command.output() {
            Ok(output) => return Ok(output),
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "无法运行 Claude Code CLI（{}）：{error}",
                    executable.display()
                ))
            }
        }
    }

    Err(format!(
        "未找到 Claude Code CLI。已检查：{}",
        attempted.join("、")
    ))
}

fn command_error(action: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let detail = if !stderr.is_empty() { stderr } else { stdout };
    if detail.is_empty() {
        format!(
            "Claude Code {action}失败（退出码 {:?}）。",
            output.status.code()
        )
    } else {
        detail
    }
}

fn list_claude_plugins_impl(workspace: Option<&Path>) -> Result<Vec<ClaudePluginView>, String> {
    let with_catalog = run_claude(&["plugin", "list", "--json", "--available"], workspace)?;
    let output = if with_catalog.status.success() {
        with_catalog
    } else {
        // Older Claude Code versions and temporarily unavailable marketplaces
        // may reject `--available`. Installed plugins remain manageable without
        // catalog descriptions, so fall back to the smaller command.
        let installed_only = run_claude(&["plugin", "list", "--json"], workspace)?;
        if !installed_only.status.success() {
            return Err(command_error("读取插件列表", &installed_only));
        }
        installed_only
    };
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("Claude Code 插件列表不是 UTF-8 文本：{error}"))?;
    let mut plugins = parse_plugin_list(&stdout)?;
    if workspace.is_some() {
        plugins.retain(|plugin| matches!(plugin.scope.as_str(), "project" | "local"));
        for plugin in &mut plugins {
            plugin.manageable = false;
        }
    }
    Ok(plugins)
}

fn set_claude_plugin_enabled_impl(
    plugin_id: &str,
    scope: &str,
    enabled: bool,
) -> Result<(), String> {
    if plugin_id.trim().is_empty() {
        return Err("插件 ID 不能为空。".into());
    }
    if scope != "user" {
        return Err("Agent Hub 当前仅支持切换用户范围的 Claude Code 插件。".into());
    }

    let action = if enabled { "enable" } else { "disable" };
    let output = run_claude(&["plugin", action, plugin_id, "--scope", scope], None)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error(
            if enabled {
                "启用插件"
            } else {
                "停用插件"
            },
            &output,
        ))
    }
}

#[tauri::command]
pub async fn list_claude_plugins(
    workspace_dir: Option<String>,
) -> Result<Vec<ClaudePluginView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = workspace_dir
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        if let Some(path) = workspace.as_deref() {
            if !path.is_dir() {
                return Err(format!("项目目录不存在：{}", path.display()));
            }
        }
        list_claude_plugins_impl(workspace.as_deref())
    })
    .await
    .map_err(|error| format!("读取 Claude Code 插件任务失败：{error}"))?
}

#[tauri::command]
pub async fn set_claude_plugin_enabled(
    plugin_id: String,
    scope: String,
    enabled: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        set_claude_plugin_enabled_impl(&plugin_id, &scope, enabled)
    })
    .await
    .map_err(|error| format!("切换 Claude Code 插件任务失败：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_list_shape_and_enriches_catalog_metadata() {
        let json = r#"{
          "installed": [{
            "id": "formatter@team-tools",
            "version": "1.2.0",
            "scope": "user",
            "enabled": true,
            "installPath": "",
            "installedAt": "2026-07-01T00:00:00Z",
            "lastUpdated": "2026-07-02T00:00:00Z"
          }],
          "available": [{
            "pluginId": "formatter@team-tools",
            "name": "Formatter",
            "description": "Formats source files",
            "marketplaceName": "team-tools"
          }]
        }"#;

        let plugins = parse_plugin_list(json).unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "Formatter");
        assert_eq!(plugins[0].marketplace, "team-tools");
        assert_eq!(plugins[0].description, "Formats source files");
        assert!(plugins[0].manageable);
    }

    #[test]
    fn parses_legacy_array_shape_and_marks_non_user_scope_read_only() {
        let json = r#"[{
          "id": "reviewer@company",
          "version": "2.0.0",
          "scope": "managed",
          "enabled": false,
          "installPath": ""
        }]"#;

        let plugins = parse_plugin_list(json).unwrap();
        assert_eq!(plugins[0].name, "reviewer");
        assert_eq!(plugins[0].marketplace, "company");
        assert!(!plugins[0].manageable);
    }

    #[test]
    fn rejects_toggling_non_user_scope() {
        let error =
            set_claude_plugin_enabled_impl("reviewer@company", "project", true).unwrap_err();
        assert!(error.contains("仅支持切换用户范围"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn background_commands_do_not_allocate_a_console_window() {
        let script = r#"
            Add-Type -Name ConsoleWindow -Namespace AgentHub -MemberDefinition '
                [System.Runtime.InteropServices.DllImport("Kernel32.dll")]
                public static extern System.IntPtr GetConsoleWindow();
            '
            [AgentHub.ConsoleWindow]::GetConsoleWindow().ToInt64()
        "#;
        let output = background_command(Path::new("powershell.exe"))
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .expect("PowerShell should be available on supported Windows versions");

        assert!(
            output.status.success(),
            "console probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
    }
}

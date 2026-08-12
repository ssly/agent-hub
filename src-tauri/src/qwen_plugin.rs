//! Qwen Code 扩展只读列表。
//!
//! Qwen Code 的原生扩展位于 `~/.qwen/extensions/<name>/`，
//! manifest 为目录下的 `qwen-extension.json`，字段含
//! `name` / `version` / `description` / `mcpServers` / `contextFileName`
//! / `commands` / `skills` / `agents` / `settings`。
//!
//! 只读原因：启停状态的持久化位置未经官方确认，这里不做任何写操作。
//!
//! 所有 IO / 解析失败都降级为空列表或跳过单个条目：目录不存在视为未安装
//! Qwen Code，绝不向前端报错。

use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::{home_dir, join_relative};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QwenPluginView {
    /// 扩展目录名，前端列表 key。
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub mcp_server_count: usize,
    pub skill_count: usize,
    pub command_count: usize,
    pub agent_count: usize,
    pub install_path: String,
}

fn qwen_extensions_root() -> PathBuf {
    join_relative(home_dir(), ".qwen/extensions")
}

pub fn list_qwen_plugins() -> Vec<QwenPluginView> {
    scan_extensions_root(&qwen_extensions_root())
}

fn scan_extensions_root(root: &Path) -> Vec<QwenPluginView> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut plugins: Vec<QwenPluginView> = entries
        .flatten()
        .filter_map(|entry| scan_extension(&entry.path()))
        .collect();

    plugins.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    plugins
}

/// 单个扩展目录：manifest 缺失或损坏就跳过这一条，不影响其他扩展。
fn scan_extension(extension_dir: &Path) -> Option<QwenPluginView> {
    if !extension_dir.is_dir() {
        return None;
    }
    let dir_name = extension_dir.file_name()?.to_string_lossy().into_owned();
    let text = fs::read_to_string(extension_dir.join("qwen-extension.json")).ok()?;
    let manifest = serde_json::from_str::<Value>(&text).ok()?;

    let name = manifest_field(&manifest, "name").unwrap_or_else(|| dir_name.clone());

    Some(QwenPluginView {
        id: dir_name,
        name,
        version: manifest_field(&manifest, "version").unwrap_or_default(),
        description: manifest_field(&manifest, "description").unwrap_or_default(),
        mcp_server_count: manifest
            .get("mcpServers")
            .and_then(Value::as_object)
            .map(|servers| servers.len())
            .unwrap_or(0),
        skill_count: manifest_array_len(&manifest, "skills"),
        command_count: manifest_array_len(&manifest, "commands"),
        agent_count: manifest_array_len(&manifest, "agents"),
        install_path: extension_dir.display().to_string(),
    })
}

fn manifest_field(manifest: &Value, field: &str) -> Option<String> {
    manifest
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// manifest 里的数组字段长度；缺失或类型不符为 0。
fn manifest_array_len(manifest: &Value, field: &str) -> usize {
    manifest
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Fixture {
        root: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            Self { root }
        }

        fn extensions_root(&self) -> PathBuf {
            self.root.path().to_path_buf()
        }

        fn write_extension(&self, dir_name: &str, manifest: &str) -> PathBuf {
            let dir = self.extensions_root().join(dir_name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("qwen-extension.json"), manifest).unwrap();
            dir
        }
    }

    #[test]
    fn missing_extensions_root_yields_empty_list() {
        let fixture = Fixture::new();
        let missing = fixture.extensions_root().join("does-not-exist");
        assert!(scan_extensions_root(&missing).is_empty());
    }

    #[test]
    fn reads_manifest_fields_and_counts_components() {
        let fixture = Fixture::new();
        let dir = fixture.write_extension(
            "my-tools",
            r#"{
                "name": "My Tools",
                "version": "1.2.3",
                "description": "a qwen extension",
                "mcpServers": {"alpha": {"command": "a"}, "beta": {"httpUrl": "https://b"}},
                "contextFileName": "QWEN.md",
                "commands": ["/a", "/b"],
                "skills": ["review"],
                "agents": ["helper", "planner", "reviewer"],
                "settings": {"theme": "dark"}
            }"#,
        );

        let plugins = scan_extensions_root(&fixture.extensions_root());
        assert_eq!(plugins.len(), 1);
        let plugin = &plugins[0];
        assert_eq!(plugin.id, "my-tools");
        assert_eq!(plugin.name, "My Tools");
        assert_eq!(plugin.version, "1.2.3");
        assert_eq!(plugin.description, "a qwen extension");
        assert_eq!(plugin.mcp_server_count, 2);
        assert_eq!(plugin.command_count, 2);
        assert_eq!(plugin.skill_count, 1);
        assert_eq!(plugin.agent_count, 3);
        assert_eq!(plugin.install_path, dir.display().to_string());
    }

    #[test]
    fn missing_component_fields_default_to_zero() {
        let fixture = Fixture::new();
        fixture.write_extension("bare", r#"{"name": "Bare"}"#);

        let plugins = scan_extensions_root(&fixture.extensions_root());
        assert_eq!(plugins.len(), 1);
        let plugin = &plugins[0];
        assert_eq!(plugin.name, "Bare");
        assert_eq!(plugin.version, "");
        assert_eq!(plugin.mcp_server_count, 0);
        assert_eq!(plugin.skill_count, 0);
        assert_eq!(plugin.command_count, 0);
        assert_eq!(plugin.agent_count, 0);
    }

    #[test]
    fn missing_name_falls_back_to_directory_name() {
        let fixture = Fixture::new();
        fixture.write_extension("fallback-ext", r#"{"version": "0.1.0"}"#);

        let plugins = scan_extensions_root(&fixture.extensions_root());
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "fallback-ext");
        assert_eq!(plugins[0].id, "fallback-ext");
    }

    #[test]
    fn corrupt_manifest_is_skipped_without_killing_others() {
        let fixture = Fixture::new();
        fixture.write_extension("broken", "{not json");
        fixture.write_extension("good", r#"{"name": "Good"}"#);
        // 非目录条目（普通文件）也应被跳过。
        fs::write(fixture.extensions_root().join("stray.txt"), b"x").unwrap();

        let plugins = scan_extensions_root(&fixture.extensions_root());
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "Good");
    }

    #[test]
    fn plugins_are_sorted_by_name_case_insensitively() {
        let fixture = Fixture::new();
        fixture.write_extension("zeta", r#"{"name": "zeta"}"#);
        fixture.write_extension("Alpha", r#"{"name": "Alpha"}"#);

        let plugins = scan_extensions_root(&fixture.extensions_root());
        let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "zeta"]);
    }

    #[test]
    fn serializes_to_camel_case() {
        let plugin = QwenPluginView {
            id: "ext".into(),
            name: "Ext".into(),
            version: "1.0.0".into(),
            description: String::new(),
            mcp_server_count: 1,
            skill_count: 2,
            command_count: 3,
            agent_count: 4,
            install_path: "/tmp/ext".into(),
        };
        let value = serde_json::to_value(&plugin).unwrap();
        assert!(value.get("mcpServerCount").is_some());
        assert!(value.get("skillCount").is_some());
        assert!(value.get("commandCount").is_some());
        assert!(value.get("agentCount").is_some());
        assert!(value.get("installPath").is_some());
        assert!(value.get("mcp_server_count").is_none());
    }
}

//! ZCode 插件市场只读列表。
//!
//! ZCode 的插件体系是 Claude Code 风格的市场制：
//!
//! - 市场登记：`~/.zcode/cli/plugins/marketplaces/<marketplace-id>/marketplace.json`
//!   （`plugins[]` 带 `name` / `version` / `cachePath` / `source`；登记版本与
//!   cachePath 中的版本目录可能不一致，一律以 cachePath 下的 manifest 为准）
//! - 插件实体：`~/.zcode/cli/plugins/cache/<marketplace>/<plugin>/<version>/`，
//!   manifest 查找优先级 `.zcode-plugin/plugin.json` → `.claude-plugin/plugin.json`
//! - 运行时数据：`~/.zcode/cli/plugins/data/<plugin>@<marketplace>/`
//!
//! 只读原因：官方文档只说启停状态写在 `~/.zcode/cli/config.json` 的 plugins 键，
//! 语义未证实；这里把 data 目录存在性作为「已安装」的推测标记，不做任何启停写操作。
//!
//! 所有 IO / 解析失败都降级为空列表或跳过单个条目：目录不存在视为未安装 ZCode，
//! 绝不向前端报错。

use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::{home_dir, join_relative};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ZCodePluginView {
    /// `<name>@<marketplace-id>`，前端列表 key。
    pub id: String,
    pub name: String,
    pub marketplace: String,
    pub version: String,
    pub description: String,
    pub author: String,
    /// 推测语义：`data/<plugin>@<marketplace>/` 目录存在即视为已安装/启用过。
    pub installed: bool,
    pub skill_count: usize,
    pub command_count: usize,
    pub hook_count: usize,
    pub install_path: String,
}

fn zcode_plugins_root() -> PathBuf {
    join_relative(home_dir(), ".zcode/cli/plugins")
}

pub fn list_zcode_plugins() -> Vec<ZCodePluginView> {
    scan_plugins_root(&zcode_plugins_root())
}

fn scan_plugins_root(root: &Path) -> Vec<ZCodePluginView> {
    let Ok(entries) = fs::read_dir(root.join("marketplaces")) else {
        return Vec::new();
    };

    let mut plugins = Vec::new();
    for entry in entries.flatten() {
        let marketplace_dir = entry.path();
        if !marketplace_dir.is_dir() {
            continue;
        }
        let marketplace_id = entry.file_name().to_string_lossy().into_owned();
        plugins.extend(scan_marketplace(root, &marketplace_dir, &marketplace_id));
    }

    plugins.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.marketplace.cmp(&right.marketplace))
    });
    plugins
}

fn scan_marketplace(
    root: &Path,
    marketplace_dir: &Path,
    marketplace_id: &str,
) -> Vec<ZCodePluginView> {
    // 损坏或缺失的 marketplace.json 只跳过这一个市场，不影响其他市场。
    let Ok(text) = fs::read_to_string(marketplace_dir.join("marketplace.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(items) = value.get("plugins").and_then(Value::as_array) else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| scan_plugin(root, marketplace_id, item))
        .collect()
}

fn scan_plugin(root: &Path, marketplace_id: &str, item: &Value) -> Option<ZCodePluginView> {
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_owned();
    let listed_version = item
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let cache_path = item
        .get("cachePath")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);

    let manifest = cache_path.as_deref().and_then(read_manifest);
    let version = manifest_field(&manifest, "version")
        .filter(|value| !value.is_empty())
        .unwrap_or(listed_version);

    let (skill_count, command_count, hook_count) = match cache_path.as_deref() {
        Some(path) => (
            count_children(&path.join("skills")),
            count_children(&path.join("commands")),
            count_children(&path.join("hooks")),
        ),
        None => (0, 0, 0),
    };

    let installed = root
        .join("data")
        .join(format!("{name}@{marketplace_id}"))
        .is_dir();

    Some(ZCodePluginView {
        id: format!("{name}@{marketplace_id}"),
        name,
        marketplace: marketplace_id.to_owned(),
        version,
        description: manifest_field(&manifest, "description").unwrap_or_default(),
        author: manifest_field(&manifest, "author").unwrap_or_default(),
        installed,
        skill_count,
        command_count,
        hook_count,
        install_path: cache_path
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    })
}

/// 两级 manifest 查找：`.zcode-plugin/plugin.json` → `.claude-plugin/plugin.json`。
/// 损坏的 manifest 返回 None（退回市场登记里的元数据），不跳过整个插件。
fn read_manifest(cache_path: &Path) -> Option<Value> {
    for relative in [".zcode-plugin/plugin.json", ".claude-plugin/plugin.json"] {
        let manifest_path = join_relative(cache_path.to_path_buf(), relative);
        let Ok(text) = fs::read_to_string(manifest_path) else {
            continue;
        };
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            return Some(value);
        }
    }
    None
}

fn manifest_field(manifest: &Option<Value>, field: &str) -> Option<String> {
    manifest
        .as_ref()?
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// 组件目录（skills/commands/hooks）的直接子项数量；目录不存在为 0。
fn count_children(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|entries| entries.flatten().count())
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
            // marketplaces/ 目录本身由 write_marketplace 按需创建。
            Self { root }
        }

        fn plugins_root(&self) -> PathBuf {
            self.root.path().to_path_buf()
        }

        fn write_marketplace(&self, marketplace_id: &str, contents: &str) {
            let dir = self
                .plugins_root()
                .join("marketplaces")
                .join(marketplace_id);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("marketplace.json"), contents).unwrap();
        }

        fn write_plugin_cache(
            &self,
            marketplace_id: &str,
            plugin: &str,
            version: &str,
            manifest_rel: Option<&str>,
            manifest: &str,
            components: &[&str],
        ) -> PathBuf {
            let cache = self
                .plugins_root()
                .join("cache")
                .join(marketplace_id)
                .join(plugin)
                .join(version);
            fs::create_dir_all(&cache).unwrap();
            if let Some(rel) = manifest_rel {
                let manifest_path = join_relative(cache.clone(), rel);
                fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
                fs::write(manifest_path, manifest).unwrap();
            }
            for component in components {
                let child = join_relative(cache.clone(), component);
                fs::create_dir_all(child.parent().unwrap()).unwrap();
                fs::write(child, b"x").unwrap();
            }
            cache
        }

        fn create_data_dir(&self, plugin: &str, marketplace_id: &str) {
            let dir = self
                .plugins_root()
                .join("data")
                .join(format!("{plugin}@{marketplace_id}"));
            fs::create_dir_all(&dir).unwrap();
        }
    }

    /// 把缓存路径嵌入 JSON 字符串字面量（Windows 路径含反斜杠，需要转义）。
    fn json_path(path: &Path) -> String {
        path.display().to_string().replace('\\', "\\\\")
    }

    #[test]
    fn missing_plugins_root_yields_empty_list() {
        let fixture = Fixture::new();
        let missing = fixture.plugins_root().join("does-not-exist");
        assert!(scan_plugins_root(&missing).is_empty());
    }

    #[test]
    fn zcode_manifest_wins_over_claude_manifest() {
        let fixture = Fixture::new();
        let cache = fixture.plugins_root().join("cache/official/alpha/1.0.0");
        fixture.write_plugin_cache(
            "official",
            "alpha",
            "1.0.0",
            Some(".claude-plugin/plugin.json"),
            r#"{"name":"alpha","version":"9.9.9","description":"claude fallback","author":"claude"}"#,
            &[],
        );
        // 同一 cache 下再写优先级的 .zcode-plugin manifest。
        let zcode_manifest = join_relative(cache, ".zcode-plugin/plugin.json");
        fs::create_dir_all(zcode_manifest.parent().unwrap()).unwrap();
        fs::write(
            zcode_manifest,
            r#"{"name":"alpha","version":"1.0.1","description":"zcode manifest","author":"zai"}"#,
        )
        .unwrap();
        fixture.write_marketplace(
            "official",
            &format!(
                r#"{{"name":"official","plugins":[{{"name":"alpha","version":"0.0.1","cachePath":"{}","source":"filesystem"}}]}}"#,
                json_path(&fixture.plugins_root().join("cache/official/alpha/1.0.0"))
            ),
        );

        let plugins = scan_plugins_root(&fixture.plugins_root());
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].version, "1.0.1");
        assert_eq!(plugins[0].description, "zcode manifest");
        assert_eq!(plugins[0].author, "zai");
    }

    #[test]
    fn falls_back_to_claude_manifest_and_listed_version() {
        let fixture = Fixture::new();
        let cache = fixture.write_plugin_cache(
            "official",
            "beta",
            "2.0.0",
            Some(".claude-plugin/plugin.json"),
            r#"{"name":"beta","description":"only claude manifest"}"#,
            &[
                "skills/review/SKILL.md",
                "skills/debug/SKILL.md",
                "commands/go.md",
            ],
        );
        fixture.write_marketplace(
            "official",
            &format!(
                r#"{{"name":"official","plugins":[{{"name":"beta","version":"2.0.0","cachePath":"{}","source":"filesystem"}}]}}"#,
                json_path(&cache)
            ),
        );

        let plugins = scan_plugins_root(&fixture.plugins_root());
        assert_eq!(plugins.len(), 1);
        // manifest 缺 version 时退回市场登记版本。
        assert_eq!(plugins[0].version, "2.0.0");
        assert_eq!(plugins[0].description, "only claude manifest");
        assert_eq!(plugins[0].skill_count, 2);
        assert_eq!(plugins[0].command_count, 1);
        assert_eq!(plugins[0].hook_count, 0);
        assert!(!plugins[0].installed);
    }

    #[test]
    fn data_directory_marks_plugin_installed() {
        let fixture = Fixture::new();
        let cache = fixture.write_plugin_cache(
            "official",
            "gamma",
            "0.1.0",
            Some(".zcode-plugin/plugin.json"),
            r#"{"name":"gamma","version":"0.1.0"}"#,
            &[],
        );
        fixture.create_data_dir("gamma", "official");
        fixture.write_marketplace(
            "official",
            &format!(
                r#"{{"name":"official","plugins":[{{"name":"gamma","version":"0.1.0","cachePath":"{}"}}]}}"#,
                json_path(&cache)
            ),
        );

        let plugins = scan_plugins_root(&fixture.plugins_root());
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].installed);
        assert_eq!(plugins[0].id, "gamma@official");
    }

    #[test]
    fn corrupt_marketplace_json_is_skipped_without_killing_others() {
        let fixture = Fixture::new();
        fixture.write_marketplace("broken", "{not json");
        let cache = fixture.write_plugin_cache(
            "official",
            "delta",
            "3.0.0",
            Some(".zcode-plugin/plugin.json"),
            r#"{"name":"delta","version":"3.0.0"}"#,
            &[],
        );
        fixture.write_marketplace(
            "official",
            &format!(
                r#"{{"name":"official","plugins":[{{"name":"delta","version":"3.0.0","cachePath":"{}"}}]}}"#,
                json_path(&cache)
            ),
        );

        let plugins = scan_plugins_root(&fixture.plugins_root());
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "delta");
        assert_eq!(plugins[0].marketplace, "official");
    }

    #[test]
    fn corrupt_manifest_falls_back_to_listing_metadata() {
        let fixture = Fixture::new();
        let cache = fixture.write_plugin_cache(
            "official",
            "epsilon",
            "4.2.0",
            Some(".zcode-plugin/plugin.json"),
            "{corrupt",
            &[],
        );
        fixture.write_marketplace(
            "official",
            &format!(
                r#"{{"name":"official","plugins":[{{"name":"epsilon","version":"4.2.0","cachePath":"{}"}}]}}"#,
                json_path(&cache)
            ),
        );

        let plugins = scan_plugins_root(&fixture.plugins_root());
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].version, "4.2.0");
        assert_eq!(plugins[0].description, "");
    }
}

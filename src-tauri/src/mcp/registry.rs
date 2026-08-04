use std::path::PathBuf;

use crate::paths::join_relative;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum McpFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone)]
pub struct McpPlatformDef {
    pub id: String,
    pub display_name: String,
    pub presence_path: PathBuf,
    pub config_path: PathBuf,
    pub format: McpFormat,
    pub mcp_key: String,
}

pub fn builtin_mcp_platforms() -> Vec<McpPlatformDef> {
    let home = dirs::home_dir().expect("no home directory");
    vec![
        McpPlatformDef {
            id: "codex".into(),
            display_name: "Codex".into(),
            presence_path: home.join(".codex"),
            config_path: join_relative(home.clone(), ".codex/config.toml"),
            format: McpFormat::Toml,
            mcp_key: "mcp_servers".into(),
        },
        McpPlatformDef {
            id: "claude-code".into(),
            display_name: "Claude Code".into(),
            presence_path: home.join(".claude"),
            config_path: home.join(".claude.json"),
            format: McpFormat::Json,
            mcp_key: "mcpServers".into(),
        },
        McpPlatformDef {
            id: "cursor".into(),
            display_name: "Cursor".into(),
            presence_path: home.join(".cursor"),
            config_path: join_relative(home.clone(), ".cursor/mcp.json"),
            format: McpFormat::Json,
            mcp_key: "mcpServers".into(),
        },
        McpPlatformDef {
            id: "antigravity".into(),
            display_name: "Antigravity".into(),
            presence_path: join_relative(home.clone(), ".gemini/config"),
            config_path: join_relative(home.clone(), ".gemini/config/mcp_config.json"),
            format: McpFormat::Json,
            mcp_key: "mcpServers".into(),
        },
        McpPlatformDef {
            id: "grok-build".into(),
            display_name: "Grok Build".into(),
            presence_path: home.join(".grok"),
            config_path: join_relative(home.clone(), ".grok/config.toml"),
            format: McpFormat::Toml,
            mcp_key: "mcp_servers".into(),
        },
        McpPlatformDef {
            id: "kimi-code".into(),
            display_name: "Kimi Code".into(),
            presence_path: home.join(".kimi-code"),
            config_path: join_relative(home.clone(), ".kimi-code/mcp.json"),
            format: McpFormat::Json,
            mcp_key: "mcpServers".into(),
        },
        McpPlatformDef {
            id: "zcode".into(),
            display_name: "ZCode".into(),
            presence_path: home.join(".zcode"),
            config_path: join_relative(home.clone(), ".zcode/cli/config.json"),
            format: McpFormat::Json,
            // ZCode nests its name→server map one level down: mcp.servers.
            mcp_key: "mcp.servers".into(),
        },
        McpPlatformDef {
            id: "kiro".into(),
            display_name: "Kiro".into(),
            presence_path: home.join(".kiro"),
            config_path: join_relative(home.clone(), ".kiro/settings/mcp.json"),
            format: McpFormat::Json,
            mcp_key: "mcpServers".into(),
        },
    ]
}

pub fn find_mcp_platform(id: &str) -> Option<McpPlatformDef> {
    builtin_mcp_platforms().into_iter().find(|p| p.id == id)
}

/// Return the MCP definition for a selected project directory.
pub fn find_workspace_mcp_platform(
    id: &str,
    workspace: &std::path::Path,
) -> Option<McpPlatformDef> {
    let mut def = find_mcp_platform(id)?;
    def.presence_path = workspace.to_path_buf();
    def.config_path = match id {
        // Claude Code uses a repository-root .mcp.json for project scope.
        "claude-code" => workspace.join(".mcp.json"),
        "codex" => workspace.join(".codex").join("config.toml"),
        "antigravity" => workspace.join(".agents").join("mcp_config.json"),
        "grok-build" => workspace.join(".grok").join("config.toml"),
        "kimi-code" => workspace.join(".kimi-code").join("mcp.json"),
        "cursor" => workspace.join(".cursor").join("mcp.json"),
        "kiro" => workspace.join(".kiro").join("settings").join("mcp.json"),
        _ => return None,
    };
    Some(def)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_project_mcp_config_paths() {
        let root = PathBuf::from("/tmp/example-project");
        assert_eq!(
            find_workspace_mcp_platform("claude-code", &root)
                .unwrap()
                .config_path,
            root.join(".mcp.json")
        );
        assert_eq!(
            find_workspace_mcp_platform("codex", &root)
                .unwrap()
                .config_path,
            root.join(".codex").join("config.toml")
        );
        assert_eq!(
            find_workspace_mcp_platform("antigravity", &root)
                .unwrap()
                .config_path,
            root.join(".agents").join("mcp_config.json")
        );
        assert_eq!(
            find_workspace_mcp_platform("grok-build", &root)
                .unwrap()
                .config_path,
            root.join(".grok").join("config.toml")
        );
        assert_eq!(
            find_workspace_mcp_platform("kimi-code", &root)
                .unwrap()
                .config_path,
            root.join(".kimi-code").join("mcp.json")
        );
    }

    #[test]
    fn grok_build_reuses_the_codex_toml_layout() {
        let grok = find_mcp_platform("grok-build").expect("grok-build MCP platform");
        let codex = find_mcp_platform("codex").expect("codex MCP platform");
        assert_eq!(grok.format, McpFormat::Toml);
        assert_eq!(grok.mcp_key, codex.mcp_key);
    }

    #[test]
    fn zcode_uses_nested_mcp_servers_key() {
        let zcode = find_mcp_platform("zcode").expect("zcode MCP platform");
        assert_eq!(zcode.format, McpFormat::Json);
        assert_eq!(zcode.mcp_key, "mcp.servers");
        assert!(zcode
            .config_path
            .ends_with(".zcode/cli/config.json".replace('/', std::path::MAIN_SEPARATOR_STR)));
    }

    #[test]
    fn builtin_order_puts_cursor_after_claude_code() {
        let ids = builtin_mcp_platforms()
            .into_iter()
            .map(|platform| platform.id)
            .collect::<Vec<_>>();
        assert_eq!(&ids[..3], ["codex", "claude-code", "cursor"]);
    }
}

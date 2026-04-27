use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum McpFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone)]
pub struct McpPlatformDef {
    pub id: String,
    pub display_name: String,
    pub config_path: PathBuf,
    pub format: McpFormat,
    pub mcp_key: String,
}

pub fn builtin_mcp_platforms() -> Vec<McpPlatformDef> {
    let home = dirs::home_dir().expect("no home directory");
    vec![
        McpPlatformDef {
            id: "claude-code".into(),
            display_name: "Claude Code".into(),
            config_path: home.join(".claude.json"),
            format: McpFormat::Json,
            mcp_key: "mcpServers".into(),
        },
        McpPlatformDef {
            id: "cursor".into(),
            display_name: "Cursor".into(),
            config_path: home.join(".cursor/mcp.json"),
            format: McpFormat::Json,
            mcp_key: "mcpServers".into(),
        },
        McpPlatformDef {
            id: "codex-cli".into(),
            display_name: "Codex CLI".into(),
            config_path: home.join(".codex/config.toml"),
            format: McpFormat::Toml,
            mcp_key: "mcp_servers".into(),
        },
        McpPlatformDef {
            id: "gemini".into(),
            display_name: "Gemini".into(),
            config_path: home.join(".gemini/settings.json"),
            format: McpFormat::Json,
            mcp_key: "mcpServers".into(),
        },
        McpPlatformDef {
            id: "kiro".into(),
            display_name: "Kiro".into(),
            config_path: home.join(".kiro/settings/mcp.json"),
            format: McpFormat::Json,
            mcp_key: "mcpServers".into(),
        },
    ]
}

pub fn find_mcp_platform(id: &str) -> Option<McpPlatformDef> {
    builtin_mcp_platforms().into_iter().find(|p| p.id == id)
}

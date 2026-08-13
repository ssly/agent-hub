use std::fs;

use serde_json::Value;

use super::parser::{
    apply_json_server, apply_toml_server, parse_server_config_input_with_format,
    remove_json_server,
};
use super::registry::{find_mcp_platform, McpFormat};

pub fn save_mcp_server(platform_id: &str, name: &str, config: Value) -> Result<(), String> {
    let def = find_mcp_platform(platform_id).ok_or("Platform not found")?;
    match def.format {
        McpFormat::Json => save_json_server(&def, name, config),
        McpFormat::Toml => save_toml_server(&def, name, config),
        McpFormat::DshCordisPatch => Err(
            "DeepSeek Harness 的 MCP 由 profile 的 cordis.patch.yml 管理（mcp-client 插件），暂不支持在此编辑。".into(),
        ),
    }
}

pub fn delete_mcp_server(platform_id: &str, name: &str) -> Result<(), String> {
    let def = find_mcp_platform(platform_id).ok_or("Platform not found")?;
    match def.format {
        McpFormat::Json => delete_json_server(&def, name),
        McpFormat::Toml => delete_toml_server(&def, name),
        McpFormat::DshCordisPatch => Err(
            "DeepSeek Harness 的 MCP 由 profile 的 cordis.patch.yml 管理（mcp-client 插件），暂不支持在此编辑。".into(),
        ),
    }
}

pub fn import_mcp_server(platform_id: &str, name: &str, config_text: &str) -> Result<(), String> {
    let def = find_mcp_platform(platform_id).ok_or("Platform not found")?;
    let config =
        parse_server_config_input_with_format(config_text, &def.mcp_key, name, def.format)?;
    save_mcp_server(platform_id, name, config)
}

fn ensure_parent(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// --- JSON ---

fn save_json_server(
    def: &super::registry::McpPlatformDef,
    name: &str,
    config: Value,
) -> Result<(), String> {
    ensure_parent(&def.config_path)?;
    let before = if def.config_path.exists() {
        fs::read_to_string(&def.config_path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let content = apply_json_server(&before, &def.mcp_key, name, &config)
        .map_err(|e| format!("Invalid JSON in {}: {}", def.config_path.display(), e))?;
    fs::write(&def.config_path, content).map_err(|e| e.to_string())
}

fn delete_json_server(def: &super::registry::McpPlatformDef, name: &str) -> Result<(), String> {
    if !def.config_path.exists() {
        return Err("Config file not found".into());
    }
    let content = fs::read_to_string(&def.config_path).map_err(|e| e.to_string())?;
    // Surgical text edit: only the target server is removed; every other field
    // (including unrelated top-level keys in e.g. .claude.json) keeps its
    // original order and formatting. Falls back to "not found" error otherwise.
    let out = remove_json_server(&content, &def.mcp_key, name)?;
    fs::write(&def.config_path, out).map_err(|e| e.to_string())
}

// --- TOML ---

fn save_toml_server(
    def: &super::registry::McpPlatformDef,
    name: &str,
    config: Value,
) -> Result<(), String> {
    ensure_parent(&def.config_path)?;
    let before = if def.config_path.exists() {
        fs::read_to_string(&def.config_path).unwrap_or_default()
    } else {
        String::new()
    };
    let content = apply_toml_server(&before, &def.mcp_key, name, &config)?;
    fs::write(&def.config_path, content).map_err(|e| e.to_string())
}

fn delete_toml_server(def: &super::registry::McpPlatformDef, name: &str) -> Result<(), String> {
    if !def.config_path.exists() {
        return Err("Config file not found".into());
    }
    let content = fs::read_to_string(&def.config_path).map_err(|e| e.to_string())?;
    // Validate TOML is parseable
    let _doc: toml::Value = toml::from_str(&content).map_err(|e| format!("Invalid TOML: {}", e))?;

    let ranges = super::parser::find_toml_server_section_ranges(&content, &def.mcp_key, name);
    if !ranges.is_empty() {
        // Remove all matched section ranges (server + nested subtables),
        // then tidy up stray blank lines.
        let mut result = String::with_capacity(content.len());
        let mut cursor = 0usize;
        for r in &ranges {
            if r.start > cursor {
                result.push_str(&content[cursor..r.start]);
            }
            cursor = r.end;
        }
        if cursor < content.len() {
            result.push_str(&content[cursor..]);
        }
        while result.contains("\n\n\n") {
            result = result.replace("\n\n\n", "\n\n");
        }
        let trimmed = result.trim_end();
        let out = if trimmed.is_empty() {
            String::new()
        } else {
            format!("{}\n", trimmed)
        };
        return fs::write(&def.config_path, out).map_err(|e| e.to_string());
    }

    // Fallback: full re-serialization
    let mut doc: toml::Value =
        toml::from_str(&content).map_err(|e| format!("Invalid TOML: {}", e))?;
    let table = doc
        .as_table_mut()
        .ok_or("Config file is not a TOML table")?;
    if let Some(servers) = table.get_mut(&def.mcp_key).and_then(|v| v.as_table_mut()) {
        servers.remove(name);
    }
    let out = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(&def.config_path, out).map_err(|e| e.to_string())
}

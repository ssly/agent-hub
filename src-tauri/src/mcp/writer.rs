use std::fs;

use serde_json::Value;

use super::parser::parse_input_config;
use super::registry::{find_mcp_platform, McpFormat};

pub fn save_mcp_server(platform_id: &str, name: &str, config: Value) -> Result<(), String> {
    let def = find_mcp_platform(platform_id).ok_or("Platform not found")?;
    match def.format {
        McpFormat::Json => save_json_server(&def, name, config),
        McpFormat::Toml => save_toml_server(&def, name, config),
    }
}

pub fn delete_mcp_server(platform_id: &str, name: &str) -> Result<(), String> {
    let def = find_mcp_platform(platform_id).ok_or("Platform not found")?;
    match def.format {
        McpFormat::Json => delete_json_server(&def, name),
        McpFormat::Toml => delete_toml_server(&def, name),
    }
}

pub fn import_mcp_server(platform_id: &str, name: &str, config_text: &str) -> Result<(), String> {
    let config = parse_input_config(config_text)?;
    save_mcp_server(platform_id, name, config)
}

fn ensure_parent(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// --- JSON ---

fn read_or_create_json_doc(path: &std::path::Path) -> Result<Value, String> {
    if path.exists() {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(Value::Object(serde_json::Map::new()))
    }
}

fn save_json_server(def: &super::registry::McpPlatformDef, name: &str, config: Value) -> Result<(), String> {
    ensure_parent(&def.config_path)?;
    let mut doc = read_or_create_json_doc(&def.config_path)?;
    let servers = doc.as_object_mut()
        .ok_or("Config file is not a JSON object")?;
    servers.insert(def.mcp_key.clone(), {
        let mut servers_map = servers.get(&def.mcp_key)
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        servers_map.insert(name.to_string(), config);
        Value::Object(servers_map)
    });
    // If mcp_key was not already in the doc, we need to re-insert
    let obj = doc.as_object_mut().unwrap();
    if !obj.contains_key(&def.mcp_key) {
        // Already handled above
    }
    let content = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(&def.config_path, content).map_err(|e| e.to_string())
}

fn delete_json_server(def: &super::registry::McpPlatformDef, name: &str) -> Result<(), String> {
    if !def.config_path.exists() { return Err("Config file not found".into()); }
    let content = fs::read_to_string(&def.config_path).map_err(|e| e.to_string())?;
    let mut doc: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let obj = doc.as_object_mut().ok_or("Config file is not a JSON object")?;
    if let Some(servers) = obj.get_mut(&def.mcp_key).and_then(|v| v.as_object_mut()) {
        servers.remove(name);
    }
    let out = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(&def.config_path, out).map_err(|e| e.to_string())
}

// --- TOML ---

fn read_or_create_toml_doc(path: &std::path::Path) -> Result<toml::Value, String> {
    if path.exists() {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(toml::Value::Table(toml::map::Map::new()))
    }
}

fn save_toml_server(def: &super::registry::McpPlatformDef, name: &str, config: Value) -> Result<(), String> {
    ensure_parent(&def.config_path)?;
    let before = if def.config_path.exists() {
        fs::read_to_string(&def.config_path).unwrap_or_default()
    } else {
        String::new()
    };
    let section_header = format!("[{}.{}]", def.mcp_key, name);
    let new_config = toml::to_string_pretty(&super::parser::json_to_toml(&config)).map_err(|e| e.to_string())?;
    let new_section = format!("{}\n{}", section_header, new_config.trim_end());

    let content = if before.trim().is_empty() {
        new_section
    } else if let Some(pos) = before.find(&section_header) {
        // Replace existing section
        let after_header = &before[pos + section_header.len()..];
        let section_end = after_header.find("\n[").map(|i| pos + section_header.len() + i + 1).unwrap_or(before.len());
        let mut result = String::with_capacity(before.len() + new_section.len());
        result.push_str(&before[..pos]);
        result.push_str(&new_section);
        result.push('\n');
        if section_end < before.len() {
            result.push_str(&before[section_end..]);
        }
        result
    } else {
        // Append new section
        format!("{}\n\n{}\n", before.trim_end(), new_section)
    };

    fs::write(&def.config_path, content).map_err(|e| e.to_string())
}

fn delete_toml_server(def: &super::registry::McpPlatformDef, name: &str) -> Result<(), String> {
    if !def.config_path.exists() { return Err("Config file not found".into()); }
    let content = fs::read_to_string(&def.config_path).map_err(|e| e.to_string())?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| e.to_string())?;
    let table = doc.as_table_mut().ok_or("Config file is not a TOML table")?;
    if let Some(servers) = table.get_mut(&def.mcp_key).and_then(|v| v.as_table_mut()) {
        servers.remove(name);
    }
    let out = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(&def.config_path, out).map_err(|e| e.to_string())
}

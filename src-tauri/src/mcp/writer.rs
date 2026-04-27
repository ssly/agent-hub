use std::fs;

use serde_json::Value;

use super::parser::{json_to_toml, parse_input_config};
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
    let mut doc = read_or_create_toml_doc(&def.config_path)?;
    let table = doc.as_table_mut()
        .ok_or("Config file is not a TOML table")?;
    let servers = table.entry(&def.mcp_key)
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let servers_table = servers.as_table_mut()
        .ok_or("mcp_servers is not a table")?;
    servers_table.insert(name.to_string(), json_to_toml(&config));
    let content = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
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

use std::fs;

use serde_json::Value;

use super::parser::{parse_input_config, find_matching_brace};
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
    // For new files or files without the mcp_key, use clean serialization
    if !def.config_path.exists() {
        let mut doc = Value::Object(serde_json::Map::new());
        let mut servers_map = serde_json::Map::new();
        servers_map.insert(name.to_string(), config);
        doc.as_object_mut().unwrap().insert(def.mcp_key.clone(), Value::Object(servers_map));
        let content = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        return fs::write(&def.config_path, content).map_err(|e| e.to_string());
    }
    let before = fs::read_to_string(&def.config_path).map_err(|e| e.to_string())?;
    // If mcp_key doesn't exist in the file, fall back to clean serialization
    if !before.contains(&format!("\"{}\"", def.mcp_key)) {
        let mut doc: Value = serde_json::from_str(&before).map_err(|e| e.to_string())?;
        let mut servers_map = serde_json::Map::new();
        servers_map.insert(name.to_string(), config);
        doc.as_object_mut()
            .ok_or("Config file is not a JSON object")?
            .insert(def.mcp_key.clone(), Value::Object(servers_map));
        let content = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        return fs::write(&def.config_path, content).map_err(|e| e.to_string());
    }
    // Use targeted text replacement to preserve existing formatting
    let after = super::parser::apply_json_server(&before, &def.mcp_key, name, &config)?;
    fs::write(&def.config_path, after).map_err(|e| e.to_string())
}

fn delete_json_server(def: &super::registry::McpPlatformDef, name: &str) -> Result<(), String> {
    if !def.config_path.exists() { return Err("Config file not found".into()); }
    let content = fs::read_to_string(&def.config_path).map_err(|e| e.to_string())?;
    // Validate JSON is parseable
    let _doc: Value = serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {}", e))?;

    let server_key = format!("\"{}\"", name);
    if let Some(key_pos) = content.find(&server_key) {
        let after_key = &content[key_pos + server_key.len()..];
        let after_key_trimmed = after_key.trim_start();
        if after_key_trimmed.starts_with(':') {
            let colon_pos = key_pos + server_key.len() + (after_key.len() - after_key_trimmed.len());
            let after_colon = content[colon_pos + 1..].trim_start();
            if after_colon.starts_with('{') {
                let brace_start = colon_pos + 1 + (content[colon_pos + 1..].len() - after_colon.len());
                if let Ok(end_pos) = find_matching_brace(&content, brace_start) {
                    // Remove the comma before or after this entry
                    // Look backwards from key_pos for a preceding comma
                    let prefix = &content[..key_pos];
                    let has_comma_after = content[end_pos + 1..].trim_start().starts_with(',');
                    if has_comma_after {
                        // Remove from key_pos to after the comma
                        let after_comma = content[end_pos + 1..].find(',').unwrap_or(0);
                        let mut result = String::with_capacity(content.len());
                        result.push_str(&content[..key_pos]);
                        result.push_str(&content[end_pos + 1 + after_comma + 1..]);
                        return fs::write(&def.config_path, result.trim_end().to_string() + "\n").map_err(|e| e.to_string());
                    } else {
                        // This is the last entry — remove preceding comma
                        if let Some(last_comma) = prefix.rfind(',') {
                            // Check that this comma is between entries (not inside a string)
                            let between = &prefix[last_comma + 1..key_pos];
                            if !between.contains('"') {
                                let mut result = String::with_capacity(content.len());
                                result.push_str(&prefix[..last_comma]);
                                result.push_str(&content[end_pos + 1..]);
                                return fs::write(&def.config_path, result.trim_end().to_string() + "\n").map_err(|e| e.to_string());
                            }
                        }
                        // Fallback: just remove the block
                        let mut result = String::with_capacity(content.len());
                        result.push_str(&content[..key_pos]);
                        result.push_str(&content[end_pos + 1..]);
                        return fs::write(&def.config_path, result.trim_end().to_string() + "\n").map_err(|e| e.to_string());
                    }
                }
            }
        }
    }

    // Fallback: full re-serialization if targeted removal failed
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
    // Validate TOML is parseable
    let _doc: toml::Value = toml::from_str(&content).map_err(|e| format!("Invalid TOML: {}", e))?;

    let section_header = format!("[{}.{}]", def.mcp_key, name);
    if let Some(pos) = content.find(&section_header) {
        // Find end of this section: next "\n[" or end of file
        let after_header = &content[pos + section_header.len()..];
        let next_section = after_header.find("\n[");
        let section_end = match next_section {
            Some(i) => pos + section_header.len() + i + 1,
            None => content.len(),
        };
        // Remove leading newlines before the section header if needed
        let prefix = &content[..pos];
        let prefix_trimmed = prefix.trim_end();
        let trim_len = prefix.len() - prefix_trimmed.len();
        let result = format!("{}{}", prefix_trimmed, &content[section_end..]);
        return fs::write(&def.config_path, result.trim_end().to_string() + "\n").map_err(|e| e.to_string());
    }

    // Fallback: full re-serialization
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| e.to_string())?;
    let table = doc.as_table_mut().ok_or("Config file is not a TOML table")?;
    if let Some(servers) = table.get_mut(&def.mcp_key).and_then(|v| v.as_table_mut()) {
        servers.remove(name);
    }
    let out = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(&def.config_path, out).map_err(|e| e.to_string())
}

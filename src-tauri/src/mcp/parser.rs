use std::fs;

use serde_json::Value;
use similar::{ChangeTag, TextDiff};

use super::registry::{find_mcp_platform, McpFormat};

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpServer {
    pub name: String,
    pub config: Value,
}

pub fn read_mcp_servers(platform_id: &str) -> Result<Vec<McpServer>, String> {
    let def = find_mcp_platform(platform_id).ok_or("Platform not found")?;
    if !def.config_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&def.config_path).map_err(|e| e.to_string())?;
    let servers = match def.format {
        McpFormat::Json => parse_json_servers(&content, &def.mcp_key),
        McpFormat::Toml => parse_toml_servers(&content, &def.mcp_key),
    }?;
    Ok(servers)
}

pub fn read_mcp_server(platform_id: &str, name: &str) -> Result<McpServer, String> {
    let servers = read_mcp_servers(platform_id)?;
    servers
        .into_iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("Server '{}' not found", name))
}

fn parse_json_servers(content: &str, mcp_key: &str) -> Result<Vec<McpServer>, String> {
    let mut doc: Value = serde_json::from_str(content).map_err(|e| e.to_string())?;
    let servers_obj = match doc.get_mut(mcp_key).and_then(|v| v.as_object_mut()) {
        Some(obj) => obj,
        None => return Ok(Vec::new()),
    };

    let mut servers = Vec::new();
    let keys: Vec<String> = servers_obj.keys().cloned().collect();
    for key in keys {
        if let Some(config) = servers_obj.get(&key).cloned() {
            servers.push(McpServer { name: key, config });
        }
    }
    servers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(servers)
}

fn parse_toml_servers(content: &str, mcp_key: &str) -> Result<Vec<McpServer>, String> {
    let mut doc: toml::Value = toml::from_str(content).map_err(|e| e.to_string())?;
    let table = match doc.get_mut(mcp_key).and_then(|v| v.as_table_mut()) {
        Some(t) => t,
        None => return Ok(Vec::new()),
    };

    let mut servers = Vec::new();
    let keys: Vec<String> = table.keys().cloned().collect();
    for key in keys {
        if let Some(val) = table.get(&key).cloned() {
            let json_val = toml_to_json(&val);
            servers.push(McpServer {
                name: key,
                config: json_val,
            });
        }
    }
    servers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(servers)
}

pub fn toml_to_json(val: &toml::Value) -> Value {
    match val {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Array(arr) => Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(tbl) => {
            let map: serde_json::Map<String, Value> = tbl
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            Value::Object(map)
        }
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
    }
}

pub fn json_to_toml(val: &Value) -> toml::Value {
    toml::Value::try_from(val).unwrap_or_else(|_| toml::Value::String(val.to_string()))
}

pub fn detect_format(text: &str) -> McpFormat {
    let trimmed = text.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        McpFormat::Json
    } else {
        McpFormat::Toml
    }
}

pub fn parse_input_config(text: &str) -> Result<Value, String> {
    let format = detect_format(text);
    match format {
        McpFormat::Json => {
            serde_json::from_str(text).map_err(|e| format!("JSON parse error: {}", e))
        }
        McpFormat::Toml => {
            let toml_val: toml::Value =
                toml::from_str(text).map_err(|e| format!("TOML parse error: {}", e))?;
            Ok(toml_to_json(&toml_val))
        }
    }
}

pub fn config_to_display(config: &Value, format: McpFormat) -> String {
    match format {
        McpFormat::Json => serde_json::to_string_pretty(config).unwrap_or_default(),
        McpFormat::Toml => {
            let toml_val = json_to_toml(config);
            toml::to_string_pretty(&toml_val).unwrap_or_default()
        }
    }
}

// --- MCP Sync Preview ---

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiffLine {
    pub tag: String, // "context" | "added" | "removed"
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpSyncPreview {
    pub server_name: String,
    pub target_format: String,
    pub target_config_path: String,
    pub has_conflict: bool,
    pub diff_lines: Vec<DiffLine>,
    pub added: usize,
    pub removed: usize,
}

pub fn preview_mcp_sync(
    source_platform_id: &str,
    target_platform_id: &str,
    server_name: &str,
) -> Result<McpSyncPreview, String> {
    let source_server = read_mcp_server(source_platform_id, server_name)?;
    let target_def = find_mcp_platform(target_platform_id).ok_or("Target platform not found")?;
    let has_conflict = read_mcp_server(target_platform_id, server_name).is_ok();

    let before_text = if target_def.config_path.exists() {
        fs::read_to_string(&target_def.config_path).unwrap_or_default()
    } else {
        match target_def.format {
            McpFormat::Json => "{}".to_string(),
            McpFormat::Toml => String::new(),
        }
    };

    let after_text = match target_def.format {
        McpFormat::Json => apply_json_server(
            &before_text,
            &target_def.mcp_key,
            server_name,
            &source_server.config,
        ),
        McpFormat::Toml => apply_toml_server(
            &before_text,
            &target_def.mcp_key,
            server_name,
            &source_server.config,
        ),
    }?;

    let diff_lines = compute_text_diff(&before_text, &after_text);
    let added = diff_lines.iter().filter(|l| l.tag == "added").count();
    let removed = diff_lines.iter().filter(|l| l.tag == "removed").count();

    Ok(McpSyncPreview {
        server_name: server_name.to_string(),
        target_format: match target_def.format {
            McpFormat::Json => "json",
            McpFormat::Toml => "toml",
        }
        .to_string(),
        target_config_path: target_def.config_path.display().to_string(),
        has_conflict,
        diff_lines,
        added,
        removed,
    })
}

pub(crate) fn apply_json_server(
    before: &str,
    mcp_key: &str,
    name: &str,
    config: &Value,
) -> Result<String, String> {
    let new_config_str = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    // Build indented config body. Prettified output:
    //   {
    //     "key": "value"
    //   }
    // We need: content at level 3 (6 spaces), closing } at level 2 (4 spaces).
    let lines: Vec<&str> = new_config_str.lines().collect();
    let mut inner_body = String::new();
    for line in &lines[1..lines.len() - 1] {
        // Add 4 spaces to existing 2-space indent → 6 spaces (level 3)
        inner_body.push_str("    ");
        inner_body.push_str(line);
        inner_body.push('\n');
    }
    // Closing brace at level 2 (4 spaces)
    inner_body.push_str("    }");

    if before.trim().is_empty() || before.trim() == "{}" {
        return Ok(format!(
            "{{\n  \"{mcp_key}\": {{\n    \"{name}\": {{\n{inner_body}\n  }}\n}}",
        ));
    }

    // If the file doesn't contain the mcp_key at all, treat as fresh
    if !before.contains(&format!("\"{}\"", mcp_key)) {
        let mut doc: Value = serde_json::from_str(before).map_err(|e| e.to_string())?;
        let obj = doc.as_object_mut().ok_or("Not a JSON object")?;
        let mut servers_map = serde_json::Map::new();
        servers_map.insert(name.to_string(), config.clone());
        obj.insert(mcp_key.to_string(), Value::Object(servers_map));
        return serde_json::to_string_pretty(&doc).map_err(|e| e.to_string());
    }

    let server_key = format!("\"{}\"", name);

    // Update existing server — find "name": { ... } and replace the block
    if let Some(key_pos) = before.find(&server_key) {
        let after_key = &before[key_pos + server_key.len()..];
        let after_key_trimmed = after_key.trim_start();
        if after_key_trimmed.starts_with(':') {
            let colon_pos =
                key_pos + server_key.len() + (after_key.len() - after_key_trimmed.len());
            let after_colon = before[colon_pos + 1..].trim_start();
            if after_colon.starts_with('{') {
                let brace_start =
                    colon_pos + 1 + (before[colon_pos + 1..].len() - after_colon.len());
                let end_pos = find_matching_brace(before, brace_start)?;
                let after_end = before[end_pos + 1..].trim_start();
                let has_comma = after_end.starts_with(',');
                let delete_end = if has_comma {
                    end_pos + 1 + after_end[1..].find(',').unwrap_or(0) + 1
                } else {
                    end_pos
                };
                let mut result = String::with_capacity(before.len() + inner_body.len());
                result.push_str(&before[..brace_start]);
                result.push_str(&inner_body);
                if has_comma {
                    result.push(',');
                }
                result.push_str(&before[delete_end + 1..]);
                return Ok(result);
            }
        }
    }

    // Append new server to the mcp_key object
    if let Some(mcp_pos) = before.find(&format!("\"{}\"", mcp_key)) {
        if let Some(obj_start) = before[mcp_pos..].find('{') {
            let abs_start = mcp_pos + obj_start;
            if let Ok(obj_end) = find_matching_brace(before, abs_start) {
                let inner_content = before[abs_start + 1..obj_end].trim();
                let comma = if inner_content.is_empty() { "" } else { "," };
                let insert = format!("{comma}\n    \"{name}\": {{\n{inner_body}");
                let mut result = String::with_capacity(before.len() + insert.len());
                result.push_str(before[..obj_end].trim_end());
                result.push_str(&insert);
                result.push('\n');
                result.push_str(&before[obj_end..]);
                return Ok(result);
            }
        }
    }

    Err("Could not locate MCP key in JSON for targeted edit".into())
}

/// Find the matching `}` for a `{` at `open_pos` in `text`
pub(crate) fn find_matching_brace(text: &str, open_pos: usize) -> Result<usize, String> {
    let chars: Vec<char> = text[open_pos..].chars().collect();
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (i, &ch) in chars.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth -= 1;
            if depth == 0 {
                return Ok(open_pos + i);
            }
        }
    }
    Err("Unmatched brace".into())
}

fn apply_toml_server(
    before: &str,
    mcp_key: &str,
    name: &str,
    config: &Value,
) -> Result<String, String> {
    let section_header = format!("[{}.{}]", mcp_key, name);
    let new_config = toml::to_string_pretty(&json_to_toml(config)).map_err(|e| e.to_string())?;
    let new_section = format!("{}\n{}", section_header, new_config.trim_end());

    if before.trim().is_empty() {
        return Ok(new_section);
    }

    // Check if the section already exists
    if let Some(pos) = before.find(&section_header) {
        // Find the end of this section: next "[xxx" at line start or end of file
        let after_header = &before[pos + section_header.len()..];
        let section_end = after_header
            .find("\n[")
            .map(|i| pos + section_header.len() + i + 1)
            .unwrap_or(before.len());
        // Replace: keep before + new section + after
        let mut result = String::with_capacity(before.len() + new_section.len());
        result.push_str(&before[..pos]);
        result.push_str(&new_section);
        result.push('\n');
        if section_end < before.len() {
            result.push_str(&before[section_end..]);
        }
        Ok(result)
    } else {
        // Append new section at end
        let trimmed = before.trim_end();
        Ok(format!("{}\n\n{}\n", trimmed, new_section))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_json_targeted_edit_existing_server() {
        let before = r#"{
  "mcpServers": {
    "server-a": {
      "command": "npx",
      "args": ["-y", "old-pkg"]
    },
    "context7": {
      "command": "old-cmd"
    },
    "server-z": {
      "command": "echo"
    }
  }
}"#;
        let new_config = serde_json::json!({
            "command": "npx",
            "args": ["-y", "@anthropic-ai/context7"]
        });
        let result = apply_json_server(before, "mcpServers", "context7", &new_config).unwrap();
        // Should only change the context7 block, not touch server-a or server-z
        assert!(result.contains("\"server-a\""));
        assert!(result.contains("\"server-z\""));
        assert!(result.contains("\"echo\""));
        assert!(result.contains("@anthropic-ai/context7"));
        assert!(!result.contains("old-cmd"));
        // server-a formatting should remain untouched
        assert!(result.contains("\"npx\","));
        assert!(result.contains("\"old-pkg\""));
    }

    #[test]
    fn test_apply_json_targeted_edit_new_server() {
        let before = r#"{
  "mcpServers": {
    "existing": {
      "command": "echo"
    }
  }
}"#;
        let new_config = serde_json::json!({"command": "npx", "args": ["test"]});
        let result = apply_json_server(before, "mcpServers", "new-server", &new_config).unwrap();
        assert!(result.contains("\"existing\""));
        assert!(result.contains("\"new-server\""));
        assert!(result.contains("\"echo\""));
        assert!(result.contains("\"test\""));
        // Verify JSON is valid
        let _: Value = serde_json::from_str(&result).expect("Result should be valid JSON");
    }

    #[test]
    fn test_apply_json_targeted_edit_new_server_empty_object() {
        let before = r#"{
  "mcpServers": {}
}"#;
        let new_config = serde_json::json!({"command": "npx"});
        let result = apply_json_server(before, "mcpServers", "first-server", &new_config).unwrap();
        assert!(result.contains("\"first-server\""));
        assert!(result.contains("\"npx\""));
        let _: Value = serde_json::from_str(&result).expect("Result should be valid JSON");
    }

    #[test]
    fn test_apply_json_targeted_edit_empty_file() {
        let config = serde_json::json!({"command": "npx"});
        let result = apply_json_server("{}", "mcpServers", "test-srv", &config).unwrap();
        assert!(result.contains("\"test-srv\""));
        assert!(result.contains("\"npx\""));
        let _: Value = serde_json::from_str(&result).expect("Result should be valid JSON");
    }

    #[test]
    fn test_find_matching_brace() {
        let text = r#"{"a": {"b": [1, 2, 3]}, "c": "x"}"#;
        let pos = find_matching_brace(text, 4).unwrap(); // opening { after {"a":
                                                         // Should find the matching } after [1, 2, 3]
        let inner = &text[4..=pos];
        assert!(inner.contains("[1, 2, 3]"));
    }
}

fn compute_text_diff(before: &str, after: &str) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(before, after);
    diff.iter_all_changes()
        .map(|change| {
            let content = change.to_string_lossy().into_owned();
            let tag = match change.tag() {
                ChangeTag::Equal => "context",
                ChangeTag::Insert => "added",
                ChangeTag::Delete => "removed",
            };
            DiffLine {
                tag: tag.to_string(),
                content,
            }
        })
        .collect()
}

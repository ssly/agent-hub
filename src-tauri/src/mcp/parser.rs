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
    if before.trim().is_empty() || before.trim() == "{}" {
        let mut doc = serde_json::Map::new();
        let mut servers = serde_json::Map::new();
        servers.insert(name.to_string(), config.clone());
        doc.insert(mcp_key.to_string(), Value::Object(servers));
        return serde_json::to_string_pretty(&Value::Object(doc)).map_err(|e| e.to_string());
    }

    let mut doc: Value = serde_json::from_str(before).map_err(|e| e.to_string())?;
    let obj = doc.as_object_mut().ok_or("Not a JSON object")?;
    let servers = obj
        .entry(mcp_key)
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or(format!("'{}' is not an object", mcp_key))?;
    servers.insert(name.to_string(), config.clone());
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}

/// Find the matching `}` for a `{` at `open_pos` in `text`
#[allow(dead_code)]
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
        // Verify valid JSON
        let parsed: Value = serde_json::from_str(&result).expect("Result should be valid JSON");
        let servers = parsed["mcpServers"].as_object().unwrap();
        // All 3 servers must exist
        assert!(servers.contains_key("server-a"));
        assert!(servers.contains_key("context7"));
        assert!(servers.contains_key("server-z"));
        // context7 updated
        assert_eq!(servers["context7"]["args"][1], "@anthropic-ai/context7");
        assert!(!result.contains("old-cmd"));
        // server-a and server-z untouched
        assert_eq!(servers["server-a"]["command"], "npx");
        assert_eq!(servers["server-a"]["args"][1], "old-pkg");
        assert_eq!(servers["server-z"]["command"], "echo");
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

    #[test]
    fn test_apply_json_update_non_last_server_preserves_following_entries() {
        // Bug repro: updating a non-last server must not corrupt the next entry
        let before = r#"{
  "mcpServers": {
    "context7": {
      "command": "old-cmd"
    },
    "server-b": {
      "command": "keep-me"
    }
  }
}"#;
        let new_config = serde_json::json!({"command": "new-cmd", "args": ["--flag"]});
        let result = apply_json_server(before, "mcpServers", "context7", &new_config).unwrap();
        // Must produce valid JSON
        let parsed: Value = serde_json::from_str(&result).expect("Result should be valid JSON");
        // Both servers must still exist
        assert!(
            parsed["mcpServers"]["context7"].is_object(),
            "context7 should exist"
        );
        assert!(
            parsed["mcpServers"]["server-b"].is_object(),
            "server-b must NOT be corrupted"
        );
        assert_eq!(
            parsed["mcpServers"]["context7"]["command"], "new-cmd",
            "context7 should be updated"
        );
        assert_eq!(
            parsed["mcpServers"]["server-b"]["command"], "keep-me",
            "server-b must be untouched"
        );
    }

    /// Helper: verify result is valid JSON and contains all expected server names
    fn assert_valid_with_servers(result: &str, expected_servers: &[&str]) -> Value {
        let parsed: Value =
            serde_json::from_str(result).unwrap_or_else(|e| panic!("Invalid JSON: {}\nGot:\n{}", e, result));
        let servers = parsed["mcpServers"]
            .as_object()
            .expect("mcpServers should be an object");
        for &name in expected_servers {
            assert!(
                servers.contains_key(name),
                "Server '{}' missing. Got keys: {:?}\nFull JSON:\n{}",
                name,
                servers.keys().collect::<Vec<_>>(),
                result
            );
        }
        parsed
    }

    #[test]
    fn test_sync_all_paths_comprehensive() {
        let config_a = serde_json::json!({"command": "npx", "args": ["-y", "pkg-a"]});
        let config_b = serde_json::json!({"command": "npx", "args": ["-y", "pkg-b"]});
        let config_c = serde_json::json!({"command": "npx", "args": ["-y", "pkg-c"]});
        let config_new = serde_json::json!({"command": "npx", "args": ["-y", "new-pkg"], "env": {"KEY": "val"}});

        // 1. Empty file → first server
        let r = apply_json_server("{}", "mcpServers", "srv-a", &config_a).unwrap();
        let p = assert_valid_with_servers(&r, &["srv-a"]);
        assert_eq!(p["mcpServers"]["srv-a"]["command"], "npx");

        // 2. Existing file with one server → append second
        let one_server = "{\n  \"mcpServers\": {\n    \"srv-a\": {\n      \"command\": \"npx\"\n    }\n  }\n}";
        let r = apply_json_server(one_server, "mcpServers", "srv-b", &config_b).unwrap();
        let _p = assert_valid_with_servers(&r, &["srv-a", "srv-b"]);

        // 3. Two servers → append third
        let two_servers = "{\n  \"mcpServers\": {\n    \"srv-a\": { \"command\": \"a\" },\n    \"srv-b\": { \"command\": \"b\" }\n  }\n}";
        let r = apply_json_server(two_servers, "mcpServers", "srv-c", &config_c).unwrap();
        let _p = assert_valid_with_servers(&r, &["srv-a", "srv-b", "srv-c"]);

        // 4. Update only server (no comma after)
        let one_srv = "{\n  \"mcpServers\": {\n    \"srv-a\": {\n      \"command\": \"old\"\n    }\n  }\n}";
        let r = apply_json_server(one_srv, "mcpServers", "srv-a", &config_new).unwrap();
        let p = assert_valid_with_servers(&r, &["srv-a"]);
        assert_eq!(p["mcpServers"]["srv-a"]["command"], "npx");
        assert_eq!(p["mcpServers"]["srv-a"]["env"]["KEY"], "val");

        // 5. Update first of two (comma after) — THE critical path
        let two = "{\n  \"mcpServers\": {\n    \"srv-a\": {\n      \"command\": \"old\"\n    },\n    \"srv-b\": {\n      \"command\": \"keep\"\n    }\n  }\n}";
        let r = apply_json_server(two, "mcpServers", "srv-a", &config_new).unwrap();
        let p = assert_valid_with_servers(&r, &["srv-a", "srv-b"]);
        assert_eq!(p["mcpServers"]["srv-a"]["command"], "npx");
        assert_eq!(p["mcpServers"]["srv-b"]["command"], "keep");

        // 6. Update last of two (no comma after)
        let r = apply_json_server(two, "mcpServers", "srv-b", &config_new).unwrap();
        let p = assert_valid_with_servers(&r, &["srv-a", "srv-b"]);
        assert_eq!(p["mcpServers"]["srv-a"]["command"], "old");
        assert_eq!(p["mcpServers"]["srv-b"]["command"], "npx");

        // 7. Update middle of three (comma both sides)
        let three = "{\n  \"mcpServers\": {\n    \"srv-a\": { \"command\": \"a\" },\n    \"srv-b\": { \"command\": \"b\" },\n    \"srv-c\": { \"command\": \"c\" }\n  }\n}";
        let r = apply_json_server(three, "mcpServers", "srv-b", &config_new).unwrap();
        let p = assert_valid_with_servers(&r, &["srv-a", "srv-b", "srv-c"]);
        assert_eq!(p["mcpServers"]["srv-a"]["command"], "a");
        assert_eq!(p["mcpServers"]["srv-b"]["command"], "npx");
        assert_eq!(p["mcpServers"]["srv-c"]["command"], "c");

        // 8. Update first of three (comma after)
        let r = apply_json_server(three, "mcpServers", "srv-a", &config_new).unwrap();
        let p = assert_valid_with_servers(&r, &["srv-a", "srv-b", "srv-c"]);
        assert_eq!(p["mcpServers"]["srv-a"]["command"], "npx");
        assert_eq!(p["mcpServers"]["srv-b"]["command"], "b");
        assert_eq!(p["mcpServers"]["srv-c"]["command"], "c");

        // 9. Update last of three (no comma after)
        let r = apply_json_server(three, "mcpServers", "srv-c", &config_new).unwrap();
        let p = assert_valid_with_servers(&r, &["srv-a", "srv-b", "srv-c"]);
        assert_eq!(p["mcpServers"]["srv-a"]["command"], "a");
        assert_eq!(p["mcpServers"]["srv-b"]["command"], "b");
        assert_eq!(p["mcpServers"]["srv-c"]["command"], "npx");
    }

    #[test]
    fn test_sync_preview_equals_actual_sync() {
        // The preview and actual sync must produce identical output
        let configs = [
            serde_json::json!({"command": "npx", "args": ["-y", "old-pkg"]}),
            serde_json::json!({"command": "npx", "args": ["-y", "new-pkg"], "env": {"X": "1"}}),
        ];
        let targets = [
            "{}",
            "{\n  \"mcpServers\": {\n    \"other\": { \"command\": \"echo\" }\n  }\n}",
            "{\n  \"mcpServers\": {\n    \"ctx7\": { \"command\": \"old\" },\n    \"other\": { \"command\": \"echo\" }\n  }\n}",
            "{\n  \"mcpServers\": {\n    \"other\": { \"command\": \"echo\" },\n    \"ctx7\": { \"command\": \"old\" }\n  }\n}",
        ];

        for (i, target) in targets.iter().enumerate() {
            for (j, config) in configs.iter().enumerate() {
                let result = apply_json_server(target, "mcpServers", "ctx7", config);
                assert!(
                    result.is_ok(),
                    "Failed for target[{}] config[{}]: {}",
                    i, j, result.unwrap_err()
                );
                let text = result.unwrap();
                let _: Value = serde_json::from_str(&text).unwrap_or_else(|e| {
                    panic!("Invalid JSON for target[{}] config[{}]: {}\nGot:\n{}", i, j, e, text)
                });
            }
        }
    }

    #[test]
    fn test_sync_write_read_roundtrip() {
        // Simulate the full sync flow: apply → parse_json_servers → verify config
        let source_config = serde_json::json!({
            "command": "npx",
            "args": ["-y", "@anthropic-ai/context7"],
            "env": {"API_KEY": "secret"}
        });
        let target_before = "{\n  \"mcpServers\": {\n    \"existing\": { \"command\": \"echo\" }\n  }\n}";

        let after = apply_json_server(target_before, "mcpServers", "context7", &source_config).unwrap();
        // Verify the written content can be re-parsed and yields the same config
        let servers = parse_json_servers(&after, "mcpServers").unwrap();
        let ctx = servers.iter().find(|s| s.name == "context7").expect("context7 should exist");
        assert_eq!(ctx.config, source_config);
        let ex = servers.iter().find(|s| s.name == "existing").expect("existing should be preserved");
        assert_eq!(ex.config["command"], "echo");
    }

    #[test]
    fn test_sync_update_existing_roundtrip() {
        // Sync to target that already has the same server → update
        let new_config = serde_json::json!({"command": "npx", "args": ["-y", "updated"]});
        let target_before = r#"{
  "mcpServers": {
    "context7": {
      "command": "old"
    },
    "other": {
      "command": "keep"
    }
  }
}"#;
        let after = apply_json_server(target_before, "mcpServers", "context7", &new_config).unwrap();
        let servers = parse_json_servers(&after, "mcpServers").unwrap();
        let ctx = servers.iter().find(|s| s.name == "context7").unwrap();
        assert_eq!(ctx.config["args"][1], "updated");
        let other = servers.iter().find(|s| s.name == "other").unwrap();
        assert_eq!(other.config["command"], "keep");
    }

    #[test]
    fn test_sync_targets_root_level_not_project_level() {
        // .claude.json may contain project-level mcpServers under projects.<path>.
        // apply_json_server must target the ROOT-level mcpServers, not project-level.
        let before = r#"{
  "projects": {
    "D:/Coding": {
      "mcpServers": {
        "proj-server": {
          "command": "proj-cmd"
        }
      }
    }
  }
}"#;
        let config = serde_json::json!({"command": "npx", "args": ["-y", "ctx7"]});
        let result = apply_json_server(before, "mcpServers", "context7", &config).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("must be valid JSON");
        // Root-level mcpServers must be created with context7
        assert!(
            parsed["mcpServers"]["context7"].is_object(),
            "root mcpServers.context7 must exist"
        );
        assert_eq!(parsed["mcpServers"]["context7"]["command"], "npx");
        // Project-level must be untouched
        assert!(
            parsed["projects"]["D:/Coding"]["mcpServers"]["proj-server"].is_object(),
            "project-level mcpServers must be preserved"
        );
        assert_eq!(
            parsed["projects"]["D:/Coding"]["mcpServers"]["proj-server"]["command"],
            "proj-cmd"
        );
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

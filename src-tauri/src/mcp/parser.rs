use std::{fs, ops::Range};

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

pub fn parse_input_config(text: &str) -> Result<Value, String> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        return serde_json::from_str(text).map_err(|e| format!("JSON parse error: {}", e));
    }
    if trimmed.starts_with('[') {
        if let Ok(json) = serde_json::from_str(text) {
            return Ok(json);
        }
    }

    let toml_val: toml::Value =
        toml::from_str(text).map_err(|e| format!("TOML parse error: {}", e))?;
    Ok(toml_to_json(&toml_val))
}

pub fn parse_server_config_input(text: &str, mcp_key: &str, name: &str) -> Result<Value, String> {
    let config = parse_input_config(text)?;
    if let Some(wrapped) = config
        .get(mcp_key)
        .and_then(|servers| servers.get(name))
        .cloned()
    {
        return Ok(wrapped);
    }
    if let Some(wrapped) = config.get(name).cloned() {
        if config.as_object().map(|obj| obj.len()) == Some(1) {
            return Ok(wrapped);
        }
    }
    Ok(config)
}

/// Parse server config with format enforcement.
/// For `Toml` format, only TOML input is accepted.
/// For `Json` format, only JSON input is accepted.
pub fn parse_server_config_input_with_format(
    text: &str,
    mcp_key: &str,
    name: &str,
    format: McpFormat,
) -> Result<Value, String> {
    let trimmed = text.trim();

    match format {
        McpFormat::Toml => {
            // For TOML platforms (Codex), only accept TOML input.
            // Reject pure JSON objects.
            if trimmed.starts_with('{') {
                return Err("Codex only supports TOML format. Please paste TOML config.".into());
            }
            let toml_val: toml::Value =
                toml::from_str(text).map_err(|e| format!("TOML parse error: {}", e))?;
            let config = toml_to_json(&toml_val);

            // Unwrap: [mcp_servers.name] -> inner config
            if let Some(wrapped) = config
                .get(mcp_key)
                .and_then(|servers| servers.get(name))
                .cloned()
            {
                return Ok(wrapped);
            }
            if let Some(wrapped) = config.get(name).cloned() {
                if config.as_object().map(|obj| obj.len()) == Some(1) {
                    return Ok(wrapped);
                }
            }
            Ok(config)
        }
        McpFormat::Json => {
            // For JSON platforms, only accept JSON input.
            if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
                return Err("This platform only supports JSON format.".into());
            }
            parse_server_config_input(text, mcp_key, name)
        }
    }
}

pub fn config_to_display(config: &Value, format: McpFormat, mcp_key: &str, name: &str) -> String {
    match format {
        McpFormat::Json => serde_json::to_string_pretty(config).unwrap_or_default(),
        McpFormat::Toml => {
            // Wrap config as {mcp_key: {name: config}} so nested subtables
            // (e.g. `env`) serialize with the full dotted path
            // `[mcp_servers.<name>.env]` instead of a context-less `[env]`.
            let mut server_table = serde_json::Map::new();
            server_table.insert(name.to_string(), config.clone());
            let mut root = serde_json::Map::new();
            root.insert(mcp_key.to_string(), Value::Object(server_table));
            let toml_val = json_to_toml(&Value::Object(root));
            toml::to_string_pretty(&toml_val)
                .unwrap_or_default()
                .trim_end()
                .to_string()
        }
    }
}

// --- Sync Core Extraction ---

/// Extracts the universal core fields for cross-platform sync:
/// `command`, `args`, and `env` (only when non-empty).
/// All platform-specific fields (type, cwd, timeout, startup_timeout_sec, etc.) are dropped.
pub fn extract_sync_core(config: &Value) -> Value {
    let mut core = serde_json::Map::new();
    if let Some(obj) = config.as_object() {
        for key in ["command", "args", "env"] {
            if let Some(val) = obj.get(key) {
                if key == "env" && val.as_object().map(|m| m.is_empty()).unwrap_or(false) {
                    continue;
                }
                core.insert(key.to_string(), val.clone());
            }
        }
    }
    Value::Object(core)
}

/// Builds the effective config to write during sync:
/// extract core from source, then overlay onto the existing target config
/// (if any) so that platform-specific fields in the target are preserved.
/// Returns `None` if the merged result equals the existing target entry
/// (no-op: nothing would change).
pub fn build_sync_config(
    source_config: &Value,
    target_platform_id: &str,
    server_name: &str,
) -> Result<Option<Value>, String> {
    let core = extract_sync_core(source_config);

    // If the target already has this server, merge core into its existing config.
    let merged = match read_mcp_server(target_platform_id, server_name) {
        Ok(existing) => {
            let mut base = existing.config.clone();
            if let (Some(base_obj), Some(core_obj)) = (base.as_object_mut(), core.as_object()) {
                for (k, v) in core_obj {
                    base_obj.insert(k.clone(), v.clone());
                }
            }
            base
        }
        Err(_) => core,
    };

    // No-op: if the merged config equals what's already in the target, skip write.
    if let Ok(existing) = read_mcp_server(target_platform_id, server_name) {
        if existing.config == merged {
            return Ok(None);
        }
    }

    Ok(Some(merged))
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

    // Build the effective sync payload: core fields merged into the existing
    // target entry (if any) so platform-specific fields are preserved in the preview.
    let sync_config =
        build_sync_config(&source_server.config, target_platform_id, server_name)?
            .unwrap_or_else(|| extract_sync_core(&source_server.config));

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
            &sync_config,
        ),
        McpFormat::Toml => apply_toml_server(
            &before_text,
            &target_def.mcp_key,
            server_name,
            &sync_config,
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
        let result = format_new_json_doc(mcp_key, name, config)?;
        serde_json::from_str::<Value>(&result).map_err(|e| e.to_string())?;
        return Ok(result);
    }

    let parsed: Value = serde_json::from_str(before).map_err(|e| e.to_string())?;
    if !parsed.is_object() {
        return Err("Not a JSON object".into());
    }

    let root_open = first_non_ws(before).ok_or("Empty JSON")?;
    if before.as_bytes().get(root_open) != Some(&b'{') {
        return Err("Not a JSON object".into());
    }

    if let Some(mcp_field) = find_json_object_field(before, root_open, mcp_key)? {
        let servers_open = skip_json_ws(before, mcp_field.value_start);
        if before.as_bytes().get(servers_open) != Some(&b'{') {
            return Err(format!("'{}' is not an object", mcp_key));
        }

        let mcp_indent = line_indent_before(before, mcp_field.key_start);
        let server_indent = infer_json_child_indent(before, servers_open, &mcp_indent);
        let closing_indent = line_indent_before(before, servers_open);
        let result = if let Some(server_field) = find_json_object_field(before, servers_open, name)?
        {
            let property = format_json_property(name, config, &server_indent, false)?;
            let mut result = String::with_capacity(before.len() + property.len());
            result.push_str(&before[..server_field.key_start]);
            result.push_str(&property);
            result.push_str(&before[server_field.value_end..]);
            result
        } else {
            let property = format_json_property(name, config, &server_indent, true)?;
            insert_json_property(before, servers_open, &property, &closing_indent)?
        };

        serde_json::from_str::<Value>(&result).map_err(|e| e.to_string())?;
        Ok(result)
    } else {
        let root_indent = line_indent_before(before, root_open);
        let field_indent = infer_json_child_indent(before, root_open, &root_indent);
        let server_indent = format!("{}  ", field_indent);
        let property =
            format_json_mcp_property(mcp_key, name, config, &field_indent, &server_indent, true)?;
        let result = insert_json_property(before, root_open, &property, &root_indent)?;
        serde_json::from_str::<Value>(&result).map_err(|e| e.to_string())?;
        Ok(result)
    }
}

/// Surgically remove a single server from a JSON config document by editing
/// the raw text, preserving the original formatting and key order of every
/// other field (both inside and outside `mcp_key`).
///
/// Returns an error if the document isn't valid JSON, or if `mcp_key`/`name`
/// isn't found (so callers can decide how to surface "nothing to delete").
pub(crate) fn remove_json_server(
    before: &str,
    mcp_key: &str,
    name: &str,
) -> Result<String, String> {
    // Empty or `{}` — nothing to remove.
    if before.trim().is_empty() || before.trim() == "{}" {
        return Err(format!("Server '{}' not found", name));
    }
    let parsed: Value = serde_json::from_str(before).map_err(|e| e.to_string())?;
    if !parsed.is_object() {
        return Err("Not a JSON object".into());
    }

    let root_open = first_non_ws(before).ok_or("Empty JSON")?;
    if before.as_bytes().get(root_open) != Some(&b'{') {
        return Err("Not a JSON object".into());
    }

    let mcp_field =
        find_json_object_field(before, root_open, mcp_key)?.ok_or_else(|| {
            format!("'{}' section not found, cannot delete '{}'", mcp_key, name)
        })?;

    let servers_open = skip_json_ws(before, mcp_field.value_start);
    if before.as_bytes().get(servers_open) != Some(&b'{') {
        return Err(format!("'{}' is not an object", mcp_key));
    }

    let server_field = find_json_object_field(before, servers_open, name)?
        .ok_or_else(|| format!("Server '{}' not found", name))?;

    let servers_close = find_matching_delim(before, servers_open, b'{', b'}')?;

    let result = remove_json_member(before, server_field.key_start, server_field.value_end, servers_open, servers_close);

    // Validate the result is still well-formed JSON before returning.
    serde_json::from_str::<Value>(&result).map_err(|e| e.to_string())?;
    Ok(result)
}

/// Remove the bytes in `[key_start, value_end)` (a full `"key": value` member)
/// from a JSON object, cleaning up the surrounding comma/whitespace so the
/// result stays valid JSON with no dangling commas or stray blank lines.
///
/// `obj_open` / `obj_close` bound the enclosing object; the member is assumed
/// to live strictly inside it.
fn remove_json_member(
    text: &str,
    key_start: usize,
    value_end: usize,
    obj_open: usize,
    obj_close: usize,
) -> String {
    let bytes = text.as_bytes();

    // 1) Extend `value_end` forward to swallow a trailing comma (and the
    //    newline/whitespace that follows it). If a trailing comma exists, the
    //    next non-ws char is either `}` (empty trailing) or another member.
    let mut after = value_end;
    while after < obj_close && matches!(bytes[after], b' ' | b'\t') {
        after += 1;
    }
    if bytes.get(after) == Some(&b',') {
        // Swallow the comma...
        let mut cut_end = after + 1;
        // ...and the line break that follows it (so no blank line is left).
        if bytes.get(cut_end) == Some(&b'\n') {
            cut_end += 1;
            if bytes.get(cut_end) == Some(&b'\r') {
                cut_end += 1;
            }
        }
        // Splice [..key_start) + [cut_end..), dropping the member + its comma.
        let mut result = String::with_capacity(text.len());
        result.push_str(&text[..key_start]);
        result.push_str(&text[cut_end..]);
        return result;
    }

    // 2) No trailing comma → this is the last member. Its preceding comma
    //    belongs to the previous member; remove it along with the line that
    //    hosted this member (from the newline before its indentation back to
    //    `value_end`). This avoids leaving a trailing comma before `}`.
    let mut cut_start = key_start;
    // Walk back over this member's leading indentation + the newline that
    // starts its line.
    while cut_start > obj_open && matches!(bytes[cut_start - 1], b' ' | b'\t') {
        cut_start -= 1;
    }
    if cut_start > obj_open && bytes[cut_start - 1] == b'\n' {
        cut_start -= 1;
        if cut_start > obj_open && bytes[cut_start - 1] == b'\r' {
            cut_start -= 1;
        }
    }
    // Now remove the preceding comma if present (the previous member's
    // trailing comma).
    let mut comma_start = cut_start;
    while comma_start > obj_open && matches!(bytes[comma_start - 1], b' ' | b'\t') {
        comma_start -= 1;
    }
    if comma_start > obj_open && bytes[comma_start - 1] == b',' {
        cut_start = comma_start - 1;
    }

    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..cut_start]);
    result.push_str(&text[value_end..]);
    result
}

fn format_new_json_doc(mcp_key: &str, name: &str, config: &Value) -> Result<String, String> {
    let property = format_json_mcp_property(mcp_key, name, config, "  ", "    ", true)?;
    Ok(format!("{{\n{}\n}}", property))
}

fn format_json_mcp_property(
    mcp_key: &str,
    name: &str,
    config: &Value,
    field_indent: &str,
    server_indent: &str,
    include_first_indent: bool,
) -> Result<String, String> {
    let key = serde_json::to_string(mcp_key).map_err(|e| e.to_string())?;
    let server = format_json_property(name, config, server_indent, true)?;
    let mut out = String::new();
    if include_first_indent {
        out.push_str(field_indent);
    }
    out.push_str(&key);
    out.push_str(": {\n");
    out.push_str(&server);
    out.push('\n');
    out.push_str(field_indent);
    out.push('}');
    Ok(out)
}

fn format_json_property(
    name: &str,
    config: &Value,
    indent: &str,
    include_first_indent: bool,
) -> Result<String, String> {
    let key = serde_json::to_string(name).map_err(|e| e.to_string())?;
    let value_text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let mut lines = value_text.lines();
    let first = lines.next().unwrap_or("null");
    let mut out = String::new();
    if include_first_indent {
        out.push_str(indent);
    }
    out.push_str(&key);
    out.push_str(": ");
    out.push_str(first);
    for line in lines {
        out.push('\n');
        out.push_str(indent);
        out.push_str(line);
    }
    Ok(out)
}

#[derive(Debug)]
struct JsonFieldSpan {
    key_start: usize,
    value_start: usize,
    value_end: usize,
}

fn first_non_ws(text: &str) -> Option<usize> {
    text.char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, _)| idx)
}

fn skip_json_ws(text: &str, mut idx: usize) -> usize {
    let bytes = text.as_bytes();
    while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\n' | b'\r' | b'\t') {
        idx += 1;
    }
    idx
}

fn parse_json_string_end(text: &str, start: usize) -> Result<usize, String> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return Err("Expected JSON string".into());
    }
    let mut idx = start + 1;
    while idx < bytes.len() {
        match bytes[idx] {
            b'\\' => idx += 2,
            b'"' => return Ok(idx + 1),
            _ => idx += 1,
        }
    }
    Err("Unterminated JSON string".into())
}

fn find_matching_delim(text: &str, open_pos: usize, open: u8, close: u8) -> Result<usize, String> {
    let bytes = text.as_bytes();
    if bytes.get(open_pos) != Some(&open) {
        return Err("Opening delimiter not found".into());
    }
    let mut depth = 0usize;
    let mut idx = open_pos;
    let mut in_string = false;
    let mut escaped = false;
    while idx < bytes.len() {
        let ch = bytes[idx];
        if escaped {
            escaped = false;
            idx += 1;
            continue;
        }
        if in_string {
            match ch {
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            idx += 1;
            continue;
        }
        if ch == b'"' {
            in_string = true;
        } else if ch == open {
            depth += 1;
        } else if ch == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Ok(idx);
            }
        }
        idx += 1;
    }
    Err("Unmatched delimiter".into())
}

fn find_json_value_end(text: &str, start: usize) -> Result<usize, String> {
    let start = skip_json_ws(text, start);
    match text.as_bytes().get(start) {
        Some(b'{') => find_matching_delim(text, start, b'{', b'}').map(|idx| idx + 1),
        Some(b'[') => find_matching_delim(text, start, b'[', b']').map(|idx| idx + 1),
        Some(b'"') => parse_json_string_end(text, start),
        Some(_) => {
            let bytes = text.as_bytes();
            let mut idx = start;
            while idx < bytes.len() && !matches!(bytes[idx], b',' | b'}' | b']') {
                idx += 1;
            }
            Ok(idx)
        }
        None => Err("Expected JSON value".into()),
    }
}

fn find_json_object_field(
    text: &str,
    object_open: usize,
    key: &str,
) -> Result<Option<JsonFieldSpan>, String> {
    if text.as_bytes().get(object_open) != Some(&b'{') {
        return Err("Expected JSON object".into());
    }
    let object_close = find_matching_delim(text, object_open, b'{', b'}')?;
    let mut idx = object_open + 1;
    while idx < object_close {
        idx = skip_json_ws(text, idx);
        if idx >= object_close {
            break;
        }
        if text.as_bytes().get(idx) == Some(&b',') {
            idx += 1;
            continue;
        }
        if text.as_bytes().get(idx) != Some(&b'"') {
            return Err("Expected JSON object key".into());
        }

        let key_start = idx;
        let key_end = parse_json_string_end(text, key_start)?;
        let decoded_key: String =
            serde_json::from_str(&text[key_start..key_end]).map_err(|e| e.to_string())?;
        idx = skip_json_ws(text, key_end);
        if text.as_bytes().get(idx) != Some(&b':') {
            return Err("Expected ':' after JSON object key".into());
        }
        let value_start = skip_json_ws(text, idx + 1);
        let value_end = find_json_value_end(text, value_start)?;
        if decoded_key == key {
            return Ok(Some(JsonFieldSpan {
                key_start,
                value_start,
                value_end,
            }));
        }
        idx = value_end;
    }
    Ok(None)
}

fn first_json_object_field_start(text: &str, object_open: usize) -> Result<Option<usize>, String> {
    let object_close = find_matching_delim(text, object_open, b'{', b'}')?;
    let mut idx = object_open + 1;
    while idx < object_close {
        idx = skip_json_ws(text, idx);
        if idx >= object_close {
            break;
        }
        if text.as_bytes().get(idx) == Some(&b',') {
            idx += 1;
            continue;
        }
        if text.as_bytes().get(idx) == Some(&b'"') {
            return Ok(Some(idx));
        }
        return Err("Expected JSON object key".into());
    }
    Ok(None)
}

fn line_indent_before(text: &str, pos: usize) -> String {
    let line_start = text[..pos].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    text[line_start..pos]
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

fn infer_json_child_indent(text: &str, object_open: usize, parent_indent: &str) -> String {
    match first_json_object_field_start(text, object_open) {
        Ok(Some(pos)) => line_indent_before(text, pos),
        _ => format!("{}  ", parent_indent),
    }
}

fn insert_json_property(
    before: &str,
    object_open: usize,
    property: &str,
    closing_indent: &str,
) -> Result<String, String> {
    let object_close = find_matching_delim(before, object_open, b'{', b'}')?;
    let inner = &before[object_open + 1..object_close];
    if inner.trim().is_empty() {
        let mut result = String::with_capacity(before.len() + property.len() + 2);
        result.push_str(&before[..object_open + 1]);
        result.push('\n');
        result.push_str(property);
        result.push('\n');
        result.push_str(closing_indent);
        result.push_str(&before[object_close..]);
        return Ok(result);
    }

    let last_non_ws = before[..object_close]
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, ch)| idx + ch.len_utf8())
        .ok_or("Invalid JSON object")?;
    let previous_char = before[..last_non_ws].chars().last();
    let mut result = String::with_capacity(before.len() + property.len() + 3);
    result.push_str(&before[..last_non_ws]);
    if previous_char != Some(',') {
        result.push(',');
    }
    result.push('\n');
    result.push_str(property);
    result.push('\n');
    result.push_str(closing_indent);
    result.push_str(&before[object_close..]);
    Ok(result)
}

/// Find the matching `}` for a `{` at `open_pos` in `text`
#[allow(dead_code)]
pub(crate) fn find_matching_brace(text: &str, open_pos: usize) -> Result<usize, String> {
    let actual_open = if text.as_bytes().get(open_pos) == Some(&b'{') {
        open_pos
    } else {
        text[open_pos..]
            .find('{')
            .map(|idx| open_pos + idx)
            .ok_or("Opening delimiter not found")?
    };
    find_matching_delim(text, actual_open, b'{', b'}')
}

pub(crate) fn apply_toml_server(
    before: &str,
    mcp_key: &str,
    name: &str,
    config: &Value,
) -> Result<String, String> {
    let new_section = render_toml_server_section(mcp_key, name, config)?;

    if before.trim().is_empty() {
        return Ok(new_section);
    }

    let _: toml::Value = toml::from_str(before).map_err(|e| format!("Invalid TOML: {}", e))?;
    let ranges = find_toml_server_section_ranges(before, mcp_key, name);
    if ranges.is_empty() {
        let result = append_toml_section(before, &new_section);
        let _: toml::Value = toml::from_str(&result).map_err(|e| e.to_string())?;
        return Ok(result);
    }

    let mut result = before.to_string();
    for idx in (0..ranges.len()).rev() {
        let range = ranges[idx].clone();
        if idx == 0 {
            let suffix = trailing_ws(&before[range.clone()]);
            let mut replacement = new_section.clone();
            if suffix.is_empty() {
                replacement.push('\n');
            } else {
                replacement.push_str(&suffix);
            }
            result.replace_range(range, &replacement);
        } else {
            result.replace_range(range, "");
        }
    }

    let _: toml::Value = toml::from_str(&result).map_err(|e| e.to_string())?;
    Ok(result)
}

fn render_toml_server_section(mcp_key: &str, name: &str, config: &Value) -> Result<String, String> {
    let mut root = toml::map::Map::new();
    let mut servers = toml::map::Map::new();
    servers.insert(name.to_string(), json_to_toml(config));
    root.insert(mcp_key.to_string(), toml::Value::Table(servers));
    Ok(toml::to_string_pretty(&toml::Value::Table(root))
        .map_err(|e| e.to_string())?
        .trim_end()
        .to_string())
}

fn append_toml_section(before: &str, new_section: &str) -> String {
    let mut result = before.to_string();
    if !result.ends_with('\n') {
        result.push('\n');
    }
    if !result.ends_with("\n\n") {
        result.push('\n');
    }
    result.push_str(new_section);
    result.push('\n');
    result
}

fn trailing_ws(text: &str) -> String {
    let trimmed_len = text.trim_end_matches(char::is_whitespace).len();
    text[trimmed_len..].to_string()
}

pub fn find_toml_server_section_ranges(text: &str, mcp_key: &str, name: &str) -> Vec<Range<usize>> {
    let headers = toml_headers(text);
    let mut ranges = Vec::new();
    for (idx, (start, path)) in headers.iter().enumerate() {
        if path.len() >= 2 && path[0] == mcp_key && path[1] == name {
            let end = headers
                .get(idx + 1)
                .map(|(next_start, _)| *next_start)
                .unwrap_or(text.len());
            ranges.push(*start..end);
        }
    }
    ranges
}

fn toml_headers(text: &str) -> Vec<(usize, Vec<String>)> {
    let mut headers = Vec::new();
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if let Some(path) = parse_toml_header_path(line) {
            headers.push((offset, path));
        }
        offset += line.len();
    }
    headers
}

fn parse_toml_header_path(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let is_array = trimmed.starts_with("[[");
    let inner_start = if is_array { 2 } else { 1 };

    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut close_idx = None;
    for (idx, ch) in trimmed.char_indices().skip(inner_start) {
        if escaped {
            escaped = false;
            continue;
        }
        if in_double {
            match ch {
                '\\' => escaped = true,
                '"' => in_double = false,
                _ => {}
            }
            continue;
        }
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        match ch {
            '"' => in_double = true,
            '\'' => in_single = true,
            ']' => {
                if is_array && trimmed.as_bytes().get(idx + 1) != Some(&b']') {
                    return None;
                }
                close_idx = Some(idx);
                break;
            }
            _ => {}
        }
    }
    let close_idx = close_idx?;
    let close_len = if is_array { 2 } else { 1 };
    let after = trimmed[close_idx + close_len..].trim();
    if !after.is_empty() && !after.starts_with('#') {
        return None;
    }
    parse_toml_key_path(&trimmed[inner_start..close_idx])
}

fn parse_toml_key_path(path: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    for (idx, ch) in path.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_double {
            match ch {
                '\\' => escaped = true,
                '"' => in_double = false,
                _ => {}
            }
            continue;
        }
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        match ch {
            '"' => in_double = true,
            '\'' => in_single = true,
            '.' => {
                parts.push(parse_toml_key_part(&path[start..idx])?);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(parse_toml_key_part(&path[start..])?);
    Some(parts)
}

fn parse_toml_key_part(part: &str) -> Option<String> {
    let part = part.trim();
    if part.is_empty() {
        return None;
    }
    if part.starts_with('"') || part.starts_with('\'') {
        let doc: toml::Value = toml::from_str(&format!("key = {}", part)).ok()?;
        doc.get("key")?.as_str().map(ToString::to_string)
    } else {
        Some(part.to_string())
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
        let parsed: Value = serde_json::from_str(result)
            .unwrap_or_else(|e| panic!("Invalid JSON: {}\nGot:\n{}", e, result));
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
        let config_new =
            serde_json::json!({"command": "npx", "args": ["-y", "new-pkg"], "env": {"KEY": "val"}});

        // 1. Empty file → first server
        let r = apply_json_server("{}", "mcpServers", "srv-a", &config_a).unwrap();
        let p = assert_valid_with_servers(&r, &["srv-a"]);
        assert_eq!(p["mcpServers"]["srv-a"]["command"], "npx");

        // 2. Existing file with one server → append second
        let one_server =
            "{\n  \"mcpServers\": {\n    \"srv-a\": {\n      \"command\": \"npx\"\n    }\n  }\n}";
        let r = apply_json_server(one_server, "mcpServers", "srv-b", &config_b).unwrap();
        let _p = assert_valid_with_servers(&r, &["srv-a", "srv-b"]);

        // 3. Two servers → append third
        let two_servers = "{\n  \"mcpServers\": {\n    \"srv-a\": { \"command\": \"a\" },\n    \"srv-b\": { \"command\": \"b\" }\n  }\n}";
        let r = apply_json_server(two_servers, "mcpServers", "srv-c", &config_c).unwrap();
        let _p = assert_valid_with_servers(&r, &["srv-a", "srv-b", "srv-c"]);

        // 4. Update only server (no comma after)
        let one_srv =
            "{\n  \"mcpServers\": {\n    \"srv-a\": {\n      \"command\": \"old\"\n    }\n  }\n}";
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
                    i,
                    j,
                    result.unwrap_err()
                );
                let text = result.unwrap();
                let _: Value = serde_json::from_str(&text).unwrap_or_else(|e| {
                    panic!(
                        "Invalid JSON for target[{}] config[{}]: {}\nGot:\n{}",
                        i, j, e, text
                    )
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
        let target_before =
            "{\n  \"mcpServers\": {\n    \"existing\": { \"command\": \"echo\" }\n  }\n}";

        let after =
            apply_json_server(target_before, "mcpServers", "context7", &source_config).unwrap();
        // Verify the written content can be re-parsed and yields the same config
        let servers = parse_json_servers(&after, "mcpServers").unwrap();
        let ctx = servers
            .iter()
            .find(|s| s.name == "context7")
            .expect("context7 should exist");
        assert_eq!(ctx.config, source_config);
        let ex = servers
            .iter()
            .find(|s| s.name == "existing")
            .expect("existing should be preserved");
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
        let after =
            apply_json_server(target_before, "mcpServers", "context7", &new_config).unwrap();
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

    #[test]
    fn test_parse_input_config_accepts_toml_table_header() {
        let text = r#"[mcp_servers.context7]
command = "npx"
args = ["-y", "context7"]
"#;
        let parsed = parse_input_config(text).unwrap();
        assert_eq!(
            parsed["mcp_servers"]["context7"]["command"],
            Value::String("npx".into())
        );
    }

    #[test]
    fn test_parse_server_config_input_extracts_wrapped_server() {
        let text = r#"[mcp_servers.context7]
command = "npx"

[mcp_servers.context7.env]
API_KEY = "secret"
"#;
        let parsed = parse_server_config_input(text, "mcp_servers", "context7").unwrap();
        assert_eq!(parsed["command"], Value::String("npx".into()));
        assert_eq!(parsed["env"]["API_KEY"], Value::String("secret".into()));
    }

    #[test]
    fn test_apply_json_server_preserves_unrelated_text() {
        let before = r#"{
  "theme": {"keep": true},
  "mcpServers": {
    "context7": {"command": "old"},
    "other": { "command": "keep" }
  },
  "tail": 1
}
"#;
        let config = serde_json::json!({"command": "new", "args": ["--flag"]});
        let result = apply_json_server(before, "mcpServers", "context7", &config).unwrap();
        assert!(result.contains(r#"  "theme": {"keep": true},"#));
        assert!(result.contains(r#"  "tail": 1"#));
        assert!(result.contains(r#""other": { "command": "keep" }"#));
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["mcpServers"]["context7"]["command"], "new");
    }

    #[test]
    fn test_apply_toml_server_uses_nested_env_section_and_preserves_other_sections() {
        let before = r#"[model]
provider = "openai"

[mcp_servers.context7]
command = "old"

[mcp_servers.context7.env]
OLD_KEY = "old"

[profiles.default]
model = "gpt-5"
"#;
        let config = serde_json::json!({
            "command": "npx",
            "args": ["-y", "context7"],
            "env": {"API_KEY": "secret"}
        });
        let result = apply_toml_server(before, "mcp_servers", "context7", &config).unwrap();
        assert!(result.contains("[model]\nprovider = \"openai\""));
        assert!(result.contains("[profiles.default]\nmodel = \"gpt-5\""));
        assert!(result.contains("[mcp_servers.context7.env]"));
        assert!(!result.contains("\n[env]\n"));
        assert!(!result.contains("OLD_KEY"));

        let parsed: toml::Value = toml::from_str(&result).unwrap();
        assert_eq!(
            parsed["mcp_servers"]["context7"]["env"]["API_KEY"].as_str(),
            Some("secret")
        );
    }

    #[test]
    fn test_apply_toml_server_quotes_dotted_server_names() {
        let config = serde_json::json!({"command": "npx"});
        let result = apply_toml_server("", "mcp_servers", "foo.bar", &config).unwrap();
        assert!(result.contains("[mcp_servers.\"foo.bar\"]"));
        let parsed: toml::Value = toml::from_str(&result).unwrap();
        assert_eq!(
            parsed["mcp_servers"]["foo.bar"]["command"].as_str(),
            Some("npx")
        );
    }

    #[test]
    fn test_apply_toml_server_stops_at_array_table_boundary() {
        let before = r#"[mcp_servers.context7]
command = "old"

[[tools]]
name = "keep"
"#;
        let config = serde_json::json!({"command": "new"});
        let result = apply_toml_server(before, "mcp_servers", "context7", &config).unwrap();
        assert!(result.contains("[[tools]]\nname = \"keep\""));
        let parsed: toml::Value = toml::from_str(&result).unwrap();
        assert_eq!(
            parsed["mcp_servers"]["context7"]["command"].as_str(),
            Some("new")
        );
    }

    // --- extract_sync_core tests ---

    #[test]
    fn test_extract_sync_core_strips_type_and_empty_env() {
        let claude_code = serde_json::json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "@upstash/context7-mcp", "--api-key", "ctx7sk-abc"],
            "env": {}
        });
        let core = extract_sync_core(&claude_code);
        let obj = core.as_object().unwrap();
        assert!(!obj.contains_key("type"), "type should be stripped");
        assert!(!obj.contains_key("env"), "empty env should be stripped");
        assert_eq!(core["command"], "npx");
        assert_eq!(core["args"][2], "--api-key");
        assert_eq!(core["args"][3], "ctx7sk-abc", "api-key in args must be preserved");
    }

    #[test]
    fn test_extract_sync_core_strips_codex_specific_fields() {
        let codex = serde_json::json!({
            "command": "npx",
            "args": ["-y", "@upstash/context7-mcp", "--api-key", "ctx7sk-abc"],
            "startup_timeout_sec": 20,
            "enabled": true,
            "required": false
        });
        let core = extract_sync_core(&codex);
        let obj = core.as_object().unwrap();
        assert!(!obj.contains_key("startup_timeout_sec"));
        assert!(!obj.contains_key("enabled"));
        assert!(!obj.contains_key("required"));
        assert_eq!(core["command"], "npx");
    }

    #[test]
    fn test_extract_sync_core_strips_gemini_specific_fields() {
        let gemini = serde_json::json!({
            "command": "npx",
            "args": ["-y", "my-mcp"],
            "timeout": 30000,
            "trust": true,
            "description": "My server",
            "includeTools": ["tool_a"]
        });
        let core = extract_sync_core(&gemini);
        let obj = core.as_object().unwrap();
        assert!(!obj.contains_key("timeout"));
        assert!(!obj.contains_key("trust"));
        assert!(!obj.contains_key("description"));
        assert!(!obj.contains_key("includeTools"));
        assert_eq!(core["command"], "npx");
    }

    #[test]
    fn test_extract_sync_core_preserves_non_empty_env() {
        let config = serde_json::json!({
            "command": "npx",
            "args": ["-y", "my-mcp"],
            "env": {"API_KEY": "secret123"},
            "type": "stdio"
        });
        let core = extract_sync_core(&config);
        assert_eq!(core["env"]["API_KEY"], "secret123", "non-empty env must be preserved");
        assert!(!core.as_object().unwrap().contains_key("type"));
    }

    #[test]
    fn test_extract_sync_core_only_three_keys() {
        let config = serde_json::json!({
            "command": "node",
            "args": ["server.js"],
            "env": {"KEY": "val"},
            "type": "stdio",
            "cwd": "/home/user",
            "timeout": 5000,
            "startup_timeout_sec": 30
        });
        let core = extract_sync_core(&config);
        let obj = core.as_object().unwrap();
        assert_eq!(obj.len(), 3, "core must have exactly command+args+env");
    }

    #[test]
    fn test_find_toml_section_ranges_includes_nested_subtables() {
        // Mirrors the Codex `node_repl` case: a server with a nested [..env]
        // subtable, followed by an unrelated section that must be preserved.
        let content = "\
model = \"gpt-5.5\"

[mcp_servers.node_repl]
args = []
command = \"/usr/bin/node_repl\"

[mcp_servers.node_repl.env]
API_KEY = \"secret\"
NODE_PATH = \"/usr/bin/node\"

[plugins.foo]
enabled = true
";
        let ranges =
            find_toml_server_section_ranges(content, "mcp_servers", "node_repl");
        // Should match both [mcp_servers.node_repl] and [mcp_servers.node_repl.env].
        assert_eq!(ranges.len(), 2, "expected 2 ranges (server + env subtable)");

        // Build the "after" text by removing the ranges.
        let mut after = String::new();
        let mut cursor = 0usize;
        for r in &ranges {
            if r.start > cursor {
                after.push_str(&content[cursor..r.start]);
            }
            cursor = r.end;
        }
        if cursor < content.len() {
            after.push_str(&content[cursor..]);
        }
        while after.contains("\n\n\n") {
            after = after.replace("\n\n\n", "\n\n");
        }
        let after = after.trim_end().to_string() + "\n";

        // The unrelated [plugins.foo] section must survive.
        assert!(after.contains("[plugins.foo]"), "plugins section must survive");
        assert!(
            !after.contains("node_repl"),
            "all node_repl traces must be removed"
        );
        assert!(after.contains("model ="), "model key must survive");
        assert!(
            !after.contains("API_KEY"),
            "env vars from deleted subtable must be removed"
        );
        // Result must still be valid TOML.
        let _: toml::Value = toml::from_str(&after).expect("result must be valid TOML");
    }

    #[test]
    fn test_find_toml_section_ranges_does_not_match_prefix_sibling() {
        // [mcp_servers.node] must NOT match when deleting "node" if only
        // [mcp_servers.node_repl] exists, and vice versa.
        let content = "\
[mcp_servers.node]
command = \"a\"

[mcp_servers.node_repl]
command = \"b\"
";
        // Deleting "node" should only touch [mcp_servers.node], leaving node_repl.
        let ranges = find_toml_server_section_ranges(content, "mcp_servers", "node");
        assert_eq!(ranges.len(), 1, "only the exact-match section should be hit");
        let mut after = String::new();
        let mut cursor = 0usize;
        for r in &ranges {
            after.push_str(&content[cursor..r.start]);
            cursor = r.end;
        }
        after.push_str(&content[cursor..]);
        assert!(after.contains("node_repl"), "node_repl must survive");
        assert!(
            !after.contains("[mcp_servers.node]\ncommand = \"a\""),
            "node section must be gone"
        );
    }

    #[test]
    fn test_config_to_display_toml_renders_nested_env_under_server() {
        // A server with a nested `env` object must render the env subtable
        // with the full dotted path `[mcp_servers.node_repl.env]`, NOT a
        // context-less `[env]` header.
        let config = serde_json::json!({
            "args": [],
            "command": "/usr/bin/node_repl",
            "startup_timeout_sec": 120,
            "env": {
                "NODE_PATH": "/usr/bin/node",
                "API_KEY": "secret",
            }
        });
        let text = config_to_display(&config, McpFormat::Toml, "mcp_servers", "node_repl");
        assert!(
            text.contains("[mcp_servers.node_repl]"),
            "server header must be present, got:\n{}",
            text
        );
        assert!(
            text.contains("[mcp_servers.node_repl.env]"),
            "env subtable must use the full dotted path, got:\n{}",
            text
        );
        assert!(
            !text.contains("\n[env]"),
            "bare [env] header must NOT appear, got:\n{}",
            text
        );
        // Round-trip: the produced text must be valid TOML.
        let _: toml::Value = toml::from_str(&text).expect("display text must be valid TOML");
    }

    // --- remove_json_server (surgical JSON delete) tests ---

    fn root_key_order(json: &str) -> Vec<String> {
        // Without preserve_order, serde_json itself sorts keys, so we can't
        // rely on it for order. Instead scan raw text for root-depth keys.
        let bytes = json.as_bytes();
        let mut keys = Vec::new();
        // depth counts nesting. The root object's `{` brings it to 1, so root
        // keys are captured exactly when depth == 1.
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escaped = false;
        let mut i = 0;
        while i < bytes.len() {
            let ch = bytes[i];
            if escaped {
                escaped = false;
                i += 1;
                continue;
            }
            if in_string {
                match ch {
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
                i += 1;
                continue;
            }
            match ch {
                b'"' => {
                    in_string = true;
                    if depth == 1 {
                        // root-level key: capture until closing quote
                        let start = i + 1;
                        let mut j = start;
                        while j < bytes.len() {
                            match bytes[j] {
                                b'\\' => j += 2,
                                b'"' => break,
                                _ => j += 1,
                            }
                        }
                        if let Ok(raw) = std::str::from_utf8(&bytes[start..j]) {
                            let decoded: String =
                                serde_json::from_str(&format!("\"{}\"", raw)).unwrap_or_default();
                            keys.push(decoded);
                        }
                    }
                }
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        keys
    }

    #[test]
    fn test_remove_json_server_preserves_unrelated_top_level_keys_order() {
        // The core regression: deleting a server must NOT re-sort every other
        // field in the document (the old BTreeMap re-serialization did this).
        let before = r#"{
  "numStartups": 42,
  "mcpServers": {
    "context7": { "command": "npx" },
    "filesystem": { "command": "node" }
  },
  "projects": {}
}"#;
        let after = remove_json_server(before, "mcpServers", "context7").unwrap();

        // Top-level key order must be unchanged.
        let keys = root_key_order(&after);
        assert_eq!(
            keys,
            vec!["numStartups", "mcpServers", "projects"],
            "top-level key order must be preserved, got:\n{}",
            after
        );

        // The targeted server is gone, the sibling survives.
        let parsed: Value = serde_json::from_str(&after).unwrap();
        assert!(
            !parsed["mcpServers"].as_object().unwrap().contains_key("context7"),
            "context7 must be removed"
        );
        assert_eq!(parsed["mcpServers"]["filesystem"]["command"], "node");
        assert_eq!(parsed["numStartups"], 42);
    }

    #[test]
    fn test_remove_json_server_first_of_multiple() {
        let before = r#"{
  "mcpServers": {
    "alpha": { "command": "a" },
    "beta": { "command": "b" }
  }
}"#;
        let after = remove_json_server(before, "mcpServers", "alpha").unwrap();
        let parsed: Value = serde_json::from_str(&after).unwrap();
        assert!(!parsed["mcpServers"].as_object().unwrap().contains_key("alpha"));
        assert_eq!(parsed["mcpServers"]["beta"]["command"], "b");
        // No leading comma left behind.
        assert!(
            !after.contains("\"mcpServers\": {\n,"),
            "no dangling comma before first entry, got:\n{}",
            after
        );
        assert!(after.contains("\"beta\""));
    }

    #[test]
    fn test_remove_json_server_last_of_multiple() {
        let before = r#"{
  "mcpServers": {
    "alpha": { "command": "a" },
    "beta": { "command": "b" }
  }
}"#;
        let after = remove_json_server(before, "mcpServers", "beta").unwrap();
        let parsed: Value = serde_json::from_str(&after).unwrap();
        assert!(!parsed["mcpServers"].as_object().unwrap().contains_key("beta"));
        assert_eq!(parsed["mcpServers"]["alpha"]["command"], "a");
        // No trailing comma before the closing brace.
        assert!(
            !after.contains("\"a\"\n    },\n  }") && !after.contains(",\n  }"),
            "no dangling comma before close, got:\n{}",
            after
        );
    }

    #[test]
    fn test_remove_json_server_middle_of_three() {
        let before = r#"{
  "mcpServers": {
    "alpha": { "command": "a" },
    "beta": { "command": "b" },
    "gamma": { "command": "c" }
  }
}"#;
        let after = remove_json_server(before, "mcpServers", "beta").unwrap();
        let parsed: Value = serde_json::from_str(&after).unwrap();
        let servers = parsed["mcpServers"].as_object().unwrap();
        assert!(!servers.contains_key("beta"));
        assert_eq!(servers["alpha"]["command"], "a");
        assert_eq!(servers["gamma"]["command"], "c");
        // Valid JSON (already asserted by parse) and exactly two entries.
        assert_eq!(servers.len(), 2);
    }

    #[test]
    fn test_remove_json_server_only_server_leaves_empty() {
        let before = r#"{
  "mcpServers": {
    "only": { "command": "x" }
  }
}"#;
        let after = remove_json_server(before, "mcpServers", "only").unwrap();
        let parsed: Value = serde_json::from_str(&after).unwrap();
        assert!(parsed["mcpServers"].as_object().unwrap().is_empty());
        assert!(parsed["mcpServers"].is_object());
    }

    #[test]
    fn test_remove_json_server_not_found_errors() {
        let before = r#"{
  "mcpServers": {
    "alpha": { "command": "a" }
  }
}"#;
        let res = remove_json_server(before, "mcpServers", "ghost");
        assert!(res.is_err(), "deleting a non-existent server must error");
        assert!(
            res.unwrap_err().contains("not found"),
            "error should mention not found"
        );
    }

    #[test]
    fn test_remove_json_server_no_mcp_key_errors() {
        let before = r#"{ "other": 1 }"#;
        let res = remove_json_server(before, "mcpServers", "alpha");
        assert!(res.is_err());
    }

    #[test]
    fn test_remove_json_server_preserves_inner_formatting() {
        // Multi-line, nested objects, arrays, comments-ish spacing — everything
        // outside the deleted entry should be byte-identical to the input.
        let before = r#"{
  "mcpServers": {
    "context7": {
      "command": "npx",
      "args": ["-y", "@upstash/context7-mcp"],
      "env": {
        "API_KEY": "secret"
      }
    },
    "filesystem": {
      "command": "node",
      "args": ["server.js"]
    }
  }
}"#;
        let after = remove_json_server(before, "mcpServers", "context7").unwrap();
        let parsed: Value = serde_json::from_str(&after).unwrap();
        assert!(parsed["mcpServers"].as_object().unwrap().contains_key("filesystem"));
        assert!(!after.contains("context7"));
        assert!(after.contains("\"filesystem\""));
        assert!(after.contains("\"server.js\""));
    }

    #[test]
    fn test_remove_json_server_inline_spacing() {
        // Compact, single-line-ish formatting with inline objects.
        let before = "{\n  \"mcpServers\": {\n    \"a\": { \"command\": \"x\" },\n    \"b\": { \"command\": \"y\" }\n  }\n}";
        let after = remove_json_server(before, "mcpServers", "a").unwrap();
        let parsed: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(parsed["mcpServers"]["b"]["command"], "y");
        assert!(!parsed["mcpServers"].as_object().unwrap().contains_key("a"));
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

// --- MCP Change Preview (Add / Delete) ---

/// Generate empty file content for a new platform config.
fn default_new_file_content(format: McpFormat, mcp_key: &str) -> String {
    match format {
        McpFormat::Json => "{}".to_string(),
        McpFormat::Toml => format!("[{}]\n", mcp_key),
    }
}

/// Preview the effect of importing/adding a server (before actually writing).
pub fn preview_import_mcp_server(
    platform_id: &str,
    name: &str,
    config: &Value,
) -> Result<McpSyncPreview, String> {
    let def = find_mcp_platform(platform_id).ok_or("Platform not found")?;
    let existing = read_mcp_server(platform_id, name).ok();

    let before_text = if def.config_path.exists() {
        fs::read_to_string(&def.config_path).unwrap_or_default()
    } else {
        default_new_file_content(def.format, &def.mcp_key)
    };

    let after_text = match def.format {
        McpFormat::Json => apply_json_server(&before_text, &def.mcp_key, name, config)?,
        McpFormat::Toml => apply_toml_server(&before_text, &def.mcp_key, name, config)?,
    };

    let diff_lines = compute_text_diff(&before_text, &after_text);
    let added = diff_lines.iter().filter(|l| l.tag == "added").count();
    let removed = diff_lines.iter().filter(|l| l.tag == "removed").count();

    Ok(McpSyncPreview {
        server_name: name.to_string(),
        target_format: match def.format {
            McpFormat::Json => "json",
            McpFormat::Toml => "toml",
        }
        .to_string(),
        target_config_path: def.config_path.display().to_string(),
        has_conflict: existing.is_some(),
        diff_lines,
        added,
        removed,
    })
}

/// Preview the effect of deleting a server (before actually deleting).
pub fn preview_delete_mcp_server(
    platform_id: &str,
    name: &str,
) -> Result<McpSyncPreview, String> {
    let def = find_mcp_platform(platform_id).ok_or("Platform not found")?;
    if !def.config_path.exists() {
        return Err("Config file not found".into());
    }

    let before_text = fs::read_to_string(&def.config_path).map_err(|e| e.to_string())?;

    let after_text = match def.format {
        McpFormat::Json => {
            // Use the same surgical text edit as the real delete so the preview
            // matches what actually gets written byte-for-byte.
            remove_json_server(&before_text, &def.mcp_key, name)?
        }
        McpFormat::Toml => {
            let ranges = find_toml_server_section_ranges(&before_text, &def.mcp_key, name);
            if ranges.is_empty() {
                // Fallback: full re-serialization
                let mut doc: toml::Value = toml::from_str(&before_text)
                    .map_err(|e| format!("Invalid TOML: {}", e))?;
                if let Some(servers) = doc
                    .as_table_mut()
                    .and_then(|t| t.get_mut(&def.mcp_key))
                    .and_then(|v| v.as_table_mut())
                {
                    servers.remove(name);
                }
                toml::to_string_pretty(&doc).map_err(|e| e.to_string())?
            } else {
                // Remove all matched section ranges (server + nested subtables),
                // then tidy up stray blank lines.
                let mut result = String::with_capacity(before_text.len());
                let mut cursor = 0usize;
                for r in &ranges {
                    if r.start > cursor {
                        result.push_str(&before_text[cursor..r.start]);
                    }
                    cursor = r.end;
                }
                if cursor < before_text.len() {
                    result.push_str(&before_text[cursor..]);
                }
                while result.contains("\n\n\n") {
                    result = result.replace("\n\n\n", "\n\n");
                }
                let trimmed = result.trim_end();
                if trimmed.is_empty() {
                    String::new()
                } else {
                    format!("{}\n", trimmed)
                }
            }
        }
    };

    let diff_lines = compute_text_diff(&before_text, &after_text);
    let added = diff_lines.iter().filter(|l| l.tag == "added").count();
    let removed = diff_lines.iter().filter(|l| l.tag == "removed").count();

    Ok(McpSyncPreview {
        server_name: name.to_string(),
        target_format: match def.format {
            McpFormat::Json => "json",
            McpFormat::Toml => "toml",
        }
        .to_string(),
        target_config_path: def.config_path.display().to_string(),
        has_conflict: false,
        diff_lines,
        added,
        removed,
    })
}

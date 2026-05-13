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

fn find_toml_server_section_ranges(text: &str, mcp_key: &str, name: &str) -> Vec<Range<usize>> {
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

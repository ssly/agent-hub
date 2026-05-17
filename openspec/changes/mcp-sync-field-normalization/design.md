## Context

agent-hub syncs MCP servers between AI-tool config files. The sync flow today:

1. `read_mcp_server(source, name)` → returns `McpServer { name, config: Value }` where `config` is the raw object/table from the source file.
2. `apply_json_server` / `apply_toml_server` inject that exact `config` into the target file via a targeted text edit that preserves the rest of the file's formatting.
3. `preview_mcp_sync` runs the same apply against the current target text to compute a diff.

The file-level minimal-diff machinery (`apply_json_server` and friends in `parser.rs`) is already solid — it edits only the target server's property span and leaves surrounding text byte-identical.

The gap is the **payload**. `config` is copied verbatim, but each platform's official install emits a different schema for the same logical server. Verified with a real `context7` install across three platforms:

- Claude Code (`~/.claude.json`): `{ "type": "stdio", "command": "npx", "args": [..., "--api-key", "..."], "env": {} }`
- Gemini (`~/.gemini/settings.json`): `{ "command": "npx", "args": [..., "--api-key", "..."] }`
- Codex (`~/.codex/config.toml`): `command`, `args` (with --api-key), `startup_timeout_sec = 20`

So Claude Code → Gemini injects `type` and empty `env`; Codex → others carries `startup_timeout_sec`. Worse: **Codex uses `deny_unknown_fields`** — any unrecognized field in its TOML will crash config parsing entirely.

**Official doc field support (stdio transport):**

| Field | Claude Code | Gemini | Codex |
|---|:-:|:-:|:-:|
| `command` | ✅ required | ✅ required | ✅ required |
| `args` | ✅ | ✅ | ✅ |
| `env` (non-empty) | ✅ | ✅ | ✅ |
| `type: "stdio"` | ✅ optional | ❌ not a field | ❌ not a field |
| `env: {}` (empty) | ✅ habit | ❌ noise | ❌ noise |
| `cwd` | ❌ silently dropped | ✅ | ✅ |
| `timeout` | ❌ | ✅ | ❌ |
| `startup_timeout_sec` | ❌ | ❌ | ✅ |
| `trust` | ❌ | ✅ | ❌ |
| Codex-specific (enabled, required, tool_timeout_sec, ...) | ❌ | ❌ | ✅ |
| Gemini-specific (description, includeTools, excludeTools, ...) | ❌ | ✅ | ❌ |

Constraint: registry config-path resolution is correct and out of scope.

## Goals / Non-Goals

**Goals:**
- Synced entries contain only the universal core fields that make the server actually work on the target.
- api-key and all meaningful runtime config travels safely (it's in `args` or `env`).
- No foreign/platform-specific fields cross platform boundaries — especially critical for Codex's `deny_unknown_fields`.
- Minimize the file diff: at the object level (only core fields) and at the field level (only changed sub-properties rewritten when the server already exists).
- Preview diff is identical to the applied diff.

**Non-Goals:**
- Changing which config file each platform uses (registry paths stay as-is).
- Adding platform-specific defaults to the target (like `startup_timeout_sec` for Codex) — user can set those manually.
- Syncing non-stdio transports (HTTP/SSE) — different platforms use different URL field names (`url` vs `httpUrl`); out of scope.
- Normalizing manual single-server import/edit — only the cross-platform sync flow.

## Decisions

### Decision 1: Whitelist-based core extraction (not per-platform profiles)

Instead of per-platform schema profiles that add/remove/transform fields, use a simpler and safer approach: **extract only the universal core fields** that all platforms support.

Universal core for stdio transport:
```
CORE_FIELDS = ["command", "args", "env"]
```

The normalizer does:
1. Create a new object with ONLY these keys from the source (if present).
2. Drop `env` if it is an empty object `{}`.
3. Drop everything else — `type`, `cwd`, `timeout`, `startup_timeout_sec`, `trust`, platform-specific fields — all gone.

```rust
fn extract_sync_core(config: &Value) -> Value {
    let mut core = serde_json::Map::new();
    if let Some(obj) = config.as_object() {
        for key in ["command", "args", "env"] {
            if let Some(val) = obj.get(key) {
                // Skip empty env
                if key == "env" {
                    if let Some(map) = val.as_object() {
                        if map.is_empty() { continue; }
                    }
                }
                core.insert(key.to_string(), val.clone());
            }
        }
    }
    Value::Object(core)
}
```

**Why whitelist over per-platform profiles:**
- Simpler: no `McpSchemaProfile` struct, no per-platform config to maintain.
- Safer: Codex's `deny_unknown_fields` means a missed field = crash. Whitelist guarantees only known-good fields cross the boundary.
- Future-proof: when platforms add new fields, they don't accidentally leak. Only an explicit whitelist update opts them in.
- api-key travels in `args` or `env` — both are in the core.

*Alternative considered:* per-platform profile struct with emit/omit/default rules. Rejected — over-engineered for the actual requirement. All three platforms agree on command+args+env; the differences are all platform-specific extras that shouldn't sync anyway.

### Decision 2: Field-level minimal update for an existing target server

Today, when the target already has the server, `apply_json_server` replaces the whole `key: value` span via `format_json_property`. Even a one-field change rewrites every line of the server block.

Change: when the target server exists, compute the core config, then compare field-by-field against the existing target config:
- Sub-properties with equal values are left as-is (their original text/formatting untouched).
- Only sub-properties whose value differs are rewritten.
- If the core config equals the existing entry's core fields, write nothing (no-op).
- Platform-specific fields already present in the target (e.g. `startup_timeout_sec` in Codex) are **preserved** — never deleted. We only touch core fields.

For JSON: recurse into the server object's fields, use the existing `find_json_object_field` span-replacement at the sub-property level.
For TOML: update/add only the changed key-value pairs within the server's section, preserving other lines.

*Alternative considered:* always replace the whole server block. Rejected — would delete the target's platform-specific settings (like `startup_timeout_sec`) and produce larger diffs.

### Decision 3: Preserve target platform-specific fields

When the target already has the server with extra platform-specific fields, the sync MUST NOT delete them. Example: Codex has `startup_timeout_sec = 20` — after syncing updated `args` from Claude Code, the `startup_timeout_sec` must remain intact.

Implementation: the field-level merge from Decision 2 only writes/overwrites core fields; it never touches keys outside the core set.

### Decision 4: Preview consumes the same core extraction

`preview_mcp_sync` calls `extract_sync_core` before `apply_*`, identically to the writer, so preview and apply stay in lockstep. A test asserts preview "after" text == writer output.

## Risks / Trade-offs

- **A server that only works with `cwd` set** → will not work on Claude Code after sync regardless (Claude Code drops `cwd`). The sync correctly reflects reality: `cwd` is not universal. User must set it manually per-platform.
- **Codex `deny_unknown_fields`** → whitelist approach eliminates this risk entirely. Only `command`/`args`/`env` are written, all recognized.
- **Target-specific fields preserved** → a subsequent sync won't "clean up" `startup_timeout_sec` etc. This is by design: the sync owns core fields only, user owns platform-specific tuning.
- **Non-stdio servers** → HTTP/SSE transport sync is out of scope. The field names differ (`url` vs `httpUrl` vs `type: "http"`). A future change can extend the core set per transport type.

## Migration Plan

No data migration. The change only affects the next sync operation. Existing config files are untouched until the user runs a sync; when they do, the preview shows exactly what will change. Rollback is reverting the code change — no persisted state is altered.

## Why

When syncing an MCP server between platforms, agent-hub copies the source platform's config object verbatim into the target file. But each tool's official install emits a slightly different schema, and critically, **Codex uses `deny_unknown_fields`** — unrecognized fields crash its config parser entirely. Even for Gemini, foreign fields are noise that bloats the diff.

Observed on a real `context7` install across three platforms (all installed via each tool's official method):

| Platform | Format | Fields the official install emits |
|---|---|---|
| Claude Code (`~/.claude.json`) | JSON | `type: "stdio"`, `command`, `args` (with --api-key), `env: {}` |
| Gemini (`~/.gemini/settings.json`) | JSON | `command`, `args` (with --api-key) |
| Codex (`~/.codex/config.toml`) | TOML | `command`, `args` (with --api-key), `startup_timeout_sec` |

After checking official docs, the three platforms share exactly **three universal fields** for stdio transport: `command`, `args`, and `env` (when non-empty). The api-key travels in `args` or `env` — both universal. Everything else (`type`, empty `env`, `cwd`, `startup_timeout_sec`, `timeout`, `trust`, ...) is platform-specific.

## What Changes

- Add a **core field extraction** step: before writing to the target, extract only the universal core (`command`, `args`, non-empty `env`) from the source config. All platform-specific fields are stripped.
- When the target **already contains** the server, perform a **field-level minimal update**: only rewrite core sub-properties whose values differ, and **preserve** any platform-specific fields already in the target.
- The sync preview reflects the core-extracted result, so the previewed diff equals the diff actually written.
- No change to registry config-path resolution — the current per-platform file paths are correct.
- No `McpSchemaProfile` struct needed — a simple whitelist is safer and simpler.

## Capabilities

### New Capabilities
- `mcp-sync`: Cross-platform MCP server synchronization — reading a server from a source platform, extracting universal core fields (command, args, env), and writing to a target platform's config file with field-level minimal-diff editing that preserves the target's platform-specific settings.

### Modified Capabilities
<!-- None: no existing spec covers MCP sync behavior. -->

## Impact

- `src-tauri/src/mcp/parser.rs` — add `extract_sync_core` function + field-level merge logic; `preview_mcp_sync` consumes the core config.
- `src-tauri/src/mcp/writer.rs` — sync path calls `extract_sync_core` before `apply_*`.
- `src-tauri/src/mcp/registry.rs` — no changes needed (whitelist approach doesn't require per-platform profiles).
- Behavior change is limited to the cross-platform sync flow; manual import/edit of a single server is unaffected.
- No new dependencies. No breaking changes to stored config files.

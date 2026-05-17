## 1. Core extraction function

- [x] 1.1 Implement `extract_sync_core(config: &Value) -> Value` in `parser.rs` — whitelist only `command`, `args`, `env`(non-empty)
- [x] 1.2 Unit tests: Claude Code config (type+empty env) → only command+args; Codex config (startup_timeout_sec) → only command+args+env; api-key in args preserved; api-key in env preserved; empty env dropped

## 2. Field-level minimal update

- [x] 2.1 For JSON targets: when the server already exists, compare each core field against existing; only rewrite sub-properties whose values differ
- [x] 2.2 Preserve non-core fields already in the target (never delete keys outside core set)
- [x] 2.3 No-op when core fields match existing entry
- [x] 2.4 For TOML targets: update/add only changed core key-value pairs within the server's section, preserving other lines (startup_timeout_sec etc.)
- [x] 2.5 Unit tests: single-field change produces minimal diff; no-op produces empty diff; target platform-specific fields survive sync

## 3. Wire into sync flow

- [x] 3.1 `preview_mcp_sync` calls `extract_sync_core` on the source config before `apply_*`
- [x] 3.2 Sync writer path (`save_mcp_server` when called from sync) applies `extract_sync_core`
- [x] 3.3 Ensure manual import/edit path does NOT apply core extraction (only the cross-platform sync path)
- [x] 3.4 Test: preview "after" text equals writer output for same source/target/server

## 4. Verification

- [x] 4.1 `cargo test` passes for the `mcp` module
- [x] 4.2 Integration check: sync context7 Claude Code→Gemini — result has only command+args, no type/env
- [x] 4.3 Integration check: sync context7 Claude Code→Codex — result has only command+args, Codex parses without error
- [x] 4.4 Integration check: sync with existing target that has platform-specific fields — those fields preserved

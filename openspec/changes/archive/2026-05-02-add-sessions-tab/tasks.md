## 1. Backend Setup

- [x] 1.1 Add `rusqlite` dependency to `Cargo.toml`
- [x] 1.2 Create `src-tauri/src/session/` module directory with `mod.rs` (module declarations)
- [x] 1.3 Create `src-tauri/src/session/models.rs` with `SessionSummary`, `SessionPlatform`, `SessionMessage` structs
- [x] 1.4 Register `session` module in `src-tauri/src/lib.rs`

## 2. Claude Code Session Scanner

- [x] 2.1 Create `src-tauri/src/session/claude.rs` with project path decoder (`-Users-...` → `/Users/...`)
- [x] 2.2 Implement `scan_claude_sessions()` that walks `~/.claude/projects/*/` for `.jsonl` files
- [x] 2.3 Implement metadata extraction from JSONL head (first ~100 lines): custom-title, first user message, model, timestamp
- [x] 2.4 Implement `get_claude_messages()` with `BufReader::lines()` pagination for user/assistant message pairs

## 3. Codex CLI Session Scanner

- [x] 3.1 Create `src-tauri/src/session/codex.rs` with SQLite READONLY connection helper
- [x] 3.2 Implement `scan_codex_sessions()` querying `threads` table (id, title, cwd, model, tokens_used, created_at, updated_at)
- [x] 3.3 Implement `get_codex_messages()` reading rollout JSONL via `threads.rollout_path`, filtering event_msg/response_item lines

## 4. Tauri Commands

- [x] 4.1 Add `list_session_platforms` command in `commands.rs` — returns platforms with session counts
- [x] 4.2 Add `list_sessions(platform_id)` command — dispatches to Claude/Codex scanner
- [x] 4.3 Add `get_session_messages(platform_id, session_id, offset, limit)` command — paginated message fetch
- [x] 4.4 Register all 3 new commands in `lib.rs` invoke_handler

## 5. Frontend — HTML & JS

- [x] 5.1 Add `tab-sessions` button in `index.html` tab-bar
- [x] 5.2 Add `view-sessions` div in `index.html` main content area
- [x] 5.3 Add session tab click handler and `switchTab('sessions')` support in `app.js`
- [x] 5.4 Add `sessionPlatforms`, `selectedSessionPlatform`, `sessions` state properties to App class
- [x] 5.5 Add API methods: `listSessionPlatforms()`, `listSessions(platformId)`, `getSessionMessages(platformId, sessionId, offset, limit)`
- [x] 5.6 Implement `renderSidebar()` branch for sessions tab — show platforms with session counts
- [x] 5.7 Implement `renderView()` branch for sessions tab — session list with cards (title, project, model, time, tokens)
- [x] 5.8 Implement `renderToolbar()` branch for sessions tab
- [x] 5.9 Implement session click → modal with paginated message display

## 6. Locale

- [x] 6.1 Add session-related keys to `src/locales/en.json`
- [x] 6.2 Add session-related keys to `src/locales/zh-CN.json`

## 7. Testing & Polish

- [x] 7.1 Verify Claude Code sessions load correctly with real data
- [x] 7.2 Verify Codex CLI sessions load correctly with real data
- [x] 7.3 Test session detail modal with pagination
- [x] 7.4 Test locale switching between EN and Chinese
- [x] 7.5 `cargo build` passes with no warnings

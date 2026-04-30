## Why

Agent Hub 目前只支持 Skills 和 MCP 两个 tab，用户无法查看各 agent 平台的对话历史。用户需要一个集中的地方浏览和管理所有 agent session，包括查看会话列表、对话内容、使用的模型、token 消耗等信息。

## What Changes

- 新增 `session` 后端模块，实现 Claude Code 和 Codex CLI 两个平台的 session 扫描
- Claude Code: 读取 `~/.claude/projects/<encoded-path>/<sessionId>.jsonl` 文件，提取首条用户消息、模型、时间戳
- Codex CLI: 查询 `~/.codex/state_5.sqlite` 的 `threads` 表获取 session 元数据（title、cwd、model、tokens_used 等）
- 新增 `rusqlite` Rust 依赖用于读取 Codex SQLite 数据库
- 新增 3 个 Tauri 命令：`list_session_platforms`、`list_sessions`、`get_session_messages`
- 前端新增 Sessions tab，复用 MCP tab 的 sidebar + 主视图模式
- 支持按平台筛选、查看 session 列表、点击查看对话详情（分页加载）
- 新增中英文 locale 翻译条目

## Capabilities

### New Capabilities
- `session-scanner`: 后端 session 扫描能力，包含 Claude Code JSONL 解析器和 Codex CLI SQLite 读取器
- `session-ui`: 前端 Sessions tab 界面，包含平台选择、session 列表、对话详情查看

### Modified Capabilities
<!-- 无现有 spec 需要修改 -->

## Impact

- **新增依赖**: `rusqlite` (Rust crate，用于读 SQLite)
- **后端文件**: 新增 `src-tauri/src/session/` 模块（mod.rs, models.rs, claude.rs, codex.rs），修改 `lib.rs` 注册命令
- **前端文件**: 修改 `index.html`（新增 tab 按钮 + view div），修改 `app.js`（新增 tab 切换和渲染逻辑）
- **Locale 文件**: 修改 `en.json` 和 `zh-CN.json`
- **无破坏性变更**: 纯新增功能，不影响现有 Skills 和 MCP tab

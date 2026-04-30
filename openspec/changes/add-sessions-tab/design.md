## Context

Agent Hub 是一个 Tauri v2 桌面应用（Rust 后端 + vanilla JS 前端），目前支持 Skills 和 MCP 两个 tab。用户需要新增 Sessions tab 来浏览各 agent 平台的对话历史。

经实际验证，本机数据状态：
- **Claude Code**: `~/.claude/projects/<encoded-path>/` 下有 JSONL 会话文件，可读
- **Codex CLI**: `~/.codex/state_5.sqlite` 的 `threads` 表有 113 条 session 记录，字段完整（title, cwd, model, tokens_used 等）
- **Kiro**: 本机无数据，一期不实现

## Goals / Non-Goals

**Goals:**
- 新增 Sessions tab，支持 Claude Code 和 Codex CLI 两个平台的 session 浏览
- 左侧 sidebar 显示平台列表和 session 数量
- 主区域显示 session 列表（标题、项目路径、时间、模型、token 数）
- 点击 session 可查看对话内容（分页加载，每页 50 条消息）
- 中英文界面支持

**Non-Goals:**
- 不支持 Kiro（一期）
- 不支持 session 编辑/删除/恢复
- 不支持跨平台 session 搜索
- 不支持实时监控新 session（需手动刷新）

## Decisions

### 1. Claude Code session 扫描策略：遍历 projects 目录

**选择**: 扫描 `~/.claude/projects/` 下所有子目录的 `.jsonl` 文件，每个文件代表一个 session。

**替代方案**: 读 `~/.claude/sessions/*.json` 获取活跃 session 元数据。
**放弃原因**: `sessions/*.json` 只包含当前活跃进程的 session，不包含历史 session。JSONL 文件才是完整的会话存储。

**实现细节**:
- 目录名即项目路径编码（`-Users-liuyang-...` → `/Users/liuyang/...`）
- 每个 JSONL 文件名即 sessionId
- 只读前几行提取 metadata：首条 `type: "user"` 消息做 title，`type: "assistant"` 消息提取 model，timestamp 字段获取时间
- 检查 `custom-title` 类型行作为标题优先级最高
- 文件修改时间作为 updated_at

### 2. Codex CLI session 扫描策略：SQLite 索引表

**选择**: 直接查询 `~/.codex/state_5.sqlite` 的 `threads` 表。

**替代方案**: 遍历 `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` 文件。
**放弃原因**: SQLite 表本身就是 Codex 维护的索引，包含 title、cwd、model、tokens_used 等丰富字段，无需自己解析 JSONL。`codex resume` 命令也读这个表。

**实现细节**:
- 用 `rusqlite` 以 READONLY 模式打开，避免锁冲突
- SQL: `SELECT id, title, cwd, model, tokens_used, created_at, updated_at FROM threads ORDER BY updated_at DESC`
- 需要 `rollout_path` 字段关联到 JSONL 文件以读取对话详情

### 3. 大文件处理：分页读取

**选择**: 消息详情按需分页加载，每页 50 条。

**原因**: 实测 Claude Code 的 JSONL 文件最大 3.1GB，不能全量读入内存。

**实现**: `BufReader::lines()` 逐行读取，按 offset/limit 截取。后端维护行偏移量映射（type 为 user/assistant 的行号）。

### 4. 前端架构：复用 MCP tab 模式

**选择**: Sessions tab 的 sidebar + 主视图布局完全复用 MCP tab 的渲染模式。

**原因**: MCP tab 已经有成熟的平台选择 → 内容渲染流程，代码模式一致，降低开发复杂度。

## Risks / Trade-offs

- **[Codex SQLite 并发访问]** → SQLite 以 READONLY 打开，不写不锁。如果 Codex 正在写 WAL 日志，用 `PRAGMA journal_mode=wal` 兼容读取。
- **[Claude JSONL 文件过大]** → 列表阶段只读文件头部（前 100 行），详情阶段分页加载。用 `BufReader` 流式读取不加载全文件。
- **[项目路径编码解析错误]** → Claude 的路径编码规则是 `-` 替换 `/`，但路径本身可能含 `-`。需要从 `sessions/*.json` 的 `cwd` 字段做反向映射验证，或直接从 JSONL 内部的 `cwd` 字段获取。
- **[非官方数据格式变更]** → Claude Code 和 Codex CLI 的本地存储格式不是公开 API，未来版本可能变更。通过只读最稳定的字段（title, timestamp, model）降低风险。

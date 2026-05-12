## Context

Agent Hub 是一个 Tauri 2.x 桌面应用（Rust 后端 + Vanilla JS 前端），当前已有 Skill 管理、MCP Server 管理、Sessions 历史查看等功能。用户日常同时运行多个 AI Agent 实例，需要一个统一面板实时监控所有 agent 的运行状态。

各 agent 的本地数据结构已通过调研确认：
- **Kiro**: `~/.kiro/sessions/cli/` 下有 `.json`（元数据）、`.jsonl`（消息流）、`.lock`（活跃标记含 PID）
- **Claude Code**: `~/.claude/projects/{path-encoded}/{session-id}.jsonl` + 进程参数含 `--resume session-id` 和 `--model`
- **Codex CLI**: `~/.codex/logs_2.sqlite`（日志）+ `~/.codex/history.jsonl`（会话历史）+ 进程检测
- **Gemini CLI**: `~/.gemini/tmp/{project}/chats/session-*.jsonl` + `logs.json`

所有数据均在用户 home 目录下，读取无需额外系统权限。

## Goals / Non-Goals

**Goals:**
- 实时展示所有正在运行的 agent 实例（存活状态、当前模型、工作目录、运行时长）
- 同一 agent 的 CLI 和桌面客户端实例统一展示，通过标签区分
- 提供"输出完成"桌面通知功能（默认关闭，用户可开关）
- 对数据受限的 agent 在 UI 中显示明确的能力说明提示
- 跨平台支持（macOS + Windows），零额外系统权限
- 极低资源消耗（事件驱动，非轮询）

**Non-Goals:**
- 不提供向 agent 发送指令/交互的能力（只读监控）
- 不存储历史监控数据（只展示当前实时状态）
- 不支持远程 agent 监控（仅本地）
- 不逆向或连接 agent 的私有 IPC 协议
- 不修改任何 agent 的文件或进程

## Decisions

### 1. 监控架构：FSEvents + 进程轮询双层检测

**选择**: 文件系统事件监听（`notify` crate）作为主要信号源，进程存活检查（`sysinfo` crate）作为兜底。

**替代方案**:
- 纯轮询：简单但浪费 CPU，延迟高
- IPC Socket 连接：实时性最好但协议未公开，版本更新易 break
- ptrace/hook 注入：侵入性强，需要额外权限，稳定性差

**理由**: FSEvents/ReadDirectoryChangesW 是内核级事件推送，CPU 消耗接近零。进程轮询（每 5 秒）处理异常退出（kill -9 不会删 lock 文件）的边界情况。两者结合既准确又轻量。

### 2. 各平台适配器模式

**选择**: 定义统一的 `AgentMonitor` trait，每个平台实现各自的适配器。

```rust
trait AgentMonitor {
    fn platform_id(&self) -> &str;
    fn watch_paths(&self) -> Vec<PathBuf>;
    fn detect_sessions(&self) -> Vec<AgentSession>;
    fn on_fs_event(&mut self, event: &notify::Event) -> Vec<StateChange>;
}
```

**理由**: 各 agent 的数据格式差异大（JSONL vs SQLite vs 进程参数），统一 trait 隔离差异，新增 agent 只需实现一个适配器。

### 3. 会话状态判定策略

| Agent | 存活判定 | 状态判定 | 输出完成判定 |
|-------|---------|---------|------------|
| Kiro | `.lock` 文件 + PID 存活 | JSONL 最后一条的 `kind` 字段 | `.json` 的 `updated_at` 更新 |
| Claude Code | `claude` 进程存在 | JSONL 文件写入活动 | JSONL 停止写入 > 3s |
| Codex | `codex` 进程存在 | sqlite WAL 写入活动 | `history.jsonl` 新行写入 |
| Gemini | `node .../gemini` 进程存在 | `logs.json` 写入活动 | JSONL 停止写入 > 3s |

### 4. 通知系统设计

**选择**: 使用 Tauri 的 `tauri-plugin-notification`，配合 debounce + 冷却时间。

- 默认关闭，用户在 Monitor tab 顶部开关启用
- 冷却时间 30 秒（同一 agent 连续完成不重复通知）
- 通知内容：agent 名称 + 标签（CLI/Desktop）+ 会话标题摘要

**替代方案**: 自定义 NSUserNotification / Windows Toast — 增加维护成本，Tauri plugin 已封装好。

### 5. UI 布局：按 Agent 分组，标签标记来源

**选择**: 左侧 agent 列表（Kiro / Claude Code / Codex / Gemini），右侧展示该 agent 下所有活跃实例。每个实例卡片右上角显示 `CLI` 或 `Desktop` 标签。

对于数据受限的 agent，在其 tab 区域顶部显示一行黄色提示条：
> ⚠️ Codex 活跃会话期间无法读取对话内容，仅显示运行状态

### 6. 配置存储

在 `~/.agent-hub/config.toml` 中新增：

```toml
[monitor]
enabled = true
notification_enabled = false
notification_cooldown_secs = 30
```

### 7. 前端通信：Tauri Event 推送

**选择**: 后端通过 `app.emit("monitor:state-changed", payload)` 推送状态变更，前端监听事件更新 UI。

**理由**: 避免前端轮询后端，保持事件驱动的一致性。状态变更频率低（秒级），event 机制完全够用。

## Risks / Trade-offs

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| Agent 更新后数据格式变化 | 解析失败，状态不准 | 适配器内做版本检测和 graceful fallback，解析失败时显示"未知状态"而非崩溃 |
| Codex 活跃时无法读取对话内容 | 用户体验不完整 | UI 明确提示限制，展示可获取的信息（进程状态、运行时长） |
| SQLite 并发读取冲突（Windows） | 读取失败 | 使用 `SQLITE_OPEN_READONLY` + WAL 模式，不干扰 Codex 写入 |
| 进程名冲突（用户自定义脚本也叫 claude） | 误判为 agent 实例 | 结合进程路径 + 命令行参数双重验证 |
| FSEvents 在极端情况下事件延迟 | 状态更新延迟 1-2 秒 | 可接受，进程轮询兜底确保最终一致 |
| Gemini CLI 的 JSONL 可能是会话结束后才写入 | 无法实时追踪状态 | 降级为仅展示进程存活状态，UI 提示说明 |
| 通知过于频繁打扰用户 | 用户体验差 | 默认关闭 + 冷却时间 + 可配置 |

## Open Questions

1. Gemini CLI 的 JSONL 是实时追加还是会话结束后写入？需要实际运行时验证
2. Codex Desktop App 的会话数据是否与 CLI 共享同一个 sqlite？需要进一步确认
3. 是否需要支持 Claude Code 的 agent 子进程（如 debate team 中的子 agent）作为独立实例展示？

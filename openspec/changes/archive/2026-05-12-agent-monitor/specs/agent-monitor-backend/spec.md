## ADDED Requirements

### Requirement: 监控服务生命周期管理

系统 SHALL 在应用启动时初始化监控服务，在应用退出时清理所有文件监听器和定时器。监控服务 SHALL 作为 Tauri managed state 注册，确保全局单例。

#### Scenario: 应用启动时初始化监控服务
- **WHEN** Agent Hub 应用启动
- **THEN** 监控服务 SHALL 自动启动文件系统监听和进程轮询
- **THEN** 监控服务 SHALL 立即执行一次全量扫描，获取当前所有活跃 agent 实例

#### Scenario: 应用退出时清理资源
- **WHEN** Agent Hub 应用退出
- **THEN** 监控服务 SHALL 停止所有文件系统 watcher
- **THEN** 监控服务 SHALL 取消所有定时器
- **THEN** 不 SHALL 残留任何后台线程

### Requirement: 文件系统事件监听

系统 SHALL 使用 `notify` crate 监听各 agent 的会话数据目录，接收文件创建、修改、删除事件。macOS 使用 FSEvents 后端，Windows 使用 ReadDirectoryChangesW 后端。

#### Scenario: 监听 Kiro 会话目录
- **WHEN** 监控服务启动
- **THEN** SHALL 监听 `~/.kiro/sessions/cli/` 目录的文件变化事件
- **THEN** 当 `.lock` 文件创建时识别为新会话启动
- **THEN** 当 `.lock` 文件删除时识别为会话结束
- **THEN** 当 `.jsonl` 文件修改时识别为会话活动

#### Scenario: 监听 Claude Code 会话目录
- **WHEN** 监控服务启动
- **THEN** SHALL 递归监听 `~/.claude/projects/` 目录下所有 `.jsonl` 文件变化
- **THEN** 当 JSONL 文件有新写入时识别为会话活动

#### Scenario: 监听 Codex 数据文件
- **WHEN** 监控服务启动
- **THEN** SHALL 监听 `~/.codex/history.jsonl` 文件变化
- **THEN** SHALL 监听 `~/.codex/logs_2.sqlite-wal` 文件变化
- **THEN** 当 history.jsonl 有新行时识别为新会话输入

#### Scenario: 监听 Gemini CLI 会话目录
- **WHEN** 监控服务启动
- **THEN** SHALL 递归监听 `~/.gemini/tmp/` 目录下 `chats/` 子目录的 `.jsonl` 文件变化
- **THEN** SHALL 监听 `~/.gemini/tmp/*/logs.json` 文件变化

#### Scenario: 监听目录不存在
- **WHEN** 某个 agent 的数据目录不存在（未安装该 agent）
- **THEN** SHALL 跳过该目录的监听，不报错
- **THEN** SHALL 在日志中记录跳过原因

### Requirement: 进程存活检测

系统 SHALL 每 5 秒执行一次进程存活检查，作为文件系统事件的兜底机制。使用 `sysinfo` crate 获取进程列表。

#### Scenario: 检测 Kiro 进程
- **WHEN** 进程轮询触发
- **THEN** SHALL 检查进程名为 `kiro-cli-chat` 的进程（CLI 实例）
- **THEN** SHALL 检查进程名为 `kiro_cli_desktop` 的进程（桌面实例）
- **THEN** 对于 lock 文件中记录的 PID，SHALL 验证该 PID 是否仍然存活

#### Scenario: 检测 Claude Code 进程
- **WHEN** 进程轮询触发
- **THEN** SHALL 检查进程名为 `claude` 的进程
- **THEN** SHALL 通过进程路径区分 CLI（`~/.local/share/claude/` 或直接 `claude`）和桌面（路径含 `Claude-3p/claude-code/`）
- **THEN** SHALL 从进程命令行参数提取 `--model` 和 `--resume` 信息

#### Scenario: 检测 Codex 进程
- **WHEN** 进程轮询触发
- **THEN** SHALL 检查进程名为 `codex` 的进程（CLI，Rust 二进制）
- **THEN** SHALL 检查进程名为 `Codex` 的 Electron 主进程（桌面客户端）
- **THEN** SHALL 通过进程路径验证是真正的 Codex 而非同名程序

#### Scenario: 检测 Gemini CLI 进程
- **WHEN** 进程轮询触发
- **THEN** SHALL 检查命令行参数包含 `/bin/gemini` 的 node 进程
- **THEN** SHALL 从进程的 cwd 推断关联的项目目录

#### Scenario: 进程异常退出（kill -9）
- **WHEN** agent 进程被强制终止，未清理 lock 文件
- **THEN** 进程轮询 SHALL 检测到 PID 不再存活
- **THEN** SHALL 将对应会话标记为已结束
- **THEN** 不 SHALL 删除或修改 agent 的 lock 文件

### Requirement: 会话状态管理

系统 SHALL 维护一个内存中的会话状态表，记录所有已发现的活跃 agent 实例及其状态。

#### Scenario: 新会话发现
- **WHEN** 通过文件事件或进程检测发现新的 agent 实例
- **THEN** SHALL 创建会话记录，包含：agent 类型、来源标签（CLI/Desktop）、PID、启动时间、工作目录、模型名称
- **THEN** SHALL 通过 Tauri event 通知前端

#### Scenario: 会话状态更新
- **WHEN** 检测到会话活动（文件写入）
- **THEN** SHALL 更新会话的 `last_activity` 时间戳
- **THEN** SHALL 根据最新数据更新会话状态（idle/active/completed）
- **THEN** SHALL 通过 Tauri event 通知前端状态变更

#### Scenario: 会话结束
- **WHEN** 检测到 agent 进程不再存活
- **THEN** SHALL 将会话标记为已结束
- **THEN** SHALL 在 30 秒后从活跃列表中移除（给用户查看最终状态的时间）
- **THEN** SHALL 通过 Tauri event 通知前端

### Requirement: Kiro 适配器数据解析

系统 SHALL 解析 Kiro 的会话元数据和消息流，提取详细状态信息。

#### Scenario: 解析会话元数据
- **WHEN** 发现 Kiro 活跃会话
- **THEN** SHALL 读取 `{session-id}.json` 获取 title、model、cwd、context_usage_percentage
- **THEN** SHALL 将 model 信息展示为会话属性

#### Scenario: 解析消息流判断状态
- **WHEN** Kiro 的 `.jsonl` 文件有新写入
- **THEN** SHALL 读取文件最后一行
- **THEN** 若 `kind` 为 `Prompt`，状态 SHALL 为 "等待响应"
- **THEN** 若 `kind` 为 `Response`，状态 SHALL 为 "已完成回复"
- **THEN** 若 `kind` 为 `ToolUse`，状态 SHALL 为 "执行工具"

### Requirement: Claude Code 适配器数据解析

系统 SHALL 通过进程参数和 JSONL 文件解析 Claude Code 会话信息。

#### Scenario: 从进程参数提取会话信息
- **WHEN** 检测到 claude 进程
- **THEN** SHALL 从 `--resume` 参数提取 session-id
- **THEN** SHALL 从 `--model` 参数提取模型名称
- **THEN** SHALL 从 `--agent-name` 参数提取 agent 名称（如有）
- **THEN** SHALL 从进程 cwd 获取工作目录

#### Scenario: 监听 JSONL 判断输出完成
- **WHEN** Claude Code 的 session JSONL 文件停止写入超过 3 秒
- **THEN** SHALL 判定为一轮输出完成
- **THEN** SHALL 触发输出完成事件

### Requirement: Codex 适配器数据解析

系统 SHALL 通过 SQLite 只读查询和 history.jsonl 解析 Codex 会话信息。

#### Scenario: 读取 Codex SQLite 日志
- **WHEN** 需要获取 Codex 会话活动信息
- **THEN** SHALL 以 `SQLITE_OPEN_READONLY` 模式打开 `logs_2.sqlite`
- **THEN** SHALL 查询最近活跃的 `process_uuid` 和 `thread_id`
- **THEN** 不 SHALL 对数据库执行任何写操作

#### Scenario: 读取 Codex 历史记录
- **WHEN** `history.jsonl` 有新行写入
- **THEN** SHALL 解析最新行获取 session_id 和用户输入文本
- **THEN** SHALL 将用户输入文本作为会话标题

#### Scenario: Codex 数据受限说明
- **WHEN** Codex 会话处于活跃状态
- **THEN** SHALL 标记该会话为"数据受限"
- **THEN** SHALL 提供受限原因："活跃会话期间无法读取对话内容"

### Requirement: Gemini CLI 适配器数据解析

系统 SHALL 通过 logs.json 和 JSONL 文件解析 Gemini CLI 会话信息。

#### Scenario: 解析 Gemini 会话数据
- **WHEN** 检测到 Gemini CLI 活跃进程
- **THEN** SHALL 从进程 cwd 确定项目名
- **THEN** SHALL 读取 `~/.gemini/tmp/{project}/logs.json` 获取最近的用户输入作为标题

#### Scenario: 监听 Gemini JSONL 判断输出完成
- **WHEN** Gemini 的 session JSONL 文件停止写入超过 3 秒
- **THEN** SHALL 判定为一轮输出完成

#### Scenario: Gemini 数据可能受限
- **WHEN** Gemini 的 JSONL 文件在会话期间未实时更新
- **THEN** SHALL 标记该会话为"数据受限"
- **THEN** SHALL 提供受限原因："会话数据可能在结束后才写入"

### Requirement: Tauri 命令接口

系统 SHALL 暴露 Tauri 命令供前端调用，获取监控状态和控制监控行为。

#### Scenario: 获取所有活跃会话
- **WHEN** 前端调用 `get_active_sessions` 命令
- **THEN** SHALL 返回所有活跃会话的列表，每个会话包含：agent_type、source_tag（CLI/Desktop）、session_id、title、model、cwd、status、started_at、last_activity

#### Scenario: 获取监控配置
- **WHEN** 前端调用 `get_monitor_config` 命令
- **THEN** SHALL 返回当前监控配置（notification_enabled、notification_cooldown_secs）

#### Scenario: 更新监控配置
- **WHEN** 前端调用 `set_monitor_config` 命令
- **THEN** SHALL 更新内存中的配置
- **THEN** SHALL 持久化到 `~/.agent-hub/config.toml`

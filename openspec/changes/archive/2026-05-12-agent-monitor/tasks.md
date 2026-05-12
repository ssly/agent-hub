## 1. 项目基础设施

- [x] 1.1 在 `Cargo.toml` 中添加依赖：`notify`、`sysinfo`、`rusqlite`（features = ["bundled"]）
- [x] 1.2 在 `tauri.conf.json` 中添加 `notification` plugin 配置
- [x] 1.3 创建 `src-tauri/src/monitor/` 模块目录结构（mod.rs, types.rs, service.rs）
- [x] 1.4 在 `~/.agent-hub/config.toml` 中扩展 `[monitor]` 配置段的读写支持

## 2. 核心类型与 Trait 定义

- [x] 2.1 定义 `AgentSession` 结构体（agent_type, source_tag, session_id, title, model, cwd, status, started_at, last_activity）
- [x] 2.2 定义 `AgentMonitor` trait（platform_id, watch_paths, detect_sessions, on_fs_event）
- [x] 2.3 定义 `SessionStatus` 枚举（Active, Idle, Completed, Ended）
- [x] 2.4 定义 `StateChange` 事件类型（Added, Updated, Removed）

## 3. 平台适配器实现

- [x] 3.1 实现 Kiro 适配器：lock 文件解析、JSON 元数据读取、JSONL 尾部读取判断状态
- [x] 3.2 实现 Claude Code 适配器：进程参数解析（--model, --resume, --agent-name）、JSONL 监听
- [x] 3.3 实现 Codex 适配器：SQLite 只读查询、history.jsonl 解析、进程检测
- [x] 3.4 实现 Gemini CLI 适配器：logs.json 解析、JSONL 监听、进程 cwd 关联

## 4. 监控服务核心

- [x] 4.1 实现 MonitorService：初始化所有适配器、启动 notify watcher、启动进程轮询定时器（5s）
- [x] 4.2 实现会话状态表（HashMap）管理：新增、更新、移除（30s 延迟）
- [x] 4.3 实现"输出完成"检测逻辑：文件停止写入 debounce（3s）
- [x] 4.4 实现 Tauri managed state 注册和生命周期管理（启动/停止）

## 5. 通知子系统

- [x] 5.1 集成 tauri-plugin-notification，实现通知发送函数
- [x] 5.2 实现冷却机制（per-session HashMap<session_id, last_notified_at>）
- [x] 5.3 实现通知开关配置的读取和持久化

## 6. Tauri 命令接口

- [x] 6.1 实现 `get_active_sessions` 命令
- [x] 6.2 实现 `get_monitor_config` / `set_monitor_config` 命令
- [x] 6.3 实现 `monitor:state-changed` 事件推送到前端
- [x] 6.4 在 `lib.rs` 中注册命令和 managed state

## 7. 前端 UI

- [x] 7.1 在导航栏添加 Monitor tab 入口（含活跃数量徽标）
- [x] 7.2 实现 agent 分组列表布局（Kiro / Claude Code / Codex / Gemini，可折叠）
- [x] 7.3 实现会话实例卡片组件（标题、来源标签 CLI/Desktop、模型、cwd、运行时长、状态指示器）
- [x] 7.4 实现数据受限提示条（黄色警告条，按 agent 类型显示不同文案）
- [x] 7.5 实现通知开关控件（顶部工具栏 toggle）
- [x] 7.6 实现空状态页面
- [x] 7.7 监听 `monitor:state-changed` 事件，实时更新 UI（动画插入/淡出移除）

## 8. 国际化

- [x] 8.1 在 `src/locales/zh-CN.json` 和 `en.json` 中添加 Monitor 相关文本
- [x] 8.2 在 `locales/zh-CN.toml` 和 `en.toml` 中添加后端通知文本

## 9. 测试与验证

- [x] 9.1 验证各适配器在目标 agent 运行时能正确检测存活和状态
- [x] 9.2 验证进程异常退出（kill -9）后状态正确更新
- [x] 9.3 验证通知开关和冷却机制工作正常
- [x] 9.4 验证目录不存在时 graceful 跳过不报错

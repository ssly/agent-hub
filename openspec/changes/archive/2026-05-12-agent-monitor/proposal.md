## Why

当前用户同时运行多个 AI Agent（Kiro CLI/桌面、Codex CLI/桌面、Claude Code CLI/桌面、Gemini CLI），缺乏统一的可视化面板来监控所有 agent 的运行状态。用户需要在多个终端窗口之间切换才能了解各 agent 的工作进度，无法及时获知某个 agent 已完成输出。Agent Hub 已有 Sessions tab 展示历史会话，但缺少**实时监控**能力。

## What Changes

- 新增 **Agent Monitor** tab，实时展示所有正在运行的 agent 实例
- 支持 4 个平台的监控：Kiro（CLI + 桌面）、Codex（CLI + 桌面）、Claude Code（CLI + 桌面）、Gemini CLI
- 同一 agent 的 CLI 和桌面实例统一在一个 tab 下展示，通过标签（`CLI` / `Desktop`）区分
- 基于文件系统事件（FSEvents/ReadDirectoryChangesW）+ 进程检测实现零轮询监控
- 支持"输出完成通知"功能（桌面通知，默认关闭，可通过开关启用）
- 对于数据受限的 agent（如 Codex 活跃时无法读取对话内容），在 UI 中显示提示说明

## Capabilities

### New Capabilities

- `agent-monitor-backend`: 后端监控服务，包含文件系统监听、进程探测、会话状态管理、各平台数据解析适配器
- `agent-monitor-ui`: 前端监控面板 UI，包含 agent 列表、状态展示、标签标记、通知开关、受限提示
- `agent-monitor-notification`: 桌面通知子系统，输出完成时推送通知，支持开关和冷却时间配置

### Modified Capabilities

（无需修改现有 spec）

## Impact

- **后端新增模块**: `src-tauri/src/monitor/` — 监控服务核心逻辑
- **前端新增**: Monitor tab UI 组件、相关 i18n 条目
- **新增依赖**: `notify` crate（文件系统监听）、`sysinfo` crate（进程检测）；Tauri notification plugin
- **配置扩展**: `~/.agent-hub/config.toml` 新增 `[monitor]` 配置段
- **跨平台**: macOS 使用 FSEvents，Windows 使用 ReadDirectoryChangesW，均由 `notify` crate 统一封装
- **权限**: 零额外系统权限，仅读取用户目录下的文件和检测用户自己的进程

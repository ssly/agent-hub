<div align="center">

**[简体中文](README.md)** · [English](README.en.md)

# Agent Hub

**统一管理本地多个 AI Agent 平台的桌面应用**

在一个应用里管理各平台的插件（Skill、MCP Server、Claude Code 原生插件）、会话历史、实时监听与账号体系，
覆盖 Claude Code · Codex CLI · Cursor · Kimi Code · Grok Build · ZCode · Kiro 等平台。

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri 2.x](https://img.shields.io/badge/Tauri-2.x-blue?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![macOS](https://img.shields.io/badge/macOS-supported-success?logo=apple&logoColor=white)](https://github.com/ssly/agent-hub/releases)
[![Windows](https://img.shields.io/badge/Windows-supported-success?logo=windows&logoColor=white)](https://github.com/ssly/agent-hub/releases)

[下载安装](#安装) · [功能](#功能) · [支持的平台](#支持的平台) · [开发](#开发)

</div>

---

## 为什么需要 Agent Hub？

如果你同时使用多个 AI 编程 Agent，大概率遇到过这些麻烦：

- 给 Claude Code 写的 Skill，要手动复制到 Cursor 才能用
- MCP Server 配置散落在全局和项目目录里，格式还各不相同（JSON、TOML）
- Claude Code 插件、Skill、MCP Server 要在不同工具里分别管理
- 翻历史会话不方便，多个账号之间切换更麻烦
- 想知道各家配额还剩多少，得挨个打开官方工具查

Agent Hub 用一个桌面应用解决这些问题。

---

## 功能

### 🧩 插件工作区

- **统一工作区** — 在同一个 Agent 页面里管理 Skill、MCP Server 和 Claude Code 原生插件
- **全局 / 项目两种范围** — 使用各 Agent 的全局用户目录，或选择项目文件夹查看仓库内配置
- **Claude Code 插件** — 浏览已安装的原生插件，用户级插件可直接启用/停用；项目级、本地与托管范围保持只读展示
- **ZCode 插件市场** — 只读浏览 ZCode 市场制插件（登记信息 + 缓存实体 + 安装状态），启停请前往 ZCode 设置

#### Skill 管理

- **自动发现** — 检测已安装的 Agent 平台及其 Skill 目录
- **Skill 浏览** — 元数据视图（名称、版本、描述），文件内联预览
- **跨平台 Diff** — 逐行对比两个平台间的同名 Skill（Myers 算法）
- **一键同步** — 在平台间复制 Skill（或整个目录），目标已存在时先展示 Diff
- **全局搜索** — 跨平台搜索 Skill 名称与描述
- **回收站** — 删除的 Skill 保留 7 天，随时可恢复

#### MCP Server 管理

- 支持 **8 个平台**，JSON / TOML 格式自动识别（含 ZCode 的嵌套 `mcp.servers` 结构，读写保留未知字段）
- 手风琴式内联编辑，变更前先展示 Diff 预览
- 支持粘贴导入原始 JSON / TOML 配置
- 项目范围的 MCP 配置可与项目 Skill 一并查看；项目范围保持只读，避免误改仓库

### 📜 会话浏览器

- 浏览 **Claude Code**、**Codex CLI**、**Kiro**、**Grok CLI**、**Kimi Code**、**ZCode** 的历史会话
- 按项目路径过滤，分页浏览
- 查看完整消息记录（用户提问与助手回复）
- **批量 HTML 导出** — 把选中的会话导出为一个可搜索、自包含的 HTML 文件，任何现代浏览器都能打开
- **终端恢复会话** — 支持 Claude Code / Codex / Kiro / Grok / Kimi；**ZCode 为 Electron 桌面应用，不支持终端恢复**。终端：macOS Warp、iTerm、Ghostty、Terminal · Windows Windows Terminal、PowerShell、CMD

### 📡 会话监听

实时展示各 Agent 进行中 / 已结束的会话（状态、最新提问、助手回复）：

- **Codex** — 向 `~/.codex/hooks.json` 注入 `UserPromptSubmit` + `Stop` 两个生命周期 Hook
- **Claude Code** — 同样的 Hook 写入 `~/.claude/settings.json`（热加载无需重启），并追加 `StopFailure` 捕获 API 出错终止
- **Cursor** — `~/.cursor/hooks.json`（IDE 与 CLI 共用），事件 `beforeSubmitPrompt` / `afterAgentResponse` / `stop`；CLI 需 ≥2026-01-16 才有生命周期事件
- **Grok Build** — 独立受管 Hook 文件 `~/.grok/hooks/agent-hub.json`（五个事件：`UserPromptSubmit`、`Stop`、`StopFailure`、`SubagentStart`、`SubagentStop`），不改动共享配置
- **Kimi Code** — `~/.kimi-code/config.toml` 的 `[[hooks]]` 表（六个事件：含 `Interrupt`、`StopFailure`、`SubagentStart`、`SubagentStop`），纯文本块增删，注释格式原样保留
- **ZCode** — `~/.zcode/cli/config.json` 的 `hooks.events`（`UserPromptSubmit` + `Stop`），需 `hooks.enabled: true` 总闸
- 「全部」视图顶部有各 Agent 监听状态 Tag，一眼看清哪些已开启；旧版 Hook 会提示卸载重装
- Hook 安装 / 卸载一律先展示 Diff 预览，且只触碰 Agent Hub 自己管理的条目

### 📊 监控面板

从系统托盘图标或侧边栏左下角打开的常驻小窗，把"配额 + 实时会话"放在同一个面板里：

- **四平台用量查询** — Codex（5h / 7d 窗口 + 重置卡）、Claude Code（官方 OAuth 登录的 5h / 7d 窗口）、Grok Build、Kimi Code；查询不到的平台自动隐藏，一个都没有时展示固定空状态
- **泡泡水 + 圆环可视化** — 最短窗口是内部水罐，其余窗口是外圈圆环，明暗主题自适应（暗色为藏蓝色系）
- **监听简版区** — 与会话监听页同源的一行式实时会话（状态点 + Agent + 提问），状态翻转时中央大点脉冲缩小并弧线归位
- **右键两级菜单** — 不透明度（80–100%）、按平台隐藏使用量、按 Agent 隐藏监听，选择持久化
- **置顶常驻** — 置顶后可随意拖拽，每 5 分钟自动刷新配额，监听事件保持实时
- 弹窗内手动刷新与「账号」页共享同一份后端缓存，两边数据始终一致

### 👤 账号

- **四平台账号保存与切换** — Codex、Claude Code、Grok Build、Kimi Code；切换时原子替换（tmp + rename）
- **Claude Code** — 同时支持自定义 API Token 和官方 `/login` OAuth 订阅账号（OAuth 凭证写回 Keychain / 凭证文件）
- **Codex** — 按 `account_id` 识别当前活跃账号；Grok / Kimi 按各自凭证稳定身份检测
- 一键清空当前登录，已存 profile 可编辑 / 删除
- **四平台用量面板** — Codex、Claude Code、Grok Build、Kimi Code 的配额窗口查询，与监控面板数据同源同步

### 🎨 通用

- **自动更新** — 支持断点续传，minisign 签名校验
- **中英双语** — 跟随系统语言，随时一键切换
- **明暗双主题** — 水墨白 + 藏蓝暗色，偏好持久化，监控面板同步跟随
- **自定义平台** — 通过配置文件接入任意 Agent 平台

---

## 安装

### macOS

从 [Releases](https://github.com/ssly/agent-hub/releases) 下载 `.dmg`，打开后把应用拖进「应用程序」。

> [!WARNING]
> **首次打开被 Gatekeeper 拦截？** 未签名应用都会遇到，执行：
> ```bash
> xattr -cr /Applications/"Agent Hub.app"
> ```
> 然后重新打开即可。

### Windows

从 [Releases](https://github.com/ssly/agent-hub/releases) 下载 `.exe` 安装包并运行。

> [!WARNING]
> **SmartScreen 警告？** 点击「更多信息」→「仍要运行」。没有付费代码签名证书的应用都会出现此提示。

---

## 支持的平台

### 插件 / Skill 管理

| 平台 | Skill 目录 |
|------|-----------|
| 共享池 | `~/.agents/skills/` |
| Codex CLI | `~/.agents/skills/`（共享池） |
| Claude Code | `~/.claude/skills/` |
| Antigravity | `~/.gemini/config/skills/` |
| Grok Build | `~/.grok/skills/` |
| Kimi Code | `~/.kimi-code/skills/` |
| ZCode | `~/.zcode/skills/`（同时读共享池） |
| Cursor | `~/.cursor/skills/` |
| Hermes | `~/.hermes/skills/` |
| Trae | `~/.trae/skills/` |
| Kiro | `~/.kiro/skills/` |

共享池（`~/.agents/skills/`）默认被 Codex、Cursor、OpenCode、Kimi Code、Grok Build、ZCode 读取；Antigravity 在项目级读取 `.agents/skills/`。

### MCP Server 管理

| 平台 | 配置路径 | 格式 |
|------|---------|------|
| Claude Code | `~/.claude.json` | JSON |
| Antigravity | `~/.gemini/config/mcp_config.json` | JSON |
| Cursor | `~/.cursor/mcp.json` | JSON |
| Grok Build | `~/.grok/config.toml` | TOML |
| Kimi Code | `~/.kimi-code/mcp.json` | JSON |
| Kiro | `~/.kiro/settings/mcp.json` | JSON |
| Codex CLI | `~/.codex/config.toml` | TOML |
| ZCode | `~/.zcode/cli/config.json`（嵌套 `mcp.servers`） | JSON |

选择项目文件夹后，Agent Hub 会把各平台映射到仓库内布局（例如 Claude Code 使用 `.claude/skills/` 和 `.mcp.json`），项目范围内容只读展示。

### 会话浏览

| 平台 | 存储位置 | 终端恢复 |
|------|---------|---------|
| Claude Code | `~/.claude/projects/` | ✅ |
| Codex CLI | `~/.codex/`（threads DB） | ✅ |
| Kiro | `~/.kiro/sessions/cli/`（仅 kiro-cli） | ✅ |
| Grok Build | `~/.grok/sessions/` | ✅ |
| Kimi Code | `~/.kimi-code/sessions/` | ✅ |
| ZCode | `~/.zcode/v2/tasks-index.sqlite` + `~/.zcode/cli/db/db.sqlite` | ❌（Electron 桌面应用） |

### 会话监听

| 平台 | 机制 | 路径 |
|------|------|------|
| Codex CLI | 生命周期 Hook（`UserPromptSubmit` + `Stop`） | `~/.codex/hooks.json` |
| Claude Code | 生命周期 Hook，热加载（+ `StopFailure`） | `~/.claude/settings.json` |
| Cursor | 生命周期 Hook（`beforeSubmitPrompt` / `afterAgentResponse` / `stop`，CLI ≥2026-01-16） | `~/.cursor/hooks.json` |
| Grok Build | 独立受管 Hook 文件（5 事件） | `~/.grok/hooks/agent-hub.json` |
| Kimi Code | `[[hooks]]` 表（6 事件，纯文本块增删） | `~/.kimi-code/config.toml` |
| ZCode | `hooks.events`（需 `hooks.enabled: true`） | `~/.zcode/cli/config.json` |

### 账号切换 / 用量查询

| 平台 | 认证来源 | 内容 |
|------|---------|------|
| Codex CLI | `~/.codex/auth.json`（ChatGPT 登录，`account_id` 识别） | 账号切换 + 5h / 7d 窗口 + 重置卡 |
| Claude Code | API Token 或官方 OAuth（Keychain / `.credentials.json`） | 账号切换 + 5h / 7d 窗口 |
| Grok Build | `~/.grok/auth.json` | 账号切换 + 计费周期窗口 |
| Kimi Code | `~/.kimi-code/config.toml` 的 Coding Plan API Key | 账号切换 + 5h / 7d 窗口 |

---

## 配置

配置文件：`~/.agent-hub/config.toml`（首次启动自动创建）。

```toml
[general]
language = "auto"    # "auto" | "zh-CN" | "en"

[[platforms]]
id = "my-agent"
display_name = "My Custom Agent"
skill_dir = "~/.my-agent/skills"
```

---

## 开发

### 环境准备

- [Rust](https://rustup.rs/)（stable）
- [Node.js](https://nodejs.org/)（Vite + Vue 工具链）

### 快速开始

```bash
npm install     # 安装前端依赖
cargo tauri dev # 开发模式，热更新
npm run dev:web # 纯浏览器调试（mock 数据）
```

| 改动内容 | 效果 |
|---------|------|
| `src/**/*.vue` 或 `src/**/*.ts` | Vite HMR 热更新 |
| `src-tauri/src/*.rs` | 自动重新编译 |

### 构建

```bash
cargo tauri build          # 发布构建
cargo tauri build --debug  # 调试构建（更快）
```

macOS 产物：`src-tauri/target/release/bundle/`（`.app`、`.dmg`）
Windows 产物：`src-tauri/target/release/bundle/`（`.exe`、`.msi`、`.nsis`）

---

## 技术栈

| 层 | 技术 |
|----|------|
| 后端 | Rust + Tauri 2.x |
| 前端 | Vue 3 + Pinia + vue-i18n + TailwindCSS v4（Vite） |
| Diff 引擎 | similar（Myers diff 算法） |
| 数据库 | SQLite（rusqlite，内置捆绑） |
| HTTP | reqwest（rustls-tls） |

---

## 许可证

[MIT](LICENSE)

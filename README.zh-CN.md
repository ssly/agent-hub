<div align="center">

**[English](README.md)** · [简体中文](README.zh-CN.md)

# Agent Hub

**本地多 AI Agent 平台统一管理工具**

统一管理 Claude Code · Codex CLI · Cursor · Gemini · Kiro 等平台的 Skill、MCP Server、会话历史和账号配置 —— 一个桌面应用搞定一切。

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri 2.x](https://img.shields.io/badge/Tauri-2.x-blue?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![macOS](https://img.shields.io/badge/macOS-supported-success?logo=apple&logoColor=white)](https://github.com/ssly/agent-hub/releases)
[![Windows](https://img.shields.io/badge/Windows-supported-success?logo=windows&logoColor=white)](https://github.com/ssly/agent-hub/releases)

[下载安装](#安装) · [功能特性](#功能特性) · [支持平台](#支持平台) · [开发](#开发)

</div>

---

## 为什么需要 Agent Hub？

如果你同时使用多个 AI 编程 Agent，大概率遇到过这些问题：
- Claude Code 写好的 Skill 要手动复制到 Cursor、Gemini
- MCP Server 配置散落在不同路径、不同格式（JSON、TOML）
- 没有方便的方式浏览历史会话，切换账号也很麻烦

Agent Hub 就是为此而生。一个桌面应用，统一管理所有平台。

---

## 功能特性

### 🧩 Skill 管理

- **自动发现** — 自动检测已安装的 Agent 平台及其 Skill 目录
- **Skill 浏览** — 查看元数据（名称、版本、描述），在线预览任意文件内容
- **跨平台 Diff** — 选择两个平台，逐行对比同一 Skill 的差异（Myers 算法）
- **一键同步** — 将 Skill（或整个文件夹）从平台 A 同步到平台 B，目标已存在时展示差异供决策
- **无效 Skill 扫描** — 检测缺失 SKILL.md、frontmatter 异常、内容为空等问题 Skill，并生成修复提示词
- **全局搜索** — 跨平台搜索 Skill 名称和描述
- **回收站** — 删除的 Skill 保留 7 天，随时可恢复

### 🔌 MCP Server 管理

- 支持 **5 个平台**，JSON/TOML 格式自动转换
- 手风琴式展开编辑，即改即存
- 跨平台同步 — 提取通用字段（`command`、`args`、`env`），保留平台特有配置
- 粘贴导入原始 JSON/TOML 配置

### 📜 会话浏览器

- 浏览 **Claude Code**、**Codex CLI**、**Kiro** 的历史会话
- 按项目路径过滤，分页浏览
- 查看完整对话历史（用户消息 + 助手回复）
- **恢复会话** — 在终端中继续历史会话，支持 Warp、iTerm、Ghostty、macOS Terminal、Windows Terminal、CMD

### ⚡ 实时进程监控

- 实时检测运行中的 Agent 实例（Claude Code、Codex、Gemini、Kiro）
- 工作状态显示：Working / Waiting / Completed
- 最近输出预览，点击展开查看完整内容
- **完成钩子** — Agent 完成一轮对话时触发 shell 脚本钩子
- **桌面通知** — 系统原生通知，支持配置冷却时间防止刷屏

### 👤 账号切换

- 保存和切换 **Claude Code**、**Codex CLI** 的认证配置
- SHA-256 哈希比对，自动检测当前活跃账号
- 一键清除当前认证（自动备份）
- 编辑配置内容和备注

### 🎨 其他

- **自动更新** — 断点续传下载，minisign 签名验证
- **中英双语** — 自动检测系统语言，UI 内即时切换
- **明暗主题** — 墨光（Light）和墨夜（Dark）两套主题
- **自定义平台** — 通过配置文件添加任意 Agent 平台

---

## 安装

### macOS

从 [Releases](https://github.com/ssly/agent-hub/releases) 下载 `.dmg`，打开后将应用拖入 Applications 文件夹。

> [!WARNING]
> **首次打开被 Gatekeeper 拦截？** 未签名应用这是正常现象，执行以下命令：
> ```bash
> xattr -cr /Applications/"Agent Hub.app"
> ```
> 然后再次打开即可。

### Windows

从 [Releases](https://github.com/ssly/agent-hub/releases) 下载 `.exe` 安装包运行。

> [!WARNING]
> **遇到 SmartScreen 拦截？** 点击"更多信息" → "仍要运行"。没有付费代码签名证书的应用都会触发此提示。

---

## 支持平台

### Skill 管理

| 平台 | Skill 目录 |
|------|-----------|
| Claude Code | `~/.claude/skills/` |
| Codex CLI | `~/.codex/skills/` |
| Cursor | `~/.cursor/skills-cursor/` |
| Gemini | `~/.gemini/skills/` |
| OpenClaw | `~/.openclaw/skills/` |
| Hermes | `~/.hermes/skills/` |
| Trae | `~/.trae/skills/` |
| Kiro | `~/.kiro/skills/` |
| OpenCode | `~/.config/opencode/skills/` |
| Shared Pool | `~/.agents/skills/` |

### MCP Server 管理

| 平台 | 配置路径 | 格式 |
|------|---------|------|
| Claude Code | `~/.claude.json` | JSON |
| Cursor | `~/.cursor/mcp.json` | JSON |
| Gemini | `~/.gemini/settings.json` | JSON |
| Kiro | `~/.kiro/settings/mcp.json` | JSON |
| Codex CLI | `~/.codex/config.toml` | TOML |
| OpenCode | `~/.config/opencode/opencode.json` | JSON |

---

## 配置

配置文件：`~/.agent-hub/config.toml`（首次运行自动创建）。

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

### 环境要求

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (TailwindCSS CLI)

### 快速开始

```bash
npm install && npm run build:css   # 安装依赖，构建 CSS
cargo tauri dev                     # 启动开发模式（热重载）
```

| 修改内容 | 效果 |
|---------|------|
| `src/**/*.vue` 或 `src/**/*.ts` | Vite HMR 热重载 |
| `src-tauri/src/*.rs` | 自动重编译 |

### 构建

```bash
cargo tauri build          # Release
cargo tauri build --debug  # Debug（更快）
```

macOS 产物：`src-tauri/target/release/bundle/`（`.app`、`.dmg`）
Windows 产物：`src-tauri/target/release/bundle/`（`.exe`、`.msi`、`.nsis`）

---

## 技术栈

| 层 | 技术 |
|----|------|
| 后端 | Rust + Tauri 2.x |
| 前端 | Vanilla JS + TailwindCSS v4 |
| Diff 引擎 | similar（Myers diff algorithm）|
| 数据库 | SQLite（rusqlite，bundled）|
| 文件监听 | notify |
| 进程信息 | sysinfo |
| HTTP | reqwest（rustls-tls）|

---

## License

[MIT](LICENSE)

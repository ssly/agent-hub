<div align="center">

**[简体中文](README.md)** · [English](README.en.md)

# Agent Hub

**统一管理本地多个 AI Agent 平台的桌面应用**

Skill、MCP、会话、实时监听与账号用量，覆盖 Codex · Claude Code · Cursor · Antigravity · Grok Build · Kimi Code · ZCode · Kiro。

[![License: BSD-3-Clause](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)
[![Tauri 2.x](https://img.shields.io/badge/Tauri-2.x-blue?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![macOS](https://img.shields.io/badge/macOS-supported-success?logo=apple&logoColor=white)](https://github.com/ssly/agent-hub/releases)
[![Windows](https://img.shields.io/badge/Windows-supported-success?logo=windows&logoColor=white)](https://github.com/ssly/agent-hub/releases)

[使用说明](#使用说明) · [功能](#功能) · [支持的平台](#支持的平台) · [开发](#开发)

</div>

---

## 使用说明

从 [Releases](https://github.com/ssly/agent-hub/releases) 下载对应系统安装包。

### macOS

下载 `.dmg`，拖入「应用程序」。

首次被 Gatekeeper 拦截时执行：

```bash
xattr -cr /Applications/"Agent Hub.app"
```

### Windows

下载并运行 `.exe` 安装包。若出现 SmartScreen，选择「更多信息」→「仍要运行」。

---

## 功能

- **插件** — 按 Agent 管理 Skill、MCP；Claude 原生插件可启停；ZCode 市场插件只读列表
- **会话** — 浏览历史、消息、HTML 导出；Claude / Codex / Kiro / Grok / Kimi 可终端恢复（ZCode 桌面端不支持）
- **监听** — Hooks 实时展示进行中 / 已结束会话；安装前 Diff 预览
- **账号** — Codex / Claude / Grok / Kimi 用量查询；Claude 支持自定义 Token 与官方 OAuth 切换
- **托盘监控面板** — 用量圆环 + 简版监听；置顶、不透明度与显隐可配置
- **其它** — 自动更新、中英双语、明暗主题、自定义平台

---

## 支持的平台

| Agent | Skills | MCP | 会话 | 监听 | 账号 |
|-------|:------:|:---:|:----:|:----:|:----:|
| Codex | ✓ | ✓ | ✓ | ✓ | ✓ |
| Claude Code | ✓ | ✓ | ✓ | ✓ | ✓ |
| Cursor | ✓ | ✓ | — | ✓ | — |
| Antigravity | ✓ | ✓ | ✓ | ✓ | — |
| Grok Build | ✓ | ✓ | ✓ | ✓ | ✓ |
| Kimi Code | ✓ | ✓ | ✓ | ✓ | ✓ |
| ZCode | ✓ | ✓ | ✓ | ✓ | — |
| Kiro | ✓ | ✓ | ✓ | ✓ | — |

---

- **Cursor**：会话历史分散且无稳定官方会话 API，暂不接入会话浏览；用量在 Web 仪表盘，无法通过本地凭证 / 公开 API 可靠查询，暂不接入账号。
- **Antigravity**：账号绑定 Google 登录与配额形态，无稳定本地 auth 切换与用量接口，暂不接入账号。
- **ZCode**：用量多在控制台，无对等本地凭证 + 用量 API，暂不接入账号。
- **Kiro**：Builder ID / AWS 订阅体系与当前账号模块不匹配，暂不接入账号。

---

## 开发

```bash
npm install
cargo tauri dev          # 开发
npm run dev:web          # 浏览器 + mock
cargo tauri build        # 发布构建
```

依赖：Rust (stable)、Node.js。配置：`~/.agent-hub/config.toml`。

| 层 | 技术 |
|----|------|
| 后端 | Rust + Tauri 2.x |
| 前端 | Vue 3 + Pinia + vue-i18n + TailwindCSS v4 |

---

## 许可证

[BSD 3-Clause](LICENSE)

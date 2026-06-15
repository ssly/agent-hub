<div align="center">

**[English](README.md)** · [简体中文](README.zh-CN.md)

# Agent Hub

**Unified management for local AI Agent platforms**

Manage Skills, MCP Servers, session history, and account profiles across
Claude Code · Codex CLI · Cursor · Gemini · Kiro and more — in one desktop app.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri 2.x](https://img.shields.io/badge/Tauri-2.x-blue?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![macOS](https://img.shields.io/badge/macOS-supported-success?logo=apple&logoColor=white)](https://github.com/ssly/agent-hub/releases)
[![Windows](https://img.shields.io/badge/Windows-supported-success?logo=windows&logoColor=white)](https://github.com/ssly/agent-hub/releases)

[Download](#installation) · [Features](#features) · [Supported Platforms](#supported-platforms) · [Development](#development)

</div>

---

## Why Agent Hub?

If you work with multiple AI coding agents, you've probably felt the pain:
- Skills written for Claude Code need to be manually copied to Cursor and Gemini
- MCP Server configs are scattered across different files in different formats (JSON, TOML)
- No easy way to browse past conversations or switch between accounts

Agent Hub solves this. One desktop app to manage them all.

---

## Features

### 🧩 Skill Management

- **Auto-discovery** — Detects installed agent platforms and their skill directories automatically
- **Skill browser** — Metadata view (name, version, description), inline file preview
- **Cross-platform diff** — Select two platforms, compare the same skill line-by-line (Myers algorithm)
- **One-click sync** — Copy a skill (or an entire folder) from platform A to platform B; shows diff when target exists
- **Invalid skill scanner** — Detects missing `SKILL.md`, broken frontmatter, empty content — generates a fix prompt for your agent
- **Global search** — Search skill names and descriptions across all platforms
- **Trash bin** — Deleted skills are kept for 7 days, restorable anytime

### 🔌 MCP Server Management

- Supports **5 platforms** with JSON and TOML format auto-conversion
- Accordion-style inline editing
- Cross-platform sync — extracts universal fields (`command`, `args`, `env`), preserves platform-specific ones
- Paste-import raw JSON/TOML config

### 📜 Session Browser

- Browse history from **Claude Code**, **Codex CLI**, and **Kiro**
- Filter by project path, paginate through conversations
- View full message history (user & assistant turns)
- **Resume sessions** in your terminal — supports Warp, iTerm, Ghostty, macOS Terminal; Windows Terminal, CMD

### ⚡ Real-Time Process Monitor

- Detects running agent instances in real time (Claude Code, Codex, Gemini, Kiro)
- Shows working state: Working / Waiting / Completed
- Last output preview with click-to-expand
- **Completion hooks** — shell script hooks that fire when an agent finishes a turn
- **Desktop notifications** — OS-native alerts with configurable cooldown

### 👤 Account Switcher

- Save and switch authentication profiles for **Claude Code** and **Codex CLI**
- SHA-256 hash comparison to auto-detect active account
- One-click clear (auto-backup before clearing)
- Edit profile content and notes

### 🎨 General

- **Auto-update** — resumable downloads with minisign signature verification
- **Bilingual UI** — English and Chinese, auto-detected from system locale, instant toggle
- **Light & Dark themes** — two built-in themes, persists preference
- **Custom platforms** — add any agent platform via config file

---

## Installation

### macOS

Download `.dmg` from [Releases](https://github.com/ssly/agent-hub/releases), open it, and drag the app to Applications.

> [!WARNING]
> **First launch blocked by Gatekeeper?** This is expected for unsigned apps. Run:
> ```bash
> xattr -cr /Applications/"Agent Hub.app"
> ```
> Then open it again.

### Windows

Download `.exe` from [Releases](https://github.com/ssly/agent-hub/releases) and run the installer.

> [!WARNING]
> **SmartScreen warning?** Click "More info" → "Run anyway". This is expected for apps without a paid code-signing certificate.

---

## Supported Platforms

### Skill Management

| Platform | Skill Directory |
|----------|----------------|
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

### MCP Server Management

| Platform | Config Path | Format |
|----------|-------------|--------|
| Claude Code | `~/.claude.json` | JSON |
| Cursor | `~/.cursor/mcp.json` | JSON |
| Gemini | `~/.gemini/settings.json` | JSON |
| Kiro | `~/.kiro/settings/mcp.json` | JSON |
| Codex CLI | `~/.codex/config.toml` | TOML |
| OpenCode | `~/.config/opencode/opencode.json` | JSON |

---

## Configuration

Config file: `~/.agent-hub/config.toml` (auto-created on first launch).

```toml
[general]
language = "auto"    # "auto" | "zh-CN" | "en"

[[platforms]]
id = "my-agent"
display_name = "My Custom Agent"
skill_dir = "~/.my-agent/skills"
```

---

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (for TailwindCSS CLI)

### Quick Start

```bash
npm install && npm run build:css   # install deps, build CSS
cargo tauri dev                     # launch dev mode with hot reload
```

| What you change | What happens |
|-----------------|-------------|
| `src/**/*.vue` or `src/**/*.ts` | Vite HMR hot reload |
| `src-tauri/src/*.rs` | Auto-recompile |

### Build

```bash
cargo tauri build          # Release
cargo tauri build --debug  # Debug (faster)
```

macOS output: `src-tauri/target/release/bundle/` (`.app`, `.dmg`)
Windows output: `src-tauri/target/release/bundle/` (`.exe`, `.msi`, `.nsis`)

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust + Tauri 2.x |
| Frontend | Vanilla JS + TailwindCSS v4 |
| Diff Engine | similar (Myers diff algorithm) |
| Database | SQLite (rusqlite, bundled) |
| File Watching | notify |
| Process Info | sysinfo |
| HTTP | reqwest (rustls-tls) |

---

## License

[MIT](LICENSE)

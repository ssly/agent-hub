<div align="center">

**[English](README.md)** · [简体中文](README.zh-CN.md)

# Agent Hub

**Unified management for local AI Agent platforms**

Manage plugins (Skills, MCP Servers, and Claude Code extensions), session history, and account profiles across
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
- MCP Server configs are scattered across global and project files in different formats (JSON, TOML)
- Claude Code plugins, Skills, and MCP Servers have to be managed in separate tools
- No easy way to browse past conversations or switch between accounts

Agent Hub solves this. One desktop app to manage them all.

---

## Features

### 🧩 Unified Plugin Workspace

- **One workspace** — Manage Skills, MCP Servers, and Claude Code native plugins from the same Agent page
- **Global or project scope** — Use each Agent's global user directory or select a project folder to inspect its local configuration
- **Claude Code plugins** — Browse installed native plugins and enable or disable user-scoped plugins; project, local, and managed scopes remain visible as read-only

#### Skill Management

- **Auto-discovery** — Detects installed agent platforms and their skill directories
- **Skill browser** — Metadata view (name, version, description), inline file preview
- **Cross-platform diff** — Compare the same skill between two platforms, line-by-line (Myers algorithm)
- **One-click sync** — Copy a skill (or an entire folder) between platforms; shows a diff when the target already exists
- **Global search** — Search skill names and descriptions across all platforms
- **Trash bin** — Deleted skills are kept for 7 days, restorable anytime

#### MCP Server Management

- Supports **5 platforms** with JSON / TOML format auto-conversion
- Accordion-style inline editing
- Cross-platform sync — extracts universal fields (`command`, `args`, `env`), preserves platform-specific ones
- Paste-import raw JSON / TOML config
- Project-scoped MCP configs can be inspected alongside project Skills; project scope is read-only to avoid accidental repository changes

### 📜 Session Browser

- Browse history from **Claude Code**, **Codex CLI**, and **Kiro**
- Filter by project path, paginate through conversations
- View full message history (user & assistant turns)
- **Batch HTML export** — Export selected conversations as one searchable, self-contained HTML file that opens in any modern browser
- **Resume sessions** in your terminal — macOS: Warp, iTerm, Ghostty, Terminal · Windows: Windows Terminal, PowerShell, CMD

### 👤 Accounts

- Save and switch authentication profiles for **Claude Code** and **Codex CLI**
- SHA-256 hash comparison auto-detects the active account
- One-click clear of the active auth, with edit/delete on saved profiles
- **Codex usage panel** — query the 5h / 7d rate-limit windows and reset credits for the current account

### 🎨 General

- **Auto-update** — resumable downloads with minisign signature verification
- **Bilingual UI** — English and Chinese, auto-detected from system locale, instant toggle
- **Light & Dark themes** — two built-in themes, preference persisted
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

### Plugin / Skill Management

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
| Shared Pool | `~/.agents/skills/` |

### MCP Server Management

| Platform | Config Path | Format |
|----------|-------------|--------|
| Claude Code | `~/.claude.json` | JSON |
| Cursor | `~/.cursor/mcp.json` | JSON |
| Gemini | `~/.gemini/settings.json` | JSON |
| Kiro | `~/.kiro/settings/mcp.json` | JSON |
| Codex CLI | `~/.codex/config.toml` | TOML |

When a project folder is selected, Agent Hub maps each platform to its repository-local layout (for example, Claude Code uses `.claude/skills/` and `.mcp.json`). Project-scoped content is shown read-only.

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
- [Node.js](https://nodejs.org/) (for the Vite + Vue toolchain)

### Quick Start

```bash
npm install     # install frontend deps
cargo tauri dev # launch dev mode with hot reload
npm run dev:web # browser-only UI with mock data
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
| Frontend | Vue 3 + Pinia + vue-i18n + TailwindCSS v4 (Vite) |
| Diff Engine | similar (Myers diff algorithm) |
| Database | SQLite (rusqlite, bundled) |
| HTTP | reqwest (rustls-tls) |

---

## License

[MIT](LICENSE)

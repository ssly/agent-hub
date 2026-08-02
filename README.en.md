<div align="center">

[简体中文](README.md) · **[English](README.en.md)**

# Agent Hub

**Unified management for local AI Agent platforms**

Manage plugins (Skills, MCP Servers, and Claude Code extensions), session history, live session monitoring,
and account profiles across Claude Code · Codex CLI · Kiro · Kimi Code · Grok Build and more — in one desktop app.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri 2.x](https://img.shields.io/badge/Tauri-2.x-blue?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![macOS](https://img.shields.io/badge/macOS-supported-success?logo=apple&logoColor=white)](https://github.com/ssly/agent-hub/releases)
[![Windows](https://img.shields.io/badge/Windows-supported-success?logo=windows&logoColor=white)](https://github.com/ssly/agent-hub/releases)

[Download](#installation) · [Features](#features) · [Supported Platforms](#supported-platforms) · [Development](#development)

</div>

---

## Why Agent Hub?

If you work with multiple AI coding agents, you've probably felt the pain:

- Skills written for Claude Code need to be manually copied to Cursor
- MCP Server configs are scattered across global and project files in different formats (JSON, TOML)
- Claude Code plugins, Skills, and MCP Servers have to be managed in separate tools
- No easy way to browse past conversations or switch between accounts
- Checking remaining quota means opening every vendor's own tool, one by one

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

- Supports **8 platforms** with JSON / TOML format auto-conversion
- Accordion-style inline editing
- Cross-platform sync — extracts universal fields (`command`, `args`, `env`), preserves platform-specific ones
- Paste-import raw JSON / TOML config
- Project-scoped MCP configs can be inspected alongside project Skills; project scope is read-only to avoid accidental repository changes

### 📜 Session Browser

- Browse history from **Claude Code**, **Codex CLI**, **Kiro**, and **Grok CLI**
- Filter by project path, paginate through conversations
- View full message history (user & assistant turns)
- **Batch HTML export** — Export selected conversations as one searchable, self-contained HTML file that opens in any modern browser
- **Resume sessions** in your terminal — macOS: Warp, iTerm, Ghostty, Terminal · Windows: Windows Terminal, PowerShell, CMD

### 📡 Session Monitor

Watch live sessions in real time — running/ended status, the latest user prompt, and the agent's reply:

- **Codex** — two lifecycle Hooks (`UserPromptSubmit` + `Stop`) injected into `~/.codex/hooks.json`
- **Claude Code** — the same Hooks added to `~/.claude/settings.json` (hot-reloaded, no restart), plus `StopFailure` for turns killed by API errors
- **Grok Build** — a standalone managed Hook file `~/.grok/hooks/agent-hub.json` (including `StopFailure`); shared configs are never touched
- **Kimi Code** — `[[hooks]]` tables in `~/.kimi-code/config.toml` (including `Interrupt` and `StopFailure`), edited as plain text blocks so comments and formatting survive
- **Kiro** — pure file watching on `~/.kiro/sessions/cli/` (incremental tail + lock-file pid liveness). Works out of the box on any kiro-cli version, with zero writes to Kiro configuration
- The "All" view shows per-agent status tags at a glance, and outdated hook installs get an upgrade prompt
- Hook install/uninstall always shows a diff preview first and only ever touches Agent Hub's own entries

### 📊 Monitor Panel

An always-available popup opened from the system tray icon or the sidebar's bottom-left corner — quota and live sessions in one place:

- **Four usage providers** — Codex (5h / 7d windows + reset credits), Claude Code (official OAuth login, 5h / 7d windows), Grok Build, and Kimi Code; non-queryable providers hide themselves, and a fixed empty state appears when none are available
- **Bubble-tank + ring visualization** — the shortest window becomes the inner water tank, larger windows wrap it as rings; adapts to light/dark themes (dark is a navy palette)
- **Mini monitor strip** — one-line live sessions from the same source as the Monitor tab (status dot + agent + prompt); on status flips a large pulse blooms at the center, shrinks, and arcs into its row
- **Two-level context menu** — window opacity (80–100%), hide usage per provider, hide monitor per agent; choices persist
- **Pin on top** — draggable while pinned, quota auto-refreshes every 5 minutes, monitor events stay real-time
- Manual refresh inside the popup shares the same backend cache as the Accounts view, so both sides always agree

### 👤 Accounts

- **Claude Code** — save and switch both custom API-token accounts and official `/login` OAuth subscription accounts (OAuth credentials are written back to the Keychain / credentials file with atomic replacement)
- **Codex CLI** — reads the current CLI login and shows account info
- SHA-256 hash comparison auto-detects the active account
- One-click clear of the active auth, with edit/delete on saved profiles
- **Four usage panels** — quota windows for Codex, Claude Code, Grok Build, and Kimi Code, synced with the Monitor Panel

### 🎨 General

- **Auto-update** — resumable downloads with minisign signature verification
- **Bilingual UI** — English and Chinese, auto-detected from system locale, instant toggle
- **Light & Dark themes** — ink-light and navy-dark palettes, preference persisted, Monitor Panel follows along
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
| Shared Pool | `~/.agents/skills/` |
| Codex CLI | `~/.agents/skills/` (Shared Pool) |
| Claude Code | `~/.claude/skills/` |
| Antigravity | `~/.gemini/config/skills/` |
| Grok Build | `~/.grok/skills/` |
| Kimi Code | `~/.kimi-code/skills/` |
| Cursor | `~/.cursor/skills/` |
| Hermes | `~/.hermes/skills/` |
| Trae | `~/.trae/skills/` |
| Kiro | `~/.kiro/skills/` |

The Shared Pool (`~/.agents/skills/`) is read by default by Codex, Cursor, OpenCode, Kimi Code, and Grok Build; Antigravity reads `.agents/skills/` at project level.

### MCP Server Management

| Platform | Config Path | Format |
|----------|-------------|--------|
| Claude Code | `~/.claude.json` | JSON |
| Antigravity | `~/.gemini/config/mcp_config.json` | JSON |
| Cursor | `~/.cursor/mcp.json` | JSON |
| Grok Build | `~/.grok/config.toml` | TOML |
| Kimi Code | `~/.kimi-code/mcp.json` | JSON |
| Kiro | `~/.kiro/settings/mcp.json` | JSON |
| Codex CLI | `~/.codex/config.toml` | TOML |

When a project folder is selected, Agent Hub maps each platform to its repository-local layout (for example, Claude Code uses `.claude/skills/` and `.mcp.json`). Project-scoped content is shown read-only.

### Session Monitoring

| Platform | Mechanism | Path |
|----------|-----------|------|
| Codex CLI | Lifecycle Hooks (`UserPromptSubmit` + `Stop`) | `~/.codex/hooks.json` |
| Claude Code | Lifecycle Hooks, hot-reloaded | `~/.claude/settings.json` |
| Grok Build | Standalone managed Hook file | `~/.grok/hooks/agent-hub.json` |
| Kimi Code | `[[hooks]]` tables (plain-text block edits) | `~/.kimi-code/config.toml` |
| Kiro | Session file watching (read-only, zero config) | `~/.kiro/sessions/cli/` |

### Usage Query

| Platform | Auth Source | Content |
|----------|-------------|---------|
| Codex CLI | `~/.codex/auth.json` (ChatGPT login) | 5h / 7d windows + reset credits |
| Claude Code | Official OAuth credentials (Keychain / `.credentials.json`, read-only, never refreshed) | 5h / 7d windows |
| Grok Build | `~/.grok/auth.json` | Billing-period window |
| Kimi Code | Coding Plan API key in `~/.kimi-code/config.toml` | 5h / 7d windows |

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

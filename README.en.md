<div align="center">

[简体中文](README.md) · **[English](README.en.md)**

# Agent Hub

**Unified desktop app for local AI Agent platforms**

Skills, MCP, sessions, live monitoring, and account usage — Codex · Claude Code · Cursor · Antigravity · Grok Build · Kimi Code · ZCode · Kiro.

[![License: BSD-3-Clause](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)
[![Tauri 2.x](https://img.shields.io/badge/Tauri-2.x-blue?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![macOS](https://img.shields.io/badge/macOS-supported-success?logo=apple&logoColor=white)](https://github.com/ssly/agent-hub/releases)
[![Windows](https://img.shields.io/badge/Windows-supported-success?logo=windows&logoColor=white)](https://github.com/ssly/agent-hub/releases)

[Usage](#usage) · [Features](#features) · [Supported Platforms](#supported-platforms) · [Development](#development)

</div>

---

## Usage

Download the installer for your OS from [Releases](https://github.com/ssly/agent-hub/releases).

### macOS

Open the `.dmg` and drag the app to Applications.

If Gatekeeper blocks the first launch:

```bash
xattr -cr /Applications/"Agent Hub.app"
```

### Windows

Run the `.exe` installer. If SmartScreen appears, choose “More info” → “Run anyway”.

---

## Features

- **Plugins** — Skills and MCP per agent; Claude native plugins can be toggled; ZCode marketplace list is read-only
- **Sessions** — History, messages, HTML export; terminal resume for Claude / Codex / Kiro / Grok / Kimi (ZCode desktop has no terminal resume)
- **Monitor** — Live running / ended sessions via Hooks, with install/uninstall diff preview
- **Accounts** — Usage for Codex / Claude / Grok / Kimi; Claude supports custom tokens and official OAuth switch
- **Tray panel** — Usage rings + mini monitor; pin, opacity, and visibility options
- **Other** — Auto-update, bilingual UI, light/dark themes, custom platforms

---

## Supported Platforms

| Agent | Skills | MCP | Sessions | Monitor | Accounts |
|-------|:------:|:---:|:--------:|:-------:|:--------:|
| Codex | ✓ | ✓ | ✓ | ✓ | ✓ |
| Claude Code | ✓ | ✓ | ✓ | ✓ | ✓ |
| Cursor | ✓ | ✓ | — | ✓ | — |
| Antigravity | ✓ | ✓ | ✓ | ✓ | — |
| Grok Build | ✓ | ✓ | ✓ | ✓ | ✓ |
| Kimi Code | ✓ | ✓ | ✓ | ✓ | ✓ |
| ZCode | ✓ | ✓ | ✓ | ✓ | — |
| Kiro | ✓ | ✓ | ✓ | ✓ | — |

---

- **Cursor**: session history is scattered with no stable official session API, so the session browser is not supported; usage is on the web dashboard and cannot be queried reliably via local credentials / a public API, so accounts are not supported.
- **Antigravity**: auth is tied to Google login and quota shape with no stable local auth-switch + usage API, so accounts are not supported.
- **ZCode**: plan usage is mainly in the console with no equivalent local credential + usage API, so accounts are not supported.
- **Kiro**: Builder ID / AWS subscription does not match the current Accounts module, so accounts are not supported.

---

## Development

```bash
npm install
cargo tauri dev          # dev
npm run dev:web          # browser + mock
cargo tauri build        # release
```

Requires Rust (stable) and Node.js. Config: `~/.agent-hub/config.toml`.

| Layer | Stack |
|-------|--------|
| Backend | Rust + Tauri 2.x |
| Frontend | Vue 3 + Pinia + vue-i18n + TailwindCSS v4 |

---

## License

[BSD 3-Clause](LICENSE)

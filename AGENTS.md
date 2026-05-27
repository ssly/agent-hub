# AGENTS.md

Agent Hub 的项目上下文文档，供 AI Agent 和开发者快速了解项目。

## 项目概述

Agent Hub 是一个基于 Tauri 2.x 的桌面应用，用于统一管理本地多个 AI Agent 平台的 Skill、MCP Server、会话和账号。当前版本 **0.9.0**。

## 架构

```
用户界面 (Vanilla JS + TailwindCSS v4)
    ↕ Tauri IPC (invoke commands)
Rust 后端 (模块化设计)
    ↕ 文件系统 / SQLite / 网络
```

前端无构建打包工具，HTML 直接加载 ES Module JS。所有前端逻辑在 `src/js/app.js` 的 `App` 类中。

## 目录结构

```
src/
  index.html              # SPA 入口
  input.css               # TailwindCSS 输入
  styles.css              # 生成的 CSS（gitignored）
  theme.css               # 明暗主题变量
  js/
    api.js                # Tauri invoke 封装（所有后端命令）
    app.js                # 主应用（App 类，所有视图）
    i18n.js               # 前端国际化
    components/           # （待提取）
  locales/
    en.json / zh-CN.json  # 前端翻译

src-tauri/src/
  main.rs                 # 入口
  lib.rs                  # 应用初始化：状态、插件、命令注册
  commands.rs             # 所有 Tauri IPC 命令处理
  config.rs               # 配置加载/保存
  i18n.rs                 # 语言检测
  state.rs                # AppState（config, locale, platforms）
  trash.rs                # 回收站
  platform/               # 平台发现和注册
    registry.rs           # 内置平台定义（9 个 Skill 平台）
    discovery.rs          # 自动发现 + 自定义平台
  skill/                  # Skill 模型、解析、扫描
  diff/                   # Myers diff 引擎
  sync/                   # Skill 同步服务
  mcp/                    # MCP Server 管理（5 个平台）
    parser.rs             # JSON/TOML 配置解析
    writer.rs             # 配置回写
  session/                # 会话浏览器
    claude.rs             # Claude Code 会话适配
    codex.rs              # Codex CLI 会话适配
    kiro.rs               # Kiro 会话适配
  switch/                 # 账号切换
    model.rs              # AuthProfile, ProfileMeta
    commands.rs           # Profile CRUD + 切换
  monitor/                # Agent 监控（未启用）

locales/
  en.toml / zh-CN.toml    # 后端翻译（Rust i18n）
```

## 关键模块

### Skill 系统

Skill 是包含 `SKILL.md` 的目录，SKILL.md 使用 YAML frontmatter（`name`、`version`、`description`）+ Markdown body。扫描器递归遍历平台目录，用 canonical path 集合防止符号链接循环。

### Diff 引擎

使用 `similar` crate 实现 Myers diff，按文件逐一对比两个平台的同名 Skill。

### MCP Server

每个平台有独立的配置格式（JSON 或 TOML），`parser.rs` 统一解析为内部模型，`writer.rs` 按原格式回写。

### 会话浏览器

每个平台有独立的会话适配器（`claude.rs`、`codex.rs`、`kiro.rs`），读取各自的会话存储格式。支持分页浏览、消息查看、终端恢复。

### 账号切换

Profile 存储在 `~/.agent-hub/switch/<agent-type>/<uuid>/`，通过 SHA-256 哈希比对检测当前活跃账号。切换时原子替换（tmp + rename）。

## 开发命令

```bash
npm install                # 前端依赖
npm run build:css          # 构建 CSS
npm run dev:css            # 监听 CSS 变更
cargo tauri dev            # 开发模式
cargo tauri build          # 生产构建
cargo test                 # Rust 测试（在 src-tauri/ 下）
npm run version [-- <ver>] # 从 git tag 同步版本号
```

## 前端约定

- 单页应用，通过 `renderView()` 切换视图
- 侧边栏 tabs：Skills、MCP、Sessions、Switch
- 数据流：`app.js` 调用 `api.js` → Tauri IPC → Rust 命令
- 国际化：`i18n.js` 加载 `locales/*.json`
- 主题：`theme.css` CSS 变量 + localStorage 持久化

## 测试

Rust 单元测试覆盖 `session`、`trash`、`mcp/parser` 模块。

```bash
cd src-tauri && cargo test
```

## CI/CD

`.github/workflows/release.yml` — 推送 `v*` tag 触发，构建 macOS（aarch64 + x86_64）和 Windows 产物，使用 minisign 签名，生成 updater manifest。

# AGENTS.md

Agent Hub 的项目上下文文档，供 AI Agent 和开发者快速了解项目。

## 项目概述

Agent Hub 是一个基于 Tauri 2.x 的桌面应用，用于统一管理本地多个 AI Agent 平台的插件（Skill、MCP Server、Claude Code 原生插件）、会话和账号。当前版本 **0.19.0**。

## 架构

```
用户界面 (Vue 3 + Vite + TailwindCSS v4 + Pinia)
    ↕ Tauri IPC (invoke commands)
Rust 后端 (模块化设计)
    ↕ 文件系统 / SQLite / 网络
```

前端基于 Vue 3 单文件组件，Vite 构建。入口 `index.html` → `src/main.ts` → `src/App.vue`。状态用 Pinia store 管理，Tauri 调用统一封装在 `src/lib/api.ts`。浏览器调试模式（`npm run dev:web`）走 `src/lib/mock-api.ts` 的 mock 数据。

## 目录结构

```
src/
  index.html              # Vite SPA 入口（根目录）
  main.ts                 # Vue 应用挂载：createApp + Pinia + vue-i18n
  App.vue                 # 根组件（布局 + 路由切换）
  assets/                 # 全局样式
    theme.css             # 明暗主题变量
    main.css              # 基础样式
  components/
    layout/               # AppSidebar, AppToolbar, AppToast
    ui/                   # AppModal, AppSelect
    plugins/              # PluginView, ClaudePluginList
    skills/               # SkillListView, SkillDetailView
    mcp/                  # McpListView
    sessions/             # SessionListView
    switch/               # SwitchView（含 Codex 用量面板）
    diff/                 # DiffView
    search/               # SearchResults
  stores/                 # Pinia stores
    app.ts                # 全局/导航
    skills.ts             # Skill 与平台
    mcp.ts                # MCP Server
    plugins.ts            # 插件工作区与全局/项目范围
    claude-plugins.ts     # Claude Code 原生插件
    sessions.ts           # 会话浏览与 HTML 导出
    switch.ts             # 账号切换 + Codex 用量
  composables/
    useToast.ts           # 全局 toast
  lib/
    api.ts                # Tauri invoke 封装（所有后端命令，含 getCodexUsage）
    mock-api.ts           # 浏览器调试 mock 数据
    utils.ts              # 工具函数
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
  claude_plugin.rs        # Claude Code 原生插件读取与启停
  session/                # 会话浏览器与批量 HTML 导出
    claude.rs             # Claude Code 会话适配
    codex.rs              # Codex CLI 会话适配
    kiro.rs               # Kiro 会话适配
  switch/                 # 账号切换 + Codex 用量
    model.rs              # AuthProfile, ProfileMeta
    commands.rs           # Profile CRUD + 切换 + get_codex_usage
  monitor/                # Agent 监控（未启用）

locales/
  en.toml / zh-CN.toml    # 后端翻译（Rust i18n）
```

## 开发命令

```bash
npm install                # 前端依赖
npm run dev                # Vite 开发服务器（前端 only）
npm run dev:web            # 浏览器调试模式（mock 数据，无需 Tauri）
npm run build              # vue-tsc 类型检查 + vite 构建
cargo tauri dev            # 开发模式（自动启动 Vite）
cargo tauri build          # 生产构建
cargo test                 # Rust 测试（在 src-tauri/ 下）
npm run version [-- <ver>] # 从 git tag 同步版本号
```

## 前端约定

- Vue 3 SPA，通过 `src/App.vue` + Pinia `stores/app.ts` 切换视图
- 侧边栏 tabs：Plugins、Sessions、Accounts；Plugins 内聚合 Skill、MCP 与 Claude Code 原生插件
- 数据流：组件 → `stores/*` → `lib/api.ts` → Tauri IPC → Rust 命令
- 国际化：vue-i18n 加载 `src/locales/*.json`
- 主题：`src/assets/theme.css` CSS 变量 + localStorage 持久化
- 新增 Tauri 命令需同时在 `src/lib/api.ts`（真实调用）和 `src/lib/mock-api.ts`（浏览器调试）加对应函数

## 关键模块

### 插件工作区

插件工作区按 Agent 聚合 Skill、MCP Server 与 Claude Code 原生插件，支持全局用户目录和项目目录两种范围。项目范围用于查看仓库内配置，当前保持只读；Claude Code 用户范围原生插件支持启用/停用。

### Skill 系统

Skill 是包含 `SKILL.md` 的目录，SKILL.md 使用 YAML frontmatter（`name`、`version`、`description`）+ Markdown body。扫描器递归遍历平台目录，用 canonical path 集合防止符号链接循环。

### Diff 引擎

使用 `similar` crate 实现 Myers diff，按文件逐一对比两个平台的同名 Skill。

### MCP Server

每个平台有独立的配置格式（JSON 或 TOML），`parser.rs` 统一解析为内部模型，`writer.rs` 按原格式回写。

### 会话浏览器

每个平台有独立的会话适配器（`claude.rs`、`codex.rs`、`kiro.rs`），读取各自的会话存储格式。支持分页浏览、消息查看、终端恢复，以及将批量选中的会话导出为可搜索、自包含的 HTML 文件。

### 账号切换

Profile 存储在 `~/.agent-hub/switch/<agent-type>/<uuid>/`，按平台稳定身份检测当前活跃账号（Codex 用 `account_id`，Claude Code 用 `env.ANTHROPIC_AUTH_TOKEN`）。切换时原子替换（tmp + rename）。Switch 视图还提供 Codex 用量查询（`get_codex_usage` 命令，调用 ChatGPT 内部 usage 接口返回 5h/7d 窗口配额）。

## 测试

Rust 单元测试覆盖 `session`、`trash`、`mcp/parser` 模块。

```bash
cd src-tauri && cargo test
```

## CI/CD

`.github/workflows/release.yml` — 推送 `v*` tag 触发，构建 macOS（aarch64 + x86_64）和 Windows 产物，使用 minisign 签名，生成 updater manifest。

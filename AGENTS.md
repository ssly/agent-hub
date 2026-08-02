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
    ui/                   # AppModal, AppSelect, AppLoading（全局默认水波 loading，托盘与主应用共用）
    plugins/              # PluginView, ClaudePluginList
    skills/               # SkillListView, SkillDetailView
    mcp/                  # McpListView
    sessions/             # SessionListView + 会话/监听共用组件（SessionCard、SessionMessagesModal、SessionResumeModal，仅组件共用、数据不共用）
    switch/               # SwitchView（含各平台用量面板）
    tray/                 # 托盘监控面板：CodexTrayView + UsageOrb（泡泡水 + 圆环可视化）+ TrayWaveLoader（查询中水波 loading）；右键两级菜单（不透明度 / 隐藏使用量 / 隐藏监听，localStorage 持久化），区域无内容时展示固定空状态
    diff/                 # DiffView
    search/               # SearchResults
  stores/                 # Pinia stores
    app.ts                # 全局/导航
    skills.ts             # Skill 与平台
    mcp.ts                # MCP Server
    plugins.ts            # 插件工作区与全局/项目范围
    claude-plugins.ts     # Claude Code 原生插件
    sessions.ts           # 会话浏览与 HTML 导出
    switch.ts             # 账号切换 + 各平台用量（Codex/Claude/Grok/Kimi）
  composables/
    useToast.ts           # 全局 toast
  directives/
    auto-resize.ts        # v-auto-resize：textarea 自适应高度
    tooltip.ts            # v-tooltip：替代原生 title 的主题化悬浮提示
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
  paths.rs                # 跨平台路径公共库：join_relative 分段拼接、home_dir、replace_file（屏蔽 Windows/Mac 差异）
  platform/               # 平台发现和注册
    registry.rs           # 内置平台定义（10 个 Skill 平台，顺序即侧边栏顺序）
    discovery.rs          # 自动发现 + 自定义平台
  skill/                  # Skill 模型、解析、扫描
  diff/                   # Myers diff 引擎
  sync/                   # Skill 同步服务
  mcp/                    # MCP Server 管理（7 个平台）
    parser.rs             # JSON/TOML 配置解析
    writer.rs             # 配置回写
  claude_plugin.rs        # Claude Code 原生插件读取与启停
  session/                # 会话浏览器与批量 HTML 导出
    claude.rs             # Claude Code 会话适配
    codex.rs              # Codex CLI 会话适配
    kiro.rs               # Kiro 会话适配
    grok.rs               # Grok CLI 会话适配（~/.grok/sessions/<编码cwd>/<uuid>/ 下 summary.json + chat_history.jsonl）
    kimi.rs               # Kimi Code 会话适配（~/.kimi-code/sessions/<wd目录>/session_<uuid>/ 下 state.json + agents/main/wire.jsonl，workDir 取自 session_index.jsonl）
  session_monitor/        # 实时会话监听（Monitor 标签页）
    capture.rs            # Hook 事件捕获：--agent-hub-{codex,claude}-hook stdin → inbox 文件
    hooks.rs              # Codex hooks.json / Claude settings.json Hook 安装与卸载（预览 diff + hash 校验）
    kiro.rs               # Kiro CLI 会话目录文件监听（唯一通道，覆盖所有版本）
    service.rs            # 多 Agent 事件聚合服务（inbox watcher + Kiro watcher + Kiro 状态定时刷新）
    types.rs              # AgentKind、HookEvent、SessionState、MonitorSnapshot
  switch/                 # 账号切换 + 用量查询（Codex/Claude/Grok/Kimi）
    model.rs              # AuthProfile, ProfileMeta
    commands.rs           # Profile CRUD + 切换 + get_codex_usage / get_claude_usage / get_grok_usage / get_kimi_usage
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
- 侧边栏顶部为纵向导航（图标 + 文字）：Plugins、Sessions、Monitor、Accounts；Plugins 内聚合 Skill、MCP 与 Claude Code 原生插件
- 数据流：组件 → `stores/*` → `lib/api.ts` → Tauri IPC → Rust 命令
- 国际化：vue-i18n 加载 `src/locales/*.json`
- 主题：`src/assets/theme.css` CSS 变量 + localStorage 持久化
- 新增 Tauri 命令需同时在 `src/lib/api.ts`（真实调用）和 `src/lib/mock-api.ts`（浏览器调试）加对应函数

## 关键模块

### 插件工作区

插件工作区按 Agent 聚合 Skill、MCP Server 与 Claude Code 原生插件，支持全局用户目录和项目目录两种范围。项目范围用于查看仓库内配置，当前保持只读；Claude Code 用户范围原生插件支持启用/停用。

平台顺序（registry 定义顺序即侧边栏顺序）：Shared Pool → Codex → Claude Code → Antigravity → Grok Build → Kimi Code → Cursor / Hermes / Trae / Kiro。关键约定：

- **Shared Pool（`~/.agents/skills`）**：Codex、Cursor、OpenCode、Kimi Code、Grok Build 官方默认读取；Claude Code 与 Antigravity 全局层不读。
- **Codex**：官方用户级 skills 仅共享池（`~/.codex/skills` 是社区误传），前端显示"Skills 在 Shared Pool 目录下"并提供跳转，不渲染自己的 Skills 区块。
- **Antigravity**（agy CLI / Antigravity 2.0）：共享 `~/.gemini/config/`（skills + mcp_config.json + plugins）；项目级为 `.agents/skills`、`.agents/mcp_config.json`（`workspace_skill_dir` 有特判，不走镜像）。
- **Grok Build / Kimi Code / Antigravity / Codex 的插件体系**只在前端小字标注（`plugin.notes.*` i18n key），不管理；Claude Code 是唯一可管理的插件体系。
- 各平台的小字标注（notes）全部在前端 i18n（`plugin.notes.<platform-id>`），后端不透传。

### Skill 系统

Skill 是包含 `SKILL.md` 的目录，SKILL.md 使用 YAML frontmatter（`name`、`version`、`description`）+ Markdown body。扫描器递归遍历平台目录，用 canonical path 集合防止符号链接循环。

### Diff 引擎

使用 `similar` crate 实现 Myers diff，按文件逐一对比两个平台的同名 Skill。

### MCP Server

每个平台有独立的配置格式（JSON 或 TOML），`parser.rs` 统一解析为内部模型，`writer.rs` 按原格式回写。

### 会话浏览器

每个平台有独立的会话适配器（`claude.rs`、`codex.rs`、`kiro.rs`、`grok.rs`、`kimi.rs`），读取各自的会话存储格式。支持分页浏览、消息查看、终端恢复，以及将批量选中的会话导出为可搜索、自包含的 HTML 文件。

### 会话监听（session_monitor）

Monitor 标签页实时展示各 Agent 的进行中/已结束会话（用户问题 + 助手回复）。各平台注册的 Hook 事件按官方事件集裁剪（`hooks.rs` 的 `managed_events`）：Codex 为 `UserPromptSubmit` + `Stop`（其 Hook 系统没有中断/失败事件，Stop 覆盖所有轮次结束），Claude Code 与 Grok Build 追加 `StopFailure`（API 错误导致轮次终止，capture 归一化为 Stop），Kimi Code 追加 `Interrupt` + `StopFailure`，Kiro 走纯文件监听。旧版本安装（受管 handler 数与当前期望不符）会在监听页顶部显示升级提示条，引导卸载重装：

- **Codex**：向 `~/.codex/hooks.json` 注入 command Hook，调用自身二进制 `--agent-hub-codex-hook` 把 stdin JSON 原子写入 `~/.agent-hub/session-monitor/inbox/`。注意 Codex 有 Hook 信任门：用户级 hooks.json 的 handler 只有在 `~/.codex/config.toml` 的 `hooks.state."<hooks.json路径>:<event>:<组>:<序号>"` 里留下 `trusted_hash` 才会执行（TUI 启动审查 / 桌面端设置 → 钩子 里确认）；安装后未信任时 Hook 静默不触发，`get_hook_status` 会检测这种状态并在 `issue` 中提示（无法复算 Codex 的信任哈希，只查条目存在性）。
- **Claude Code**：同一机制，写入 `~/.claude/settings.json` 的 `hooks` 字段（热加载无需重启），Hook 参数为 `--agent-hub-claude-hook`。
- **Grok Build**：官方支持 hooks（`~/.grok/hooks/*.json` 全局免信任门，新会话生效），Agent Hub 使用独立受管文件 `~/.grok/hooks/agent-hub.json`（不编辑共享配置），Hook 参数 `--agent-hub-grok-hook`。注意 Grok 的 stdin 载荷是 camelCase（`hookEventName`/`sessionId`/`lastAssistantMessage`，事件值为 `user_prompt_submit`/`stop`/`stop_failure`），capture 统一归一化为 PascalCase。
- **Kimi Code**：官方支持 hooks（`~/.kimi-code/config.toml` 的 `[[hooks]]` 表，新会话生效），Hook 参数 `--agent-hub-kimi-hook`。注册四个事件：`UserPromptSubmit`、`Stop`、`Interrupt`（Kimi 在用户 Esc/Ctrl+C 中断时不发 Stop 只发 Interrupt，capture 归一化为 Stop；进程被直接杀死则无事件，"进行中"状态会残留——hook 方案固有限制）、`StopFailure`。因 config.toml 是用户主配置，安装/卸载走纯文本块增删（按 `--agent-hub-kimi-hook` 标记识别受管 `[[hooks]]` 块），不做 TOML 全量序列化，注释和格式原样保留。注意 Kimi 的 `prompt` 字段是 content-part 数组（`[{type:"text",text:…}]`），capture 的 `prompt_field` 负责拼接文本部分；`Stop` 载荷只有 `stop_hook_active`，不带助手回复文本。监听捕获的 sessionId 即 `session_<uuid>` 目录名，与会话浏览适配器互通，监听卡片可查看消息/恢复。
- **Kiro**：纯文件监听 `~/.kiro/sessions/cli/`（`.jsonl` 增量 tail 提取 Prompt/AssistantMessage；状态为 turn 级，与 Codex/Claude 对齐——提问置进行中、回复置已结束；`.lock` pid 只做单向兜底：进程死亡才把"进行中"翻转为"已结束"，10s 间隔刷新并主动推送；注意 kiro-cli 运行期间对 `.lock` 持有 OS 级独占锁，Windows 上 LockFileEx 是强制锁导致读不到内容——存在但不可读的 lock 必须判"进行中"，只有 lock 文件消失才判"已结束"），任意 kiro-cli 版本开箱即用、对 Kiro 配置零写入。面板提供"打开/关闭监听"开关（`~/.agent-hub/session-monitor/kiro-monitor.json` 持久化，运行时启停 watcher；关闭时状态线程仅空转休眠）。全局 Hook 方案（`~/.kiro/hooks/`）已验证在稳定版 kiro-cli 2.x 不生效（仅 IDE 1.0.182+ / v3 支持），故不采用。来源标记约定：监听到的 Kiro 会话全部来自 `~/.kiro/sessions/cli/`（kiro-cli 专属目录，IDE 客户端不写这里），因此卡片/托盘行按 `source === 'terminal'` 标记为 "Kiro CLI"（i18n `session_monitor.agent_kiro_cli`），左侧 Agent 列表与无法确认来源的场景统一用 "Kiro"；会话浏览平台名固定为 "Kiro CLI"。Codex 同理：监听行按 Hook originator（`CODEX_INTERNAL_ORIGINATOR_OVERRIDE` 含 desktop/chatgpt）标记为 "ChatGPT 客户端"；会话浏览按 `threads.source` 列映射（`vscode`→chatgpt、`cli`/`codex_cli`→terminal，其余 None 回退 "Codex"），`SessionSummary.source` 透传给前端 badge。

安装/卸载统一走预览 diff + before-hash 双重校验 + 原子写，只移除自己管理的 handler。`SessionMonitorService` 按 agent 路由事件到各自快照（`{codex,claude,kiro,grok,kimi}-state.json`），经 `session-monitor:{agent}-changed` Tauri event 推前端。旧的 `monitor/` 模块（FSEvents + 进程扫描）已停用，不要混淆。

### 账号切换

Profile 存储在 `~/.agent-hub/switch/<agent-type>/<uuid>/`，按平台稳定身份检测当前活跃账号（Codex 用 `account_id`，Claude Code 用 `env.ANTHROPIC_AUTH_TOKEN`）。切换时原子替换（tmp + rename）。Switch 视图还提供 Codex 用量查询（`get_codex_usage` 命令，调用 ChatGPT 内部 usage 接口返回 5h/7d 窗口配额）。

Claude Code 同时支持官方 `/login` OAuth 订阅账号（meta.json `kind` 为 `token`|`oauth`，老 profile 默认 `token`）：settings.json 无 env token 且凭证存在（macOS keychain `Claude Code-credentials` 或 `<CLAUDE_CONFIG_DIR|~/.claude>/.credentials.json`）即视为 oauth 模式，身份用 `~/.claude.json` 的 `oauthAccount.accountUuid`（回退 email）。oauth profile 的 config.json 存凭证 JSON 原文，切换时写回 keychain（`security add-generic-password -U`，失败回落原子写 `.credentials.json` 0600）并从 settings.json 移除 env token；清除/删除当前账号对 oauth 模式拒绝（须先在 Claude Code 中 /logout）。Token 策略只读不刷新，过期提示用户打开一次 Claude Code。`get_claude_usage` 命令用 OAuth access token 调 `https://api.anthropic.com/api/oauth/usage` 返回 5h/7d 窗口（`UsageProviderAvailability.claude_code` 标记凭证可用）。

## 测试

Rust 单元测试覆盖 `session`、`trash`、`mcp/parser` 模块。

```bash
cd src-tauri && cargo test
```

## CI/CD

`.github/workflows/release.yml` — 推送 `v*` tag 触发，构建 macOS（aarch64 + x86_64）和 Windows 产物，使用 minisign 签名，生成 updater manifest。

## macOS 应用名本地化

Dock/菜单栏显示名走系统语言：`src-tauri/Info.plist` 覆盖 `CFBundleDisplayName`/`CFBundleName` 为 "Agent Hub"（Tauri 会合并该文件，不覆盖 productName），`src-tauri/macos/{en,zh-Hans}.lproj/InfoPlist.strings` 提供英文 "Agent Hub" / 中文 "智能体中枢"，经 `bundle.resources` 映射进 app bundle 的 Resources 根目录。跟随 macOS 系统语言，与应用内语言切换无关。

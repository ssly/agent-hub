# AGENTS.md

Agent Hub 的项目上下文文档，供 AI Agent 和开发者快速了解项目。

## 项目概述

Agent Hub 是一个基于 Tauri 2.x 的桌面应用，用于统一管理本地多个 AI Agent 平台的插件（Skill、MCP Server、Claude Code 原生插件）、会话和账号。当前版本 **0.24.4**。

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
    plugins/              # PluginView, ClaudePluginList, ZCodePluginList, QwenPluginList
    skills/               # SkillListView, SkillDetailView
    mcp/                  # McpListView
    sessions/             # SessionListView + 会话/监听共用组件（SessionCard、SessionMessagesModal、SessionResumeModal，仅组件共用、数据不共用）
    switch/               # SwitchView（含各平台用量面板）
    tray/                 # 托盘监控面板：CodexTrayView + UsageOrb（泡泡水 + 圆环可视化）+ TrayWaveLoader（查询中水波 loading）+ useTrayDock.ts（边缘吸附 composable）；左上角控件（不透明度滑块 / 刷新间隔滑块（后端内存共享，非 localStorage）/ 隐藏使用量 / 隐藏监听 / mini（按钮已隐藏、代码保留，SHOW_MINI_TOGGLE 常量控制），localStorage 持久化；两区不可同时隐藏；mini 仅圆环+短名+恢复正常），区域无内容时展示固定空状态。边缘吸附（macOS Dock 式；判定与动画全在后端 tray.rs；面板无置顶概念——右上角为 X 关闭按钮（close_usage_tray：清 dock 态 + 隐藏），浮动面板失焦自动隐藏，吸附即常驻）：拖动全程窗口在松手时被钳制在屏幕内（X 物理边界并集 / Y 工作区，拖不出桌面）；吸附唯一触发条件是"光标在面板外持续 200ms（前端 mouseenter/mouseleave 经 set_usage_tray_hovered 上报）且窗口贴着显示器外侧左/右边缘"（双屏相接的缝不吸附；拖动中绝不吸附——触发时校验最近 200ms 无移动）；吸附后收成 20×72 竖条；悬停竖条滑出面板（expand_usage_tray；macOS 后台悬停靠 core-graphics 光标轮询，弹层打开时经 set_usage_tray_overlay 暂停自动收回）、光标移开 350ms 后收回（collapse_usage_tray，拖动中的 mouseleave 由后端按最近移动时间拦截）；拖着展开的面板远离边缘松手（Moved 停住 190ms 判定）即退出 dock 态，停在边缘则收回竖条；状态经 `usage-tray-dock-changed` 事件（edge + expanded）推给前端 useTrayDock.ts 镜像，尺寸动画前发 `usage-tray-dock-animating` 让前端隐藏内容、落地再显示；竖条内容为每个监听会话一个状态点（绿=工作中、灰=已结束，与监听条同一数据源），点数即会话数，高度按点数动态调整（前端经 resize_usage_tray_dock 推高度）；docked 态失焦不隐藏；点托盘图标/侧栏按钮重开一律是"全新打开"（清 dock、居中、400×120 起始高度，不再用记忆位置）
    diff/                 # DiffView
    search/               # SearchResults
  stores/                 # Pinia stores
    app.ts                # 全局/导航
    skills.ts             # Skill 与平台
    mcp.ts                # MCP Server
    plugins.ts            # 插件工作区与全局/项目范围
    claude-plugins.ts     # Claude Code 原生插件
    zcode-plugins.ts      # ZCode 插件市场（只读）
    qwen-plugins.ts       # Qwen Code 扩展（只读）
    sessions.ts           # 会话浏览与 HTML 导出
    switch.ts             # 账号切换 + 各平台用量（Codex/Claude/Grok/Kimi/DeepSeek）
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
    registry.rs           # 内置平台定义（12 个 Skill 平台，顺序即侧边栏顺序）
    discovery.rs          # 自动发现 + 自定义平台
  skill/                  # Skill 模型、解析、扫描
  diff/                   # Myers diff 引擎
  sync/                   # Skill 同步服务
  mcp/                    # MCP Server 管理（9 个平台；mcp_key 支持点分嵌套路径，如 ZCode 的 mcp.servers）
    parser.rs             # JSON/TOML 配置解析
    writer.rs             # 配置回写
  claude_plugin.rs        # Claude Code 原生插件读取与启停
  zcode_plugin.rs         # ZCode 插件市场只读扫描（marketplaces + cache + data 目录）
  qwen_plugin.rs          # Qwen Code 扩展只读扫描（~/.qwen/extensions/*/qwen-extension.json）
  session/                # 会话浏览器与批量 HTML 导出
    claude.rs             # Claude Code 会话适配
    codex.rs              # Codex CLI 会话适配
    antigravity.rs        # Antigravity 会话适配（列表读 ~/.gemini/antigravity-cli/conversation_summaries.db，消息读 brain/<id>/.../transcript.jsonl；恢复 `agy --conversation=<id>`）
    kiro.rs               # Kiro 会话适配
    grok.rs               # Grok CLI 会话适配（~/.grok/sessions/<编码cwd>/<uuid>/ 下 summary.json + chat_history.jsonl）
    kimi.rs               # Kimi Code 会话适配（~/.kimi-code/sessions/<wd目录>/session_<uuid>/ 下 state.json + agents/main/wire.jsonl，workDir 取自 session_index.jsonl）
    qwen.rs               # Qwen Code 会话适配（~/.qwen/projects/<sanitized-cwd>/chats/<sessionId>.jsonl 单文件单会话，cwd 非字母数字字符替换为 -，archive/ 子目录跳过；恢复 `qwen --resume <id>`）
    zcode.rs              # ZCode 会话适配（列表读 ~/.zcode/v2/tasks-index.sqlite 的 tasks 表，消息读 ~/.zcode/cli/db/db.sqlite 的 message+part 表；Electron 桌面应用，不支持终端恢复）
  session_monitor/        # 实时会话监听（Monitor 标签页）
    capture.rs            # Hook 事件捕获：--agent-hub-{codex,claude,cursor,grok,kimi,qwen,zcode,antigravity}-hook stdin → inbox 文件
    hooks.rs              # 各平台 Hook 配置安装与卸载（预览 diff + hash 校验）
    service.rs            # 多 Agent 事件聚合服务（inbox watcher）
    types.rs              # AgentKind、HookEvent、SessionState、MonitorSnapshot
  switch/                 # 账号切换 + 用量查询（Codex/Claude/Grok/Kimi/DeepSeek）
    model.rs              # AuthProfile, ProfileMeta
    commands.rs           # Profile CRUD + 切换 + get_codex_usage / get_claude_usage / get_grok_usage / get_kimi_usage
    monitor_settings.rs   # 用量监听共享设置（仅进程内存）：刷新间隔 1–10 分钟（默认 5）、当前选中 Agent、按 Agent 监听启停；setter 发 usage-monitor-settings-changed 事件，主窗口与托盘双窗口同步
    deepseek.rs           # DeepSeek 余额查询：Key 仅存本地 ~/.agent-hub/deepseek.json（0600），调官方 /user/balance（管理接口，不消耗 token）；仅在账号页，不进托盘 provider
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

平台顺序（`platform/registry.rs` 定义顺序即侧边栏顺序，会话/监听/账号子集保持同一相对顺序）：Shared → Codex → Claude Code → Cursor → Antigravity → Grok Build → Kimi Code → Qwen Code → ZCode → Kiro。关键约定：

- **Shared（id `shared`，`~/.agents/skills`）**：Codex、Cursor、OpenCode、Kimi Code、Grok Build、ZCode 官方默认读取；Claude Code 与 Antigravity 全局层不读。显示名中英文均为 Shared（侧边栏无中文 Agent 名）。
- **Codex**：官方用户级 skills 仅 Shared（`~/.codex/skills` 是社区误传），前端显示 Skills 在 Shared 目录下并提供跳转，不渲染自己的 Skills 区块。
- **Antigravity**（agy CLI / Antigravity 2.0）：共享 `~/.gemini/config/`（skills + mcp_config.json + plugins）；项目级为 `.agents/skills`、`.agents/mcp_config.json`（`workspace_skill_dir` 有特判，不走镜像）。
- **Grok Build / Kimi Code / Antigravity / Codex 的插件体系**只在前端小字标注（`plugin.notes.*` i18n key），不管理；Claude Code 是唯一可启停管理的插件体系；ZCode 插件市场为只读列表（见下条）。
- **ZCode**（智谱 Z.ai 官方编程工具）：skills 为 `~/.zcode/skills`（项目级 `.zcode/skills`）并同时读 Shared；MCP 在 `~/.zcode/cli/config.json` 的 `mcp.servers`（嵌套 map，server schema 严格——未知键会被 ZCode 整个丢弃，读写必须经 serde_json::Value 保留未知字段；禁用写 `"enabled": false`）。插件为 Claude Code 风格的市场制，**只读列表**（`get_zcode_plugins`，`zcode_plugin.rs`）：`~/.zcode/cli/plugins/marketplaces/<id>/marketplace.json` 登记插件（`plugins[].cachePath` 为准，登记 version 可能与缓存目录不一致），实体在 `cache/<市场>/<插件>/<版本>/`（manifest 优先 `.zcode-plugin/plugin.json`，回退 `.claude-plugin/plugin.json`）；`installed` 按 `data/<plugin>@<marketplace>/` 目录存在性判定（**推测语义**，官方文档只说启停状态在 config.json 的 plugins 键、本机未见，未证实），不做启停，UI 小字引导去 ZCode 设置操作。
- 各平台的小字标注（notes）全部在前端 i18n（`plugin.notes.<platform-id>`），后端不透传。
- **Qwen Code**（阿里通义千问编程工具，id `qwen`）：skills 为 `~/.qwen/skills`（项目级 `.qwen/skills`，走默认镜像无特判）；MCP 在 `~/.qwen/settings.json` 顶层 `mcpServers`（JSON，Gemini 风格；settings.json 还承载 hooks/auth 等其他配置，读写走 parser.rs 的外科式 JSON 路径保留未知字段）。扩展为**只读列表**（`get_qwen_plugins`，`qwen_plugin.rs`）：扫 `~/.qwen/extensions/<name>/qwen-extension.json`，统计 mcpServers 键数与 commands/skills/agents 数组长度；启停状态持久化位置未确认，不做启停。会话浏览与会话监听（hooks）已接入，见下文对应章节；账号切换不接入（无多账号概念、无远端用量接口，CLI `/stats` 仅为本地聚合）。

### Skill 系统

Skill 是包含 `SKILL.md` 的目录，SKILL.md 使用 YAML frontmatter（`name`、`version`、`description`）+ Markdown body。扫描器递归遍历平台目录，用 canonical path 集合防止符号链接循环。

### Diff 引擎

使用 `similar` crate 实现 Myers diff，按文件逐一对比两个平台的同名 Skill。

### MCP Server

每个平台有独立的配置格式（JSON 或 TOML），`parser.rs` 统一解析为内部模型，`writer.rs` 按原格式回写。

### 会话浏览器

每个平台有独立的会话适配器（`claude.rs`、`codex.rs`、`kiro.rs`、`grok.rs`、`kimi.rs`、`qwen.rs`、`zcode.rs`），读取各自的会话存储格式。支持分页浏览、消息查看、终端恢复（ZCode 是 Electron 桌面应用，无终端恢复命令，`build_resume_command` 对其返回明确错误，由恢复弹窗展示），以及将批量选中的会话导出为可搜索、自包含的 HTML 文件。平台显示名用产品名（如 "Kiro"），具体客户端在会话卡片 badge 上按 `SessionSummary.source` 区分（Kiro 会话全部来自 `~/.kiro/sessions/cli`，只有 kiro-cli 写这里，故 source 固定 `terminal`、badge 标 "Kiro CLI"；Codex 按 `threads.source` 列映射，`vscode`→ChatGPT 客户端）。

### 会话监听（session_monitor）

Monitor 标签页实时展示各 Agent 的进行中/已结束会话（用户问题 + 助手回复）。监听的 Agent 列表按"平台已安装"过滤：`list_available_monitor_agents` 命令遍历 `AgentKind::ALL`，按 `hooks.rs::agent_presence_path`（与 `platform/registry.rs` 的 presence_path 同目录语义）存在性判定，未安装的平台在监听页侧边栏、合并视图、托盘监听区（含 dock 红绿灯）都不显示；前端探测失败时降级为全量显示，不误隐藏。各平台注册的 Hook 事件按官方事件集裁剪（`hooks.rs` 的 `managed_events`）：Codex 为 `UserPromptSubmit` + `Stop`（其 Hook 系统没有中断/失败事件，Stop 覆盖所有轮次结束；官方另有 SubagentStart/SubagentStop，源码证实子 agent 轮次只发 SubagentStop，无需过滤），Claude Code 为 `UserPromptSubmit` + `Stop` + `StopFailure`（官方文档明确 Stop 仅主轮、子 agent 走 SubagentStop，安全），Grok Build 追加 `SubagentStart` + `SubagentStop`（原因见下），Kimi Code 追加 `Interrupt` + `StopFailure` + `SubagentStart` + `SubagentStop`，Qwen Code 为 `UserPromptSubmit` + `Stop` + `StopFailure`（同 Claude 语义，见下条），Cursor 为 `beforeSubmitPrompt` + `afterAgentResponse` + `stop`，ZCode 为 `UserPromptSubmit` + `Stop`（不使用 matcher）。旧版本安装（受管 handler 数与当前期望不符）会在监听页顶部显示升级提示条，引导卸载重装：

- **Codex**：向 `~/.codex/hooks.json` 注入 command Hook，调用自身二进制 `--agent-hub-codex-hook` 把 stdin JSON 原子写入 `~/.agent-hub/session-monitor/inbox/`。注意 Codex 有 Hook 信任门：用户级 hooks.json 的 handler 只有在 `~/.codex/config.toml` 的 `hooks.state."<hooks.json路径>:<event>:<组>:<序号>"` 里留下 `trusted_hash` 才会执行（TUI 启动审查 / 桌面端设置 → 钩子 里确认）；安装后未信任时 Hook 静默不触发，`get_hook_status` 会检测这种状态并在 `issue` 中提示（无法复算 Codex 的信任哈希，只查条目存在性）。**Windows**：Codex 不是直接 spawn hook 命令，而是经**会话 shell** 执行（codex-rs `build_hooks_for_config` 取环境检测到的 shell，Windows 默认 PowerShell → `powershell -NoProfile -Command "<command>"`）；裸的引号路径命令（`"C:\…\x.cmd" --arg`）在 PowerShell 里是解析错误（缺少 `&` 调用符），hook 以 exit code 1 失败且我们的二进制根本不会启动。因此 Windows 上 Codex 的 hook 命令必须带 `cmd /c` 前缀（`cmd /c "<shim>" --arg`，见 `hooks.rs::windows_hook_command`）——PowerShell 把它当原生命令调用，cmd 会话 shell 下嵌套 `cmd /c` 也能按 cmd 引号规则正确解析；其他 agent 保持裸引号 shim 形式（Grok 已实测）。来源标记约定：监听行按 Hook originator（`CODEX_INTERNAL_ORIGINATOR_OVERRIDE` 含 desktop/chatgpt）标记为 "ChatGPT 客户端"；会话浏览按 `threads.source` 列映射（`vscode`→chatgpt、`cli`/`codex_cli`→terminal，其余 None 回退 "Codex"），`SessionSummary.source` 透传给前端 badge。
- **Claude Code**：同一机制，写入 `~/.claude/settings.json` 的 `hooks` 字段（热加载无需重启），Hook 参数为 `--agent-hub-claude-hook`。capture 有来源校验：Claude 载荷必须是 snake_case（`hook_event_name`），纯 camelCase 载荷直接丢弃——因为 Grok CLI 会兼容执行 `~/.claude/settings.json` 里的 hook 并喂自己的 camelCase 载荷（实测），不拦截的话一次 Grok 运行会在 Claude 监听里种出幻影会话。
- **Grok Build**：官方支持 hooks（`~/.grok/hooks/*.json` 全局免信任门，新会话生效），Agent Hub 使用独立受管文件 `~/.grok/hooks/agent-hub.json`（不编辑共享配置），Hook 参数 `--agent-hub-grok-hook`。注册五个事件：`UserPromptSubmit`、`Stop`、`StopFailure`、`SubagentStart`、`SubagentStop`。注意 Grok 的 stdin 载荷是 camelCase（`hookEventName`/`sessionId`/`lastAssistantMessage`，事件值为 `user_prompt_submit`/`stop`/`stop_failure`），capture 统一归一化为 PascalCase。**Windows**：release 是 GUI 子系统，直接 spawn 二进制时 stdin 常为空导致监听静默失败；安装 Hook 时写 `~/.agent-hub/hook-runner/agent-hub-hook.cmd` 作为命令入口（由 cmd 转发到 exe），升级/重装后需在监听页卸载再安装 Grok Hook。失败时看 `~/.agent-hub/session-monitor/hook-capture-error.log`。实测（grok 0.2.x）三个坑都已处理：① 用户 prompt 在载荷里被 `<user_query>` 标签包裹，capture 解包后再展示；② Grok 子 agent 是独立子会话（自己的 sessionId），会发自己的 `user_prompt_submit` 但永不发 `stop`——不过滤的话每次 Task 工具调用都种出一个永远"进行中"、显示内部任务 prompt 的幻影行；capture 用 `SubagentStart` 载荷里的 `subagentId`（即子会话 sessionId）把子会话记入 ignored-sessions.json，其后续事件全部丢弃（Grok 子轮结束只发 `subagent_stop`，不发普通 `stop`，无需 Kimi 那种标记过滤）；③ 主轮结束后还会追加一个 `reason: "shutdown"` 的第二个 `stop`（会话关闭信号），capture 直接丢弃——对已知会话它只是重复标已结束，而对 hook 覆盖前创建的内部会话（如实测发现的 `grok-build-plan` 会话）它是唯一事件，不过滤会种出一条"暂未捕获用户问题"的噪音行。另外 Kimi 的 UserPromptSubmit 会把粘贴图片以 base64 塞进 prompt content parts，capture 的 stdin 上限因此是 8 MiB（只提取 text part），debug 日志对超 64 KiB 的载荷只留预览；hook-debug.jsonl / hook-capture-error.log 均整行单次 write_all 追加（writeln! 分块写会被并发 hook 进程交织损坏）。
- **Cursor**：官方 hooks（`~/.cursor/hooks.json`，IDE 与 CLI 共用；CLI 需 ≥2026-01-16 才有生命周期事件，旧版页内有升级提示），事件为 camelCase 生命周期名：`beforeSubmitPrompt`（归一化为 UserPromptSubmit）、`afterAgentResponse`（归一化为 AssistantResponse）、`stop`（载荷带 `status: completed|aborted|error`，覆盖正常/中断/出错，无需单独失败事件）。关键语义：**只有 `stop` 决定轮次结束**——`afterAgentResponse` 在一个 generation 内可能触发多次（每条助手消息一次），service 里 AssistantResponse 只填回复文本、不动状态（无前置 prompt 的新行默认 Ended，避免残留"进行中"）。会话关联用 `conversation_id`，轮次关联用 `generation_id`；capture 要求载荷必须含 `conversation_id`（Grok 兼容执行 `~/.cursor/hooks.json` 时喂的载荷没有它，以此拦截幻影事件）。子 agent 官方有独立 `subagentStart`/`subagentStop` 事件（与 Claude 同构，主 `stop` 应不受子 agent 影响），但因本机 CLI 版本过旧未实测，如有异常先查 hook-debug.jsonl。
- **Kimi Code**：官方支持 hooks（`~/.kimi-code/config.toml` 的 `[[hooks]]` 表，新会话生效），Hook 参数 `--agent-hub-kimi-hook`。注册六个事件：`UserPromptSubmit`、`Stop`、`Interrupt`（Kimi 在用户 Esc/Ctrl+C 中断时不发 Stop 只发 Interrupt，capture 归一化为 Stop；进程被直接杀死则无事件，"进行中"状态会残留——hook 方案固有限制）、`StopFailure`、`SubagentStart`、`SubagentStop`。后两者用于修正一个实测缺陷：Kimi 在**子 agent 的模型轮次结束时也发普通 `Stop`**（载荷与主轮 `Stop` 完全相同——只有 `stop_hook_active`，无法按字段区分；实测定序为 SubagentStart → 子 agent Stop（可能多个）→ SubagentStop → 主轮 Stop），不过滤的话每次 Agent 工具调用都会把监听卡片误标"已结束"。capture 用标记文件过滤：`~/.agent-hub/session-monitor/kimi-subagents/<sessionId>/<millis>-<uuid>` 每个在飞子 agent 一个文件（建/删免锁，并发安全），SubagentStart 建、SubagentStop 删最旧一个，带活标记时的 `Stop` 直接丢弃；`Interrupt`/`StopFailure` 永不过滤（关乎整轮，丢了会卡在"进行中"）。SubagentStop 丢失（进程被杀）时标记 1 小时过期自动清理。因 config.toml 是用户主配置，安装/卸载走纯文本块增删（按 `--agent-hub-kimi-hook` 标记识别受管 `[[hooks]]` 块），不做 TOML 全量序列化，注释和格式原样保留。注意 Kimi 的 `prompt` 字段是 content-part 数组（`[{type:"text",text:…}]`），capture 的 `prompt_field` 负责拼接文本部分；`Stop` 载荷只有 `stop_hook_active`，不带助手回复文本。监听捕获的 sessionId 即 `session_<uuid>` 目录名，与会话浏览适配器互通，监听卡片可查看消息/恢复。
- **Qwen Code**：官方 hooks（`~/.qwen/settings.json` 顶层 `hooks` 键，结构与 Claude Code 同构：`hooks.<EventName>[{matcher, hooks:[{type: "command", command, timeout}]}]`，无信任门，新会话生效），Hook 参数 `--agent-hub-qwen-hook`。注册 `UserPromptSubmit` + `Stop` + `StopFailure`（对齐 Claude：官方语义 Stop 仅主轮、子 agent 走 SubagentStop，故不注册 SubagentStart/SubagentStop、也不做 Kimi 式标记过滤）。stdin 载荷 snake_case 与 Claude 同形（`session_id`/`hook_event_name`/`prompt`/`last_assistant_message`），capture 来源校验要求同时含 `hook_event_name` + `session_id`（防 camelCase 载荷交叉执行产生幻影事件）。settings.json 承载全部用户配置，安装/卸载走 Claude 同款通用 JSON 外科路径（serde_json::Value 保留未知字段）。**timeout 单位是毫秒**（Qwen `hookRunner` 默认 60000、官方文档写 "Timeout in milliseconds"；Claude/Codex 是秒）——受管 handler 写 `10000`（10s）；旧版误写 `10`（=10ms）会在 Windows 上几乎必定超时（`.cmd` + GUI 子系统启动更慢），状态检测会识别为旧版本并提示重置。**Windows**：Qwen `hookRunner` 用 `spawn(cmd.exe, ['/d','/s','/c', command], {shell:false})`。Node 会再 QuoteCmdArg 包一层，cmd `/s` 剥掉外层引号后剩下 `\"path\"`，裸引号路径和 `cmd /c "path" --arg` 都会报「不是内部或外部命令」（已在真实 Windows 上用 node spawn 核实）。能活下来的写法是**不带引号的路径**（用户名无空格时）：`C:\Users\you\.agent-hub\hook-runner\agent-hub-hook.cmd --agent-hub-qwen-hook`；路径有空格则写 `shell: powershell` + `& 'path' --arg`。升级/重装后需在监听页重置 Qwen Hook。Qwen 也认顶层 `disableAllHooks`，状态检测会提示。**已知风险（未实测）**：Qwen Code 源自 Gemini CLI 分支，若实测发现子 agent 轮次也发普通 `Stop`（Kimi 同款缺陷），需用 hook-debug.jsonl 验证后再注册 SubagentStart/SubagentStop 并加过滤。
- **Kiro**：官方 hooks（全局 `~/.kiro/hooks/` + 项目 `.kiro/hooks/`，IDE 与 CLI 共用；Web/Mobile 无 hooks）。Agent Hub 写独立受管文件 `~/.kiro/hooks/agent-hub.json`（v1 schema：`hooks[]` + `trigger` + `action.command`），Hook 参数 `--agent-hub-kiro-hook`。注册 `UserPromptSubmit` + `Stop`；command 成功时 **stdout 必须为空**（Kiro 会把 stdout 注入上下文）。覆盖 Kiro IDE + Kiro CLI，不覆盖 Web/Mobile。
- **ZCode**：官方支持 hooks（用户级 `~/.zcode/cli/config.json`，session 启动时快照、只对新 session 生效，无信任门），Hook 参数 `--agent-hub-zcode-hook`。结构与 Claude Code 神似但带总闸：受管 handler 写在 `hooks.events.<事件>` 下，执行器为 `type: "process"`（command=二进制路径 + args 数组，不走 shell），且必须 `hooks.enabled: true` 才生效（安装时自动置 true；卸载只移除受管 handler，事件数组/ `events` 变空则连带移除，若 `hooks` 只剩 `enabled` 则整个移除恢复默认关闭；用户有其他 handler 时 `enabled` 保持原样）。config.json 还承载其他用户配置，读写走 serde_json::Value 外科式操作保留未知字段。stdin 载荷为 snake_case（附 camelCase alias）：`session_id`（sess_<uuid>）、`hook_event_name`、`cwd`，UserPromptSubmit 带 `prompt`，Stop 带 `last_assistant_message`。**已知风险（未实测）**：ZCode 官方事件集只有 7 个（SessionStart/UserPromptSubmit/PreToolUse/PermissionRequest/PostToolUse/PostToolUseFailure/Stop），**没有子 agent 专属事件**，但有 Agent/Task 工具；官方文档称 Stop 是"主模型准备结束"时触发，暗示子 agent 不发 Stop，但若实测发现子 agent 轮次也发普通 Stop（Kimi 同款缺陷），hook 层没有事件可用于过滤，需要 service 侧启发式（如 Stop 后检查会话 DB 是否有新消息）。ZCode 是桌面应用无法自动化测试，需在客户端里手动触发 Task 工具后用 hook-debug.jsonl 验证。

- **Antigravity**：官方 hooks（`~/.gemini/config/hooks.json`，CLI/IDE/2.0 共用，无信任门），Hook 参数 `--agent-hub-antigravity-hook`。受管命名条目 `agent-hub`，注册 `PreInvocation` + `Stop`（官方无 UserPromptSubmit；用户文案从 `transcriptPath` 的 transcript.jsonl 读最后一条 `USER_INPUT`）。stdin 为 camelCase（`conversationId`/`workspacePaths`/`transcriptPath`），载荷不带 hookEventName，capture 按字段形状推断事件。新会话生效。

安装/卸载统一走预览 diff + before-hash 双重校验 + 原子写，只移除自己管理的 handler。`SessionMonitorService` 按 agent 路由事件到各自快照（`{codex,claude,cursor,grok,kimi,qwen,zcode,antigravity}-state.json`），经 `session-monitor:{agent}-changed` Tauri event 推前端。调试 hook 载荷：debug 构建或设 `AGENT_HUB_HOOK_DEBUG=1` 时 capture 会把每个原始载荷追加到 `~/.agent-hub/session-monitor/hook-debug.jsonl`（含用户 prompt，仅本地调试用，release 构建默认不写）。旧的 `monitor/` 模块（FSEvents + 进程扫描）已停用，不要混淆。

### 账号切换

Profile 存储在 `~/.agent-hub/switch/<agent-type>/<uuid>/`，按平台稳定身份检测当前活跃账号（Codex 用 `account_id`，Claude Code 用 `env.ANTHROPIC_AUTH_TOKEN`）。切换时原子替换（tmp + rename）。Switch 视图还提供 Codex 用量查询（`get_codex_usage` 命令，调用 ChatGPT 内部 usage 接口返回 5h/7d 窗口配额）。

Claude Code 同时支持官方 `/login` OAuth 订阅账号（meta.json `kind` 为 `token`|`oauth`，老 profile 默认 `token`）：settings.json 无 env token 且凭证存在（macOS keychain `Claude Code-credentials` 或 `<CLAUDE_CONFIG_DIR|~/.claude>/.credentials.json`）即视为 oauth 模式，身份用 `~/.claude.json` 的 `oauthAccount.accountUuid`（回退 email）。oauth profile 的 config.json 存凭证 JSON 原文，切换时写回 keychain（`security add-generic-password -U`，失败回落原子写 `.credentials.json` 0600）并从 settings.json 移除 env token；清除/删除当前账号对 oauth 模式拒绝（须先在 Claude Code 中 /logout）。Token 策略只读不刷新，过期提示用户打开一次 Claude Code。`get_claude_usage` 命令用 OAuth access token 调 `https://api.anthropic.com/api/oauth/usage` 返回 5h/7d 窗口（`UsageProviderAvailability.claude_code` 标记凭证可用）。

**用量监听共享设置**（`switch/monitor_settings.rs`）：主窗口（账号页 + 侧栏版本号旁的设置弹窗）与托盘监控面板是两个独立 webview，共享状态全部放在后端进程内存（重启归零）：刷新间隔（1–10 分钟，默认 5）、当前选中 Agent、按 Agent 的监听启停（缺省=启用）。setter 返回完整快照并广播 `usage-monitor-settings-changed`，两端各自监听套用；选中 Agent 双向同步（托盘 tab 点击 ↔ 账号页 selectAgent，事件回环按相等值幂等终止）。启停按钮在账号页用量面板头部（刷新按钮旁，`ListeningToggle.vue`）；停用时面板内容收起为提示行，且账号页进入、托盘打开不再自动查询（手动刷新按钮仍可用）；托盘的 provider tab 直接隐藏被停用的 Agent（选中项被停用时回落到第一个仍启用的 tab，全部停用时显示固定空状态）。自动刷新只在"活跃面板"发生：账号页定时器要求主窗口可见（`document.visibilityState`），托盘定时器要求托盘窗口可见（浮动打开或吸附竖条均可），两侧间隔同为共享值。

## 测试

Rust 单元测试覆盖 `session`、`trash`、`mcp/parser` 模块。

```bash
cd src-tauri && cargo test
```

## CI/CD

`.github/workflows/release.yml` — 推送 `v*` tag 触发，构建 macOS（aarch64 + x86_64）和 Windows 产物，使用 minisign 签名，生成 updater manifest。**发布流程防竞态**：三个 matrix job 统一上传到 **draft** release（`releaseDraft: true`，资产用 `releaseAssetNamePattern: agent-hub_[version]_[arch][setup][ext]` 命名，updater 清单 URL 自动跟随），全部完成后 publish job 才 `gh release edit --draft=false` 公开（job 不 checkout，必须设 `GH_REPO`，否则 gh 找不到仓库，draft 永远不会公开）——构建窗口期 `releases/latest` 一直指向上一个正式版，检查更新不会拿到残缺的 latest.json。

## 应用名称统一约定

**macOS 主窗口与 Dock**：红灯/关闭主窗口时 `prevent_close` + `hide`（不销毁），进程由菜单栏托盘保活；点 Dock 图标走 `RunEvent::Reopen` → `show_main_window`（先 `app.show()` 解除 Cmd+H 级应用隐藏，再 show/unminimize/focus main；main 已销毁则按 `tauri.conf.json` 参数重建）。调度中心/App Exposé 在主窗口隐藏时看不到应用窗口属预期，重新打开后恢复。

显示名统一为 **Agent Hub**（不分语言）：`tauri.conf.json` 的 `productName` 与主窗口 `title`、`src-tauri/Info.plist` 的 `CFBundleDisplayName`/`CFBundleName`、`src-tauri/macos/{en,zh-Hans}.lproj/InfoPlist.strings`、应用内标题（`app.title` i18n）全部是 "Agent Hub"。文件名统一 **agent-hub** 格式：可执行文件由 Cargo 包名天然产出（`agent-hub`）；release 资产经 tauri-action 的 `releaseAssetNamePattern: agent-hub_[version]_[arch][setup][ext]` 命名（GitHub 会把空格转成点，自定义改名步骤不要按带空格的名字匹配资产）。

# TODOS

跨 sprint 的 follow-up 列表。每条 follow `What / Why / Pros / Cons / Context / Depends on` 格式。

## T1: Codex / Gemini Tier-1 升级 (sqlite WAL + logs.json 反向工程)

**What**: 把 Codex / Gemini 从 Tier-2 (只 "running · 12m") 升级为完整状态机 (工具调用 / turn-end 检测)。

**Why**: 设计中明说 "Tier-2 是能力局限子集,不是产品选择"。用户看到 Codex/Gemini 永远只 "running",会质疑「为什么不如 Kiro/Claude」。Success Criteria 明说「4 种 agent UX 一致」,Sprint 1 暂时未达。

**Pros**:
- 服设计意图 (Tier-2 是临时局限)
- Codex 生态会越用越多 (项目内多 agent),拖久成本越高
- AI 帮实现,human dev 边际成本低

**Cons**:
- Codex 可能改 sqlite schema → 反向工程要重做
- Gemini logs.json 实时性未验证 (openspec design.md open question #1) — 有可能写时机晚到无法表达 working 状态
- 两个生态都不公开协议,版本强升风险

**Context**: 检查文件: `~/.codex/logs_2.sqlite` (history table) + `~/.codex/history.jsonl` + `~/.gemini/tmp/{project}/chats/session-*.jsonl` + `logs.json`。

**Depends on / blocked by**: Sprint 1 (Tier-1/Tier-2 架构 + 状态机) 落地。仅是 adapter 实现工作,架构不变。

**Estimate**: 每个 agent 0.5-1 天 + 实际运行验证 = 总 2-3 天。

---

## T2: Frontend 自动化测试框架 (Vitest + Playwright)

**What**: 让 `src/js/` (api.js / app.js / components) 有自动化测试。当前 package.json 只 dev:css/build:css/version,无 test runner。

**Why**:
- Sprint 1 monitor tab 的 8+ UX flow 全是 manual checklist;每次 UI refactor 都要人走一遍
- user preference 「测试过多」— frontend 没任何自动化覆盖违反原则
- monitor tab 以外还有 sessions tab / skill 管理 / mcp 管理,同样需要

**Pros**:
- Manual QA 1.5 小时一次 → 自动化后 ≈30 秒
- regression 在 PR 阶段抓,不是上线后
- Tauri 官方推荐 Vitest + Playwright

**Cons**:
- Vanilla JS + Tauri invoke,Vitest mock 略复杂
- Playwright 需 Tauri dev server,CI 加 macOS runner 成本高 (或 Linux + xvfb)
- 加进项目后维护成本

**Context**: agent-hub 是 Tauri 2.x 桌面应用,vanilla JS + tailwind v4。后端走 `cargo test`,几个模块已覆盖 (`session/`, `mcp/parser`, `trash`)。Sprint 1 加 22 个 backend 单测 — 差距在 frontend。

**Depends on / blocked by**: 独立。不阻 Sprint 1。首批测试覆盖 monitor tab UX (复用本次 manual checklist 转代码)。

**Estimate**: 2-3 天 (含 framework setup + 首批测试)。

---

## Smaller Deferred Items (Sprint 1 review 发现,放这里以免遗忘)

- **JSONL parsing version detection**: agent 升级后 JSONL 格式变化的 graceful fallback。当前是 `serde_json::from_str` Result silently skip,看不到 schema drift。建议: adapter 解析失败 N 次后 emit `data_limited_reason: "schema-changed"`,前端显示「Kiro 数据格式可能已升级,部分信息暂不可读」。Sprint 2 见反馈再写。
- **Claude Code session_id JSONL UUID fallback**: `claude-{pid}` 在没有 `--resume` 的新会话时使用 (adapters.rs:443-444)。可改为从 JSONL 文件名提取 UUID,跨重启稳定。Sprint 2 follow-up,不阻塞。
- **Multi-turn title 刷新策略**: open question #5 (title 显示用户单条还是整段最后一次提问)。先按「最近用户消息」,后续看反馈再调。
- **4 agent 同时完成的全局通知节流**: open question #4。先 per-session cooldown,后续看用户反馈是否需要 global rate-limit。
- **Subtitle 抗抖动多 tool 聚合**: open question #3。先「最后一个 tool · Ys」,后续看是否要 1 秒窗口聚合显示「调用 3 个工具」。

---

## Codex Cross-Model Review 提出的 P2/P3 follow-ups

(P0/P1 进 Sprint 1,见设计文档 Outside Voice 章节)

### P2 — Sprint 2 / 长期

**T3: Windows feasibility 验证**
- **What**: 在 Windows 上验证 sysinfo 进程访问 (cwd/exe/cmd 字段) / notify file watcher 行为 / 路径处理 / 通知 plugin 是否一致
- **Why**: 设计假设 macOS 行为,Windows 上可能 sysinfo 拿不到 cwd / FSEvents 等价物的事件粒度不同 / Windows 通知 toast 和 macOS 行为差异
- **Pros**: agent-hub 自称 desktop app 跨平台。如果 Windows 实际不工作要么修要么明示「Windows 仅 partial 支持」
- **Cons**: 需要 Windows 测试环境 + 实际跑 4 个 agent 在 Windows 上 (其中 Kiro/Codex/Gemini 在 Windows 上是否常见?)
- **Estimate**: 1-2 天 (含修复)

**T4: `refresh_processes_specifics` 字段验证**
- **What**: 验证 `ProcessRefreshKind::new()` 是否包含 cwd/cmd/exe 字段,如不包含改用 `ProcessRefreshKind::everything()` 或显式 enable 所需字段
- **Why**: Perf #1 改为 refresh-based 后,如果 RefreshKind 配置错,Codex/Gemini 检测会因为字段缺失静默失效
- **Estimate**: 30 分钟 (Sprint 1 实施 Perf #1 时如果发现就 inline 修)

**T5: Frontend attention semantics**
- **What**: row 状态变化 (working→finished) 时的视觉提示 — blink / halo glow / 角标 / 声音
- **Why**: row 不动只 subtitle 变,用户会 miss completion。但加 blink/shake 之前需要先有 unread 规则避免噪音
- **Depends on**: P1 finished→idle 归档语义定义 (Sprint 1 完成)
- **Estimate**: 0.5 天

**T6: Last assistant reply 显示规则**
- **What**: subtitle 显示规则 — tool-only turn (无 text block) / 结构化输出 (JSON / table) / 代码块 / 大响应 (>1KB) / 二进制 / 错误信息
- **Why**: 当前假设「最后 assistant reply 是 displayable text」,边界 case 没规则
- **Pros**: subtitle 在所有 case 都有合理 fallback (例: tool-only 显示「✓ 完成 N 次工具调用」)
- **Estimate**: 0.5-1 天 (含 Kiro / Claude Code 双 format)

### P3 — 实施反思

**T7: 监控并行化效果**
- **What**: Sprint 1 实施时跟踪 S1/S2/S6 并行 worktree 是否产生实际 merge 冲突。如果产生,退为顺序合并并记录经验
- **Why**: Codex 警告并行化 overconfident,但具体冲突在没实际跑过之前不确定
- **Estimate**: 实施期记录,无额外成本

**T8: S8 (debounce) 排序**
- **What**: 实施时把 debounce + state caching (S8) 提到 S6 之后、S4/S5 之前。原计划放最后会被迫返工
- **Why**: debounce 影响 adapter 怎么观察 fs event,后加会冲突现有 stateful adapter 逻辑
- **Estimate**: 调整执行顺序,实施期 0 成本

---

## /plan-design-review 提出的设计 TODOs (Sprint 2)

(Pass 1-7 决策已进 Sprint 1,见设计文档 /plan-design-review 章节)

### TD1: theme.css 重构清理 `!important`

**What**: 现有 778 行 theme.css 含大量 `!important` declarations 跟 Tailwind utility classes 冲突。Monitor tab 实施时本 sprint 自纪律 (新 CSS 写到专用 class 而非 inline Tailwind),但全局问题未解。

**Why** (audit FINDING-009): HTML 用的 Tailwind class 是「假信息」(被 theme.css 覆盖),新组件要么用「正确」class 被覆盖要么加更多 `!important` 雪球滚下去。

**Pros**: 一次清理后,前端 CSS 维护成本下降,新组件不用打 `!important` 仗,Tailwind class 在 HTML 里读到的就是实际效果。

**Cons**: 涉及 ~778 行 CSS 重构 + 全部 .js render 函数检查,工作量 3-5 天。需要回归测试整个 UI。

**Context**: 现 architecture: index.html 用 Tailwind utility,theme.css 全 `!important` 覆盖 → 实际样式来自 theme.css。建议: HTML 用 semantic CSS class,theme.css 定义 token + 组件样式,Tailwind 仅用于 layout utilities (flex/grid/padding)。

**Depends on / blocked by**: 独立。Monitor tab Sprint 1 不阻塞,但建议在 Sprint 2 之前。

**Estimate**: 3-5 天 (含回归测试)。

### TD2: Sessions tab 视觉与 Monitor tab 对齐

**What**: Sprint 1 把 Monitor tab 升级为 WhatsApp 范式 (Q主A辅 / 相对时长 / 状态点 / agent icon)。Sessions tab 还是旧 list 风格 — 风格不一致。

**Why**: 用户在 Monitor tab 习惯了新范式,切到 Sessions tab 后视觉断层。一致性是产品 polish 的基础。

**Pros**: 整体视觉一致,降低用户心智切换成本。Sessions tab 复用 Monitor tab 的 agent icon / 时间格式 / Q主A辅 layout。

**Cons**: Sessions tab 是历史会话查看,信息组织跟实时 monitor 不完全一样 (没有 working state / 通知 cooldown 等),不能完全照搬。

**Context**: src/js/app.js:2078 renderSessionsView 是 Sessions tab 入口。当前用旧的 list-of-sessions 风格。

**Estimate**: 1-2 天。

### TD3: Agent type icon 资源决定

**What**: Sprint 1 Pass 4 决定保留 agent icon (Kiro 波浪 / Claude 书 / Codex 双箭头 / Gemini 星),实施时需要 4 个 SVG line icon 资源。

**Why**: 单色线 icon 一致性是「serious dev tool 不是 SaaS slop」的关键。如果 4 个 icon 风格不统一 (来源不同 icon set) 视觉会破。

**Pros**: 提前定 (Lucide / Phosphor / Heroicons / 自己画) 避免 Sprint 1 实施时 ad-hoc 选导致风格不统一。

**Cons**: 选择多,Lucide 没 kiro 这种品牌 icon。可能需要混 (Lucide 通用 + 自画品牌)。

**Context**: agent-hub 现没用图标库 (检查 package.json)。Lucide 是开源最广 + 一致性好的 line icon set。

**Estimate**: 1-2 小时 (选 + 拷贝 4 个 SVG 进 src/icons/)。Sprint 1 实施时建议第一步做。

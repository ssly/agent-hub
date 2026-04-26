# Agent Hub — 产品需求文档 (PRD)

## 1. 背景与动机

本地运行着多个 AI Agent 平台（Claude Code、Codex CLI、Cursor、OpenClaw、Hermes、Trae、Gemini 等），每个平台各有自己的 skill/plugin/MCP 生态。当前痛点：

1. **碎片化**：skill 散落在各平台目录中，无法一目了然地看到「我有哪些 agent，每个 agent 有哪些 skill」
2. **重复劳动**：写好一个 skill 想在多个平台用，只能手动复制或用软链接，但软链接不灵活（无法选择性地同步部分 skill）
3. **版本分裂**：同一 skill 在不同平台有不同版本，无法快速发现差异、对齐内容
4. **缺乏治理**：不知道哪个 skill 只在单平台有、哪些已经过时

## 2. 产品定位

Agent Hub 是一个 **终端工具（TUI）**，作为本地多 Agent 平台的管理中枢，提供 skill 的统一视图、对比和同步能力。

```
┌─────────────────────────────────────────────────┐
│                  Agent Hub                       │
│  统一管理 Claude / Codex / Cursor / OpenClaw /   │
│  Hermes / Trae / Gemini 的 Skill 生态            │
└─────────────────────────────────────────────────┘
```

## 3. 目标用户

在本地同时使用 2 个以上 AI Agent 平台的开发者。

## 4. 核心概念

| 概念 | 定义 |
|------|------|
| **Platform** | 一个 AI Agent 平台（如 Claude Code、Codex CLI） |
| **Skill** | 一个技能单元，由 `SKILL.md`（YAML frontmatter + Markdown body）定义，可能包含 `references/`、`scripts/` 等子目录 |
| **Sync** | 将一个 Platform 的 skill 复制到另一个 Platform |
| **Diff** | 比较同一 skill 在不同 Platform 之间的内容差异 |

## 5. 功能需求

### 5.1 P0 — 第一版必须实现

#### F1: 自动发现 Platform

- 扫描预定义的候选路径，自动检测已安装的 Platform
- 支持的 Platform 及其 skill 路径：

| Platform | 候选 Skill 路径 |
|----------|----------------|
| Claude Code | `~/.claude/skills/` |
| Codex CLI | `~/.codex/skills/` |
| Cursor | `~/.cursor/skills-cursor/` |
| OpenClaw | `~/.openclaw/skills/` |
| Hermes | `~/.hermes/skills/` |
| Trae | `~/.trae/skills/` |
| Shared Pool | `~/.agents/skills/` |

- 检测逻辑：目录存在即视为已安装
- 支持手动添加自定义 Platform 路径（通过配置文件）

#### F2: Skill 列表总览

- 展示所有已发现的 Platform 及其 skill 数量
- 每行显示：Platform 名称、skill 总数、路径
- 支持展开查看某个 Platform 下的所有 skill 列表
- Skill 列表展示：名称、版本（如有）、描述（截断显示）、是否为软链接

#### F3: Skill 详情查看

- 选中 skill 后展示完整的 SKILL.md 内容
- 显示元数据：名称、版本、描述、依赖、文件大小、最后修改时间
- 列出 skill 目录下的所有文件（references/、scripts/ 等）

#### F4: 跨 Platform Diff

- 选择一个 skill 后，展示哪些 Platform 有同名 skill
- 两两对比 diff（使用类似 `git diff` 的统一格式）
- 标注差异行数统计（新增 N 行、删除 N 行、修改 N 行）

#### F5: 国际化 (i18n)

- 支持语言：简体中文（zh-CN）、英语（en）
- **语言检测优先级**：
  1. 配置文件中明确指定的语言（最高优先级）
  2. 操作系统语言环境（读取 `LANG` / `LC_ALL` 环境变量，匹配 `zh_CN` / `zh` → 中文，其他 → 英语）
  3. 无法识别时默认英语（en）
- 用户可在配置文件中手动设置语言，也可在 TUI 中通过快捷键临时切换
- UI 文案全部走 i18n，包括：菜单标题、按钮文字、提示信息、错误信息、帮助文本
- **不做国际化的内容**：skill 自身的名称和描述（这些由 skill 作者决定）、文件路径、diff 内容

#### F6: Skill 同步

- 将 skill 从 Platform A 复制到 Platform B
- **冲突处理**（核心场景）：
  - 目标 Platform 不存在该 skill → 直接复制
  - 目标 Platform 已存在该 skill → 展示 diff，用户三选一：
    1. **使用源覆盖目标**（source → target）
    2. **保留目标不变**（keep target）
    3. **取消操作**
- 同步范围：整个 skill 目录（包括 SKILL.md + 子目录）
- 同步完成后刷新视图

### 5.2 P1 — 第二版

- Plugin 管理（发现、对比、同步）
- MCP Server 配置管理
- Skill 批量同步（一次选择多个 skill 同步到目标 Platform）
- Skill 搜索（按关键词搜索所有 Platform 的 skill）

### 5.3 P2 — 未来

- Hook 管理视图
- Platform 配置编辑
- Skill 模板市场（从远端拉取 skill）
- 变更监听（文件系统 watcher，自动刷新）

## 6. 用户流程

### 流程 1：查看所有 Agent 的 Skill

```
启动 Agent Hub
  → 自动扫描已安装 Platform
  → 展示 Platform 列表（名称 + skill 数量）
  → 选中某 Platform → 展示该 Platform 的 skill 列表
  → 选中某 skill → 展示 skill 详情
```

### 流程 2：同步 Skill 到另一个 Agent

```
在 skill 列表或详情页 → 按 [s] 触发同步
  → 选择目标 Platform
  → 检测目标是否已存在同名 skill
    → 不存在：确认后直接复制
    → 已存在：展示 diff 对比
      → 用户选择：源覆盖 / 保留目标 / 取消
  → 执行复制 → 刷新视图
```

### 流程 3：对比同一个 Skill 的差异

```
在 Platform 列表 → 选择「跨 Platform 对比」
  → 输入 skill 名称
  → 展示拥有该 skill 的所有 Platform
  → 选择两个 Platform 进行 diff
  → 展示统一 diff 格式的对比结果
```

## 7. 非功能需求

| 维度 | 要求 |
|------|------|
| 性能 | 启动时间 < 500ms（skill < 1000 个时） |
| 平台 | macOS（Linux 可选） |
| 交互 | 纯键盘操作，符合 TUI 常规快捷键（vim 风格） |
| 安全 | 只读操作无风险；同步操作需用户二次确认 |
| 国际化 | 支持中文简体和英语，自动检测系统语言，可配置覆盖 |
| 可扩展 | Platform 定义可配置，方便未来新增 Platform |

## 8. 技术约束

- 语言：Rust
- TUI 框架：ratatui
- 不使用外部数据库，直接读取文件系统
- 配置文件：TOML 格式（`~/.agent-hub/config.toml`）

## 9. 成功指标

1. 能在一个界面看到所有 Platform 及其 skill
2. 能在 3 步内完成一个 skill 的跨 Platform 同步
3. Diff 结果准确（与 `diff -u` 输出一致）

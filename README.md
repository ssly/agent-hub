# Agent Hub

统一管理本地多个 AI Agent 平台（Claude Code、Codex CLI、Cursor、Gemini、Kiro 等）的 Skill、MCP Server、会话历史和账号配置的桌面工具。

## 功能

### Skill 管理

- **平台总览** — 自动发现已安装的 Agent 平台，展示各平台 Skill 数量和目录
- **Skill 详情** — 元数据（名称、版本、描述、文件列表）查看，在线预览任意文件内容
- **跨平台 Diff** — 选择两个平台对比同一 Skill 差异，行级增删标注（Myers 算法）
- **一键同步** — Skill 从平台 A 同步到平台 B，目标已存在时展示差异供决策
- **文件夹分组** — 嵌套子文件夹支持，按层级分组，可整文件夹批量同步
- **无效 Skill 扫描** — 检测缺失 SKILL.md、frontmatter 异常、内容为空等问题 Skill
- **全局搜索** — 跨平台搜索 Skill 名称和描述
- **回收站** — 删除的 Skill 保留 7 天，支持恢复或永久删除
- **符号链接** — 自动识别并透明处理符号链接

### MCP Server 管理

- 支持 Claude Code、Cursor、Gemini、Kiro、Codex CLI 五个平台的 MCP 配置
- 手风琴式展开编辑，JSON/TOML 格式自动转换
- 跨平台同步 MCP Server 配置
- 粘贴导入 MCP 配置

### 会话浏览器

- 浏览 Claude Code、Codex CLI、Kiro 的历史会话
- 按项目路径过滤，分页浏览
- 查看完整会话消息（对话历史）
- 在终端中恢复会话（支持 Warp、iTerm、Ghostty、macOS Terminal；CMD、Windows Terminal）

### 账号切换

- 保存和切换 Claude Code、Codex CLI 的认证配置
- SHA-256 哈希比对自动检测当前活跃账号
- 配置内容编辑、备注管理
- 一键清除当前认证（自动备份）

### 其他

- **自动更新** — 断点续传下载，minisign 签名验证
- **中英双语** — 自动检测系统语言，UI 内即时切换
- **明暗主题** — 墨光（Light）和墨夜（Dark）两套主题

## 安装

### macOS

下载 `.dmg` 打开，将应用拖入 Applications 文件夹：

- **有管理员权限**：拖到 `/Applications`
- **无管理员权限**：拖到 `~/Applications`

首次打开若被 Gatekeeper 拦截：

```bash
xattr -cr /Applications/"Agent Hub.app"
```

### Windows

下载 `.exe` 安装包运行。如遇 SmartScreen 拦截，点击"更多信息" → "仍要运行"。

## 技术栈

| 层 | 技术 |
|----|------|
| 后端 | Rust + Tauri 2.x |
| 前端 | Vanilla JS + TailwindCSS v4 |
| Diff | similar（Myers diff algorithm）|
| 数据库 | SQLite（rusqlite，bundled）|
| 文件监听 | notify |
| 进程信息 | sysinfo |
| HTTP | reqwest（rustls-tls）|

## 开发

```bash
npm install                # 安装 TailwindCSS CLI
npm run build:css          # 构建 CSS（首次或改样式后）
cargo tauri dev            # 启动开发模式（热重载）
```

开发模式下：
- 修改 `src/js/*.js` 或 `src/index.html` 会自动刷新窗口
- 修改 `src-tauri/src/*.rs` 会自动重编译
- 另开终端 `npm run dev:css` 监听 CSS 变更

## 打包

```bash
cargo tauri build          # Release
cargo tauri build --debug  # Debug（更快）
```

macOS 产物在 `src-tauri/target/release/bundle/`（`.app`、`.dmg`）。
Windows 产物在 `src-tauri/target/release/bundle/`（`.exe`、`.msi`、`.nsis`）。

## 配置

配置文件：`~/.agent-hub/config.toml`（首次运行自动创建）

```toml
[general]
language = "auto"    # "auto" | "zh-CN" | "en"

[[platforms]]
id = "my-agent"
display_name = "My Custom Agent"
skill_dir = "~/.my-agent/skills"
```

## 支持的平台

### Skill 管理

| 平台 | 目录 |
|------|------|
| Claude Code | `~/.claude/skills/` |
| Codex CLI | `~/.codex/skills/` |
| Cursor | `~/.cursor/skills-cursor/` |
| Gemini | `~/.gemini/skills/` |
| OpenClaw | `~/.openclaw/skills/` |
| Hermes | `~/.hermes/skills/` |
| Trae | `~/.trae/skills/` |
| Kiro | `~/.kiro/skills/` |
| Shared Pool | `~/.agents/skills/` |

### MCP Server 管理

| 平台 | 配置路径 | 格式 |
|------|---------|------|
| Claude Code | `~/.claude.json` | JSON |
| Cursor | `~/.cursor/mcp.json` | JSON |
| Gemini | `~/.gemini/settings.json` | JSON |
| Kiro | `~/.kiro/settings/mcp.json` | JSON |
| Codex CLI | `~/.codex/config.toml` | TOML |

## License

MIT

# Agent Hub（枢纽）

一个桌面应用，统一管理本地多个 AI Agent 平台（Claude Code、Codex CLI、Cursor、OpenClaw、Hermes、Trae 等）的 Skill 生态。

## 功能

- **平台总览** — 自动发现已安装的 Agent 平台，一目了然查看各平台的 Skill 数量和目录位置
- **Skill 详情** — 查看 Skill 元数据（名称、版本、描述、文件列表），点击任意文件在线预览内容
- **跨平台 Diff** — 选择两个平台对比同一个 Skill 的差异，精确到行级别的增删标注
- **一键同步** — 将 Skill 从平台 A 同步到平台 B，目标已存在时展示差异供你决定覆盖或保留
- **全局搜索** — 跨平台搜索 Skill 名称和描述
- **中英双语** — 自动检测系统语言，支持即时切换

## 技术栈

- **后端**: Rust + Tauri 2.x
- **前端**: Vanilla JS + TailwindCSS v4
- **Diff 引擎**: similar (Myers diff algorithm)

## 开发

```bash
# 安装前端依赖
npm install

# 构建 CSS（首次或修改样式后）
npm run build:css

# 启动开发模式（热重载）
cargo tauri dev
```

开发模式下：
- 修改 `src/js/*.js` 或 `src/index.html` 会自动刷新窗口
- 修改 `src-tauri/src/*.rs` 会自动重编译
- 另开终端运行 `npm run dev:css` 可监听 CSS 变更

## 打包

### macOS

```bash
# Release 版本（正式发布）
cargo tauri build

# Debug 版本（快速测试）
cargo tauri build --debug
```

产物位于 `src-tauri/target/release/bundle/`：

| 格式 | 路径 |
|------|------|
| .app | `macos/Agent Hub.app` |
| .dmg | `dmg/Agent Hub_0.1.0_aarch64.dmg` |

Debug 版本将 `release` 替换为 `debug`。

### Windows

```powershell
# 前置：安装 Visual Studio Build Tools 和 WebView2
# https://visualstudio.microsoft.com/visual-cpp-build-tools/
# WebView2 已内置于 Windows 11，Windows 10 需手动安装

# 安装前端依赖
npm install

# 构建 CSS
npm run build:css

# Release 版本
cargo tauri build

# Debug 版本
cargo tauri build --debug
```

产物位于 `src-tauri/target/release/bundle/`：

| 格式 | 路径 |
|------|------|
| .exe | `../agent-hub.exe` |
| .msi | `msi/Agent Hub_0.1.0_x64_en-US.msi` |
| .nsis | `nsis/Agent Hub_0.1.0_x64-setup.exe` |

## 配置

配置文件位于 `~/.agent-hub/config.toml`（首次运行自动创建）：

```toml
[general]
# 语言设置: "auto" | "zh-CN" | "en"
language = "auto"

# 自定义平台（在预定义之外追加）
[[platforms]]
id = "my-agent"
display_name = "My Custom Agent"
skill_dir = "~/.my-agent/skills"
```

## 支持的平台

| 平台 | Skill 目录 |
|------|-----------|
| Claude Code | `~/.claude/skills/` |
| Codex CLI | `~/.codex/skills/` |
| Cursor | `~/.cursor/skills-cursor/` |
| OpenClaw | `~/.openclaw/skills/` |
| Hermes | `~/.hermes/skills/` |
| Trae | `~/.trae/skills/` |
| Shared Pool | `~/.agents/skills/` |

## License

MIT

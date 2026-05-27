# CLAUDE.md

Agent Hub 项目开发指南。

## 项目概况

Tauri 2.x 桌面应用（Rust 后端 + Vanilla JS 前端），统一管理本地 AI Agent 平台的 Skill、MCP Server、会话和账号。版本 0.9.0。

## 技术约束

- 前端无打包工具，HTML 直接加载 JS（ES Module）
- CSS 由 TailwindCSS v4 CLI 生成：`npm run build:css`
- 所有前端逻辑在 `src/js/app.js` 的 `App` 类中
- 后端命令在 `src-tauri/src/commands.rs` 注册
- API 调用统一通过 `src/js/api.js` 封装

## 开发流程

```bash
npm install && npm run build:css   # 首次
cargo tauri dev                     # 开发模式
```

- 改 `.rs` → 自动重编译
- 改 `.js`/`.html` → 自动刷新窗口
- 改 CSS → 需运行 `npm run dev:css` 或手动 `npm run build:css`

## 代码风格

- Rust：标准 Rust 风格，模块按功能拆分
- JS：类方法，模板字符串拼接 HTML，无框架
- 国际化：UI 文字使用 `i18n.t('key')`，翻译文件在 `src/locales/*.json` 和 `locales/*.toml`
- 新增后端命令：在 `commands.rs` 添加 `#[tauri::command]` 函数，在 `lib.rs` 注册
- 新增前端功能：在 `app.js` 的 `App` 类中添加方法，通过 `api.js` 调用后端

## 关键文件

| 文件 | 作用 |
|------|------|
| `src/js/app.js` | 前端所有 UI 逻辑 |
| `src/js/api.js` | Tauri IPC 调用封装 |
| `src-tauri/src/commands.rs` | 后端所有命令处理 |
| `src-tauri/src/lib.rs` | 应用初始化和命令注册 |
| `src-tauri/src/platform/registry.rs` | 平台定义 |
| `src-tauri/src/mcp/parser.rs` | MCP 配置解析 |

## 版本管理

- 版本号在 `src-tauri/tauri.conf.json` 和 `Cargo.toml` 中同步
- 使用 `npm run version` 从 git tag 读取并写入
- 推送 `v*` tag 触发 CI 构建

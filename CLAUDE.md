# CLAUDE.md

Agent Hub 项目开发指南。

## 项目概况

Tauri 2.x 桌面应用（Rust 后端 + Vue 3 前端），统一管理本地 AI Agent 平台的 Skill、MCP Server、会话和账号。

## 技术约束

- 前端 Vue 3 + Vite + Pinia + TailwindCSS v4（`@tailwindcss/vite` 插件，无 CLI）
- 入口 `index.html` → `src/main.ts` → `src/App.vue`
- 状态用 Pinia store（`src/stores/*`），Tauri 调用封装在 `src/lib/api.ts`
- 浏览器调试模式（`npm run dev:web`）走 `src/lib/mock-api.ts` mock 数据
- 后端命令在各模块的 `commands.rs` 实现，统一在 `src-tauri/src/lib.rs` 注册

## 开发流程

```bash
npm install                # 首次
cargo tauri dev            # 开发模式（自动启动 Vite）
npm run dev:web            # 仅前端 + mock 数据（无 Tauri）
```

- 改 `.rs` → 自动重编译
- 改 `.vue`/`.ts` → Vite HMR 热更新

## 代码风格

- Rust：标准 Rust 风格，模块按功能拆分
- Vue：`<script setup lang="ts">` 单文件组件，Pinia store 管理状态
- 国际化：UI 文字使用 vue-i18n `t('key')`，翻译在 `src/locales/*.json`
- 新增后端命令：添加 `#[tauri::command]` 函数，在 `lib.rs` 注册
- 新增前端功能：在 `src/lib/api.ts`（+ `mock-api.ts`）加调用，在对应 store + 组件实现

## 关键文件

| 文件 | 作用 |
|------|------|
| `src/main.ts` | Vue 应用挂载（Pinia + vue-i18n） |
| `src/App.vue` | 根组件（布局 + 视图切换） |
| `src/lib/api.ts` | Tauri IPC 调用封装 |
| `src/stores/*` | Pinia 状态管理 |
| `src-tauri/src/lib.rs` | 应用初始化和命令注册 |
| `src-tauri/src/platform/registry.rs` | 平台定义 |
| `src-tauri/src/mcp/parser.rs` | MCP 配置解析 |

## 版本管理

- 版本号在 `src-tauri/tauri.conf.json` 和 `Cargo.toml` 中同步
- 使用 `npm run version` 从 git tag 读取并写入
- 推送 `v*` tag 触发 CI 构建

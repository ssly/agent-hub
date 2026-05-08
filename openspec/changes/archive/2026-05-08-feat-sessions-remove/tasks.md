## 1. 后端删除命令与平台适配

- [x] 1.1 在 `src-tauri/src/commands.rs` 新增 Tauri 命令 `delete_session(platform_id, session_id)`，并完成命令注册。
- [x] 1.2 在 `src-tauri/src/session/mod.rs` 增加 `session::delete_session` 入口，完成平台路由与错误归一化。
- [x] 1.3 实现 Claude 会话删除：解析会话文件并安全移除对应会话产物。
- [x] 1.4 实现 Codex 会话删除：采用归档语义（`archived = 1`），使已删除会话从列表中过滤。
- [x] 1.5 实现 Kiro 会话删除：移除列表/消息读取所依赖的会话产物。

## 2. 前端 API 与 Sessions 交互

- [x] 2.1 在 `src/js/api.js` 新增 `deleteSession(platformId, sessionId)` API 封装。
- [x] 2.2 在 `src/js/app.js` 的 Sessions 列表中新增删除按钮 UI，并提供本地化标签与逐项状态。
- [x] 2.3 实现会话删除的二次点击确认行为（确认态 + 超时重置），与现有破坏性操作模式一致。
- [x] 2.4 在确认删除后调用后端接口，展示成功/失败反馈，并在删除进行中防止重复提交。
- [x] 2.5 删除成功后刷新会话数据，保持稳定的分页/列表状态。

## 3. 本地化与验证

- [x] 3.1 在 `src/locales/en.json` 与 `src/locales/zh-CN.json` 增加会话删除/确认/成功/失败文案键值。
- [x] 3.2 为后端删除路由与平台特定删除行为补充/扩展测试（包含失败路径）。
- [x] 3.3 在 Sessions 标签页执行 Claude Code、Codex CLI、Kiro 三平台的手工端到端删除验证。
- [x] 3.4 验证删除改动后，会话列表、加载更多分页、消息查看与恢复操作无回归。

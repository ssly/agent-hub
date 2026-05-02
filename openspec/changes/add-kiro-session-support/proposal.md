## 背景与动机

Agent Hub 的 Sessions 目前只覆盖 Claude Code 和 Codex CLI，本地已经有 Kiro 使用历史但无法统一查看与恢复。现在补上 Kiro 会话支持，可以让多平台会话管理能力完整落地，减少跨工具切换成本。

## 变更内容

- 新增 Kiro 平台的会话发现能力：识别本机 Kiro 会话数据源并纳入 Sessions 平台列表。
- 新增 Kiro 会话分页列表能力：提供会话标题、时间、模型、路径、消息数等摘要信息。
- 新增 Kiro 会话消息读取能力：支持按 offset/limit 分页拉取消息内容并映射为统一消息模型。
- 新增 Kiro 会话恢复入口能力：生成并下发 Kiro 的恢复命令，接入现有终端恢复流程。
- 增强异常与兼容处理：当 Kiro 安装存在但无会话、索引损坏或字段缺失时，返回可恢复错误并保持 UI 可用。

## 能力定义

### 新增能力
- `kiro-sessions`: 统一定义 Kiro 会话的发现、列表、详情消息读取与恢复行为契约。

### 修改能力
- （无）

## 影响范围

- 受影响代码：
  - `src-tauri/src/session/mod.rs`（新增 `kiro` 分发）
  - `src-tauri/src/session/kiro.rs`（新增 Kiro 会话解析实现）
  - `src-tauri/src/commands.rs`（沿用现有会话命令，必要时补充错误映射）
  - `src/js/app.js`（前端无需协议变更，仅消费新增平台数据）
- API：
  - 复用现有 `list_session_platforms` / `list_sessions` / `get_session_messages` / `resume_session`。
- 系统层面：
  - 读取本地 Kiro 会话存储（以用户主目录下 Kiro 数据目录为准）。
- 风险：
  - Kiro 本地存储结构版本差异导致字段缺失；需做容错与回退策略。

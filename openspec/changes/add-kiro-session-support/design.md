## 背景

Agent Hub 的 Sessions 能力当前仅覆盖 Claude Code 与 Codex CLI，后端分为平台发现（`list_session_platforms`）、会话列表（`list_sessions`）、消息分页读取（`get_session_messages`）与恢复会话（`resume_session`）四个统一入口。Kiro 已在本机安装并产生真实会话，但暂未接入该统一模型。

本机验证到的 Kiro 数据形态：
- 会话元数据目录：`~/.kiro/sessions/cli/*.json`
- 会话消息日志：`~/.kiro/sessions/cli/*.jsonl`
- 元数据字段：`session_id`, `cwd`, `created_at`, `updated_at`, `title`, `session_state.rts_model_state.model_info.model_id`
- 消息日志字段：`kind: Prompt|AssistantMessage`, `data.content[].{kind:"text",data:"..."}`
- Kiro CLI 恢复命令：`kiro-cli chat --resume-id <session-id>`

约束：
- 本地存储格式不是稳定公共 API，需按“尽量读取、安全回退”的策略容错。
- Sessions 现有前端协议不变，新增平台必须复用统一 `SessionSummary` / `SessionMessage` 模型。

## 目标 / 非目标

**目标：**
- 在不改前端协议的前提下接入 Kiro 会话平台。
- 支持 Kiro 会话平台发现、分页列表、消息分页读取、恢复命令生成。
- 当 Kiro 目录存在但个别会话文件损坏时，仍返回可用结果而不是整体失败。
- 维持与现有 Claude/Codex 同级别的分页与性能表现（流式读取 JSONL）。

**非目标：**
- 不实现 Kiro 会话删除、重命名、归档。
- 不实现跨目录的 `kiro-cli chat --resume` 语义模拟（仅支持明确 `--resume-id`）。
- 不引入新的数据库或缓存层。

## 关键决策

### 1) 引入独立 `kiro.rs` 解析器模块，而非混入现有平台逻辑
- **决策**：新增 `src-tauri/src/session/kiro.rs`，并在 `session/mod.rs` 做分发。
- **原因**：与 `claude.rs` / `codex.rs` 一致，边界清晰，便于后续新增平台。
- **备选方案**：在 `mod.rs` 直接实现 Kiro 解析。
- **未采纳原因**：会让调度层与平台实现耦合，后续维护困难。

### 2) 以 `~/.kiro/sessions/cli/*.json` 作为列表主数据源，`*.jsonl` 作为消息源
- **决策**：列表读取 `.json`，消息读取 `.jsonl`。
- **原因**：`.json` 有稳定会话摘要字段（title/cwd/time/model），无需从大日志回推；`.jsonl` 更适合增量分页读取消息。
- **备选方案**：仅扫描 `.jsonl` 推导摘要。
- **未采纳原因**：字段完整性差，且解析成本更高。

### 3) Kiro 消息角色映射采用固定规则
- **决策**：
  - `kind == "Prompt"` 映射 `role=user`
  - `kind == "AssistantMessage"` 映射 `role=assistant`
  - 仅拼接 `data.content[]` 中 `kind == "text"` 的文本段
- **原因**：与实际样本一致，可复用现有 `SessionMessage`。
- **备选方案**：保留多模态分段结构。
- **未采纳原因**：需要变更前端协议，超出本次范围。

### 4) 恢复命令统一使用 `kiro-cli chat --resume-id`
- **决策**：在 `resume_session` 增加 `kiro` 分支，命令为 `kiro-cli chat --resume-id <id>`。
- **原因**：经本机命令帮助验证，该参数是精确恢复目标会话的稳定入口。
- **备选方案**：`--resume`。
- **未采纳原因**：依赖当前目录最近会话，无法保证与选中的 session 一致。

### 5) 容错策略：跳过坏文件、保留好文件
- **决策**：单文件解析失败时跳过该会话并继续；仅在根目录不可读时返回错误。
- **原因**：用户价值是“尽可能看到可用会话”，不应因单个坏文件导致整个平台不可用。
- **备选方案**：任何解析错误即 fail-fast。
- **未采纳原因**：会显著损伤可用性。

## 风险 / 取舍

- **[Kiro 文件结构版本变更]** → 通过多字段回退（title/session_id、updated_at/file mtime、model 可空）降低耦合。
- **[大会话 JSONL 读取耗时]** → 维持 offset/limit + `BufReader::lines()` 流式读取，避免整文件加载。
- **[会话目录分层变化（如 v2 子目录）]** → 首版固定 `~/.kiro/sessions/cli`，后续可扩展目录探测策略。
- **[用户机器无 `kiro-cli` 命令但有会话文件]** → 列表照常展示；恢复时返回明确错误信息。

## 迁移计划

1. 新增 `session/kiro.rs` 并在 `session/mod.rs` 注册 `count/list/get_messages` 分支。
2. 在 `resume_session` 添加 `kiro` 恢复命令分支。
3. 增加单元测试（解析与分页）与真实数据 smoke test（有数据时执行）。
4. 本地 `cargo test` 与 `cargo check` 验证。
5. 发布后若出现结构差异，优先通过字段回退修复，不改 API。

回滚策略：
- 如线上出现严重兼容问题，移除 `kiro` 平台分支即可回退，不影响 Claude/Codex。

## 待确认问题

- Kiro 是否会在未来切换会话根目录（如迁移到 `~/Library/Application Support/kiro-cli`）并废弃 `~/.kiro/sessions/cli`？
- 是否需要在 UI 中标注 Kiro 的 `model_id=auto` 与真实底层模型不一致的语义？

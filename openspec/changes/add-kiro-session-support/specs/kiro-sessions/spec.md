## ADDED Requirements

### Requirement: Kiro 会话平台发现
系统 MUST 在 `~/.kiro/sessions/cli/` 下存在本地 Kiro 会话元数据文件时，将 Kiro 识别为可用会话平台。

#### Scenario: 本地存在 Kiro 会话
- **WHEN** 平台发现流程执行，且存在至少一个 `~/.kiro/sessions/cli/*.json` 会话元数据文件
- **THEN** 系统 SHALL 返回 id 为 `kiro`、display name 为 `Kiro` 的会话平台
- **AND** `session_count` SHALL 等于发现到的有效 Kiro 会话元数据文件数量

#### Scenario: 本地无 Kiro 会话
- **WHEN** `~/.kiro/sessions/cli/` 不存在，或不存在有效会话元数据文件
- **THEN** 系统 SHALL 在平台列表中省略 Kiro

### Requirement: Kiro 会话列表读取
系统 MUST 按 `updated_at` 倒序返回 Kiro 会话列表，并将每条会话映射为统一 `SessionSummary` 模型。

#### Scenario: 从元数据文件映射会话摘要
- **WHEN** Kiro 元数据文件包含 `session_id`、`cwd`、`created_at`、`updated_at`、`title`
- **THEN** 返回的 `SessionSummary` SHALL 映射为：
  - `id` 来源于 `session_id`
  - `project_path` 来源于 `cwd`
  - `title` 来源于 `title`（若为空则回退为 `session_id`）
  - `started_at` 来源于 `created_at` 并转换为毫秒级时间戳
  - `updated_at` 来源于 `updated_at` 并转换为毫秒级时间戳
  - `platform_id` 固定为 `kiro`

#### Scenario: 模型字段提取
- **WHEN** 元数据包含 `session_state.rts_model_state.model_info.model_id`
- **THEN** 返回的 `SessionSummary.model` SHALL 使用该值
- **AND** 字段缺失或为空时 SHALL 返回 `null`

#### Scenario: 元数据文件异常或部分损坏
- **WHEN** 某个元数据文件无法解析为有效 JSON，或缺少会话标识所需字段
- **THEN** 系统 SHALL 跳过该文件并继续处理其他会话
- **AND** 系统 SHALL NOT 因单文件失败而使整体列表接口失败

### Requirement: Kiro 会话消息分页读取
系统 MUST 从 `~/.kiro/sessions/cli/<session-id>.jsonl` 流式分页读取对话消息，并仅返回用户/助手消息。

#### Scenario: Prompt 行映射为用户消息
- **WHEN** JSONL 行的 `kind` 为 `Prompt`
- **THEN** 系统 SHALL 产出 `role=user` 的 `SessionMessage`
- **AND** `content` SHALL 为 `data.content[]` 中文本片段的拼接结果

#### Scenario: AssistantMessage 行映射为助手消息
- **WHEN** JSONL 行的 `kind` 为 `AssistantMessage`
- **THEN** 系统 SHALL 产出 `role=assistant` 的 `SessionMessage`
- **AND** `content` SHALL 为 `data.content[]` 中文本片段的拼接结果

#### Scenario: 消息分页边界
- **WHEN** 消息查询请求指定 `offset` 与 `limit`
- **THEN** 系统 SHALL 在跳过前 `offset` 条映射消息后，最多返回 `limit` 条消息
- **AND** 系统 SHALL 使用逐行流式读取，而不是将整个 JSONL 文件一次性载入内存

### Requirement: Kiro 会话恢复命令集成
系统 MUST 通过现有终端启动流程支持恢复选中的 Kiro 会话。

#### Scenario: 恢复指定 Kiro 会话
- **WHEN** `resume_session` 以 `platform_id=kiro` 和有效 session id 被调用
- **THEN** 系统 SHALL 生成命令 `kiro-cli chat --resume-id <session-id>`
- **AND** 若存在 `project_path`，系统 SHALL 在恢复命令前追加 `cd <project_path> &&`
- **AND** 系统 SHALL 在指定终端中启动该组合命令

#### Scenario: Kiro CLI 命令不可用
- **WHEN** 用户请求恢复 Kiro 会话但 PATH 中不存在 `kiro-cli`
- **THEN** 系统 SHALL 返回可执行的明确错误，指出 Kiro CLI 未安装或不可用

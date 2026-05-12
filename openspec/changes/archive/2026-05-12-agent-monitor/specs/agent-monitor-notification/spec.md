## ADDED Requirements

### Requirement: 输出完成通知触发

系统 SHALL 在检测到 agent 一轮输出完成时，根据用户配置决定是否发送桌面通知。

#### Scenario: 通知已启用且检测到输出完成
- **WHEN** 通知功能已启用
- **AND** 检测到某 agent 实例一轮输出完成
- **THEN** SHALL 发送桌面通知
- **THEN** 通知标题 SHALL 为 agent 名称 + 来源标签（如 "Kiro CLI 完成回复"）
- **THEN** 通知正文 SHALL 包含会话标题摘要（截断至 60 字符）

#### Scenario: 通知已关闭
- **WHEN** 通知功能已关闭
- **AND** 检测到某 agent 实例一轮输出完成
- **THEN** 不 SHALL 发送桌面通知

### Requirement: 通知冷却机制

系统 SHALL 对同一 agent 实例的通知实施冷却时间，避免频繁打扰。

#### Scenario: 冷却时间内不重复通知
- **WHEN** 已对某 agent 实例发送通知
- **AND** 该实例在冷却时间（默认 30 秒）内再次完成输出
- **THEN** 不 SHALL 发送重复通知

#### Scenario: 冷却时间过后正常通知
- **WHEN** 上次通知已超过冷却时间
- **AND** 该实例再次完成输出
- **THEN** SHALL 正常发送通知

#### Scenario: 不同实例独立冷却
- **WHEN** agent A 实例处于冷却中
- **AND** agent B 实例完成输出
- **THEN** agent B SHALL 正常发送通知（冷却互不影响）

### Requirement: 通知权限处理

系统 SHALL 正确处理操作系统通知权限的各种状态。

#### Scenario: 首次启用通知
- **WHEN** 用户首次打开通知开关
- **AND** 系统尚未授权通知权限
- **THEN** SHALL 触发系统通知权限请求弹窗
- **THEN** 若用户拒绝，SHALL 将开关恢复为关闭状态并提示用户

#### Scenario: 权限已授予
- **WHEN** 用户打开通知开关
- **AND** 系统已授权通知权限
- **THEN** SHALL 直接启用通知功能，无额外弹窗

### Requirement: 通知配置持久化

系统 SHALL 将通知相关配置持久化到 `~/.agent-hub/config.toml`。

#### Scenario: 保存通知配置
- **WHEN** 用户修改通知开关或冷却时间
- **THEN** SHALL 将配置写入 `[monitor]` 配置段
- **THEN** 下次启动时 SHALL 恢复上次的配置状态

#### Scenario: 配置文件不存在
- **WHEN** 配置文件不存在
- **THEN** SHALL 使用默认值（notification_enabled = false, notification_cooldown_secs = 30）

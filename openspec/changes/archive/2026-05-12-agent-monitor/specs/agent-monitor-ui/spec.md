## ADDED Requirements

### Requirement: Monitor Tab 入口

系统 SHALL 在主导航栏中新增 "Monitor" tab 入口，与现有的 Skills、MCP、Sessions tab 并列。

#### Scenario: 导航栏显示 Monitor tab
- **WHEN** 用户打开 Agent Hub 应用
- **THEN** 导航栏 SHALL 显示 "Monitor" tab（图标 + 文字）
- **THEN** tab 文字 SHALL 支持中英双语（中文："监控"，英文："Monitor"）

#### Scenario: Monitor tab 显示活跃数量徽标
- **WHEN** 有 agent 实例正在运行
- **THEN** Monitor tab SHALL 在图标旁显示活跃实例数量徽标（如 "3"）
- **WHEN** 无活跃实例
- **THEN** 不 SHALL 显示徽标

### Requirement: Agent 分组列表

系统 SHALL 按 agent 类型分组展示所有活跃实例，每个 agent 类型作为一个可折叠的分组。

#### Scenario: 展示 agent 分组
- **WHEN** 用户切换到 Monitor tab
- **THEN** SHALL 按以下顺序展示 agent 分组：Kiro、Claude Code、Codex、Gemini
- **THEN** 每个分组标题 SHALL 显示 agent 名称和该分组下的活跃实例数
- **THEN** 有活跃实例的分组 SHALL 默认展开
- **THEN** 无活跃实例的分组 SHALL 默认折叠并显示为灰色

#### Scenario: 未安装的 agent 不显示
- **WHEN** 某个 agent 未安装（数据目录不存在且无进程）
- **THEN** 该 agent 分组 SHALL 不显示在列表中

### Requirement: 会话实例卡片

系统 SHALL 为每个活跃的 agent 实例展示一个卡片，包含关键状态信息。

#### Scenario: 卡片基本信息展示
- **WHEN** 存在活跃的 agent 实例
- **THEN** 卡片 SHALL 显示以下信息：
  - 会话标题（用户最近一次输入的摘要，截断至 50 字符）
  - 来源标签：`CLI` 或 `Desktop`（以 badge 形式显示在卡片右上角）
  - 模型名称（如 "claude-opus-4-7"、"gpt-5.3-codex"）
  - 工作目录（显示最后两级路径，如 "code/agent-hub"）
  - 运行时长（从启动时间计算，格式如 "2h 15m" 或 "3m 20s"）
  - 当前状态指示器

#### Scenario: 状态指示器样式
- **WHEN** 会话状态为 "active"（正在生成响应）
- **THEN** SHALL 显示绿色脉冲圆点 + 文字 "运行中"
- **WHEN** 会话状态为 "idle"（等待用户输入）
- **THEN** SHALL 显示灰色圆点 + 文字 "空闲"
- **WHEN** 会话状态为 "completed"（刚完成输出）
- **THEN** SHALL 显示蓝色圆点 + 文字 "已完成"
- **WHEN** 会话状态为 "ended"（进程已退出，等待移除）
- **THEN** SHALL 显示红色圆点 + 文字 "已结束"

#### Scenario: 来源标签样式
- **WHEN** 实例来源为 CLI
- **THEN** SHALL 显示带终端图标的 `CLI` 标签（深色背景 + 浅色文字）
- **WHEN** 实例来源为 Desktop
- **THEN** SHALL 显示带窗口图标的 `Desktop` 标签（浅色背景 + 深色文字）

### Requirement: 数据受限提示

系统 SHALL 对数据受限的 agent 在其分组区域顶部显示提示条，说明监控能力的限制。

#### Scenario: Codex 数据受限提示
- **WHEN** 用户查看 Codex 分组且有活跃实例
- **THEN** SHALL 在分组顶部显示黄色提示条
- **THEN** 提示内容 SHALL 为："⚠️ Codex 活跃会话期间无法读取对话内容，仅显示运行状态和基本信息"

#### Scenario: Gemini 数据受限提示
- **WHEN** Gemini 会话被标记为数据受限
- **THEN** SHALL 在 Gemini 分组顶部显示黄色提示条
- **THEN** 提示内容 SHALL 为："⚠️ Gemini CLI 的会话数据可能在结束后才完整写入，实时状态仅供参考"

#### Scenario: 无受限时不显示提示
- **WHEN** agent 分组无数据受限标记
- **THEN** 不 SHALL 显示提示条

### Requirement: 通知开关控件

系统 SHALL 在 Monitor tab 顶部提供通知功能的开关控件。

#### Scenario: 通知开关展示
- **WHEN** 用户进入 Monitor tab
- **THEN** SHALL 在 tab 顶部工具栏显示通知开关（toggle switch）
- **THEN** 开关旁 SHALL 显示文字 "完成通知"
- **THEN** 开关默认状态 SHALL 为关闭

#### Scenario: 启用通知
- **WHEN** 用户打开通知开关
- **THEN** SHALL 调用后端 `set_monitor_config` 更新配置
- **THEN** 若系统未授权通知权限，SHALL 触发系统授权弹窗
- **THEN** 开关 SHALL 切换为开启状态

#### Scenario: 关闭通知
- **WHEN** 用户关闭通知开关
- **THEN** SHALL 调用后端 `set_monitor_config` 更新配置
- **THEN** SHALL 立即停止发送桌面通知

### Requirement: 空状态展示

系统 SHALL 在无任何活跃 agent 时展示空状态页面。

#### Scenario: 无活跃 agent
- **WHEN** 用户进入 Monitor tab 且无任何活跃 agent 实例
- **THEN** SHALL 显示空状态插图和文字
- **THEN** 文字 SHALL 为："当前没有正在运行的 Agent 实例"
- **THEN** SHALL 显示支持的 agent 列表及其检测路径说明

### Requirement: 实时更新

系统 SHALL 通过 Tauri event 接收后端推送的状态变更，实时更新 UI 而无需用户手动刷新。

#### Scenario: 新实例出现
- **WHEN** 后端推送 `monitor:state-changed` 事件，类型为新增
- **THEN** SHALL 在对应 agent 分组中动画插入新卡片
- **THEN** SHALL 更新 tab 徽标数字

#### Scenario: 实例状态变更
- **WHEN** 后端推送状态变更事件
- **THEN** SHALL 更新对应卡片的状态指示器和相关信息
- **THEN** 不 SHALL 导致页面闪烁或重排

#### Scenario: 实例消失
- **WHEN** 后端推送实例结束事件
- **THEN** SHALL 将卡片状态更新为 "已结束"
- **THEN** 30 秒后 SHALL 以淡出动画移除卡片
- **THEN** SHALL 更新 tab 徽标数字

### Requirement: 国际化支持

Monitor tab 的所有文本 SHALL 支持中英双语，遵循现有 i18n 机制。

#### Scenario: 中文环境
- **WHEN** 系统语言为中文
- **THEN** 所有 UI 文本 SHALL 显示中文（"监控"、"运行中"、"空闲"、"已完成"、"已结束"、"完成通知"等）

#### Scenario: 英文环境
- **WHEN** 系统语言为英文
- **THEN** 所有 UI 文本 SHALL 显示英文（"Monitor"、"Running"、"Idle"、"Completed"、"Ended"、"Completion Notification" 等）

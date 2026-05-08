## ADDED Requirements

### Requirement: 会话删除必须进行明确的二次确认
系统在 Sessions 标签页删除任意会话前，MUST 要求两步确认。

#### Scenario: 首次点击进入确认状态
- **WHEN** 用户首次点击会话卡片的删除操作
- **THEN** 删除控件必须进入确认状态，并明确提示需要再次点击确认

#### Scenario: 二次点击确认删除
- **WHEN** 用户在确认状态下再次点击同一会话的删除控件
- **THEN** 系统必须对该会话执行删除操作

#### Scenario: 确认状态超时
- **WHEN** 用户在设定的确认窗口内未完成二次确认
- **THEN** 删除控件必须恢复为非确认状态，且不删除会话

### Requirement: 删除成功后会话必须稳定地从 Sessions 结果中移除
删除成功后，该会话在对应平台的 Sessions 列表中 MUST 不得再次出现。

#### Scenario: 活跃列表中删除成功
- **WHEN** 删除操作返回成功
- **THEN** 后续列表结果中必须不包含被删除会话
- **AND** 必须基于刷新后的数据重新计算 `total/offset/has_more` 状态

#### Scenario: 删除发生在分页边界
- **WHEN** 删除发生在当前已加载页面的尾部附近
- **THEN** UI 必须刷新会话数据，保证可见列表有效且不重复

### Requirement: 删除失败时必须提供可执行反馈且不破坏会话浏览
若删除失败，系统 MUST 保持列表可用，并展示错误反馈。

#### Scenario: 后端删除失败
- **WHEN** 后端因文件缺失、权限不足或平台状态不支持而无法删除目标会话
- **THEN** UI 必须展示包含错误细节的失败提示
- **AND** 会话列表必须保持可用，且无需重启应用

### Requirement: 必须执行符合平台存储模型的删除语义
后端 MUST 按各平台存储模型应用对应删除行为。

#### Scenario: 删除 Claude Code 会话
- **WHEN** 删除 Claude Code 会话
- **THEN** 后端必须从 Claude 会话存储中移除对应会话产物

#### Scenario: 删除 Codex CLI 会话
- **WHEN** 删除 Codex CLI 会话
- **THEN** 后端必须将该会话标记为 archived，使其在 Sessions 列表查询中被排除

#### Scenario: 删除 Kiro 会话
- **WHEN** 删除 Kiro 会话
- **THEN** 后端必须移除会话列表所使用的对应 Kiro 会话产物

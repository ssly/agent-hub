## 1. 会话模块接线

- [x] 1.1 创建 `src-tauri/src/session/kiro.rs`，并在 `src-tauri/src/session/mod.rs` 注册模块导出
- [x] 1.2 在 `list_session_platforms()` 中新增 `kiro` 分支，按 Kiro 会话数量写入平台信息
- [x] 1.3 在 `list_sessions()` 分发逻辑中新增 `kiro` 分支，并复用现有分页上限策略
- [x] 1.4 在 `get_session_messages()` 分发逻辑中新增 `kiro` 分支

## 2. Kiro 会话发现与列表能力

- [x] 2.1 实现 Kiro 会话根目录解析（`~/.kiro/sessions/cli`）以及目录缺失时的安全返回
- [x] 2.2 实现 `*.json` 元数据文件扫描，并忽略 `.lock` 与非会话文件
- [x] 2.3 将 Kiro 元数据 JSON 映射为 `SessionSummary` 字段（`id/title/cwd/model/created_at/updated_at/platform_id`）
- [x] 2.4 按 `updated_at` 倒序排序并应用 offset/limit 分页
- [x] 2.5 实现单文件容错（单个坏文件跳过，不影响整体列表）

## 3. Kiro 消息分页解析器

- [x] 3.1 实现指定会话 `<session-id>.jsonl` 文件定位逻辑
- [x] 3.2 使用 `BufReader` 逐行流式读取 JSONL，并将 `kind=Prompt` 映射为 `role=user`
- [x] 3.3 将 `kind=AssistantMessage` 映射为 `role=assistant`，并提取拼接文本内容片段
- [x] 3.4 对映射后的用户/助手消息应用 offset/limit 分页
- [x] 3.5 对缺失或不可读的 JSONL 文件返回可定位问题的错误信息

## 4. 恢复命令集成

- [x] 4.1 在 `resume_session()` 中新增 `kiro` 分支，生成 `kiro-cli chat --resume-id <session-id>`
- [x] 4.2 复用现有项目路径前缀行为（`cd <project_path> && ...`）用于 Kiro 恢复
- [x] 4.3 确保 `kiro-cli` 不可用时终端启动错误能正确透传

## 5. 质量门禁与验证

- [x] 5.1 为 Kiro 元数据解析补充单元测试（必填字段映射与回退策略）
- [x] 5.2 为 Kiro JSONL 消息映射与分页边界补充单元测试（`Prompt` / `AssistantMessage`）
- [x] 5.3 增加集成式 smoke test：无 Kiro 数据时自动跳过且不报错
- [x] 5.4 运行 `cargo fmt`、`cargo test`、`cargo check` 并通过

## 6. 产品行为验收

- [x] 6.1 在有本地 Kiro 数据的机器上验证 Sessions 页可显示 Kiro 平台与正确会话数量
- [x] 6.2 验证打开 Kiro 会话后消息分页与角色渲染正确
- [x] 6.3 验证恢复动作会在目标终端启动预期命令

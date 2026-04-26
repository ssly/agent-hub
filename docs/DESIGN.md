# Agent Hub — 技术设计文档

## 1. 系统架构

```
┌──────────────────────────────────────────────────────────┐
│                      TUI Layer (ratatui)                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌─────────────┐ │
│  │Platform  │ │Skill     │ │Skill     │ │Diff Viewer  │ │
│  │List      │ │List      │ │Detail    │ │             │ │
│  └──────────┘ └──────────┘ └──────────┘ └─────────────┘ │
├──────────────────────────────────────────────────────────┤
│                    App State (Redux-like)                │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  platforms: Vec<Platform>                            │ │
│  │  selected_platform: Option<usize>                    │ │
│  │  selected_skill: Option<usize>                       │ │
│  │  mode: AppMode                                      │ │
│  │  diff_result: Option<DiffResult>                     │ │
│  │  sync_state: Option<SyncState>                       │ │
│  └─────────────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────────────┤
│                    Core Services                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌─────────────┐ │
│  │Discovery │ │Skill     │ │Diff      │ │Sync         │ │
│  │Service   │ │Parser    │ │Engine    │ │Service      │ │
│  └──────────┘ └──────────┘ └──────────┘ └─────────────┘ │
├──────────────────────────────────────────────────────────┤
│                    File System Layer                      │
│  (direct filesystem read/write, no database)             │
└──────────────────────────────────────────────────────────┘
```

## 2. 项目结构

```
agent-hub/
├── Cargo.toml
├── docs/
│   ├── PRD.md
│   └── DESIGN.md
├── src/
│   ├── main.rs              # 入口，启动 TUI
│   ├── app.rs               # App 状态机
│   ├── config.rs            # 配置文件加载 (~/.agent-hub/config.toml)
│   ├── i18n.rs              # 国际化模块（语言检测、翻译加载）
│   ├── platform/
│   │   ├── mod.rs
│   │   ├── registry.rs      # Platform 注册表（预定义 + 自定义）
│   │   └── discovery.rs     # 自动发现已安装 Platform
│   ├── skill/
│   │   ├── mod.rs
│   │   ├── model.rs         # Skill 数据模型
│   │   ├── parser.rs        # SKILL.md 解析器 (YAML frontmatter + body)
│   │   └── scanner.rs       # 扫描 Platform 目录获取 skill 列表
│   ├── diff/
│   │   ├── mod.rs
│   │   └── engine.rs        # 统一 diff 算法
│   ├── sync/
│   │   ├── mod.rs
│   │   └── service.rs       # Skill 同步（复制目录）
│   └── ui/
│       ├── mod.rs
│       ├── theme.rs          # 颜色主题
│       ├── platform_list.rs  # Platform 列表视图
│       ├── skill_list.rs     # Skill 列表视图
│       ├── skill_detail.rs   # Skill 详情视图
│       ├── diff_view.rs      # Diff 对比视图
│       ├── sync_dialog.rs    # 同步确认对话框
│       └── help_bar.rs       # 底部快捷键提示栏
├── locales/
│   ├── en.toml               # 英语翻译文件
│   └── zh-CN.toml            # 简体中文翻译文件
└── tests/
    ├── test_skill_parser.rs
    ├── test_discovery.rs
    ├── test_diff.rs
    └── test_i18n.rs
```

## 3. 数据模型

### 3.1 Platform

```rust
/// 一个 AI Agent 平台
struct Platform {
    /// 平台标识（如 "claude-code", "codex-cli"）
    id: String,
    /// 显示名称（如 "Claude Code", "Codex CLI"）
    display_name: String,
    /// skill 目录绝对路径
    skill_dir: PathBuf,
    /// 是否在本地已检测到（目录存在）
    installed: bool,
    /// 已发现的 skill 列表
    skills: Vec<Skill>,
}

/// 预定义的 Platform 注册表
struct PlatformRegistry {
    platforms: Vec<PlatformDef>,
}

struct PlatformDef {
    id: String,
    display_name: String,
    skill_dir: PathBuf,  // 使用 ~ 展开的绝对路径
}
```

### 3.2 Skill

```rust
/// 一个技能单元
struct Skill {
    /// 来自 YAML frontmatter 的名称
    name: String,
    /// 版本号（可选）
    version: Option<String>,
    /// 描述
    description: String,
    /// 所属 Platform 的 id
    platform_id: String,
    /// skill 目录的绝对路径
    path: PathBuf,
    /// SKILL.md 的绝对路径
    skill_file: PathBuf,
    /// SKILL.md 的完整内容（frontmatter + body）
    content: String,
    /// YAML frontmatter 解析后的原始键值
    metadata: HashMap<String, serde_yaml::Value>,
    /// 是否为软链接
    is_symlink: bool,
    /// 软链接指向的实际路径
    symlink_target: Option<PathBuf>,
    /// 目录下的所有文件（相对路径）
    files: Vec<PathBuf>,
    /// 最后修改时间
    modified_at: Option<std::time::SystemTime>,
    /// 文件总大小（字节）
    total_size: u64,
}
```

### 3.3 DiffResult

```rust
/// 两个 skill 之间的差异
struct DiffResult {
    source_platform: String,
    target_platform: String,
    skill_name: String,
    /// 按文件分组的 diff
    file_diffs: Vec<FileDiff>,
    /// 统计
    stats: DiffStats,
}

struct FileDiff {
    /// 文件相对路径（相对于 skill 目录）
    file_path: String,
    /// 统一 diff 格式的行
    hunks: Vec<DiffLine>,
    /// 该文件的统计
    stats: DiffStats,
    /// 仅在源端存在
    source_only: bool,
    /// 仅在目标端存在
    target_only: bool,
}

struct DiffStats {
    added: usize,
    removed: usize,
    changed: usize,
}

enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
    Header(String),
}
```

### 3.4 SyncState

```rust
/// 同步操作的中间状态
enum SyncState {
    /// 选择目标 Platform
    SelectTarget {
        source_skill: Skill,
        available_targets: Vec<Platform>,
    },
    /// 展示 diff，等待用户决策
    ConfirmOverwrite {
        source_skill: Skill,
        target_platform: Platform,
        diff: DiffResult,
    },
    /// 执行中
    InProgress,
    /// 完成
    Done,
}
```

## 4. 核心算法

### 4.1 Platform 发现

```rust
impl PlatformRegistry {
    fn builtin() -> Self {
        let home = dirs::home_dir().expect("no home directory");
        Self {
            platforms: vec![
                PlatformDef { id: "claude-code".into(), display_name: "Claude Code".into(),
                    skill_dir: home.join(".claude/skills") },
                PlatformDef { id: "codex-cli".into(), display_name: "Codex CLI".into(),
                    skill_dir: home.join(".codex/skills") },
                PlatformDef { id: "cursor".into(), display_name: "Cursor".into(),
                    skill_dir: home.join(".cursor/skills-cursor") },
                PlatformDef { id: "openclaw".into(), display_name: "OpenClaw".into(),
                    skill_dir: home.join(".openclaw/skills") },
                PlatformDef { id: "hermes".into(), display_name: "Hermes".into(),
                    skill_dir: home.join(".hermes/skills") },
                PlatformDef { id: "trae".into(), display_name: "Trae".into(),
                    skill_dir: home.join(".trae/skills") },
                PlatformDef { id: "shared-pool".into(), display_name: "Shared Pool".into(),
                    skill_dir: home.join(".agents/skills") },
            ]
        }
    }
}

impl DiscoveryService {
    /// 扫描所有注册的 Platform，返回已安装的列表
    fn discover(&self, registry: &PlatformRegistry) -> Vec<Platform> {
        registry.platforms.iter()
            .filter(|p| p.skill_dir.exists())
            .map(|p| self.scan_platform(p))
            .collect()
    }

    /// 扫描单个 Platform 的所有 skill
    fn scan_platform(&self, def: &PlatformDef) -> Platform {
        let skills = fs::read_dir(&def.skill_dir)
            .unwrap_or_default()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_dir() || path.is_symlink() {
                    let skill_file = path.join("SKILL.md");
                    if skill_file.exists() {
                        Some(SkillParser::parse(&path, &def.id))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        Platform {
            id: def.id.clone(),
            display_name: def.display_name.clone(),
            skill_dir: def.skill_dir.clone(),
            installed: true,
            skills,
        }
    }
}
```

### 4.2 SKILL.md 解析

```rust
impl SkillParser {
    fn parse(skill_dir: &Path, platform_id: &str) -> Skill {
        let skill_file = skill_dir.join("SKILL.md");
        let content = fs::read_to_string(&skill_file).unwrap_or_default();

        // 解析 YAML frontmatter: ---\n<yaml>\n---\n<body>
        let (metadata, body) = Self::parse_frontmatter(&content);

        let name = metadata.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| skill_dir.file_name().unwrap().to_str().unwrap())
            .to_string();

        let is_symlink = skill_dir.is_symlink();
        let symlink_target = if is_symlink {
            fs::read_link(skill_dir).ok()
        } else {
            None
        };

        // 递归列出目录下的所有文件
        let files = Self::list_files(skill_dir);

        Skill {
            name,
            version: metadata.get("version").and_then(|v| v.as_str()).map(String::from),
            description: metadata.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            platform_id: platform_id.to_string(),
            path: skill_dir.to_path_buf(),
            skill_file,
            content,
            metadata,
            is_symlink,
            symlink_target,
            files,
            modified_at: fs::metadata(skill_dir).ok().and_then(|m| m.modified().ok()),
            total_size: Self::calc_total_size(skill_dir),
        }
    }

    fn parse_frontmatter(content: &str) -> (HashMap<String, serde_yaml::Value>, String) {
        // 解析 --- 之间的 YAML
        if content.starts_with("---") {
            let end = content[3..].find("---").map(|i| i + 3);
            if let Some(end) = end {
                let yaml_str = &content[3..end];
                let body = content[end + 3..].trim();
                let metadata = serde_yaml::from_str(yaml_str).unwrap_or_default();
                return (metadata, body.to_string());
            }
        }
        (HashMap::new(), content.to_string())
    }
}
```

### 4.3 Diff 引擎

```rust
impl DiffEngine {
    /// 比较两个 skill 目录的差异
    fn diff(source: &Skill, target: &Skill) -> DiffResult {
        let mut file_diffs = Vec::new();

        // 收集所有文件（相对路径）
        let source_files: HashSet<PathBuf> = source.files.iter().cloned().collect();
        let target_files: HashSet<PathBuf> = target.files.iter().cloned().collect();

        let all_files: BTreeSet<PathBuf> = source_files.union(&target_files).cloned().collect();

        for file in all_files {
            let source_path = source.path.join(&file);
            let target_path = target.path.join(&file);

            match (source_files.contains(&file), target_files.contains(&file)) {
                (true, false) => {
                    file_diffs.push(FileDiff {
                        file_path: file.display().to_string(),
                        hunks: vec![DiffLine::Header(
                            format!("--- (仅在 {})", source.platform_id)
                        )],
                        stats: DiffStats {
                            added: fs::read_to_string(&source_path).map(|s| s.lines().count()).unwrap_or(0),
                            removed: 0, changed: 0,
                        },
                        source_only: true,
                        target_only: false,
                    });
                }
                (false, true) => {
                    file_diffs.push(FileDiff {
                        file_path: file.display().to_string(),
                        hunks: vec![DiffLine::Header(
                            format!("+++ (仅在 {})", target.platform_id)
                        )],
                        stats: DiffStats {
                            added: 0,
                            removed: fs::read_to_string(&target_path).map(|s| s.lines().count()).unwrap_or(0),
                            changed: 0,
                        },
                        source_only: false,
                        target_only: true,
                    });
                }
                (true, true) => {
                    let source_content = fs::read_to_string(&source_path).unwrap_or_default();
                    let target_content = fs::read_to_string(&target_path).unwrap_or_default();

                    if source_content != target_content {
                        let hunks = Self::unified_diff(&source_content, &target_content);
                        let stats = Self::count_stats(&hunks);
                        file_diffs.push(FileDiff {
                            file_path: file.display().to_string(),
                            hunks,
                            stats,
                            source_only: false,
                            target_only: false,
                        });
                    }
                }
            }
        }

        let stats = file_diffs.iter().fold(DiffStats { added: 0, removed: 0, changed: 0 }, |acc, d| {
            DiffStats {
                added: acc.added + d.stats.added,
                removed: acc.removed + d.stats.removed,
                changed: acc.changed + d.stats.changed,
            }
        });

        DiffResult {
            source_platform: source.platform_id.clone(),
            target_platform: target.platform_id.clone(),
            skill_name: source.name.clone(),
            file_diffs,
            stats,
        }
    }

    /// 使用 Myers diff 算法生成统一 diff 格式
    fn unified_diff(source: &str, target: &str) -> Vec<DiffLine> {
        // 使用 similar crate 实现高效 diff
        // similar::TextDiff::from(source, target)
        //     .unified_diff()
        //     .iter()
        //     .map(|line| match line.tag() {
        //         ChangeTag::Equal => DiffLine::Context(line.to_string()),
        //         ChangeTag::Insert => DiffLine::Added(line.to_string()),
        //         ChangeTag::Delete => DiffLine::Removed(line.to_string()),
        //     })
        //     .collect()
        todo!("使用 similar crate 实现")
    }
}
```

### 4.4 同步服务

```rust
impl SyncService {
    /// 同步 skill 目录（递归复制）
    fn sync(source: &Skill, target_platform: &Platform) -> Result<(), SyncError> {
        let target_dir = target_platform.skill_dir.join(&source.name);

        if target_dir.exists() {
            return Err(SyncError::TargetExists { path: target_dir });
        }

        Self::copy_dir_recursive(&source.path, &target_dir)
    }

    /// 覆盖同步（需用户确认）
    fn sync_overwrite(source: &Skill, target_platform: &Platform) -> Result<(), SyncError> {
        let target_dir = target_platform.skill_dir.join(&source.name);

        // 先删除目标
        if target_dir.exists() {
            // 如果目标是软链接，只删除链接本身
            if target_dir.is_symlink() {
                fs::remove_file(&target_dir)?;
            } else {
                fs::remove_dir_all(&target_dir)?;
            }
        }

        Self::copy_dir_recursive(&source.path, &target_dir)
    }

    fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), SyncError> {
        // 使用 fs_extra::dir::copy 或手动递归复制
        // 保留文件权限和时间戳
        todo!("实现递归目录复制")
    }
}
```

## 5. 国际化 (i18n) 设计

### 5.1 语言检测流程

```
启动
  ├─ 读取 config.toml 中 [general] language 字段
  │   ├─ 有值 → 使用指定语言
  │   └─ 无值 → 检测系统语言
  │       ├─ 读取 LANG / LC_ALL 环境变量
  │       ├─ 匹配 zh_CN / zh_CN.* / zh → zh-CN
  │       ├─ 匹配 en / en_US / en_GB / C → en
  │       └─ 其他 / 无法识别 → en (默认)
  └─ 加载对应翻译文件
```

### 5.2 数据模型

```rust
use std::collections::HashMap;

/// 支持的语言
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Locale {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}

impl Default for Locale {
    fn default() -> Self {
        Self::En
    }
}

/// 翻译键 → 翻译文本
struct I18n {
    locale: Locale,
    translations: HashMap<String, String>,
}

impl I18n {
    /// 从语言检测流程初始化
    fn init(config_locale: Option<Locale>) -> Self {
        let locale = config_locale
            .unwrap_or_else(Self::detect_system_locale);
        let translations = Self::load_translations(locale);
        Self { locale, translations }
    }

    /// 检测系统语言
    fn detect_system_locale() -> Locale {
        let lang = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .unwrap_or_default();

        if lang.starts_with("zh_CN") || lang.starts_with("zh_CN.") || lang == "zh" {
            Locale::ZhCn
        } else {
            Locale::En
        }
    }

    /// 从 embedded 或文件系统加载翻译
    fn load_translations(locale: Locale) -> HashMap<String, String> {
        // 优先从二进制内嵌的 TOML 读取（include_str!）
        // 回退到 ~/.agent-hub/locales/ 目录
        let content = match locale {
            Locale::ZhCn => include_str!("../locales/zh-CN.toml"),
            Locale::En => include_str!("../locales/en.toml"),
        };
        toml::from_str(content).unwrap_or_default()
    }

    /// 获取翻译文本，找不到 key 则返回 key 本身
    fn t(&self, key: &str) -> &str {
        self.translations.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    /// 支持插值的翻译
    fn t_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        let template = self.translations.get(key)
            .map(|s| s.as_str())
            .unwrap_or(key);
        let mut result = template.to_string();
        for (k, v) in args {
            result = result.replace(&format!("{{{}}}", k), v);
        }
        result
    }
}
```

### 5.3 翻译文件格式

`locales/en.toml`:

```toml
[ui]
title = "Agent Hub"
platforms = "Platforms"
skills = "Skills"
skill_detail = "Skill Detail"
diff_viewer = "Diff Viewer"
help = "Help"

[platform]
installed = "{count} skills"
not_installed = "Not installed"

[skill]
name = "Name"
version = "Version"
description = "Description"
files = "Files"
platform = "Platform"
symlink = "Symlink → {target}"
no_description = "(No description)"

[action]
detail = "Detail"
sync = "Sync"
diff = "Diff"
refresh = "Refresh"
search = "Search"
help_action = "Help"
quit = "Quit"

[sync]
title = "Sync Skill"
source = "Source: {platform} / {skill}"
target = "Target: {platform}"
conflict_warning = "Target platform already has a skill with the same name!"
overwrite_source = "(1) Overwrite with source"
keep_target = "(2) Keep target"
cancel = "(3) Cancel"
sync_confirm = "Sync {skill} to {platform}?"
sync_done = "Sync completed successfully"
sync_failed = "Sync failed: {error}"

[diff]
only_in_source = "Only in {platform}"
only_in_target = "Only in {platform}"
lines_added = "+{n} lines"
lines_removed = "-{n} lines"
no_diff = "No differences"

[error]
read_failed = "Failed to read: {path}"
parse_failed = "Failed to parse skill: {path}"
write_failed = "Failed to write: {path}"
permission_denied = "Permission denied: {path}"

[help]
title = "Keyboard Shortcuts"
navigation = "j/↓  Move down    k/↑  Move up"
open = "Enter  Open/Expand"
back = "Esc  Back/Cancel"
sync_key = "s  Sync to other platform"
diff_key = "d  Compare diff"
refresh_key = "r  Refresh"
search_key = "/  Search"
lang_key = "L  Switch language"
quit_key = "q  Quit"
```

`locales/zh-CN.toml`:

```toml
[ui]
title = "Agent Hub"
platforms = "平台列表"
skills = "技能列表"
skill_detail = "技能详情"
diff_viewer = "差异对比"
help = "帮助"

[platform]
installed = "{count} 个技能"
not_installed = "未安装"

[skill]
name = "名称"
version = "版本"
description = "描述"
files = "文件"
platform = "平台"
symlink = "软链接 → {target}"
no_description = "（无描述）"

[action]
detail = "详情"
sync = "同步"
diff = "对比"
refresh = "刷新"
search = "搜索"
help_action = "帮助"
quit = "退出"

[sync]
title = "同步技能"
source = "源: {platform} / {skill}"
target = "目标: {platform}"
conflict_warning = "目标平台已存在同名技能！"
overwrite_source = "(1) 使用源覆盖目标"
keep_target = "(2) 保留目标不变"
cancel = "(3) 取消"
sync_confirm = "将 {skill} 同步到 {platform}？"
sync_done = "同步完成"
sync_failed = "同步失败: {error}"

[diff]
only_in_source = "仅在 {platform}"
only_in_target = "仅在 {platform}"
lines_added = "+{n} 行"
lines_removed = "-{n} 行"
no_diff = "无差异"

[error]
read_failed = "读取失败: {path}"
parse_failed = "解析技能失败: {path}"
write_failed = "写入失败: {path}"
permission_denied = "权限不足: {path}"

[help]
title = "快捷键"
navigation = "j/↓  下移    k/↑  上移"
open = "Enter  打开/展开"
back = "Esc  返回/取消"
sync_key = "s  同步到其他平台"
diff_key = "d  差异对比"
refresh_key = "r  刷新"
search_key = "/  搜索"
lang_key = "L  切换语言"
quit_key = "q  退出"
```

### 5.4 UI 中的使用方式

所有 UI 组件通过 `App` 持有的 `I18n` 实例获取翻译文本：

```rust
struct App {
    i18n: I18n,
    // ...
}

impl App {
    // 在 UI 渲染中使用
    fn render_help_bar(&self, frame: &mut Frame, area: Rect) {
        let i = &self.i18n;
        let text = format!(
            "[Enter]{}  [s]{}  [d]{}  [L]{}  [?]{}  [q]{}",
            i.t("action.detail"),
            i.t("action.sync"),
            i.t("action.diff"),
            // L 键切换语言 — 显示当前 locale 标识
            format!("{}({})", i.t("action.lang"), self.i18n.locale_tag()),
            i.t("action.help_action"),
            i.t("action.quit"),
        );
        // ...
    }
}
```

运行时按 `L` 键可在 zh-CN 和 en 之间即时切换（无需重启）。

### 5.5 翻译文件加载策略

```
优先级（从高到低）:
1. ~/.agent-hub/locales/{locale}.toml   ← 用户可自定义覆盖
2. 二进制内嵌 include_str!("../locales/{locale}.toml")  ← 默认翻译

合并规则：用户文件中的 key 覆盖内嵌的 key，未覆盖的保留内嵌值。
这样用户可以只覆盖部分文案，不必维护完整的翻译文件。
```

### 5.6 配置文件中的语言设置

在 `config.toml` 的 `[general]` 节中添加：

```toml
[general]
# 语言设置: "zh-CN" | "en" | "auto"
# "auto" = 自动检测系统语言（默认）
language = "auto"
```

## 6. UI 设计

### 6.1 布局结构

```
┌─ Agent Hub ──────────────────────────────────────────────────────┐
│ ┌─ Platforms ──────────┐ ┌─ Skills ───────────────────────────┐ │
│ │ ● Claude Code  (64)  │ │ ○ apple-calendar    v1.0.0  🔗    │ │
│ │   Codex CLI   (110)  │ │ ○ lark-base         v1.0.0  🔗    │ │
│ │   Cursor       (9)   │ │ ○ lark-calendar     v1.0.0  🔗    │ │
│ │   OpenClaw     (23)  │ │ ○ lark-contact      v1.0.0  🔗    │ │
│ │   Hermes       (34)  │ │ ○ lark-doc          v1.0.0        │ │
│ │   Trae         (23)  │ │ ○ lark-im           v1.0.0  🔗    │ │
│ │   Shared Pool  (23)  │ │ ● lark-sheets       v1.0.0        │ │
│ │                      │ │ ○ review            v0.1.0        │ │
│ │                      │ │ ○ ship              v1.0.0        │ │
│ │                      │ │                                    │ │
│ │                      │ │                                    │ │
│ └──────────────────────┘ └────────────────────────────────────┘ │
│ ┌─ Skill Detail ──────────────────────────────────────────────┐ │
│ │ Name: lark-sheets                                           │ │
│ │ Version: 1.0.0    Platform: Claude Code    Files: 3        │ │
│ │ Description: 飞书电子表格：创建和操作电子表格。创建表格并写入... │ │
│ │                                                              │ │
│ │ Files: SKILL.md, references/sheets-api.md, scripts/...      │ │
│ └──────────────────────────────────────────────────────────────┘ │
│ [Enter]详情  [s]同步  [d]对比  [?]帮助  [q]退出                  │
└──────────────────────────────────────────────────────────────────┘
```

### 6.2 同步对话框

```
┌─ 同步 Skill ────────────────────────────────────────────────────┐
│                                                                  │
│  源: Claude Code / lark-sheets                                   │
│  目标: Codex CLI                                                 │
│                                                                  │
│  ⚠ 目标 Platform 已存在同名 skill！                              │
│                                                                  │
│  ┌─ Diff: SKILL.md ───────────────────────────────────────────┐ │
│  │ - # 飞书电子表格 (Claude Code 版本)                         │ │
│  │ + # 飞书电子表格 (Codex CLI 版本)                           │ │
│  │   创建和操作电子表格。                                       │ │
│  │ - 默认使用 lark-cli 调用                                    │ │
│  │ + 默认使用 codex skill 调用                                 │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  (1) 使用源覆盖目标    (2) 保留目标不变    (3) 取消              │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 6.3 交互快捷键

| 按键 | 功能 |
|------|------|
| `j` / `↓` | 下移光标 |
| `k` / `↑` | 上移光标 |
| `Enter` | 进入详情 / 展开 |
| `Esc` | 返回上层 / 取消 |
| `s` | 同步到其他 Platform |
| `d` | Diff 对比（选择两个 Platform） |
| `L` | 切换语言（zh-CN ↔ en） |
| `r` | 刷新（重新扫描） |
| `/` | 搜索 skill |
| `?` | 帮助 |
| `q` | 退出 |

### 6.4 应用状态机

```rust
enum AppMode {
    /// Platform 列表（默认）
    PlatformList,
    /// 某个 Platform 的 Skill 列表
    SkillList,
    /// Skill 详情
    SkillDetail,
    /// Diff 对比视图
    DiffView {
        source: usize,     // skill index
        target_platform: String,
    },
    /// 同步流程
    SyncSelectTarget,
    SyncConfirm {
        diff: DiffResult,
    },
    /// 搜索
    Search,
}
```

## 7. 依赖库

| 库 | 用途 | 版本 |
|----|------|------|
| `ratatui` | TUI 框架 | ^0.29 |
| `crossterm` | 终端后端 | ^0.28 |
| `serde` + `serde_yaml` | YAML frontmatter 解析 | latest |
| `serde_json` | JSON 解析（配置） | latest |
| `similar` | Diff 算法（Myers） | ^2 |
| `dirs` | 获取 home 目录 | ^6 |
| `toml` | 配置文件 | latest |
| `walkdir` | 递归目录遍历 | ^2 |
| `fs_extra` | 递归目录复制 | latest |
| `unicode-width` | CJK 字符宽度计算 | latest |
| `clap` | CLI 参数解析 | ^4 |
| `sys-locale` | 系统语言检测（跨平台） | ^0.3 |

## 8. 配置文件

`~/.agent-hub/config.toml`:

```toml
# 自动生成的默认配置

[general]
# 语言设置: "zh-CN" | "en" | "auto"
# "auto" = 自动检测系统语言（默认）
language = "auto"

# 自定义 Platform（在预定义之外追加）
[[platforms]]
id = "my-custom-agent"
display_name = "My Custom Agent"
skill_dir = "/path/to/skills"

# 显示偏好
[display]
show_symlink_target = true
max_description_width = 80

# 同步偏好
[sync]
# 跳过软链接目标检测（始终跟随复制实际内容）
follow_symlinks = true
```

## 9. 错误处理策略

| 场景 | 处理方式 |
|------|---------|
| Platform 目录不存在 | 跳过，标记为未安装 |
| SKILL.md 解析失败 | 使用目录名作为 name，description 置空 |
| 文件读取权限不足 | 在 UI 中显示警告，跳过该文件 |
| 同步时目标写入失败 | 回滚（删除已复制的部分），显示错误 |
| 软链接循环 | 检测并跳过，显示警告 |

## 10. 测试策略

| 类型 | 覆盖内容 |
|------|---------|
| 单元测试 | SKILL.md 解析、diff 算法、路径处理 |
| 集成测试 | Platform 发现、skill 扫描（使用临时目录） |
| 手动测试 | TUI 交互、同步流程 |

测试中使用临时目录模拟 Platform 结构：

```rust
#[test]
fn test_discover_platforms() {
    let tmp = tempdir::TempDir::new("agent-hub-test").unwrap();
    let skill_dir = tmp.path().join(".claude/skills");
    fs::create_dir_all(skill_dir.join("test-skill")).unwrap();
    fs::write(skill_dir.join("test-skill/SKILL.md"), "---\nname: test\n---\nbody").unwrap();
    // ...
}
```

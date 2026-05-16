# Agent Hub — 视觉与交互设计规范 v1.0

> 一份可以直接落地到 `src/theme.css` 的设计风格文档。
> 灵感来源：**Things 3**（克制的留白、温柔的动作、藏起来的功能）+ **Notion 中文站 / 水墨风**（纸感底色、墨色字、宋体落款、低饱和强调）。
> 适用范围：Agent Hub 桌面应用（Tauri + 原生 HTML + Tailwind v4）。

---

## 0. 一句话定位

> **「桌面上的一本笔记本」**——你打开 Agent Hub，应该像翻开一本素白的活页本，
> 内容是主角，工具藏起来；任何颜色出现都必须有意义。

设计的根问题不是「黑还是白」，而是当前 UI **同时使用了五种饱和度很高的颜色**（橙/紫/青/绿/红）当作按钮底，
信息密度又高，眼睛找不到落点。本规范的核心动作是：**降饱和、提层次、给留白**。

---

## 1. 设计哲学

### 1.1 三条铁律

1. **内容至上（Content First）**
   一屏最显眼的应当是数据本身（skill 名、会话标题、文件内容），不是控件。
   工具栏、按钮、徽标、分割线都要"退后半步"。

2. **一处强调（One Voice）**
   每个视图只允许**一个主色按钮**（Primary）。其他动作降级为 Secondary / Ghost。
   绝不再出现"橙色 Check + 紫色 Diff + 青色 Sync"三个高饱和按钮并排的局面。

3. **静默工具（Silent Tools）**
   次要按钮（删除、刷新、语言切换、同步图标）默认是**墨灰**，仅在 hover / focus 时才显色。
   工具应当像铅笔——你拿起它时才显现，放下时归于纸面。

### 1.2 美学锚点

| 维度 | 参考 | 我们的做法 |
|---|---|---|
| 间距 | Things 3 | 行高 1.65、垂直气息≥12px、组与组之间 24-32px |
| 字色 | Notion 水墨 | 主文本 #2A2A2E（墨色），不用纯黑；中文用宋黑/思源黑 |
| 强调色 | Things 3 蓝 → 墨青 | `#3A6B8C`（远山青）替代亮青 `#7DD3FC` |
| 容器 | Notion 卡片 | 几乎不用阴影；用 1px 极淡边线区隔层次 |
| 圆角 | Things 3 | 8–14px，避免过圆（不要做成 macOS 控件） |
| 动效 | Things 3 | 120–240ms，缓动统一 `cubic-bezier(.2,.8,.2,1)`，**不缩放** |

---

## 2. 双主题：Ink Light（默认） / Ink Night

### 2.1 Ink Light — 水墨白

> 米白宣纸 + 远山墨青。默认主题，工作时长 8 小时不刺眼。

```
背景层次：
  Canvas    #F8F6F1   宣纸底（带 0.5% 暖黄）
  Surface   #FFFFFE   主内容卡片
  Sunken    #F2EFE7   侧栏 / 工具栏（比 Canvas 略沉）
  Hover     #EDE9DE   交互态
  Active    #E5DFD0   按下态

字色：
  Ink       #2A2A2E   主文本（墨色，不用 #000）
  Ink-2     #5B5B61   次要文本
  Ink-3     #8C8B86   弱化文本 / 占位
  Ink-4     #B7B5AC   极弱（分割线、disabled）

强调：
  Accent    #3A6B8C   远山青（Primary 按钮、品牌、链接、高亮）
  Accent-Soft  rgba(58,107,140,.10)
  Highlight #C9A961  落款黄（仅用于品牌字、激活态左侧 marker）
  Success   #5A8F6B   苔绿
  Warning   #B07A3E   赭石（替代 #FCD34D 刺眼黄）
  Danger    #B0524A   朱砂（替代 #FCA5A5 粉红）

线条：
  Hairline  rgba(42,42,46,.06)   极淡分割线
  Border    rgba(42,42,46,.10)   常规边线
  Border-Strong rgba(42,42,46,.18)  强调边线（focus、selected）
```

### 2.2 Ink Night — 水墨夜

> 给夜间党的备选。**不是深紫**——是深岩灰带一点蓝，像未干透的墨。

```
背景层次：
  Canvas    #1C1D1F   岩底
  Surface   #232427   主卡片
  Sunken    #18191B   侧栏 / 工具栏
  Hover     #2D2E32
  Active    #34353A

字色：
  Ink       #E8E6DF   宣纸白（暖，不用纯白）
  Ink-2     #B0AEA6
  Ink-3     #7C7A73
  Ink-4     #4F4E48

强调：
  Accent    #7DA8C9   月光青（夜间用更亮的远山青）
  Accent-Soft  rgba(125,168,201,.14)
  Highlight #D9B97C
  Success   #8FB89A
  Warning   #D69963
  Danger    #D88078
```

**切换规则**：在侧栏底部加一个 ☾/☀ 切换；遵循系统偏好 `prefers-color-scheme`；
切换不重载页面，仅替换 `:root` 上的 `data-theme` 属性。

---

## 3. 字体系统

### 3.1 字族

| 用途 | 字体 | 备用 | 字重 |
|---|---|---|---|
| 中文 UI | **PingFang SC / 苹方** | 思源黑体 CN、HarmonyOS Sans SC | 400 / 500 / 600 |
| 英文 UI | **Inter** | -apple-system、SF Pro Text | 400 / 500 / 600 / 700 |
| 品牌字 / 大标题 | **Newsreader**（衬线）+ **思源宋体** | Georgia、Source Han Serif | 500 / 600 |
| 等宽 | **JetBrains Mono** | SF Mono、Cascadia | 400 / 500 |

```css
--font-sans: 'Inter', 'PingFang SC', 'Source Han Sans CN', -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
--font-serif: 'Newsreader', 'Source Han Serif CN', Georgia, 'Songti SC', serif;
--font-mono: 'JetBrains Mono', 'SF Mono', 'Fira Code', ui-monospace, monospace;
```

> ⚠️ **当前 index.html 加载的是 Plus Jakarta Sans，请删除**，改为 Inter + Newsreader。

### 3.2 字号 / 行高

| Token | 字号 | 行高 | 字重 | 用途 |
|---|---|---|---|---|
| `text-display` | 22px | 30 | 600 | 详情页标题（"benchmark"） |
| `text-h1` | 18px | 26 | 600 | 模态框标题 |
| `text-h2` | 15px | 22 | 600 | 卡片小标题、品牌"智能体中枢" |
| `text-body` | 14px | 22 | 400 | 列表行主文本 |
| `text-meta` | 13px | 20 | 400 | 描述、副信息 |
| `text-caption` | 12px | 18 | 500 | 标签、徽标 |
| `text-mono` | 13px | 20 | 400 | 路径、代码、PATH |

中文字号在 Tauri 上对应建议 `letter-spacing: 0.01em`，避免挤在一起。
所有正文统一 `line-height: 1.65`，**段落之间 8px 气息**。

### 3.3 品牌处理

把当前的渐变青→绿→紫品牌字换成 **「智能体中枢」六个字 Newsreader 风格的衬线** + 右侧落款黄一个小印章式圆点：

```
Agent Hub                                     ·
智能体中枢                                落款
```

- 中文用 `font-serif` + `font-weight: 500`，颜色 `var(--ink)`
- 后面跟一个 6×6 的 `var(--highlight)` 圆点
- **绝不再用三色渐变 text-fill**，那是 Web3 风，不是水墨风

---

## 4. 间距 / 圆角 / 阴影 / 边线 Token

### 4.1 8px Grid Spacing

```
--space-1:  4px   细微（图标/文字间距）
--space-2:  8px   常用气息
--space-3:  12px  紧凑组件内 padding
--space-4:  16px  标准内 padding
--space-5:  24px  组间距
--space-6:  32px  区块间距
--space-8:  48px  大区块
--space-10: 64px  hero / 空态
```

### 4.2 圆角

```
--radius-xs:  4px   徽标、tag
--radius-sm:  8px   按钮、输入框、列表项
--radius-md:  10px  卡片
--radius-lg:  14px  Modal、Sheet
--radius-pill: 999px  Pill 按钮 / 计数徽标
```

### 4.3 阴影（极简）

```
--shadow-mist:  0 1px 2px rgba(42,42,46,.04)
                /* 卡片默认 */
--shadow-soft:  0 4px 12px rgba(42,42,46,.06),
                0 1px 2px rgba(42,42,46,.04)
                /* hover / 浮起 */
--shadow-modal: 0 24px 48px rgba(42,42,46,.12),
                0 2px 6px rgba(42,42,46,.06)
                /* Modal */
```

> 夜间版本把 `rgba(42,42,46,...)` 换成 `rgba(0,0,0,...)` 并加倍透明度。

### 4.4 边线

只用三种粗细：
- `hairline`（极淡，列表项之间）—— `1px solid var(--hairline)`
- `border`（常规） —— `1px solid var(--border)`
- `border-strong`（focus / selected） —— `1px solid var(--border-strong)` 或左边 2px marker

**禁止内描边 + 阴影同时出现**。要么用阴影抬起，要么用边线区隔，不要叠加。

### 4.5 动效

```
--ease-out:   cubic-bezier(.2, .8, .2, 1)   /* 进入 */
--ease-in:    cubic-bezier(.4, 0, .9, .2)   /* 退出 */
--ease-soft:  cubic-bezier(.4, 0, .2, 1)    /* 通用 */

--dur-fast:   120ms   /* 颜色、不透明度 */
--dur-base:   180ms   /* 进入退出 */
--dur-slow:   240ms   /* 模态、面板 */
```

**禁止**：
- `transform: scale()` 作为 hover 反馈（会撑开布局，让人眼睛抖）
- 任何超过 280ms 的过渡（产品要"快"）

---

## 5. 组件规范

### 5.1 标题栏 / 侧栏顶部（Brand + Toolbox）

```
┌───────────────────────────────────────────┐
│  Agent Hub  ·                  EN  ⟳  ‹   │
│  智能体中枢                                │
└───────────────────────────────────────────┘
```

- 高度 56px（比当前更舒展）
- 背景 `var(--surface-sunken)`，底边 `hairline`
- 品牌字两行：英文 14px Newsreader 500 + 中文 13px Newsreader 500
- 右侧三个图标按钮（语言/刷新/折叠）：18×18 icon，颜色 `--ink-3`，hover → `--ink-1`
- 三个按钮之间用 `--space-1`，**不要边框**

### 5.2 Tab 条

当前 4 个 tab（技能/MCP/会话/监控）问题是：未激活的 tab 颜色太浅，激活的下划线又太粗，没有节奏。

新规：
```
┌────────────────────────────────────────┐
│  技能    MCP    会话    监控           │
│  ──                                    │
└────────────────────────────────────────┘
```

- Tab 文本：未激活 `--ink-3` 13px / 激活 `--ink` 13px **500**
- 激活态下划线：宽度只覆盖文字 + 4px padding，**2px 高，颜色 `--accent`**
- Tab 之间 24px 间距（左对齐，不要 flex-1 平均分），整条放在容器左 padding 16px 起
- 切换 tab 时下划线**滑动**（180ms ease-out）到下一个 tab 位置

### 5.3 平台列表项

```
┌────────────────────────────────────┐
│  Shared Pool              121      │
│  Claude Code              77       │
│ ▎ Codex CLI               43       │
│   OpenAI Codex CLI agent skills   │
│   /Users/liuyang/.codex/skills     │
│  OpenClaw                 26       │
└────────────────────────────────────┘
```

- 高度：默认 36px；选中态展开成两行/三行（描述 + 路径）
- 选中态左侧：**2px 实心 marker，颜色 `--accent`**，背景 `--accent-soft`
- 计数徽标：右对齐、`pill` 形状、12px、`--ink-3` 文字、无背景
- 描述行：`--ink-3` 12px
- 路径行：`--mono` 11px `--ink-4`，**最大行宽 omit 截断**
- 行间 `hairline` 分割，**没有圆角块感**——更像清单

### 5.4 主区工具栏

去掉橙色 / 紫色 / 青色三种高对比按钮。统一三个层级：

| 层级 | 用法 | 视觉 |
|---|---|---|
| **Primary** | 单屏唯一动作（保存、确认） | `--accent` 实底 + 白字 |
| **Secondary** | 常用动作（同步、对比） | 透明 + `border` + `--ink-2` 字；hover 变 `--hover` 底 |
| **Ghost** | 静默动作（返回、关闭、刷新） | 仅图标 + `--ink-3`；hover 变 `--ink-1` |
| **Danger** | 删除、清空（**仅在 hover 显色**） | 默认 `--ink-3`，hover `--danger` |

工具栏右侧布局：`[Ghost: 返回] [面包屑] ─ flex ─ [Secondary: Diff] [Secondary: Sync] [Primary: Check]`

> 注意：当前的「Check」按钮是橙色（warning 色当 CTA），这是用色错误。
> 「Check」是一个**主动作**应当是 Primary 墨青；
> 一旦发现问题，**它本身才会变成 warning 赭石**，作为状态反馈。

### 5.5 列表行（Skill 列表）

当前每行：`autoplan  Auto-review pipeline...  🔗 81KB`，密度合适，但缺乏呼吸。

新规：
```
┌──────────────────────────────────────────────────────────┐
│  autoplan                                          81 KB │
│  Auto-review pipeline — reads the full CEO, design...   │
│  ─────────────────────────────────────────────────────── │
│  benchmark                                        31 KB │
│  Performance regression detection using the browse...    │
└──────────────────────────────────────────────────────────┘
```

- 名称：14px 500 `--ink`
- 描述：13px 400 `--ink-2`，**单行省略**
- 右侧体积徽标：12px `--ink-3`，淡 pill 背景 `--hover`
- 链接图标 `🔗`：从描述里**移除**，并入 hover 时显示的悬浮 "打开符号链接" Ghost 按钮
- 行高 56px（两行字 + padding）
- 行间用 `hairline`，**没有圆角包裹**——一整列像章节目录
- hover：背景 `--hover`，**左侧出现 2px `--accent-soft` marker**（不是 marker 本身，是更淡的色）
- 选中：背景 `--accent-soft`，左侧 marker 变实色

### 5.6 详情页（Skill Detail）

当前问题：黄色的"平台:"标签 + 青色路径 + 黄色绿色 PATH，眼花。

新规：用「**键值对网格**」+ 「**信息行省去多余颜色**」。

```
benchmark                                    ⟪Back

平台      codex-cli
PATH      /Users/liuyang/.codex/skills/gstack-benchmark
大小      31.5 KB
软链接    →  /Users/liuyang/.claude/.agents/skills/gstack-benchmark/
文件      2 个文件
          SKILL.md
          agents/openai.yaml

描述
─────────────────────────────────────────────────
Performance regression detection using the browse daemon.
Establishes baselines for page load times, Core Web Vitals…


SKILL.md                                      Close
─────────────────────────────────────────────────
...代码…
```

- 标题用 22px Newsreader / 600 + 中文配 PingFang
- 字段标签：60px 固定宽度、12px caption、`--ink-3`、**全部小写无大写化**（去掉 `text-transform: uppercase`，太工程化）
- 字段值：14px `--ink`，可选中
- 软链路径：`mono` 13px `--ink-2`，箭头 `→` 用 `--accent` 提示这是一次「跳转」
- 文件列表：缩进 16px、`mono` 13px、悬停整行 `--hover`
- 「描述」与「SKILL.md」用 24px space + `hairline` 分章节
- 代码块容器：背景 `--sunken`、`hairline` 边框、行号 `--ink-4`、关键字 **不要语法高亮**（保持纯净，让 mono 字体替我们承担节奏感）

### 5.7 会话卡（Session Card）—— 重灾区

当前卡片同时塞了：标题、日期、路径、模型名、时间、3 个按钮（查看消息 灰、恢复会话 绿、删除 红），按钮颜色把"卡片是什么"喂得太满。

新规：**一行内容 + 一组动作 + 极少颜色**。

```
┌─────────────────────────────────────────────────────────────────┐
│  我使用了cc-switch切换各个api，但是我hook里面有 masko 这个...    │
│  /Users/liuyang/.claude    ·  deepseek-v4-pro                   │
│  2026/5/15 00:19:11        开始时间 00:46:33                    │
│                                                                  │
│                              查看消息   恢复会话                 │
│                                                  ⋯              │
└─────────────────────────────────────────────────────────────────┘
```

- 卡片：`--surface` 底、`--radius-md`、`hairline` 边、**默认无阴影**
- 标题：14px 500 `--ink`，**两行省略**
- 元数据行：13px `--ink-2`，用 `·` 中点分隔（**不要换行不要标签**）
- 「查看消息」= Ghost、「恢复会话」= Primary、删除 = **三点菜单内**（次级动作）
- 卡片 hover 时整体 `--shadow-soft`，**不变形**
- 整列卡片间距 12px

> 关键改动：**把"删除"按钮从卡片右下角移出**，进入「⋯」溢出菜单。
> 原因：删除是一个**双阶段+破坏性**操作，永远不应当 primary-level。

### 5.8 按钮规格

```
高度：32px（标准）/ 28px（紧凑）/ 24px（micro，仅 icon-only）
水平 padding：12px（标准）/ 10px / 0（icon-only）
图标尺寸：14px（标准）/ 12px / 16px（icon-only）
字号：13px / 500
圆角：8px（标准）/ 999px（pill 计数）
间距：相邻按钮间 6px

按下：背景加深 6%，**不缩放**
禁用：opacity 0.4、cursor not-allowed
loading：替换文字为转圈 spinner，禁用点击
```

### 5.9 输入框

```
高度：32px
背景：--surface
边框：1px solid --border
圆角：8px
内 padding：10px 12px
字号：13px

focus：border-color → --accent，**外发光去掉**
       （改成 `outline: 2px solid var(--accent-soft)` 内描边）
placeholder：--ink-3 letter-spacing 0.01em
```

### 5.10 Modal

```
背景遮罩：rgba(28,29,31,.4) + backdrop-filter blur(8px)
        （Light 模式遮罩用 rgba(42,42,46,.25)）
卡片：--surface 底、--radius-lg、--shadow-modal
最大宽：560px（窄）/ 720px（标准）/ 920px（宽）
垂直 padding：24px
水平 padding：32px

标题区：18px 600 `--ink`，下方 8px 副标题 `--ink-2`
内容区：16px 段间距
动作区：右对齐，间距 8px；Ghost · Secondary · Primary 由左到右
关闭按钮：右上角 Ghost、20×20、`×` 用 12px 线条
```

### 5.11 Toast

```
位置：右下角 24px / 24px
背景：--surface
边框：hairline
圆角：8px
内 padding：12px 16px
最大宽：360px

左侧 2px 实心 marker：
  success → --success
  warning → --warning
  danger  → --danger
  info    → --accent

时长：2.5s（短信息）/ 4s（含动作）
动效：从下方 12px 滑入 240ms ease-out
```

### 5.12 空态

不要再放一堆 "暂无数据"。用一句**有人味的句子**：

```
        ┌─────────────┐
        │   ✦         │
        └─────────────┘
       这里还是一张白纸
   把第一个 MCP 配置贴进来，开始
```

- 图标：14×14 `--ink-4`，使用 ✦ / · / ⌥ 这类极简符号，**不要 emoji**
- 文案：14px `--ink-3`
- 居中、垂直 padding 80px
- 下方可选「Secondary 按钮」做一步引导

### 5.13 滚动条

```
宽度：8px（hover 时显示）/ 0px（默认隐藏）
轨道：透明
拇指：rgba(42,42,46,.16)，圆角 4px
hover 拇指：rgba(42,42,46,.28)
```

不要做超出半透明的滚动条，桌面应用要让人**看得见**当前位置。

---

## 6. 信息密度与节奏

### 6.1 三种"密度档"

| 视图 | 行高 | 字号 | 间距 |
|---|---|---|---|
| **舒朗**（详情页、Modal） | 1.65 | 14 | 24 |
| **常规**（Skill 列表、会话卡） | 1.5  | 14 | 16 |
| **紧凑**（侧栏平台列表、文件树） | 1.4  | 13 | 8  |

不允许在一屏内混用三种密度。

### 6.2 节奏：每屏只能有 1+2+N

- **1** 个主标题（视图入口标题）
- **2** 个 Primary/Secondary 动作（最多）
- **N** 条等节奏内容

如果你想加第 3 个动作按钮——先检查是不是该折到溢出菜单。

---

## 7. 交互与微互动

### 7.1 鼠标光标

- 所有可点击 → `cursor: pointer`
- 文本 selectable 区 → `cursor: text`
- disabled → `cursor: not-allowed`

### 7.2 Hover 反馈

```
列表行：背景 --hover，左侧出现淡 marker
卡片  ：阴影从 --shadow-mist → --shadow-soft（提升约 1px）
按钮  ：背景加深 4%（不变形）
图标  ：颜色 --ink-3 → --ink-1
链接  ：颜色 --accent → --accent 加深 8%（不下划线）
```

### 7.3 Focus

```
所有可聚焦元素：outline 2px solid var(--accent-soft)
                outline-offset 2px
按钮 focus-visible：再加一圈 1px var(--accent) 内描边
```

### 7.4 危险操作

**两阶段确认**——单击 ≠ 触发：

```
[ 删除 ]    →（首次点击）   [ 再点击确认删除 ]   →（再点 800ms 内）真正执行
                                ↑ 此时文字变 --danger、800ms 后退回
```

绝不弹原生 `confirm()`；除非用户在 1s 内放手 + 移开鼠标 + 点击别处，否则视为放弃。

### 7.5 键盘

| 快捷键 | 动作 |
|---|---|
| `⌘1 / ⌘2 / ⌘3 / ⌘4` | 切换 Tab |
| `⌘K` | Focus 搜索框 |
| `⌘\` | 折叠/展开侧栏 |
| `⌘.` | 关闭 Modal |
| `↑ / ↓` | 在列表内移动 |
| `Enter` | 打开当前项 |
| `Backspace` | 在详情页返回上一级 |

每条都要在对应控件上加 `title` 或 tooltip 提示，不要藏。

### 7.6 加载

- 不再用整屏 spinner。
- 列表加载用 **skeleton 占位行**（高度同实际行高、`--hover` 底、`loadingPulse` 1.5s）。
- 异步按钮 = 替换文字为 `⟳` 转圈（**保持按钮宽度不变**——这一条当前实现里需要补 `min-width`）。

---

## 8. 字符 / 中英混排细节

中文用户体验里最容易被忽略的几条：

1. 中英文之间**自动空格**。`pangu.js` 或在写文案时手工加空格：
   `Performance 测试 / 已加载 50 个 / 共 188 项`
2. 中文标点用全角（，。；："）；英文环境用半角。**不要混用**。
3. 中文数字用阿拉伯数字：`已加载 50 / 共 188`，**不要**「已加载五十」。
4. 中文 `font-feature-settings: 'kern' 1, 'palt' 1`（Tauri 上 WebKit 支持）。

---

## 9. 当前 UI 五大问题与一一对应的解法

> 直接对应你截图里能看到的痛点。

| # | 问题 | 解法 |
|---|---|---|
| 1 | 深紫罗兰背景 `#1a1a2e` 沉闷、不亲切 | 切换到 Ink Light `#F8F6F1`，夜间用 Ink Night `#1C1D1F` |
| 2 | 「Check」橙色 + 「对比」紫色 + 「同步」青色 三个高饱和按钮并排，CTA 不明 | Check=Primary 墨青、Diff=Secondary 透明、Sync=Secondary 透明 |
| 3 | 会话卡上「绿色 恢复会话 + 红色 删除」喧宾夺主 | 恢复会话=Primary、删除塞进 `⋯` 溢出菜单 |
| 4 | 品牌字「智能体中枢」用了青→绿→紫渐变文字，与水墨主题违和 | 改 Newsreader 衬线 + 落款黄圆点 |
| 5 | 列表行用 `text-yellow-400 / text-cyan-400` 混色标签，无层次 | 全部统一到 `--ink-2 / --ink-3`，**仅文字大小区分主次**；颜色只在 hover 时显现 |

---

## 10. 落地路线（5 步迁移计划）

为避免一次性大改 990 行 `theme.css` 出 bug，分阶段：

### Phase 1 — Token 替换（半天，零功能影响）

在 `:root` 顶部新增 Light/Night 两套 Token；保留旧变量名作为别名映射，例如：

```css
--bg-root: var(--canvas);
--bg-surface: var(--surface);
--accent: var(--accent);  /* 颜色值变了 */
```

视觉立即变成 Ink Light。这一步**不删任何选择器**，零风险。

### Phase 2 — 字体替换（10 分钟）

`index.html` 替换 Google Fonts URL：
```html
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Newsreader:wght@500;600&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet" />
```

`theme.css` 更新 `--font-sans` / `--font-serif`。

### Phase 3 — 品牌字 + Tab 条（1h）

修改 `aside h1` 选择器：去掉渐变，换衬线 + 落款圆点。
重写 `#tab-bar` 样式：下划线只覆盖文字宽度、滑动过渡。

### Phase 4 — 按钮三层级（2h）

新增 `.btn-primary / .btn-secondary / .btn-ghost / .btn-danger` 四个工具类；
**逐个把现有 `bg-cyan-700 / bg-purple-700 / bg-green-700 / bg-red-700` 替换**。
注意：删除按钮要从会话卡右下移到 `⋯` 菜单（需要 `app.js` 改 dom）。

### Phase 5 — 列表行与卡片节奏（半天）

- Skill 列表换两行布局 + hairline 分割
- 会话卡按 5.7 重排
- 平台列表加 marker

### 可选 Phase 6 — Ink Night 主题

加 `data-theme="night"` 切换；在侧栏底部加 ☾/☀ 图标。

---

## 11. 全量 CSS Token 表（直接复用）

把这一段贴到 `src/theme.css` 顶部即可：

```css
:root,
:root[data-theme="light"] {
  /* Surfaces */
  --canvas:        #F8F6F1;
  --surface:       #FFFFFE;
  --sunken:        #F2EFE7;
  --hover:         #EDE9DE;
  --active:        #E5DFD0;

  /* Ink */
  --ink:           #2A2A2E;
  --ink-2:         #5B5B61;
  --ink-3:         #8C8B86;
  --ink-4:         #B7B5AC;

  /* Accents */
  --accent:        #3A6B8C;
  --accent-soft:   rgba(58, 107, 140, 0.10);
  --accent-strong: #2E5773;
  --highlight:     #C9A961;
  --success:       #5A8F6B;
  --warning:       #B07A3E;
  --danger:        #B0524A;

  /* Lines */
  --hairline:      rgba(42, 42, 46, 0.06);
  --border:        rgba(42, 42, 46, 0.10);
  --border-strong: rgba(42, 42, 46, 0.18);

  /* Shadows */
  --shadow-mist:   0 1px 2px rgba(42, 42, 46, 0.04);
  --shadow-soft:   0 4px 12px rgba(42, 42, 46, 0.06),
                   0 1px 2px rgba(42, 42, 46, 0.04);
  --shadow-modal:  0 24px 48px rgba(42, 42, 46, 0.12),
                   0 2px 6px rgba(42, 42, 46, 0.06);

  /* Radius */
  --radius-xs:  4px;
  --radius-sm:  8px;
  --radius-md:  10px;
  --radius-lg:  14px;
  --radius-pill: 999px;

  /* Spacing */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 24px;
  --space-6: 32px;
  --space-8: 48px;
  --space-10: 64px;

  /* Motion */
  --ease-out:  cubic-bezier(.2, .8, .2, 1);
  --ease-in:   cubic-bezier(.4, 0,  .9, .2);
  --ease-soft: cubic-bezier(.4, 0,  .2, 1);
  --dur-fast:  120ms;
  --dur-base:  180ms;
  --dur-slow:  240ms;

  /* Fonts */
  --font-sans:  'Inter', 'PingFang SC', 'Source Han Sans CN', -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
  --font-serif: 'Newsreader', 'Source Han Serif CN', Georgia, 'Songti SC', serif;
  --font-mono:  'JetBrains Mono', 'SF Mono', 'Fira Code', ui-monospace, monospace;
}

:root[data-theme="night"] {
  --canvas:        #1C1D1F;
  --surface:       #232427;
  --sunken:        #18191B;
  --hover:         #2D2E32;
  --active:        #34353A;

  --ink:           #E8E6DF;
  --ink-2:         #B0AEA6;
  --ink-3:         #7C7A73;
  --ink-4:         #4F4E48;

  --accent:        #7DA8C9;
  --accent-soft:   rgba(125, 168, 201, 0.14);
  --accent-strong: #9FBED7;
  --highlight:     #D9B97C;
  --success:       #8FB89A;
  --warning:       #D69963;
  --danger:        #D88078;

  --hairline:      rgba(232, 230, 223, 0.06);
  --border:        rgba(232, 230, 223, 0.10);
  --border-strong: rgba(232, 230, 223, 0.18);

  --shadow-mist:   0 1px 2px rgba(0, 0, 0, 0.20);
  --shadow-soft:   0 4px 12px rgba(0, 0, 0, 0.30),
                   0 1px 2px rgba(0, 0, 0, 0.20);
  --shadow-modal:  0 24px 48px rgba(0, 0, 0, 0.50),
                   0 2px 6px rgba(0, 0, 0, 0.30);
}

/* 跟随系统 */
@media (prefers-color-scheme: dark) {
  :root:not([data-theme]) {
    color-scheme: dark;
    /* 在此把 night token 复制一份；或在 JS 启动时设置 data-theme */
  }
}
```

---

## 12. 按钮工具类（直接复用）

```css
.btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: 32px;
  padding: 0 var(--space-3);
  font-size: 13px;
  font-weight: 500;
  line-height: 1;
  border-radius: var(--radius-sm);
  border: 1px solid transparent;
  cursor: pointer;
  transition: background var(--dur-fast) var(--ease-soft),
              color var(--dur-fast) var(--ease-soft),
              border-color var(--dur-fast) var(--ease-soft);
  user-select: none;
}

.btn-primary {
  background: var(--accent);
  color: #FFFFFE;
}
.btn-primary:hover  { background: var(--accent-strong); }
.btn-primary:active { background: var(--accent-strong); filter: brightness(.95); }

.btn-secondary {
  background: transparent;
  color: var(--ink);
  border-color: var(--border);
}
.btn-secondary:hover { background: var(--hover); }

.btn-ghost {
  background: transparent;
  color: var(--ink-3);
  padding: 0 var(--space-2);
}
.btn-ghost:hover { color: var(--ink); background: var(--hover); }

.btn-danger {
  background: transparent;
  color: var(--ink-3);
}
.btn-danger:hover  { color: var(--danger); background: rgba(176, 82, 74, 0.08); }
.btn-danger[data-confirming="true"] {
  color: var(--danger);
  border-color: var(--danger);
}

.btn-icon { width: 28px; padding: 0; justify-content: center; }
.btn-sm   { height: 28px; padding: 0 var(--space-2); font-size: 12px; }

.btn:disabled { opacity: .4; cursor: not-allowed; }
```

---

## 13. 取舍说明（写给未来的我）

| 我们选了 | 我们放弃了 | 原因 |
|---|---|---|
| Ink Light 默认 | 继续深紫罗兰 | 桌面工具一天用 8 小时，亮色更友好；夜间走 Ink Night 不丢深色用户 |
| 单一墨青 `#3A6B8C` | 之前的青/绿/紫多强调色 | 一处强调，否则视线没有落点 |
| 衬线品牌字 | 渐变文字 | 渐变文字是 Web3/Crypto 美学，与水墨格格不入 |
| 移除大量颜色徽标 | 把按钮做得花花绿绿 | 颜色是数据，不是装饰；当真的状态出现时才允许显色 |
| `⋯` 收纳删除 | 红色「删除」常驻 | 破坏性动作不该 primary-level |
| `hairline` 分割列表 | 圆角块 + 阴影 | 卡片堆叠会让密集列表看起来"像便利店货架"；分割线才是清单 |
| 不缩放的 hover | `scale(1.02)` 浮起 | 缩放在密集列表里抖布局 |
| 不用 emoji 图标 | 🔗/🗑/⚡ 之类 | emoji 渲染各系统差异大，跨平台后会变样 |

---

## 14. 验收 Checklist（落地后逐项过）

视觉：
- [ ] 默认 Ink Light，可手动 ☾/☀ 切换到 Ink Night
- [ ] 一屏只有一个 Primary 按钮（墨青）
- [ ] 列表行没有圆角块，用 hairline 分割
- [ ] 品牌字是衬线 + 落款圆点（**没有渐变**）
- [ ] 没有 emoji 图标
- [ ] Tab 下划线只覆盖文字宽度且滑动切换

交互：
- [ ] 所有按钮 cursor: pointer
- [ ] 删除按钮藏在 `⋯` 菜单且两阶段确认
- [ ] hover 不会让任何元素变形（无 scale）
- [ ] 焦点环可见（键盘 Tab 走一圈无丢失）
- [ ] ⌘1/2/3/4 切 Tab，⌘K 聚焦搜索，⌘\ 折叠侧栏

内容：
- [ ] 中英文之间有空格
- [ ] 字段标签去掉 `text-transform: uppercase`
- [ ] 没有混用全/半角标点

性能：
- [ ] 没有 `transform: scale()` 类 hover
- [ ] 滚动条 hover 才显形
- [ ] `prefers-reduced-motion` 用户全部动效降级

---

## 15. 灵感参考（截图/链接）

- **Things 3** — `https://culturedcode.com/things/`
  借鉴：左侧栏密度、空态文案、一处强调。
- **Notion 中文站** — `https://www.notion.com/zh-cn`
  借鉴：纸感配色、衬线大标题、低饱和度。
- **iA Writer** — `https://ia.net/writer`
  借鉴：等宽 + 衬线的搭配节奏；专注模式。
- **Reeder 5** — `https://reederapp.com/`
  借鉴：列表分章节感、阅读体的明亮亮色模式。
- **Cron / Notion Calendar** — `https://www.notion.com/product/calendar`
  借鉴：Modal 阴影、按钮三层级。

---

> 文档版本：v1.0 / 起草于 2026-05-15
> 维护：本文 + `src/theme.css` 必须同步；改 token 走文档评审。

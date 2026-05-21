# Agent Hub — Visual Design Spec v1.1

> 适用范围：Skill 列表页 + Skill 详情页升级。
> 基线：沿用 `src/theme.css` 中的 Ink Light v1.0 token，本文只新增/扩展，不破坏既有规则。
> 实现优先级：列表页 KPI/表格/分页 > 详情页 Hero/元数据/文件列表。

---

## § 1 设计原则

1. **沿用 Ink Light**：水墨白底（`--canvas` #F8F6F1）+ 远山青强调色（`--accent` #3A6B8C）+ 矿物金高亮（`--highlight` #C9A961）。本次升级不引入新色相。
2. **从"密集列表"转向"信息卡片 + 数据表格"**：旧版偏 Things 3 灵感的纯列表；新版加入"仪表盘"维度（顶部 KPI），把单页信息层级显式化。
3. **节奏感优先于装饰**：所有新组件靠 *间距、对齐、字阶* 拉开层级，不靠投影/渐变；阴影只用 `--shadow-mist`，最重不超过 `--shadow-soft`。
4. **表格 > 卡片网格**：列表页改用密度更高的 Data Table；移动端再退回卡片栈（>768px 表格）。
5. **避免破坏既有 utility class**：所有新样式走新 CSS 类前缀 `.ah-`（Agent Hub），不再叠加 Tailwind 颜色 override。

---

## § 2 新增 Token

追加到 `:root` 与 `:root[data-theme="night"]`：

### 2.1 间距与尺寸

| Token | 值 | 用途 |
|---|---|---|
| `--space-1` | `4px` | 微间距 |
| `--space-2` | `8px` | 紧凑栈 |
| `--space-3` | `12px` | 卡片内 padding 底层 |
| `--space-4` | `16px` | 卡片间距 / 列表行 padding |
| `--space-5` | `20px` | 卡片内 padding 主层 |
| `--space-6` | `24px` | 区块间距 |
| `--space-8` | `32px` | 页面主纵向节奏 |
| `--space-10` | `40px` | 页面外边距 |
| `--ah-card-pad` | `20px` | KPI/元数据卡内 padding |
| `--ah-row-h` | `60px` | 表格行高 |
| `--ah-icon-chip-lg` | `48px` | KPI/Hero 图标 chip 边长 |
| `--ah-icon-chip-md` | `36px` | 元数据卡图标 chip 边长 |
| `--ah-icon-chip-sm` | `32px` | 表格行头像 |

### 2.2 字阶（语义命名）

| Token | size / weight / family | 用途 |
|---|---|---|
| `--fs-display` | `28px / 600 / serif` | 详情页 Hero 标题 |
| `--fs-h1` | `22px / 600 / serif` | 列表页页面标题 |
| `--fs-h2` | `16px / 600 / sans` | 区块标题（"文件列表" / "描述"） |
| `--fs-kpi` | `28px / 600 / serif` | KPI 数字 |
| `--fs-kpi-unit` | `13px / 400 / sans` | KPI 单位（"个技能"） |
| `--fs-label` | `12px / 500 / sans` | 卡片小标签、表头 |
| `--fs-body` | `13.5px / 400 / sans` | 表格行主文本、描述 |
| `--fs-meta` | `12.5px / 400 / mono` | 大小、路径、版本号 |

### 2.3 Avatar 色板（8 色，§5 详述）

```css
--ah-avatar-1: #6B8FA8;  /* 远山青 */
--ah-avatar-2: #8A9A7B;  /* 苔藓绿 */
--ah-avatar-3: #B07A3E;  /* 赭石 */
--ah-avatar-4: #A88B6B;  /* 沙褐 */
--ah-avatar-5: #7B7B8E;  /* 烟紫 */
--ah-avatar-6: #C9A961;  /* 矿物金 */
--ah-avatar-7: #5A8F8B;  /* 青瓷 */
--ah-avatar-8: #9C6F7E;  /* 绛绯 */
```

每色对应的 soft 背景：取 `rgba(同色, 0.12)`；图标本身用同色 100% 不透明。

夜间模式：所有 avatar 色保持色相，明度向上拉 6–10%，soft 背景透明度提到 0.18。

---

## § 3 组件规范

### 3.1 KPI Card（统计卡）

**用途**：列表页顶部 4 张概览卡，传达"全局健康度"。

**结构**：

```
┌─────────────────────────────────────────┐
│  [chip]      技能总数                   │  ← label, --fs-label, --ink-3
│  48×48                                  │
│              121      个技能            │  ← number(--fs-kpi) + unit(--fs-kpi-unit, --ink-3)
└─────────────────────────────────────────┘
```

```html
<article class="ah-kpi">
  <div class="ah-kpi__chip" data-tone="accent">
    <svg class="icon-grid" />
  </div>
  <div class="ah-kpi__body">
    <p class="ah-kpi__label">技能总数</p>
    <p class="ah-kpi__value">
      <span class="ah-kpi__num">121</span>
      <span class="ah-kpi__unit">个技能</span>
    </p>
  </div>
</article>
```

**规格**：

| 项 | 值 |
|---|---|
| 容器背景 | `--surface` |
| 边框 | `1px solid --hairline` |
| 圆角 | `--radius-md` (10px) |
| 阴影 | `--shadow-mist` |
| Padding | `--ah-card-pad` (20px) 全四向 |
| 内栈方向 | 横向：chip 在左，body 在右 |
| chip-body gap | `--space-4` (16px) |
| chip 尺寸 | 48×48，圆角 `--radius-sm` (8px) |
| chip 背景 | tone 对应的 soft（如 `--accent-soft`、`--success-soft`、`--warning-soft`、`--highlight-soft`） |
| chip 图标颜色 | 对应 tone 主色 |
| chip 图标尺寸 | 22px |
| label 与 value 间距 | `--space-2` (8px) |
| number 与 unit 间距 | `--space-2` (8px) |
| label 颜色 | `--ink-3` |
| number 颜色 | `--ink`，font `--font-serif` |
| unit 颜色 | `--ink-3` |

**4 张的 tone 分配**（固定，不哈希）：

| 卡 | tone | chip 图标 |
|---|---|---|
| 技能总数 | `accent` | grid / squares |
| 启用中 | `success` | code-brackets `<>` |
| 最近更新 | `warning` | clock |
| 总大小 | `highlight` | smile/dot |

**状态**：

- hover：边框 `--border`（更深一档），阴影 `--shadow-soft`，过渡 `--dur-base var(--ease-soft)`。
- 数字加载占位：用 `loading-pulse` 类（已存在），把 number 文本替换为 24×60px 灰块。

---

### 3.2 Data Table（数据表格）

**用途**：列表页主区域；垂直密度优先。

**结构**：

```
┌──── 技能名称 ─────────── 描述 ───────────────── 大小 ⇅ ──── ⋯ ─┐
│ ◯  1password           Set up and use 1Password CLI...   4 KB    ⋯ │
│ ◯  Agent Browser       A fast Rust-based headless ...    12 KB   ⋯ │
│ ●  Code   v1.0.4       Coding workflow with planning...  9 KB    ⋯ │ ← selected
│ ◯  FFmpeg Video Editor Generate FFmpeg commands ...      10 KB   ⋯ │
└─────────────────────────────────────────────────────────────────────┘
```

**布局**：CSS Grid，列模板：

```css
grid-template-columns:
  minmax(220px, 1.4fr)     /* name */
  minmax(280px, 2.4fr)     /* desc */
  120px                    /* size */
  40px;                    /* kebab */
column-gap: var(--space-4);
```

#### 3.2.1 表头 `.ah-thead`

| 项 | 值 |
|---|---|
| 高度 | 44px |
| 背景 | 透明 |
| 下边框 | `1px solid --border` |
| 字体 | `--fs-label`，weight 500，`--ink-3` |
| Padding L/R | 与 row 对齐：左 16px，右 12px |
| 可排序列 | 文本右侧 4px 加入排序箭头图标 ⇅，hover 显示完整排序态 |
| 排序激活 | 文本变 `--ink`，箭头改 ↑ 或 ↓ |

#### 3.2.2 表行 `.ah-row`

| 项 | 值 |
|---|---|
| 高度 | `--ah-row-h` (60px) |
| 下边框 | `1px solid --hairline` |
| Padding | `0 12px 0 16px` |
| 列对齐 | name/desc 左对齐居中；size 右对齐居中；kebab 居中 |
| Hover 背景 | `--hover` |
| Selected | 左 2px `--accent` 实线，背景 `--accent-soft` |
| 过渡 | `background --dur-fast var(--ease-soft)` |

##### Name 列

- Avatar 32×32 圆形（`border-radius: 50%`），soft 背景 + 主色图标 18px。
- 名称：`--fs-body`，weight 500，`--ink`。
- 版本号 chip（可选）：紧跟名称，`padding: 1px 6px`，`background: --sunken`，`border-radius: --radius-pill`，`font: 11px mono`，`color: --ink-3`，`margin-left: 8px`。

##### Description 列

- `--fs-body`，`--ink-2`，单行 ellipsis：

```css
.ah-row__desc {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
```

##### Size 列

- `--fs-meta`，`--ink-3`，右对齐，固定宽 120px。

##### Kebab 列

- 28×28 ghost 按钮，图标 `more-vertical` 16px，`--ink-4`。
- hover：`--hover` 背景 + `--ink-2` 颜色。
- 点击：弹出下拉菜单（菜单本身复用现有 modal token 即可，本规范不展开）。

**Empty State**（无搜索结果）：表头保留，下方一个 240px 高的居中 `.empty-state`（既有类）。

---

### 3.3 Pagination

**结构**：

```
共 121 项                  ◁  1  2  3  4  …  13  ▷               [ 10 条/页 ▾ ]
```

```html
<nav class="ah-pagination">
  <span class="ah-pagination__total">共 121 项</span>
  <ul class="ah-pagination__pages">
    <li><button class="ah-page-btn" aria-label="上一页">‹</button></li>
    <li><button class="ah-page-btn is-active">1</button></li>
    <li><button class="ah-page-btn">2</button></li>
    <li><span class="ah-page-ellipsis">…</span></li>
    <li><button class="ah-page-btn">13</button></li>
    <li><button class="ah-page-btn" aria-label="下一页">›</button></li>
  </ul>
  <div class="ah-pagination__size">
    <select>10 条/页</select>
  </div>
</nav>
```

**规格**：

| 项 | 值 |
|---|---|
| 容器 | 横向 flex，space-between，高度 48px |
| `__total` | `--fs-body`，`--ink-3` |
| `.ah-page-btn` | 28×28，`--radius-pill`，背景透明，`--ink-2`，font 13/500 |
| `.ah-page-btn:hover` | `--hover` 背景，`--ink` |
| `.ah-page-btn.is-active` | `--accent` 实心背景，白字，`--shadow-mist` |
| 上/下页禁用 | opacity 0.35, cursor not-allowed |
| 页码间隔 | gap `--space-1` (4px) |
| 省略号 | 28×28 居中 `--ink-4` |
| size select | 复用 `#session-terminal-select` 风格：`--sunken` 底，`--border` 边，`--radius-sm` |

---

### 3.4 Page Header（页面标题栏）

**用途**：列表页顶部的"Shared Pool"主标题区域；位于 toolbar 之下、KPI 之上。

**结构**：

```
Shared Pool                                                        [ ⚠ 检查 ]
```

| 项 | 值 |
|---|---|
| 容器 | 横向 flex，`align-items: center`，`justify-content: space-between` |
| 容器 padding | `0` 横向 + `0 0 --space-5 0` 底（与下方 KPI 留 20px） |
| 标题字体 | `--fs-h1` (22px serif 600)，`--ink` |
| 主按钮 | `.btn.btn-primary` 既有；icon-leading 三角警示 14px |
| 副标题（可选） | 标题下 4px，`--fs-meta`，`--ink-3`，用于显示当前 Platform 路径 |

注：当前的 `#toolbar` 保留作为顶部全局工具条；本组件是 *页面内* 标题，居于内容区第一行。两者不冲突。

---

### 3.5 Detail Hero

**用途**：详情页第一屏，给出 skill 的"身份卡"。

**结构**：

```
┌──────────────────────────────────────────────────────────────┐
│ ┌────┐                                                       │
│ │ ⟨⟩ │   Code                                                │  ← display
│ │ 64 │   Coding workflow with planning, implementation...    │  ← subtitle
│ └────┘                                                       │
└──────────────────────────────────────────────────────────────┘
```

```html
<header class="ah-hero">
  <div class="ah-hero__icon" data-tone="accent">
    <svg class="icon-code" />
  </div>
  <div class="ah-hero__text">
    <h1 class="ah-hero__title">Code</h1>
    <p class="ah-hero__subtitle">Coding workflow with planning, implementation, verification, and testing for clean software development.</p>
  </div>
</header>
```

| 项 | 值 |
|---|---|
| 容器背景 | `--surface` |
| 边框 | `1px solid --hairline` |
| 圆角 | `--radius-lg` (14px) |
| 阴影 | `--shadow-mist` |
| Padding | `24px 28px` |
| 内栈 | 横向，icon-text gap `--space-5` (20px) |
| icon chip | 64×64（注意比 KPI 大一档），圆角 `--radius-md`，背景 tone soft，图标 32px |
| 标题 | `--fs-display` (28px serif 600)，`--ink`，行高 1.25 |
| 副标题 | `--fs-body`，`--ink-2`，最多 2 行，`-webkit-line-clamp: 2` |
| 标题与副标题间距 | `--space-2` (8px) |

---

### 3.6 Metadata Card Row（元数据卡片行）

**用途**：详情页 Hero 之下，5 张并列的元信息卡。

**布局**：

- 5 列等宽 Grid：`grid-template-columns: repeat(5, minmax(0, 1fr));`
- 列间距 `--space-3` (12px)
- 行外边距：`margin: --space-5 0 --space-6 0`

**单卡结构**：

```
┌─────────────────────┐
│  [icon]             │  ← chip 36×36, top-left
│                     │
│  平台               │  ← label
│  shared-pool        │  ← value
└─────────────────────┘
```

```html
<article class="ah-meta">
  <div class="ah-meta__chip"><svg /></div>
  <p class="ah-meta__label">平台</p>
  <p class="ah-meta__value">shared-pool</p>
</article>
```

| 项 | 值 |
|---|---|
| 背景 | `--surface` |
| 边框 | `1px solid --hairline` |
| 圆角 | `--radius-md` |
| 阴影 | `--shadow-mist` |
| Padding | `16px` |
| chip | 36×36，圆角 `--radius-sm`，背景 `--sunken`，图标 18px `--ink-2`（中性色，不与 KPI 抢眼） |
| chip 下间距 | `--space-3` (12px) |
| label | `--fs-label`，`--ink-3` |
| value | `--fs-body` (13.5px)，`--ink`，weight 500；溢出 ellipsis 单行 |

**5 张固定语义**：

| 序 | label | value 来源 | 图标 |
|---|---|---|---|
| 1 | 平台 | `platform.name` | layers |
| 2 | 路径 | `skill.path`（绝对路径，必要时 tooltip） | folder |
| 3 | 版本 | `skill.version`（无则 "—"） | tag |
| 4 | 大小 | 格式化字节 | cube |
| 5 | 文件数 | `files.length + " 个文件"` | file |

**响应式**：宽度 < 960px 时切换为 2 列 + 末行自适应；< 600px 时 1 列。

---

### 3.7 File List

**用途**：详情页"文件列表"区块。

**结构**：

```
文件列表
┌───────────────────────────────────────────────────────────┐
│  📄  SKILL.md                                     1.2 KB  │
│  📄  _meta.json                                   0.8 KB  │
│  📄  criteria.md                                  1.3 KB  │
│  📄  execution.md                                 1.6 KB  │ ← hover/active
│  ...                                                       │
└───────────────────────────────────────────────────────────┘
```

```html
<section class="ah-files">
  <h2 class="ah-section-title">文件列表</h2>
  <ul class="ah-files__list">
    <li class="ah-file">
      <svg class="ah-file__icon" />
      <span class="ah-file__name">SKILL.md</span>
      <span class="ah-file__size">1.2 KB</span>
    </li>
    ...
  </ul>
</section>
```

| 项 | 值 |
|---|---|
| 区块标题 `.ah-section-title` | `--fs-h2` (16px sans 600)，`--ink`，下间距 `--space-3` |
| 列表容器 | 背景 `--surface`，边框 `1px solid --hairline`，圆角 `--radius-md`，溢出 hidden |
| 单行 `.ah-file` | grid: `24px 1fr auto`，gap `--space-3`，高 44px，padding `0 16px` |
| 行间分隔 | 内部行之间 `1px solid --hairline`，首行无 |
| icon | 18px `--ink-3`，根据扩展名换图标（md/json/default） |
| name | `--fs-meta` (mono 12.5px)，`--accent`（保留既有 file-item 颜色继承） |
| size | `--fs-meta`，`--ink-3`，右对齐 |
| hover | 背景 `--hover`，name 颜色 `--accent-strong` |
| 点击 | 触发既有 `loadFile()` 逻辑，无视觉态变化（详情下方文件查看器响应） |

---

### 3.8 Detail Toolbar（详情页顶部工具条）

复用既有 `#toolbar` 容器，但调整内部布局：

```
[‹ 返回 / Code]                              [⚠ 检查] [对比] [同步]
```

| 项 | 值 |
|---|---|
| 容器 padding | 沿用 `8px 18px` |
| 面包屑 | `--fs-body`，前缀图标 14px `--ink-3`，"返回" `--ink-3` 可点，分隔符 ` / `，当前 skill 名 `--ink` weight 500 |
| 按钮组间距 | gap `--space-2` (8px) |
| 检查 | `.btn.btn-primary`，含警示图标 |
| 对比 / 同步 | `.btn.btn-secondary` |

---

## § 4 页面布局

### 4.1 公共容器

- 内容主区域（`<main>` 内部）最大宽度：`1200px`。
- 水平外边距：`max(24px, (100vw - 1200px) / 2)`。
- 顶部 `#toolbar` 已粘性；其下内容容器 padding-top `--space-6` (24px)。

### 4.2 Skill 列表页

纵向节奏（自上而下）：

```
#toolbar                              [既有，44px 高]
├── --space-6 (24px) ──
Page Header（标题 + 检查按钮）         [§3.4]
├── --space-5 (20px) ──
KPI Row（4 张 statc cards）           [§3.1]
├── --space-6 (24px) ──
Data Table                            [§3.2]
├── --space-3 (12px) ──
Pagination                            [§3.3]
├── --space-8 (32px) ──（页尾留白）
```

KPI Row Grid：`grid-template-columns: repeat(4, minmax(0, 1fr)); column-gap: --space-4;`

响应式：

- ≥ 1080px：4 列
- 720–1079px：2 列 2 行
- < 720px：单列；表格降级为卡片栈（每行变成"Name + 描述堆叠 + 底部 meta 行"），分页改为简化版（只保留上/下页 + 当前页/总页）

### 4.3 Skill 详情页

纵向节奏：

```
#toolbar  [§3.8]
├── --space-6 (24px) ──
Detail Hero                           [§3.5]
├── --space-5 (20px) ──
Metadata Card Row（5 张）              [§3.6]
├── --space-6 (24px) ──
文件列表                              [§3.7]
├── --space-6 (24px) ──
描述（plain prose）
├── --space-8 (32px) ──
```

**描述区块**：

| 项 | 值 |
|---|---|
| 区块标题 | `.ah-section-title`（"描述"，同 §3.7 风格） |
| 容器 | 无卡片包裹，纯段落 |
| 段落字体 | `--fs-body`，`--ink-2`，行高 1.7 |
| 段落最大宽度 | `72ch` |

### 4.4 全局断点

| 名 | 宽度 | 切换内容 |
|---|---|---|
| `--bp-md` | 768px | 表格 → 卡片栈；元数据 5 列 → 2 列 |
| `--bp-lg` | 1080px | KPI 2 列 → 4 列 |
| `--bp-xl` | 1280px | （预留） |

---

## § 5 Avatar Chip 色板分配规则

**目的**：列表每行的圆形头像需要"有辨识度但不喧宾夺主"。

**约束**：

- 8 色固定（§2.3 中 `--ah-avatar-1` ~ `--ah-avatar-8`）。
- 全部低饱和、偏冷或大地色，与水墨主调融洽；HSL 饱和度区间 `18%–32%`，亮度 `42%–56%`。
- 禁止霓虹色、纯红、亮蓝、品红、青柠。

**分配函数**（前端实现，纯函数）：

```ts
function avatarToneFromName(name: string): 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 {
  // FNV-1a 32-bit
  let h = 2166136261 >>> 0;
  for (let i = 0; i < name.length; i++) {
    h ^= name.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return ((h % 8) + 1) as 1|2|3|4|5|6|7|8;
}
```

- 输入：skill 名（不含 platform 前缀，小写归一化后再 hash）。
- 输出：`1..8`，对应 `--ah-avatar-${n}`。
- 同名 skill 跨刷新结果稳定。
- 首字母也作为 fallback 显示（图标库未匹配时，chip 内显示首字母大写，字号 14，weight 600，颜色为对应 tone 主色）。

**视觉对照**：

| Tone | HEX | 用途示例 |
|---|---|---|
| 1 远山青 | `#6B8FA8` | 默认/技术类 |
| 2 苔藓绿 | `#8A9A7B` | 生产力/笔记 |
| 3 赭石 | `#B07A3E` | 创意/写作 |
| 4 沙褐 | `#A88B6B` | 文档 |
| 5 烟紫 | `#7B7B8E` | AI/思考 |
| 6 矿物金 | `#C9A961` | 商业/分析 |
| 7 青瓷 | `#5A8F8B` | 工具/utility |
| 8 绛绯 | `#9C6F7E` | 社交/媒体 |

---

## § 6 与既有 Token 的复用 / 冲突清单

### 6.1 复用（直接用现有 token，无需新增）

| 现有 | 在新组件中的角色 |
|---|---|
| `--canvas` | 页面背景 |
| `--surface` | KPI / Hero / 元数据卡 / 文件列表容器底 |
| `--sunken` | 元数据 chip 背景 / 版本号 chip 背景 / select 背景 |
| `--hover` | 表格行 hover / 文件行 hover / 分页按钮 hover |
| `--ink` / `--ink-2` / `--ink-3` / `--ink-4` | 全套文本层级 |
| `--accent` / `--accent-soft` / `--accent-strong` | 主按钮、选中态、图标 chip 主 tone |
| `--highlight` / `--highlight-soft` | KPI "总大小"卡 tone |
| `--success` / `--warning` / `--danger` 系列 | KPI 其余三张 + 状态徽标 |
| `--hairline` / `--border` | 卡边框 / 表格分隔 |
| `--shadow-mist` / `--shadow-soft` | 卡片层级 |
| `--radius-sm` / `--radius-md` / `--radius-lg` / `--radius-pill` | 各组件圆角 |
| `--font-sans` / `--font-serif` / `--font-mono` | 字体三轨 |
| `--dur-fast` / `--dur-base` / `--ease-soft` / `--ease-out` | 过渡 |
| `.btn` / `.btn-primary` / `.btn-secondary` / `.btn-ghost` | 按钮系 |
| `#toolbar` / `#btn-back` | 详情页工具条骨架 |

### 6.2 新增（§2 已列）

- 间距阶梯 `--space-1..10` + 组件级 `--ah-card-pad` / `--ah-row-h` / `--ah-icon-chip-*`。
- 字阶语义命名 `--fs-display/h1/h2/kpi/kpi-unit/label/body/meta`。
- Avatar 8 色 `--ah-avatar-1..8`。
- 类前缀 `.ah-*`。

### 6.3 冲突 / 注意点

| 冲突点 | 处理 |
|---|---|
| 旧 `#view-skills` 内 padding 16px 与新 Page Header 节奏不一致 | 把 `#view-skills` padding 改为 `--space-6` 顶部 + 水平复用容器策略；旧 `.flex.items-center.rounded` 行样式整体废弃，改用新 `.ah-row`。 |
| 旧 `.skill-item` / `.skill-delete-btn` | 保留 CSS 直到旧视图全部下线；新视图不复用这两个类。 |
| 旧 `#view-detail h2` serif 24px | 升级为 `--fs-display` 28px，并改放进 `.ah-hero__title`。原 `.text-yellow-400` 标签全部废弃，由 `.ah-meta__label` 取代。 |
| 旧 `.file-item` | 沿用 mono + accent 色逻辑，但移除 padding/radius，改由父 `.ah-file` 网格控制；保留 `.file-item` 作为可点击锚点别名即可。 |
| Tailwind 颜色 utility 残留 | 不为新组件添加 Tailwind 颜色类；保持 `theme.css` override 范围不扩张。 |
| 暗色模式 | 所有新 token 均需在 `:root[data-theme="night"]` 重写明度（KPI tone soft 透明度提到 0.18；avatar 色明度 +6–10%；阴影沿用既有夜间阴影）。 |
| 表格列宽与窄屏 | 见 §4.4 断点；< 768px 必须降级到卡片栈，否则 4 列 Grid 会挤压描述列 |

---

## § 7 实现交付节奏（建议）

1. **PR-1（基础设施）**：把 §2 token、§5 avatar 工具函数加入 `theme.css` + 一个 `src/js/avatar.js`。无视觉变化。
2. **PR-2（列表页）**：实现 §3.1 / §3.2 / §3.3 / §3.4 + §4.2 布局。
3. **PR-3（详情页）**：实现 §3.5 / §3.6 / §3.7 / §3.8 + §4.3 布局。
4. **PR-4（响应式与暗色）**：§4.4 断点 + 夜间模式微调。
5. **PR-5（清理）**：删除 §6.3 中标记废弃的旧类与对应 JS 渲染分支。

每个 PR 都可单独 ship，互不阻塞。

---

附：相关源文件
- 现有 token：[src/theme.css](src/theme.css)
- 现有列表渲染入口：[src/js/app.js](src/js/app.js)
- 项目结构总览：[docs/DESIGN.md](docs/DESIGN.md)

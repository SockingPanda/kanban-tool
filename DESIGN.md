---
name: "kanban-tool"
description: "用于检查、运行和维护持久工作项的安静本地工作台。"
colors:
  background-surface: "light-dark(#ffffff, #262626)"
  background-body: "light-dark(#f1f1f1, #1b1b1b)"
  background-card: "light-dark(#ffffff, #1b1b1b)"
  background-popover: "light-dark(#ffffff, #1b1b1b)"
  background-muted: "light-dark(#f1f1f1, #1b1b1b)"
  accent-graphite: "light-dark(#262626, #ebebeb)"
  accent-muted: "light-dark(#f1f1f1, #262626)"
  neutral: "light-dark(#0000000F, #FFFFFF1A)"
  overlay: "light-dark(#00000080, #000000CC)"
  overlay-hover: "light-dark(#0000000D, #FFFFFF0D)"
  overlay-pressed: "light-dark(#0000001A, #FFFFFF1A)"
  text-primary: "light-dark(#171717, #fafafa)"
  text-secondary: "light-dark(#525252, #a3a3a3)"
  text-disabled: "light-dark(#a3a3a3, #525252)"
  text-accent: "light-dark(#262626, #ebebeb)"
  on-accent: "light-dark(#ffffff, #171717)"
  success: "light-dark(#007004, #9fe59b)"
  error: "light-dark(#a50c25, #ffc6c1)"
  warning: "light-dark(#745b00, #fdcf4f)"
  success-muted: "light-dark(#c5e5c0, #84c9803D)"
  error-muted: "light-dark(#facecb, #ff9e973D)"
  warning-muted: "light-dark(#f8da9d, #deb4333D)"
  border: "light-dark(#00000014, #FFFFFF1A)"
  border-emphasized: "light-dark(#d4d4d4, #525252)"
  skeleton: "light-dark(#ebebeb, #525252)"
  background-gray: "light-dark(#e5e5e5, var(--color-neutral))"
  text-gray: "light-dark(#262626, #e5e5e5)"
  background-blue: "light-dark(#c4ddfb, #9eb7ff3D)"
  text-blue: "light-dark(#00458c, #c7d3ff)"
  info-fill: "light-dark(#0074e2, #6d9cfe)"
  info-on: "light-dark(#ffffff, #171717)"
  success-fill: "light-dark(#198100, #64af4c)"
  success-on: "light-dark(#ffffff, #171717)"
  warning-fill: "#ffce2f"
  warning-on: "#171717"
  error-fill: "light-dark(#e33f4a, #ff705d)"
  error-on: "light-dark(#ffffff, #171717)"
typography:
  display:
    fontFamily: "Figtree, -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif"
    fontSize: "2.625rem"
    fontWeight: 400
    lineHeight: 1.2381
  headline:
    fontFamily: "Figtree, -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 600
    lineHeight: 1.3333
  title:
    fontFamily: "Figtree, -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif"
    fontSize: "1.25rem"
    fontWeight: 600
    lineHeight: 1.4
  body:
    fontFamily: "Figtree, -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.4286
  label:
    fontFamily: "Figtree, -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 500
    lineHeight: 1.4286
  code:
    fontFamily: "ui-monospace, \"SF Mono\", Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.4286
rounded:
  none: "0.25rem"
  inner: "0.375rem"
  element: "0.625rem"
  container: "0.75rem"
  page: "1.75rem"
  full: "9999px"
spacing:
  spacing-0: "0px"
  spacing-0-5: "2px"
  spacing-1: "4px"
  spacing-1-5: "6px"
  spacing-2: "8px"
  spacing-3: "12px"
  spacing-4: "16px"
  spacing-5: "20px"
  spacing-6: "24px"
  spacing-8: "32px"
  spacing-10: "40px"
  spacing-12: "48px"
components:
  button-primary:
    backgroundColor: "{colors.accent-graphite}"
    textColor: "{colors.on-accent}"
    typography: "{typography.label}"
    rounded: "{rounded.element}"
    padding: "{spacing.spacing-2} {spacing.spacing-3}"
    size: "md"
  button-secondary:
    backgroundColor: "{colors.background-surface}"
    textColor: "{colors.text-primary}"
    typography: "{typography.label}"
    rounded: "{rounded.element}"
    padding: "{spacing.spacing-2} {spacing.spacing-3}"
    size: "md"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.accent-graphite}"
    typography: "{typography.label}"
    rounded: "{rounded.element}"
    padding: "{spacing.spacing-2} {spacing.spacing-3}"
    size: "md"
  button-destructive:
    backgroundColor: "{colors.error-muted}"
    textColor: "{colors.error}"
    typography: "{typography.label}"
    rounded: "{rounded.element}"
    padding: "{spacing.spacing-2} {spacing.spacing-3}"
    size: "md"
  badge-neutral:
    backgroundColor: "{colors.background-gray}"
    textColor: "{colors.text-gray}"
    typography: "{typography.code}"
    rounded: "{rounded.full}"
    padding: "{spacing.spacing-1} {spacing.spacing-2}"
    height: "20px"
  badge-status:
    backgroundColor: "{colors.info-fill}"
    textColor: "{colors.info-on}"
    typography: "{typography.code}"
    rounded: "{rounded.full}"
    padding: "{spacing.spacing-1} {spacing.spacing-2}"
    height: "20px"
  input-field:
    backgroundColor: "{colors.background-surface}"
    textColor: "{colors.text-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.element}"
    padding: "{spacing.spacing-1} {spacing.spacing-2}"
    height: "2.25rem"
  navigation-item:
    backgroundColor: "transparent"
    textColor: "{colors.text-secondary}"
    typography: "{typography.label}"
    rounded: "{rounded.inner}"
    padding: "{spacing.spacing-2} {spacing.spacing-3}"
    height: "2.25rem"
  card-task:
    backgroundColor: "{colors.background-card}"
    textColor: "{colors.text-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.container}"
    padding: "{spacing.spacing-3}"
  task-inspector:
    backgroundColor: "{colors.background-surface}"
    textColor: "{colors.text-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.none}"
    padding: "{spacing.spacing-4}"
    width: "24rem"
  task-list-table:
    backgroundColor: "{colors.background-surface}"
    textColor: "{colors.text-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.container}"
    padding: "{spacing.spacing-2} {spacing.spacing-3}"
  mutation-dialog:
    backgroundColor: "{colors.background-popover}"
    textColor: "{colors.text-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.container}"
    padding: "{spacing.spacing-5}"
    width: "32rem"
---

# Design System: kanban-tool

## Overview

**Creative North Star: "The Quiet Local Workbench"**

`kanban-tool` 是一个本地优先的操作界面，用于检查、认领、运行和维护持久工作项。视觉语言应当平静、精确、紧凑：信息近在手边，操作含义明确，界面为证据留出空间而不是堆叠装饰。当前实现采用 Astryx `neutral` 主题 `0.3.0`，以石墨强调色、暖灰画布、白色工作表面和安静的细线边框为基线。

这个系统是一次只由一个人操作的工作台，不是协作动态流，也不是营销海报。任务事实、标识符、就绪信号和变更结果通过层次、间距和语义状态颜色保持清晰。已批准的 Plane/cobalt mockup 只是未来视觉参考；它们不会给当前 Astryx neutral 基线增加 cobalt token。

**Key Characteristics:**

- 安静的本地优先操作界面，操作入口明确。
- 暖灰画布与白色表面上的石墨中性色对比。
- 证据优先的层次：任务事实、标识符与状态始终可见。
- 静止表面保持扁平；只有必须浮起的层或需要确认的状态才使用深度。

## Colors

配色以石墨中性为主轴，搭配暖灰画布、白色工作表面和克制的状态语义色；它不是第二套强调色系统。

### Primary

- **Graphite Accent** (`--color-accent`)：唯一用于操作、焦点、选中标签和活动指示器的信号。

### Neutral

- **Warm Gray Canvas** (`--color-background-body`)：页面与 shell 背景，让白色工作表面保持清晰。
- **Working Surface** (`--color-background-surface`)：需要清晰前景的控件、app shell 区域、inspector 和 dialog。
- **Card Surface** (`--color-background-card`)：task 与内容 card；静止时保持安静。
- **Popover Surface** (`--color-background-popover`)：临时菜单与确认 layer。
- **Primary Graphite Text** (`--color-text-primary`)：标题、task title 和高优先级事实。
- **Secondary Graphite Text** (`--color-text-secondary`)：辅助文案、metadata 和安静的 navigation label。
- **Hairline Border** (`--color-border`)：分隔线与低强调边界。
- **Emphasized Border** (`--color-border-emphasized`)：需要清晰边缘的控件、table 和 field。

### Semantic status

`--color-success`、`--color-warning` 和 `--color-error` 表达系统状态与合法结果。`--color-background-blue`、`--color-text-blue` 等分类徽章颜色用于数据分类；它们不会替代石墨操作强调色。

### Named Rules

**The One Accent Rule.** 使用 `--color-accent` 表达操作和焦点。success、warning、error 与分类颜色只保留给各自的语义；不要把已批准的未来 cobalt mock 引入当前操作 token。

## Typography

**展示字体：** Figtree（使用 `--font-family-heading` 中的系统无衬线 fallback）

**正文字体：** Figtree（使用 `--font-family-body` 中的系统无衬线 fallback）

**标签/等宽字体：** `ui-monospace`，使用 Astryx `--font-family-code` 中的 `SF Mono`/Monaco/Consolas fallback

**气质：** Figtree 让工作台保持易读而克制的人性化观感，紧凑的 14px 基础字号比例支持密集证据。代码字体栈则有意保持工具化：机器事实应当一眼可识别，但不让整个界面变成 terminal。

### Hierarchy

- **Display** (`--text-display-1-*`, normal)：只用于稀疏的页面级引导。
- **Headline** (`--text-heading-1-*`, semibold)：页面标题与主要 section heading。
- **Title** (`--text-heading-2-*`, semibold)：board、inspector 与 panel heading。
- **Body** (`--text-body-*`, normal)：task description、解释与辅助文字。
- **Label** (`--text-label-*`, medium)：button label、field label 与紧凑 navigation。
- **Code** (`--text-code-*`, normal)：ID、ref、hash、timestamp 等面向机器的事实。

### Named Rules

**The Identifier Rule.** task ID、ref、hash、timestamp 和 code excerpt 必须使用 `--font-family-code` 字体栈；解释性文字保持 Figtree，让证据与解释在视觉上清晰分开。

## Layout

shell 是一个 elevated Astryx `AppShell`，包含可折叠的 `SideNav` 和单一内容区域。`LayoutContent` 使用已观察到的 `padding={6}` 间距节奏，页面内容限制在 `72rem` 以内，避免密集 evidence 变成难以阅读的文字墙。在宽屏保持 navigation 常驻，并在工作表面失去有效宽度之前折叠它。

board 使用横向状态列，track 在 `18rem` 到 `24rem` 之间，column gap 为 `1rem`，board 间距节奏为 `1.25rem`。Explorer view 使用 primary-content-plus-inspector grid（`minmax(0, 1fr)` 加 `minmax(18rem, 24rem)`）；到 `68rem` 时收窄 inspector，`56rem` 时切为单列。Settings 在 `64rem` 以下从三列变为两列，在 `42rem` 以下变为一列。Table 保持 `50rem` 的最小宽度并允许滚动，不压缩标识符。

使用 Astryx spacing scale（`--spacing-0` 至 `--spacing-12`）作为共享间距节奏。小控件与 metadata 可以使用较低 step；card、panel 和 dialog 使用中间 step，让界面保持紧凑而不挤压扫描行。

## Elevation & Depth

这是一个默认扁平的 system。Card、table 和 inspector 通过白/灰色调对比与 1px border 表达结构，selection 与 drag state 则使用 accent ring。Astryx `Card` 默认 `elevation="none"`；`--shadow-low`、`--shadow-med` 和 `--shadow-high` 只保留给临时 popover、dialog 以及必须浮在 workbench 上方的 layer。mutation dialog 另外使用已观察到的 scrim 与宽 drop shadow，使合法确认成为独立的时刻。

### Shadow Vocabulary

- **Low ambient lift** (`--shadow-low`)：小型浮动菜单或有意抬高的表面。
- **Medium transient lift** (`--shadow-med`)：必须与邻近内容拉开距离的 popover 或 inspector-like layer。
- **High modal lift** (`--shadow-high`)：dialog 或阻断式确认 layer。
- **Mutation dialog shadow** (`0 1rem 3rem var(--color-border)`)：现有 dialog treatment；与 native scrim 配对，不要用于 card 静止状态。

### Named Rules

**The Flat-by-Default Rule.** surface 在静止时保持扁平。只有 surface 浮在内容上方或 modal state 需要分隔时才添加 shadow；绝不要把 elevation 当作普通 task card 的装饰。

## Shapes

form language 是温和但有纪律的圆角：inner affordance 使用 `--radius-inner`，control 与 button 使用 `--radius-element`，card 与 dialog 使用 `--radius-container`，badge 使用 `--radius-full`。Border 保持 1px 且安静，只有 control 需要清晰边缘时才使用 emphasized border。不要把方形 control 与圆角 container 混用，也不要引入 Astryx scale 之外的新 radius。

反复出现的 silhouette 是暖灰画布上带边界的白色表面。Input、selector 与 button 保持紧凑 hit target；task label 与 status badge 只有在表达分类时才使用 pill shape，不把它当作通用装饰。

## Components

### Buttons

- **Shape：** Astryx button 使用 element radius 与紧凑 medium size；共享 padding 由 frontmatter 记录。
- **Primary：** 石墨强调色填充配 on-accent text，用于 create、confirm 和其他正向 mutation。
- **Hover / Focus：** 使用安静的强调色变化或 tint；全局 focus treatment 是带 offset 的 accent outline，在 strict CSP 下也必须可见。
- **Secondary / Ghost / Destructive：** secondary 使用 surfaced、emphasized border；ghost 保持透明 surface 并使用 graphite text；destructive 使用 semantic error pair，仅用于不可逆 maintenance action。

### Chips / Badges

- **Style：** Astryx `Badge` 是紧凑的 full-pill label。Neutral badge 使用 gray categorical pair；`info`、`success`、`warning` 与 `error` 使用主题的 semantic fill 及对比 text。
- **State：** badge 用于分类 priority、status 与 readiness。它们不是 miniature button；如果 label 可操作，应渲染带正确 focus treatment 的 Button。

### Cards / Containers

- **Corner Style：** task card 使用 container radius 与 1px boundary。
- **Background：** 在 warm gray canvas 上使用 card surface；不要创建 per-card accent background。
- **Shadow Strategy：** 默认无 elevation；hover、selected 与 dragged state 添加安静的 accent ring 或 border treatment。
- **Border：** 静止时使用 hairline，仅在 card selected 或嵌入 control 时使用 emphasized。
- **Internal Padding：** Board task card 使用 Astryx `Card padding={3}`；更大的 panel 使用中间 spacing step。

### Inputs / Fields

- **Style：** native semantic input、select 与 textarea 使用 surface background、emphasized border、element radius 和 compact height。
- **Focus：** accent outline 始终可见；当 semantic status field 需要时，Task Inspector 可以使用既有 fallback focus color。
- **Error / Disabled：** error field 使用 semantic error pair 与 leading status edge；disabled field 使用 disabled text token，但不降低 hit target。

### Navigation

- **Style：**可折叠的 Astryx `SideNav` 是带 grouped heading 与 compact item 的安静 surface。Selected item 获得 accent edge/tint 与 primary text；未 selected item 使用 secondary text。确保 label 与 collapse control 支持 keyboard access。

### Task Inspector

右侧 inspector 是 surfaced、带 border 的区域，而不是浮动 card。将 control、metadata、readiness 与 mutation action 放在由 hairline 分隔的显式 section 中；在窄屏 Explorer layout 中，inspector 变为 normal flow column。

### Task List Table

list view 是 evidence table，不是 dashboard。使用 sticky body-tone header、1px row boundary、compact cell、semibold task link，并用 mono stack 展示 ref 与 date。在已观察到的 `50rem` 最小宽度下保留横向滚动。

### Mutation Dialog

使用带 `showModal()` 与 focus containment 的 native `<dialog>` path。surfaced dialog、native scrim、明确的 cancel/confirm 层次和既有 mutation shadow，让 destructive 或 retryable action 清晰可辨，而无需增加第二套 overlay system。

## Do's and Don'ts

### Do:

- **Do** 使用 graphite `--color-accent` 表达 action、focus、selected tab 和 active edge；semantic color 只表达 status。
- **Do** 将 identifier、ref、hash 和 timestamp 保持在 Astryx code stack 中。
- **Do** 在普通静止 surface 上使用 1px hairline、Astryx radius scale 与 `Card elevation="none"`。
- **Do** 保留 strict CSP 所要求的 static Astryx CSS import order 与 native semantic fallback path。

### Don't:

- **Don't** 将 approved future mockup 的 cobalt value 或 Plane-specific chrome 复制到当前 Astryx neutral token set。
- **Don't** 添加 SaaS collaboration chrome、装饰性 dashboard ornament 或第二个 action accent。
- **Don't** 给普通 task card 添加 resting shadow，也不要把 badge 当作 interactive control。
- **Don't** 在当前 web surface 引入 Tailwind、Radix、shadcn、runtime style injection 或 inline `<style>` block。

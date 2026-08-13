---
name: "kanban-tool Web"
description: "Plane-faithful、agent-first 的本地可观察控制面。"
colors:
  canvas: "light-dark(#f6f7f8, #0f1113)"
  surface: "light-dark(#ffffff, #14171a)"
  layer: "light-dark(#f0f2f4, #191d21)"
  layer-active: "light-dark(#e7edf3, #20262c)"
  border: "light-dark(#dfe3e7, #2a2f35)"
  border-strong: "light-dark(#c6cdd4, #3a424a)"
  text-primary: "light-dark(#15181c, #f4f6f8)"
  text-secondary: "light-dark(#59616a, #a7afb8)"
  text-tertiary: "light-dark(#747d87, #858f99)"
  accent: "light-dark(#0877bd, #4ba9e8)"
  accent-muted: "light-dark(#e4f3fc, #123247)"
  success: "light-dark(#147a3d, #46c878)"
  warning: "light-dark(#9a6500, #e9ac38)"
  danger: "light-dark(#b42332, #ef6570)"
typography:
  ui:
    fontFamily: "Figtree, Inter, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif"
    fontSize: "0.875rem"
    lineHeight: 1.4
  title:
    fontFamily: "Figtree, Inter, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif"
    fontSize: "1rem"
    fontWeight: 600
    lineHeight: 1.35
  page-title:
    fontFamily: "Figtree, Inter, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif"
    fontSize: "1.25rem"
    fontWeight: 600
    lineHeight: 1.3
  code:
    fontFamily: "ui-monospace, \"SFMono-Regular\", Consolas, monospace"
    fontSize: "0.75rem"
    lineHeight: 1.4
rounded:
  control: "0.375rem"
  card: "0.5rem"
  popover: "0.625rem"
  pill: "9999px"
spacing:
  rail: "3.5rem"
  context-sidebar: "15.5rem"
  topbar: "2.75rem"
  control-height: "2rem"
  compact-row: "2.5rem"
  comfortable-row: "3rem"
---

# Design System: kanban-tool Web

本文件是 `apps/web` 的设计 owner；在 Web/Desktop 控制面范围内，它覆盖仓库根
`DESIGN.md` 中已经被本轮重构替代的旧视觉方向。

## Creative North Star

**Dense Local Control Surface**

界面应像 Plane 那样安静、紧凑、层次清晰，但只表达 kanban-tool 的真实事实。AI agents
在后台驱动 durable queue，人类进入界面是为了快速判断项目、任务、执行和异常状态，而不是
阅读营销页面。默认深色用于持续观察场景，完整保留浅色；两种主题都使用同一层级、密度和
语义色。

Plane 是唯一绑定的产品工艺参考。学习其双层导航、canvas/surface/layer 阶梯、密集数据视图、
紧凑 toolbar、filter/display 控件族和 side-peek。不得复制 Plane 的源码、品牌、图标、文案、
业务模型或没有 canonical contract 的数据。

## Composition

桌面端由四个稳定区域组成：

1. `ProductRail`：固定宽 `3.5rem`，顶部为 kanban-tool 标识，主项仅 `Projects`，底部仅
   `Settings`。
2. `ProjectsSidebar`：默认宽 `15.5rem`，允许在 `14rem–20rem` 内调整。展示 `Home`、
   `Projects`、项目搜索/列表，以及当前项目下的 `Overview`、`Tasks`。
3. `ProjectHeader`：高 `2.75rem`，承载 breadcrumb、当前 surface 标题、view switch 和
   scope 内操作。
4. `MainSurface`：Projects collection、identity-only Overview 或 Tasks workspace；只有
   Tasks 选择 task 时出现 inspector side-peek。

`/app/` 是不挂 `BoardLive`/SSE 的 Projects collection。进入 canonical
`/app/boards/:slug/...` 后才建立唯一 board session。Project switch 必须释放旧 session 和
失效选择。

窄屏不压缩成缩小版桌面：context sidebar 变为 drawer，task inspector 变为 sheet/full-screen，
board/table 只在自身 region 横向滚动，页面本身不得横向溢出。

响应式 shell 由 [`responsive-shell.ts`](src/lib/responsive-shell.ts) 统一决定，边界是：`mobile`
`<768px`、`tablet` `768px–<1024px`、`desktop` `>=1024px`。`mobile` 隐藏 `ProductRail`，使用
compact topbar 和 sidebar drawer；`tablet` 保留 `ProductRail`，将 context sidebar 作为 modal
drawer；`desktop` 使用 rail/sidebar/main 三列，sidebar 可在 `14rem–20rem` 内调整。Task inspector
随 shell mode 分别呈现为 `fullscreen`、`dialog`、`side-peek`；这些模式和无页面横向溢出由
[`plane-tasks-acceptance.spec.ts`](tests/plane-tasks-acceptance.spec.ts) 验收。

## Surface and Depth

- `canvas` 只出现一次，承载整页背景。
- `surface` 用于 rail、context sidebar 与主内容的同级区域。
- `layer` 用于 column、toolbar group、selected navigation 和 table header。
- card 默认由 surface 差异或单一 hairline 分隔，不叠加 resting shadow。
- shadow 只给 popover、command menu、side-peek overlay 和 dialog；必须有偏移和柔和模糊。
- 分区优先使用对齐、间距和 hairline，不用嵌套卡片堆叠页面。

## Type and Density

- UI 基准为 14px，dense metadata 可用 12px；禁止用整页等宽字体制造“技术感”。
- project/task 标题使用 Figtree/Inter 类的人文无衬线；ID、ref、run id、hash 和精确时间才用
  mono。
- 页面标题保持 `1.25rem` 左右，不使用旧版巨型 `clamp()` heading、eyebrow 或全大写技术标签。
- board card、table row、toolbar 和 sidebar item 以高扫读性为先；metadata 对齐并保持稳定列宽。

## Interaction Grammar

Projects、Tasks 与 diagnostics 共享以下交互责任，但不共享领域数据模型：

- `ResourceHeader`：breadcrumb、标题、数量/状态和主操作。
- `ViewSwitcher`：同一 canonical 数据的呈现方式；URL 是 active view 的来源。
- `FilterBar`：query-backed filters、active chips、clear 与保存入口。
- `DisplayMenu`：density、visible fields、group、sort；只显示当前 view 真正支持的选项。
- `ProjectPicker`：搜索 canonical boards，明确 loading/offline/error/empty。
- `SidePeekFrame`：保留 selection URL、focus return 与真实 task inspector read model。
- `StateBoundary`：loading、empty、offline、stale、recovering、error、retry 都是正式状态。

Board、List、Table 与 Map 只是 projection。`Table` 可以是 List 的 display variant；Map 明确为依赖
关系图。没有 timeline read model 前，Timeline/Gantt 只允许以 unsupported story 出现在
Storybook，不得伪装成可用产品功能。

## Components

- Astryx component、layout、token、composition 与 template 是默认实现；先通过 Astryx CLI 发现，
  再写产品组合。
- 新代码不使用原生 `<div>`/`<span>` 承担布局，不新增手写 CSS/CSS Modules。需要 utility styling
  时允许 Tailwind；需要深度定制时允许可追溯版本的 `astryx swizzle`。
- 现存 React/CSS 是迁移存量，不是新 surface 的模板。Tailwind arbitrary value、swizzled component
  和本地 token 都不能形成与 Astryx 平行的设计系统。
- 控件高度以 `2rem` 为主，hit target 通过外部 padding/布局保证；button radius 为 6px。
- icon 使用一致的 16px、1.5px stroke 系统；不得使用 emoji 或 Unicode glyph 充当图标。
- selected nav、focus、primary action 使用单一 operational blue accent；状态颜色只表达
  success/warning/danger，且必须同时有文本或图标。
- task card 使用 8px radius、单一 border/surface depth；hover/selected/dragged 的反馈互不混淆。
- table 是 evidence surface：sticky header、紧凑 row、稳定列、局部横滚。
- inspector 是主视图旁的结构区域，不是漂浮 dashboard card；移动端才转为 protected sheet。
- mutation 继续使用语义化 dialog、明确 cancel/confirm 和 pending/error feedback。

## Imagery and Content

这是数据密集型工具，不使用装饰性插画、渐变 hero、玻璃质感或纹理背景。当前 project contract
没有 cover，因此 Projects/Overview 不显示生成封面、emoji、团队头像、进度、风险或活动 feed。
空状态可以使用真实 icon system 和简短恢复动作，不使用虚构场景图。

文案直接说明事实和恢复路径。技术字段保持 literal；没有 contract 的值不以 placeholder 假装存在。

## Motion

- hover/focus/selection 使用 `120–180ms` ease-out；数据 layout 不做逐项入场动画。
- side-peek/drawer 可用一次轻量位置与阴影过渡，初始内容必须立即可见。
- drag、reorder 与 status transition 的 motion 只说明空间/状态变化。
- `prefers-reduced-motion` 下移除非必要位移与平滑滚动，不影响 focus return。

## Accessibility

所有 rail/sidebar item、view switch、filter、task row/card、side-peek 与 dialog 都必须键盘可达，
有清晰 focus ring 和语义名称。文本对比至少满足常规文字 4.5:1；状态不得只靠颜色。中文、英文、
窄屏、长 project/task 名称与真实错误文案均是 Storybook/Playwright 的正式验收状态。

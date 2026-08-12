# Web UI 工作约定

## 产品边界

- `apps/web` 是 Browser 与 Tauri 共用的 product UI owner；两者加载 `kanban serve` 提供的同一
  `/app/` artifact。
- canonical mutation 经 generated contract 校验后的 localhost HTTP client 进入共享 service path；
  `tasks.status` 和 server transition 结果是生命周期事实。
- URL 持有 board、view、task inspector、filter、sort、search 和 pagination；用户偏好只写
  `kb:web:*` localStorage。

## 组件与样式

- 页面结构、视觉层级、响应式、排版或交互反馈改动使用项目级 `$impeccable`；它提供设计方法和
  审查证据，不替代本文件的 Astryx、strict CSP、领域语义或交付边界。
- Astryx 是 UI 的主实现系统。开始实现前先运行已安装 CLI 的 `build`、`search`、`template`、
  `component` 或 `docs`，采用官方 component、layout、token、composition 与 ready template。
- 新代码不使用原生 `<div>`/`<span>` 承担布局，不新增手写 CSS、CSS Modules、magic value 或自建
  token；布局和样式先用 Astryx props/components/tokens，仍需 utility 时允许 Tailwind。
- 深度定制允许 `astryx swizzle <Component>`，但 swizzled owner 必须可追溯到对应 Astryx 版本，并在
  `astryx upgrade` 时单独审阅；不得以 swizzle 建立与 Astryx 平行的通用组件库。
- 不引入 Shadcn、直接 Radix wrapper、CVA 或第二套通用组件库。
- `apps/web` 不导入 `@tauri-apps/*`；Host、tray、single-instance 和 deep link 属于 Desktop shell。

## Contract 与交付

- `src/lib/api/generated/` 只由 `xtask web-contracts generate` 写入；手写 transport 从 `unknown` 经
  generated validator 得到 typed value，不使用 unchecked generic request 或 wire type assertion。
- Map/ELK、Markdown 和其他重依赖按 route 或 inspector section lazy-load；共享模块使用直接 import，
  不通过宽泛 barrel 扩大 bundle。
- 一次完成一个 capability-ledger 纵向切片；同一切片包含 states、errors、keyboard/a11y、Playwright
  与 invalidation evidence。实现进度和 review finding 写入 Kanban task，不复制进本文件。

<!-- ASTRYX:START -->
Astryx v0.3.0 · 155 components
CLI: run every command as `pnpm exec astryx <cmd>` (shown below as `astryx ...`).

SETUP (once, in your app entry e.g. main.tsx) — without these, components render unstyled:
  import "@astryxdesign/core/reset.css";
  import "@astryxdesign/core/astryx.css";

WORKFLOW — discover, don't guess. Before writing UI:
1. `astryx build "<idea>"` — START HERE: returns a kit (closest [page] + [block]s + [component]s). No args = full playbook.
2. `astryx template <name> [--skeleton]` — scaffold the [page]/[block]s it named, or study their layout. Templates are reference code.
3. `astryx component <Name>` — props + examples for every component you use.

RULES:
- No <div> — components do all layout/spacing. Full page → AppShell; sidebar nav → SideNav.
- Frame first: pick the shell (AppShell / Layout+LayoutPanel) and budget regions in px BEFORE writing content (`astryx docs layout`).
- Dense data = rows (Table, List/Item) edge-to-edge — never Card-wrapped list items. Card = dashboard widgets, galleries, settings groups only.
- Status → StatusDot/Token; Badge only for counts and enumerated states, never decoration.
- Custom styling: component props first; else style/className with tokens — var(--color-*|--spacing-*|--radius-*). No raw hex/px. (No StyleX/Tailwind compiler here — don't use xstyle/utility classes.)
- Tokens for every value (`astryx docs tokens`). Brand/accent via `astryx theme` — never override --color-* in :root.
- SELF-CHECK before you finish: re-read the file and replace any raw <div>/<span> layout, imported .css/@apply, or hardcoded value (#hex, 16px) with the component or a token (var(--color-*|--spacing-*|…)). If unsure a component/prop exists, run `astryx component <Name>` / `astryx search "<thing>"`; don't hand-roll CSS.

MORE CLI:
  search "<query>"   find any component / hook / doc / template / block
  component --list   155 components by category
  template --list    page + block recipes
  docs <topic>       color, elevation, icons, illustrations, internationalization, layout, migration, motion, principles, shape, spacing, styling, theme, tokens, typography
  swizzle <Name>     eject component source for deep customization
  upgrade --apply    run after any @astryxdesign/core bump
<!-- ASTRYX:END -->

## kanban-tool 的 Astryx 主路径

- 上述 managed block 由 `pnpm exec astryx init --features agents --agent codex` 生成，并在升级
  `@astryxdesign/core` 后通过 `pnpm exec astryx upgrade --apply` 更新；不要手工编辑 marker 内正文。
- managed block 是默认实现规则；不得以“领域组合”为由跳过 Astryx discovery、layout、component、
  token 或 template。
- 项目明确覆盖 managed block 中“不要使用 Tailwind compiler”的限制：允许 Tailwind utilities，但不以
  arbitrary value 重建 token 系统。项目也明确允许 `astryx swizzle`，以 Astryx source 为深度定制起点。
- 新代码不得新增手写 CSS 或原生布局 `<div>`/`<span>`。现存 CSS/semantic fallback 是迁移存量，修改
  相关 surface 时应优先替换为 Astryx/Tailwind，不把存量模式复制到新组件。
- Astryx、swizzle 与 Tailwind 仍不能绕过 strict CSP、canonical 领域模型或 production owner；编译后
  CSS 可以由 `style-src 'self'` 加载，runtime inline style injection 仍须通过 CSP/browser gate。

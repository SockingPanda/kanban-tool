# Astryx 基础切片记录

本文记录 Stage 00 的 Astryx browser-first 基础切片。它不是下一阶段的组件目录，也不替代
`capability-ledger.md`；用途是保留可复用的安装、CSP、组件边界和浏览器验证结论。

## 结论

- UI 以 `apps/web` 的生产 `/app/` artifact 为唯一浏览器入口，保持 Vite base `/app/`。
- Astryx 依赖固定为 `@astryxdesign/core@0.3.0`、`@astryxdesign/theme-neutral@0.3.0`、
  `@astryxdesign/cli@0.3.0`，StyleX peer 固定为 `@stylexjs/stylex@0.19.0`；Vite 固定为
  `8.2.1`。
- 组件使用精确 subpath：`@astryxdesign/core/Button`、`Card`、`Table`、`theme` 和 `VStack`。
  Stage 00 曾以普通 React + CSS Modules + 静态 token CSS 补齐领域组合；这些现在是待迁移存量。
- strict CSP 下禁止 runtime style injection，因此 TextInput、Dialog、Selector、Popover 在 Stage 00
  使用了语义 HTML/CSS Modules fallback。当前新增实现改为 Astryx 主路径，并允许受控 swizzle 与
  编译期 Tailwind；升级 Astryx 或迁移相关 surface 后须重跑本文的 CSP seam。
- 主题由 `package.json.astryx.theme` 接线到 `src/theme/astryx.js`；源文件通过 `defineTheme`
  继承 neutral，使用 `astryx theme build` 生成静态 CSS/JS/d.ts。`astryx.config.mjs` 只使用
  CLI 0.3.0 发布的 `AstryxConfig` 字段。

## CLI 与模板证据

在 `apps/web` 目录运行：

```text
pnpm exec astryx --version
# 0.3.0

pnpm run astryx:doctor
# exit 0；pass 6、warn 1、fail 0、info 1
# 当前外置盘上的 pnpm package links 在 Node Dirent 中不报告 directory/symlink，导致 CLI 误报
# “No @astryxdesign/theme-* packages are installed”；实际 package、CSS import 与 Theme provider 均已接线

pnpm run astryx:templates
# 可用 page template 包含 kanban-board、table-page、settings-sidebar、shell-side-nav；
# incident-console 当前 isReady=false。

pnpm run astryx:search "task table"
pnpm run astryx:component Button --props
pnpm run astryx:manifest
```

`apps/web/AGENTS.md` 包含由 `astryx init --features agents --agent codex` 生成的
`<!-- ASTRYX:START -->` managed block，使 Codex 先用 `build`、`template`、`component`、`search` 和
`docs` 发现官方能力，并以 Astryx component/layout/token 作为主实现。升级 Astryx 后由
`astryx upgrade --apply` 更新 managed block，不手工维护其组件库存。

本文件以下 CSS/CSP seam 是 Stage 00 的已验证历史证据，不再定义新增 UI 的实现方法。当前方法由
`apps/web/AGENTS.md` 与 ADR 0006 持有：新代码不新增原生布局 `<div>`/`<span>` 或手写 CSS，允许
Tailwind utility 与可追溯的 `astryx swizzle`；现存 React/CSS fallback 只作为待迁移存量。

`kanban-board` skeleton 仅用于确认官方组件和布局入口，没有把模板逻辑、数据模型或文案复制到
产品代码。生产切片只保留当前需要的 Button/Card/Table/VStack/Theme seam。

## 样式与 strict CSP

入口静态加载顺序是：

1. `@astryxdesign/core/reset.css`
2. `@astryxdesign/core/astryx.css`
3. `src/theme/astryx.css`（由 `astryx theme build src/theme/astryx.ts` 生成）
4. `src/layers.css` 的 `reset, astryx-base, astryx-theme, product` layer 声明
5. `src/styles.css` 与 `src/foundation.module.css` 的产品层

Preview middleware 发送以下 CSP（没有 `unsafe-inline`）：

```text
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:;
font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self';
frame-ancestors 'none'
```

Stage 00 没有引入 `@astryxdesign/build`、`@stylexjs/unplugin`、Tailwind、Radix 或 Shadcn，以便隔离
验证已发布 Astryx CSS artifact 的 strict-CSP seam。该历史选择不再禁止后续使用编译期 Tailwind；
Tailwind 产物仍必须作为同源静态 CSS 通过 CSP/browser gate，Shadcn 与直接 Radix wrapper 仍不允许。

验证项包括 `page.locator("[style]")` 和 `page.locator("style")` 都为零；没有 inline style
prop、动态 `<style>`、外部字体或远程 CSS。

### core 0.3.0 的 fallback 依据

安装包源码提供了可复核的边界证据：

- `src/TextInput/TextInput.tsx` 无条件把 `disabledMessageTooltip.ref` 挂到 wrapper；
  `src/Layer/useLayer.tsx` 的 `addAnchorName` 写入 `el.style.anchorName`。
- 同一 `useLayer.tsx` 的 `anchorStyle` 计算 `positionAnchor`、`positionArea`、
  `positionTryFallbacks`，并通过 `style={{...stylexResult.style, ...anchorStyle, ...extraStyle}}`
  输出。
- `src/Dialog/Dialog.tsx` 的 inner/container 和 sizing path 通过 `stylex.props` 计算动态尺寸。

因此官方 TextInput/Dialog/Selector/Popover 会在当前 core 版本产生运行时 style 属性。Stage 00 曾
改用普通 `<input>`、CSS Modules 和原生 `<dialog>` 取得基线证据。当前新增实现应优先使用 Astryx；
确需深度定制时允许从 `astryx swizzle` 起步，并通过同样的 `[style]`/CSP/三引擎测试。

## Overlay 与浏览器语义

fallback overlay 不是 `<dialog open>` 的非模态伪装：React ref 在状态变化时调用
`HTMLDialogElement.showModal()`/`close()`，并监听 `cancel`/`close` 同步 React state、恢复 trigger
focus。`showModal()` 提供 top-layer 与背景 inert；额外的 capture-phase Tab guard 处理部分引擎在
仅一个可聚焦控件时退回 `document.body` 的行为，确保焦点仍在 dialog 内。Playwright 断言
`dialog.matches(":modal")`、Tab 约束、Escape 关闭与 focus return。

## RED → GREEN 证据

TDD seam 首先在旧 placeholder 上运行：

```text
pnpm --filter @kanban-tool/web exec playwright test tests/foundation.spec.ts --project chromium
# RED：title 仍为 “Kanban Tool”，没有 html[data-theme="light"]；断言失败。
```

实现后使用 production preview 验证：

```text
pnpm --filter @kanban-tool/web typecheck
pnpm --filter @kanban-tool/web vite-build
pnpm --filter @kanban-tool/web exec playwright test tests/foundation.spec.ts \
  --project chromium --project firefox
# 当前固定 1440×900 run 只覆盖 Chromium、Firefox；visual baseline 只在 Chromium 项目执行。
```

`typecheck` 的 UI-only 结果是在并行 web-contracts generator 写入
`src/lib/api/generated/` 之前取得的；随后 generator 接线期间的 combined gate 会因为缺少
`json-schema-to-ts`/`ajv` 以及尚未导出的 generated symbols 失败。该失败属于 contract generator
接线状态，不应被表述为本 UI seam 已通过；主线完成 generator 接线后必须重新运行 combined
typecheck/lint。

测试覆盖：Astryx Button/Card/Table/VStack 的可见性和 computed padding、第二行 table row 不被
table/card 裁切、light/dark 和长中英文案、Popover/anchor feature detection、严格 CSP 下无
console/pageerror、`<dialog>:modal`、Tab containment、Escape 和 focus return。Playwright gate
不把 WebKit/Safari 作为目标；Linux packaged WebKitGTK/Tauri smoke 留给后续 Desktop 阶段。

## 构建与后续边界

当前 `vite build` 产物约为：CSS 158.6 kB（gzip 27.8 kB），主 JS 421.5 kB（gzip 127.0 kB），
另有约 1.8 kB 的 Tooltip chunk。体积只作为基础线，不代表后续 route bundle 目标。

本切片不实现 API client、路由、SSE、board/task domain 状态或 Desktop host 生命周期；这些由
后续 capability 纵向切片按 contract、TDD、Playwright 和持久 SSE 证据推进。

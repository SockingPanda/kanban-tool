# Astryx 基础切片记录

本文记录 Stage 00 的 Astryx browser-first 基础切片。它不是下一阶段的组件目录，也不替代
`capability-ledger.md`；用途是保留可复用的安装、CSP、组件边界和浏览器验证结论。

## 结论

- UI 以 `apps/web` 的生产 `/app/` artifact 为唯一浏览器入口，保持 Vite base `/app/`。
- Astryx 依赖固定为 `@astryxdesign/core@0.3.0`、`@astryxdesign/theme-neutral@0.3.0`、
  `@astryxdesign/cli@0.3.0`，StyleX peer 固定为 `@stylexjs/stylex@0.19.0`；Vite 固定为
  `8.2.1`。
- 组件使用精确 subpath：`@astryxdesign/core/Button`、`Card`、`Table`、`theme` 和 `VStack`。
  领域组合由普通 React + CSS Modules + 静态 token CSS 表达。
- strict CSP 下禁止 runtime style injection，因此 TextInput、Dialog、Selector、Popover 在本切片
  使用语义 HTML/CSS Modules fallback；这不是兼容旧版，而是对 `core@0.3.0` 的明确 production
  边界。升级 Astryx 后须单独重跑本文的 CSP seam。
- 主题由 `package.json.astryx.theme` 接线；`astryx.config.mjs` 只使用 CLI 0.3.0 发布的
  `AstryxConfig` 字段。

## CLI 与模板证据

在 `apps/web` 目录运行：

```text
pnpm exec astryx --version
# 0.3.0

pnpm run astryx:doctor
# exit 0；pass 6、warn 1（AGENTS.md 没有 CLI marker）、fail 0、info 1

pnpm run astryx:templates
# 可用 page template 包含 kanban-board、table-page、settings-sidebar、shell-side-nav；
# incident-console 当前 isReady=false。
```

`kanban-board` skeleton 仅用于确认官方组件和布局入口，没有把模板逻辑、数据模型或文案复制到
产品代码。生产切片只保留当前需要的 Button/Card/Table/VStack/Theme seam。

## 样式与 strict CSP

入口静态加载顺序是：

1. `@astryxdesign/core/reset.css`
2. `@astryxdesign/core/astryx.css`
3. `@astryxdesign/theme-neutral/theme.css`
4. `src/layers.css` 的 `reset, astryx-base, astryx-theme, product` layer 声明
5. `src/styles.css` 与 `src/foundation.module.css` 的产品层

Preview middleware 发送以下 CSP（没有 `unsafe-inline`）：

```text
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:;
font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self';
frame-ancestors 'none'
```

本切片不引入 `@astryxdesign/build`、`@stylexjs/unplugin`、Tailwind、Radix 或 Shadcn。原因是
产品边界要求静态 CSS；build/plugin 方案会把编译/开发运行时样式层带入页面，不能作为本切片的
strict-CSP 证据。组件只使用已发布的 Astryx CSS artifact，领域样式留在 CSS Modules。

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

因此官方 TextInput/Dialog/Selector/Popover 会在当前 core 版本产生运行时 style 属性。生产 seam
改用普通 `<input>`、CSS Modules 和原生 `<dialog>`，不 swizzle、不 fork、不放宽 CSP。后续
升级 core 后应先以同样的 `[style]`/CSP/三引擎测试重新评估，再决定是否切回官方组件。

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
  --project chromium --project firefox --project webkit
# 最新固定 1440×900 run：13 passed、2 skipped；visual baseline 只在 Chromium 项目执行，
# Firefox/WebKit 的该测试按项目约束 skip。
```

`typecheck` 的 UI-only 结果是在并行 web-contracts generator 写入
`src/lib/api/generated/` 之前取得的；随后 generator 接线期间的 combined gate 会因为缺少
`json-schema-to-ts`/`ajv` 以及尚未导出的 generated symbols 失败。该失败属于 contract generator
接线状态，不应被表述为本 UI seam 已通过；主线完成 generator 接线后必须重新运行 combined
typecheck/lint。

测试覆盖：Astryx Button/Card/Table/VStack 的可见性和 computed padding、第二行 table row 不被
table/card 裁切、light/dark 和长中英文案、Popover/anchor feature detection、严格 CSP 下无
console/pageerror、`<dialog>:modal`、Tab containment、Escape 和 focus return。WebKit project
只代理 Playwright WebKit engine，明确不等同于 Linux packaged WebKitGTK/Tauri smoke。

## 构建与后续边界

当前 `vite build` 产物约为：CSS 158.6 kB（gzip 27.8 kB），主 JS 421.5 kB（gzip 127.0 kB），
另有约 1.8 kB 的 Tooltip chunk。体积只作为基础线，不代表后续 route bundle 目标。

本切片不实现 API client、路由、SSE、board/task domain 状态或 Desktop host 生命周期；这些由
后续 capability 纵向切片按 contract、TDD、Playwright 和持久 SSE 证据推进。

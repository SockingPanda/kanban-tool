# Plane-only Web Surface Brief

## Goal

将 kanban-tool Web/Desktop 重构为 Plane-faithful 的 agent-first 人类可观察控制面。用户能在多个
canonical board/project 间切换，通过 Projects、Project Overview 和 Tasks 的多种真实视图快速
理解当前状态并在必要时介入。

## Stable product truth

- AI agents 是主要执行者，人类主要观察并在必要时操作。
- `project` 只映射 canonical board；没有 workspace/team。
- `tasks.status` 是事实，Board/List/Table/Map 只是 projection。
- 所有 mutation 继续经过 typed application/service path。
- Projects collection 不挂默认 board session 或 SSE。
- Project Overview 第一版只展示真实 identity/description/archive 信息。
- Timeline/Gantt 在没有 canonical read model 前不得作为可用功能。

## Committed visual world

- Plane 是唯一产品 craft 参考；不混入 Linear、Notion、GitHub 等视觉语言。
- 固定 product rail + Projects context sidebar + compact project header + dense main surface。
- 默认深色、完整浅色，canvas/surface/layer 逐级建立层次。
- operational blue 只用于 action/focus/selected，status 使用独立语义色。
- 14px UI、紧凑 toolbar/table/card、8px 左右 corner、hairline 分隔、overlay 才使用 shadow。
- 无装饰插画、无 project cover、无虚假指标、无 emoji 图标。

## Comps

用户于 2026-08-11 批准以下三张作为一套共同实现基准：

1. `.impeccable/mocks/plane-only-refresh/01-projects-collection.png`
2. `.impeccable/mocks/plane-only-refresh/02-project-overview.png`
3. `.impeccable/mocks/plane-only-refresh/03-tasks-workspace.png`

它们不是三选一：Projects collection 定义集合页，Project Overview 定义项目事实页，Tasks
workspace 定义任务多视图与 side-peek；三者共同定义同一 shell、密度与视觉语言。

## Composition contract

- 固定 product rail + Projects context sidebar + compact header + main surface。
- rail 只包含 kanban-tool mark、Projects 与底部 Settings。
- context sidebar 只包含 Home、Projects、项目搜索/列表，以及当前项目的 Overview、Tasks。
- Projects collection 使用 identity-only rows，不以生命周期列或 dashboard metrics 填满页面。
- Overview 保持紧凑 title/description/facts，Runs、Events、Signals、Ontology 只进入 More。
- Tasks 使用 compact view switch、filters/display、dense Board/List/Table/Map 与共享 side-peek。
- canvas、surface、layer 依次建立深度；hairline 为默认分隔，overlay 才使用 shadow。
- 基础字号约 14px，6px control radius、8px card radius，operational blue 只表示 action/focus/selection。

## Do not literalize

- 图片里的 `K` 只是临时 app mark，不是已经批准的品牌 logo。
- 图片中的 project/task 内容只用于验证真实字段与密度；正式 UI 必须读取 canonical data。
- 图片的固定像素宽度不是窄屏规范；sidebar、inspector 和 board/table overflow 必须按响应式契约转换。
- 生成图不决定最终 icon library、精确文案、hover/focus/disabled 状态或组件 API。
- 不把任何 PNG 区域裁进生产界面；核心 UI 全部使用语义化 React、Astryx、CSS 与可访问图标实现。

## Implementation inventory

| Visible ingredient | Production medium | Contract |
| --- | --- | --- |
| Product rail | Semantic React + Astryx/CSS + consistent SVG icons | Projects/Settings only; keyboard and tooltip states |
| Projects context sidebar | Semantic nav/tree + CSS layout | search, loading/error/empty, active project and Overview/Tasks |
| Compact resource header | React composition + CSS | breadcrumb, project switch, view/action ownership |
| Projects collection rows | React + `BoardListItem` fixture/read model | identity/description/archive only |
| Project Overview | Semantic definition rows and links | no aggregate metrics or N+1 |
| View switch / Filters / Display | Buttons, menus and URL-backed state | only supported Board/List/Table/Map options |
| Board columns and task cards | Existing typed task projection + reusable surfaces | status remains canonical; no invented metadata |
| Task side-peek | Existing Inspector read model in shared frame | URL selection, focus return, responsive sheet |
| Typography and surfaces | Astryx neutral theme + app-owned static CSS | dark/light, 14px density, hairlines, restrained accent |
| Icons | Existing or selected consistent 16px stroke SVG system | no emoji/Unicode glyphs; accessible names |
| Raster imagery | Accepted omission | no production raster assets are required by this UI world |

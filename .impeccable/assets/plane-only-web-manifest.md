# Plane-only Web shipped-asset manifest

## 审计范围与结论

本清单只覆盖用户在 2026-08-11 批准的三张 Plane-only 视觉基线，以及它们对
`apps/web` 实现的资产边界：

- `.impeccable/mocks/plane-only-refresh/01-projects-collection.png`
- `.impeccable/mocks/plane-only-refresh/02-project-overview.png`
- `.impeccable/mocks/plane-only-refresh/03-tasks-workspace.png`

三张图的对应 `.png.json` 均有 `approved: true`、`approval_source: "user"`、
`product_reference: "Plane-only"`，并分别指向同名 `.prompt.txt`。文件实际是
`1672 × 941` 的 RGB PNG；prompt 中的 `1920 × 1080` 是生成请求尺寸，不是生产资产
尺寸。三张图都已逐张打开检查，确认内容是应用 UI，而不是照片、插画或需要保留的
图像素材。

**结论：`no-raster = true`。** 这套 UI 世界不需要任何 production raster asset；
所有可见 ingredient 都应由语义化 React/HTML、Astryx primitives、app-owned 静态 CSS
和可访问的 inline SVG/icon 实现。批准 PNG 只能留在 `.impeccable/mocks/` 作为审阅
基线，禁止裁剪、`<img>` 引用、CSS `url(...)`、data URI 或复制到 `/app/` artifact。
因此 `produce` 与 `direct` 均为空，完整清单在 `semantic` 分组。

### 总体复制与许可边界

- Plane 只提供信息架构、密度和控件工艺参考。不得复制 Plane 的名称、logo/商标、
  图标路径、插画、CSS token、页面源码、组件结构或 AGPL-3.0 代码；不得把 Plane
  参考仓库或其运行时依赖加入 `apps/web`。仓库研究记录中的固定提交与许可证证据见
  `.impeccable/research/plane-frontend-reference.md`。
- 生产 UI 只能使用仓库已有的 `@astryxdesign/core@0.3.0`、
  `@astryxdesign/theme-neutral@0.3.0`（精确 subpath、静态 CSS）以及
  `apps/web` 自有 React/CSS/SVG。若未来引入第三方 icon 或字体，必须先有独立的
  license/source 记录；本清单不授权新增依赖。
- 图片 prompt 中的项目名、任务名和 run id 是视觉验证数据，不是可硬编码的产品事实。
  正式页面必须读取现有 typed read model/i18n；不得把 prompt 或截图中的示例内容、
  Plane 业务术语带入 canonical UI。

## 资产分组

### `produce`

无。没有需要生成、清理、抠图、压缩或重绘的 standalone image。若后续出现 cover、
插画或照片需求，必须另开有 source、license、目标尺寸、提示词和用户批准记录的资产
任务，不能从这三张 PNG 裁出 shipping crop。

### `direct`

无。三张批准图是 UI mock，不是可直接发布的 standalone source asset；不能以“改名”、
压缩或裁剪的方式进入 production。

### `semantic`

以下每行的 `qa_status: accepted` 表示“该视觉角色已通过批准图审计，并接受 semantic
实现、不产生 raster”的资产分类；页面落地后仍须执行该行的 acceptance check。

| id | 视觉 ingredient → production source/method（implementation） | owner component | licensing/copy boundary | acceptance check | notes | qa_status |
| --- | --- | --- | --- | --- | --- | --- |
| `shell.frame` | 四区 shell（product rail、Projects context sidebar、compact header、main surface）用语义 `<nav>`/`header`/`main` 与 `@astryxdesign/core/AppShell`、`Layout`、`LayoutContent`、`SideNav` 组合；层级、hairline、尺寸和 dark/light 由 app-owned CSS token 负责。 | `ProductShell`；重构后分别由 `ProductRail`、`ProjectsSidebar`、`ProjectHeader`、`MainSurface` 维护。 | 只复用仓库已安装 Astryx API；不复制 Plane DOM、CSS 或源码，不把 mock 的像素布局当作固定响应式规格。 | 以三张基线逐一比对拓扑、层级和密度；1440/1672 宽桌面与窄屏截图均无页面横溢；strict CSP 下没有 runtime style injection。 | 参考图是统一 shell 基线，不是生产截图。 | `accepted` |
| `rail.mark` | rail 顶部使用 `kanban-tool` 文本和 app-owned `KanbanMark`（如确有需要则 inline SVG/vector）；Projects 与 Settings 是语义链接，tooltip/accessible name 由 React 提供。 | `ProductRail` / `ProductShell` 的 rail header。 | 图中的白色 `K` 只是临时 app mark，未获 logo 批准；不能把它当品牌 logo，也不能描摹 Plane mark 或携带其商标。生产身份只表达 `kanban-tool`。 | 人工检查 mark 与 Plane 视觉/路径不相同；dark/light 对比达标；键盘聚焦和屏幕阅读器名称存在；产物无 PNG 文件依赖，SVG 仅允许 inline。 | 若产品最终只需要文字，可完全省略 mark；不得为了填空白生成图片。 | `accepted` |
| `iconography.system` | 建立一个 app-owned `Icon` primitive，使用 16px、约 1.5px stroke 的 inline SVG；复用现有 `StaticIcon` 方向或从 Astryx/仓库自有路径实现，`aria-hidden` 与 label 按是否有可见文字决定。 | `Icon` shared primitive；`ProductRail`、`ProjectsSidebar`、`ProjectHeader`、`ViewSwitcher`、`FilterBar`、`DisplayMenu`、`TaskInspector` 消费。 | 禁止 emoji、Unicode glyph、Plane 图标路径、Plane icon package 或 AGPL 源码；不为“像截图”下载外部 icon sprite。 | 逐项检查 Home/Projects/Overview/Tasks、folder、search、filter、display、view、status、close、chevron、run 等图标：统一尺寸/stroke、可聚焦控件有可读名称、颜色不是唯一状态编码。 | 图中的 folder、home、table、map、gear、run 等都属于 UI icon，不是 raster media。 | `accepted` |
| `sidebar.projects` | 用 semantic nav/tree、按钮和 CSS grid/flex 实现 Home、Projects、项目搜索/列表及当前 project 的 Overview/Tasks；loading/error/empty 使用状态组件，不内嵌图片。 | `ProjectsSidebar`；现有 `ProjectPicker`、`ProductShell` nav seam 可复用。 | 项目只映射 canonical board；不引入 workspace/team/member/协作模型，不复制 Plane sidebar 文案或源码。 | 项目切换、active state、focus return、drawer（窄屏）可键盘完成；列表只来自 `BoardListItem`/typed board read，不凭截图硬编码 project。 | 三张图的侧栏内容是共同 shell 基线；`Wiki`/`Story Workshop` 等字样只能在真实 board 数据存在时出现。 | `accepted` |
| `header.resource` | compact header 使用 semantic breadcrumb、project switch trigger、view/action buttons；下拉/输入在 strict CSP 下用 native HTML + CSS Modules fallback，避免会注入 inline style 的组件路径。 | `ProjectHeader` / `ResourceHeader`；现有 `ProductShell` header seam。 | 仅使用 `apps/web` 路由和 read model；不能照搬 Plane 的 command/menu API、文案或图标资源。 | URL 是 active view/project 的来源；header action 不改变第二状态机；Open Tasks 等操作可访问、pending/error 明确。 | Overview 图中的 `kanban-tool / Overview`、project picker、Open Tasks 是语法示例，不是固定文案来源。 | `accepted` |
| `projects.collection` | identity-only project rows 用 `<ul>`/`<a>` 或 button、folder inline SVG、hairline 和 surface token；描述、slug、archive 信息由 board list read model 提供，不用封面或卡片图片。 | `ProjectsCollection`（可复用现有 `BoardListSurface`、`BoardListItem`、`ProjectPicker`）。 | 只展示 contract 保证的 `id`、`slug`、`name`、`description`、archive；不得编造 lifecycle、progress、risk、owner、cover、avatar、task counts。 | 01 comp 的三行密度/对齐可扫读；搜索、empty/error/retry 有语义状态；实现中无 `<img>`、`background-image`、mock path 或 data URI。 | 行内 folder 是 icon，不是项目目录 raster。 | `accepted` |
| `project.overview` | 用 `<dl>`/`<dt>`/`<dd>` identity rows、Explore link 和 More list；仅渲染 board identity/description/archive 与真实 Tasks 入口。 | `ProjectOverview` / `MainSurface`；与 `ProjectHeader`、`SidePeekFrame` 的共享 layout 组合。 | 不把 More 中 Runs/Events/Signals/Ontology 伪装成已实现指标，不引入 N+1 aggregate query、cover、activity 或团队数据；不复制 Plane overview 页面。 | 02 comp 的 title/description/facts/More 拓扑匹配；slug 使用 mono；不存在 contract 的字段不出现在 DOM；链接和 chevron 有可读名称。 | More strip 的 play/calendar/signal/ontology 仅是 app-owned SVG icon + links。 | `accepted` |
| `tasks.toolbar` | Board/List/Table/Map segmented switch、Filters、Display、search、active filter chip 用 `<nav>`、button、menu 和 URL-backed state；Timeline/Gantt 保持 unsupported。 | `TasksWorkspace` / `ExplorerPage`；`ViewSwitcher`、`FilterBar`、`DisplayMenu`。 | 只表达 `tasks.status` 的 projection；不复制 Plane filter DSL、saved views 或 command palette 源码/术语，不添加未获 contract 支持的选项。 | 03 comp toolbar 顺序与密度可比对；键盘可操作、URL 深链保留 view/filter/search；Board/List/Table/Map 读取同一 canonical task set。 | `All tasks` chip 是 projection lens，不是第二事实。 | `accepted` |
| `tasks.board` | 列、task card、status chip、step count 和 blocker/readiness 文本由 typed task projection + CSS surface/card 实现；不截图卡片，不烘焙阴影/圆角进图片。 | `BoardView` / `TasksWorkspace`；`BoardLive` 继续是唯一 live session owner。 | `tasks.status` 是唯一事实；不得自动 claim/review、伪造 progress、owner、avatar、日期或 Plane workflow。所有 mutation 仍走 typed service path。 | 列标题与卡片顺序/密度对齐 03 comp；状态同时有文字/图标；拖放/键盘动作合法；无 mock PNG 引用；Board/List 切换后事实一致。 | 03 图中 `Todo/Ready/Running/Review/Done/Blocked` 与 `#503–#508` 仅作视觉验证数据。 | `accepted` |
| `tasks.inspector` | side-peek 用 semantic `<aside>`/region、固定 header、section divider、status badge、mono run id 和 Open details button；窄屏转 sheet/full-screen，阴影只由 CSS overlay 负责。 | `TaskInspector` / `SidePeekFrame`；现有 `ExplorerPage` selection URL 与 inspector read model。 | 不复制 Plane issue detail 源码/字段；只读取真实 task/step/run；不把截图中 `#504`、run id 或文本作为生产 fixture。 | `?task=` 选择与关闭后 focus return 正确；桌面约 390px inspector 不挤坏 board，窄屏无页面横溢；status 有文字；无 raster/backdrop image。 | 03 comp 的 selected card、Running、Required step、Run、Open details 是 side-peek 组成示例。 | `accepted` |
| `theme.type.surface` | Astryx neutral theme + `layers.css`/`styles.css`/CSS Modules 静态 token 表达 canvas/surface/layer、hairline、14px Figtree/Inter/system stack、mono metadata、6px control/8px card radius；不使用 gradient/glass/resting shadow。 | `apps/web/src/styles.css`、`layers.css` 与各 feature CSS Modules；Astryx theme owner。 | 不复制 Plane cobalt/purple token 或字体文件；不放宽 strict CSP，不从远端加载字体/CSS，保持仓库已有 license/依赖边界。 | dark/light 都通过浏览器 computed style 检查；常规文字对比至少 4.5:1；`[style]`、动态 `<style>`、外部 font/CSS 均为零；普通 surface 无 shadow。 | 蓝色只表示 action/focus/selection，状态色同时有文本/图标。 | `accepted` |
| `media.empty` | 空数据、过滤无结果、无项目等状态用 semantic heading/text + app-owned state icon（可选）+ Retry/Clear action；不制作插画、场景图或空白 PNG。 | `StateBoundary`、`ProjectPicker`、`ProjectsCollection`、`TasksWorkspace`。 | 不引入生成插画、Plane empty-state artwork、外部 stock 或未授权图像；空状态文案来自 i18n/真实恢复路径。 | 分别覆盖“真空”“过滤无结果”“无 board”三类；有 `role="status"`/可读 label 与下一步；扫描 artifact 无 `<img>`/`data:image`。 | brief 明确接受 omission；icon 不是 media asset。 | `accepted` |
| `media.loading` | loading 使用 semantic `role="status"`、骨架块/静态 CSS shimmer（按 reduced-motion 可关闭）和可读文本；不使用 spinner GIF、视频或截图。 | `StateBoundary`、`BoardLive`、`ProjectPicker`、`ExplorerPage`。 | 只用 app CSS animation；不复制 Plane loading component/source，不加入外部动画素材或远程资源。 | 首屏内容未加载时仍有焦点/状态语义；`prefers-reduced-motion` 移除非必要动画；CSP 与 artifact 扫描无图片 URL。 | skeleton 只占布局，不伪造 canonical values。 | `accepted` |
| `media.error` | error/offline/stale/recovering 用 semantic `role="alert"`/`status`、简短事实说明、Retry/Reconnect Button 和 inline state icon；CSS 负责颜色和层级。 | `StateBoundary`、`BrowserConnectivityProvider`、`BoardLive`、`ExplorerPage`。 | 不使用错误插画、截图或 Plane 文案；错误 detail 保持现有 localized error/canonical boundary。 | 错误、offline、stale、recovering 分态可区分；retry 不隐藏失败；焦点可达且有回归路径；无 raster/media reference。 | 这三张图没有 error media，故这里记录真实处理而非补图。 | `accepted` |
| `content.imagery.policy` | Projects、Overview、Tasks 均只组装文字、数据、CSS surface 和 SVG icons；project cover、avatar、activity illustration、photo、纹理背景全部不渲染。 | 所有上述 owner，尤其 `ProjectsCollection`、`ProjectOverview`、`TasksWorkspace`。 | 禁止把 approved PNG 当背景或附件；任何未来 image-native 需求都必须另建 manifest、source/license 与审批链。 | 对 build artifact 与源码执行 `rg`/审阅：无三张 mock 路径、`.png/.jpg/.webp`、`url(...)`、`<img>` 或 `data:image`；视觉 review 确认 UI 仍有层级而非用渐变填洞。 | 这是本 manifest 的 no-raster 守门行。 | `accepted` |

## 执行顺序

1. 保持三张批准 PNG、`.png.json` 和 `.prompt.txt` 仅在 `.impeccable/mocks/plane-only-refresh/`，不创建 `.impeccable/assets` 下的图片文件。
2. 先落实共享 `Icon`、`ProductShell`/四区 shell、dark/light token 和 `StateBoundary`，以免各 surface 产生第二套 icon 或状态媒体。
3. 按 Projects collection → Project Overview → Tasks toolbar/Board → Task Inspector 的顺序接入 semantic owner；所有内容来自 typed read model/i18n。
4. 在 desktop、light/dark、窄屏、loading/empty/error/offline、键盘与 reduced-motion 场景截图和审阅；执行 strict CSP 与 no-raster 扫描。
5. 最后运行与当前 diff 匹配的仓库 gate，并审阅 `/app/` artifact 中没有新增图片或 Plane 代码/品牌引用。

## Blockers

当前无 manifest 级 blocker：批准图路径、metadata、prompt 和实现边界均可审计，且 no-raster 决策已由 brief 与三张图共同支持。

若父实现后来要求 production logo、cover、插画、照片或外部 icon/font，它超出本清单授权范围，必须在实现前补独立 source/license/审批记录；不得以截图裁剪或 Plane 资源替代。

## Assumptions

- `ProductShell`、`BoardView`、`ExplorerPage`、`TaskInspector` 与现有 read model 是可复用 owner；若父实现拆分出同职责组件，必须保留本清单的语义和边界。
- `@astryxdesign/core@0.3.0` 的 strict-CSP fallback 继续遵循 `apps/web/docs/astryx-spike.md`：需要动态样式的 TextInput/Dialog/Selector/Popover 用已验证的 native HTML/CSS fallback。
- mock 中的 `K`、项目名、任务内容、run id 和状态数量只用于 visual density/structure 审阅；生产数据、文案和状态仍以 canonical contract 为准。

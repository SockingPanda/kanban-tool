# Plane 前端参考：适合 kanban-tool 的信息架构与交互模式

> 结论状态：研究建议，不是已实现产品行为。
> 观察日期：2026-08-11（Asia/Shanghai）。
> Plane 固定版本：[`1c8a60f858d8472aa56e29994ec1c7926da2c6ce`](https://github.com/makeplane/plane/tree/1c8a60f858d8472aa56e29994ec1c7926da2c6ce)。
> kanban-tool 对照版本：`49a111d7dce94c80aa6a68501476cd2315623e61`。

## 结论

适合借鉴的不是 Plane 的 workspace、cycle、module 等产品模型，而是它处理高密度工作信息的方法：

1. 用「全局项目切换 → 当前 board 上下文 → 视图 → 快速详情」稳定页面层级。
2. 卡片和表格负责扫描，完整属性和动作进入右侧快速详情。
3. Board、List、筛选和显示选项只是同一事实集的不同镜头，不产生第二套状态。
4. 次要动作在 hover、focus 或菜单中渐进披露，但键盘和触屏仍有可达入口。
5. loading、过滤无结果、真正空数据、局部失败分别呈现并给出下一步。
6. 颜色只表达状态、优先级、风险和动作；其余层级靠间距、字号、表面和细边框完成。

对 kanban-tool 最合适的方向可以概括为：

> 学 Plane 的信息组织与交互语法，不移植 Plane 的产品模型。

## 证据边界

Plane 的导航分层、紧凑布局、工作项详情和命令入口可从其
[v1.2.0](https://github.com/makeplane/plane/releases/tag/v1.2.0)、
[v1.3.0](https://github.com/makeplane/plane/releases/tag/v1.3.0) 发布说明，以及固定提交中的
[Kanban card](https://github.com/makeplane/plane/blob/1c8a60f858d8472aa56e29994ec1c7926da2c6ce/apps/web/core/components/issues/issue-layouts/kanban/block.tsx)、
[layout switcher](https://github.com/makeplane/plane/blob/1c8a60f858d8472aa56e29994ec1c7926da2c6ce/apps/web/core/components/base-layouts/layout-switcher.tsx)、
[issue detail root](https://github.com/makeplane/plane/blob/1c8a60f858d8472aa56e29994ec1c7926da2c6ce/apps/web/core/components/issues/issue-detail/root.tsx)、
[resizable sidebar](https://github.com/makeplane/plane/blob/1c8a60f858d8472aa56e29994ec1c7926da2c6ce/apps/web/core/components/sidebar/resizable-sidebar.tsx)
中交叉验证。

Plane 仓库采用
[AGPL-3.0](https://github.com/makeplane/plane/blob/1c8a60f858d8472aa56e29994ec1c7926da2c6ce/LICENSE)。
本项目只吸收设计原则和交互模式，并用自己的领域语言、Astryx primitives、CSS Modules
和 typed service path 独立实现；不复制 Plane 的源码、组件、图标、插画、CSS token 或页面结构。

## 当前 kanban-tool 的主要差距

当前 Web 已经具备可靠的产品底座，不需要重做技术栈：

- `ProductShell` 已提供可折叠全局侧栏；board 内已有
  `board/list/map/runs/events` 路由。
- generated Web contract 已包含 boards read，URL 与 sync controller 也已有 board selector
  和 board isolation 语义；当前 shell 尚未提供可见的项目切换入口。
- `BoardLive` 是唯一 SSE/session owner，UI 重排无需也不得建立第二套同步生命周期。
- Board 已支持 legal transition、鼠标 DND、键盘 DND 和分页。
- `?task=` 已提供可深链的右侧 Inspector。
- 主题、密度、语言和侧栏偏好已经保存在本机 `kb:web:*` preference。
- Astryx + semantic native fallback 已在 strict CSP 下验证。

真正的问题集中在信息层次：

- 当前卡片同时铺开 status、assignee、reason、日期、heartbeat、labels、readiness
  和全部 legal actions，扫描成本高（`apps/web/src/features/board/BoardView.tsx:218`）。
- Inspector 固定为 18–24rem，内容分区和动作层级仍偏工程控制台；打开后会明显挤压看板
  （`apps/web/src/features/explorer/ExplorerPage.module.css:89`）。
- List 有搜索和多种筛选，但控件分散；搜索仅属于 List，输入会立即改 URL，没有全局搜索或命令面板
  （`apps/web/src/features/explorer/TaskListView.tsx:158`）。
- 当前 List 固定八列且没有列显隐或 row selection
  （`apps/web/src/features/explorer/TaskListView.tsx:233`）。
- `apps/web/docs/capability-ledger.md` 中关于 250ms 全局搜索、列显隐、row selection、
  board switcher 和列内虚拟滚动的描述与当前源码不一致；本文一律以源码为准。

## 适配矩阵

| Plane 模式 | 对 kanban-tool 的适配 | 优先级 | 边界 |
| --- | --- | --- | --- |
| 全局项目切换与 board 内水平 tab 分层 | 左栏顶端固定项目切换器，其下只放 Board、Signals、Ontology、System；当前 board 的 Board/List/Map/Runs/Events 留在内容顶部 | P0 | “项目”映射现有 canonical board，不引入 workspace/teamspace 或第二实体层 |
| 紧凑 Kanban card | 默认只显示 ref、单行标题、priority、少量 labels，以及一条 blocker/readiness 提示；次要元数据进 Inspector | P0 | claim、lease、非法状态原因不能被视觉降噪隐藏 |
| hover/focus quick actions | 卡片上只保留一个主动作和更多菜单；hover 与 `focus-visible` 同时显示 | P0 | 动作只能来自现有 exact legal transition；不能让 hover 成为唯一入口 |
| side-peek/detail | 沿用 `?task=`，把 Inspector 重组为 sticky header/action bar、Overview、Properties、Relations、Activity；窄屏全宽 | P0 | 不复制一套 detail 状态；mutation 仍走共享 controller/service |
| Board/List 即时切换 | 把现有路由做成紧凑 segmented switcher；保持 URL、选中任务和筛选上下文 | P0 | 两个视图必须读取同一 canonical task/status |
| 独立 filter row 与 active chips | 工具栏下固定一行筛选 chip，支持逐个清除和 clear all；补足 multi-status UI | P0 | 筛选只改变 projection，不改变 readiness/claim 事实 |
| 分态 empty/loading/error | 区分无任务、过滤无结果、加载中和局部失败；分别提供创建、清筛选、骨架和 Retry | P0 | mutation 失败必须回显 canonical 未成功，不能假性乐观 |
| display properties | List 先支持 density、sticky Ref/Title 和列显隐；Board 只提供紧凑/标准密度 | P1 | 当前不存在，属于新增本机 UI preference |
| Power K / keyboard | 窄化为 task ID/title、视图、run log 搜索与当前上下文动作；输入框内不劫持单键 | P1 | 当前不存在；claim/start/retry 必须调用原子 typed operation |
| saved view | 本机保存少量 `layout + filters + sort + display` preset，如“待认领”“我正在跑”“待 review” | P1 | 只是本地 lens，不是 server state、共享对象或第二状态机 |
| 可调整/hover-peek 侧栏 | 在现有 collapse 基础上增加 resize；是否做 hover peek 以后再评估 | P2 | 单用户默认顺序应稳定，暂不做复杂拖拽导航持久化 |
| 最近访问 | 只显示最近任务、失败 run 或上次 board，帮助恢复工作 | P2 | 不造 dashboard；最近项只能是可重建 projection |

Plane 对这些模式的产品说明还可见于官方
[Layouts](https://docs.plane.so/core-concepts/issues/layouts)、
[Filters](https://docs.plane.so/core-concepts/issues/visualise_filter)、
[Display options](https://docs.plane.so/core-concepts/issues/display-options)、
[Views](https://docs.plane.so/core-concepts/views)、
[Power K](https://docs.plane.so/core-concepts/power-k) 和
[Keyboard shortcuts](https://docs.plane.so/support/keyboard-shortcuts) 文档。

## 推荐的第一条落地纵向切片

第一批只做信息架构和视觉层次，不新增 backend、schema 或产品实体：

1. 在全局左栏加入可搜索的项目切换器，消费现有 boards read，并保持 board isolation。
2. 压缩 board header，把当前视图切换、筛选入口和“新建任务”收在一条稳定 toolbar。
3. 将 task card 收敛为扫描摘要，保留 blocker/readiness 的可见文字，并把次要动作移入可访问菜单。
4. 将现有 Inspector 改造成 side-peek 式详情，加入固定标题/动作区和清晰 section。
5. 统一 Board/List 的筛选表达，用 active chips、结果计数和 clear all 显示当前 lens。
6. 补齐无任务、过滤无结果、加载、局部失败四类状态及焦点恢复。

这条切片可以完全复用当前 route、query、SSE owner、task mutation controller
和 Astryx/CSS Modules，不需要改变 `tasks.status`、claim 事务或 protocol owner。

建议验收：

- 1440×900 下，至少能同时看到更多卡片标题，且 blocker/readiness、priority 和主动作仍可辨识。
- 鼠标、键盘和触屏都能打开详情及合法动作；关闭详情后焦点回到来源卡片或行。
- Board 与 List 切换、刷新和深链后仍保持 canonical URL/query 语义。
- 项目切换会释放旧 board session、清理旧 selection/filter/token，并以新 board canonical ID
  建立隔离连接；旧响应或事件不能污染新项目。
- hover-only 控件都有 `focus-visible` 或菜单替代，并带可读 label。
- loading、empty、no-result、error 不再共用模糊占位。
- strict CSP 不放宽，不新增第二 UI library，不引入 runtime inline style。

## 明确不借鉴

- workspace、team、member、invite、RBAC、presence、watcher、notification directory。
- SaaS 登录、公网访问、云同步、远程 workspace 或浏览器离线 mutation queue。
- Plane 的 cycles、modules、epics、roadmaps、Gantt、analytics；除非以后由本项目领域需求独立证明。
- 自定义 workflow/status，或把 board column 当成 `tasks.status` 之外的第二套状态机。
- 客户端通用 `transition(targetStatus)`、前端自动 claim/review、绕过 atomic claim 的快捷动作。
- 没有真实 typed bulk operation 时的 checkbox、row selection 或批量工具栏。
- server-saved/shareable views、复杂 PQL 和多 scope filter。
- 为“像 Plane”而复制其颜色、紫色品牌、插画、图标、圆角、阴影或源码组件。

## 实施护栏

- Astryx 仍是组件与 token 基础；不加入 Tailwind、Radix、shadcn 或第二组件库。
- `TextInput/Dialog/Selector/Popover` 在当前 Astryx 0.3.0 strict CSP 边界下继续使用已验证的
  semantic native fallback。
- `BoardLive` 继续作为唯一 live session owner；视图组件只消费其状态与动作。
- 所有 lifecycle action 继续从 server/task mutation controller 的 exact legal options 生成。
- 任何新的 rendered 能力必须先走 Rust protocol/catalog/schema owner 与 Web contract selection，
  不能凭 Plane 已有功能直接在 adapter 中拼装。
- saved view、density、列显隐等偏好若落地，只保存 UI lens，不保存 canonical task fact。

## 参考来源

Plane 官方来源：

- [Plane repository](https://github.com/makeplane/plane)
- [固定提交 `1c8a60f8`](https://github.com/makeplane/plane/tree/1c8a60f858d8472aa56e29994ec1c7926da2c6ce)
- [v1.2.0 release](https://github.com/makeplane/plane/releases/tag/v1.2.0)
- [v1.3.0 release](https://github.com/makeplane/plane/releases/tag/v1.3.0)
- [Customize navigation](https://docs.plane.so/workspaces-and-users/customize-navigation)
- [Layouts](https://docs.plane.so/core-concepts/issues/layouts)
- [Filters](https://docs.plane.so/core-concepts/issues/visualise_filter)
- [Display options](https://docs.plane.so/core-concepts/issues/display-options)
- [Views](https://docs.plane.so/core-concepts/views)
- [Power K](https://docs.plane.so/core-concepts/power-k)
- [Keyboard shortcuts](https://docs.plane.so/support/keyboard-shortcuts)

kanban-tool 对照来源：

- `apps/web/src/ProductShell.tsx`
- `apps/web/src/App.tsx`
- `apps/web/src/features/board/BoardView.tsx`
- `apps/web/src/features/explorer/ExplorerPage.tsx`
- `apps/web/src/features/explorer/TaskInspector.tsx`
- `apps/web/src/features/explorer/TaskListView.tsx`
- `apps/web/src/lib/router.ts`
- `apps/web/docs/astryx-spike.md`
- `apps/web/docs/capability-ledger.md`
- `crates/kanban-core/docs/state_machine.md`
- `docs/architecture.md`

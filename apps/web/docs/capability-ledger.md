# Astryx Web UI 能力 Ledger

## 1. 作用、范围与来源

本文件是 browser-first Astryx Web UI 重写的 active migration ledger。它以现有
`apps/desktop` 的**用户实际可见 rendered surface** 为范围，不把 `KanbanApi` 上存在但当前没有
UI 入口的 typed operation 自动扩成产品范围。目标是由 `kanban serve` 同源托管 `/app/`，让 Browser
和 Tauri 消费同一 Web artifact；现有功能语义、状态机、错误语义和确认边界保持不变，视觉采用
Astryx baseline。

事实来源：

- Shell/状态编排：`apps/desktop/src/App.tsx`、`apps/desktop/src/app/AppShell.tsx`、
  `apps/desktop/src/app/useRuntimeConfigState.ts`、`useTaskCollectionState.ts`、
  `useSelectedTaskDetailState.ts`、`useTaskMutations.ts`。
- 页面与交互：`apps/desktop/src/features/**` 及其同名测试。
- 当前 typed HTTP 边界：`apps/desktop/src/lib/api.ts`、`apps/desktop/src/lib/api/**`；最终 wire
  contract 由 `kanban-protocol` 及其生成 artifact 持有。
- 事件缓存/失效：`apps/desktop/src/features/events/event-invalidation.ts`、
  `event-polling.ts`、`event-cache.ts`。

当前 URL 基线为 **none**：没有 router/history；view、selected task、board、filter、pagination
均为 React state。目标 URL 是 `/app/`、`/app/boards/:boardSlug/{board,list,map,runs,events,signals,ontology,health,maintenance}`、
`/app/settings`，Task Inspector 使用 `?task=t_...`；每个 target route 都必须保留本表中的 capability。

## 2. 统一完成规则

一行能力只有在以下证据齐全时才可标记为 cutover-ready：

1. Browser 与 Tauri 通过同一 `/app/` artifact 完成用户结果，且 URL 可复制/刷新恢复关键状态；
2. reads/writes 通过生成 contract 的 typed client 和 runtime validator，禁止 unchecked generic cast；
3. 成功、空态、loading、degraded、错误和确认边界均有可观察行为；
4. SSE 事件能按本表的 invalidation class 收敛；断线按约定 refetch/polling/reconnect；
5. 对应现有测试保持通过，并补列出的 Playwright flow；键盘与可访问性等价；
6. stage review 记录 evidence、风险和 rollback 点。未列为目标或明确 dead/no-op 的内容不得在重写中
   顺手加入。

所有 SSE 失效按 [`sse-invalidation.md`](./sse-invalidation.md) 的精确 literal、epoch/isolation、
query root 与 observed-scope 规则执行；ledger 中的 `task.*`、`dependency.*`、transition 等写法
只是文档集合引用，运行时不得 wildcard 匹配，必须按该文档 §3.2 的 exact literal 表和 scope gate
执行。其中 active-board business event 的 `search-status`，以及 `task_id` 非空时的 observed global
`maintenance-status`，属于跨 capability 的 projection 规则，不在每一行重复猜测。`task_id == null` 的 service event 仍可能只推进 search cursor 而没有 FTS/vector job，
该 stale/no-convergence 限制必须作为 Stage 02 evidence，而不是通过循环 refetch 掩盖。

## 3. Rendered capability ledger

| id | target route / current URL | 用户结果（当前 rendered surface） | reads / writes | state / error | existing tests | future Playwright | SSE invalidation | cutover decision |
|---|---|---|---|---|---|---|---|---|
| `shell.runtime` | `/app/` / `none`（React state） | 加载 runtime、显示 active board/actor/API、侧栏收展、导航 10 个 view、theme cycle、全局错误条 | `runtime_config`（Tauri）或 Vite env；`GET /api/v1/boards?include_archived=false`；`GET /api/v1/boards/:board/columns`；board/list/map/runs 读 `GET /api/v1/stats?board=`；无 shell 写入（board switch 走 `set_runtime_board`/Web config） | runtime/collection/detail 错误统一 AppShell assertive alert；pending action；board switch 清理 selected/draft/filter/token/error | `app/task-explorer-chrome.test.ts`、`shadcn-controls.test.ts`、`shell-state-boundaries.test.ts`、`board-switch-state.test.ts`、`theme.test.ts`、`sidebar-state.test.ts`、`sidebar-animation.test.ts`、`input-accessibility.test.ts`、`layout-scroll-contract.test.ts`、`queue-counts.test.ts`、`task-selection.test.ts` | `P0-shell-runtime`、`P0-navigation-url`、`P1-theme-and-sidebar` | `board.created`/`board.archived`：inactive lifecycle 由 window focus + visible ≤15s global freshness 维护；active `board.archived` 立即刷新 boards/columns/task projection；未知/断线触发 runtime reconnect 或全局 error | 纳入；同源 `/app/runtime.json` 取代生产 Vite/Tauri 分叉，保持 board isolation |
| `task.collection` | `/app/boards/:board/{board,list}` / `none` | 全局搜索（250ms debounce）、refresh、Archived 开关；板/列表读取任务集合 | Board: `GET /api/v1/boards/:board/tasks/by-status?...`；List: `GET /api/v1/boards/:board/tasks?...`；stats/columns；无写 | loading/previous data/empty；查询错误进全局错误条；`searchMeta` 当前为 null | `features/board/useBoardTasks.test.ts`、`app/task-explorer-chrome.test.ts` | `P0-board-load`、`P0-list-search`、`P1-refresh-and-archive` | `task.*`/`dependency.*` → board tasks/stats/search status/map 按事件分类 | 纳入；当前 query/key 语义迁移为 URL state + generated API |
| `board.view` | `/app/boards/:board/board` / `none` | 列内虚拟滚动、task card 选择、DND 跨列；卡片显示 status/priority/due/scheduled/heartbeat/steps/deps/labels/reason | 读同 `task.collection` Board endpoint；写 transitions：`POST /api/v1/tasks/:id/transitions/{specify,promote,claim,complete,submit-review,block,unblock,archive}` | 非法 drop/同列 i18n 错误；triage specify 需 description；block 需 reason；running 无 token 的 complete/block/archive force confirm | `board-card-state.test.ts`、`board-layout.test.ts`、`drag-policy.test.ts`、`useBoardTasks.test.ts` | `P0-board-dnd`、`P0-board-open-detail`、`P1-board-keyboard-transition` | `task.*`/`dependency.*` 更新 board rows、stats、map、selected detail | 纳入；保持状态机合法动作与 force-confirm |
| `list.view` | `/app/boards/:board/list` / `none` | status/priority/plan filters、reset、列显隐/重置、sort、rows/page、pagination、open detail；当前有 row-selection count | 读 `GET /api/v1/boards/:board/tasks?...`；stats/columns；列表本身无写 | empty/loading/refreshing；页变更会 prune stale row selection；query 错误全局 alert | `features/list/table-state.test.ts`、`app/task-explorer-chrome.test.ts`、`shadcn-controls.test.ts` | `P0-list-filter-sort-page`、`P1-list-column-menu`、`P1-list-open-detail` | `task.*`/`dependency.*` → tasks/stats/search status；comment-only 不刷新 list stats | 纳入 filters/sort/page；删除没有 bulk action 或其他用户结果的 row selection |
| `map.view` | `/app/boards/:board/map` / `none` | filter all/blocked/ready/running/unplanned/incomplete-steps、hide isolated、show done context、zoom、refresh、node inspect/open detail | `GET /api/v1/boards/:board/task-map?active_only=true&context_depth=1&include_done_context=...&include_archived_context=false&hide_isolated=...&limit_nodes=240`；detail 读 selected task | loading/empty/map failed/truncated local Alert；selected node 可被 filter 隐藏 | `task-map/BoardTaskMapView.test.ts`、`TaskGraphCanvas.test.ts`、`TaskGraphNodeCard.test.tsx`、`task-graph-layout.test.ts`、`task-graph-scale.test.ts`、`task-map-colors.test.ts` | `P0-map-load-filter`、`P1-map-open-detail`、`P1-map-zoom` | 仅按 `sse-invalidation.md` 的 map `B` 分类刷新：`dependency.added`、`dependency.removed`、`task.created`、`task.updated`、`task.archived`、`task.specified`、`task.promoted`、`task.claimed`、`task.completed`、`task.submitted_for_review`、`task.blocked`、`task.unblocked`、`task.recomputed`、`task.released`、`task.reopened`、`task.reclaimed`、`task.execution_plan.not_required`、`task.execution_plan.planned`、`task.execution_plan.unplanned`、`task.step.created`、`task.step.done`、`task.step.removed`、`task.step.reopened`、`task.step.skipped`、`task.step.updated`、`task.export_sanitized`；这些 map `B` 事件同时失效 active-board observed `task-neighborhood(*)`；`task.heartbeat`、`task.comment.created`、label/proposal/retry policy 不刷新 map；未知/gap 保守 refetch | 纳入；limit 必须保持 240，ELK/renderer lazy |
| `runs.view` | `/app/boards/:board/runs` / `none` | 依赖已选 task；显示 run rows（status/worker/owner/start/finish/exit/error）和首个有 log 的 run log | `GET /api/v1/tasks/:id/runs`；有 log 时 `GET /api/v1/runs/:run_id/log` | 无 task/无 runs/无 log empty；task detail query 错误全局 alert；RunRow 当前不可点选，log 自动选择 | `features/task-detail/useTaskDetail.test.ts`、`detail-invalidation.test.ts`、`lib/runs-contract.test.ts` | `P0-runs-open`、`P1-run-log` | 当前 protocol 不存在 `run.*`；已声明的 `task.*` envelope 携 `run_id` 时定向失效 `task-runs`/`task-run-log`/detail；未来 `run.*` 必须先更新 protocol，未知 kind 走 conservative fallback | 纳入；保留依赖已有 selection 的行为，URL 化 selected task |
| `events.view` | `/app/boards/:board/events` / `none` | 当前首批 150 条 board events（按 `id ASC`，不是最近 150），目标 cache 以 `id` 仅保留最后 150；手动 refresh，显示 kind/task/run/time/actor | `GET /api/v1/events?board=&after=0&limit=150`；无写 | loading/empty；当前 query error 退化为空态，重写需显式错误；后台 poll error 走全局 alert | `events/event-cache.test.ts`、`event-invalidation.test.ts`、`event-polling.test.ts` | `P0-events-stream`、`P1-events-refresh` | 当前单页不能视为 fresh；目标持久 SSE 先分页 catch-up 到 high-watermark，再按 `id` 合并去重并裁剪最后 150；未知/gap → refetch | 纳入；轮询升级为持久 SSE primary，保留 catch-up/fallback |
| `signals.view` | `/app/boards/:board/signals` / `none` | status tabs（review/open/confirmed/resolved/rejected/superseded/all）、kind CSV/task ref filter、row select、refresh、observation/evidence JSON | `GET /api/v1/boards/:board/signals/review?...`；`GET /api/v1/signals/:signal_id`；无写 | local Alert `presentApiError`；loading/empty/none selection | `features/signals/SignalsWorkbench.test.tsx` | `P0-signals-filter-detail`、`P1-signals-refresh` | signal events → signals root/detail；未知 kind 保守 signals refetch | 纳入；generic signal review，不扩成 ontology mutation |
| `ontology.view` | `/app/boards/:board/ontology` / `none` | include all、groupBy label/atom/proposal、signal/group select、refresh、atom explain；lifecycle confirm/reject/resolve no change | reads：`GET /api/v1/boards/:board/label-ontology/signals`、`/label-ontology/review`、`GET /api/v1/label-ontology/signals/:id`、`GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain`；write：`POST /api/v1/boards/:board/label-ontology/actions`（仅 lifecycle + reason） | local error Alert；reason/status gate；rendered lifecycle mutation success invalidates ontology root/signal detail，service 证据表明不改 canonical atoms；若未来暴露 semantics/atom mutation，必须另加 `label-ontology-atom(board,*)` invalidation | `features/ontology/OntologyReviewWorkbench.test.tsx` | `P0-ontology-review`、`P1-ontology-lifecycle`、`P1-atom-explain` | 当前 generic `signal.*` 不触碰 `label_ontology_*`；rendered ontology lifecycle 走 root/detail；未知 kind（含 semantics/atom producer）保守 F | 纳入 rendered review/lifecycle；canonical apply/validate/revert 保持非目标 |
| `health.view` | `/app/boards/:board/health` / `none` | refresh、ok/db/version/db_path/db_fingerprint metrics、runtime config display | `GET /health`；无写 | loading/empty/no response；local Alert `presentApiError` | `features/health/health-runtime.test.ts` | `P0-health`、`P1-health-error` | health 不是 event projection；SSE disconnect 不覆盖 health query | 纳入；同源 runtime/API identity 统一 |
| `maintenance.view` | `/app/boards/:board/maintenance` / `none` | stats/search status/global maintenance status；doctor、checkpoint、backup/export、portable import、legacy SQLite v30 import、vacuum、run/rebuild/cleanup；确认对话框及结果 | reads：`GET /api/v1/stats?board=`、`GET /api/v1/search/status?board=`、global `GET /api/v1/maintenance/status`；writes：`POST /api/v1/maintenance/{doctor,checkpoint,backup,export,import,import-v30,vacuum,run,rebuild,cleanup}` | 每 panel `ErrorText`；destructive/import/maintenance 有确认；maintenance query observed 且 window visible 时独立 ≤5s global polling（SSE healthy 也不停），focus/visibility 立即 refetch；每次本地 maintenance mutation 发起与 settle 都 invalidate/refetch global status | `lib/maintenance-contract.test.ts` | `P0-maintenance-read`、`P1-maintenance-confirmations`、`P1-maintenance-results` | 当前 protocol 不存在 `maintenance.*`/`projection.*`；HTTP mutation scope 的每次本地写入都 invalidate global `maintenance-status`，并 refetch 相关 status/stats/search roots；未知 kind 走 conservative fallback；未来事件必须先更新 protocol | 纳入；不因 UI 重写引入 DB/schema migration |
| `settings.view` | `/app/settings` / `none` | locale system/zh-CN/en selector；只读 board/actor/API/database/fingerprint | `GET /health`；locale/theme/sidebar 写 localStorage；无 server write | health error local Alert；reported value fallback `not reported` | `app/theme.test.ts`、`lib/runtime-board.test.ts`、`features/health/health-runtime.test.ts` | `P1-settings-locale-runtime` | 无直接 event invalidation；health 由 query refetch | 纳入；localStorage 使用新 `kb:web:*`，不迁移旧 key |
| `task.detail` | `/app/boards/:board/{board,list,map,runs}?task=t_...` / `none`（right Sheet） | header/metadata/markdown description；one-hop map；collapsible dependencies/steps/comments/runs/events；edit/save/cancel；动作确认 | reads：`GET /api/v1/tasks/:id`、`/dependencies`、`/neighborhood?depth=1&limit_nodes=40`、`/steps`、`/comments`、lazy `/runs`、`/events?board&task_id&limit=50`、`/runs/:run/log`、`/attachments`、manual `/labels/suggestions`；writes 见下列 detail rows | detail child query errors合并全局 alert；panel loading/empty；expanded state 控制低频 reads；mutation pending/action label | `app/task-detail-capability-cutline.test.ts`、`features/task-detail/useTaskDetail.test.ts`、`detail-invalidation.test.ts`、`comment-list-state.test.ts`、`dependency-group.test.tsx`、`description-state.test.ts`、`label-suggestions.test.ts`、`markdown-description.test.tsx`、`task-draft.test.ts` | `P0-task-inspector-deeplink`、`P0-task-detail-read`、`P1-task-panels` | 当前 protocol 无 `run.*`；已声明 `task.*`（有 `run_id` 时）及 dependency/comment/step literal 按 child bucket 定向失效；若 `task-label-suggestions(task_id)` 已手动观察，也按 title/description/label mutation 定向失效；未知 kind → conservative fallback/refetch | 纳入完整 rendered detail；保留 lazy panel 与确认语义 |
| `task.create` | `/app/boards/:board/{board,list}` / `none`（header dialog） | title 必填、description 可选、first required step 可选；成功选中新 task 并关闭 dialog | write `POST /api/v1/boards/:board/tasks`；可追加 `POST /api/v1/tasks/:id/steps` | title 空/creating disabled；失败全局 alert；成功 invalidate board+task | `lib/create-task-contract.test.ts`、`app/task-explorer-chrome.test.ts` | `P0-task-create` | `task.created` → active board tasks/stats/search-status/map/events；不刷新 global `boards`（board switcher 走 focus/15s freshness path） | 纳入；idempotency/actor 保持 generated contract |
| `task.transition` | detail / drag：`/app/boards/:board/{board,list}?task=` / `none` | Specify/Promote/Claim/Heartbeat/Complete/Review/Block/Unblock/Archive；DND 同一 legal policy；block reason；force confirmation | writes `POST /api/v1/tasks/:id/transitions/{action}`；claim 返回 claim token | `legal-actions.ts`/`drag-policy.ts` gate status/claim/description/required steps；失败全局 alert；success reconcile token + invalidate | `features/task-actions/legal-actions.test.ts`、`features/board/drag-policy.test.ts`、`lib/action-policy.test.ts`、`lib/transitions-contract.test.ts` | `P0-transition-matrix`、`P0-force-confirm`、`P1-dnd-keyboard-command` | transition event → task/detail/board/stats/map/runs according kind | 纳入；不新增 Reopen/Release UI；保持现有 legal action cutline |
| `task.comments` | detail `?task=` / `none`（collapsible） | newest/oldest sort、page previous/next、comment body、add comment；decision/signal metadata 展示 | read `GET /api/v1/tasks/:id/comments`；write `POST /api/v1/tasks/:id/comments` | empty/page disabled/pending；失败全局 alert；comment mutation 只刷新 timeline buckets | `comment-list-state.test.ts`、`lib/comments-contract.test.ts`、`detail-invalidation.test.ts` | `P1-comments` | `task.comment.created` → task-comments/task-events/search status，不刷新 steps/map/stats | 纳入；保持 timeline scope |
| `task.dependencies` | detail `?task=` / `none`（collapsible） | parent/child links select task、add by ref、remove parent | read `GET /api/v1/tasks/:id/dependencies`；writes POST `/dependencies`, DELETE `/dependencies/:parent` | input/pending/empty；cycle/server errors全局 alert；mutation只刷新 dependency + neighborhood | `dependency-group.test.tsx`、`app/task-mutation-invalidation.test.ts`、`lib/api.test.ts` | `P1-dependencies` | `dependency.added/removed` → dependencies/neighborhood/tasks/map/events | 纳入；保留 board isolation/cycle errors |
| `task.steps` | detail `?task=` / `none`（execution plan collapsible） | 查看 required/optional/link 状态、create required step、attach linked task、mark plan not required、select linked task | read `GET /api/v1/tasks/:id/steps`；writes `POST /api/v1/tasks/:id/steps`、`POST /api/v1/tasks/:id/execution-plan/not-required` | input/pending/empty；Complete 对 incomplete required steps disabled；失败全局 alert | `features/task-detail/useTaskDetail.test.ts`、`lib/steps-contract.test.ts`、`task-detail-capability-cutline.test.ts` | `P1-steps-create-link`、`P1-plan-not-required` | `task.step.created/done/removed/reopened/skipped/updated` → task-steps/neighborhood/detail/events；status transition affects board/stats；未来其他 step kind 先更新 protocol，未知 kind conservative fallback | 纳入当前 rendered actions；update/remove/complete/skip/reopen controls 不扩入 |
| `task.labels` | detail `?task=` / `none`（labels section） | add/remove label；显式 request suggestions；apply suggested label；degraded reasons 展示 | reads manual `GET /api/v1/tasks/:id/labels/suggestions`；writes POST `/labels`、DELETE `/labels/:label` | suggestions 默认 disabled/not fetched；pending/error local panel + global action error；already applied disabled | `label-suggestions.test.ts`、`task-detail-capability-cutline.test.ts`、`lib/api.test.ts` | `P1-labels`、`P1-label-suggestions` | `task.label.added/removed` 与 task title/description update → 若 `task-label-suggestions(task_id)` 已观察则定向失效；其他 label/task events 仍按 catalog；不把 ontology semantics 变成 task label UI | 纳入；suggestions 保持手动，不把 ontology semantics 变成 task label UI |
| `task.attachments` | detail `?task=` / `none`（attachments section） | file choose/upload、download、delete；metadata filename/type/size/hash/time | read `GET /api/v1/tasks/:id/attachments`；write POST `/attachments`、GET bytes `/attachments/:attachment_id`、DELETE `/attachments/:attachment_id` | no file/pending/empty；API error全局 action alert；当前 delete 无二次确认 | `lib/attachments-contract.test.ts`、`app/task-detail-capability-cutline.test.ts`、`app/task-mutation-invalidation.test.ts` | `P1-attachments` | 当前 protocol 不存在 `attachment.*`；HTTP mutation scope 定向失效 `task-attachments(task_id)`/相关 task timeline，未知 kind 走 conservative fallback；当前 §1.2 样本按 unknown 处理；若要提升为 known 或新增事件，先更新 protocol/schema/fixture/taxonomy | 纳入 rendered upload/download/delete；保持当前无二次确认的边界 |

## 4. Typed-but-not-rendered 非目标

以下 operation 当前只有 `KanbanApi`/contract/test 入口，没有 rendered UI；本次 parity 不自动加入：

- Board CRUD：`createBoard`、`getBoard`、`archiveBoard`（当前只读 board switcher）。
- Context/vector/proposal 扩展：`buildContext`、vector UI、proposal/label semantic mutation UI；README 中的
  “vector/context” 不是当前 rendered surface。
- Step mutation：`updateStep`、`removeStep`、`completeStep`、`skipStep`、`reopenStep`；当前只显示步骤状态并
  创建/链接/标记 plan not required。
- Lifecycle convenience/transition：`releaseTask`、`transition` 的 `release`/`reopen`；当前 legal actions 与
  drag policy 不暴露 Reopen/Release。
- Search/event/signal/run wrappers：`searchTasks`、`searchTasksByStatus`、`listEventsAfter`、generic
  `listSignals`、`getRun`；rendered surface 使用 `listTasks`/`listTasksByStatus`、`listBoardEvents`、
  `reviewSignals`、`listRuns`/run log。
- 仅 API alias：`export`、`import`、`importV30`；UI 走 `exportData`、`importData`、
  `importLegacySqliteV30`。

若未来要纳入其中任一 operation，必须新建明确 capability、验收 flow、contract/schema 证据和 scope 决策，
不得在迁移中以“顺手补齐”为由加入。

## 5. Dead/no-op 候选与删除规则

- **删除：** `features/list/ListView.tsx` 的 `select` 列（全选/行 checkbox）只更新 `rowSelection` 并
  显示 `{count} selected`，没有 bulk command、回调或 API。它不进入 Astryx Web。
- **删除：** `AppShell.tsx` 的 `SearchBackendBadge` 依赖 `searchMeta`，而
  `useBoardTasks.loadBoardTasks` 当前固定返回 `searchMeta: null`，所以它不是 rendered capability。若未来
  要显示 search backend 状态，需要另建 capability 并接入真实 `/search/status` 或 search metadata。
- **删除：** `features/board/board-config.ts` 的 hard-coded `fallbackColumns` 不得在新 Web 形成第二份
  board column 配置；加载失败显示 typed error，空列使用真实空态，列和状态映射只来自 server contract。
- `RunsView` 的 RunRow 不可点击不是自动 dead：当前 run log 自动选择第一个 `has_log` run。若新设计改为可选
  run，需补 URL/Playwright/contract；否则保持自动选择并记录为 intentional。
- `TaskGraphNodeCard` click=inspect、double-click/open detail 是两个不同结果；重写必须提供键盘等价路径，不能
  因事件重构丢失 open detail。
- 旧 shell 的 command/status/inert controls 已由 `app/task-explorer-chrome.test.ts` 约束为不存在；不得在
  Astryx shell 回填。

## 6. Cutover 证据索引

每个 row 的 `future Playwright` id 在实现阶段扩展为真实 spec 路径，并将以下证据回填到 Kanban stage task：

- Browser Chromium full ledger；Firefox key paths；Linux packaged Tauri smoke；
- SSE reconnect/catch-up/unknown-event/gap → refetch；断线 polling fallback；
- WCAG 2.2 AA、键盘核心流程、focus、axe critical/serious zero；
- 视觉基线与性能预算；
- exact matched host/web/deb/manifest rollback，DB unchanged。

本文件只记录可复用的当前能力与迁移边界；实现进度、单次失败、review finding 和一次性 workaround 留在
Kanban stage task 或 runbook，不把本 ledger 变成日志。

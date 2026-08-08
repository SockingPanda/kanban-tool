# 持久 SSE 与 Web Query Invalidation

本文是 Stage 00 为 Astryx Web UI 留下的可执行同步契约。它把当前
`kanban-protocol::StreamEventData` 的事件 envelope、当前 rendered surface 的 query key 和
后续 Stage 02 的持久 SSE 生命周期连接起来。事件 kind 的事实源仍是
[`KNOWN_EVENT_KINDS`](../../../crates/kanban-protocol/src/event_payload.rs) 与
`StreamEventData` schema；本页不新增服务器事件，也不把 Desktop 里曾出现但 protocol 未声明的
`board.updated`、`task.restored`、`task.deleted` 或 `run.*` 当成当前能力。

相关边界：

- 当前 rendered capability 和目标页面见 [`capability-ledger.md`](./capability-ledger.md)。
- wire DTO、schema 和 SSE endpoint declaration 见
  [`crates/kanban-protocol/docs/schema.md`](../../../crates/kanban-protocol/docs/schema.md) 及
  [`history_catalog.rs`](../../../crates/kanban-protocol/src/history_catalog.rs)。
- browser-first 与持久 SSE 的架构决定见
  [`ADR 0006`](../../../docs/adr/0006_browser_first_astryx_web_ui.md)。
- 下文的 `invalidate` 是把 query 标为 stale 并让活跃 observer 重新读取；`refetch` 是立即读取。
  “全 board refetch”指同一 active board 的所有相关 query 一起 refetch，而不是读另一个 board。

## 1. 事件 envelope 与 discriminated union

### 1.1 Transport 形状

每个 SSE frame 的目标形状保持现有 `/api/v1/stream/events` wire contract：

```text
event: <kind>
id: <id>
data: {
  "id": 123,
  "event_id": "e_...",
  "board_id": "b_...",
  "task_id": "t_..." | null,
  "run_id": "r_..." | null,
  "kind": "<kind>",
  "actor": "..." | null,
  "payload": <typed payload or arbitrary JSON>,
  "created_at": 1700000000
}
```

`id` 是 SSE cursor（当前为 `i64`），`event_id` 是存储事件的稳定字符串标识；两者都必须参与
去重与异常检测。`event` 字段和 JSON 的 `kind` 必须相同。`board_id`、`task_id`、`run_id`、
`actor` 在 envelope 中是显式 nullable 字段，不能按字段缺失处理。

### 1.2 Known union

生成 contract 必须把下列 literal kind 生成成带 `kind` 常量的 discriminated union。每个分支的
`payload` 使用 protocol 已有 typed DTO，不能由 Web 手写第二份 payload 类型：

| 家族（仅为实现分组） | 当前精确 kind literal |
|---|---|
| board | `board.created`、`board.archived` |
| dependency | `dependency.added`、`dependency.removed` |
| label | `label.created`、`label.deleted` |
| signal | `signal.recorded`、`signal.reviewed` |
| task 基础/状态 | `task.archived`、`task.blocked`、`task.claimed`、`task.completed`、`task.created`、`task.promoted`、`task.reclaimed`、`task.recomputed`、`task.released`、`task.reopened`、`task.specified`、`task.submitted_for_review`、`task.unblocked`、`task.updated`、`task.export_sanitized` |
| task comment | `task.comment.created` |
| execution plan | `task.execution_plan.not_required`、`task.execution_plan.planned`、`task.execution_plan.unplanned` |
| heartbeat | `task.heartbeat` |
| task label | `task.label.added`、`task.label.removed` |
| label proposal | `task.label_proposal.accepted`、`task.label_proposal.proposed`、`task.label_proposal.rejected` |
| retry policy | `task.retry_policy.updated` |
| task step | `task.step.created`、`task.step.done`、`task.step.removed`、`task.step.reopened`、`task.step.skipped`、`task.step.updated` |

上表的“家族”只是文档和实现分组，不是新的 wire pattern。例如 `task.step.*` 在本页只代表上
面列出的六个 literal，不允许用 `startsWith("task.step.")` 把未来 kind 静默当成已知事件。生成的
union 必须保留 literal；未知 kind 走 fallback。

Known payload 的有效性是该 literal 在当前 protocol 中声明的**全部精确 typed branches 的并集**：
例如 `board.created` 可匹配 `BoardCreatedPayload` 或 `EmptyPayload`，`task.blocked` 可匹配
reason/retry/result 三种 payload，`task.reclaimed` 可匹配 reclaimed/retry 两种 payload；只要
命中其中任一 branch 即为 known valid，只有全部 branch 都不匹配才是 payload mismatch。Stage 02
必须审计当前 service producers；只有被证明确实不可达的历史 fallback 才能删除，并同步更新
protocol/schema/fixtures。Web 不得私自收紧 union，也不能把当前合法 variant 判作 unknown。

当前 protocol 没有 `attachment.*`、`maintenance.*`、`projection.*` 或 `run.*` 业务 kind；
attachments/maintenance 的 HTTP mutation 仍按 mutation scope 处理，不能为了填补 ledger 而在
SSE taxonomy 中发明这些事件。将来若 server 增加 kind，必须先更新 protocol union、schema、fixture
和本页，再进入 Web 实现。

当前 service 可能已经持久化但 protocol 尚未纳入 known union 的样本包括
`task.attachment.created`、`task.attachment.deleted`、`label.semantics.updated` 和
`label.semantics.deleted`。它们必须在 Stage02/09 作为真实 unknown-path fixture：保留原始 envelope、
触发 conservative fallback，不得偷偷升级为 known kind。

### 1.3 Unknown fallback 与 schema 异常

```ts
type KnownStreamEvent = /* generated union, kind is a literal */
type UnknownStreamEvent = {
  id: number
  event_id: string
  board_id: string
  task_id: string | null
  run_id: string | null
  kind: string // not one of the generated literals
  actor: string | null
  payload: unknown // lossless JSON; never cast to a known payload
  created_at: number
}
```

- `kind` 不在当前 40 个 literal 中时，且 envelope 已通过当前 connection 的 epoch 与 active-board
  isolation gate，保留整条 envelope 和原始 `payload`，记录 telemetry，按 **unknown conservative
  path** 处理：先完成 canonical fingerprint/dedupe，再把原 envelope 合并到 event cache，随后对
  active board 做全量 refetch；不能在 dedupe/cursor 之前写 cache。
- 已知 kind 但 payload 不通过该 kind 的全部 typed branches，或 task-scope cross-validator 失败时，
  不得降级成 known 空 payload，也不得忽略事件；这是 protocol mismatch，直接 `F` + reconnect，
  且不得写 `seenIds`/`seenEventIds`、cursor 或任何 event/query cache。
- unknown 事件不产生 canonical/server mutation，也不驱动“猜出来”的局部 projection patch；完成
  上述去重后允许继续读取已知事件，但在全量 refetch 完成前不得把 cache 视为一致。

### 1.4 Transport-control heartbeat

`task.heartbeat` 是当前 40 个 literal 中的业务事件，必须按表中 task projection 规则处理；它不
是连接保活的替代品。浏览器看不到 SSE comment，因此客户端可观测的 heartbeat 必须是独立的
transport-control named event，精确 wire name 冻结为 `event: kb-heartbeat`：它没有业务 `id`，不进入
`StreamEventData` union，也不写入 `events(board)`/`task-events(task_id)` cache。SSE comment 可额外
作为网络层 keepalive，但不能作为 client timeout 或 switchback 的证据。control frame 的 DTO、
精确 event name、data 形状和 schema 必须在 Stage 02 作为 protocol contract + tests 冻结，不能
把它臆造为新的业务 kind。

### 1.5 Processing order 与 isolation gate

每个 connection 都有递增的 `connectionEpoch`/generation token（绑定 active board 与该次
`EventSource` 注册）；board switch、主动 reconnect 或 recovery 都先使旧 epoch 失效，再创建新
epoch。每个 raw SSE frame 必须严格按以下顺序处理，不能先 union/cache 再检查 board：

0. **Epoch/generation gate：**frame 回调携带的 epoch 若不是当前 active connection epoch，立即
   silent discard（business 与 `kb-heartbeat` control 均如此）：不重置新连接 liveness，不触发
   `F`/reconnect，不进入 envelope/union/fingerprint/cursor/cache/invalidation。只有当前 epoch 的
   frame 才继续；因此旧 board/旧 connection 的 cross-board frame 也不能制造 isolation anomaly。
1. **按 SSE `event` name 路由 control frame：**先检查 SSE `event` name 是否严格等于
   `kb-heartbeat`。命中时只使用 Stage 02 冻结的独立 heartbeat DTO/schema 验证 control data；合法
   control frame 只重置 liveness timer，不进入 business envelope、cursor、board-isolation、union、
   fingerprint 或任何 event/query cache，也不触发 invalidation。`kb-heartbeat` control frame 的
   结构不合法（包括携带不应有的业务字段）按 protocol anomaly，执行 `F` + reconnect。只有 event
   name 不是 `kb-heartbeat` 的 frame 才进入下面的 business path；当前不开放其他 control name。
2. **Envelope validate：**先验证 business frame 的 SSE `id`/`event` 与 JSON envelope 的结构、必填
   字段和字段类型；header `id` 必须可解析为 safe integer，且数值严格等于 `data.id`，同时要求
   `event === kind`。header/data id 不一致、超 safe integer、非法/溢出值均是 protocol anomaly，
   立即 `F` + reconnect，不推进 cursor；绝不能让 header id 推进 cursor、却用另一份 data id 做
   fingerprint。此步只确认 envelope 形状，不先把 `kind` 解析成 known/unknown union。
3. **Board isolation gate：**用 envelope 的 `board_id` 与当前 canonical `activeBoardId` 比较。
   若不匹配（包括尚未识别的 unknown kind），直接 discard raw frame：不进入 union、fingerprint、
   cursor、任何 event/query cache，也不触发局部 invalidation；记录 isolation anomaly，立刻对
   active board 执行 `F` 并 reconnect。
4. **Known payload + task-scope cross-validation：**通过 isolation 的 known frame 必须匹配该
   literal 当前 protocol 声明的任一 typed branch，并通过 §1.6 精确 task-scope metadata（包括
   dependency `parent_task_id`）；全部 branch 不匹配或 scope 异常时立即 `F` + reconnect，不写
   seen/cursor/cache。unknown frame 不得猜测 scope，保留 raw fallback 并继续下一步。
5. **Canonical fingerprint/dedupe/cursor：**仅在前述 gate 成功后，按 §5.1 结构归一化并比较
   fingerprint，再接受 cursor；此步骤之前不得写 seen map、cursor、event/query cache。
6. **Active-board apply：**去重通过后，known frame 执行 `E` timeline 与表中 `B/D/S`；unknown
   frame 先执行无损 `E` 合并，再执行 conservative `F`。

这条顺序保证跨 board unknown 绝不污染 active-board projection，旧 epoch 也不能干扰新连接；
discard/anomaly 的 frame 会在 recovery reconnect 中从最后 confirmed cursor 重新请求。

### 1.6 Task scope metadata gate

当前 `StreamEventData` schema 允许 `task_id: null`，但下列**精确列出的 task-scoped known
literal** 的 targeted invalidation 必须依赖非空 `task_id`（以及 dependency event payload 的
`parent_task_id`）：

```text
task.archived
task.blocked
task.claimed
task.comment.created
task.completed
task.created
task.execution_plan.not_required
task.execution_plan.planned
task.execution_plan.unplanned
task.heartbeat
task.label.added
task.label.removed
task.label_proposal.accepted
task.label_proposal.proposed
task.label_proposal.rejected
task.promoted
task.reclaimed
task.recomputed
task.released
task.reopened
task.retry_policy.updated
task.specified
task.step.created
task.step.done
task.step.removed
task.step.reopened
task.step.skipped
task.step.updated
task.submitted_for_review
task.unblocked
task.updated
task.export_sanitized
dependency.added
dependency.removed
```

对上述 literal，`task_id == null` 是 protocol anomaly：不做局部猜测，直接 `F` + reconnect；不得
用开放的 `startsWith("task.")` 或其他字符串模式替代这份精确 scope。`signal.recorded`/
`signal.reviewed` 的 `task_id` 仍可选，`board.*`/`label.*` 依其表中 board/optional 语义处理。
Stage 02 必须由 protocol owner 增加显式 scope metadata/schema，或由生成的 cross-validator +
fixtures 证明该约束；Web 不手抄第二份 scope inventory。

`task_id`、dependency payload 的 `parent_task_id` 与 step payload 的 `linked_task_id` 都是 opaque
ID，本身不携带可供 Web 推断的 board；same-board invariant 必须由 service/protocol producer
保证。Web 只能使用 active-board-scoped query 或已验证的 in-memory mapping 做 targeted invalidation，
不得按 ID 前缀/字符串猜 board；若 lookup/contract 证据矛盾，再按 protocol anomaly → `F` 处理。
Stage 02 producer fixtures 必须覆盖 dependency parent 与 step linked 的跨 board 拒绝。

## 2. Query key 词汇与 patch 边界

Web 使用以下稳定的 query key 族。筛选、排序、分页等参数位于带参数的子 key；对某个根 key
做 invalidation 必须覆盖其所有子 key。

| key | 作用域 | 说明 |
|---|---|---|
| `boards` | global | board switcher；由 window focus + visible ≤15s global freshness 维护，不从 active-board SSE 推断 inactive lifecycle |
| `columns(board)` | board | server columns，不复制 fallback columns |
| `events(board)` | board | 当前 `GET /api/v1/events?after=0&limit=150` 按 `id ASC` 返回最早 150；目标持久 SSE 必须分页 catch-up 到 high-watermark，再由 client cache 按 `id` 仅保留最后 150 作为“最近”窗口，不能把单页当作已追平 |
| `tasks(board, filters)` | board | board/list 的任务集合；实现上可用 `tasks(board)` root invalidation 覆盖所有 filters |
| `stats(board)` | board | status counters、stale claims、plan/step counters |
| `search-status(board)` | board | 搜索 projection 状态 |
| `board-task-map(board, options)` | board | task graph；root invalidation 覆盖 options |
| `task-detail(task_id)` | task | inspector 主实体 |
| `task-dependencies(task_id)` | task | parent/child dependencies |
| `task-neighborhood(task_id)` | task | one-hop graph |
| `task-steps(task_id)` | task | execution plan 与 steps |
| `task-runs(task_id)` | task | run rows |
| `task-run-log(run_id)` | run | run log bytes/metadata |
| `task-events(task_id)` | task | inspector timeline |
| `task-comments(task_id)` | task | comments page |
| `task-attachments(task_id)` | task | attachments |
| `task-label-suggestions(task_id)` | task | 手动请求的 label suggestion candidates；默认不自动读取 |
| `signals(board, filters)` | board | rendered signals workbench |
| `signal(signal_id)` | signal | selected signal detail |
| `label-ontology(board, filters)` | board | ontology signals/review/groups |
| `label-ontology-signal(signal_id)` | signal | selected ontology signal |
| `label-ontology-atom(board, atom_ref)` | board | atom explain |
| `maintenance-status` | global | database-wide maintenance/projection status；不绑定 active board，也不为 board switch 建 alias |
| `health`、`runtime` | host | `/health` 和 `/app/runtime.json`；它们不是 event projection |

`maintenance-status` 的 freshness 不能由 active-board SSE 推断：该 SSE 只能覆盖当前 board，无法
观察其他 board 触发的全局 maintenance/projection 变化。仅当 `maintenance-status` query 已被观察
且 window 可见时，才独立以不超过 5 秒的 global polling 读取
`GET /api/v1/maintenance/status`，即使 SSE healthy 也不能停掉这条 polling；window focus 或从
hidden→visible 时立即 refetch，hidden 时可暂停轮询但恢复可见必须先 refetch。每一次本地
`/api/v1/maintenance/*` mutation 发起时先 invalidate 该 global root，request settle（成功或失败）
后再 invalidate/refetch；同时按 mutation 影响 invalidate 对应 board 的 `stats`/`search-status`。
这条 global observer/polling 路径不创建 `maintenance-status(board)` 别名。

事件 envelope 的 payload 大多只包含变更摘要（例如 `task.step.*` 没有完整 step title，
`task.updated` 是空对象）。因此 Web **唯一默认的局部 patch 是 event timeline cache**：在
`events(board)` 和有 `task_id` 时的 `task-events(task_id)` 中插入 envelope、按 `id` 排序、去重并
裁剪窗口。不要从摘要 payload 拼出一个假的 `Task`、`Comment`、`Run` 或 `Label`。其他 key 必须
按下表 invalidate/refetch；只有 mutation 的完整 response 才可以作为 canonical cache patch。

## 3. 当前 event kind 到 query invalidation

### 3.1 表中约定

- `E`：追加到 `events(board)`；有 `task_id` 时同时追加到 `task-events(task_id)`。事件列表自身只
  在去重后更新，不因为每个 event 再做一次无条件全量 GET。
- `D`：对列出的 task/run/signal key 做 targeted invalidate/refetch。payload 不足以安全 patch
  时必须重新 GET；没有对应缓存时不创建伪数据。
- `B`：task projection 的 board-wide invalidate/refetch：`tasks(board)`、`stats(board)`、
  `search-status(board)`、`board-task-map(board)`。凡 `B` 包含 `board-task-map`，还必须 invalidate
  当前 active board 已观察的 `task-neighborhood(*)`（或由已证明完整的 reverse index 精准定位），
  因为 map projection 含相邻完整 Task 与 step/dependency edges。`events(board)` 不在 `B` 中；
  正常事件由 `E` 局部合并，只有 catch-up/gap/protocol anomaly 才经 `F` 重新读取。
- `F`：full board consistency refetch：`columns(board)`、`tasks(board)`、`stats(board)`、
  `search-status(board)`、`board-task-map(board)`、`events(board)`、`signals(board, *)`、
  `label-ontology(board, *)`、`maintenance-status`；将 active-board 缓存中的
  `task-detail(*)`、`task-attachments(*)` 与 `task-label-suggestions(*)` 标 stale 并 refetch 当前可见者；selected task 的
  detail subtree 必须覆盖当前已观察的 `task-dependencies`、`task-neighborhood`、`task-steps`、
  `task-comments`、`task-runs`、`task-run-log`、`task-events`、`task-attachments`、
  `task-label-suggestions` child queries，
  同时 refetch 当前已观察的 `signal(signal_id)`、`label-ontology-signal(signal_id)`、
  `label-ontology-atom(board, atom_ref)` detail。`boards` 仅在表明确列出时加入；不得把其他 board
  的 detail 带进本次 F。`events(board)` 的 F/断线恢复必须从最后 confirmed cursor 分页 catch-up
  到 high-watermark，再按 `id` 合并并将 client cache 裁剪为最后 150；不得用单次
  `after=0&limit=150` 的最早一页替换 timeline，也不能把该单页当作 fresh。
- `S`：signals/ontology projection；分别按表中列出的 root/detail key invalidate。

跨 projection 的 service 规则不依赖具体 kind 行：每个通过当前 epoch、envelope、board isolation、
known payload/task-scope cross-validation 和 dedupe 的 active-board business event 都会推进该
board 的 MAX event cursor，因此至少 invalidate 当前观察中的 `search-status(board)`。当 envelope
`task_id` 非空时，service 还会 enqueue FTS/vector job 并改变 global `maintenance-status`；因此在
该 query 已观察且 window 可见时立即 invalidate 当前 global root，但仍必须保留上面的独立 ≤5s
global polling，不能以 active-board SSE 健康作为全局 freshness 证据。当前 `task_id == null` 的 event 不 enqueue
该 job，但仍推进 `board_last_event_id`，可能令 search status 显示 stale 却没有 job 收敛；这是现有
service 限制，Stage 02 必须实测并记录，不能靠循环 refetch 掩盖。transport-control heartbeat 不
属于 business event，不推进上述 cursor。

### 3.2 精确 kind 与动作

| 精确 kind（没有隐含新增 kind） | patch / cache | 必须 invalidate/refetch | 路径/原因 |
|---|---|---|---|
| `board.created` | `E` | active-board SSE 不可见 inactive board 的 lifecycle；`boards` 由 global freshness path（window focus refetch + visible 时最多 15s polling）维护；若新 board 已成为 active，再读取 `columns(board)` | 不把 active-board SSE 误当成全局 board switcher feed；不刷新未观察的 task workbench |
| `board.archived` | `E` | 仅当 active-board SSE 收到该 board 的 archive 时，立即 `boards`、`columns(board)` 与 active board 的 `B` roots；inactive board 由 global freshness path 发现 | 只刷新 board lifecycle 直接影响的 roots；不因 board event 刷未观察的 signals/ontology/columns |
| `dependency.added`、`dependency.removed` | `E` | child `task_id` 的 `D`: `task-detail(task_id)`、`task-dependencies(task_id)`、`task-neighborhood(task_id)`；payload `parent_task_id` 对应 parent 的同三组 `D`（与 child 相同时去重）；`B`: `tasks(board)`、`board-task-map(board)`、`search-status(board)` | parent/child 两端的 dependency、neighborhood 和 detail 都可能变化；dependency event 本身不直接刷新 `stats`，但 service 若因 readiness 改变额外发 `task.recomputed`，由该已知 event 的 `B` 负责 stats |
| `label.created` | `E` | `D`: active selected task 的 `task-detail(task_id)`（若 envelope 带 task）；`S`: `label-ontology(board, *)`；observed `search-status(board)`（按 board MAX event cursor 跨 projection 规则） | 无 task binding 的 board label 元数据，不推断不存在的 label list query；当前 service 无已核实 producer，但不能因此从 cursor 规则豁免 |
| `label.deleted` | `E` | `D`: active-board `task-detail(*)` 与 `task-label-suggestions(*)` roots（按 query metadata/board predicate，仅标 stale 同 board 的缓存 detail/suggestions）；`S`: `label-ontology(board, *)`、`label-ontology-atom(board, *)`; `B`: `tasks(board)`、`search-status(board)` | payload 只有 `removed_task_bindings` 计数，没有 task IDs；removed semantics/atoms 会让打开的 detail、suggestions 与 atom explain stale；不刷新 map（map node 不渲染 labels），不跨 board 触碰 detail |
| `signal.recorded`、`signal.reviewed` | `E` | `S`: `signals(board, *)`、`signal(signal_id)`（若 payload 有 `signal_id`）；`search-status(board)`；若 envelope 有 `task_id`，再 `D`: `task-detail(task_id)`、`task-comments(task_id)` | service 仅更新 generic `signals` 与 signal backlink comment metadata；不直接写 `label_ontology_*` 或 atoms，因此不额外刷新 ontology/atom detail；event payload 仍不是完整 signal，不能 patch detail |
| `task.comment.created` | `E` | `D`: `task-comments(task_id)`；`search-status(board)` | 不刷新 `tasks`、`stats`、`board-task-map`、`task-steps`；comment body 只从 comments API 读取 |
| `task.created` | `E` | `B`: `tasks(board)`、`stats(board)`、`search-status(board)`、`board-task-map(board)`；`D`: `task-detail(task_id)` 与 `task-label-suggestions(task_id)`（selected/observed 时）、带 run 的 `task-runs`/`task-run-log` | 事件只有 status 摘要；B 本身不含 global `maintenance-status`，但 `task_id` 非空时按跨 projection 规则另行 invalidate observed global `maintenance-status`；B 不触碰未观察的 signals/ontology/columns；suggestions 默认不自动读取 |
| `task.updated` | `E` | `B`: `tasks(board)`、`stats(board)`、`search-status(board)`、`board-task-map(board)`；`D`: `task-detail(task_id)`、`task-label-suggestions(task_id)`（若已观察） | 空 payload 不能判定只改了 title/priority/due；update 也可修改 `max_retries`，而 `stats.stale_claims` 读取 `retry_count`/`max_retries`，因此不能跳过 `stats`；title/description 也是 suggestions 输入，已观察 suggestions 不能保留旧结果 |
| `task.archived` | `E` | `B`: `tasks`、`stats`、`search-status`、`board-task-map`；`D`: `task-detail(task_id)`；有 `run_id` 再失效 `task-runs(task_id)`、`task-run-log(run_id)`；同时将 active-board 已缓存/观察的 `task-detail(*)`、`task-dependencies(*)`、`task-neighborhood(*)` 标 stale（可由完整 reverse index 精准替代）并 refetch 可见者 | force archive running task 会取消关联 `task_runs`；archive 跨越 done/archive 依赖边界时，direct children 的 `dependency_blocked`/count 也会变化，不能只刷新事件中心 |
| `task.completed`、`task.reopened` | `E` | `B`: `tasks`、`stats`、`search-status`、`board-task-map`；`D`: `task-detail(task_id)`；有 `run_id` 再失效 `task-runs(task_id)`、`task-run-log(run_id)`；同时将 active-board 已缓存/观察的 `task-detail(*)`、`task-dependencies(*)`、`task-neighborhood(*)` 标 stale（可由完整 reverse index 精准替代）并 refetch 可见者 | completed/reopened 跨越 done/archive 依赖边界时，direct children 无 child event 也会改变 `dependency_blocked`/count；不能只刷新事件中心 |
| `task.specified`、`task.promoted`、`task.claimed`、`task.blocked`、`task.unblocked`、`task.recomputed`、`task.released`、`task.submitted_for_review`、`task.reclaimed` | `E` | `B`: `tasks`、`stats`、`search-status`、`board-task-map`; `D`: `task-detail(task_id)`；有 `run_id` 再失效 `task-runs(task_id)`、`task-run-log(run_id)` | 状态/派生状态变化需要完整 task projection；map 事件的 observed `task-neighborhood(*)` 由 `B` 约定一并失效 |
| `task.execution_plan.not_required`、`task.execution_plan.planned`、`task.execution_plan.unplanned` | `E` | `B`: `tasks`、`stats`、`search-status`、`board-task-map`; `D`: `task-detail(task_id)`、`task-steps(task_id)`、`task-neighborhood(task_id)` | plan state 同时控制 board badge、map filter、start gate 和 stats；必须 refetch |
| `task.heartbeat` | `E` | `D`: `task-detail(task_id)`；`B`: `tasks`、`stats`、`search-status`; 有 `run_id` 再失效 `task-runs`/`task-run-log` | heartbeat 会改变 board card 与 stale claim；不刷新 `board-task-map` |
| `task.label.added`、`task.label.removed` | `E` | `D`: `task-detail(task_id)`、`task-label-suggestions(task_id)`（若已观察）；`B`: `tasks`、`search-status` | suggestions 会读取 task 当前 applied label ids 并计算 `already_applied`；label mutation 后不能保留旧 candidates；不刷新 `stats`/map |
| `task.label_proposal.proposed`、`task.label_proposal.accepted`、`task.label_proposal.rejected` | `E` | `S`: `label-ontology(board, *)`；`D`: `task-detail(task_id)`；`search-status(board)` | 三个 literal 都在 §1.6 要求非空 `task_id`；null 会先被 scope gate 判 anomaly→`F`，不会到达此行；proposal status 不是已应用 task label，不得把它当 `task.label.*` |
| `task.retry_policy.updated` | `E` | `D`: `task-detail(task_id)`；`stats(board)`（stale claim 的 retry metadata）；`search-status(board)`；有 `run_id` 再失效 `task-runs`/`task-run-log` | 当前没有已核实 producer，保留 generated union 的保守契约；不凭 kind 把它扩成 status transition，若 Stage02 证明 board row 展示 retry，再补 `tasks` |
| `task.step.created`、`task.step.done`、`task.step.removed`、`task.step.reopened`、`task.step.skipped`、`task.step.updated` | `E` | parent `task_id` 的 `D`: `task-detail(task_id)`、`task-steps(task_id)`、`task-neighborhood(task_id)`；若 payload `linked_task_id` 非空，再定向失效同 board 的 `task-neighborhood(linked_task_id)`（与 parent 相同时去重）；`B`: `tasks`、`stats`、`board-task-map`、`search-status` | 每个当前 step payload 都含 `linked_task_id`；graph 把 parent→linked task 作为 edge，因此 linked task neighborhood 也会 stale；不因 step 摘要刷新 linked 主实体 |
| `task.export_sanitized` | `E` | `B`: `tasks`、`stats`、`search-status`、`board-task-map`; `D`: `task-detail(task_id)`；有 `run_id` 再失效 `task-runs`/`task-run-log` | 可能同时改变 status/run/claim projection；仍只刷新 task projection + targeted detail |

`task.*`、`task.step.*` 等写法在表中仅是排版分组；真正的实现必须以每行列出的 literal 集合
匹配，不能接收表外的相似字符串并当作已知行为。

### 3.3 Full board refetch 的硬边界

以下情况才不允许只做局部失效：

1. `UnknownStreamEvent`、known payload validator 失败、board isolation violation、catch-up
   gap/顺序异常；此时按 `F` refetch，清除受影响 board 的 stale error 后再恢复 SSE。
2. 单次 catch-up 达到 `limit` 仍有下一页、服务端报告无法连续补齐、或 `event_id` 与 `id` 映射
   冲突；不能假设只少了一条 event。
3. 运行时不能把一个 known literal 映射到本页的确定一行时，按 `F` 处理，不降级到猜测性的
   局部 patch。

普通 known event（包括 board/task lifecycle、status、plan、step、label、comment、dependency）
执行表中 `E + B + targeted D/S`，并遵守上面的 observed `search-status`/global `maintenance-status`
跨 projection 规则；不会因为“可能影响较多”而刷新未观察的 signals、ontology、columns 或其他
workbench。

### 3.4 F barrier、snapshot generation 与 replay

`F` 不是若干无序 query 的集合，而是带 barrier 的 recovery generation：

F/R 期间所有 frame、HTTP response、mutation result 都绑定同一格式的
`syncToken = (activeBoardId, connectionEpoch, fallbackGeneration | recoveryGeneration)`；F 使用
`fallbackGeneration`，R 使用 `recoveryGeneration`，同一时刻只允许一个 recovery generation。board
切换、epoch 变更或 generation 变更立即使旧 token 失效并 silent discard；snapshot publish 与每次
buffer replay 前都必须做 final token check。

1. 进入 `F` 时递增 `fallbackGeneration`，冻结
   `fallbackBaseCursor = C0 = lastConfirmedCursor`，并暂停直接写 projection cache/seen map/cursor。
   当前 `connectionEpoch` 内，经过 envelope、board isolation、known union 与 task-scope
   cross-validation 的合法同 board known business frame 进入 generation buffer（按 `id`/canonical
   fingerprint 去重），不直接覆盖 cache。通过 envelope/epoch/isolation 的合法 unknown kind 也要
   以 lossless raw envelope + canonical fingerprint 进入 unknown buffer：首次观察的 canonical
   `(id,event_id,fingerprint)` 才允许 E timeline 恰好一次并递增 conservative F；exact duplicate
   只复用已有 seed/revision，不再次触发 F；同 id/event_id 的 fingerprint conflict 仍是 anomaly。
   unknown 不推断 B/D/S，也不当作 malformed anomaly；payload/scope mismatch 仍按 anomaly recovery
   规则处理。当前 epoch 的合法 `kb-heartbeat` 只重置 liveness，不进入 business buffer。
2. F 所发出的每个 HTTP snapshot/refetch（包括 paginated `events` catch-up、active-board detail
   roots 与 global `maintenance-status`）都携带当前 `syncToken`（即 `activeBoardId`、
   `connectionEpoch` 与 `fallbackGeneration`）以及该 query root 发出请求时的
   `perQueryRevision`。每个 root 维护 `(syncToken, perQueryRevision)` response guard：事件的
   invalidation 每次递增该 root revision；response 只有 token 仍当前且 response revision 等于 root
   最新 revision 才可 publish，否则 silent discard 并按最新 revision 重读。首次观察某个 canonical
   unknown 才递增 scoped `fullRefetchRevision`，并递增/标记所有 observed F roots 的 target
   revision；它之前发出的任何 root response 都因 revision 旧而丢弃。exact duplicate 只复用已有
   seed/revision，不再次 F；多个不同 unknown 可合并到最新 revision/boundary，但 barrier 只能在该
   最新 full-root refetch settle 后完成。晚到的旧 GET 不得覆盖新 event/projection，也不得改写
   cursor/seen map。
3. snapshot/catch-up 与 projection effect replay 必须分开处理：
   - 先对 `events` catch-up pages 用同一 envelope、isolation、union/scope validator，建立本地
     `snapshotCanonicalById`/`snapshotCanonicalByEventId` 和 `snapshotSeenSeed`；`knownAcceptedSet`
     与 `unknownAcceptedSet` 都只包含 `C0 < id <= H` 的无 gap active-board events，`H` 是 event
     canonical boundary，不是 tasks/stats/map/detail HTTP snapshot 自带的 cursor。unknown seed 必须
     保留 lossless raw envelope，后续只做 E 恰好一次并延续 conservative F；每个首次观察的 unknown
     都递增 `fullRefetchRevision`，在该 unknown 首次被观察后重新发起全 observed-scope F snapshot；exact
     duplicate 只复用已有 revision/seed，不能只做 E 或凭猜测执行 B/D/S。
   - required projection roots 的 HTTP snapshot 在当前 token/revision guard 下 publish；这些响应
     没有 event cursor，不能据此宣称 `C0..H` 的 projection effects 已应用。snapshot publish 成功后
     才提交 `snapshotSeenSeed`/`snapshotConfirmedBoundary = H`，使 E timeline 能按 canonical
     fingerprint 去重。
   - 对 `knownAcceptedSet` 中每一条 known business event，严格按表中声明的 B/D/S effects 各执行
     一次（即使该 event fingerprint 已在 `snapshotSeenSeed` 中；seen 只控制 E/dedupe，不抑制
     projection effect）。每个 effect 对目标 query root 递增 `perQueryRevision`；barrier 只有在所有
     observed roots 达到 replay-to-H 的最新 target revision 且 response guard 通过后才算完成。E
     timeline 可与 seed canonical dedupe，但不能代替 B/D/S effect replay。`unknownAcceptedSet` 不
     产生 B/D/S effect；它在 E timeline lossless merge 后复用该 unknown 首次观察时已递增的
     `fullRefetchRevision`，使已有 root response 失效，并把当前 F 标记为 conservative continuation，
     等待最新全根 snapshot settle；exact duplicate 不重复 revision/F。
   - generation buffer 之后按 `id`/server order 处理：`id <= H` 必须与 snapshot canonical
     fingerprint 比较，完全一致则丢弃且不再执行 E/B/D/S；缺失或 fingerprint 冲突都是 anomaly，
     重新开启 `F`，不得把 boundary 当作已验证。仅 `id > H` 的 frame 才按 §1.5 与标准
     fingerprint/dedupe/E/B/D/S pipeline replay；unknown frame 仍按 raw-envelope E+F conservative
     path replay，不执行猜测性的 B/D/S；若 canonical registry 尚未登记它，首次观察才递增
     `fullRefetchRevision`、重新发起全 observed-scope F snapshot；exact duplicate 不重复 revision/F，
     且在最新 root settle 前不能提交该 unknown 的 cursor。每个 effect 都必须在 apply/seen
     成功后推进对应 root revision，再提交 cursor。H 的 replay-to-projection 完成后才可 commit `lastConfirmedCursor = H`，
     随后顺序 apply H 后 buffer；重放完成前不能恢复 live apply。
4. mutation echo 也绑定这两个 token，并遵守同一 overlap barrier：event_id-only response 继续留在
   scoped pending registry；带完整 canonical envelope 的 echo 在 F 期间不得直写 cache，先按其
   `id`/fingerprint 与 `H` 比较，`id <= H` 且完全一致时只清理 pending（不重复 E，也不重复已经
   replay-to-H 的 B/D/S），`id <= H` 缺失/冲突或 `id > H` 则等待 barrier 后按 §5.3 pipeline reconcile。
   没有 event id 的 HTTP entity response 只能暂存为 mutation result，待 barrier 后与 canonical SSE
   echo reconcile，不能把 entity 当作 timeline event；过期 response 静默丢弃，不能以旧 mutation
   覆盖新 snapshot。
5. 若 token 过期、snapshot/response guard 失败、boundary/replay 出现 gap/conflict，必须丢弃该
   generation 的全部 staged projection、`snapshotSeenSeed`/canonical maps、buffer 与 query-revision
   staging；只保留 telemetry，不得把任何 staged projection/buffer 转移到下一代。若失败发生在
   `commit lastConfirmedCursor = H` 之前，`lastConfirmedCursor` 仍保持 C0 未推进，下一次 `F` 从该
   C0 重新向 server catch-up；若 H 已成功 commit，后续 H 后 buffer 的失败只保留 H 这个 confirmed
   cursor，下一代从 H 重新 catch-up。两种情况都不能携带 ambiguous carry-over。

no-gap 的客户端提交顺序固定为 `validate → canonicalize → publish guarded snapshot → replay
projection effects/seen → commit lastConfirmedCursor`；任何 cursor 先于 projection/seen 的实现都是
非法，失败时必须保留最近 confirmed boundary（仍在 H commit 前为 C0、H commit 后为 H）并重新进入
新的 `F` generation。

这条 barrier 同时适用于断线立即读取、unknown fallback、gap recovery 与 board switch 后的 catch-up；
`events(board)` 仍必须分页追到 high-watermark，再把 client cache 裁剪为最后 150。

### 3.5 Protocol anomaly retry budget 与 circuit breaker

malformed envelope、payload/scope conflict、当前 epoch 的 cross-board frame、duplicate-id conflict、
无法解析的 `kb-heartbeat`/其他 control DTO，或无法证明的 cursor gap 不能从同一 cursor 无限重连。为每个
`(activeBoardId, lastConfirmedCursor)` 维护跨 automatic recovery epoch 持久的 aggregate bounded
retry budget/circuit；`anomalySignature` 只用于 telemetry 与分类，不是预算 identity。signature 由
异常类别、SSE event name、可解析 id/event_id/board/kind 及 validator error code 组成，不对 raw text
直接 hash；服务端每次生成不同坏 id/event_id 时仍会命中同一 board/cursor aggregate。

通过 envelope/epoch/isolation 的合法 unknown kind 不是 malformed anomaly：它进入当前 F/R 的
lossless raw-envelope + E conservative path，延续（或升级）F，但不消耗该 aggregate circuit budget；
只有 payload/scope/control/duplicate/gap 等 protocol anomaly 才计入预算。

- 同一 `(activeBoardId,lastConfirmedCursor)` aggregate 最多尝试 3 次 recovery，采用有界 backoff
  （例如 250ms、1s、5s）；每次仍不推进 `lastConfirmedCursor`，不把坏 frame 写入 seen/cache。
  普通 transport transient 不带 anomaly signature，也不消耗 anomaly aggregate，仍按连接状态机在
  ≤5s 内 reconnect。旧 epoch 的 frame/control 只 silent discard，不消耗当前 aggregate。
- aggregate 超过预算后进入 terminal/degraded protocol error/circuit-open 状态：停止对该坏 cursor
  的紧密 SSE 重试，保留 5s HTTP polling/refetch 读取 observed projections，并向用户/telemetry 暴露
  可见错误。polling 不能越过未验证 cursor，也不能借循环 refetch 掩盖异常；改变坏 id/event_id
  不能绕过 circuit。
- **清除规则：**manual retry 可显式重置该 board/cursor aggregate；服务端返回经过完整 protocol
  validation、无 gap 且将 `lastConfirmedCursor` 推进到新的 compatible boundary 时，先提交新 boundary
  再清除旧 aggregate；board change 清除旧 board 的 aggregate。仅 automatic reconnect/新
  `connectionEpoch` 不清除预算；清除前 cursor 仍是最后 confirmed 值。

`circuit-open` 优先级高于本页 §6 的普通断线、liveness、≤5s reconnect 与 switchback 规则：打开后
不创建新的 `EventSource`，不因 stalled timer/`onerror` 自动发起 SSE reconnect，也不因 named
heartbeat 恢复 switchback。只有 manual retry、已验证并提交的 compatible boundary，或 board change
才能离开 `circuit-open`；其余时间只保留下节规定的 observed-scope HTTP polling/refetch。§6 的
≤5s reconnect SLO 只适用于 circuit 尚未打开的 ordinary transport transient。

### 3.6 Disconnect R barrier（F 的 scoped 子集）

`onerror`/EOF 的立即一致性读取必须进入独立的 `R` recovery generation；它是 `F` barrier 的
observed-scope 子集，不得退化成“先 GET、再随意接受 live frame”：

1. 若当前 aggregate 已 `circuit-open`，跳过 R，按上面的优先级只运行 observed HTTP polling。否则
   先使旧 `connectionEpoch` 失效，递增 `recoveryGeneration`，并 fence/cancel 所有旧 epoch 的在途
   query/HTTP promise；捕获 `(activeBoardId,lastConfirmedCursor)`，暂停 observed projection/cache、
   seen map 与 cursor 的直接写入。旧响应即使稍后完成，也只能在 token gate 被丢弃。
2. R 新建 SSE/HTTP recovery 时，每个 frame、query response 和 mutation result 都绑定
   `syncToken = (activeBoardId, connectionEpoch, recoveryGeneration)`；board switch、epoch change
   或 generation change 立即使 token 失效并 silent discard。重连早到且通过 envelope/isolation/
   union/scope gate 的同 board known frame 或合法 unknown envelope 进入 R buffer，不能越过 snapshot
   直接 apply；unknown 保留 raw envelope，只走 E+F conservative continuation，不消耗 circuit budget。
   合法 `kb-heartbeat` 只恢复 liveness，不进入 business buffer，malformed control 进入 §3.5 aggregate。
3. R snapshot 只读取断线时 observed roots 与 paginated `events` catch-up/high-watermark，但沿用
   §3.4 的 staging、canonical seen/fingerprint seed、`snapshotConfirmedBoundary` overlap 规则。
   snapshot publish 前与 publish 后、以及每次 replay 前都要再次检查完整 `syncToken`；token 过期或
   旧 HTTP 返回时丢弃整个结果，不能覆盖新 event projection。`id <= boundary` 的 buffer frame
   只有 fingerprint 完全一致才丢弃；`id > boundary` 才按标准 pipeline replay，且每帧
   `apply projection/seen → commit cursor` 前都要 final token check。R 中首次观察 unknown 时递增
   `fullRefetchRevision` 并重新发起全 observed-scope F snapshot；unknown 之前的 root response 因
   revision 旧而丢弃，exact duplicate 只复用已有 revision/seed，多个不同 unknown 可合并，barrier
   等最新全根 snapshot settle。
4. R 的 snapshot 与 buffer replay 完成、cursor 已确认、无 gap/conflict/anomaly 后，才恢复 live
   apply 并停止 polling timer；期间再断线则使当前 R token 失效并从新的 `lastConfirmedCursor` 开启
   新 R。任一 boundary 不可证明、fingerprint conflict 或 protocol anomaly 都升级为 F/§3.5
   aggregate，不能跳过坏 cursor；board switch 则直接丢弃旧 R 并按新 board 建立独立 state。

## 4. Board isolation 与事件顺序

- 每个 SSE connection 绑定一个 active board。客户端用 runtime/`boards` query 得到 canonical
  `activeBoardId`，不能用 slug 与 `board_id` 字符串直接比较。
- 收到 `board_id !== activeBoardId` 的事件是 isolation violation：直接 discard raw frame（包括
  unknown），不进入 union/fingerprint/cursor，不写入任何 active-board query，也不把它用于其他
  board；记录异常，立即 `F` refetch active board 并重开带正确 `board` 的 connection。不要为了
  “赶 cursor”把跨 board event 合并到当前 cache。
- board switch 必须先取消旧 connection、清空旧 board 的 active observer/selection 关联，再以
  新 board 的 cursor 建立 connection。旧 board 的 event 到达竞态只允许被丢弃。
- `events(board)` 和 `task-events(task_id)` 的显示排序按 `id`/server order，不能按客户端收到
  时间排序。`created_at` 仅用于显示。

Web sync controller 同时接收 URL 使用的 runtime board selector（通常是 slug）和由 `boards` query
解析出的 canonical `boards.id`。selector 只进入 `/api/v1/stream/events` 与 HTTP query；isolation、
recovery token、cursor budget、boundary 和 telemetry 的 board identity 一律使用 canonical ID。
在 selector 完成到 canonical ID 的 query boundary 之前不得启动 controller，也不能把 selector 当作
事件 `board_id` 的替代值。

`boards` 是唯一的低频 global freshness 例外：active-board SSE 按 isolation 只看当前 board，
因此无法可靠观察 inactive board 的 `board.created`/`board.archived`。board switcher 在 window
focus 时 refetch，并在页面可见期间以不超过 15 秒的 polling 维护新鲜度；该路径与 active-board
SSE 分离。Stage 02 必须用 CLI 在另一 board create/archive 后回到 Web 的 switcher，证明最终会
更新；active board 若收到 `board.archived`，则按表中规则立即刷新。

## 5. Cursor、catch-up、dedupe 与 mutation echo

### 5.1 去重规则

每个 board connection 维护：

1. `lastCursor`（即 `lastConfirmedCursor`）：最近确认、且已完成 projection/seen 提交的 numeric
   SSE `id`；
2. `seenIds`：最近窗口内的 `id -> canonical envelope fingerprint` 映射；
3. `seenEventIds`：最近窗口内的 `event_id -> canonical envelope fingerprint` 映射。

每个 active-board epoch 的 `seenIds` 与 `seenEventIds` 都至少保留最近 2048 个完整 fingerprint
条目，容量必须覆盖 1000 条 catch-up 与 polling/SSE overlap；board switch 时清空并开启新的
epoch。窗口外的旧 id 即使再次到达，也因 `id < lastCursor` 继续按 anomaly → `F` 处理，不能当作
exact duplicate 重复 apply。

fingerprint 禁止对 raw text 或普通 `JSON.stringify` 直接 hash。必须先对已解析 JSON 做 canonical
structural normalization：object key 递归稳定排序，array 顺序保留，`null`/boolean/string/number
按 JSON 语义归一化；随后按 protocol 提供的 shared/generated helper 的固定字段顺序纳入完整
business envelope（至少 `id`、`event_id`、`board_id`、`task_id`、`run_id`、`kind`、`actor`、
`payload`、`created_at`，SSE `event` name 已先验证为同一 `kind`）。可存 canonical string 或
canonical value，但 SSE 与 polling 必须得到相同结果；Web 不手抄 wire 字段清单，Stage 02 应由
shared/generated helper 承载 canonicalization。payload 仅键序不同仍须判为 exact duplicate，实际
字段值不同才判 conflict。

收到 frame 时先按 §1.5 的 epoch/generation → control-route → envelope validate → board isolation →
known payload/task-scope cross-validation 顺序；只有全部通过后才做 canonical fingerprint dedupe 和
以下 cursor 规则。跨 board frame（包括 unknown）与旧 epoch frame 在此之前已经 discard，不得进入
本节；scope/payload anomaly 也不得先写 seen/cursor/cache。

- `id` 或 `event_id` 已见且 fingerprint 完全一致：这是 exact duplicate，只更新 cursor（若更大），
  不再 patch/invalidate；cursor 仍须遵守先前 projection/seen 已成功的提交顺序，F barrier 内由
  `snapshotConfirmedBoundary` 规则统一处理。
- 任一已见 key 映射到不同 fingerprint（即使另一个 key 看起来相同）：protocol anomaly，走
  `F` + reconnect；不能只按 `event_id` 或只按 `id` 去重。
- `id == lastCursor` 但不是已知 exact duplicate：也是 anomaly，走 `F` + reconnect；`id < lastCursor`
  且不是 exact duplicate 同样如此，不能直接覆盖 cursor。
- Web reload/board switch 时从当前 `events(board)` 最大 id 或最近成功 catch-up cursor 初始化
  `after`；服务端仍以 `Last-Event-ID` 为重连依据，二者必须指向同一个已应用边界。

`id` 是全局存储 cursor 时，数值跳跃本身不等于 board gap（其他 board 的事件会占号）；只有
服务端 catch-up 不能从请求 cursor 连续补到 live、明确返回 gap、或发生重复/逆序异常时才判 gap。
Stage 02 必须用跨 board fixture 验证这一点；若服务端改为 active-board contiguous sequence，
可增加连续性断言，但不能在此之前凭 `id + 1` 猜测。

### 5.2 Catch-up 到 live 的无缝切换

持久 SSE 必须在一个可证明的 subscribe boundary 下实现“catch-up 后 live”，不能先完成一次普通
历史查询、之后才订阅 live（这会在两步之间丢事件）：

1. 首次连接和客户端主动重建连接都发送 `?board=<active>&after=<lastCursor>`。浏览器原生
   `EventSource` 的自动重连会把固定 URL 中旧的 `after` 与新的 `Last-Event-ID` 一起送回；这是
   正常恢复，不应因二者不同而拒绝。server 必须接受两个合法 cursor 并选择较新的值（通常取
   `Last-Event-ID`）；较低但格式合法的 header 由 `max` 选择较新的 cursor，不单独拒绝；只有格式非法
   或与已确认状态矛盾时才走 conservative recovery。
   客户端主动关闭后重新 `new EventSource` 不能依赖自定义 header，只依赖已经更新的
   `after=<lastCursor>`。
2. server 必须先取得 high-watermark 并原子建立 live subscription/buffer（或使用等价的原子
   watermark+subscription 操作），再补发 `(after, watermark]`。补发期间产生的事件进入该连接
   的 buffer；补发结束后按序 drain 所有 `id > watermark` 的 buffered event，随后才进入 live。
   具体 publisher/锁实现留 Stage 02，但 no-gap invariant 不可被普通“query 完再 subscribe”取代。
3. 客户端在 catch-up、buffer drain 和 live 阶段都使用同一个 union validation、board isolation、
   dedupe 和本页 invalidation 状态机。批次达到 `limit` 时继续 catch-up，不得把“收到一批”当作
   已追平。
4. 若无法证明 watermark/buffer 边界没有 gap，停止把 cache 当作 fresh，执行 `F`，然后以 refetch
   后的 cursor 重新连接。

### 5.3 Mutation echo

- mutation response 是 canonical entity；若返回完整 `Task`/`Comment` 等，可以 patch 对应 query
  或按 mutation scope invalidate，但**不要**把 response 合成一条伪 SSE event。
- 当前 mutation response 不统一返回 `event_id`；因此 event timeline 只能由 `/events`/SSE
  envelope 写入。若 response 只有 `event_id`（没有完整 canonical envelope），只登记到独立的
  `pendingMutationEventIds`（可带 bounded TTL、mutation scope 和 task/board 关联），**不得**预注册
  `seenEventIds` 占位。对应 SSE echo 到达时仍必须完整执行 envelope validate、board isolation、
  union/fallback、fingerprint 与正常的 timeline patch/invalidation，然后清除 pending entry；不能
  按 event id 盲目吞掉 echo。
- 若 mutation response 自身携带完整 canonical envelope，必须把它送入与 SSE/polling 相同的标准
  ingestion pipeline：envelope validate → board isolation → known union/unknown fallback → canonical
  fingerprint/dedupe → `E` timeline patch 与声明的 `B/D/S` invalidation/reconcile；pipeline 完成后
  才登记 `seenIds`/`seenEventIds` 的 fingerprint。不得只预登记、绕过 union 或伪造 SSE；后续 SSE
  echo 仅在 canonical fingerprint 完全一致时作为 exact duplicate 跳过。HTTP response 不是 SSE
  frame 时，不生成伪造的 SSE header；envelope validate 使用其 canonical JSON 字段，§1.5 的
  header `id === data.id` safe-integer gate 仅约束真实 SSE transport。
- 同一操作的 HTTP 成功与 SSE echo 不得产生两份 entity。优先按 `event_id`/`id` 去重；没有稳定
  event id 时不要按 title、created_at 等易碰撞字段猜相等，而是保持 mutation response cache，
  让 SSE 只刷新 timeline 与声明的 query key。
- mutation 失败不预先应用 event；本地 optimistic patch 若存在必须在失败时回滚，并在对应 SSE
  事件到达时按本页分类重新 reconcile。

`pendingMutationEventIds` 在 board switch、reconnect recovery 和 TTL 到期时清理；清理只移除 pending
提示，不改变 `seenIds`/`seenEventIds` 的 canonical fingerprint 历史。

## 6. 断线、polling fallback 与 switchback

SSE 是 primary sync path，query cache 只是可重建 projection。连接状态机必须满足：

先判断 §3.5 的 `circuit-open`：open 时跳过下列普通 reconnect/liveness/switchback 分支，只运行
observed-scope HTTP polling；未 open 的 `onerror`/EOF 才进入 §3.6 `R` barrier。

1. **断线立即一致性读取：** `R` 先 fence/cancel 旧 epoch 的在途 query，再 refetch 当前已观察的
   task projection roots（`tasks`、`stats`、`search-status`、`board-task-map`）与 `events`；其中
   `events` 必须按最后 confirmed cursor 分页 catch-up 到 high-watermark 后再裁剪最后 150，不能只读
   `after=0` 的首批；若 selected task 或某个 panel 已打开，同时 refetch 它的 detail subtree、
   signals/ontology/maintenance 等相关 roots。重连早到 frame 进入 R buffer，所有 snapshot publish
   与 replay 遵守 token/fingerprint boundary；R 完成前不得恢复 live apply。未观察的 workbench 不因
   一次断线被强制读取；只有后续 catch-up/gap/protocol anomaly 才升级为 `F`，而不是把断线本身
   等同于全 active-board consistency refetch。
2. **5 秒 polling fallback：** SSE 未恢复期间每 5 秒调用 `GET /api/v1/events?board=&after=
   <lastCursor>&limit=...`，复用完全相同的 union/dedupe/invalidation 逻辑；polling 不另建一套
   event kind 表。分页达到 `limit` 继续 catch-up，必要时再次 `F`。`circuit-open` 时该 polling
   是唯一自动同步路径；它仍可在完整 validation/no-gap 后建立 compatible boundary 并按 §3.5 清除
   aggregate，但清除前不得创建 `EventSource`。
3. **重连：** circuit 未打开且断线后在 5 秒内发起带 `Last-Event-ID`/`after` 的 SSE reconnect；先
   catch-up，确认 cursor 追平后再停止 polling。重连期间的 polling 与 SSE 可能重叠，seen id 规则
   必须使其幂等。
4. **switchback：** circuit 未打开且没有 active `F`/`R` barrier 时，SSE 连续完成一次 catch-up、
   收到有效 live business frame 或 named heartbeat control frame，且无 protocol anomaly 后，才可
   停止 polling timer；保留最后一次 polling promise 的结果，禁止旧请求覆盖新 board cache。`F`/`R`
   active 时 barrier completion gate 优先：heartbeat 只能重置 liveness，live frame 只能进入
   generation buffer，二者都不能提前停止 polling 或恢复 live apply；必须等 snapshot publish、
   buffer replay、cursor confirmed、无 anomaly 且 final token check 全部通过。SSE comment 不算
   heartbeat 证据。若新 SSE 再次断线，回到第 1 步；circuit-open 不执行 switchback。
5. **心跳与 liveness：** server 必须以不超过 15 秒的间隔发送精确 `event: kb-heartbeat` control
   frame。任一通过 envelope/isolation/union（或 unknown fallback）处理的合法 active-board 业务
   frame，或该 control frame，都会重置 client liveness timer；跨 board/discarded frame 不重置。
   连续 35 秒没有可观测 frame（两个 15 秒周期加 5 秒 jitter）即判定 stalled：circuit 未打开时必须
   进入 §3.6 `R` barrier，由 R fence/cancel 旧 query、refetch observed-scope roots、进入 5 秒 polling
   并按 token/buffer/publish 规则发起 SSE reconnect；不能执行裸 refetch+reconnect。circuit-open 时
   不得由 stalled timer 自动重连，只维持 polling。`task.heartbeat` 仍是业务 event，不把 control
   frame 写进 `events` query；Stage 02 必须用 fake clock 覆盖 timer reset、35 秒边界和 stalled 三步动作。

health/runtime query 不属于 event projection；SSE 断线不能用旧 health 值掩盖，health 只在其
自身 query stale、用户 refresh 或 Stage 02 host health flow 要求时读取。

### 6.1 可测 SLO 与证据归属

以下指标必须在真实 host/browser 测试中采样，不能只以代码审查宣称通过：

| 指标 | 采样与通过条件 | Pass/Fail gate |
|---|---|---|
| live event commit → active query/UI invalidation | 通过 `SyncTelemetry.record` collector seam 记录 server event commit、browser 收到并完成 query invalidation 的时间戳；只有闭合 host→browser 样本后才能计算 p95 ≤ 1s、p99 ≤ 2s | Stage 09 browser/Tauri gate 采样并判定；本阶段不宣称 percentile 结果 |
| named heartbeat control frame / liveness | 精确 `event: kb-heartbeat` 间隔 ≤ 15s；任一合法 active-board business/control frame 重置 timer；连续 35s 无可观测 frame 判 stalled：仅当 circuit 未打开时经 §3.6 `R` barrier 做 observed-scope refetch + 5s polling + reconnect，circuit-open 只保留 HTTP polling 且不得自动 SSE；comment 不计入样本 | Stage 02 protocol/host contract + fake-clock timer test；Stage 09 runtime smoke |
| reconnect attempt | circuit 未打开的 ordinary transport transient，断线到发起 SSE reconnect ≤ 5s；记录固定 URL/`after` 与自动 `Last-Event-ID` 恢复路径；circuit-open 不计入此 SLO | Stage 02 reconnect integration + Stage 09 Playwright |
| 1000-event catch-up | 注入 1000 条 active-board events，验证无丢失、无重排、无重复应用，且 fingerprint/dedupe 结果可审计 | Stage 02 catch-up fixture；Stage 09 browser evidence 复测 |

`SyncTelemetry.record` 是 Web 侧采集 seam，host SSE response、browser frame 接收和 query
invalidation 完成时间戳必须在同一集成样本中关联。Chromium/Firefox/Tauri 端到端 gate 负责保存样本
并明确 Pass/Fail；未闭合 host→browser 样本或只通过单元测试的指标不得标记为 SLO 已达成，也不得
伪造 p95/p99。

## 7. Stage 02 必须实测或确认的假设

本页刻意不把尚未实现的 host 行为写成既成事实。Stage 02/后续测试必须至少证明：

- `kanban serve` 的 persistent `/api/v1/stream/events` 以
  `max(after, Last-Event-ID)` 初始化 exclusive cursor，先补发该 cursor 之后的分页事件，再持续
  轮询新事件并发送 `event: kb-heartbeat`；malformed/unsafe 或与已确认状态矛盾的
  `Last-Event-ID` 在响应提交前拒绝，较低但合法的值由 `max` 选择较新的 cursor。
  仍需在 host/browser 集成边界验证 high-watermark/subscribe 的 no-gap 证明、断线期间的完整补发
  和真实端到端采样，不能把单元测试或静态审阅当成这些证据。
- `id` 是否全局递增、board-filter 后是否允许跳号；若不是全局 cursor，更新本页 gap 判定和
  fixture；不能仅凭单 board 测试决定。
- live publisher 是否保证 active-board order、catch-up/live 原子切换和 1000 条无丢失/无重排；
  需要跨 board、重连、重复 frame、limit 边界和服务端重启 fixture；另加超过 150 条 event 的
  fixture，证明当前 `events?after=0&limit=150` 只返回最早一页，目标 client 必须继续分页追到
  high-watermark 后才把 cache 裁剪为最后 150，不能把单页误判为 fresh。
- 精确 `event: kb-heartbeat` named transport-control event 的 DTO/data 形状、≤15 秒间隔和
  graceful shutdown 行为；合法 active-board business/control frame 的 timer reset、连续 35 秒
  stalled：circuit 未打开时经 §3.6 R barrier 做 observed-scope refetch + 5 秒 polling + reconnect，
  circuit-open 只保持 HTTP polling 且不自动 SSE；以及 fake-clock 边界测试。SSE comment 只能额外
  keepalive，不能作为浏览器 timeout/switchback 证据，control frame 也不应伪装成当前 40 个业务 kind。
- `board_id` 与 runtime `activeBoard` slug 的映射、board switch 竞态以及跨 board event 的
  isolation 行为。
- connection epoch/generation race fixture：旧 board/旧 `EventSource` 的 business frame 与
  `kb-heartbeat` 必须 silent discard，不能重置新连接 liveness、推进 cursor 或触发 F；只有当前
  epoch 的跨 board frame 才记录 isolation anomaly。
- disconnect/stalled R-barrier fixture：`onerror`/EOF 与 35 秒 stalled 都必须 fence/cancel 旧在途
  query、递增 recovery generation、把早到 frame 放入 token-bound buffer，并在 snapshot publish/
  replay 前后验证 `(activeBoardId, connectionEpoch, recoveryGeneration)`；验证 `id <= boundary` 的
  canonical fingerprint overlap、`id > boundary` replay、cursor commit 顺序，以及重连期间旧 HTTP
  不能覆盖新 projection。F/R 未完成时 heartbeat 只能 reset liveness，不能停止 polling/switchback。
- F watermark/projection fixture：冻结 `fallbackBaseCursor=C0`，catch-up 至 `H` 后使用无 cursor 的
  tasks/stats/map/detail snapshots；验证 `snapshotSeenSeed` 与 projection effect replay 分离，所有
  `C0 < id <= H` known event 的 B/D/S 恰好一次，per-root `(syncToken,perQueryRevision)` guard
  丢弃晚到 response，barrier 等 replay-to-H 最新 revision settle。注入 C0→H 与已有 F/R 中的合法
  unknown，验证 lossless raw envelope + E 恰好一次、首次 canonical `(id,event_id,fingerprint)` 才
  递增 `fullRefetchRevision` 并重新发起全 observed-scope F snapshot（旧 roots response 丢弃）；同一
  unknown 的 catch-up/SSE exact duplicate 只复用已有 seed/revision，不自循环触发 F，fingerprint
  conflict 仍 anomaly。unknown 不消耗 circuit；generation 失败不得转移 staged projection/buffer，
  新 generation 从最近 confirmed boundary 重新 catch-up，且 barrier 能在 duplicate-only overlap 后
  settle。
- aggregate circuit fixture：malformed `kb-heartbeat`/control DTO、不断变化 id/event_id 的坏帧和
  payload/duplicate/gap anomaly 在同一 `(activeBoardId,lastConfirmedCursor)` 上合计最多 3 次；
  automatic epoch 不清除预算，circuit-open 后不自动创建 SSE，HTTP observed polling 仍继续；仅
  manual retry、validated compatible boundary 或 board change 清除。
- global maintenance observer fixture：active board SSE 无法观察另一 board 的 maintenance 变化；
  仅在 `maintenance-status` query observed 且 window visible 时保持独立 ≤5 秒 polling，SSE healthy
  也不停，focus/visibility 立即 refetch；每个本地 maintenance mutation 发起与 settle 都必须
  invalidate/refetch global root。
- `board_last_event_id`/search status 与 FTS/vector outbox 的实际关系：所有 accepted business
  event 是否推进 board MAX cursor；`task_id != null` 是否 enqueue job 并改变 maintenance status；
  `task_id == null` 是否出现 search stale/no-job（不以循环 refetch 掩盖），需用 signal/label/board
  null-task producer fixture 记录当前限制。
- mutation response 是否会补充 `event_id`，以及各 mutation 的 event 与 response 是否一一对应；
  在没有统一 event id 之前必须遵守“不合成 SSE”的规则；若 response 只有 event id，验证
  `pendingMutationEventIds` 的 bounded TTL/scope、SSE echo 完整落库后清理 pending、以及 board
  switch/reconnect 清理，不得预注册 `seenEventIds` 或按 id 盲目吞 echo。
- `task.retry_policy.updated`、plan/step/label/ontology event 是否确实影响 stats/search
  projection；若真实 server 依赖关系不同，只能通过新的 protocol/service evidence 调整本页，
  不能在 Web 端猜测。
- unknown kind、known payload mismatch、duplicate id、same `event_id`/different payload、
  catch-up truncation、SSE header `id` 与 `data.id` 不一致/超 safe/非法以及 server 500 的浏览器
  可见错误与 telemetry 证据；这些 case 必须落进 Playwright/contract fixture，确保 safe-integer
  gate、禁止 cursor 分叉和 conservative `F` 真正发生。
- known payload fixture 必须覆盖每个当前 protocol typed branch（包括 `board.created` 的
  `BoardCreatedPayload`/`EmptyPayload`、`task.blocked` 的 reason/retry/result、`task.reclaimed`
  的 reclaimed/retry），只有全部 branch 失败才记录 mismatch；service producer 审计结果若要
  删除不可达 fallback，先同步 protocol/schema/fixtures 与 taxonomy。
- `task.step.*` fixture 必须同时覆盖 `linked_task_id: null` 与非空且同 board 的 linked task，验证
  parent/linked `task-neighborhood` 定向失效及同 parent 去重。`linked_task_id` payload 本身不含
  board 字段，Stage 02 由 service/protocol producer fixture 证明跨 board step link 被拒绝；Web
  只使用 active-board-scoped neighborhood endpoint 做 targeted invalidation，若 lookup/contract
  证据矛盾再按 protocol anomaly → `F`，不得扩大到另一 board 的 detail/query。
- `task.completed`、`task.archived`、`task.reopened` 必须各有 direct-child dependency fixture：
  parent 在 done/archive 边界切换、child 无新 event 但 `dependency_blocked`/unfinished count 改变；
  验证 active-board cached/observed `task-detail(*)`、`task-dependencies(*)`、
  `task-neighborhood(*)` stale/refetch（或证明 reverse index 完整）。

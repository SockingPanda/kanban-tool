# 本地 HTTP API 规范

本 API 是 `kanban serve` 提供的本机应用服务入口。CLI、MCP 和 Desktop
只能通过 typed localhost client 调用它；它们不打开数据库，也不各自实现业务状态转换。

默认监听地址为 `http://127.0.0.1:8721`，只接受 loopback 绑定。所有产品路由的基础路径为
`/api/v1`；健康检查是 `/health`。

本文件先描述当前已接入的 single-host 路由；它不是最终功能边界。labels、signals、graph、
vector、context、projection、maintenance 与 SSE 仍按 parity ledger 恢复到同一 localhost
API，并随每个纵向切片补齐本规范。task search 已通过 Turso FTS projection 接入；旧的直接
数据库路径不会恢复。

## 1. 通用契约

### 1.1 HTTP

- JSON 请求使用 `Content-Type: application/json`。
- JSON 响应使用 `Content-Type: application/json`。
- GET 查询只使用各 endpoint 列出的 URL query 参数。task list 与 event list 会严格拒绝
  未知参数；没有 query contract 的 endpoint 不把未知参数解释为新能力。
- 服务端只绑定 loopback；client 也拒绝非 loopback URL。
- 正常响应使用 `{ "data": ... }`；事件列表额外包含 `meta`。

<!-- schema-doc-ignore: 通用 data envelope 的说明性最小示例，不代表具体 endpoint contract -->
```json
{ "data": {} }
```

分页/游标响应使用：

<!-- schema-doc-ignore: 分页 meta 的说明性示例，具体 endpoint 仍以各自 fixture 为准 -->
```json
{ "data": [], "meta": { "limit": 100, "offset": 0, "total": 0 } }
```

事件列表的 `meta` 形状为 `{ "next_after": 123 }`。

### 1.2 actor

mutation 请求中的 actor 由服务端按以下顺序解析：

1. JSON body 的 `actor`（如果该 request DTO 有此字段）；
2. `X-KB-Actor` 请求头；
3. `kanban serve` 启动时配置的默认 actor。

actor 会被写入 canonical mutation/event 审计记录。只读请求不需要 body；client 会发送
`X-KB-Actor`，服务端不会据此改变查询结果。comment create 是命名上的例外：
`CreateCommentRequest.author` 占据 body actor 的优先级，并作为 comment author/event
actor；body 未提供 author 时才回退到 header 和 host 默认值。

### 1.3 错误封装与状态码

由 handler/ApplicationService 返回的产品错误使用：

<!-- schema-doc-ignore: 错误 envelope 的说明性示例，code/message 用于解释状态码映射 -->
```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot promote task from status done"
  }
}
```

`code` 是稳定机器契约，`message` 只供人阅读，调用方不得解析 message。Axum 在 route
匹配之前产生的 malformed path、method-not-allowed 等框架级 4xx 不属于该产品 error
envelope；typed client 不会主动构造这类请求。

| `error.code` | HTTP | 用途 |
|---|---:|---|
| `invalid_input` | 400 | handler 已接收的 JSON、path/query value 或字段值无效 |
| `not_found` | 404 | board、task、step、dependency 或 run 不存在 |
| `conflict` | 409 | 一般业务冲突或唯一性冲突 |
| `idempotency_conflict` | 409 | 同一实体 key 重放但 canonical payload 不同 |
| `dependency_cycle` | 409 | dependency 会形成环 |
| `execution_plan_required` | 409 | promote 前没有计划或 not-required 标记 |
| `steps_incomplete` | 409 | review/done 的必需 step 尚未完成 |
| `dependency_blocked` | 409 | 依赖阻止进入目标状态 |
| `claim_conflict` | 409 | 并发 claim 或 claim 条件冲突 |
| `invalid_transition` | 409 | 状态机拒绝转换 |
| `claim_token_mismatch` | 403 | claim owner/token 不匹配 |
| `feature_not_available` | 501 | 当前 single-host 尚未提供该 surface |
| `internal` | 500 | storage 或 host 内部错误 |

### 1.4 ID 与 task selector

HTTP path 中的 task 必须使用 canonical 全局 `t_...` ID；run 使用全局 `r_...` ID，step
使用全局 `step_...` ID。typed `kanban-client` 对 `t_...` 直接透传；对 board 上下文中的
`board#seq`、`#seq`、数字 seq，先通过 task list 解析为全局 ID，再发起后续请求。服务端
不在 mutation handler 内实现第二套 selector 语义。

## 2. 健康与看板（只读）

### `GET /health`

返回 `HealthResponse`：`data.ok`、`data.db`（当前为 `"turso"`）、`data.version`、
`data.db_path` 和 `data.db_fingerprint`。该路由只检查已打开 host 的健康状态，不创建备用数据库。

### `GET /api/v1/boards`

Query：`include_archived`（默认 `false`）。返回 `ListBoardsResponse`，即 `data` 为
`ApiBoard[]`。

### `GET /api/v1/boards/{board}/columns`

返回 `ListBoardColumnsResponse`，即固定看板列的 `ApiBoardColumn[]`。列的 status 使用
`triage`、`todo`、`scheduled`、`ready`、`running`、`blocked`、`review`、`done`、`archived`。

当前 host 没有 board create/get/archive route；调用这些旧路径应视为未提供功能，而不是
另起一个数据库路径。

### `GET /api/v1/stats`

Query：`board`（默认 `default`）。返回 `StatsResponse`（`data: QueueStats`）：
`board_id`、`generated_at`、按 status 计数、过期 running claim、blocked reason 计数、
未规划 active task 数，以及仍有未完成 required step 的 active parent 数。该 query 通过
ApplicationService 读取 canonical Turso snapshot，不执行 claim/reclaim 或其他 mutation。

## 3. Task 读取与创建

### `GET /api/v1/boards/{board}/tasks`（只读）

返回 `ListTasksResponse`：`data: ApiTask[]` 与
`meta: { limit, offset, total }`。

支持的 query 参数：

- `status`：可重复，任务状态枚举；
- `priority`：可重复，`0..=3`；
- `plan_filter`：`plan_needed`、`has_steps`、`incomplete_required_steps`；
- `assignee`、`q`、`include_archived`；
- `limit`（默认 100，最大 1000）、`offset`（默认 0）；
- `sort`：`seq`、`-seq`、`title`、`-title`、`status`、`-status`、
  `position`、`-position`、`priority`、`-priority`、`assignee`、
  `-assignee`、`scheduled_at`、`-scheduled_at`、`due_at`、`-due_at`、
  `created_at`、`-created_at`、`updated_at`、`-updated_at`（默认 `position`）。

当前 task list 尚未接通 label filter；传入 `label` 会暂时返回
`feature_not_available`。labels 切片完成后必须恢复该 query contract。

### `POST /api/v1/boards/{board}/tasks`（mutation）

请求为 `CreateTaskRequest`：`title` 必填；可选 `task_id`、`idempotency_key`、
`description`、`status`（`triage|todo|scheduled|ready`）、`assignee`、`priority`、
`scheduled_at`、`due_at`、`max_retries`、`metadata`、`actor`。`labels` 和 `depends_on`
字段当前必须为空；queue/labels 切片必须在共享 application transaction 中恢复这两个
surface。

成功返回 HTTP `201` 与 `CreateTaskResponse { data: ApiTask }`。同一 board 内相同
`idempotency_key` 与相同 canonical payload 返回已有 task；payload 不同返回
`idempotency_conflict`。请求中的 `status` 是期望初始状态；ApplicationService 仍会应用
execution-plan、依赖与排期 guard，例如尚未满足 ready 条件时返回的 task 会处于 `todo`。

### `GET /api/v1/tasks/{task_id}`（只读）

`task_id` 必须是全局 `t_...`。返回 `GetTaskResponse`：`data: ApiTask`，当前不带 ontology
`meta`。`include=ontology` 当前尚未接通，ontology 切片完成后必须恢复。

## 4. Task search 与 FTS projection

### `GET /api/v1/search/tasks`

Query：`board`（默认 `default`）、`q`、可重复的 `status` 与 `label`、`assignee`、
`include_archived`（默认 `false`）、`limit`（默认 20，最大 1000）和 `offset`（默认 0）。
返回 `SearchTasksResponse`：`data.hits` 为带 `task_id`、`seq`、`score`、highlight `snippet`
和 `ApiTask` 的结果，`data.meta` 描述 backend、generation、index version、event lag 与
fallback reason；分页 `meta` 保留 `limit`/`offset`。

exact `t_...`、`board#seq`、`#seq` 或数字 seq 走 canonical selector 查询；普通文本在
Turso FTS ready 时使用 `task_search_fts`。索引尚未 ready、落后或 provider/query 失败时
回退 canonical SQL，并在 `data.meta.stale` 与 `fallback_reason` 中标记，不触碰第二个数据源。

### `GET /api/v1/search/tasks/by-status`

使用同一 query contract，按请求中每个 `status` 返回一个 `SearchTaskStatusWindow`，每个窗口
带独立 `search_meta` 与 `page`。顺序与重复 status 保持请求顺序；任务结果仍受 board、label、
assignee、archive、query 和分页过滤。

### `GET /api/v1/search/status`

Query：`board`（默认 `default`）。返回 `SearchStatusResponse`，报告 Turso FTS capability、
projection generation、ready/degraded/stale、最后 event 与 lag。projection 不可用时
`backend` 为 `canonical`、`derived_index` 为 `false`，但 search query 仍可用 canonical fallback。

### `POST /api/v1/search/index/rebuild` 与 `POST /api/v1/search/index/sync`

Query：`board`（默认 `default`），请求体为空 JSON。`rebuild` 从 canonical task、comment、run
和 event 事实重建 `task_search_fts`；`sync` 在存在 pending projection job 或 event lag 时
执行同一可重放 rebuild。两者返回 `SearchStatusResponse`，不会修改 canonical task 状态。

## 5. Execution plan 与 task state machine

所有以下 endpoint 都调用同一个 ApplicationService/state machine；不存在通用的
`POST .../transitions/{target_status}`。

### `POST /api/v1/tasks/{task_id}/execution-plan/not-required`（mutation）

请求：`{ "reason": string, "actor": string|null }`。返回 `MarkExecutionPlanNotRequiredResponse`
（`data: ApiExecutionPlan`）。这是 walking skeleton 中显式完成计划前置条件的操作。

### `POST /api/v1/tasks/{task_id}/transitions/promote`（mutation）

请求：`{ "actor": string|null }`。只允许状态机认可的 todo/scheduled 到 ready 转换；返回
`PromoteTaskResponse`（`data: ApiTask`）。

### `POST /api/v1/tasks/{task_id}/transitions/claim`（mutation）

请求字段：`actor`、`ttl_ms`（默认 300000）、可选 `worker_profile`、`metadata`。这是原子
`ready -> running` claim，同时创建 active run 和 event。返回 `ClaimTaskResponse`：
`data.task`、`data.run`、`data.claim_token`、`data.claim_expires_at`。竞争调用恰有一个成功，
失败者收到 `claim_conflict`。

### `POST /api/v1/tasks/{task_id}/transitions/heartbeat`（mutation）

请求：`claim_token` 必填，`ttl_ms`（默认 300000），可选 `actor`、`note`。token/owner 不匹配
返回 `claim_token_mismatch`；成功返回 `HeartbeatTaskResponse`（`data: ApiTask`）。

### `POST /api/v1/tasks/{task_id}/transitions/release`（mutation）

请求：`claim_token` 必填，可选 `actor`。只有 active running claim owner 能调用；事务内将 task
回到 ready、清除 claim、取消 active run 并写 `task.released`。成功返回 `ReleaseTaskResponse`
（`data: ApiTask`），失败不会留下部分写入。

### `POST /api/v1/tasks/{task_id}/transitions/submit-review`（mutation）

请求：可选 `actor`、`claim_token`、`summary`，以及 `force`（默认 false）。返回
`SubmitReviewTaskResponse`（`data: ApiTask`）。

### `POST /api/v1/tasks/{task_id}/transitions/complete`（mutation）

请求：可选 `actor`、`claim_token`、`summary`、`result`，以及 `force`（默认 false）。返回
`CompleteTaskResponse`（`data: ApiTask`）。

### `POST /api/v1/tasks/{task_id}/transitions/block`（mutation）

请求：`reason` 必填；可选 `actor`、`claim_token`、`force`（默认 false）。返回
`BlockTaskResponse`（`data: ApiTask`）。

review、complete、block 的 claim 校验、required steps、依赖检查与 run 更新均在同一
application transaction 中完成。

## 6. Comments

### `GET /api/v1/tasks/{task_id}/comments`（只读）

返回 `ListCommentsResponse`（`data: ApiComment[]`）。

### `POST /api/v1/tasks/{task_id}/comments`（mutation）

请求为 `CreateCommentRequest`：`body` 必填；可选 `idempotency_key`、`author`、`kind`
（wire enum 为 `note|decision|signal`）、`author_type`（`user|agent`）、`agent_type`、
`metadata`。当前 canonical path 只接受 `note` 与 `decision`；`signal` 稳定返回
`feature_not_available`。signals 切片必须恢复 `signal` 并保持 comment backlink。
成功返回 HTTP `201` 与 `CreateCommentResponse`（`data: ApiComment`）。idempotency key
属于 task；相同 key/相同 payload 重放返回已有 comment，不同 payload 返回
`idempotency_conflict`。

## 7. Attachments

### `GET /api/v1/tasks/{task_id}/attachments`（只读）

返回 `ListAttachmentsResponse`（`data: ApiAttachment[]`）。数据库只保存附件 metadata；
`rel_path` 是 host attachment root 下、按 `{board_id}/{task_id}/` 隔离的相对路径。

### `POST /api/v1/tasks/{task_id}/attachments`（mutation）

请求为 `CreateAttachmentRequest`：`filename` 必填，`content` 是 JSON byte array（可为空）；
可选 `id`、`content_type`、`sha256`、`actor`。host 先在同文件系统 staging 写入并
`fsync`，再原子发布文件，随后在同一 Turso transaction 写 metadata 与
`task.attachment.created` event；checksum 不匹配、绝对/穿越路径和 symlink destination
直接拒绝。重复 `id` 且 metadata/content 相同返回已有记录，不同 payload 返回 `conflict`。
成功返回 HTTP `201` 与 `CreateAttachmentResponse`。

### `GET /api/v1/tasks/{task_id}/attachments/{attachment_id}`（download）

返回原始 bytes；`Content-Type` 来自 metadata（缺失时为 `application/octet-stream`），并
附带 `X-KB-Attachment-ID` 与可选 `X-KB-Attachment-SHA256`。host 重新校验 size/SHA 后才返回；
metadata 指向的文件缺失或校验失败返回 storage error。

### `DELETE /api/v1/tasks/{task_id}/attachments/{attachment_id}`（mutation）

要求 `X-KB-Actor`。active board/task 才能删除；文件先移动到 host root 的 `.trash/`，再在
同一 Turso transaction 删除 metadata 并写 `task.attachment.deleted` event。事务失败会恢复
canonical path；成功返回 `DeleteAttachmentResponse`。列表/下载/删除均按 task id 与 board-scoped
metadata 查询，不能跨 task 读取文件。

## 8. Steps 与 execution plan

### `GET /api/v1/tasks/{task_id}/steps`（只读）

返回 `ListStepsResponse`（`data.task_id`、`data.steps`、`data.execution_plan`）。

### `POST /api/v1/tasks/{task_id}/steps`（mutation）

请求：`title` 必填；可选 `idempotency_key`、`body`、`linked_task_ref`、`position`、
`required`（默认 true）、`actor`。`linked_task_ref` 在 HTTP contract 中必须是全局
`t_...` ID；board-local selector 由 typed adapter 先解析。成功返回 HTTP `201` 与
`CreateStepResponse`。step create 的 key 只在当前 task 内幂等。

### `PATCH /api/v1/tasks/{task_id}/steps/{step_id}`（mutation）

请求可更新 `title`、`body`、`linked_task_ref`/`unlink_task`、`position`、`required`、
`actor`；不改变 step status。返回 `UpdateStepResponse`（同一 `ApiTaskSteps` 形状）。

## 9. Dependencies

### `GET /api/v1/tasks/{task_id}/dependencies`（只读）

返回 `ListDependenciesResponse`（`data.task`、`parents`、`children`、`edges`）。

### `POST /api/v1/tasks/{task_id}/dependencies`（mutation）

请求：`parent_task_id` 必须是同一 board 的全局 task ID，可选 `actor`。返回
`AddDependencyResponse`。复合唯一约束保证重复 add 幂等；跨 board、未知 task 和 dependency
cycle 拒绝。

### `DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}`（mutation）

删除同一 board 的 parent edge，返回 `RemoveDependencyResponse`（当前 dependencies 快照）。
目标 task 存在但 edge 已不存在时是成功 no-op，不追加 remove event。

## 10. Runs 与 log（run 不是独立 mutation surface）

run 只能由 task claim 创建，并由 heartbeat/release/review/complete/block 同事务更新。HTTP
没有 run create/update endpoint；以下全部是只读查询。

### `GET /api/v1/tasks/{task_id}/runs`

返回 `ListRunsResponse`（`data: ApiRun[]`）。

### `GET /api/v1/runs/{run_id}`

返回 `GetRunResponse`（`data: ApiRun`）。

### `GET /api/v1/runs/{run_id}/log`

返回 `GetRunLogResponse`：`data.run_id`、`data.content`、`data.truncated`。读取使用固定
256 KiB 的文件尾部 snapshot；超过上限时返回最后 256 KiB 并设置 `truncated=true`。
typed client 不发送 `tail` query，服务端也不会把未知 query 解释为可配置读取范围；没有
任意文件路径输入或第二种 log 协议。

## 11. Events

### `GET /api/v1/events`

Query：`board`（默认 `default`）、可选全局 `task_id`、`after`（默认 0）、`limit`（默认
100，服务端上限 1000，超过时收敛到 1000）。返回 `ListEventsResponse`：
`data: StreamEventData[]` 与 `meta.next_after`。
known event kind 使用 typed payload；未知 kind 保留原 JSON payload，不被 adapter 丢弃。

event list 是只读；所有 mutation 通过 ApplicationService 写 canonical event。

## 12. 停止路径

服务停止后，client 返回 `server_unavailable`，不得 fallback 到嵌入式数据库、旧 SQLite
路径或另一个 host。迁移期间尚未接通的 labels/signals/maintenance 等命令暂时返回
`feature_not_available`，不会触碰数据库；只要该临时响应仍存在，对应 parity 项就不能
标记完成。

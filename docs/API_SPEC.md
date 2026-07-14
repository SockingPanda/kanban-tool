# Local Web API SPEC

本 API 只面向 localhost Web UI 和本地脚本。它不是远程协作 API。

默认监听：

```text
127.0.0.1:8721
```

Base path：

```text
/api/v1
```

---

## 1. 通用约定

### 1.1 Content Type

Request：

```http
Content-Type: application/json
```

Response：

```http
Content-Type: application/json
```

SSE：

```http
Content-Type: text/event-stream
```

### 1.2 Actor

因为没有多用户系统，actor 是审计字段。

来源优先级：

1. Request body `actor`。
2. Header `X-KB-Actor`。
3. Server 默认 actor。
4. OS username。

### 1.2.1 Request header contracts

除 SSE stream 外，当前 83 个 HTTP endpoint 都拥有 operation-specific、exact、
`deny_unknown_fields` header contract；每个 contract 都包含可选 `Accept-Language`，并按真实
handler 输入选择 locale、locale + actor、locale + JSON content type，以及它们的
optional-body 变体。`X-KB-Actor` 只出现在会解析 actor 的 mutation handler。

有必需 JSON body 的 endpoint 要求且只允许一个 `Content-Type`；允许空 body 的 archive、
promote、reclaim、unblock、label proposal propose/accept/reject endpoint 将其建模为可选；没有
body 的 endpoint 不声明 `Content-Type`。这些 cardinality 是 transport contract，不改变 Axum
对具体媒体类型和 malformed JSON 的既有 400 行为。

SSE 的 `Last-Event-ID` 仍明确 `Excluded`：当前 runtime 忽略该 header，没有 resume contract；
不得因为其它 endpoint 已关闭 headers 就把它推断为 adopted input。

### 1.3 Success Response

成功响应按 endpoint 的元数据契约使用以下 wire envelope：

`DataEnvelope` 仅包含 `data`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {}
}
```

`MetadataEnvelope` 的 `meta` 是必需字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {},
  "meta": {}
}
```

`OptionalMetadataEnvelope` 只在 endpoint 产生对应元数据时包含 `meta`；没有元数据时
直接省略该字段，不返回 `"meta": null`。具体 endpoint 使用哪一种 envelope 及其
`meta` 字段由该 endpoint 的响应示例和说明定义。

### 1.4 Error Response

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot claim task from status todo"
  }
}
```

`error.code` 是稳定机器契约。`error.message` 是 human-readable 文案，会根据
`Accept-Language` 在 `zh-CN` 和 `en` 之间选择；未传 header 时保持既有默认 `en`。
客户端逻辑必须读取 `error.code`，不要解析 `error.message`。

### 1.5 HTTP Status Mapping

| Error code | HTTP status |
|---|---:|
| `invalid_input` | 400 |
| `not_found` | 404 |
| `conflict` | 409 |
| `dependency_cycle` | 409 |
| `invalid_transition` | 409 |
| `dependency_blocked` | 409 |
| `execution_plan_required` | 409 |
| `steps_incomplete` | 409 |
| `claim_conflict` | 409 |
| `claim_token_mismatch` | 403 |
| `internal` | 500 |

---

## 2. Health

### `GET /health`

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "ok": true,
    "db": "ok",
    "version": "2.1.2",
    "db_path": "/home/alice/.local/share/kb/kb.db",
    "db_fingerprint": "sqlite:131072:1717520000000"
  }
}
```

`db_path` and `db_fingerprint` let local Desktop/Web development surfaces verify
which local SQLite runtime answered the request. If the configured database file
has been deleted, `/health` returns `400 invalid_input` instead of recreating an
empty SQLite file. Other API routes apply the same missing-file guard before
running handlers, so stale/deleted runtimes fail explicitly instead of opening a
new empty database at the configured path. `/health` also validates that the
database has the expected migrated schema and returns `400 invalid_input` for an
empty or uninitialized SQLite file.

---

## 3. Boards

### 3.1 List boards

```http
GET /api/v1/boards?include_archived=false
```

Archived boards are hidden by default. Pass `include_archived=true` to include them.

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "b_01HX...",
      "slug": "default",
      "name": "Default",
      "description": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000,
      "archived_at": null
    }
  ]
}
```

### 3.2 Create board

```http
POST /api/v1/boards
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "slug": "agent-work",
  "name": "Agent Work",
  "description": "Local agent board",
  "actor": "alice"
}
```

Response status is `201 Created`. Board slugs must be unique, non-empty, no longer than 64 bytes, start with a lowercase ASCII letter or digit, contain only lowercase ASCII letters, digits, `.`, `_`, `-`, and must not start with reserved ID prefixes such as `b_`, `t_`, `r_`, `c_`, `a_`, `l_`, `col_`, or `e_`. Duplicate or invalid slugs return the normal `400 invalid_input` error envelope, not `500`.

### 3.3 Get board

```http
GET /api/v1/boards/{board_slug_or_id}
```

### 3.4 Archive board

```http
POST /api/v1/boards/{board}/archive
```

Archive marks `archived_at` and writes a `board.archived` event; it does not mutate tasks. The operation is rejected with `409 invalid_transition` if the board has active `running` tasks or `running` task runs. After archive, ordinary task mutations on that board are rejected, while audit history endpoints remain readable when called with explicit task/board identity.

### 3.5 B2-C6 boards exact contracts

四个 boards endpoint 使用 endpoint-specific contract roots：list query、create request、
get/archive path 与各自 success response。四个 success response 只共享闭合 `ApiBoard` component；
server 显式把 SQLite application record 映射为 wire DTO，不直接序列化 `BoardRecord`。
archive request body 继续复用既有 `ArchiveBoardRequest` contract。

`include_archived` 缺省为 `false`，传入 `true` 时真实转发到 service 并返回 archived boards。
Desktop `listBoards` caller exact 校验 `data` envelope 与 `ApiBoard` 的全部字段，missing、mistyped
或 extra field 返回 `invalid_response`。running-work archive guard、archived audit history、
not-found status/error code 与 locale-dependent message 不属于 schema authority，继续由
service/adapter 保证。四个 endpoint 的 headers obligation 仍为 `Todo`，migration state 保持
`Generated`。

---

## 4. Tasks

### 4.1 List tasks

```http
GET /api/v1/boards/{board}/tasks
```

Query params：

| Param | 说明 |
|---|---|
| `status` | 可重复：`?status=ready&status=running`。 |
| `priority` | 可重复：`?priority=0&priority=2`，值为 P0-P3 的 `0..3`。P0 表示 incident/blocker/must-handle-immediately；P3 是普通 backlog/低优先级/默认。 |
| `assignee` | 按 assignee。 |
| `label` | 按 label 名称或 id 过滤，可重复；多个 label 使用 AND 语义。 |
| `plan_filter` | 可重复：`plan_needed` / `has_steps` / `incomplete_required_steps`。 |
| `q` | title/description 搜索；task ref 形状按精确匹配处理。 |
| `include_archived` | bool。 |
| `limit` | 默认 100。 |
| `offset` | 分页 offset。 |
| `sort` | `seq` / `title` / `status` / `position` / `priority` / `assignee` / `scheduled_at` / `due_at` / `created_at` / `updated_at`，前缀 `-` 表示降序。`priority` sorts P0 -> P3; `-priority` sorts P3 -> P0. |

这两个 task-read endpoint 使用同一套严格 raw-query 语法，但各自拥有独立的 exact path/query
contract，并由两个 server-local typed Axum extractor 分别绑定真实 `{board}` path 与唯一 raw URI
query 消费点；handler 只接收已解析 request，不持有 `RawQuery` 或第二套 `Query<T>` extractor。
只有 `status`、`priority`、`label`、`plan_filter` 可以重复；不同语义值按 URI 首次出现顺序
保留，任何重复语义值返回 `400 invalid_input`。`assignee`、`q`、`include_archived`、
`limit`、`offset`、`sort` 任一重复也返回 `400`。任何未知 key 失败关闭；旧 `search`
alias 已删除，只接受 `q`。

raw query 最多 8192 bytes。参数对上限不是独立字面量，而是由 9 个 `status`、4 个
`priority`、3 个 `plan_filter`、32 个 `label` 和 6 个 scalar 参数推导出的 54。
`q` 最多 1024 个 Unicode 字符，`assignee` 与单个 `label` 最多 128 个。未提供 query 时
默认 `include_archived=false`、`limit=100`、`offset=0`、`sort=position`。`limit` 的 wire
authority 是 `kanban-contract` 的 1000；SQLite service defensive maximum 直接引用唯一
application authority，server 对该实际 service path 建立编译期相等门禁。`offset` 最大为
`i64::MAX`。空的 `q`、`assignee` 归一化为未提供；label 会规范化 Unicode 边缘空白，但必须
包含至少一个非空白字符，且 raw 字符长度不得超过 128；该预算在 trim 前计算，随后会被移除
的 Unicode 边缘空白也计入 128 字符。空或纯 Unicode 空白 `label`、enum、bool、数字或 sort
值无效。
query 使用严格 form 解码：`+` 表示空格，`%HH` 必须完整且解码结果必须是 UTF-8；合法
UTF-8 与 `&`、`/`、`=`、`+`、空格必须由标准 form encoder 转义，非法 percent encoding
或 UTF-8 返回 `400`。

Priority 只表达相对重要性和排序，不表达可 claim 状态。`ready` 才表示任务已显式进入可执行队列；普通 `ready` task 可以是 P1/P2/P3，不应为了表示“可做”全部标成 P0。P0 只用于 incident、当前目标 blocker 或必须立即处理的任务；P0 task 若仍缺规格、排期未到或依赖未完成，仍不能被 claim。

`q` 对 task ref 形状使用精确匹配而不是文本 contains 匹配：纯数字 `12`、
`#12` 匹配 `{board}` 内的 seq；`board#12` / `board/#12` 只在显式 board
与 `{board}` 相同时匹配；`t_...` 只匹配 `{board}` 内的 task id。其他文本仍执行
title/description 模糊搜索。

响应（以下为字段节选；完整、可消费的成功响应以 `schemas/fixtures/api/list-tasks-response.v1.valid.json` 为准）：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "t_01HX...",
      "seq": 12,
      "board_id": "b_01HX...",
      "board_slug": "agent-work",
      "ref": "agent-work#12",
      "title": "实现状态机",
      "description": "...",
      "status": "ready",
      "priority": 1,
      "position": 1024,
      "assignee": null,
      "scheduled_at": null,
      "due_at": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000,
      "labels": [
        {
          "id": "l_01HX...",
          "board_id": "b_01HX...",
          "name": "core",
          "color": null
        }
      ],
      "dependency_blocked": false,
      "unfinished_parent_count": 0
    }
  ],
  "meta": {
    "limit": 100,
    "offset": 0,
    "total": 1
  }
}
```

### 4.2 Create task

```http
POST /api/v1/boards/{board}/tasks
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "实现状态机",
  "description": "Markdown spec",
  "status": "ready",
  "assignee": "local-worker",
  "priority": 1,
  "scheduled_at": null,
  "due_at": null,
  "max_retries": 2,
  "depends_on": ["t_01HX..."],
  "labels": ["core"],
  "metadata": {},
  "actor": "alice"
}
```

Notes：

- `status` 只能是 `triage|todo|scheduled|ready`。
- 若不传 `status`，服务端计算初始状态。
- 若存在未完成 dependencies（parent 不是 `done` 或 `archived`），不能创建为 `ready`。
- 若 execution plan 仍为 `unplanned`，不能创建为 `ready`；先添加 step，或显式标记 `not_required` 并填写 reason。
- Task 响应会暴露派生 dependency 和 execution-plan 字段：`dependency_blocked`、`unfinished_parent_count`、`execution_plan_state`、required/optional step counts。它们是查询元数据，不是可写 task 字段。
- `priority` 是整数等级 `0..3`：`0` = P0 incident/blocker/must-handle-immediately，`1` = P1 近期重点，`2` = P2 重要后续，`3` = P3 普通 backlog/低优先级/默认。创建时会拒绝非法值。
- `labels` 可选。名称会先 trim；空白名称会被拒绝；所有 label 必须已存在于当前 board。任一 label 缺失时，整个 create 返回 `400 invalid_input`，且不会写入 `tasks`、`labels`、`task_labels` 或 `task_events`。Task create 不提供 create-missing 模式。
- `priority` 缺省为 `3`，`labels` / `depends_on` 缺省为空数组；其它 nullable 字段可显式传 `null`。`metadata` 只接受 JSON object 或 `null`，其 object 内容是 opaque extension，不在 transport layer 解释。
- path、request 与 `201` success response 分别由 `CreateTaskPath`、`CreateTaskRequest` 与 `CreateTaskResponse { data: ApiTask }` 拥有。request status 使用 create-only 的闭合 `triage|todo|scheduled|ready` vocabulary；公开 response 不包含 `claim_token`。
- handler 只做 contract 到 application input 的显式映射，并继续单次调用 `create_task_with_labels_and_dependencies`。label、dependency、retry policy、metadata validity 与 initial readiness 仍在同一 SQLite transaction/service guard 中处理；任一失败都整体回滚。

### 4.3 List task windows by status

```http
GET /api/v1/boards/{board}/tasks/by-status?status=triage&status=ready&include_archived=false&limit=50&offset=0&sort=-updated_at
```

这个只读 endpoint 将 board column 查询批量化为一次请求，并接受 4.1 节定义的同一套严格
query 语法。每个重复 `status` 生成一个独立 task window；`limit` 与 `offset` 分别应用到
每个 window。response 中的 status 顺序与 URI 中的重复参数顺序一致；省略 `status` 时返回
空的 `statuses` array。

响应（以下为字段节选；完整、可消费的成功响应以 `schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json` 为准）：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "statuses": [
      {
        "status": "ready",
        "tasks": [
          {
            "id": "t_01HX...",
            "ref": "default#12",
            "status": "ready",
            "title": "实现状态机"
          }
        ],
        "page": {
          "limit": 50,
          "offset": 0,
          "total": 3
        }
      }
    ]
  },
  "meta": {
    "limit": 50,
    "offset": 0
  }
}
```

### 4.4 Get task

```http
GET /api/v1/tasks/{task_id}
```

`task_id` is the global `t_...` id and is not scoped by board. Responses include `board_id`, `board_slug`, and `ref` so clients can render copyable `board#seq` task refs.

Query params：

| Param | 说明 |
|---|---|
| `include` | 可选。当前识别 `ontology`；可用逗号分隔，其他 include 值暂时保持兼容性忽略。 |

默认响应只包含 `data: ApiTask`，不返回 `meta`。传 `include=ontology` 时，`data`
保持同一 `ApiTask`，并在 `meta.details.ontology_summary` 返回该 task 的 label
ontology signal 摘要；没有 ontology signals 时为 `null`。Summary 是只读 task-level
工作流提示，包含 signal/status/degraded/stale/action counts、oldest open/confirmed
signal time/age、latest signal/action time、当前 `suggest_input_hash` 和最多 5 条
sample signals（id/kind/status/proposed_action/score/stale/degraded/action count）。完整
queue/review 仍使用 `/label-ontology/signals`、`/label-ontology/review` 和
`/label-ontology/signals/{signal_id}`。

Task detail aggregate endpoints such as
`GET /api/v1/tasks/{task_id}/detail?include=dependencies,steps,runs,events,comments,neighborhood`,
panel-specific timelines, or execution-context bundles are not part of the
current API. They remain a follow-up optimization candidate for reducing
TaskDetail panel fan-out after the existing per-panel routes and cache
invalidation behavior are stable.

### 4.5 Update task fields

```http
PATCH /api/v1/tasks/{task_id}
```

允许字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "新的标题",
  "description": "新的描述",
  "assignee": "worker-a",
  "priority": 1,
  "scheduled_at": 1717520000000,
  "due_at": 1717600000000,
  "max_retries": 2,
  "metadata": {},
  "actor": "alice",
  "expected_lock_version": 7
}
```

`priority` updates reject values outside `0..3`.

`max_retries: null` 清空 retry policy。Task DTOs include `execution_plan_state`, `required_step_count`, `completed_required_step_count`, and `optional_step_count` so clients can show plan readiness without separately listing steps.

禁止字段：

- `status`
- `claim_token`
- `current_run_id`
- `completed_at`

`PATCH` 不能直接设置 canonical `status`；状态必须通过 transition endpoint 修改。
不过允许字段仍会走 shared service path。更新 `description`、`scheduled_at`
等影响 spec 或 schedule 的字段后，服务端可以根据 spec、schedule 和
当前 dependencies 重新计算 active task 的目标状态，并写入对应事件。
Dependency edge 必须通过 dependency endpoints 修改；`max_retries` 只更新
retry policy，不是 status recompute 触发器。

---

## 5. Transitions

Transition request 使用各 endpoint 独立的封闭 DTO，未知顶层字段返回 `400`，不共享通用
transition/token body。Promote、reclaim、unblock 和 task archive 可完全省略 body；出现
body 时仍按对应 DTO 校验。`actor` 的解析优先级保持为 body、`X-KB-Actor`、server default。
Claim 与 heartbeat 省略 `ttl_ms` 时使用 `300000`；reclaim、complete、submit-review、block
与 archive 省略 `force` 时均为 `false`，不能绕过 lease、token 或状态机 guard。Claim-token mismatch
response 不回显客户端提交的错误 token，也不暴露服务端保存的真实 token。

### 5.1 Specify

```http
POST /api/v1/tasks/{task_id}/transitions/specify
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "description": "补全后的规格",
  "scheduled_at": null,
  "actor": "alice"
}
```

### 5.2 Promote

```http
POST /api/v1/tasks/{task_id}/transitions/promote
```

Promote is rejected with `409 execution_plan_required` while the task execution plan is `unplanned`.

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "dispatcher"
}
```

### 5.3 Claim / Start

```http
POST /api/v1/tasks/{task_id}/transitions/claim
```

Claim/start is rejected with `409 execution_plan_required` while the task execution plan is `unplanned`.

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice",
  "ttl_ms": 300000,
  "worker_profile": null,
  "metadata": {}
}
```

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "status": "running"
    },
    "run": {
      "id": "r_01HX...",
      "status": "running"
    },
    "claim_token": "ct_01HX...",
    "claim_expires_at": 1717520300000
  }
}
```

### 5.4 Heartbeat

```http
POST /api/v1/tasks/{task_id}/transitions/heartbeat
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "ct_01HX...",
  "ttl_ms": 300000,
  "note": "still running",
  "actor": "worker-default"
}
```

Explicit heartbeat remains supported. For a `running` task, a later valid task-scoped activity event also refreshes the task lease and active run heartbeat as an implicit liveness signal; that implicit renewal does not emit an extra `task.heartbeat` event. Board-level events and events without `task_id` do not renew a task.

### 5.5 Complete

```http
POST /api/v1/tasks/{task_id}/transitions/complete
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "ct_01HX...",
  "summary": "实现完成，测试通过",
  "result": {},
  "force": false,
  "actor": "worker-default"
}
```

`result` 是可选 opaque JSON value；schema 只约束字段存在形式，不收紧其内部结构。

### 5.6 Submit Review

```http
POST /api/v1/tasks/{task_id}/transitions/submit-review
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "ct_01HX...",
  "summary": "等待人工检查",
  "force": false,
  "actor": "worker-default"
}
```

Submit-review 不接受 `result`；该字段与其它未知顶层字段一样返回 `400`。

### 5.7 Block

```http
POST /api/v1/tasks/{task_id}/transitions/block
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "等待 API schema 确认",
  "claim_token": null,
  "force": false,
  "actor": "alice"
}
```

### 5.8 Unblock

```http
POST /api/v1/tasks/{task_id}/transitions/unblock
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice"
}
```

Response target 由服务端计算，不由客户端指定。

### 5.9 Reopen

```http
POST /api/v1/tasks/{task_id}/transitions/reopen
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "重新执行修正验证失败",
  "actor": "alice"
}
```

只允许 reopen `done` task，`reason` 必填且不能为空。Response target 由服务端按 spec、schedule、dependency 和 execution plan readiness 重新计算；`completed_at` 会清空，`result_summary` / natural JSON `result` 保留（持久层仍存于 `result_json`）。事件 `task.reopened` 的 payload 包含 `from`、`to`、`reason` 和 `original_completed_at`。

直接依赖该 task 的 child 中，仅 `triage|todo|scheduled|ready` 会重新计算；`running|blocked|review|done|archived` 不隐式改写。

### 5.10 Reclaim

```http
POST /api/v1/tasks/{task_id}/transitions/reclaim
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "force": false,
  "to_status": "ready",
  "reason": "claim expired",
  "actor": "dispatcher"
}
```

`to_status` 是封闭枚举，只接受 `ready` 或 `blocked`；其它 task status 返回 `400`。

### 5.11 Archive

```http
POST /api/v1/tasks/{task_id}/transitions/archive
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "force": false,
  "actor": "alice"
}
```

---

## 6. Dependencies

### 6.1 Add dependency

```http
POST /api/v1/tasks/{child_task_id}/dependencies
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "parent_task_id": "t_01HX...",
  "actor": "alice"
}
```

Response status is `201 Created` when a new edge is inserted. Re-adding the
same parent/child edge is idempotent and returns `200 OK` with the same
dependency envelope; it does not write another `dependency.added` event or
recompute the child status again. Dependency changes may demote an invalid
`ready` child to `todo`, but they do not auto-promote `todo` children to
`ready`. Reopening a `done` parent recomputes direct children only when the child is `triage|todo|scheduled|ready`; it leaves `running|blocked|review|done|archived` children unchanged.

### 6.2 Remove dependency

```http
DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}
```

### 6.3 List dependencies

```http
GET /api/v1/tasks/{task_id}/dependencies
```

Add/remove/list dependency endpoints return the same dependency envelope. For
the existing wire shape, `parents` and `children` are full `ApiTask` arrays.
The additional `task` and `edges` fields provide a compact hydrated relationship
view with stable named parent/child objects.

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_child",
      "board_id": "b_default",
      "board_slug": "default",
      "ref": "default#2",
      "title": "child",
      "status": "todo"
    },
    "parents": [],
    "children": [],
    "edges": [
      {
        "parent": {
          "id": "t_parent",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#1",
          "title": "parent",
          "status": "done"
        },
        "child": {
          "id": "t_child",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#2",
          "title": "child",
          "status": "todo"
        }
      }
    ]
  }
}
```

### 6.4 Steps and execution plan

Steps are ordered execution-plan items owned by a task. A step can be plain
text, or it can link to an existing normal task for context. A linked task is
not a dependency edge: linking does not affect dependency readiness, and the
linked task status does not automatically complete the step. Step completion is
tracked independently with `todo | done | skipped`.

```http
GET /api/v1/tasks/{task_id}/steps
POST /api/v1/tasks/{task_id}/steps
PATCH /api/v1/tasks/{task_id}/steps/{step_id}
DELETE /api/v1/tasks/{task_id}/steps/{step_id}
POST /api/v1/tasks/{task_id}/steps/{step_id}/done
POST /api/v1/tasks/{task_id}/steps/{step_id}/skip
POST /api/v1/tasks/{task_id}/steps/{step_id}/reopen
POST /api/v1/tasks/{task_id}/execution-plan/not-required
```

Create request:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "Write acceptance checks",
  "body": "Cover dependency and plan guards",
  "linked_task_ref": "default#13",
  "position": 2048,
  "required": true,
  "actor": "alice"
}
```

`linked_task_ref` is optional. Omit it for a plain text step. When present, it
must resolve to a non-archived task on the same board and must not be the parent
task itself.

Update request:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "Write acceptance checks",
  "body": null,
  "linked_task_ref": "default#14",
  "unlink_task": false,
  "position": 4096,
  "required": false,
  "actor": "alice"
}
```

Step status requests:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "note": "Implemented and verified",
  "actor": "alice"
}
```

`skip` and `reopen` use the same envelope but name the text field `reason`.

Mark not required request:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "Small text-only cleanup",
  "actor": "alice"
}
```

Step list and mutation responses return the parent task step snapshot:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_parent",
    "steps": [
      {
        "id": "st_01HX...",
        "parent_task_id": "t_parent",
        "title": "Write acceptance checks",
        "body": "Cover dependency and plan guards",
        "linked_task": { "id": "t_child", "ref": "default#13" },
        "position": 2048,
        "required": true,
        "status": "todo",
        "resolution_note": null,
        "resolved_by": null,
        "resolved_at": null,
        "created_by": "alice",
        "created_at": 1717520000000,
        "updated_by": "alice",
        "updated_at": 1717520000000
      }
    ],
    "execution_plan": {
      "board_id": "b_01HX...",
      "task_id": "t_parent",
      "state": "planned",
      "reason": null,
      "updated_by": "system",
      "updated_at": 0
    }
  }
}
```

`POST /execution-plan/not-required` returns the execution plan record directly.
Missing linked task targets return `404 not_found`; self-links, cross-board
links, archived linked tasks, and empty titles return `400 invalid_input` in the
standard error envelope. Completing or archiving a parent with incomplete
required steps returns `409 steps_incomplete`. Required steps are complete for
that guard only when their step status is `done` or `skipped`.

### 6.5 Task neighborhood

```http
GET /api/v1/tasks/{task_id}/neighborhood?depth=1&limit_nodes=250&include_archived_context=false
```

This read-only endpoint returns the selected task, direct dependency parents,
direct dependency children, direct linked-step parents/children, and every dependency or step edge whose source and target are
both visible. V1 only accepts `depth=1`; deeper graph expansion is intentionally
reserved for later.

Response:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "center_task_id": "t_01HX...",
    "nodes": [
      {
        "task": { "id": "t_01HX...", "ref": "default#12", "status": "ready" },
        "role": "center",
        "context_only": false
      }
    ],
    "edges": [
      {
        "id": "dependency:t_parent->t_child",
        "source_task_id": "t_parent",
        "target_task_id": "t_child",
        "kind": "dependency",
        "required": true,
        "blocking": true
      }
    ],
    "meta": {
      "depth": 1,
      "context_depth": 0,
      "node_count": 1,
      "edge_count": 0,
      "truncated": false,
      "limit_nodes": 250,
      "include_archived_context": false
    }
  }
}
```

`task` uses the same public task DTO as task list/detail responses and does not
expose `claim_token`.

### 6.6 Board task map

```http
GET /api/v1/boards/{board}/task-map?active_only=true&context_depth=1&limit_nodes=250&include_done_context=true&include_archived_context=false&hide_isolated=false
```

This read-only endpoint returns an operational graph for the board. By default it
includes all active, non-archived tasks (`triage`, `todo`, `scheduled`, `ready`,
`running`, `blocked`, `review`) plus at most one dependency-hop of non-archived
context. Done context is included by default and marked `context_only`; archived
context is excluded unless explicitly requested. V1 only accepts
`context_depth=0` or `context_depth=1`.

Node roles are `active` for active board tasks and `context` for one-hop context.
Dependency and step edges are returned only when both endpoints are visible. Dependency edges use `kind=dependency`, `required=true`, and `blocking=true`; step edges use `kind=step`, preserve the step `required` flag, and set `blocking=false`. Pure text steps have no task node and therefore do not appear as graph edges. The `meta` object reports active statuses, node/edge counts, truncation, limit, and the query context flags.


---

## 7. Comments

### 7.1 List comments

```http
GET /api/v1/tasks/{task_id}/comments
```

Comments are task-id scoped. Listing comments remains available for archived boards because it is read-only audit history; creating comments on archived boards is rejected.

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "c_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "author": "alice",
      "author_type": "user",
      "agent_type": null,
      "body": "这里需要确认边界条件。",
      "kind": "note",
      "metadata": {},
      "created_at": 1717520000000
    }
  ]
}
```

### 7.2 Add comment

```http
POST /api/v1/tasks/{task_id}/comments
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "body": "这里需要确认边界条件。",
  "kind": "note",
  "author_type": "user",
  "agent_type": null,
  "author": "alice",
  "metadata": {}
}
```

Notes：

- `kind` 默认为 `note`，当前允许 `note|decision|signal`。
- `decision` records meaningful multi-option choices; body remains the readable fallback, and structured decision metadata is carried by `metadata`.
- `author_type` marks who produced the comment and allows `user|agent`. If omitted, the service defaults to `user`.
- `agent_type` is optional open text for `author_type=agent` comments, such as `executor` or `reviewer`. Non-empty `agent_type` with `author_type=user` is rejected as `400 invalid_input`.
- `metadata` 默认为 `{}`，必须是 JSON object；response 同样使用自然 JSON `metadata` object。普通 note/signal metadata 保持开放且无损，不能因为键名与专用协议碰撞而在 transaction 提交后收紧。`kind=decision` 时必须包含非空 `options`，每个 option 必须有非空 `slug` / `title` / `detail`，slug 必须是唯一小写 ASCII slug，`selected` 必须匹配 option slug，`reason` 必须非空，`risk` / `verification` 如果出现也必须非空。无效 decision metadata 返回 `400 invalid_input`。
- `author` 走通用 actor 语义；也可以用 `X-KB-Actor` 或 server 默认 actor。
- 创建评论会写入 `task.comment.created` event。


### 7.3 B2-C3 comments wire contracts

`GET` 与 `POST /api/v1/tasks/{task_id}/comments` 各自拥有独立、闭合的 path 与 success root；
`POST` 另有独立、闭合的 request root。两者只共享 contract-owned `ApiComment` component 与
既有 shared error component。GET 没有 query/body，POST 没有 query；B7 已为两个 endpoint
登记并采用精确 header contract。endpoint surface migration 仍诚实标记为 `Generated`，直到
后续 API adoption cohort 补齐真实 router producer/contract consumer 的 exact surface evidence。

`ApiComment.author_type` 仅允许 `user|agent`，`kind` 仅允许 `note|decision|signal`，
`agent_type` 是必须出现但可为 `null` 的字段。`metadata` 是开放、无损的 response object。
create request 的 `metadata` 保持开放 JSON object；decision 的精确 typed shape 由独立的
`metadata.decision.input` / `NoTransport` contract 与真实 CLI producer/consumer witnesses 拥有。
运行时原始 JSON object 继续进入 SQLite service 的 decision cross-field guard，schema 不替代
selected/option uniqueness、slug、非空值、board archive 与 transaction/event 约束。

---

## 8. Runs

### 8.1 List task runs

```http
GET /api/v1/tasks/{task_id}/runs
```

Run listing is task-id scoped and remains available for archived boards as read-only audit history.

### 8.2 Get run

```http
GET /api/v1/runs/{run_id}
```

### 8.3 Get run log

```http
GET /api/v1/runs/{run_id}/log
```

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "run_id": "r_01HX...",
    "content": "worker output\n",
    "truncated": false
  }
}
```

Notes：

- Response 不包含 `claim_token`。
- 当前最多返回末尾 256 KiB；更大的 log 会设置 `truncated: true`。
- 若 run 没有 `log_path` 或文件不存在，返回 `not_found`。
- 若 `log_path` 不在受信任日志目录或文件名不匹配 `<run_id>.log`，返回 `invalid_input`。

---

## 9. Stats

### 9.1 Queue stats

```http
GET /api/v1/stats?board=default
```

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "board_id": "b_01HX...",
    "generated_at": 1717520000000,
    "status_counts": [
      {"status": "ready", "count": 3},
      {"status": "running", "count": 1}
    ],
    "stale_claims": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "title": "stale worker",
        "claim_owner": "dispatcher",
        "claim_expires_at": 1717520000000,
        "last_heartbeat_at": 1717519900000,
        "current_run_id": "r_01HX...",
        "retry_count": 1,
        "max_retries": 3
      }
    ],
    "blocked_reasons": [
      {"reason": "waiting on operator", "count": 2}
    ],
    "unplanned_active_tasks": 4,
    "active_parents_with_incomplete_required_steps": 1
  }
}
```

Notes：

- `stale_claims` 只包含 `running` 且 `claim_expires_at <= now` 的任务。
- `blocked_reasons` 按数量降序、reason 升序排序。

---

## 10. Events

### 10.1 List events

```http
GET /api/v1/events?board=default&after=0&limit=100
```

`board` accepts board slug or id. Events for archived boards remain readable so clients can inspect the audit trail after archive.

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": 123,
      "event_id": "e_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "run_id": "r_01HX...",
      "kind": "task.claimed",
      "actor": "alice",
      "payload": {"claim_owner":"alice","metadata":{}},
      "created_at": 1717520000000
    }
  ],
  "meta": {
    "next_after": 123
  }
}
```

### 10.2 SSE stream

```http
GET /api/v1/stream/events?board=default&after=123
```

SSE event：

```text
event: task.claimed
id: 124
data: {"id":124,"event_id":"e_...","board_id":"b_...","task_id":"t_...","run_id":"r_...","kind":"task.claimed","actor":"alice","payload":{"claim_owner":"alice","metadata":{}},"created_at":1717520000000}
```

`board`、`task_id`、`after`、`limit` 是该 endpoint 唯一接受的 query key，均只能出现一次；
未知或重复 key 返回标准 `400 invalid_input` envelope。默认值分别为 `default`、未提供、`0`、
`100`，runtime 将 `limit` 防御性限制到 `1000`。每个事件严格按 `event`、`id`、`data`
frame 顺序输出；`data` 是完整的 `StreamEventData` JSON，不允许额外字段。
`task_id`、`run_id`、`actor` 都是 required-nullable：键必须出现，值可以显式为 `null`。
39 个 known kind 的 payload 与 kind 使用同一 tagged union；缺字段、额外字段或 sibling
status/state 错配会 fail closed。未来 unknown kind 的合法 JSON payload 保持 lossless。

Reconnect：

- V1 implementation emits a finite snapshot of existing matching events and closes; clients should reconnect or poll `GET /api/v1/events` for updates.
- Browser clients may send Last-Event-ID, but V1 only honors the `after` query parameter.
- V1 finite snapshot 不发送 SSE comment/heartbeat frame；因此 heartbeat 不是 JSON payload
  contract，`Last-Event-ID` 也不是已采用的 header input contract。这两项只有未来 runtime
  真正实现后才能迁移为 typed contract。
- 若 event 已被压缩/清理，客户端重新 fetch board snapshot。

---

## 11. Columns / UI Settings

### 11.1 List columns

```http
GET /api/v1/boards/{board}/columns
```

### 11.2 Update columns

```http
PATCH /api/v1/boards/{board}/columns
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "columns": [
    {"id": "col_triage", "title": "Triage", "position": 10, "hidden": false},
    {"id": "col_done", "title": "Done", "position": 80, "hidden": false}
  ]
}
```

MVP 不允许 column 改变 canonical status。

---

## 12. 标签 API

```http
GET /api/v1/boards/{board}/labels
POST /api/v1/boards/{board}/labels
GET /api/v1/boards/{board}/labels/semantics
GET /api/v1/boards/{board}/labels/{label_id}/semantics
PUT /api/v1/boards/{board}/labels/{label_id}/semantics
DELETE /api/v1/boards/{board}/labels/{label_id}/semantics?expected_semantics_hash=<hash>&reason=<text>
GET /api/v1/boards/{board}/labels/atoms
GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain
GET /api/v1/boards/{board}/labels/atom-index/status
POST /api/v1/boards/{board}/labels/atom-index/rebuild
GET /api/v1/boards/{board}/labels/atom-index/query?q=<text>&polarity=positive&limit=24
GET /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels/bootstrap
DELETE /api/v1/tasks/{task_id}/labels/{label_id}
GET /api/v1/boards/{board}/signals
GET /api/v1/boards/{board}/signals/review
GET /api/v1/signals/{signal_id}
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals
GET /api/v1/boards/{board}/label-ontology/review
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/revert
POST /api/v1/boards/{board}/label-ontology/validate
```

Board 级标签创建请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "core",
  "color": "blue"
}
```

Label 响应结构，用于 board 级标签创建和 label 列表：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "id": "l_01HX...",
  "board_id": "b_01HX...",
  "name": "core",
  "color": "blue",
  "created_at": 1717520000000,
  "updated_at": 1717520000000
}
```

`POST /api/v1/boards/{board}/labels` 按 board 作用域创建 label，并按 label
名称保持幂等。如果该 board 上已存在同名 label，响应返回已有 label。空白 name
会被拒绝。Base label identity CRUD 属于 vocabulary registry，不属于 ontology
ledger；创建 label identity 不写 `label_ontology_actions`，也不会创建
`label_semantics` 或 `label_atoms`。

Task 标签添加请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "core"
}
```

或批量添加：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "names": ["core", "api"]
}
```

如果需要在绑定时显式创建缺失 label identity：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "names": ["scratch-label"],
  "create_missing": true
}
```

`POST /api/v1/tasks/{task_id}/labels` 会把指定 name 或 names 的 label 绑定到 task。
`name` 与 `names` 互斥；二者都缺失、二者同时出现或 `names` 为空数组都会返回
invalid input。批量添加在同一 transaction 内执行，并先验证所有 label 名称；如果
任一 label 为空白或非法，不会创建 canonical label，也不会留下部分 task-label 绑定。
默认情况下，如果该 task 所属 board 上还不存在指定 name 的 label，请求会返回
invalid input，且不会增加 `labels` 或 `task_labels` 记录。传入
`"create_missing": true` 时，API 会只创建缺失的 canonical label identity，并绑定到
task；不会生成 `label_semantics` 或 `label_atoms`。重复绑定已有 task-label 关系不会
重复写入。成功响应返回更新后的 task，包含当前 `labels` 列表；显式创建模式下如果
本次创建了 label，响应 `meta.created_labels` 会列出新建 labels。

Task label bootstrap 请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "database",
  "description": "Database persistence work",
  "applies_when": ["touches SQLite migrations"],
  "excludes_when": ["UI-only polish"],
  "positive_examples": ["new table migration"],
  "negative_examples": ["CSS-only tweak"],
  "actor": "alice"
}
```

`POST /api/v1/tasks/{task_id}/labels/bootstrap` 是一次性 new-label adoption API：
在同一 transaction 内创建 task 所属 board 上缺失的 canonical label，或复用没有既有
semantics 的同名 label，写入该 label 的 `label_semantics`，同步重建 SQLite
`label_atoms`，标脏派生的 label atom vector index，并把该 label 绑定到 task。
`name` 按 label 名称解析；空白名称会被拒绝。语义输入会 trim 并丢弃空白值，且必须至少
提供 `description` 或一个非空语义数组值。

Bootstrap API 默认不会覆盖已有 `label_semantics`。如果同名 label 已经有
semantics，请求会失败，并要求调用方改用专用 semantics mutation 或
proposal/adoption 路径；重复调用同一 task/label 只在目标 label 仍无 semantics 时保持
task-label 绑定幂等。成功响应状态为 `201 Created`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "ref": "default#12",
      "labels": [
        {"id": "l_01HX...", "board_id": "b_01HX...", "name": "database", "color": null}
      ]
    },
    "semantics": {
      "label_id": "l_01HX...",
      "board_id": "b_01HX...",
      "label_name": "database",
      "description": "Database persistence work",
      "applies_when": ["touches SQLite migrations"],
      "excludes_when": ["UI-only polish"],
      "positive_examples": ["new table migration"],
      "negative_examples": ["CSS-only tweak"],
      "atoms": []
    }
  }
}
```

HTTP bootstrap 不包含 CLI `--verify` 的 orchestration：请求体没有 vector config、
minimum score 或 verify flag，响应也没有 `verification` 字段。该 endpoint 不会替调用方
重建 label atom vector index、运行 `label suggest` 或检查分数门槛；需要 pre-commit
staged verification 的零写入失败语义时使用 CLI `label bootstrap --verify`。API 调用后如需
诊断，可显式执行 index rebuild / suggest / review 流程，但这不具备 CLI staged verifier 的
同一事务 adoption contract。

`DELETE /api/v1/tasks/{task_id}/labels/{label_id}` 会移除 task 上的指定 label，
`{label_id}` 接受 label id 或 label 名称。成功响应同样返回更新后的 task，包含
当前 `labels` 列表。只有关联行发生变化时，label attach/remove 才写入 task
label event；该操作不改变 task status。

### 12.1 Label semantics, atoms, and atom index

`GET /api/v1/boards/{board}/labels/semantics` 返回当前 board 已定义 semantics 的
列表。`GET /api/v1/boards/{board}/labels/{label_id}/semantics` 返回单个 label
semantics；`{label_id}` 只接受 canonical `l_...` label id。Label name 允许包含
`/` 等 path 不安全字符，因此 semantics API path 不支持按 label name 寻址；需要按
名称查找时，先调用 `GET /api/v1/boards/{board}/labels` 获取对应 id。

`PUT /api/v1/boards/{board}/labels/{label_id}/semantics` 写入已有 label 的语义字典，
同步重建该 label 的 SQLite `label_atoms`，并标脏派生的 label atom vector index。
请求 body：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice",
  "expected_semantics_hash": "optional-current-hash",
  "replace": false,
  "reason": "Add a repeated boundary observed during label review",
  "source_signal_ids": ["los_..."],
  "description": "Backend service work",
  "applies_when": ["touches Rust service code"],
  "excludes_when": ["CSS-only"],
  "positive_examples": ["add API handler"],
  "negative_examples": ["adjust spacing"],
  "remove_applies_when": [],
  "remove_excludes_when": [],
  "remove_positive_examples": [],
  "remove_negative_examples": []
}
```

默认 `replace=false`，请求按 patch 语义处理：`description` 只在提供非空值时覆盖当前
description，数组字段会追加到对应集合，`remove_*` 数组删除匹配文本；缺省字段不会清空
已有 semantics。传 `replace=true` 时才完整替换五个语义字段，此时缺省数组视为空数组，
并且不能同时传任何 `remove_*` 字段。`expected_semantics_hash` 是 CAS guard；如果与
当前 `semantics_hash` 不一致，请求返回 conflict 且不写入。服务会 trim 并丢弃空白值。
每次实际改变 canonical semantics/atoms 的 constructive semantics write 都会在同一
SQLite transaction 写入一条 `update_semantics` root ontology action，记录 actor、reason、
source signal links（如有）、before/after hash 和单份 change snapshot；实际 added/removed
atoms 通过 `label_ontology_action_atom_effects` 写 `added` / `removed` rows。Description-only
patch 会写一条 root action 和零 atom effects；no-op patch 不写 action/effects，也不标脏
label atom index。生成 atoms 时，有 description
的 label 会生成一个 canonical `description` atom：
`label: {name}\ndescription: {description}`；没有 description 时才使用 `name`
fallback atom。atom text 会进一步规范化 whitespace：每个非空行内部 collapse，
canonical 行分隔保留。同一 label 下相同 `polarity + kind + normalized_text` 的 atom
会去重并保留首次 ordinal，`id` / `content_hash` 不包含 ordinal，因此只调整数组顺序
不会改变同一文本 atom identity。响应使用 `DataEnvelope`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "label_id": "l_01HX...",
    "board_id": "b_01HX...",
    "label_name": "backend",
    "description": "Backend service work",
    "applies_when": ["touches Rust service code"],
    "excludes_when": ["CSS-only"],
    "positive_examples": ["add API handler"],
    "negative_examples": ["adjust spacing"],
    "created_at": 1717520000000,
    "updated_at": 1717520000000,
    "atoms": [
      {
        "id": "la_...",
        "label_id": "l_01HX...",
        "board_id": "b_01HX...",
        "label_name": "backend",
        "polarity": "positive",
        "kind": "applies_when",
        "text": "touches Rust service code",
        "ordinal": 2,
        "content_hash": "...",
        "created_at": 1717520000000,
        "updated_at": 1717520000000
      }
    ]
  }
}
```

`DELETE /api/v1/boards/{board}/labels/{label_id}/semantics?expected_semantics_hash=<hash>&reason=<text>`
是 CAS-protected semantics clear：`expected_semantics_hash` 与非空 `reason` 都必填。
它删除该 label 的 semantics 与 SQLite atoms，但不删除 canonical label identity 或
task-label binding；同一 transaction 写一条 `update_semantics` root ontology action，
after snapshot 为空，并为实际 removed atoms 写 `removed` atom effects，随后标脏 label
atom index。Hash mismatch 时 canonical、action、effects 和 dirty state 全不变。成功返回：

```http
DELETE /api/v1/boards/default/labels/l_01HX/semantics?expected_semantics_hash=sem_abc123&reason=Retire%20obsolete%20semantics
X-Kanban-Actor: alice
```

<!-- schema-doc: contract=api.label-semantics-delete.response fixture=schemas/fixtures/api/delete-response.v1.valid.json -->
```json
{ "data": { "deleted": true } }
```

`GET /api/v1/boards/{board}/labels/atoms` 返回 SQLite `label_atoms` materialized
projection。它由 `label_semantics` 和 label name 展开、随 semantics mutation 同事务重建，
是 `lancedb_label_atoms` 派生索引的输入；不要把它描述成独立于 semantics 的第二份
semantic truth。

`GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain` 按当前 atom id 或稳定
`content_hash` 解析 atom，并返回 `LabelAtomExplainRecord`：`query`、`atom`、
`current_semantics`、`provenance_actions`、`supporting_signals`、
`validation_history`、`legacy_untracked` 和 `legacy_reason`。当前 atom 存在但没有
ontology provenance action 引用其 id 或 content hash 时返回 `200` 且
`legacy_untracked=true`；未知 id/hash 返回 not found。

`GET /api/v1/boards/{board}/labels/atom-index/status` 返回 label atom vector index
状态。server no-heavy route 通过 vector helper adapter 报告当前 helper 能力。无 vector provider、
adapter 不可用或 helper 缺失时仍返回 `200` disabled 状态。JSON 保留兼容字段 `message`，并额外返回结构化
`diagnostics: string[]`、`dirty: boolean | null`、`board_dirty: boolean | null`；调用方应使用结构化字段判断 dirty/error，
而不要解析 `message` 文案。同一 `VectorStoreStatus` shape 也用于 `/api/v1/vector/status`。

`POST /api/v1/boards/{board}/labels/atom-index/rebuild` 通过 vector helper adapter 调用
label atom 专用 `rebuild-label-atoms` helper command，重建 `lancedb_label_atoms` 派生索引并更新
`label_atom_index_boards` / `lancedb_label_atoms` status。helper/provider 缺失返回显式 API error，
不得写 canonical label truth，也不得把 chunk store status 当作 label atom rebuild success。
`GET /api/v1/boards/{board}/labels/atom-index/query` 通过 vector helper adapter 查询派生的
`lancedb_label_atoms` 索引。请求必须提供 `q=<text>` 或 `vector_json=<json-array>` 之一，二者互斥；
`embedding_model` 可选，`include_vector=true` 可要求 raw vector hit 返回向量，`polarity` 可选且只接受
`positive` / `negative`，`limit` 默认 24。hit 中的 `distance` 是 LanceDB `_distance`，不是 solver
similarity score。未配置 provider、adapter/helper 不可用或 vector store 不可用时，query 返回显式 API error，
不修改 SQLite truth。

### 12.2 Task label suggestions

```http
GET /api/v1/tasks/{task_id}/labels/suggestions?limit=5&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15
```

返回 task-level label suggestions。带可用 label atom vector store 的部署使用 task title + description embedding
查询 `lancedb_label_atoms`：正向 atoms 按 residual 多轮检索，负向 atoms 固定用原始
query 检索并做 penalty / suppression。solver 在 label group 层执行 Group OMP 选择，
再把选中 label 的 top positive atom vectors 作为 basis 做 non-negative refit；
`coverage` / `residual_norm` 来自 atom-level fitted vector，其中
`coverage = clamp(1 - residual_norm, 0.0, 1.0)`，因此二者不是两份独立证据；
`coverage_cosine` 是原始 query 与 fitted vector 的 cosine similarity，可作为
独立补充指标。候选 label 只有在
tentative refit 后带来足够 residual norm 降幅才会进入结果；coverage 或
residual norm 达到停止阈值后，solver 会提前停止而不是凑满
`max_selected_labels`。candidate group 与已选 label 语义向量过度相似时会被跳过，
以减少重复语义 label 同时出现在 `selected_labels`；这不会合并或删除 canonical
labels。`needs_new_label` 是兼容字段，只表示存在需要人工 review 的 label
coverage 诊断；具体原因必须读取 `reason_codes`，并结合 evidence atoms、
diagnostics 与人工语义判断，不应仅凭该布尔值创建 vocabulary。接口不会创建新
label，也不会写入 `label_semantics` / `label_atoms`。

`limit` 只控制 response 中 `selected_labels` / `candidates` 的最大条数，不会收窄
solver 内部搜索能力。内部能力由 `candidate_limit`、`atom_limit` 和
`max_selected_labels` 分别控制：候选 label group 数、每轮 atom vector 检索上限、
以及最多进入 non-negative refit 的 label 数。所有 limit 参数都必须是
`1..=1000`；`min_score` 必须在 `0..=1`。

未配置 provider、label vector adapter/feature 不可用、LanceDB 表缺失、索引为空或索引
dirty 时，接口仍返回 `200` 和结构化 degraded JSON；普通 label CRUD、task
list/search/filter 与状态转移不受影响。Dirty 判断来自结构化 status/SQLite dirty
字段，不依赖 `message` 文案。无 provider 时 `needs_new_label=false`，
避免把 #105 的新 label 创建流程误触发。

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_01HX...",
    "board_id": "b_01HX...",
    "selected_labels": [
      {
        "label_id": "l_01HX...",
        "label_name": "backend",
        "score": 0.82,
        "weight": 0.82,
        "already_applied": false,
        "evidence_atoms": [
          {
            "atom_id": "la_...",
            "label_id": "l_01HX...",
            "label_name": "backend",
            "polarity": "positive",
            "kind": "applies_when",
            "text": "touches server code",
            "score": 0.91
          }
        ],
        "negative_evidence_atoms": []
      }
    ],
    "candidates": [],
    "coverage": 0.82,
    "coverage_cosine": 0.91,
    "residual_norm": 0.18,
    "needs_new_label": false,
    "reason_codes": ["degraded_result", "label_atom_index_dirty"],
    "degraded": true,
    "diagnostics": ["label_atom_index_dirty"]
  }
}
```

稳定 diagnostics 包括：

- `vector_store_disabled`
- `label_atom_index_dirty`
- `label_atom_index_empty`
- `label_atom_index_error`
- `vector_query_error`

非 degraded coverage review 的稳定 `reason_codes` 包括：

- `no_selected_labels`
- `coverage_below_threshold`
- `residual_above_threshold`
- `unexplained_residual`

### 12.3 Label semantic proposals

```http
POST /api/v1/tasks/{task_id}/label-proposals?limit=5&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15
GET /api/v1/tasks/{task_id}/label-proposals
GET /api/v1/label-proposals/{proposal_id}
POST /api/v1/label-proposals/{proposal_id}/accept
POST /api/v1/label-proposals/{proposal_id}/reject
```

`POST /api/v1/tasks/{task_id}/label-proposals` 创建一次新 label proposal attempt。
请求 body 可为空或仅包含 `actor`；此时默认 provider 不可用，接口返回 `200`
degraded attempt，不创建 canonical label、`label_semantics`、`label_atoms` 或
`task_labels`。

Provider boundary：API 当前只支持空/default provider 或请求 body 中显式传入的
本地/offline candidate。真实 LLM provider 不在 `kanban-sqlite` 中实现；如果未来
server 支持本机 AI/runtime，它必须在 server/local/独立 AI crate 层实现
`LabelProposalProvider` adapter，并把 candidate 交给 SQLite service 做 deterministic
validation 和 persistence。

带本地/offline provider 输出时：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "proposal": {
    "name": "database",
    "description": "Database persistence work",
    "applies_when": ["touches SQLite migrations"],
    "excludes_when": ["UI-only polish"],
    "positive_examples": ["new table migration"],
    "negative_examples": ["CSS-only tweak"]
  },
  "actor": "alice",
  "source_signal_ids": ["los_..."],
  "ontology_actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "allow_retarget": false,
  "retarget_reason": null
}
```

数组字段缺省时按空数组处理。服务先读取当前 label suggestion 的启发式
`coverage` / `coverage_cosine` / `residual_norm` / top1 existing label。coverage 充足时不写 proposal；
coverage 不足且候选语义有效，并且残差 top1+margin 校验明确通过时，返回 `201` 并持久化
`proposed` proposal。与现有 label 发生 normalized-name 冲突的候选持久化为 `rejected`，diagnostics 包含
`near_duplicate_label_conflict`。Normalized-name conflict 忽略大小写、空白和标点，
是 deterministic near-duplicate heuristic。
`source_signal_ids` 可选；传入时，proposal 创建成功后会在同一 transaction 写入
`create_label_proposal` ontology action，并通过 action-signal links 记录该 proposal
由哪些 confirmed vocabulary-gap signals 支持。Proposal row 与 provenance action
要么同时写入，要么一起回滚。Source signals 默认必须属于同一 board、状态为
`confirmed`、kind 为 `vocabulary_gap`、`proposed_action` 为 `bootstrap_label`，且
normalized `proposed_label_name` 等于 proposal name。`ontology_actor` 只控制
`create_label_proposal` action provenance；省略时使用 `actor` 字符串作为
`type=user` actor。确需 retarget confirmed same-board source signal 时，必须传
`allow_retarget=true` 和非空 `retarget_reason`；reason 和 source signal 原始
target/proposed label 会写入 `change_json.retarget_override`。Override 不放宽
board/status 要求。

POST proposal route 接受与 label suggestion 相同的 query 参数。`limit` 只截断
suggestion 输出；`candidate_limit`、`atom_limit`、`max_selected_labels` 和 `min_score`
调节用于 heuristic coverage / residual validation 的底层 solver。

当 server 配置了可用 vector provider 时，proposal attempt 与 label suggestion
使用同一套 LanceDB label atom store。coverage 不足的候选会在持久化前执行残差
top1+margin 校验：候选语义的 residual score 和现有 label top1 都按返回 atom
vector 在本地计算 cosine similarity，不从 LanceDB distance 推导；候选必须超过现有
label top1，且超过幅度达到固定 margin。校验失败时候选仍会以 `rejected` proposal 持久化，diagnostics
包含 `label_proposal_residual_top1_failed` 或
`label_proposal_residual_margin_insufficient`。未配置 provider、feature 不可用或
vector 检索失败时返回 degraded attempt，不创建 canonical label、`label_semantics`、
`label_atoms` 或 `task_labels`。如果 residual validation 不可用或 degraded，且没有
明确通过 top1+margin 校验，attempt 返回 `proposal=null`，不新增 proposal row，
diagnostics 包含 `label_proposal_residual_validation_unavailable` 和具体原因。

Attempt response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_...",
    "board_id": "b_...",
    "proposal": null,
    "degraded": true,
    "diagnostics": ["label_proposal_provider_unavailable", "vector_store_disabled"],
    "heuristic_coverage": 0.0,
    "heuristic_coverage_cosine": 0.0,
    "heuristic_residual_norm": 1.0,
    "top1_existing_label_id": null,
    "top1_existing_label_name": null
  }
}
```

Accept/reject body：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "coverage 不足，接受新 label",
  "actor": "alice",
  "source_signal_ids": ["los_..."],
  "ontology_actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "allow_retarget": false,
  "retarget_reason": null
}
```

Accept 只允许 `proposed` proposal。成功后会通过与 task-label bootstrap 相同的 adoption
primitive 创建 canonical `labels` 行与对应 `label_semantics` / `label_atoms`，
标脏 label atom index，并在同一 transaction 写入一条 `bootstrap_label` root ontology
action 和对应 added atom effects；
proposal status、canonical writes 和 provenance action 要么一起成功，要么一起回滚。
它不会自动写 `task_labels`。`source_signal_ids` 可选；省略时仍记录 bootstrap action，
但没有 action-signal links。传入时，accept 会通过 action-signal links 记录 new-label
bootstrap provenance。Source signals 必须属于同一 board 且处于 `confirmed`。
`actor` 字符串仍用于 proposal decision event；`ontology_actor` 只控制 accept 产生的
`bootstrap_label` ontology action provenance。省略 `ontology_actor` 时，bootstrap
action 使用 `actor` 字符串作为 `type=user` actor。`type=agent` 必须提供非空
`agent_type`；`type=user` 不能提供 `agent_type`。Source signals 默认还必须是
`vocabulary_gap` + `bootstrap_label`，且 normalized `proposed_label_name` 必须等于
proposal name。确实需要 retarget confirmed same-board source signal 时，必须传
`allow_retarget=true` 和非空 `retarget_reason`；bootstrap action
`change_json.retarget_override` 会记录 reason、source signal 原始 target/proposed
label 和最终 proposal/result label。如果 proposal 已有 `create_label_proposal`
action，accept 产生的 `bootstrap_label` action 会把 `parent_action_id` 指向该
creation action。Override 不放宽 board/status 要求。Reject 标记为
`rejected`，不接受 `source_signal_ids`、`ontology_actor` 或 retarget options。
accepted/rejected proposal 再次决策返回普通 `400 invalid_input` error envelope。

### 12.4 Generic signal ledger

Generic signal ledger API 提供 board-scoped 只读 inbox，用于展示 agent/product
在 kanban 工作流中记录的通用 signal，例如 CLI 参数摩擦、提示误导、参数设计问题或
operator 发现。它独立于 label ontology ledger；这些 endpoint 不创建、确认、拒绝、
resolve 或 supersede signal，也不会把通用 signal 混入 ontology review groups。

```http
GET /api/v1/boards/{board}/signals?status=open&kind=agent_cli_friction&task_ref=default%23123&include_all=false&limit=100
GET /api/v1/boards/{board}/signals/review?status=confirmed&kind=agent_cli_friction&task_ref=default%23123&include_all=false&limit=100
GET /api/v1/signals/{signal_id}
```

`GET /api/v1/boards/{board}/signals` 和 `/signals/review` 返回同一只读 DTO；
`review` endpoint 是 Desktop / operator console 的语义化入口。默认只返回 `open`
和 `confirmed`。可重复传 `status` 和 `kind`，并按 `task_ref` 过滤。
`include_all=true` 且没有显式 `status` 时返回完整历史；`limit` 使用普通列表上限。
这些 list/review routes 是 board-scoped surface；只返回该 board 的 signal rows。
`GET /api/v1/signals/{signal_id}` 是 operator-wide detail lookup，用于从 backlink、
inbox row 或审计记录直接打开已知 signal。该 detail route 不改变 signal 的
`board_id` truth，也不让 board-scoped list/review 泄漏其它 board 的 signal。

`signal_observations.task_id`、`run_id` 和 `comment_id` 是 provenance/history soft refs。
当前 service 写入路径、doctor 和 import final gate 维护这些 refs 与 observation board
的一致性；未来若需要把所有来源关系硬化，可迁移为 board-composite FK。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "sig_...",
      "board_id": "b_...",
      "observation_id": "obs_...",
      "kind": "agent_cli_friction",
      "title": "--require 参数命名不符合 agent 惯用预期",
      "summary": "agent 尝试使用 --required/--requires，实际 CLI 只接受 --require。",
      "severity": "medium",
      "status": "open",
      "dedupe_key": "kanban-task-create-require",
      "superseded_by_signal_id": null,
      "reviewed_by": null,
      "reviewed_at": null,
      "review_reason": null,
      "created_at": 1782930000000,
      "updated_at": 1782930000000,
      "observation": {
        "id": "obs_...",
        "board_id": "b_...",
        "task_id": "t_...",
        "task_ref_snapshot": "default#123",
        "run_id": "r_...",
        "comment_id": null,
        "actor": "codex",
        "agent_type": "codex",
        "source": "codex-hook",
        "evidence": {"command":"kanban task create --required ..."},
        "created_at": 1782930000000
      }
    }
  ]
}
```

`GET /api/v1/signals/{signal_id}` 返回单条 signal：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "id": "sig_...",
    "observation": {}
  }
}
```

### 12.5 Label ontology ledger

Label ontology ledger API 记录 task 标注过程、review queue、ontology mutation
provenance 和 validation history。Ledger 不会自动修改 task labels；canonical
binding 仍通过 task label API 或 CLI 完成。

所有 ontology actor object 使用 `{ "name": string, "type": "user"|"agent",
"agent_type": string|null }`。`type=agent` 必须提供非空 `agent_type`；
`type=user` 必须省略或传 `null`。

```http
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals?status=open&kind=false_negative&task_ref=default%2312&target_label_ref=cli&proposed_label_name=database&include_all=false&limit=100
GET /api/v1/boards/{board}/label-ontology/review?group_by=label&include_all=false&limit=100
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/revert
POST /api/v1/boards/{board}/label-ontology/validate
```

`POST /api/v1/tasks/{task_id}/label-ontology/observations` 在一个 transaction 中写入
observation 和 child signals。HTTP endpoint 不自行运行 `label suggest`；调用方必须传入
由工具采集且未改写的 `suggestion_snapshot`，或在没有 suggest 证据时显式传空 snapshot。
服务端会从 snapshot 派生 observation metrics，agent/reviewer 只提交候选、最终判断、
signals、candidate atom 和 rationale。请求 body：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
  "agent_candidates": [],
  "suggestion_snapshot": {
    "selected_labels": [],
    "coverage": 0.61,
    "coverage_cosine": 0.74,
    "residual_norm": 0.39,
    "needs_new_label": false,
    "degraded": false,
    "diagnostics": []
  },
  "final_decision": {},
  "capture_fingerprint": "optional-stable-key",
  "signals": [
    {
      "kind": "false_negative",
      "target_label_ref": "cli",
      "related_labels": [],
      "proposed_action": "add_positive_atom",
      "candidate_atom": {
        "polarity": "positive",
        "kind": "applies_when",
        "text": "extends CLI subcommands, command arguments, help output, or machine-readable JSON behavior"
      },
      "proposal": {},
      "agent_selected": true,
      "suggest_state": "candidate",
      "suggest_score": 0.08,
      "suggest_rank": 6,
      "final_selected": true,
      "rationale": "The task expands the CLI surface.",
      "confidence": 0.9
    }
  ]
}
```

HTTP ontology DTOs use natural JSON fields for new clients:
`agent_candidates`, `suggestion_snapshot`, `final_decision`, `diagnostics`,
signal `related_labels` / `proposal`, action `change` / `validation`, and
validate `validation`. Legacy escaped-string request siblings (`related_labels_json`,
`proposal_json`, `change_json`, `validation_json`, and the observation `*_json`
aliases) are no longer accepted by the public HTTP API. Unknown legacy keys fail
closed with `400 invalid_input`; clients must send the natural JSON fields. When `suggestion_snapshot`
contains `coverage`, `coverage_cosine`, `residual_norm`, `needs_new_label`,
`degraded`, or `diagnostics`, the server derives the stored observation metrics
from that snapshot. If the request also supplies the matching top-level
`suggest_*` field or `diagnostics` and the values conflict, the request returns
`400 invalid_input`. New clients should not repeat snapshot facts as top-level
scalars.

Service 会读取当前 task snapshot、解析 `target_label_ref`、计算 normalized proposed
label name、signal key 和 candidate atom content hash。`capture_fingerprint` 为空时
按 task、snapshots 和 signals 派生；同一 board 重复 fingerprint 会被唯一约束拒绝。
Observation response 返回 created observation，并展开 child `signals`。Observation
包含完整审计用 `task_snapshot_json.content_hash`，以及只基于 label suggest 输入
（normalized title + description）的 `suggest_input_hash`；后者用于后续 validation
comparability。

Signal 输入会在写入前做 ontology contract 校验。`candidate_atom` 的
`applies_when` / `positive_example` 只能使用 `positive` polarity，
`excludes_when` / `negative_example` 只能使用 `negative` polarity。
`add_positive_atom` 必须提供 target label 和 positive candidate atom；
`add_negative_atom` 必须提供 target label 和 negative candidate atom；
`update_semantics` 必须提供 target label；`bootstrap_label` 必须提供
`proposed_label_name`；`rename_label` 必须提供 target label 和
`proposed_label_name`；`split_label` / `merge_labels` 必须提供 target label 和非空
`related_labels`。Observation metric `suggest_coverage`、
`suggest_coverage_cosine`、`suggest_residual_norm` 以及 signal metric
`suggest_score` / `confidence` 必须是 finite `0.0..=1.0`；`suggest_rank` 必须为
`null` 或 `>= 1`。违反这些契约的 request 返回 `400 invalid_input`，不会写入
observation 或 signals。`rename_label` / `split_label` / `merge_labels` 当前只作为
review signal proposed_action 保存，不能通过 public HTTP route 写入 canonical structure
mutation action；旧 structure-plan rows 只读展示为 unsupported validation requirement。

`GET /api/v1/boards/{board}/label-ontology/signals` 默认只返回 `open` 和
`confirmed`。可重复传 `status` 和 `kind`，并按 `task_ref`、`target_label_ref`、
`proposed_label_name`、`include_all`、`limit` 过滤。

`GET /api/v1/boards/{board}/label-ontology/review` 返回只读聚合 review queue。
`group_by` 支持 `label`、`candidate_atom`、`proposed_label`、以及 opt-in `cluster`，
默认 `label`；`include_all=false` 默认只聚合 `open` 和
`confirmed` signals，`true` 时包含完整历史；`limit` 限制 group 数量。响应
`meta` 回显 `group_by`、`include_all` 和 `limit`。每个 group 包含：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "group_by": "label",
  "key": "lab_...",
  "label_id": "lab_...",
  "label_name": "cli",
  "candidate_atom_polarity": "positive",
  "candidate_atom_kind": "applies_when",
  "candidate_text": "extends CLI subcommands",
  "candidate_content_hash": "14ada47e4b0566c5",
  "proposed_label_name": null,
  "proposed_label_name_normalized": null,
  "cluster_key": null,
  "cluster_reason": null,
  "task_count": 2,
  "signal_count": 3,
  "open_count": 2,
  "confirmed_count": 1,
  "resolved_count": 0,
  "rejected_count": 0,
  "superseded_count": 0,
  "degraded_count": 1,
  "average_score": 0.31,
  "median_score": 0.28,
  "oldest_signal_at": 1781780000000,
  "latest_signal_at": 1781780100000,
  "sample_task_refs": ["default#12"],
  "signal_ids": ["los_..."],
  "action_count": 1,
  "action_ids": ["loa_..."],
  "proposal_ids": [],
  "labels": [{"id": "lab_...", "name": "cli"}],
  "candidate_atom_variants": [
    {
      "content_hash": "14ada47e4b0566c5",
      "polarity": "positive",
      "kind": "applies_when",
      "text": "extends CLI subcommands",
      "signal_count": 2
    }
  ]
}
```

Groups sort by distinct `task_count` desc, then `confirmed_count` desc,
`latest_signal_at` desc, and `key` asc。`group_by=cluster` 是可禁用的只读辅助视图：
默认不会启用，不写 canonical atoms，不确认、应用、validate 或关闭 signal，也不会创建
新的 SQLite truth 表。cluster key 每次请求时从已有 signal 文本重建，优先使用
lexical-normalized candidate text，其次 proposed label，再其次 rationale，最后退回到
kind/action/target/proposed-label scope 组合；所有 cluster key 都带有 signal kind、
proposed action、target label 和 proposed-label scope，避免跨 label/action/boundary 误合并；
`cluster_reason` 说明 key 来源。`GET /api/v1/label-ontology/signals/{signal_id}`
返回：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "signal": {},
    "observation": {},
    "actions": []
  }
}
```

`POST /api/v1/boards/{board}/label-ontology/actions` 写 review/lifecycle action：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "alice", "type": "user", "agent_type": null},
  "action_type": "confirm",
  "signal_ids": ["los_..."],
  "reason": "Observed across independent CLI tasks",
  "superseded_by_signal_id": null,
  "parent_action_id": null,
  "target_label_ref": null,
  "result_label_ref": null,
  "result_atom_id": null,
  "result_atom_content_hash": null,
  "result_proposal_id": null,
  "canonical_before_hash": null,
  "canonical_after_hash": null,
  "validation_requirement": null,
  "validation_status": null,
  "validation_effective_outcome": null
}
```

该公共 action endpoint 只接受 lifecycle action types：`confirm`、`reject`、
`supersede` 和 `resolve_no_change`，并会同步更新 source signal status。请求中的
`parent_action_id`、`target_label_ref`、result 字段、canonical hash、`change`、
`validation_requirement`、`validation_status`、
`validation_effective_outcome` 和 `validation` 必须为
`null`/缺省；否则返回
`invalid_input`。`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
`update_semantics`、`create_label_proposal`、`bootstrap_label`、`revert_ontology_mutation`、`validate` 等 mutation/validation action
types 不允许通过该 generic endpoint 写入；canonical mutation provenance 必须由
semantics PUT、apply atom、proposal create/accept、task-label bootstrap 或 validate 等
专用 route 在同一 transaction 内写入。`supersede` 写入时会沿 replacement
`superseded_by_signal_id` 链检查，若链路回到任一 source signal 或 replacement chain
自身已有环，则返回 `invalid_input`，不会写入新的 supersede action。

`POST /api/v1/boards/{board}/label-ontology/apply/atom` 对已有 label 执行
read-modify-upsert semantics，并写入 atom provenance action：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "signal_ids": ["los_1", "los_2"],
  "label_ref": "cli",
  "kind": "applies_when",
  "text": "extends CLI subcommands, command arguments, help output, or machine-readable JSON behavior",
  "reason": "Repeated false-negative signal across CLI surface tasks",
  "allow_retarget": false,
  "retarget_reason": null
}
```

Source signals 必须属于同一 board 且已 `confirmed`。`kind` 只接受
`applies_when`、`positive_example`、`excludes_when`、`negative_example`。如果 canonical
内容实际新增 atom，成功后返回 `add_positive_atom` 或 `add_negative_atom` action，
记录 result atom soft reference、content hash、before/after canonical hash、单份
change snapshot 和一个 `added` atom effect，并把 `validation_requirement` 置为
`required`。如果同内容 atom 已经存在，成功后返回
`adopt_existing_atom` provenance-only action，记录 existing atom soft reference、相同的
before/after canonical hash 和 source signal links；该 action 不修改 semantics/atoms、
不标脏 atom index，`validation_requirement=none` 且 effective outcome 为
`not_required`。默认要求所有带 `target_label_id` 的 source signals 都指向 `label_ref`；
不匹配时返回 `400 invalid_input` 并列出 offending signal ids。Atom text 可由 reviewer
泛化，不要求等于 source signal 的 candidate text。确实需要 retarget confirmed
same-board signals 时，必须传 `allow_retarget=true` 和非空 `retarget_reason`；
action `change_json.retarget_override` 会记录 reason、source signal 原始 target/proposed
label 和最终 target label。Override 不放宽 board/status 要求。该 route 只有在
canonical atom 实际新增时才标脏 label atom index；vector rebuild 和 suggest validation
在 transaction 外执行。

`POST /api/v1/boards/{board}/label-ontology/revert` 追加可追溯 rollback action，并把
目标 label semantics 恢复为被撤销 mutation action 的 before snapshot：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "reviewer", "type": "user", "agent_type": null},
  "target_action_id": "loa_...",
  "expected_current_hash": "optional-current-semantics-hash",
  "reason": "Rollback test-only atom mutation"
}
```

当前只支持 `add_positive_atom`、`add_negative_atom` 和 `update_semantics`。Route 要求
当前 canonical semantics hash 仍等于 `target_action_id` 的 `canonical_after_hash`；
`expected_current_hash` 非空时还必须等于当前 hash。成功后返回
`revert_ontology_mutation` action：`parent_action_id` 指向被撤销 action，source signal
links 从目标 action 复制，`change` 记录被撤销 action、before/after revert snapshot 和
`index_dirty=true`，并为本次 revert 实际 added/removed atoms 写 atom effects，随后标脏
label atom index。该 action 的 `validation_requirement` 为 `unsupported`，可记录
external failed/partial 诊断，但不会被当作可 trusted-passed 的 pending validation。该 route
不删除或修改原 action，也不处理 bootstrap label identity / task binding rollback；CLI
staged bootstrap verify 的失败路径在提交前零写入，不再依赖提交后的恢复流程。

`POST /api/v1/boards/{board}/label-ontology/validate` 追加 external attestation
validation action。HTTP route 接收调用方提交的自然 JSON `validation`，
但当前不运行 vector rebuild、index query 或 `label suggest`，因此它不能产生
trusted automated `passed`。需要 trusted automated validation 时使用 CLI
`label ontology validate --trusted`，由工具采集 index/suggest evidence 后写入。

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "parent_action_id": "loa_...",
  "signal_ids": ["los_1", "los_2"],
  "reason": "Source task still does not select the target label after atom rebuild",
  "validation_status": "failed",
  "validation": {
    "evidence_type": "external_attestation",
    "reviewer": "codex",
    "cases": [
      {
        "signal_id": "los_1",
        "case_type": "positive_atom",
        "passed": false,
        "before": {
          "target": {"label_id": "l_cli", "selected": false, "score": 0.12},
          "coverage": 0.61
        },
        "after": {
          "degraded": false,
          "target": {"label_id": "l_cli", "selected": false, "score": 0.14},
          "coverage": 0.60,
          "notes": "Manual review of stored suggest output did not meet pass criteria"
        }
      }
    ]
  }
}
```

Service 会把 supplied `validation` 包进 validation envelope，附上
source signal cases、observation task snapshot / suggest input hash 与当前 task hash
对比、parent action result 引用和 summary。公共 supplied/collected payload 只保存在
top-level `manual`；generated `cases[]` 通过 `after.manual_case_ref` 指向
`manual.cases[]` 中对应 signal 的原始 evidence，避免多 signal validation 把同一 payload
重复存入每个 case。`parent_action_id` 必须指向同一 board 上
`validation_requirement=required` 的 canonical mutation action，且 parent action 必须带有
canonical result evidence（例如 atom/result label/proposal 引用、canonical hash 和
非空 change snapshot）。HTTP supplied JSON 是 external attestation；它可保存
`failed` / `partial` 诊断，但 `validation_status="passed"` 会返回 `invalid_input`，
因为 passed validation 需要工具采集的 `trusted_automated` evidence。`unsupported`
parent 可以记录 external failed/partial 诊断，但不能 passed。结构化字段或字符串
`"automated"` 本身不构成可信来源。

Trusted automated validation 的 persisted payload 由 CLI collector 生成，而不是由 HTTP
caller 手写：top-level `evidence_type="trusted_automated"`、`collector.source`、
非空 `embedding_model`、object `solver_options`、clean `index.status`、
`index.generation` 和覆盖每个 linked source signal 的 `cases[]`。CLI collector 在长
SQLite transaction 外 rebuild atom index 并运行 suggest；写 action 时 service 在短
transaction 中重新核验 parent action、source signals、canonical after hash、index
dirty/error 状态和 generation。Trusted 表示工具采集、current hash/index generation
一致，并在指定 cases/controls 上机械通过；它不是全局语义正确性证明。

Typed policy 按 parent action 检查：

- `add_positive_atom`：`case_type="positive_atom"`，`after.degraded=false`，
  `after.evidence_atoms[]` 必须包含 parent `result_atom_id` 或
  `result_atom_content_hash`；target label 必须 selected 或 score >= 0.50；
  score/coverage 不能比 before 恶化。
- `add_negative_atom`：`case_type="negative_atom"`，`after.evidence_atoms[]`
  不用于 result negative atom 校验；parent result atom 必须出现在
  `after.negative_evidence_atoms[]`。false-positive task 上必须证明
  `after.target.selected=false`，或 before/after score 都存在且 after score 低于
  before score。必须提供至少一个 `after.positive_controls[]` 且每个 control
  passed 且未 regressed；若没有 positive control，必须提供带非空 reason 的
  `after.positive_control_waiver`。
- `bootstrap_label`：`case_type="bootstrap_label"`，所有 linked source signals
  都必须有 passed case；new/result label 必须 selected 或 score >= 0.50；
  evidence atoms 必须来自 result label。

Validation comparability 默认使用 observation 的 `suggest_input_hash`；status、
`updated_at`、`lock_version` 或 task label binding 只改变完整 snapshot 时写入
`task_metadata_drift` / `label_binding_drift` warning，不会让 passed validation stale。
title/description 变化会写入 `suggest_input_drift` 并使 case incomparable；旧
observation 缺少 `suggest_input_hash` 时写入 `legacy_suggest_input_hash_missing`，
不能静默 passed。`passed` 会把 linked source signals 转为 `resolved`；`failed` 与
`partial` 保留 signals 供后续修正或人工处理。

---

## 13. Search

### 13.1 Search tasks

```http
GET /api/v1/search/tasks?board=default&q=needle&status=ready&label=backend&assignee=worker-a&include_archived=false&limit=20&offset=0
```

默认 CLI/server build 启用 `tantivy-backend`。SQLite DB 旁存在 `index/v1/tasks/` 时，search 使用 Tantivy task index。Tantivy index 缺失、损坏、过期或二进制显式以 `--no-default-features` 构建时会回落到 SQLite，并带上 stale metadata。搜索匹配 task title、description、comments、run summary/error 和 event kind/payload。

`label` 按 label 名称或 id 过滤，可重复，并在评分和分页前使用 AND 语义。
带 label 过滤的 search 即使存在可用 Tantivy index，也会使用 SQLite fallback，
以确保结果反映当前 task-label 关联行。

Task ref 形状的 `q` 始终使用 SQLite 精确匹配语义，即使当前存在可用 Tantivy index：
纯数字 `12` 和 `#12` 匹配请求 `board` 内的 seq；`board#12` 和 `board/#12`
只在显式 board 等于请求 board 时匹配；`t_...` 只匹配请求 board 内的 task id。
Ref 形状 query 不会从 title、description、comments、runs 或 events 中返回模糊匹配。

Response:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "hits": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "score": 60.0,
        "snippet": "ready spec needle",
        "task": {
          "id": "t_01HX...",
          "seq": 12,
          "status": "ready",
          "title": "实现状态机"
        }
      }
    ],
    "meta": {
      "backend": "sqlite",
      "stale": false,
      "index_version": null,
      "last_event_id": 42,
      "index_lag_events": 0
    }
  },
  "meta": {
    "limit": 20,
    "offset": 0
  }
}
```

Task mutations do not write Tantivy inside their SQLite transactions. When served by `kanban serve` with `tantivy-backend`, a background loop makes one prompt startup `sync_search_index` attempt and then syncs every `--search-sync-interval-ms` milliseconds by default (`5000`; `0` disables). Manual `kanban index sync` remains available after normal task changes, and `kanban index rebuild` replaces the derived index. The Tantivy state is stored in board-scoped `app_settings` under `search.tasks.state.<board_id>` and round-trips through existing export/import.

### 13.2 Search task windows by status

```http
GET /api/v1/search/tasks/by-status?board=default&q=needle&status=ready&status=review&include_archived=false&limit=50&offset=0
```

This read-only endpoint batches board search columns into one request. It
accepts the same query, board, label, assignee, archive, and pagination params as
`GET /api/v1/search/tasks`, but returns one independent search window per
repeated `status`. `limit` and `offset` apply per status window. Status order in
the response follows query parameter order. Omitting `status` returns an empty
`statuses` array.

Response:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "statuses": [
      {
        "status": "ready",
        "tasks": [
          {
            "id": "t_01HX...",
            "ref": "default#12",
            "status": "ready",
            "title": "实现状态机"
          }
        ],
        "search_meta": {
          "backend": "sqlite",
          "stale": false,
          "index_version": null,
          "last_event_id": 42,
          "index_lag_events": 0
        },
        "page": {
          "limit": 50,
          "offset": 0,
          "total": null
        }
      }
    ]
  },
  "meta": {
    "limit": 50,
    "offset": 0
  }
}
```

### 13.3 Search status

```http
GET /api/v1/search/status?board=default
```

Response:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "backend": "sqlite",
    "derived_index": false,
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0,
    "message": "SQLite fallback search is active; no derived index exists yet"
  }
}
```

When the current `MAX(task_events.id)` is greater than the stored Tantivy `last_event_id`, `stale=true` and `index_lag_events` reports that high-watermark lag.
If background sync is disabled, delayed, or fails, search keeps returning current SQLite fallback results with stale metadata instead of trusting an out-of-date derived index.

---

## 14. Maintenance

### 14.1 Doctor

```http
POST /api/v1/maintenance/doctor
```

Response includes SQLite integrity, migration/user version, expired running tasks, orphan run checks, dependency cycle count, archived dependency edge count, missing and suspicious run log counts, executable status invariant counts for dependency/spec/schedule violations, foundation relationship consistency diagnostics, label ontology ledger diagnostics, and Knowledge Substrate diagnostics. Archived parent -> active child edges are allowed historical dependency edges; archived child edges from active parents are counted.

Foundation relationship diagnostics are read-only:

- `consistency_errors` / `consistency_warnings` summarize board consistency findings for base relationship rows.
- `consistency_issues[]` reports structured findings with `severity`, `code`, `message`, and `record_ids`.
- Covered tables: `task_labels`, `task_dependencies`, `task_steps`, `task_execution_plans`, `task_runs`, `task_comments`, `signal_observations`, `signals`, `task_events`, and `task_attachments`.
- v24+ databases require `signal_observations` and `signals` for the generic signal ledger.
- Hard errors mean a row's `board_id` differs from a referenced task / label / run / comment / observation board. The message includes `table`, `row`, `row_board`, `referenced`, and `referenced_board`.
- v25+ databases add board-scoped composite FKs for `signals.observation_id` and `signals.superseded_by_signal_id`.
- These checks complement service-layer board-scoped writes. `task_labels`, `task_dependencies`, `task_steps`, `task_execution_plans`, `task_runs`, `task_comments`, `signals`, and `task_attachments` are protected by board-scoped composite FKs in current schema. `signal_observations` and `task_events` retain nullable source references; doctor/import still check those board relationships as a hard-error diagnostic layer for corrupted JSONL/raw-SQL inputs.
- `PRAGMA foreign_key_check` results are surfaced as hard-error `consistency_issues[]` with table, rowid, parent table, and FK index. Import runs the same gate before commit and rolls back on violation.
- Nonzero `consistency_errors` make `ok=false`.

Ontology ledger diagnostics are read-only:

- `ontology_ledger_errors` / `ontology_ledger_warnings` summarize hard errors and warnings.
- `ontology_ledger_issues[]` reports structured findings with `severity`, `code`, `message`, and `record_ids`.
- v12+ databases require `label_ontology_observations`, `label_ontology_signals`, `label_ontology_actions`, `label_ontology_action_atom_effects`, and `label_ontology_action_signals`.
- Hard errors include cross-board ontology links, orphan action-signal/action-effect links, missing parent/supersede references, label/proposal/task board mismatches, signal supersede cycles, and action parent cycles. Nonzero errors make `ok=false`.
- Warnings are reserved for rebuildable or historically explainable soft references, such as an action `result_atom_id` whose current `label_atoms` row was rebuilt away.

Derived-layer diagnostics are read-only:

- `outbox_pending` / `outbox_running` / `outbox_failed` summarize `index_outbox`.
- `derived_dirty_stores` counts stores with `dirty=true`.
- `derived_error_stores` counts stores with `last_error` or failed outbox.
- `derived_stores[]` reports each store's `store_name`, `schema_version`, `last_event_id`, `dirty`, `last_error`, and pending/running/failed outbox counts for that store target.

`derived_stores[].last_event_id` is the store-level successful event watermark, not a board-local watermark. `dirty=true` means the store still has unfinished outbox on any board or a recent update failure; a board-scoped sync/rebuild can advance the watermark while leaving the store dirty if another board still has pending or failed work.

These fields do not make Tantivy/Oxigraph/LanceDB authoritative. SQLite remains the source of truth, and dirty derived stores remain rebuildable caches.

### 14.2 Checkpoint

```http
POST /api/v1/maintenance/checkpoint
```

Runs `PRAGMA wal_checkpoint(TRUNCATE)` and returns `busy`, `log_frames`, and `checkpointed_frames`.

### 14.3 Backup

MVP 建议只提供 CLI backup，不开放 HTTP backup。

---

## 15. Web UI Interaction Rules

1. 拖拽列时调用 transition endpoint。
2. 普通字段编辑调用 `PATCH /tasks/{id}`。
3. Web UI 不显示 claim_token，除非 debug 模式。
4. running task 的 complete/block 操作，若无 token，则 UI 走 `force=true` 并要求确认。
5. blocked task unblock 后目标列由服务端返回，前端不要预设。
6. SSE 收到 event 后，优先 refetch affected task，避免客户端状态机漂移。

### Signal Comments

API comment DTOs use `kind: "signal"` for signal ledger backlink comments. The natural `metadata` object contains `type:"signal_link"`, `signal_id`, `observation_id`, `signal_kind`, and `signal_status` for service-generated backlinks. Generic signal comment metadata remains open and lossless; clients should render the body as a readable fallback and may link to a signal detail only when the complete backlink shape is present.


## Transport catalog implementation note

本规范所列的每个 API/SSE method/path 都由 `kanban-contract` 的 endpoint descriptor catalog 作为唯一实现 source。注册 handler 时使用稳定 `operation_id` 与 `adapter_id`；这两个 identity 分别表示公开 endpoint 和 server runtime binding，不是 Rust type name、函数地址或 `stringify!` 推导值。


## B1-C2b task-read 成功响应契约

`GET /api/v1/boards/:board/tasks` 与 `GET /api/v1/boards/:board/tasks/by-status` 各自拥有独立、精确且闭合的成功响应契约，仅共享 `ApiTask`、`ApiLabel` 与既有 `OffsetPaginationMeta`/`TotalPaginationMeta` primitives。列表响应为 `data[]` 与既有 `TotalPaginationMeta { limit, offset, total }`；按状态响应包含有序窗口，每个窗口使用同一 `TotalPaginationMeta`，外层使用既有 `OffsetPaginationMeta { limit, offset }`。这只是 Rust 类型复用，JSON wire 形状不变。

Desktop 仅对这两个读取端点使用 endpoint-specific recursive exact parser：成功响应的 envelope、`meta`、窗口、共享 `ApiTask`、`ApiLabel` 与既有 `OffsetPaginationMeta`/`TotalPaginationMeta` primitives 都必须闭合且完整，pagination 数值必须是非负 safe integer；错误响应也必须是闭合的 `error { code, message, details? }` envelope。任何 malformed、mixed、missing 或 extra shape 统一返回 `invalid_response`，合法错误继续保留 `code`、`message` 与可选 `details`。其它 generic optional envelope 不受影响。两个 endpoint 的 headers 仍为 `Todo`，本次不采纳任何其它 endpoint。


## B2-C3 comments exact contract

List/create comments 使用各自的 exact path/response root，create 另有 exact request root；五个 roots 均有真实 producer/consumer witness。`author_type` 仅为 `user|agent`，`kind` 仅为 `note|decision|signal`；agent 的 `agent_type` 可缺失、null 或空白（空白归一化为 null），user 携带非空 `agent_type` 仍由 service 拒绝。unknown enum 在 typed API 边界返回 400。写入 comment 与 `task.comment.created` event 保持同一 SQLite transaction。B7 已采用 endpoint-specific header contracts；surface migration 仍为 Generated，等待后续 API adoption cohort 的 exact surface evidence。

### 8.4 B2-C4 exact run-read contracts

List/get 分别拥有闭合 path 与 success root，只共享 contract-owned `ApiRun`。Run status 是闭合
enum：`running|succeeded|failed|canceled|expired`。`worker_profile`、`worker_pid`、
`finished_at`、`exit_code`、`summary` 与 `error` 都是必须出现但可为 `null` 的字段。
`claim_token` 只存在于显式 claim transition response，不进入 list/get run；SQLite `log_path`
只供独立 get-run-log handler 解析受信任文件，不进入 list/get run。读取 archived task 的历史仍被
允许；list 先由全局 task id 解析真实 board，再通过同一 SQLite service path board-scope 查询。
Headers 保持 `Todo`，两个 endpoint 因而保持 `Generated`。schema 只约束 JSON shape，不替代
claim、complete、reopen、archive 或 board isolation service guard。
### 12.6 B4-C1 exact task-label association contracts

`GET /api/v1/tasks/{task_id}/labels`、`POST /api/v1/tasks/{task_id}/labels` 与 `DELETE /api/v1/tasks/{task_id}/labels/{label_id}` 分别拥有闭合、endpoint-specific path/success root；POST 另有闭合 request root。成功响应递归复用同一个 contract-owned `ApiLabel`/`ApiTask` wire shape；POST 的可选 `meta.created_labels` 仅在实际创建 label identity 时出现。Desktop add/remove caller 对 envelope、task、nested labels 与 created-label metadata 做递归 exact 校验，malformed/extra/null meta 统一返回 `invalid_response`。

本批不收敛 board label identity、bootstrap、semantics、atoms/index、suggestion/proposal 或 ontology ledger endpoint。HTTP headers 保持 `Todo`，三个 endpoint 因而保持 `Generated`；name/names 互斥、batch transaction、board/archive scope、幂等与事件语义继续由既有 SQLite service guard 拥有。

## B5-C2 execution/dependency exact contracts

以下 endpoint 已采用各自闭合的 path 与 success response root；有 JSON request 的 endpoint
同时采用闭合 request root：

- `GET|POST /api/v1/tasks/{task_id}/dependencies`
- `DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}`
- `POST /api/v1/tasks/{task_id}/execution-plan/not-required`
- `GET /api/v1/runs/{run_id}/log`
- `GET /api/v1/boards/{board}/columns`

Dependency 三个 success root 只复用 contract-owned `ApiDependencies`、`ApiDependencyTask`、
`ApiDependencyEdge` 和既有 `ApiTask` components；它们仍是 endpoint-specific roots，不是一个
family shortcut。add 的 `201 Created` / idempotent `200 OK`、cycle rejection、edge/event/status
recompute transaction 与 board scope 继续由 SQLite service guard 决定。Mark-plan response 只返回
execution-plan record，不能绕过 required-step 或 task-status guard。

Get-run-log 的 runtime 本来就是 JSON envelope，因此采用
`{ data: { run_id, content, truncated } }` exact root；`log_path` 和 claim token 保持私有，256 KiB
tail window 与 lossy UTF-8 读取语义不变。List-columns 返回完整 column fields，包括 required-nullable
`wip_limit`，column status 仍是 canonical task status enum，不能改变 `tasks.status`。

这些 endpoint 的无 query/body 维度标记 `NotApplicable`。通用 actor、locale 与 content-type
headers 本批保持 `Todo`，由后续统一 transport-closure cohort 处理，因此 endpoint migration
保持 `Generated`。

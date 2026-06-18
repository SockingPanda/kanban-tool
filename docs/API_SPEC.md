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

### 1.3 Success Response

```json
{
  "data": {},
  "meta": {}
}
```

### 1.4 Error Response

```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot claim task from status todo",
    "details": {
      "task_id": "t_...",
      "status": "todo"
    }
  }
}
```

### 1.5 HTTP Status Mapping

| Error code | HTTP status |
|---|---:|
| `invalid_input` | 400 |
| `not_found` | 404 |
| `invalid_transition` | 409 |
| `dependency_blocked` | 409 |
| `claim_conflict` | 409 |
| `claim_token_mismatch` | 403 |
| `db_busy` | 503 |
| `internal` | 500 |

---

## 2. Health

### `GET /health`

Response：

```json
{
  "data": {
    "ok": true,
    "db": "ok",
    "version": "1.3.0",
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
| `q` | title/description 搜索；task ref 形状按精确匹配处理。 |
| `include_archived` | bool。 |
| `limit` | 默认 100。 |
| `offset` | 分页 offset。 |
| `sort` | `seq` / `title` / `status` / `position` / `priority` / `assignee` / `scheduled_at` / `due_at` / `created_at` / `updated_at`，前缀 `-` 表示降序。`priority` sorts P0 -> P3; `-priority` sorts P3 -> P0. |

Priority 只表达相对重要性和排序，不表达可 claim 状态。`ready` 才表示任务已显式进入可执行队列；普通 `ready` task 可以是 P1/P2/P3，不应为了表示“可做”全部标成 P0。P0 只用于 incident、当前目标 blocker 或必须立即处理的任务；P0 task 若仍缺规格、排期未到或依赖未完成，仍不能被 claim。

`q` 对 task ref 形状使用精确匹配而不是文本 contains 匹配：纯数字 `12`、
`#12` 匹配 `{board}` 内的 seq；`board#12` / `board/#12` 只在显式 board
与 `{board}` 相同时匹配；`t_...` 只匹配 `{board}` 内的 task id。其他文本仍执行
title/description 模糊搜索。

Response：

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
    "offset": 0
  }
}
```

### 4.2 Create task

```http
POST /api/v1/boards/{board}/tasks
```

Request：

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
- Task 响应会暴露派生 dependency 字段：`dependency_blocked` 和 `unfinished_parent_count`。它们是查询元数据，不是可写 task 字段。
- `priority` 是整数等级 `0..3`：`0` = P0 incident/blocker/must-handle-immediately，`1` = P1 近期重点，`2` = P2 重要后续，`3` = P3 普通 backlog/低优先级/默认。创建时会拒绝非法值。
- `labels` 可选。名称会先 trim；空白名称会被拒绝；缺失的 board label 会在绑定到 task 前创建。

### 4.3 Get task

```http
GET /api/v1/tasks/{task_id}
```

`task_id` is the global `t_...` id and is not scoped by board. Responses include `board_id`, `board_slug`, and `ref` so clients can render copyable `board#seq` task refs.

Query params：

| Param | 说明 |
|---|---|
| `include` | 可选。当前识别 `ontology`；可用逗号分隔，其他 include 值暂时保持兼容性忽略。 |

默认响应只包含 `data: TaskDto`，不返回 `meta`。传 `include=ontology` 时，`data`
保持同一 `TaskDto`，并在 `meta.details.ontology_summary` 返回该 task 的 label
ontology signal 摘要；没有 ontology signals 时为 `null`。Summary 是只读 task-level
工作流提示，包含 signal/status/degraded/stale/action counts、oldest open/confirmed
signal time/age、latest signal/action time、当前 `suggest_input_hash` 和最多 5 条
sample signals（id/kind/status/proposed_action/score/stale/degraded/action count）。完整
queue/review 仍使用 `/label-ontology/signals`、`/label-ontology/review` 和
`/label-ontology/signals/{signal_id}`。

### 4.4 Update task fields

```http
PATCH /api/v1/tasks/{task_id}
```

允许字段：

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

`max_retries: null` 清空 retry policy。

禁止字段：

- `status`
- `claim_token`
- `current_run_id`
- `completed_at`

`PATCH` 不能直接设置 canonical `status`；状态必须通过 transition endpoint 修改。
不过允许字段仍会走 command service。更新 `description`、`scheduled_at`
等影响 spec 或 schedule 的字段后，服务端可以根据 spec、schedule 和
当前 dependencies 重新计算 active task 的目标状态，并写入对应事件。
Dependency edge 必须通过 dependency endpoints 修改；`max_retries` 只更新
retry policy，不是 status recompute 触发器。

---

## 5. Transitions

### 5.1 Specify

```http
POST /api/v1/tasks/{task_id}/transitions/specify
```

Request：

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

Request：

```json
{
  "actor": "dispatcher"
}
```

### 5.3 Claim / Start

```http
POST /api/v1/tasks/{task_id}/transitions/claim
```

Request：

```json
{
  "actor": "alice",
  "ttl_ms": 300000,
  "worker_profile": null,
  "metadata": {}
}
```

Response：

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

```json
{
  "claim_token": "ct_01HX...",
  "ttl_ms": 300000,
  "note": "still running",
  "actor": "worker-default"
}
```

### 5.5 Complete

```http
POST /api/v1/tasks/{task_id}/transitions/complete
```

Request：

```json
{
  "claim_token": "ct_01HX...",
  "summary": "实现完成，测试通过",
  "result": {},
  "force": false,
  "actor": "worker-default"
}
```

### 5.6 Submit Review

```http
POST /api/v1/tasks/{task_id}/transitions/submit-review
```

Request：

```json
{
  "claim_token": "ct_01HX...",
  "summary": "等待人工检查",
  "actor": "worker-default"
}
```

### 5.7 Block

```http
POST /api/v1/tasks/{task_id}/transitions/block
```

Request：

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

```json
{
  "actor": "alice"
}
```

Response target 由服务端计算，不由客户端指定。

### 5.9 Reclaim

```http
POST /api/v1/tasks/{task_id}/transitions/reclaim
```

Request：

```json
{
  "force": false,
  "to_status": "ready",
  "reason": "claim expired",
  "actor": "dispatcher"
}
```

### 5.10 Archive

```http
POST /api/v1/tasks/{task_id}/transitions/archive
```

Request：

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
`ready`.

### 6.2 Remove dependency

```http
DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}
```

### 6.3 List dependencies

```http
GET /api/v1/tasks/{task_id}/dependencies
```

Response：

```json
{
  "data": {
    "parents": [],
    "children": []
  }
}
```

---

## 7. Comments

### 7.1 List comments

```http
GET /api/v1/tasks/{task_id}/comments
```

Comments are task-id scoped. Listing comments remains available for archived boards because it is read-only audit history; creating comments on archived boards is rejected.

Response：

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
      "metadata_json": "{}",
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

- `kind` 默认为 `note`，当前允许 `note|decision`。
- `decision` records meaningful multi-option choices; body remains the readable fallback, and structured decision metadata is carried by `metadata`.
- `author_type` marks who produced the comment and allows `user|agent`. If omitted, the service defaults to `user`.
- `agent_type` is optional open text for `author_type=agent` comments, such as `executor` or `reviewer`. Non-empty `agent_type` with `author_type=user` is rejected as `400 invalid_input`.
- `metadata` 默认为 `{}`，必须是 JSON object；response 使用 `metadata_json` 字符串保持现有 DTO 风格。`kind=decision` 时必须包含非空 `options`，每个 option 必须有非空 `slug` / `title` / `detail`，slug 必须是唯一小写 ASCII slug，`selected` 必须匹配 option slug，`reason` 必须非空，`risk` / `verification` 如果出现也必须非空。无效 decision metadata 返回 `400 invalid_input`。
- `author` 走通用 actor 语义；也可以用 `X-KB-Actor` 或 server 默认 actor。
- 创建评论会写入 `task.comment.created` event。

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
    ]
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

```json
{
  "data": [
    {
      "id": 123,
      "event_id": "e_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "kind": "task.claimed",
      "actor": "alice",
      "payload": {},
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
data: {"id":124,"event_id":"e_...","task_id":"t_...","kind":"task.claimed","payload":{}}
```

Reconnect：

- V1 implementation emits a finite snapshot of existing matching events and closes; clients should reconnect or poll `GET /api/v1/events` for updates.
- Browser clients may send Last-Event-ID, but V1 only honors the `after` query parameter.
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
DELETE /api/v1/boards/{board}/labels/{label_id}/semantics
GET /api/v1/boards/{board}/labels/atoms
GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain
GET /api/v1/boards/{board}/labels/atom-index/status
POST /api/v1/boards/{board}/labels/atom-index/rebuild
GET /api/v1/boards/{board}/labels/atom-index/query?q=<text>&polarity=positive&limit=24
GET /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels/bootstrap
DELETE /api/v1/tasks/{task_id}/labels/{label_id}
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals
GET /api/v1/boards/{board}/label-ontology/review
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/validate
```

Board 级标签创建请求：

```json
{
  "name": "core",
  "color": "blue"
}
```

Label 响应结构，用于 board 级标签创建和 label 列表：

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
会被拒绝。

Task 标签添加请求：

```json
{
  "name": "core"
}
```

或批量添加：

```json
{
  "names": ["core", "api"]
}
```

`POST /api/v1/tasks/{task_id}/labels` 会把指定 name 或 names 的 label 绑定到 task。
`name` 与 `names` 互斥；二者都缺失、二者同时出现或 `names` 为空数组都会返回
invalid input。批量添加在同一 transaction 内执行，并先验证所有 label 名称；如果
任一 label 为空白或非法，不会创建 canonical label，也不会留下部分 task-label 绑定。
如果该 task 所属 board 上还不存在指定 name 的 label，会先创建 label。重复绑定已有
task-label 关系不会重复写入。成功响应返回更新后的 task，包含当前 `labels` 列表。

Task label bootstrap 请求：

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
在同一 transaction 内创建或复用 task 所属 board 上的 canonical label，写入/覆盖
该 label 的 `label_semantics`，同步重建 SQLite `label_atoms`，标脏派生的 label
atom vector index，并把该 label 绑定到 task。`name` 按 label 名称解析；空白名称会
被拒绝。语义输入会 trim 并丢弃空白值，且必须至少提供 `description` 或一个非空语义
数组值。重复调用同一 task/label 不会重复写 `task_labels`，但会按最新输入 upsert
semantics。成功响应状态为 `201 Created`：

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

```json
{
  "description": "Backend service work",
  "applies_when": ["touches Rust service code"],
  "excludes_when": ["CSS-only"],
  "positive_examples": ["add API handler"],
  "negative_examples": ["adjust spacing"]
}
```

数组字段可缺省为空数组；服务会 trim 并丢弃空白值。生成 atoms 时，有 description
的 label 会生成一个 canonical `description` atom：
`label: {name}\ndescription: {description}`；没有 description 时才使用 `name`
fallback atom。atom text 会进一步规范化 whitespace：每个非空行内部 collapse，
canonical 行分隔保留。同一 label 下相同 `polarity + kind + normalized_text` 的 atom
会去重并保留首次 ordinal，`id` / `content_hash` 不包含 ordinal，因此只调整数组顺序
不会改变同一文本 atom identity。响应使用 Envelope：

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

`DELETE /api/v1/boards/{board}/labels/{label_id}/semantics` 删除该 label 的 semantics
与 SQLite atoms，但不删除 canonical label 或 task-label 绑定。成功返回：

```json
{ "data": { "deleted": true } }
```

`GET /api/v1/boards/{board}/labels/atoms` 返回 SQLite truth 中的 `label_atoms`。
这些 atoms 是 `lancedb_label_atoms` 派生索引的输入。

`GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain` 按当前 atom id 或稳定
`content_hash` 解析 atom，并返回 `LabelAtomExplainRecord`：`query`、`atom`、
`current_semantics`、`provenance_actions`、`supporting_signals`、
`validation_history`、`legacy_untracked` 和 `legacy_reason`。当前 atom 存在但没有
ontology provenance action 引用其 id 或 content hash 时返回 `200` 且
`legacy_untracked=true`；未知 id/hash 返回 not found。

`GET /api/v1/boards/{board}/labels/atom-index/status` 返回 label atom vector index
状态。无 vector provider 或未启用 `vector-lancedb` feature 时仍返回 `200` disabled
状态。JSON 保留兼容字段 `message`，并额外返回结构化
`diagnostics: string[]`、`dirty: boolean | null`、`board_dirty: boolean | null`；
调用方应使用结构化字段判断 dirty/error，而不要解析 `message` 文案。
同一 `VectorStoreStatus` shape 也用于 `/api/v1/vector/status`。

`POST /api/v1/boards/{board}/labels/atom-index/rebuild` 用已配置 vector store 重建
派生的 `lancedb_label_atoms`。`GET /api/v1/boards/{board}/labels/atom-index/query`
查询该派生索引，`q` 必填，`polarity` 可选且只接受 `positive` / `negative`，
`limit` 默认 24；hit 中的 `distance` 是 LanceDB `_distance`，不是 solver
similarity score。未配置 provider、feature 不可用或 vector store 不可用时，rebuild/query
返回显式 API error，不修改 SQLite truth。

### 12.2 Task label suggestions

```http
GET /api/v1/tasks/{task_id}/labels/suggestions?limit=5&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15
```

返回 task-level label suggestions。当前使用 task title + description embedding
查询 `lancedb_label_atoms`：正向 atoms 按 residual 多轮检索，负向 atoms 固定用原始
query 检索并做 penalty / suppression。solver 在 label group 层执行 Group OMP 选择，
再把选中 label 的 top positive atom vectors 作为 basis 做 non-negative refit；
`coverage` / `residual_norm` 来自 atom-level fitted vector；`coverage_cosine`
是原始 query 与 fitted vector 的 cosine similarity。候选 label 只有在
tentative refit 后带来足够 residual norm 降幅才会进入结果；coverage 或
residual norm 达到停止阈值后，solver 会提前停止而不是凑满
`max_selected_labels`。candidate group 与已选 label 语义向量过度相似时会被跳过，
以减少重复语义 label 同时出现在 `selected_labels`；这不会合并或删除 canonical
labels。接口不会创建新 label，也不会写入 `label_semantics` / `label_atoms`。

`limit` 只控制 response 中 `selected_labels` / `candidates` 的最大条数，不会收窄
solver 内部搜索能力。内部能力由 `candidate_limit`、`atom_limit` 和
`max_selected_labels` 分别控制：候选 label group 数、每轮 atom vector 检索上限、
以及最多进入 non-negative refit 的 label 数。所有 limit 参数都必须是
`1..=1000`；`min_score` 必须在 `0..=1`。

未配置 provider、未启用 `vector-lancedb` feature、LanceDB 表缺失、索引为空或索引
dirty 时，接口仍返回 `200` 和结构化 degraded JSON；普通 label CRUD、task
list/search/filter 与状态转移不受影响。Dirty 判断来自结构化 status/SQLite dirty
字段，不依赖 `message` 文案。无 provider 时 `needs_new_label=false`，
避免把 #105 的新 label 创建流程误触发。

Response：

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
  "actor": "alice"
}
```

数组字段缺省时按空数组处理。服务先读取当前 label suggestion 的启发式
`coverage` / `coverage_cosine` / `residual_norm` / top1 existing label。coverage 充足时不写 proposal；
coverage 不足且候选语义有效，并且残差 top1+margin 校验明确通过时，返回 `201` 并持久化
`proposed` proposal。与现有 label 发生 normalized-name 冲突的候选持久化为 `rejected`，diagnostics 包含
`near_duplicate_label_conflict`。Normalized-name conflict 忽略大小写、空白和标点，
是 deterministic near-duplicate heuristic。

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

```json
{
  "reason": "coverage 不足，接受新 label",
  "actor": "alice",
  "source_signal_ids": ["los_..."]
}
```

Accept 只允许 `proposed` proposal。成功后会创建 canonical `labels` 行与对应
`label_semantics` / `label_atoms`，并标脏 label atom index；不会自动写
`task_labels`。`source_signal_ids` 可选；传入时，accept 会在同一 transaction 内写入
`bootstrap_label` ontology action，并通过 action-signal links 记录 new-label
bootstrap provenance。Source signals 必须属于同一 board 且处于 `confirmed`。
Reject 标记为 `rejected`，不接受 `source_signal_ids`。accepted/rejected proposal
再次决策返回普通 `400 invalid_input` error envelope。

### 12.4 Label ontology ledger

Label ontology ledger API 记录 task 标注过程、review queue、ontology mutation
provenance 和 validation history。Ledger 不会自动修改 task labels；canonical
binding 仍通过 task label API 或 CLI 完成。

```http
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals?status=open&kind=false_negative&task=default%2312&label=cli&proposed_label=database&include_all=false&limit=100
GET /api/v1/boards/{board}/label-ontology/review?group_by=label&include_all=false&limit=100
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/validate
```

`POST /api/v1/tasks/{task_id}/label-ontology/observations` 在一个 transaction 中写入
observation 和 child signals。请求 body：

```json
{
  "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
  "agent_candidates_json": "[]",
  "suggestion_snapshot_json": "{}",
  "final_decision_json": "{}",
  "suggest_coverage": 0.61,
  "suggest_coverage_cosine": 0.74,
  "suggest_residual_norm": 0.39,
  "suggest_needs_new_label": false,
  "suggest_degraded": false,
  "diagnostics_json": "[]",
  "capture_fingerprint": "optional-stable-key",
  "signals": [
    {
      "kind": "false_negative",
      "target_label_ref": "cli",
      "related_labels_json": "[]",
      "proposed_action": "add_positive_atom",
      "candidate_atom": {
        "polarity": "positive",
        "kind": "applies_when",
        "text": "extends CLI subcommands, command arguments, help output, or machine-readable JSON behavior"
      },
      "proposal_json": "{}",
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
`related_labels_json`。Observation metric `suggest_coverage`、
`suggest_coverage_cosine`、`suggest_residual_norm` 以及 signal metric
`suggest_score` / `confidence` 必须是 finite `0.0..=1.0`；`suggest_rank` 必须为
`null` 或 `>= 1`。违反这些契约的 request 返回 `400 invalid_input`，不会写入
observation 或 signals。

`GET /api/v1/boards/{board}/label-ontology/signals` 默认只返回 `open` 和
`confirmed`。可重复传 `status` 和 `kind`，并按 `task`、`label`、
`proposed_label`、`include_all`、`limit` 过滤。

`GET /api/v1/boards/{board}/label-ontology/review` 返回只读聚合 review queue。
`group_by` 支持 `label`、`candidate-atom` / `candidate_atom`、`proposed-label` /
`proposed_label`，默认 `label`；`include_all=false` 默认只聚合 `open` 和
`confirmed` signals，`true` 时包含完整历史；`limit` 限制 group 数量。响应
`meta` 回显 `group_by`、`include_all` 和 `limit`。每个 group 包含：

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
`latest_signal_at` desc, and `key` asc。`GET /api/v1/label-ontology/signals/{signal_id}`
返回：

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
  "change_json": null,
  "validation_status": null,
  "validation_json": null
}
```

该公共 action endpoint 只接受 lifecycle action types：`confirm`、`reject`、
`supersede` 和 `resolve_no_change`，并会同步更新 source signal status。请求中的
`parent_action_id`、`target_label_ref`、result 字段、canonical hash、`change_json`、
`validation_status` 和 `validation_json` 必须为 `null`/缺省；否则返回
`invalid_input`。`add_positive_atom`、`add_negative_atom`、`bootstrap_label`、
`validate` 等 mutation/validation action types 不允许通过该 generic endpoint 写入；
canonical mutation provenance 必须由 apply/proposal accept/validate 等专用 route 在
同一 transaction 内写入。`supersede` 写入时会沿 replacement
`superseded_by_signal_id` 链检查，若链路回到任一 source signal 或 replacement chain
自身已有环，则返回 `invalid_input`，不会写入新的 supersede action。

`POST /api/v1/boards/{board}/label-ontology/apply/atom` 对已有 label 执行
read-modify-upsert semantics，并写入 atom provenance action：

```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "signal_ids": ["los_1", "los_2"],
  "label_ref": "cli",
  "kind": "applies_when",
  "text": "extends CLI subcommands, command arguments, help output, or machine-readable JSON behavior",
  "reason": "Repeated false-negative signal across CLI surface tasks"
}
```

Source signals 必须属于同一 board 且已 `confirmed`。`kind` 只接受
`applies_when`、`positive_example`、`excludes_when`、`negative_example`。成功后返回
`add_positive_atom` 或 `add_negative_atom` action，记录 result atom soft reference、
content hash、before/after canonical hash 和 diff，并把 validation status 置为
`pending`。该 route 会标脏 label atom index；vector rebuild 和 suggest validation
在 transaction 外执行。

`POST /api/v1/boards/{board}/label-ontology/validate` 追加 validation action：

```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "parent_action_id": "loa_...",
  "signal_ids": ["los_1", "los_2"],
  "reason": "Source tasks now select the target label after atom rebuild",
  "validation_status": "passed",
  "validation_json": "{\"evidence_type\":\"automated\",\"embedding_model\":\"local-embeddings-v1\",\"solver_options\":{\"candidate_limit\":24,\"atom_limit\":64},\"index\":{\"status\":\"ready\",\"dirty\":false,\"generation\":42},\"cases\":[{\"signal_id\":\"los_1\",\"case_type\":\"positive_atom\",\"passed\":true,\"before\":{\"target\":{\"label_id\":\"l_cli\",\"selected\":false,\"score\":0.12},\"coverage\":0.61},\"after\":{\"degraded\":false,\"target\":{\"label_id\":\"l_cli\",\"selected\":true,\"score\":0.72},\"coverage\":0.78,\"evidence_atoms\":[{\"id\":\"la_...\",\"content_hash\":\"...\",\"label_id\":\"l_cli\"}]}}]}"
}
```

Service 会把 supplied `validation_json` 包进 validation envelope，附上 source signal
cases、observation task snapshot / suggest input hash 与当前 task hash 对比、parent
action result 引用和 summary。`parent_action_id` 必须指向同一 board 上 `validation_status=pending`
的 canonical mutation action，且 parent action 必须带有 canonical result evidence
（例如 atom/result label/proposal 引用、canonical hash 和非空 change snapshot）。
`passed` 还必须提供 automated typed evidence：top-level `evidence_type="automated"`、
非空 `embedding_model`、object `solver_options`、`index.status` / `index.generation`
和覆盖每个 linked source signal 的 `cases[]`；dirty/error atom index、
空 `{}`、reviewer attestation 或无类型 evidence 会返回 `invalid_input`。Service 不在
长 SQLite mutation transaction 内执行 embedding/index 查询；调用方在 transaction 外收集
before/after suggest evidence，service 在短 transaction 中核验 parent action、index
状态和 evidence contract 后写 validation action。

Typed policy 按 parent action 检查：

- `add_positive_atom`：`case_type="positive_atom"`，`after.degraded=false`，
  `after.evidence_atoms[]` 必须包含 parent `result_atom_id` 或
  `result_atom_content_hash`；target label 必须 selected 或 score >= 0.50；
  score/coverage 不能比 before 恶化。
- `add_negative_atom`：`case_type="negative_atom"`，`after.evidence_atoms[]`
  必须包含 parent result atom；false-positive task 上 target label score 必须下降或
  不再 selected；若提供 `after.positive_controls[]`，每个 control 必须 passed 且未 regressed。
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

默认后端是 SQLite fallback。二进制启用 `tantivy-backend` 且 SQLite DB 旁存在 `index/v1/tasks/` 时，search 使用 Tantivy task index。Tantivy index 缺失、损坏或过期时会回落到 SQLite，并带上 stale metadata。搜索匹配 task title、description、comments、run summary/error 和 event kind/payload。

`label` 按 label 名称或 id 过滤，可重复，并在评分和分页前使用 AND 语义。
带 label 过滤的 search 即使存在可用 Tantivy index，也会使用 SQLite fallback，
以确保结果反映当前 task-label 关联行。

Task ref 形状的 `q` 始终使用 SQLite 精确匹配语义，即使当前存在可用 Tantivy index：
纯数字 `12` 和 `#12` 匹配请求 `board` 内的 seq；`board#12` 和 `board/#12`
只在显式 board 等于请求 board 时匹配；`t_...` 只匹配请求 board 内的 task id。
Ref 形状 query 不会从 title、description、comments、runs 或 events 中返回模糊匹配。

Response:

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

### 13.2 Search status

```http
GET /api/v1/search/status?board=default
```

Response:

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

Response includes SQLite integrity, migration/user version, expired running tasks, orphan run checks, dependency cycle count, archived dependency edge count, missing and suspicious run log counts, executable status invariant counts for dependency/spec/schedule violations, label ontology ledger diagnostics, and Knowledge Substrate diagnostics. Archived parent -> active child edges are allowed historical dependency edges; archived child edges from active parents are counted.

Ontology ledger diagnostics are read-only:

- `ontology_ledger_errors` / `ontology_ledger_warnings` summarize hard errors and warnings.
- `ontology_ledger_issues[]` reports structured findings with `severity`, `code`, `message`, and `record_ids`.
- v12+ databases require `label_ontology_observations`, `label_ontology_signals`, `label_ontology_actions`, and `label_ontology_action_signals`.
- Hard errors include cross-board ontology links, orphan action-signal links, missing parent/supersede references, label/proposal/task board mismatches, signal supersede cycles, and action parent cycles. Nonzero errors make `ok=false`.
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

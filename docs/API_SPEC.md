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
    "version": "1.1.3",
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
| `label` | 按 label。 |
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
- Task responses expose derived dependency fields: `dependency_blocked` and `unfinished_parent_count`. They are query metadata and are not writable task fields.
- `priority` is an integer level `0..3`: `0` = P0 incident/blocker/must-handle-immediately, `1` = P1 near-term focus, `2` = P2 important follow-up, `3` = P3 ordinary backlog/low/default. Create rejects invalid values.

### 4.3 Get task

```http
GET /api/v1/tasks/{task_id}
```

`task_id` is the global `t_...` id and is not scoped by board. Responses include `board_id`, `board_slug`, and `ref` so clients can render copyable `board#seq` task refs.

Query params：

| Param | 说明 |
|---|---|
| `include` | `comments,runs,events,dependencies,labels`。 |

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
      "author_type": "human",
      "agent_type": null,
      "body": "这里需要确认边界条件。",
      "kind": "text",
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
  "kind": "text",
  "author_type": "human",
  "agent_type": null,
  "author": "alice"
}
```

Notes：

- `kind` 默认为 `text`，当前允许 `text|system|worker|decision`。
- `decision` records meaningful multi-option choices. Use this stable Markdown body shape: `Problem: ...`, `Options: ...`, `Choice: ...`, `Reason: ...`, `Risk/validation: ...`. Skip it for trivial naming, formatting, or purely mechanical choices.
- `author_type` marks who produced the comment and allows `human|agent|system`. If omitted, the service infers `worker -> agent`, `system -> system`, and all other kinds as `human`; `decision` therefore defaults to `human`.
- `agent_type` is optional open text for `author_type=agent` comments, such as `executor` or `reviewer`. Non-empty `agent_type` with `author_type=human` or `system` is rejected as `400 invalid_input`.
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

## 12. Labels

```http
GET /api/v1/boards/{board}/labels
POST /api/v1/boards/{board}/labels
POST /api/v1/tasks/{task_id}/labels
DELETE /api/v1/tasks/{task_id}/labels/{label_id}
```

---

## 13. Search

### 13.1 Search tasks

```http
GET /api/v1/search/tasks?board=default&q=needle&status=ready&assignee=worker-a&include_archived=false&limit=20&offset=0
```

Default backend is SQLite fallback. When the binary is built with `tantivy-backend` and `index/v1/tasks/` exists beside the SQLite DB, search uses the Tantivy task index. Missing, corrupt, or stale Tantivy indexes fall back to SQLite with stale metadata. Search matches task title, description, comments, run summary/error, and event kind/payload.

Task ref-shaped `q` values always use SQLite exact-match semantics, even when a
usable Tantivy index exists: pure numeric `12` and `#12` match seq within the
requested `board`; `board#12` and `board/#12` match only when the qualified board
is the requested board; `t_...` matches only a task id on the requested board.
Ref-shaped queries do not return fuzzy matches from title, description, comments,
runs, or events.

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

Response includes SQLite integrity, migration/user version, expired running tasks, orphan run checks, dependency cycle count, archived dependency edge count, missing and suspicious run log counts, executable status invariant counts for dependency/spec/schedule violations, and Knowledge Substrate diagnostics. Archived parent -> active child edges are allowed historical dependency edges; archived child edges from active parents are counted.

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

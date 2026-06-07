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
    "version": "0.1.0"
  }
}
```

---

## 3. Boards

### 3.1 List boards

```http
GET /api/v1/boards
```

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

### 3.3 Get board

```http
GET /api/v1/boards/{board_slug_or_id}
```

### 3.4 Archive board

```http
POST /api/v1/boards/{board}/archive
```

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
| `assignee` | 按 assignee。 |
| `label` | 按 label。 |
| `q` | title/description 搜索。 |
| `include_archived` | bool。 |
| `limit` | 默认 100。 |
| `offset` | 分页 offset。 |
| `sort` | `position` / `priority` / `created_at` / `updated_at`。 |

Response：

```json
{
  "data": [
    {
      "id": "t_01HX...",
      "seq": 12,
      "board_id": "b_01HX...",
      "title": "实现状态机",
      "description": "...",
      "status": "ready",
      "priority": 10,
      "position": 1024,
      "assignee": null,
      "scheduled_at": null,
      "due_at": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000
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
  "priority": 10,
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
- 若存在未完成 dependencies，不能创建为 `ready`。

### 4.3 Get task

```http
GET /api/v1/tasks/{task_id}
```

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
  "priority": 20,
  "scheduled_at": 1717520000000,
  "due_at": 1717600000000,
  "max_retries": 2,
  "metadata": {},
  "actor": "alice",
  "expected_lock_version": 7
}
```

`max_retries: null` 清空 retry policy。

禁止字段：

- `status`
- `claim_token`
- `current_run_id`
- `completed_at`

状态必须通过 transition endpoint 修改。

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

Response：

```json
{
  "data": [
    {
      "id": "c_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "author": "alice",
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
  "author": "alice"
}
```

Notes：

- `kind` 默认为 `text`，当前允许 `text|system|worker`。
- `author` 走通用 actor 语义；也可以用 `X-KB-Actor` 或 server 默认 actor。
- 创建评论会写入 `task.comment.created` event。

---

## 8. Runs

### 8.1 List task runs

```http
GET /api/v1/tasks/{task_id}/runs
```

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
- 当前最多返回 256 KiB；更大的 log 会设置 `truncated: true`。
- 若 run 没有 `log_path` 或文件不存在，返回 `not_found`。

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

## 13. Maintenance

### 13.1 Doctor

```http
POST /api/v1/maintenance/doctor
```

Response includes SQLite integrity, migration/user version, expired running tasks, orphan run checks, dependency cycle count, archived dependency edge count, missing run log count, and executable status invariant counts for dependency/spec/schedule violations.

### 13.2 Checkpoint

```http
POST /api/v1/maintenance/checkpoint
```

Runs `PRAGMA wal_checkpoint(TRUNCATE)` and returns `busy`, `log_frames`, and `checkpointed_frames`.

### 13.3 Backup

MVP 建议只提供 CLI backup，不开放 HTTP backup。

---

## 14. Web UI Interaction Rules

1. 拖拽列时调用 transition endpoint。
2. 普通字段编辑调用 `PATCH /tasks/{id}`。
3. Web UI 不显示 claim_token，除非 debug 模式。
4. running task 的 complete/block 操作，若无 token，则 UI 走 `force=true` 并要求确认。
5. blocked task unblock 后目标列由服务端返回，前端不要预设。
6. SSE 收到 event 后，优先 refetch affected task，避免客户端状态机漂移。

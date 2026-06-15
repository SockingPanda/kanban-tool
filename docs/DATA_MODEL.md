# Data Model

本文件定义领域模型、SQLite 表、ID、时间、JSON、附件、事件与常用查询。

---

## 1. ID 规范

所有 public ID 使用带前缀的 ULID/UUID-like string，便于日志和 CLI 区分。

| 对象 | 前缀 | 示例 |
|---|---|---|
| Board | `b_` | `b_01HY...` |
| Task | `t_` | `t_01HY...` |
| Run | `r_` | `r_01HY...` |
| Comment | `c_` | `c_01HY...` |
| Attachment | `a_` | `a_01HY...` |
| Label | `l_` | `l_01HY...` |
| Column | `col_` | `col_ready` |
| Event | `e_` | `e_01HY...` |

`task_events.id` 同时保留自增 integer，用于 SSE offset 和顺序分页。

---

## 2. 时间规范

所有时间字段使用：

```text
INTEGER unix epoch milliseconds UTC
```

字段命名：

- `created_at`
- `updated_at`
- `scheduled_at`
- `started_at`
- `completed_at`
- `archived_at`
- `claim_expires_at`
- `last_heartbeat_at`

Rust 内部建议使用 `time::OffsetDateTime`，DB 边界转换为 `i64` milliseconds。

---

## 3. JSON 字段规范

SQLite 中 JSON 存 `TEXT`，必须满足：

```sql
CHECK(json_valid(field_name))
```

默认值：

```json
{}
```

用途：

| 字段 | 说明 |
|---|---|
| `tasks.metadata_json` | 轻量扩展信息。 |
| `task_runs.metadata_json` | worker profile、环境、命令摘要等。 |
| `task_events.payload_json` | event payload。 |

禁止把大对象、stdout/stderr 全量日志、附件 blob 放进 JSON。

---

## 4. Board

Board 是本地 project/board，不是 tenant。

主要字段：

| 字段 | 说明 |
|---|---|
| `id` | `b_` prefixed ID。 |
| `slug` | CLI/Web 使用的人类可读短名。 |
| `name` | 展示名。 |
| `description` | 可选说明。 |
| `created_at` | 创建时间。 |
| `updated_at` | 更新时间。 |
| `archived_at` | 归档时间。 |

默认 board：

```text
default
```

Board slug 由 service 层校验：必须唯一、非空、不超过 64 bytes，以小写 ASCII 字母或数字开头，只能包含小写 ASCII 字母、数字、`.`、`_`、`-`，并且不能使用 `b_`、`t_`、`r_`、`c_`、`a_`、`l_`、`col_`、`e_` 等保留前缀。这样可以避免和 public ID、`board#seq` task ref、路径式 alias 语法冲突。

Archived board 默认不出现在 board list，也不接受普通 task/comment/dispatcher 写入。归档只设置 board 的 `archived_at` 并写入 `board.archived` event，不改变 task 状态；如果 board 上仍有 `running` task 或 `running` run，归档会被拒绝。Events、runs、comments 等只读历史仍可通过显式 task/board identity 查询，用于审计。

---

## 5. Task

Task 是核心对象，既是看板卡片，也是可执行工作单元。

### 5.1 字段分组

#### Identity

| 字段 | 说明 |
|---|---|
| `id` | Task ID。 |
| `board_id` | 所属 board。 |
| `seq` | board 内递增数字，便于显示 `board#12`。 |

Task public identity 有两层：

- `id` 是全局唯一 `t_...`，可跨 board 直接定位 task。
- `seq` 只在同一 board 内唯一，CLI/API 展示时应组合成 `board_slug#seq`，例如 `agent-work#12`。

#### Content

| 字段 | 说明 |
|---|---|
| `title` | 必填。 |
| `description` | Markdown 文本。 |
| `status_reason` | block 等状态原因。 |
| `result_summary` | 完成摘要。 |
| `metadata_json` | 扩展字段。 |

#### Workflow

| 字段 | 说明 |
|---|---|
| `status` | canonical status。 |
| `priority` | integer enum-like priority level: `0` = P0 highest, `1` = P1, `2` = P2, `3` = P3 lowest/default. DB default is `3` and values are constrained with `CHECK(priority BETWEEN 0 AND 3)` after migrations. Create/update commands reject values outside P0-P3. |
| `position` | UI 排序键。 |
| `scheduled_at` | 计划时间。 |
| `due_at` | 截止时间，仅展示/过滤，不驱动状态机。 |
| `retry_count` | 已 retry 次数。 |
| `max_retries` | 最大 retry 次数。 |

#### Actor / Execution

| 字段 | 说明 |
|---|---|
| `assignee` | 人或 worker profile 名称。 |
| `created_by` | actor string。 |
| `claim_token` | active claim token。 |
| `claim_owner` | active claim actor。 |
| `claim_expires_at` | claim 过期时间。 |
| `last_heartbeat_at` | heartbeat 时间。 |
| `current_run_id` | active/latest run id。 |

#### Timestamps

| 字段 | 说明 |
|---|---|
| `created_at` | 创建。 |
| `updated_at` | 更新。 |
| `started_at` | 首次进入 running。 |
| `completed_at` | 完成。 |
| `archived_at` | 归档。 |

#### Concurrency

| 字段 | 说明 |
|---|---|
| `lock_version` | optimistic lock。 |

### 5.2 Priority 语义

`priority` 表示任务的相对重要性和排序权重，不表示状态机可执行性。`ready`
表示任务已经被人工或服务显式放入可 claim 队列；P0-P3 只影响列表、DAG/frontier
和 dispatcher 在可选任务之间的排序。

优先级约定：

| Priority | 语义 | 示例 |
|---|---|---|
| `0` / P0 | incident、阻断当前目标、必须立即处理的任务。少量使用，不作为普通 ready 默认值。 | 修复导致本地队列无法 claim 的回归；解除发布前 P1/P0 reviewer blocker。 |
| `1` / P1 | 近期待办焦点，当前迭代或当前工作流应优先完成。 | 今天要完成的实现切片；当前 PR 必须补齐的测试。 |
| `2` / P2 | 重要 follow-up，但不阻塞当前主线。 | 整理文档示例；补充非关键 smoke。 |
| `3` / P3 | 普通 backlog、低优先级或默认值。 | 想法、低风险清理、未来可做的体验改进。 |

`ready` 与 P0 不能互相替代：

- 普通可执行任务应是 `ready` + P1/P2/P3，而不是为了进入队列全部标成 P0。
- P0 任务如果仍缺规格、排期未到或依赖未完成，仍不能被 claim；它应保持
  `triage`、`scheduled` 或 `todo`，直到满足状态机 guard 后再 promote 到 `ready`。
- Dispatcher 只 claim `ready` 任务；在多个 `ready` 任务之间，才按 priority 从
  P0 到 P3 排序。

---

## 6. Dependency

表：`task_dependencies`

字段：

| 字段 | 说明 |
|---|---|
| `parent_task_id` | 前置任务。 |
| `child_task_id` | 被阻塞任务。 |
| `created_at` | 创建时间。 |

语义：

```text
parent done or archived => child may become ready
parent neither done nor archived => child cannot be ready/running
```

添加依赖时必须做环检测。归档 parent 会满足 hard dependency guard，但 dependency edge 保留为历史，不会自动 promote child。

---

## 7. Run

表：`task_runs`

Run 是一次 execution attempt。

### 7.1 Run status

```text
running | succeeded | failed | canceled | expired
```

### 7.2 字段

| 字段 | 说明 |
|---|---|
| `id` | `r_` prefixed ID。 |
| `task_id` | 关联 task。 |
| `status` | run 状态。 |
| `worker_profile` | worker profile 名。 |
| `worker_pid` | 本机 PID。 |
| `claim_token` | 对应 claim。 |
| `started_at` | run 开始。 |
| `last_heartbeat_at` | 最近 heartbeat。 |
| `finished_at` | run 结束。 |
| `exit_code` | worker 退出码。 |
| `summary` | 简短摘要。 |
| `error` | 错误文本。 |
| `log_path` | stdout/stderr 日志路径。 |
| `metadata_json` | 执行元数据。 |

### 7.3 约束

- active `running` task 必须有 active run。
- 一个 task 可以有多个历史 run。
- 同一 task 同时最多一个 running run。

SQLite 不强制最后一条，需要 service 层和 transaction 保证。

---

## 8. Event

表：`task_events`

Event 是 append-only 事实记录。

### 8.1 Event kind

建议初始 kind：

```text
board.created
board.updated
board.archived

task.created
task.updated
task.specified
task.promoted
task.claimed
task.heartbeat
task.completed
task.submitted_for_review
task.blocked
task.unblocked
task.reclaimed
task.archived
task.restored
task.deleted

dependency.added
dependency.removed
task.comment.created
attachment.added
attachment.removed
run.started
run.finished
```

### 8.2 Payload 示例

```json
{
  "from_status": "ready",
  "to_status": "running",
  "claim_owner": "alice",
  "claim_ttl_ms": 300000
}
```

### 8.3 使用场景

- Task detail timeline。
- SSE event stream。
- Debug dispatcher。
- CLI `kanban events`。
- 未来 export/import。

---

## 9. Comment

表：`task_comments`

字段：

| 字段 | 说明 |
|---|---|
| `id` | Comment ID。 |
| `task_id` | 关联 task。 |
| `author` | actor string。 |
| `author_type` | `user` / `agent`，表示评论作者身份；本地操作者是 `user`，其它自动化来源是 `agent`。 |
| `agent_type` | 可选 open text，仅用于 `author_type=agent`，例如 `executor` / `reviewer`。 |
| `body` | Markdown 文本。 |
| `kind` | `note` / `decision`，表示 comment 内容语义，不表示作者身份。 |
| `metadata_json` | `kind` 对应的结构化 payload；默认 `{}`，必须是合法 JSON object。`kind=decision` 时必须符合 decision schema。 |
| `created_at` | 创建时间。 |

旧 comment rows / JSONL import 会迁移到新语义：旧 `human` 变为 `user`，旧 `agent/system` 或 `worker/system` 来源变为 `agent`，旧 `text/system/worker` 内容变为 `note`。没有结构化 metadata 的旧 `decision` 也按 `note` 保留 body fallback。

Comment 创建时也写一条 `task_events(kind='task.comment.created')`。

Decision comment metadata schema：

- `options`：非空 array。
- 每个 option 是 object，且包含非空 string `slug`、`title`、`detail`。
- `slug` 必须是稳定小写 ASCII slug：以小写字母或数字开头，只包含小写字母、数字和 `-`；同一 decision 内唯一。
- `selected`：非空 string，必须匹配某个 option slug。
- `reason`：非空 string。
- `risk` / `verification`：可选；如果出现，必须是非空 string。
- 未知顶层字段允许保留，但不参与状态机、dispatcher 或 event 语义。

---

## 10. Attachment

Blob 不存 DB。

默认路径：

```text
~/.local/share/kb/attachments/<board_id>/<task_id>/<attachment_id>/<filename>
```

DB 存：

| 字段 | 说明 |
|---|---|
| `id` | Attachment ID。 |
| `task_id` | 关联 task。 |
| `filename` | 原始文件名。 |
| `rel_path` | 相对 data dir 的路径。 |
| `content_type` | MIME。 |
| `size_bytes` | 大小。 |
| `sha256` | 内容 hash。 |
| `created_by` | actor。 |
| `created_at` | 上传时间。 |

安全要求：

- `filename` 必须 sanitize。
- `rel_path` 必须在 data dir 内。
- 不允许 `../` path traversal。

---

## 11. Label

Label 是轻量分类。

字段：

| 字段 | 说明 |
|---|---|
| `id` | Label ID。 |
| `board_id` | 所属 board。 |
| `name` | 标签名。 |
| `color` | UI color token。 |
| `created_at` | 创建时间。 |
| `updated_at` | 更新时间。 |

同一 board 内 label name 唯一。

Task 与 label 的关联通过 `task_labels(task_id, label_id)` join table 表达。
Label 只用于分类、过滤和展示；添加或移除 label 不改变 `tasks.status`，
不触发 dependency recompute，也不会让 dispatcher claim `review` 或其他非
`ready` 状态。

---

## 12. Column

Column 是 UI 展示层。

字段：

| 字段 | 说明 |
|---|---|
| `id` | Column ID。 |
| `board_id` | 所属 board。 |
| `status` | 映射的 canonical status。 |
| `title` | UI 名称。 |
| `position` | UI 排序。 |
| `hidden` | 是否隐藏。 |
| `wip_limit` | 可选 WIP limit。 |

MVP：一个 status 对应一个 column。

---

## 13. Knowledge Substrate

Knowledge Substrate 表只支持实体身份、关系镜像、派生 outbox 和派生 store 健康状态。SQLite task/run/comment/event 仍是 operational source of truth。

### 13.1 Entity registry

表：`entities`

字段：

| 字段 | 说明 |
|---|---|
| `uri` | 稳定 `kb://...` entity URI。 |
| `kind` | `task` / `run` / `comment` / `artifact` / `skill` / `project`。 |
| `source_table` | 来源 SQLite 表。 |
| `source_id` | 来源 row id。 |
| `board_id` | 可选 board scope。 |
| `task_id` | 可选 task scope。 |
| `title` | 展示标题。 |
| `summary` | 简短摘要。 |
| `content_hash` | 内容 hash，用于派生层判断变化。 |
| `created_at` / `updated_at` / `archived_at` | 生命周期时间。 |

### 13.2 Relation graph mirror

表：`relation_predicates`、`entity_relations`

`relation_predicates` 定义受控 predicate；`entity_relations` 存可重建关系镜像。关系层用于 graph/context 查询，不改变 task 状态机。状态机仍以 `tasks.status`、`task_dependencies` 和 service transaction 为准。

### 13.3 Index outbox

表：`index_outbox`

字段：

| 字段 | 说明 |
|---|---|
| `id` | 自增 job id。 |
| `source_event_id` | 来源 `task_events.id`，允许事件被删除/导入时置空。 |
| `target` | `tantivy` / `oxigraph` / `lancedb` / `all`。 |
| `entity_uri` | 目标 entity。 |
| `action` | `upsert` / `delete` / `rebuild`。 |
| `payload_json` | 有界 job payload。 |
| `status` | `pending` / `running` / `done` / `failed`。 |
| `attempts` | 尝试次数。 |
| `last_error` | 最近失败原因。 |
| `created_at` / `updated_at` | job 时间。 |

`index_outbox` 是 at-least-once 派生 job surface。task mutation transaction 只写 SQLite truth、event、entity/outbox 记录，不直接写 Tantivy/Oxigraph/LanceDB。

### 13.4 Derived store state

表：`derived_store_state`

字段：

| 字段 | 说明 |
|---|---|
| `store_name` | 派生 store 名称，例如 `tantivy_tasks`、`oxigraph_relations`、`lancedb_chunks`。 |
| `schema_version` | store schema/contract 版本。 |
| `last_event_id` | store 已成功提交的全局 `task_events.id` 水位。 |
| `dirty` | 是否仍有未完成 outbox、失败 outbox 或最近一次 store 更新失败。 |
| `last_rebuild_at` | 最近成功 rebuild 时间。 |
| `last_sync_at` | 最近成功 sync 时间。 |
| `last_error` | 最近失败证据。 |
| `updated_at` | 状态更新时间。 |

`last_event_id` 是 store 全局成功处理水位，不是 board 局部水位。成功 sync/rebuild 只能单调推进这个值；当一个 board sync 完成但其他 board 仍有 pending/running/failed outbox 时，`dirty` 必须保持 true。`dirty=false` 只表示同一 store target 当前没有 unfinished outbox 且最近一次 store 更新没有失败。

`last_error` 成功后清空，失败时保留错误证据并保持 `dirty=true`。Operator 应通过 `kanban derived status`、`kanban doctor`、maintenance API 和对应 `sync/rebuild` 命令恢复派生层；派生 store 损坏或落后不改变 SQLite task truth。

## 14. 常用查询

### 14.1 Board task list

```sql
SELECT *
FROM tasks
WHERE board_id = ?
  AND status != 'archived'
ORDER BY
  CASE status
    WHEN 'triage' THEN 10
    WHEN 'todo' THEN 20
    WHEN 'scheduled' THEN 30
    WHEN 'ready' THEN 40
    WHEN 'running' THEN 50
    WHEN 'blocked' THEN 60
    WHEN 'review' THEN 70
    WHEN 'done' THEN 80
    ELSE 90
  END,
  position ASC,
  priority ASC,
  created_at ASC;
```

### 14.2 Ready queue

```sql
SELECT *
FROM tasks t
WHERE t.board_id = ?
  AND t.status = 'ready'
  AND t.claim_token IS NULL
  AND NOT EXISTS (
    SELECT 1
    FROM task_dependencies d
    JOIN tasks p ON p.id = d.parent_task_id
    WHERE d.child_task_id = t.id
      AND p.status NOT IN ('done','archived')
  )
ORDER BY t.priority ASC, t.created_at ASC
LIMIT ?;
```

### 14.3 Expired claims

```sql
SELECT *
FROM tasks
WHERE status = 'running'
  AND claim_expires_at IS NOT NULL
  AND claim_expires_at <= ?;
```

### 14.4 Event stream

```sql
SELECT *
FROM task_events
WHERE board_id = ?
  AND id > ?
ORDER BY id ASC
LIMIT ?;
```

---

## 15. Export Format

建议支持 JSONL export：

```bash
kanban export --board default --format jsonl > board.jsonl
```

每行：

```json
{"type":"task","data":{...}}
{"type":"event","data":{...}}
{"type":"comment","data":{...}}
{"type":"dependency","data":{...}}
```

MVP 可先只 export，不做 import。

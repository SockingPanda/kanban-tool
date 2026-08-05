# Canonical 数据模型

本文件只描述当前 Turso 单 Host 的 canonical schema。权威来源是
`crates/kanban-store-turso/src/schema.rs`；应用服务负责领域校验，数据库负责
外键、唯一性、`CHECK` 和事务约束。SQLite 旧表、导入导出格式、标签/信号、全文、
图、向量和 projection 不属于当前数据模型。

当前 baseline 包含 `schema_migrations` 和 9 张业务表：`boards`、`board_columns`、
`tasks`、`task_execution_plans`、`task_steps`、`task_dependencies`、`task_runs`、
`task_comments`、`task_events`。

## 1. ID、时间和 JSON

实体 ID 由 ULID 组成并带固定前缀：

| 实体 | 前缀 |
|---|---|
| board | `b_` |
| task | `t_` |
| step | `step_` |
| run | `r_` |
| comment | `c_` |
| event | `e_` |
| column | `col_` |

`task_events.id` 是 `INTEGER PRIMARY KEY AUTOINCREMENT` 自增游标；`event_id` 是公开的 `e_...`
身份。领取 token 是临时凭证，不是实体 ID，通常为 `claim_...`。

时间列统一为 `INTEGER`，含义是 UTC Unix epoch milliseconds（Rust 边界使用
`i64`）。`created_at`、`updated_at`、`scheduled_at`、`due_at`、`started_at`、
`completed_at`、`archived_at`、`claim_expires_at`、`last_heartbeat_at`、
`finished_at`、`resolved_at` 都遵循此格式。

JSON 存为 `TEXT`，相关列由 `CHECK(json_valid(...))` 保护：

- `tasks.metadata_json`、`task_runs.metadata_json` 默认 `{}`；
- `tasks.result_json` 可空，但非空时必须是合法 JSON；
- `task_comments.metadata_json` 默认 `{}` 且必须是 JSON object；
- `task_events.payload_json` 默认 `{}`，允许任意合法 JSON（未知事件载荷保持无损）。

## 2. Schema migration 和看板

### `schema_migrations`

`version INTEGER PRIMARY KEY`、`name TEXT NOT NULL`、`checksum TEXT NOT NULL DEFAULT ''`、
`applied_at INTEGER NOT NULL`。启动时在 immediate transaction 中执行 embedded
canonical SQL，并以 `INSERT OR IGNORE` 写入当前版本，因此初始化可重复执行。

### `boards`

字段为 `id`、`slug`、`name`、`description`、`created_at`、`updated_at`、`archived_at`。
`id` 必须匹配 `b_%`，`slug` 唯一且非空，`name` 非空。默认 seed 是
`(id=b_default, slug=default, name=Default)`。

### `board_columns`

字段为 `id`、`board_id`、`status`、`title`、`position`、`hidden`、`wip_limit`、
`created_at`、`updated_at`。`board_id` 外键指向 `boards`；`status` 只能是
`triage|todo|scheduled|ready|running|blocked|review|done|archived`；`hidden` 只能为
`0/1`，`wip_limit` 为空或非负。`UNIQUE(board_id,status)` 和
`UNIQUE(board_id,position)` 保证一个看板内列唯一。

首次初始化固定 seed 九列，位置为 10 到 90（步长 10）：

```text
triage / todo / scheduled / ready / running / blocked / review / done / archived
```

`archived` 列默认 `hidden=1`，其余列可见。seed column ID 为
`col_<board-id 去掉 b_ 前缀>_<status>`，例如 `col_default_ready`。

## 3. 任务和执行计划

### `tasks`

字段按职责分组如下：

- 身份：`id`、`board_id`、`seq`、`idempotency_key`；
- 内容：`title`、`description`、`status_reason`、`result_summary`、`result_json`、
  `metadata_json`；
- 看板排序：`priority`、`position`、`scheduled_at`、`due_at`；
- 操作者：`created_by`、`assignee`；
- 领取和运行：`claim_token`、`claim_owner`、`claim_expires_at`、
  `last_heartbeat_at`、`current_run_id`；
- 生命周期：`created_at`、`updated_at`、`started_at`、`completed_at`、`archived_at`；
- 重试和并发：`retry_count`、`max_retries`、`lock_version`。

约束：

- `id` 匹配 `t_%`，`title` 非空；`board_id` 外键级联删除；
- `status` 只能是九个 canonical 状态；`priority` 为 0 到 3，`retry_count` 和
  `max_retries`（非空时）非负，`lock_version` 非负；
- `UNIQUE(board_id,id)`、`UNIQUE(id,board_id)` 和 `UNIQUE(board_id,seq)`；
- `running` 任务必须同时有 `claim_token`、`claim_owner`、`claim_expires_at`；
- `(board_id,idempotency_key)` 有局部唯一索引（key 非空时），用于 task.create 的
  幂等。相同 key 且 canonical payload 相同返回原任务，不同 payload 返回冲突；
- `seq` 是看板内的递增引用序号，创建在同一 immediate transaction 中分配。

任务状态是唯一 canonical 状态真相，列只负责展示映射。`task_ref`（例如
`default#12`）由 `board.slug` 和 `seq` 组合，不写入数据库。

### `task_execution_plans`

字段为 `board_id`、`task_id`、`state`、`reason`、`updated_by`、`updated_at`。
`state` 只能是 `unplanned|planned|not_required`；`task_id` 是主键并以复合外键
`(task_id,board_id)` 指向同一看板的任务。创建任务同时写入 `unplanned` 行；创建
第一个 step 会变为 `planned`，显式 `task.plan.not_required` 会写入
`not_required` 和原因。

### `task_steps`

字段为 `id`、`board_id`、`parent_task_id`、`idempotency_key`、`position`、`title`、
`body`、`linked_task_id`、`required`、`status`、`resolution_note`、`resolved_by`、
`resolved_at`、`created_by`、`created_at`、`updated_by`、`updated_at`。

- `id` 匹配 `step_%`，`title` 非空，`required` 为 `0/1`，`status` 为
  `todo|done|skipped`；
- `(parent_task_id,board_id)` 为复合外键并级联删除；`linked_task_id`（可空）也以
  `(linked_task_id,board_id)` 约束同看板，且不得等于父任务；
- `(parent_task_id,idempotency_key)` 有局部唯一索引；相同 key 和相同 payload 返回
  已有 step，不同 payload 返回 `idempotency_conflict`；
- `position` 是父任务内排序键。必需 step 只有在 `done` 或 `skipped` 时才不阻塞
  `task.done`。

## 4. 依赖、运行和协作记录

### `task_dependencies`

字段为 `board_id`、`parent_task_id`、`child_task_id`、`created_at`。
`PRIMARY KEY(parent_task_id,child_task_id)` 提供自然幂等；禁止自依赖。父、子均以
带 `board_id` 的复合外键指向 `tasks(id,board_id)`，因此不能跨看板。创建依赖时应用
服务在同一事务前检查可达路径，拒绝形成环；数据库唯一性只负责重复边。

### `task_runs`

字段为 `id`、`board_id`、`task_id`、`status`、`worker_profile`、`worker_pid`、
`claim_token`、`claim_owner`、`claim_expires_at`、`started_at`、`last_heartbeat_at`、
`finished_at`、`exit_code`、`summary`、`error`、`log_path`、`metadata_json`。

`status` 只能是 `running|succeeded|failed|canceled|expired`；`task_id` 通过
`(task_id,board_id)` 复合外键关联任务。`idx_task_runs_one_active` 是
`UNIQUE(task_id) WHERE status='running'`，保证每个任务最多一个 active run；另有
`(task_id,started_at DESC)` 查询索引。run 由 claim 同事务创建，adapter 只能读取，
不能独立创建或修改 run。

`log_path` 只是相对/绝对路径文本。dispatcher 生成 `<log_root>/<run_id>.log`，
HTTP run.log 只接受配置的 canonical log root 下、精确 run 文件名的 regular file，
单次最多读取 256 KiB；数据库字段不能成为任意文件读取入口。

### `task_comments`

字段为 `id`、`board_id`、`task_id`、`idempotency_key`、`author`、`author_type`、
`agent_type`、`body`、`kind`、`metadata_json`、`created_at`。
`author_type` 只能是 `user|agent`，`agent_type` 仅在 agent 时允许；`kind` 为
`note|decision`；`body` 和 `author` 非空。`(task_id,board_id)` 复合外键保证看板
隔离；`(task_id,idempotency_key)` 局部唯一索引提供 comment.create 幂等，冲突规则
与 task.create 相同。创建评论与 `task.comment.created` 事件在同一事务完成。

### `task_events`

字段为自增 `id`、唯一 `event_id`、`board_id`、可空 `task_id`、可空 `run_id`、
`kind`、可空 `actor`、`payload_json`、`created_at`。`event_id` 匹配 `e_%`，`kind`
非空，`payload_json` 必须是合法 JSON；task/run 引用存在时以复合外键保证同看板。
事件只追加，按 `id` 升序分页，`after` 是排他游标；`board_id,id` 和
`task_id,id` 有查询索引。已知事件由 API 做精确 payload 校验，未知 kind 保留原始
合法 JSON。

## 5. 一致性边界

1. server 为唯一数据库 owner；每个连接打开 `PRAGMA foreign_keys = ON`。所有
   child row 的 `board_id` 先指向 `boards`，涉及 task/run 的关系再由复合外键保证
   board isolation。
2. mutation 使用 immediate transaction；task transition、claim、run、event 以及
   plan/step/dependency/comment 的相关写入必须整批提交或整批回滚。
3. claim 使用 task 的状态、空 claim 字段和 `lock_version` 做 compare-and-set；失败
   不得创建 run 或 event。heartbeat、release、review、done、block 同样用 owner/token
   和 lock version 保护。
4. canonical 数据只有本文件列出的表。搜索、图、向量、缓存或其他派生数据不能反向
   写入或替代这些事实。

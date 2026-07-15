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

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
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

### 4.1 Board isolation 责任边界

SQLite 是 canonical truth，但 board isolation 由 schema、service 和 diagnostic gate 共同保证：

1. DB constraint：所有 board-scoped rows 都有 `board_id` 并引用 `boards(id)`；
   referenced task / label / run id 也各自有 FK，确保引用对象存在。`task_labels`、
   `task_dependencies`、`task_steps`、`task_execution_plans`、`task_runs`、`task_comments`、`task_attachments` 和较新的
   label semantics / atoms / ontology link 表使用包含 `board_id` 的复合 FK，直接阻止这些
   关系表出现 cross-board row。`task_events` 保留 nullable task/run refs 与
   `ON DELETE SET NULL` 语义，由 INSERT/UPDATE triggers 校验非空 refs 的 board scope。
2. Service guard：CLI、HTTP、desktop 和 dispatcher 的正常写路径必须先在同一 board
   scope 内 resolve task、label、run 等对象，再写关系 row；例如 task label binding、
   dependency、comment、event、run 和 attachment 都不应跨 board 组合。
3. Doctor/import check：`kanban doctor` 和 JSONL import final gate 会只读检查基础关系表
   中 `row.board_id` 与 referenced task / label / run 的 board 是否一致，并运行
   `PRAGMA foreign_key_check`。任一 violation 都会成为 hard-error issue；import 会在
   commit 前回滚整个 replace transaction。

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
表示任务已经被人工或服务显式放入可 claim 队列；P0-P3 只影响列表和 dispatcher 在可选任务之间的排序。

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

Schema-level invariant：`parent_task_id` 和 `child_task_id` 必须都属于 row
`board_id`。旧数据库升级到 composite FK schema 前会先检查 existing cross-board rows；
发现不一致时 migration 会失败并要求先用 doctor/repair 清理。

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

parent 从 `done` reopen 后，直接 child 中仅 `triage|todo|scheduled|ready` 会按 readiness 重算；`running|blocked|review|done|archived` 不隐式改写。


---

## 7. Step / Execution Plan

Step 是父任务内部的有序执行步骤，不是阻塞依赖关系。Step 可以是普通文本，
也可以链接到另一个普通 task 作为上下文。链接 task 不会自动创建
`task_dependencies` 边，也不会用 linked task 的状态自动完成 step；step 自己有独立的
`todo | done | skipped` 状态。

### 7.1 Steps

表：`task_steps`

Schema-level invariant：`parent_task_id` 必须属于 row `board_id`；可选的
`linked_task_id` 也必须属于同一 board，且不能等于 `parent_task_id`。Service 还必须
拒绝 archived parent、archived linked task、空白标题和 cross-board link。

字段：

| 字段 | 说明 |
|---|---|
| `id` | Step ID。 |
| `board_id` | 所属 board。 |
| `parent_task_id` | 被规划的父任务。 |
| `position` | 父任务内步骤排序键。 |
| `title` | 步骤标题。 |
| `body` | 可选说明文本。 |
| `linked_task_id` | 可选上下文 task。 |
| `required` | 是否阻塞父任务 complete/archive。 |
| `status` | `todo`、`done` 或 `skipped`。 |
| `resolution_note` | done/skip/reopen 的说明。 |
| `resolved_by` | 最近一次 resolution actor。 |
| `resolved_at` | 最近一次 resolution 时间。 |
| `created_by` | 创建 actor。 |
| `created_at` | 创建时间。 |
| `updated_by` | 最近更新 actor。 |
| `updated_at` | 最近更新时间。 |

索引：

- `idx_steps_parent_position(parent_task_id, position)`
- `idx_steps_linked_task(linked_task_id)`
- `idx_steps_board_status(board_id, status)`

语义：

```text
parent task contains ordered step
optional linked_task_id supplies task context only
```

Step 不会直接驱动 `dependency_blocked` 或 `unfinished_parent_count`。Required step
只参与 execution-plan guard：父任务不能 complete/archive，直到所有 required step
都是 `done` 或 `skipped`。

### 7.2 Execution plans

表：`task_execution_plans`

字段：

| 字段 | 说明 |
|---|---|
| `board_id` | 所属 board。 |
| `task_id` | 被规划的 task。 |
| `state` | `unplanned`、`planned` 或 `not_required`。 |
| `reason` | `not_required` 的说明。 |
| `updated_by` | 最近更新 actor。 |
| `updated_at` | 最近更新时间。 |

索引：

- `idx_execution_plans_board_state(board_id, state)`

派生口径：

```text
steps count > 0 => planned
explicit not_required row and no steps => not_required
otherwise => unplanned
```

事件：

```text
task.step.created
task.step.updated
task.step.removed
task.step.done
task.step.skipped
task.step.reopened
task.execution_plan.planned
task.execution_plan.not_required
```

## 8. Run

表：`task_runs`

Schema-level invariant：`task_id` 必须属于 row `board_id`。这保证 run attempt
不能在 SQLite 层跨 board 指向 task。

Run 是一次 execution attempt。

### 8.1 Run status

```text
running | succeeded | failed | canceled | expired
```

### 8.2 字段

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

### 8.3 约束

- active `running` task 必须有 active run。
- 一个 task 可以有多个历史 run。
- 同一 task 同时最多一个 running run。

SQLite 不强制最后一条，需要 service 层和 transaction 保证。

---

## 9. Event

表：`task_events`

Event 是 append-only 事实记录。

### 9.1 Event kind

API/SSE 当前类型化的 39 个 known kind：

```text
board.created
board.archived
dependency.added
dependency.removed
label.created
label.deleted
signal.recorded
signal.reviewed
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
```

### 8.2 Payload 示例

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_owner": "alice",
  "metadata": {}
}
```

`task_events.kind/payload_json` 的 SQLite storage 允许未来 unknown kind。Events API 与 SSE
对上面 39 个 known kind 使用精确 sibling payload contract，known mismatch fail closed；unknown
kind 的合法 JSON payload 保持 lossless。外层 `task_id`、`run_id`、`actor` 都是
required-nullable。portable JSONL 的 event payload 仍是 opaque JSON，不复用该 typed union。

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
| `kind` | `note` / `decision` / `signal`，表示 comment 内容语义，不表示作者身份。`signal` 是 signal ledger backlink。 |
| `metadata_json` | `kind` 对应的结构化 payload；默认 `{}`，必须是合法 JSON object。`kind=decision` 时必须符合 decision schema。`kind=signal` backlink metadata 包含 `type:"signal_link"`、`signal_id`、`observation_id`、`signal_kind`、`signal_status`。 |
| `created_at` | 创建时间。 |

旧 comment rows / JSONL import 会迁移到新语义：旧 `human` 变为 `user`，旧 `agent/system` 或 `worker/system` 来源变为 `agent`，旧 `text/system/worker` 内容变为 `note`。没有结构化 metadata 的旧 `decision` 也按 `note` 保留 body fallback。

Comment 创建时也写一条 `task_events(kind='task.comment.created')`。

`metadata_json` 是 SQLite canonical storage 列；CLI/API response 会解码成自然、无损的
`metadata` object。普通 note/signal metadata 保持开放。只有 service-generated backlink 的
完整 shape 由 `SignalLinkMetadataOutput` 独立证明，不能把用户自定义的同名键碰撞当成协议。

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
| `color` | UI 颜色 token。 |
| `created_at` | 创建时间。 |
| `updated_at` | 更新时间。 |

同一 board 内 label name 唯一。

Task 与 label 的关联通过 `task_labels(task_id, label_id)` 关联表表达。
Label 只用于分类、过滤和展示；添加或移除 label 不改变 `tasks.status`，
不触发 dependency recompute，也不会让 dispatcher claim `review` 或其他非
`ready` 状态。

### 11.1 Label semantics

`labels` 仍是 canonical label identity：名称、颜色和 board 作用域由 `labels`
定义。`task_labels` 仍是 task 的最终 label 绑定事实。语义推荐和向量检索使用
额外 truth 表，不替代这两张表。
`labels` identity CRUD 是基础 vocabulary registry，不写 ontology mutation ledger；
`label delete` 不会隐式删除 semantics/atoms，必须先通过 CAS-protected semantics clear
清空语义。

表：`label_semantics`

| 字段 | 说明 |
|---|---|
| `label_id` | 关联 `labels(id)`，一条 label 最多一条 semantics。 |
| `board_id` | 冗余 board scope，用复合外键保证 label/board 一致。 |
| `description` | label 的自然语言说明。 |
| `applies_when` | JSON string array，正向适用条件。 |
| `excludes_when` | JSON string array，反向排除条件。 |
| `positive_examples` | JSON string array，正向示例。 |
| `negative_examples` | JSON string array，反向示例。 |
| `created_at` / `updated_at` | 语义记录时间。 |

表：`label_atoms`

`label_atoms` 是从 `label_semantics` 与 label name 展开的 SQLite materialized
projection。它保存 positive 与 negative 两种 polarity，供后续 Group OMP/NNLS label
solver 和 LanceDB atom retrieval 使用；它随 semantics mutation 同事务重建，不是独立于
`label_semantics` 的第二份 semantic truth。

| 字段 | 说明 |
|---|---|
| `id` | 稳定 `la_...` atom id。 |
| `label_id` / `board_id` | 关联 canonical label 与 board。 |
| `polarity` | `positive` / `negative`。 |
| `kind` | `name`、`description`、`applies_when`、`positive_example`、`excludes_when`、`negative_example`；有 description 时，`description` atom 是 `label: {name}\ndescription: {description}` canonical atom，无 description 时才使用 `name` fallback atom。 |
| `text` | trim 且规范化 whitespace 后的 atom 文本；每个非空行内部 whitespace collapse，canonical 行分隔保留，空文本不入库。 |
| `ordinal` | 同一 label 展开后的顺序；同语义重复 atom 去重时保留首次出现的 ordinal。 |
| `content_hash` | atom 语义内容 hash，用于派生层判断变化；输入为 `label_id + polarity + kind + normalized_text`，不包含 `ordinal`。 |
| `created_at` / `updated_at` | projection row 时间。 |

派生向量表：`kb_label_atoms`

`kb_label_atoms` 是 LanceDB 中的可重建 label atom 向量表，独立于 task chunk 表
`kb_chunks`。它按 `board_id`、`embedding_model`、`polarity` 查询 atom evidence，
返回 `label_id`、atom id、`polarity`、`kind`、`text` 和 LanceDB `_distance` 原始
distance 等字段。语义 label 候选会用返回的 atom vector 在本地重新计算
query/residual cosine similarity，不把 distance 当作 solver score。派生表损坏或缺少
provider 时只让 label atom index degraded，不影响普通 label CRUD、`task_labels` 绑定
或 task 状态机。

### 11.2 Generic signal ledger

Generic signal ledger 保存 agent/product 在 kanban 工作流中发现的通用问题信号，
例如 CLI 参数摩擦、提示误导、参数设计不符合 agent 惯用方式，或 operator 发现的
产品反馈。它是 board-scoped 审计账本和只读 inbox 数据源，不替代 `tasks.status`、
task comments、runs、events 或 label ontology ledger。

- `signal_observations` 保存一次观察的来源、actor、task/run/comment 关联和原始证据。
- `signals` 保存一个可独立 review 的通用 signal，并指向对应 observation。
- 通用 signal 与 `label_ontology_signals` 分离；ontology signals 仍只服务 label
  semantics/atom/proposal review 和 mutation provenance。
- 当前 public HTTP surface 只读取通用 signal；lifecycle 写操作仍由 CLI/runtime
  signal record 流程负责。
- Board-scoped list/review surface 只通过 board 路由读取：
  `/api/v1/boards/{board}/signals*`。单条详情
  `GET /api/v1/signals/{signal_id}` 是 operator-wide detail lookup，用于从
  backlink 或 inbox row 直接打开已知 signal；它不改变 signal 的 `board_id`
  truth，也不把 signal 混入其它 board 的列表。
- `signal_observations.task_id`、`run_id`、`comment_id` 是 provenance/history
  soft refs。当前一致性由 service 写入路径、doctor 和 import final gate 维护；
  这些 refs 允许保留历史来源语义，未来如需把全部来源关系硬化，可迁移为
  board-composite FK。

表：`signal_observations`

一行表示一次 agent 或 operator 观察。Observation 可关联 task、run 或 comment；
这些关联用于定位来源，不改变对应实体状态。

| 字段 | 说明 |
|---|---|
| `id` | `obs_...` observation id。 |
| `board_id` | 来源 board scope。 |
| `task_id` / `task_ref_snapshot` | 可空。来源 task 与捕获时的人类 ref 快照；task 后续改动不影响快照。 |
| `run_id` | 可空。来源 execution run。 |
| `comment_id` | 可空。来源 comment。 |
| `actor` / `agent_type` | 捕获者名称与可选 agent type。 |
| `source` | 可空。信号来源，例如 `codex-hook`、`cli` 或 `operator`。 |
| `evidence_json` | JSON object 字符串，保存命令、stderr、上下文片段、hook 提示等原始证据。 |
| `created_at` | 创建时间。 |

表：`signals`

一行表示一个可独立进入 operator inbox 的通用 signal。它只描述发现的问题和 review
lifecycle，不直接触发修复或修改 canonical workflow。

| 字段 | 说明 |
|---|---|
| `id` | `sig_...` signal id。 |
| `board_id` / `observation_id` | board scope 与来源 observation。 |
| `kind` | 通用 signal 类型，例如 `agent_cli_friction`。 |
| `title` / `summary` | 面向 operator 的短标题与摘要。 |
| `severity` | 文本严重度，例如 `info`、`medium` 或 `high`。 |
| `status` | `open`、`confirmed`、`resolved`、`rejected`、`superseded`。 |
| `dedupe_key` | 可空。用于调用方聚合相似 signal。 |
| `superseded_by_signal_id` | 可空。指向同 board 的替代 signal。 |
| `reviewed_by` / `reviewed_at` / `review_reason` | lifecycle review 记录。 |
| `created_at` / `updated_at` | 创建与更新时间。 |

默认 review queue 只读取 `open` 与 `confirmed` signals；完整历史需显式
`include_all` 或指定 status。

### 11.3 Label ontology ledger

Label ontology ledger 记录 task 标注过程里的证据、分歧 signal、review/action 历史
和 validation 结果。它是可查询的审计账本，不替代 canonical truth：

- `labels` / `task_labels` 仍决定 task 当前实际绑定哪些 label。
- `label_semantics` 决定 label 的 canonical 语义；`label_atoms` 是它的 SQLite
  materialized atom projection。
- `label_semantic_proposals` 仍负责新 label proposal lifecycle。
- ontology ledger 覆盖 semantics/atom mutations；base `labels` identity CRUD 位于
  ledger 之外，只写普通 events。

这些表是 label 系统中的不同角色，不是六个严格独立的存储层。`label suggest` 是计算结果，
`kb_label_atoms` 是可重建检索投影，proposal 和 ledger 是需要持久审计的 SQLite records；
它们都不能直接替代 `task_labels` 的当前绑定事实。

表：`label_ontology_observations`

一行表示一次完整的 task label 判断过程。它保存当时的 task 快照、agent 候选、
`label suggest` 快照、最终选择和由 snapshot 派生的 solver 指标；即使 task、label 或
atoms 后续变化，仍能还原当时为什么产生 signal。Observation 是只读 provenance：
record 写入不会修改 `task_labels`、`label_semantics`、`label_atoms`、label atom index 或
proposal。

| 字段 | 说明 |
|---|---|
| `id` | `lor_...` observation id。 |
| `board_id` / `task_id` | 来源 board 与 task。 |
| `task_ref_snapshot` | 捕获时的人类 ref，例如 `default#42`。 |
| `task_snapshot_json` | 捕获时的 task title、description、labels、version/hash 等快照。 |
| `suggest_input_hash` | 可空。按 label suggest 输入（normalized title + description）计算的窄 hash，用于 validation comparability；旧 observation 缺失时按 legacy incomparable 处理，不能静默 passed。 |
| `agent_candidates_json` | agent 原始候选 labels、置信度和理由。 |
| `suggestion_snapshot_json` | 完整 suggestion 输出、参数、模型和 index 状态快照；新 capture path 要保存未改写的原始 snapshot。 |
| `final_decision_json` | 最终接受、拒绝和未采用 labels 的判断。 |
| `suggest_coverage` / `suggest_coverage_cosine` / `suggest_residual_norm` | 可查询的 solver 指标。新 capture path 从 `suggestion_snapshot_json` 派生这些值；调用方不应重复手写。`suggest_coverage = clamp(1 - suggest_residual_norm, 0.0, 1.0)`，二者不是独立证据；`suggest_coverage_cosine` 是 query 与 fitted vector 的 cosine similarity，可作为补充指标。 |
| `suggest_needs_new_label` / `suggest_degraded` | 捕获时 suggestion 状态。新 capture path 从 `suggestion_snapshot_json` 派生这些值。`suggest_needs_new_label` 是 coverage review 兼容字段，不等于自动 vocabulary gap；判断新 label 需要结合 reason codes、evidence、diagnostics 和人工语义判断。 |
| `diagnostics_json` | suggestion diagnostics 数组。新 capture path 从 snapshot 的 `diagnostics` 派生；冲突的重复输入会被拒绝。 |
| `capture_fingerprint` | 同一 board 内幂等 fingerprint。 |
| `created_by` / `created_by_type` / `agent_type` | 捕获者身份。 |
| `created_at` | 创建时间。 |

表：`label_ontology_signals`

一行只表达一个可独立 review 的 ontology 问题，例如某个已有 label 漏选、
suggest 误选、存在 vocabulary gap 或 label 边界/名称问题。

| 字段 | 说明 |
|---|---|
| `id` | `los_...` signal id。 |
| `observation_id` / `board_id` | 来源 observation 与 board scope。 |
| `kind` | `false_negative`、`false_positive`、`vocabulary_gap`、`name_issue`、`boundary_issue`、`structure_issue`。 |
| `status` | `open`、`confirmed`、`resolved`、`rejected`、`superseded`。 |
| `target_label_id` / `target_label_name_snapshot` | 已有 label 目标；名称快照用于历史解释。 |
| `related_labels_json` | split/merge 等多 label 关系快照。 |
| `proposed_action` | `observe`、`add_positive_atom`、`add_negative_atom`、`update_semantics`、`bootstrap_label`、`rename_label`、`split_label`、`merge_labels`。 |
| `candidate_atom_polarity` / `candidate_atom_kind` / `candidate_text` | 建议 atom 的 polarity、kind 和泛化文本。 |
| `candidate_content_hash` | 按 `label_id + polarity + kind + normalized_text` 计算的聚合键。 |
| `proposed_label_name` / `proposed_label_name_normalized` | vocabulary gap 或 rename 候选。 |
| `proposal_json` | 新 label 或结构变更的候选语义快照。 |
| `agent_selected` / `suggest_state` / `suggest_score` / `suggest_rank` / `final_selected` | agent、suggest 与最终判断之间的分歧证据。 |
| `rationale` / `confidence` | 可审查理由和可选置信度。 |
| `signal_key` | observation 内幂等键。 |
| `superseded_by_signal_id` / `status_reason` | 关闭或替代原因。 |
| `created_at` / `updated_at` / `reviewed_at` / `closed_at` | 生命周期时间。 |

`label ontology review` 是基于 signals 的只读聚合投影，不是新的 canonical truth，也不是
新的可持久化 derived store。group key 来自调用方选择的维度：`label` 使用目标 label，
`proposed-label` 使用 normalized proposed label name，`candidate-atom` 优先使用
`candidate_content_hash`。没有 candidate
atom 的 signals 不会进入一个全局空值 bucket；fallback key 会带上 signal kind、target
label 或 proposed label、以及 proposed action，例如
`no-candidate-atom|kind:vocabulary_gap|proposed:ontology ledger|action:bootstrap_label`。
因此一个 group 的含义是“这些 signals 共享同一个 review key”，不是“这些 signals 已被证明
来自同一个根因”。

`cluster` 是 opt-in duplicate-signal review-aid，不默认启用，不写 canonical atoms，不自动
confirm/apply/validate/mutate，也不成为 SQLite truth。cluster key 每次 review 查询时从
已有 signal 文本和 review scope 重建：key 始终包含 signal kind、proposed action、target
label snapshot（或 id fallback）以及 proposed label scope，再附加优先级最高的
lexical-normalized candidate text，其次 proposed label，再其次 rationale，最后退回纯
scope 组合。这个 scope 前缀避免把相同文本但不同 label boundary/action 的 signals 强制
合并；输出中的 `cluster_key` 和 `cluster_reason` 只解释这个辅助分组来源。

Review queue 的默认排序使用 distinct source task count（`task_count`）作为主要热度指标，
再按 confirmed count、latest signal time 和 key 排序。`signal_count` 只是 group 内原始
signal 行数；同一 task 可以贡献多条 signals，所以它不能单独代表模型错误率、precision、
recall 或 label suggest 质量。需要质量指标时必须另有 denominator，例如 agreement cohort
或固定评估集。

`label ontology quality` 是一个只读 analytics 投影，不新增表，也不写 canonical truth。
它把 `label_ontology_observations` 作为 denominator 来源并在输出中记录该来源、distinct
task 数、observation 数、agreement/degraded observation 数、时间范围和 task ref sample；
同时把 `label_ontology_signals` 作为 raw disagreement numerator 来源，按 kind/status
给出原始 signal counts。只有当 denominator 中存在 agreement observations 时，才会给出
`disagreement_task_rate`；只有 signals 的数据集会明确返回 rate unavailable，避免把分歧
记录误称为错误率。Precision/recall 仍需要带 expected labels 的独立评估 cohort，当前
ledger signal 不能单独提供这些指标。

长期 label ontology regression corpus 属于测试/评估基础设施，不是新的 SQLite truth。
当前固定 corpus 测试使用临时 DB 和内存 label atom index 跟踪 important labels 的 known
positive/negative-control tasks，并比较 `label suggest` 的 selected labels、score 与
evidence atoms。Corpus run 本身应保持只读 canonical ontology；只有测试中显式模拟的
临时 semantics/atom 变更才会用于证明 comparison 能发现回归。真实 DB 上的长期 corpus
需要等稳定任务集积累后再扩展，不应替代 ledger signals、trusted validation 或人工 review。

当前没有 label-ontology 专属 graph projection。`label_ontology_*` 表本身就是 SQLite
provenance truth；`kanban graph` / Oxigraph 只投影 Knowledge Substrate 的
`entity_relations`，不保存或拥有 label ontology action/signal truth。若未来出现明确的
rename/split/merge 或 provenance relationship 查询需求，新增 projection 必须从
`labels`、`label_semantics`、`label_atoms`、`label_semantic_proposals` 和
`label_ontology_*` 重建，并通过 `index_outbox` / `derived_store_state` 表达 dirty、sync、
rebuild 和 error 状态；删除或损坏 graph 不得改变 canonical label/ontology/ledger rows。

表：`label_ontology_actions`

Action 是 append-only history，表示 reviewer/agent 实际确认、拒绝、修改 ontology 或
记录 validation 的动作。直接修改 label semantics 或接受 proposal 时，provenance
也写成 action。

| 字段 | 说明 |
|---|---|
| `id` | `loa_...` action id。 |
| `board_id` | board scope。 |
| `parent_action_id` | validation 等后续 action 指向被验证的 mutation action。 |
| `action_type` | `confirm`、`reject`、`supersede`、`resolve_no_change`、`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、`update_semantics`、`create_label_proposal`、`bootstrap_label`、`rename_label`、`split_label`、`merge_labels`、`validate`、`revert_ontology_mutation`。 |
| `reason` | 必填人工或 agent 理由。 |
| `target_label_id` / `result_label_id` | 修改目标与结果 label。 |
| `result_atom_id` / `result_atom_content_hash` | 新增或采用 atom 的软引用和稳定 hash。 |
| `result_proposal_id` | 关联的 `label_semantic_proposals`。 |
| `canonical_before_hash` / `canonical_after_hash` | 修改前后 canonical semantics hash。 |
| `change_json` | before/after/diff 或其它可解释变更快照。 |
| `validation_requirement` | `none`、`required`、`unsupported`。表达 parent mutation 是否需要 typed validation policy；不改写历史 attempt outcome。 |
| `validation_status` | `not_required`、`pending`、`passed`、`failed`、`partial`。对 mutation parent 是历史兼容/base status；对 `validate` action 表示一次 attempt outcome。 |
| `validation_json` | validation evidence envelope；service 会包装 supplied/collected payload、source signal cases、task snapshot comparability、parent action result 引用和 summary。公共 supplied/collected payload 只保存在 top-level `manual`；generated `cases[]` 用 `after.manual_case_ref` 指向 `manual.cases[]` 中对应 signal 的 evidence，避免把同一 payload 复制到每个 case。`failed` / `partial` 可保存 external/manual attestation 诊断。`passed` action 只能来自工具采集的 `trusted_automated` evidence（collector source、embedding model、solver options、clean atom index status/generation、per-signal before/after cases），并按 parent action 校验 positive atom、negative atom、bootstrap label 和 negative positive-control/waiver policy；调用方手写 JSON 或自称 `automated` 不构成可信来源。 |
| `created_by` / `created_by_type` / `agent_type` | action actor。 |
| `created_at` | 创建时间。 |

`validation_effective_outcome` 是读取 DTO 中的 reducer 结果，不是独立存储列。它按
`validation_requirement` 和 latest validation child action（`created_at,id`）计算：
`not_required`、`unsupported`、`pending`、`passed`、`failed` 或 `partial`。只有
`required + trusted passed` 会 resolve linked source signals；`unsupported` 可以记录
external failed/partial 诊断，但拒绝 passed。

`label_ontology_action_atom_effects` 连接一条 root mutation action 与本次实际 added/removed
atom snapshots。它保存 `board_id`、`action_id`、`label_id_snapshot`、`atom_id_snapshot`、
`atom_content_hash`、`polarity`、`kind`、`text`、`effect` 和 `created_at`；`effect` 只允许
`added` / `removed`，唯一约束为 `(action_id, atom_content_hash, effect)`。Action 使用
board-scoped composite FK；atom snapshot 不使用 live FK，因为 `label_atoms` 会随 projection
重建。

`result_atom_id` 故意不是强 FK。`label_atoms` 会随 semantics rebuild delete/insert；
历史 action/effect 依赖 `result_atom_content_hash`、effect row 和 `change_json` 中的 atom
snapshot 保持可解释。Atom explain 查询会优先使用
`label_ontology_action_atom_effects`，也允许用 legacy `result_atom_id` /
`result_atom_content_hash` 兼容旧数据。`adopt_existing_atom` 表示新的 source signal 采用了当前已存在 atom，
不代表 canonical 内容新增。已有 atom 如果来自旧 semantics 写入而没有任何 ontology action 引用，
查询结果只标记 `legacy_untracked=true`，不会伪造 provenance。

`create_label_proposal` action 对同一 `(board_id, result_proposal_id)` 唯一；proposal
accept 生成的 `bootstrap_label` action 通过 `parent_action_id` 指向这条 creation
action，从而让 proposal creation -> bootstrap acceptance provenance 链路保持无歧义。

`revert_ontology_mutation` 是 append-only rollback history：它不会修改或删除原 mutation
action，而是用 `parent_action_id` 指向被撤销 action，并把 canonical semantics 恢复到该
action 的 `change_json.before` / `canonical_before_hash` snapshot。当前实现只覆盖
label-scoped semantics/atom mutations（`add_positive_atom`、`add_negative_atom`、
`update_semantics`），成功后标脏 label atom index 并保持 validation pending；bootstrap
的 label identity / task binding rollback 不由该 action 类型表达。

当前 constructive ontology mutation path 的责任边界如下：

- `label_semantics` 是 canonical ontology truth；`label_atoms` 是它的 SQLite materialized
  projection；`label_ontology_actions` 是 append-only provenance，不是第二份 truth。
- `update_semantics`、`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
  `create_label_proposal` 和 `bootstrap_label` action 只能由专用 service path 写入。
  `adopt_existing_atom` 是 provenance-only path，before/after hash 相同，只连接新的
  source signals 到 existing atom，不修改 canonical semantics/atoms，也不标脏 atom
  index；其它 constructive mutation 与对应 canonical write 位于同一 SQLite transaction。
- 每个 semantics/atom mutation transaction 只写一条 root mutation action；`change_json`
  只保存一次 before/after semantics snapshot。实际 added/removed atoms 写入
  `label_ontology_action_atom_effects`，description-only patch 写零 effect，no-op patch 不写
  action/effect/index dirty。
- Manual mutation 可以没有 source signals，但仍必须记录 actor、reason、before/after
  hash 和 change snapshot。Signal-driven mutation 会额外写入
  `label_ontology_action_signals` links。
- `label semantics upsert` 默认是 patch/CAS path：`expected_semantics_hash` 防止
  lost update；缺省字段不清空旧 semantics；`replace=true` 才执行完整替换，并将缺省
  arrays 解释为空集合。
- Direct task-label bootstrap 与 proposal accept 共用 adoption primitive。Task-label
  bootstrap 可创建或复用无 semantics 的同名 canonical label；proposal accept 当前会先拒绝
  任何 existing normalized-name conflict，因此成功路径创建新 canonical label。二者都会写
  semantics/atoms、标脏 label atom index，并写一个 `bootstrap_label` root action 和 added
  atom effects；proposal accept
  不写 `task_labels`，task-label bootstrap 会绑定来源 task。失败时 canonical writes 与
  provenance action 一起回滚。
- `rename_label`、`split_label`、`merge_labels` 仍可作为 signal proposed_action 或 legacy
  action 读取；当前 public service/CLI/HTTP 不再写新的 structure plan mutation action。旧
  structure plan action 的 validation requirement 解释为 `unsupported`。
- `legacy_untracked=true` 只表示当前 atom 没有可匹配的 ontology action，例如旧数据或
  destructive cleanup 后的历史缺口；新 constructive mutation 不应依赖这种兼容路径来解释
  provenance。

表：`label_ontology_action_signals`

多对多连接 action 与 signals。多个 signals 可以支持一次 atom 修改；同一个 signal
也可以先被 confirm，随后关联 mutation action 和 validation action。

默认 review queue 只读取 `open` 与 `confirmed` signals；完整历史需显式 include all。
Mutation action 写入后通常保持 source signals 为 `confirmed`。只有 trusted automated
`passed` validation 会把 linked source signals 转为 `resolved`；external/manual
attestation、`failed` 或 `partial` validation 只追加历史，不删除 signals，也不把问题
伪装成已验证关闭。

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
| `store_name` | 派生 store 名称，例如 `tantivy_tasks`、`oxigraph_relations`、`lancedb_chunks`、`lancedb_label_atoms`。 |
| `schema_version` | store schema/contract 版本。 |
| `last_event_id` | store 已成功提交的全局 `task_events.id` 水位。 |
| `dirty` | 是否仍有未完成 outbox、失败 outbox 或最近一次 store 更新失败。 |
| `last_rebuild_at` | 最近成功 rebuild 时间。 |
| `last_sync_at` | 最近成功 sync 时间。 |
| `last_error` | 最近失败证据。 |
| `updated_at` | 状态更新时间。 |

`last_event_id` 是 store 全局成功处理水位，不是 board 局部水位。成功 sync/rebuild 只能单调推进这个值；当一个 board sync 完成但其他 board 仍有 pending/running/failed outbox 时，`dirty` 必须保持 true。`dirty=false` 只表示同一 store target 当前没有 unfinished outbox 且最近一次 store 更新没有失败。

`last_error` 成功后清空，失败时保留错误证据并保持 `dirty=true`。Operator 应通过 `kanban derived status`、`kanban doctor`、maintenance API 和对应 `sync/rebuild` 命令恢复派生层；派生 store 损坏或落后不改变 SQLite task truth。

表：`label_atom_index_boards`

`label_atom_index_boards` 只跟踪可重建的 `lancedb_label_atoms` 派生层在各 board
上的刷新状态，不是 label truth。`label_semantics` / `label_atoms` 更新会把对应
board 标脏；单个 board 的 label atom rebuild 成功只清理该 board 的 dirty 标记。
只有该 store 下所有 board 都不 dirty 时，`derived_store_state.dirty` 才能变为
`false`。

### 11.2 Label semantic proposals

表：`label_semantic_proposals`

`label_semantic_proposals` 是新增 label 的持久提案生命周期，不是 canonical
label truth。它只记录“现有 label atom suggestion 覆盖不足时，外部/manual provider
给出的候选语义”。未显式 accept 前，不创建 `labels`、`label_semantics`、
`label_atoms` 或 `task_labels`。

| 字段 | 说明 |
|---|---|
| `id` | `lp_...` proposal id。 |
| `board_id` / `task_id` | 提案来源 task。 |
| `status` | `proposed` / `accepted` / `rejected`。provider 不可用不写成 status，而是返回 degraded attempt。 |
| `name` / `description` / `applies_when` / `excludes_when` / `positive_examples` / `negative_examples` | 候选 label semantics。数组字段为 JSON string array。 |
| `heuristic_coverage` / `heuristic_coverage_cosine` / `heuristic_residual_norm` | 来自当前 residual label suggestion solver 的覆盖/残差元数据，用于记录 proposal 创建时现有 label atoms 的覆盖程度；`heuristic_coverage = clamp(1 - heuristic_residual_norm, 0.0, 1.0)`，二者不是独立证据；`heuristic_coverage_cosine` 是 query 与 fitted vector 的 cosine similarity。 |
| `top1_existing_label_id` / `top1_existing_label_name` | 当前启发式 top1 existing label。 |
| `diagnostics_json` | JSON string array，包含 degraded、冲突或 validation 诊断。 |
| `decision_reason` / `resolved_label_id` / `decided_at` | accept/reject 决策信息；accept 后 `resolved_label_id` 指向新建 canonical label。 |

Accept 只允许 `proposed` proposal。accept 通过共享 adoption primitive 创建同 board 的
canonical `labels` 行，并写入对应 `label_semantics` / `label_atoms`，同时标脏
`lancedb_label_atoms` 派生 store，写入 `bootstrap_label` provenance action，并把
`resolved_label_id` 指向 result label；proposal status、canonical writes 与 action
provenance 同 transaction 提交。它不写入 `task_labels`，不会把新 label 自动绑定到来源
task。

Reject 将 proposal 标记为 `rejected`。与现有 label 发生 normalized-name 冲突的
候选会持久化为 `rejected`，diagnostics 包含 `near_duplicate_label_conflict`。
Normalized-name 冲突是忽略大小写、空白和标点后的 deterministic near-duplicate
heuristic。

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

## 15. Export / Import Format

JSONL export/import 是 portable board snapshot 格式：

```bash
kanban export --board default --format jsonl --out board.jsonl
kanban import --input board.jsonl --replace
```

每行：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"type":"task","data":{...}}
{"type":"event","data":{...}}
{"type":"comment","data":{...}}
{"type":"dependency","data":{...}}
```

Generic signal ledger 使用稳定 record types：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"type":"signal_observation","data":{...}}
{"type":"signal","data":{...}}
```

Label ontology ledger 使用稳定 record types：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"type":"label_ontology_observation","data":{...}}
{"type":"label_ontology_signal","data":{...}}
{"type":"label_ontology_action","data":{...}}
{"type":"label_ontology_action_atom_effect","data":{...}}
{"type":"label_ontology_action_signal","data":{...}}
```

Portable descriptor authority 共覆盖 21 个 discriminator；input/output 各有 exact root，共
42 个 Draft 2020-12 schemas。每行 `data` 闭合，required-nullable key 必须存在但可显式为
`null`，真实 export producer 与 import consumer 使用同一 descriptor/fixture registry。
SQLite 中的 `evidence_json`、`related_labels_json`、`proposal_json`、`change_json`、
`validation_json` 等仍是 canonical storage 列；公开 adapter 只暴露去掉 `_json` 后的自然 JSON。

Import 另有一条仅向前的 compatibility migration，用于读取 natural JSON contract 采用前、
由上一版 exporter 生成的 storage-native JSONL snapshot。该格式以 `column.hidden=0|1`
以及 `metadata_json` / `payload_json` 等真实 SQLite 列形状识别；同一 snapshot 必须保持
单一格式，不能混用 storage-native 与 natural records。同一 record 只要同时出现 natural
renamed key 与 storage-native renamed key，就会在 normalization 前被拒绝，不能让 legacy
值静默覆盖 natural 值。Importer 只对 coherent 父版本 record 把上一版 JSON text 列和 integer
boolean 转为当前 natural record，再执行同一 exact contract validation 与下述 transaction/final
consistency gates。当前及后续 export 始终只写 natural JSON，不再产生 storage-native keys；
这不是长期双轨 public contract。

导入时会在同一 transaction 中先插入 rows，再运行 final consistency gate。基础关系表
会检查 `task_labels`、`task_dependencies`、`task_runs`、`task_comments`、
`signal_observations`、`signals`、`task_events`、`task_attachments` 的 row board 与
referenced task / label / run / comment / observation board 是否一致；失败时整个
`--replace` import transaction 回滚，不提交部分数据。

Ontology rows 也在同一 transaction 中插入，并延迟回填
`label_ontology_signals.superseded_by_signal_id` 与
`label_ontology_actions.parent_action_id`，避免依赖同表自引用 rows 的文件顺序。导入完成前会校验 ontology ledger board isolation：observation/signal board、action
parent board、action-signal link board、label/proposal soft reference board 必须一致；
orphan action-signal links、supersede cycles 和 action parent cycles 会导致 import
失败。

Generic `signals.superseded_by_signal_id` 同样会延迟回填，避免依赖同表自引用 rows 的文件顺序。

`kanban doctor --json` 对上述基础关系表、SQLite `PRAGMA foreign_key_check`、ontology
ledger consistency 和 generic signal ledger board consistency 规则做只读巡检。
基础关系表问题返回 `consistency_errors`、`consistency_warnings`、
`consistency_issues[]`；ontology ledger 问题返回 `ontology_ledger_errors`、
`ontology_ledger_warnings`、`ontology_ledger_issues[]`。Issue 包含 `severity`、
`code`、`message`、`record_ids`，用于定位损坏 row；基础关系表 message 包含
`table`、`row`、`row_board` 和 `referenced_board`，foreign-key issue 会记录 table、
rowid、parent table 和 FK index。Hard error 覆盖 row board mismatch、
missing v12 ontology table、跨 board link、orphan action-signal/action-effect link、generic
signal orphan/cross-board context、generic signal supersede cycle、parent/supersede 异常、label/proposal/task board mismatch、
supersede cycle 和 action parent cycle；非零
error 让 `ok=false`。Warning 保留给仍可解释或可重建的软引用，例如历史 action 的
`result_atom_id` 已被当前 `label_atoms` rebuild 删除。

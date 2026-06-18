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

`label_atoms` 是从 `label_semantics` 与 label name 展开的稳定、可检索 atom truth。
它保存 positive 与 negative 两种 polarity，供后续 Group OMP/NNLS label solver 和
LanceDB atom retrieval 使用。

| 字段 | 说明 |
|---|---|
| `id` | 稳定 `la_...` atom id。 |
| `label_id` / `board_id` | 关联 canonical label 与 board。 |
| `polarity` | `positive` / `negative`。 |
| `kind` | `name`、`description`、`applies_when`、`positive_example`、`excludes_when`、`negative_example`；有 description 时，`description` atom 是 `label: {name}\ndescription: {description}` canonical atom，无 description 时才使用 `name` fallback atom。 |
| `text` | trim 且规范化 whitespace 后的 atom 文本；每个非空行内部 whitespace collapse，canonical 行分隔保留，空文本不入库。 |
| `ordinal` | 同一 label 展开后的顺序；同语义重复 atom 去重时保留首次出现的 ordinal。 |
| `content_hash` | atom 语义内容 hash，用于派生层判断变化；输入为 `label_id + polarity + kind + normalized_text`，不包含 `ordinal`。 |
| `created_at` / `updated_at` | atom truth 时间。 |

派生向量表：`kb_label_atoms`

`kb_label_atoms` 是 LanceDB 中的可重建 label atom 向量表，独立于 task chunk 表
`kb_chunks`。它按 `board_id`、`embedding_model`、`polarity` 查询 atom evidence，
返回 `label_id`、atom id、`polarity`、`kind`、`text` 和 LanceDB `_distance` 原始
distance 等字段。语义 label 候选会用返回的 atom vector 在本地重新计算
query/residual cosine similarity，不把 distance 当作 solver score。派生表损坏或缺少
provider 时只让 label atom index degraded，不影响普通 label CRUD、`task_labels` 绑定
或 task 状态机。

### 11.2 Label ontology ledger

Label ontology ledger 记录 task 标注过程里的证据、分歧 signal、review/action 历史
和 validation 结果。它是可查询的审计账本，不替代 canonical truth：

- `labels` / `task_labels` 仍决定 task 当前实际绑定哪些 label。
- `label_semantics` / `label_atoms` 仍决定 label 的 canonical 语义和 atom truth。
- `label_semantic_proposals` 仍负责新 label proposal lifecycle。

表：`label_ontology_observations`

一行表示一次完整的 task label 判断过程。它保存当时的 task 快照、agent 候选、
`label suggest` 快照、最终选择和 solver 指标；即使 task、label 或 atoms 后续变化，
仍能还原当时为什么产生 signal。

| 字段 | 说明 |
|---|---|
| `id` | `lor_...` observation id。 |
| `board_id` / `task_id` | 来源 board 与 task。 |
| `task_ref_snapshot` | 捕获时的人类 ref，例如 `default#42`。 |
| `task_snapshot_json` | 捕获时的 task title、description、labels、version/hash 等快照。 |
| `suggest_input_hash` | 可空。按 label suggest 输入（normalized title + description）计算的窄 hash，用于 validation comparability；旧 observation 缺失时按 legacy incomparable 处理，不能静默 passed。 |
| `agent_candidates_json` | agent 原始候选 labels、置信度和理由。 |
| `suggestion_snapshot_json` | 完整 suggestion 输出、参数、模型和 index 状态快照。 |
| `final_decision_json` | 最终接受、拒绝和未采用 labels 的判断。 |
| `suggest_coverage` / `suggest_coverage_cosine` / `suggest_residual_norm` | 可查询的 solver 指标。 |
| `suggest_needs_new_label` / `suggest_degraded` | 捕获时 suggestion 状态。 |
| `diagnostics_json` | suggestion diagnostics 数组。 |
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

表：`label_ontology_actions`

Action 是 append-only history，表示 reviewer/agent 实际确认、拒绝、修改 ontology 或
记录 validation 的动作。直接修改 label semantics 或接受 proposal 时，provenance
也写成 action。

| 字段 | 说明 |
|---|---|
| `id` | `loa_...` action id。 |
| `board_id` | board scope。 |
| `parent_action_id` | validation 等后续 action 指向被验证的 mutation action。 |
| `action_type` | `confirm`、`reject`、`supersede`、`resolve_no_change`、`add_positive_atom`、`add_negative_atom`、`update_semantics`、`create_label_proposal`、`bootstrap_label`、`rename_label`、`split_label`、`merge_labels`、`validate`。 |
| `reason` | 必填人工或 agent 理由。 |
| `target_label_id` / `result_label_id` | 修改目标与结果 label。 |
| `result_atom_id` / `result_atom_content_hash` | 新增或采用 atom 的软引用和稳定 hash。 |
| `result_proposal_id` | 关联的 `label_semantic_proposals`。 |
| `canonical_before_hash` / `canonical_after_hash` | 修改前后 canonical semantics hash。 |
| `change_json` | before/after/diff 或其它可解释变更快照。 |
| `validation_status` | `not_required`、`pending`、`passed`、`failed`、`partial`。 |
| `validation_json` | validation evidence；service 会包装 supplied payload、source signal cases 和 summary。`passed` action 需要 automated typed evidence（embedding model、solver options、clean atom index generation、per-signal before/after cases），并按 parent action 校验 positive atom、negative atom 或 bootstrap label policy；`failed` / `partial` 可保存诊断 payload。 |
| `created_by` / `created_by_type` / `agent_type` | action actor。 |
| `created_at` | 创建时间。 |

`result_atom_id` 故意不是强 FK。`label_atoms` 会随 semantics rebuild delete/insert；
历史 action 依赖 `result_atom_content_hash` 和 `change_json` 中的 atom snapshot 保持可解释。
Atom explain 查询会优先解析当前 `label_atoms.id`，也允许用
`result_atom_content_hash` / `label_atoms.content_hash` 作为软引用恢复 rebuild 后的历史
provenance。已有 atom 如果来自旧 semantics 写入而没有任何 ontology action 引用，
查询结果只标记 `legacy_untracked=true`，不会伪造 provenance。

表：`label_ontology_action_signals`

多对多连接 action 与 signals。多个 signals 可以支持一次 atom 修改；同一个 signal
也可以先被 confirm，随后关联 mutation action 和 validation action。

默认 review queue 只读取 `open` 与 `confirmed` signals；完整历史需显式 include all。
Mutation action 写入后通常保持 source signals 为 `confirmed`，validation 通过后再
转为 `resolved`。Validation 失败会追加 failed validation action，不删除历史。

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
| `heuristic_coverage` / `heuristic_coverage_cosine` / `heuristic_residual_norm` | 来自当前 residual label suggestion solver 的覆盖/残差元数据，用于记录 proposal 创建时现有 label atoms 的覆盖程度；`heuristic_coverage_cosine` 是 query 与 fitted vector 的 cosine similarity。 |
| `top1_existing_label_id` / `top1_existing_label_name` | 当前启发式 top1 existing label。 |
| `diagnostics_json` | JSON string array，包含 degraded、冲突或 validation 诊断。 |
| `decision_reason` / `resolved_label_id` / `decided_at` | accept/reject 决策信息；accept 后 `resolved_label_id` 指向新建 canonical label。 |

Accept 只允许 `proposed` proposal。accept 创建同 board 的 canonical `labels` 行，
并写入对应 `label_semantics` / `label_atoms`，同时标脏 `lancedb_label_atoms`
派生 store；它不写入 `task_labels`，不会把新 label 自动绑定到来源 task。

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

```json
{"type":"task","data":{...}}
{"type":"event","data":{...}}
{"type":"comment","data":{...}}
{"type":"dependency","data":{...}}
```

Label ontology ledger 使用稳定 record types：

```json
{"type":"label_ontology_observation","data":{...}}
{"type":"label_ontology_signal","data":{...}}
{"type":"label_ontology_action","data":{...}}
{"type":"label_ontology_action_signal","data":{...}}
```

导入时会在同一 transaction 中先插入 ontology rows，再延迟回填
`label_ontology_signals.superseded_by_signal_id` 与
`label_ontology_actions.parent_action_id`，避免依赖同表自引用 rows 的文件顺序。导入完成前会校验 ontology ledger board isolation：observation/signal board、action
parent board、action-signal link board、label/proposal soft reference board 必须一致；
orphan action-signal links、supersede cycles 和 action parent cycles 会导致 import
失败。

`kanban doctor --json` 对同一组 ontology ledger consistency 规则做只读巡检，并额外返回
`ontology_ledger_errors`、`ontology_ledger_warnings`、`ontology_ledger_issues[]`。Issue
包含 `severity`、`code`、`message`、`record_ids`，用于定位损坏 row。Hard error 覆盖
missing v12 ontology table、跨 board link、orphan action-signal link、parent/supersede
异常、label/proposal/task board mismatch、supersede cycle 和 action parent cycle；非零 error
让 `ok=false`。Warning 保留给仍可解释或可重建的软引用，例如历史 action 的
`result_atom_id` 已被当前 `label_atoms` rebuild 删除。

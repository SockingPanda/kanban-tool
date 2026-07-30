# 数据模型

本文件定义领域模型、SQLite 表、ID、时间、JSON、附件、事件与常用查询。

---

## 1. ID 规范

除预置看板列这类固定 ID 外，公开实体 ID 通常使用带前缀的 ULID（类 UUID 字符串），
便于在日志和 CLI 中区分。

| 对象 | 前缀 | 示例 |
|---|---|---|
| 看板（Board） | `b_` | `b_01HY...` |
| 任务（Task） | `t_` | `t_01HY...` |
| 步骤（Step） | `step_` | `step_01HY...` |
| 执行记录（Run） | `r_` | `r_01HY...` |
| 评论（Comment） | `c_` | `c_01HY...` |
| 附件（Attachment） | `a_` | `a_01HY...` |
| 标签（Label） | `l_` | `l_01HY...` |
| 看板列（Column） | `col_` | `col_ready` |
| 事件（Event） | `e_` | `e_01HY...` |

`task_events.event_id` 保存带 `e_` 前缀的公开事件 ID；`task_events.id` 是单独的自增整数，
用于 SSE 偏移量和顺序分页。

领取凭证不是实体 ID。`tasks.claim_token` 与 `task_runs.claim_token` 使用
`claim_...` 格式，例如 `claim_01HY...`；调用方必须把它视为临时凭证，不应当作可公开枚举的
稳定身份。

---

## 2. 时间规范

所有时间字段使用：

```text
INTEGER，UTC Unix 时间戳（毫秒）
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

Rust 内部建议使用 `time::OffsetDateTime`，在数据库边界转换为以毫秒表示的 `i64`。

---

## 3. JSON 字段规范

SQLite 中 JSON 存 `TEXT`，必须满足：

```sql
CHECK(json_valid(field_name))
```

默认值：

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{}
```

用途：

| 字段 | 说明 |
|---|---|
| `tasks.metadata_json` | 轻量扩展信息。 |
| `task_runs.metadata_json` | worker 配置、环境、命令摘要等。 |
| `task_events.payload_json` | 事件载荷。 |

禁止把大对象、完整 stdout/stderr 日志或附件二进制内容放进 JSON。

---

## 4. 看板（Board）

看板（Board）表示本地项目或看板，不是租户。

主要字段：

| 字段 | 说明 |
|---|---|
| `id` | 带 `b_` 前缀的 ID。 |
| `slug` | CLI 和 Web 使用的人类可读短名。 |
| `name` | 展示名。 |
| `description` | 可选说明。 |
| `created_at` | 创建时间。 |
| `updated_at` | 更新时间。 |
| `archived_at` | 归档时间。 |

默认看板：

```text
default
```

Board slug 由服务层校验：必须唯一、非空、不超过 64 字节，以小写 ASCII 字母或数字开头，只能包含小写 ASCII 字母、数字、`.`、`_`、`-`，并且不能使用 `b_`、`t_`、`r_`、`c_`、`a_`、`l_`、`col_`、`e_` 等保留前缀。这样可以避免与公开 ID、`board#seq` 任务引用和路径式别名语法冲突。

已归档的看板默认不出现在看板列表中，也不接受普通任务、评论或实验性 dispatcher 写入。归档只设置看板的 `archived_at` 并写入 `board.archived` 事件，不改变任务状态；如果看板上仍有 `running` 任务或 `running` 执行记录，归档会被拒绝。事件、执行记录、评论等只读历史仍可通过明确的任务或看板身份查询，用于审计。

### 4.1 看板隔离的责任边界

SQLite 是权威事实来源，但看板隔离由数据库结构、服务和诊断门禁共同保证：

1. 数据库约束：所有看板作用域内的行都有 `board_id` 并引用 `boards(id)`；
   被引用的任务、标签和执行记录 ID 也各自有外键，确保引用对象存在。`task_labels`、
   `task_dependencies`、`task_execution_plans`、`task_runs`、`task_comments`、
   `task_attachments` 和较新的标签语义、atom 与本体链接表使用包含 `board_id`
   的复合外键，直接阻止这些关系表出现跨看板行。`task_steps` 的父任务也由复合外键
   约束；可选的 `linked_task_id` 只有普通任务外键，其同看板约束由服务守卫与诊断或
   导入门禁负责。`task_events` 保留可空的 task/run 引用和
   `ON DELETE SET NULL` 语义，由 INSERT/UPDATE 触发器校验非空引用的看板作用域。
2. 服务守卫：CLI、HTTP、桌面端和实验性 dispatcher 的正常写路径必须先在同一看板
   作用域内解析 task、label、run 等对象，再写入关系行；例如任务标签绑定、
   依赖、评论、事件、run 和附件都不应跨看板组合。
3. Doctor 和导入检查：`kanban doctor` 与 JSONL 导入的最终门禁会只读检查基础关系表
   中 `row.board_id` 与被引用任务、标签、执行记录所属看板是否一致，并运行
   `PRAGMA foreign_key_check`。任何违规都会成为严重错误；导入会在
   提交前回滚整个替换事务。

---

## 5. 任务（Task）

任务（Task）是核心对象，既是看板卡片，也是可执行工作单元。

### 5.1 字段分组

#### 身份

| 字段 | 说明 |
|---|---|
| `id` | 任务 ID。 |
| `board_id` | 所属看板。 |
| `seq` | 看板内递增数字，便于显示 `board#12`。 |

任务的公开身份分为两层：

- `id` 是全局唯一的 `t_...`，可跨看板直接定位任务。
- `seq` 只在同一看板内唯一，CLI/API 展示时应组合成 `board_slug#seq`，例如 `agent-work#12`。

#### 内容

| 字段 | 说明 |
|---|---|
| `title` | 必填。 |
| `description` | Markdown 文本。 |
| `status_reason` | 阻塞等状态原因。 |
| `result_summary` | 完成摘要。 |
| `result_json` | 完成结果的自然 JSON；存储值必须是合法 JSON，CLI/API 公开为解码后的 `result`。 |
| `metadata_json` | 扩展字段。 |

#### 工作流

| 字段 | 说明 |
|---|---|
| `status` | 权威状态。 |
| `priority` | 类枚举的整数优先级：`0` = 最高的 P0，`1` = P1，`2` = P2，`3` = 最低且默认的 P3。数据库默认值是 `3`，迁移后由 `CHECK(priority BETWEEN 0 AND 3)` 约束。创建和更新命令会拒绝 P0—P3 之外的值。 |
| `position` | UI 排序键。 |
| `scheduled_at` | 计划时间。 |
| `due_at` | 截止时间，仅展示/过滤，不驱动状态机。 |
| `retry_count` | 已重试次数。 |
| `max_retries` | 最大重试次数。 |

#### 操作者与执行

| 字段 | 说明 |
|---|---|
| `assignee` | 人或 worker 配置名称。 |
| `created_by` | 操作者字符串。 |
| `claim_token` | 当前领取凭证，格式为 `claim_...`。 |
| `claim_owner` | 当前领取者。 |
| `claim_expires_at` | 领取过期时间。 |
| `last_heartbeat_at` | 最近心跳时间。 |
| `current_run_id` | 当前或最近的 run ID。 |

#### 时间戳

| 字段 | 说明 |
|---|---|
| `created_at` | 创建。 |
| `updated_at` | 更新。 |
| `started_at` | 首次进入 running。 |
| `completed_at` | 完成。 |
| `archived_at` | 归档。 |

#### 并发

| 字段 | 说明 |
|---|---|
| `lock_version` | 乐观锁版本。 |

### 5.2 优先级语义

`priority` 表示任务的相对重要性和排序权重，不表示状态机可执行性。`ready`
表示任务已经由人工或服务明确放入可领取队列；P0—P3 只影响列表排序，以及内部实验性 dispatcher 在候选任务之间的排序。

优先级约定：

| 优先级 | 语义 | 示例 |
|---|---|---|
| `0` / P0 | 事故、阻断当前目标或必须立即处理的任务。应当少量使用，不作为普通 `ready` 任务的默认值。 | 修复导致本地队列无法领取任务的回归；解除发布前的 P1/P0 审查阻塞。 |
| `1` / P1 | 近期工作焦点，当前迭代或当前工作流应优先完成。 | 今天要完成的实现切片；当前 PR 必须补齐的测试。 |
| `2` / P2 | 重要的后续任务，但不阻塞当前主线。 | 整理文档示例；补充非关键冒烟测试。 |
| `3` / P3 | 普通待办、低优先级或默认值。 | 想法、低风险清理、未来可做的体验改进。 |

`ready` 与 P0 不能互相替代：

- 普通可执行任务应是 `ready` + P1/P2/P3，而不是为了进入队列全部标成 P0。
- P0 任务如果仍缺规格、排期未到或依赖未完成，仍不能被领取；它应保持
  `triage`、`scheduled` 或 `todo`，直到满足状态机守卫后再提升到 `ready`。
- 内部实验性 dispatcher 只领取 `ready` 任务；只有在多个 `ready` 任务之间，才按
  P0 到 P3 排序。这不是当前公开支持的使用路径。

---

## 6. 依赖（Dependency）

表：`task_dependencies`

数据库结构不变量：`parent_task_id` 和 `child_task_id` 必须都属于该行的
`board_id`。旧数据库升级到复合外键结构前会先检查已有的跨看板行；
发现不一致时迁移会失败，并要求先用 doctor/repair 清理。

字段：

| 字段 | 说明 |
|---|---|
| `parent_task_id` | 前置任务。 |
| `child_task_id` | 被阻塞任务。 |
| `board_id` | 两个任务共同所属的看板。 |
| `created_at` | 创建时间。 |

语义：

```text
前置任务为 done 或 archived => 后续任务可以变为 ready
前置任务既不是 done 也不是 archived => 后续任务不能进入 ready/running
```

添加依赖时必须做环检测。归档前置任务会满足强依赖守卫，但依赖边会作为历史保留，也不会自动提升后续任务。

前置任务从 `done` 重新打开后，直接后续任务中只有 `triage|todo|scheduled|ready` 会按就绪条件重新计算；`running|blocked|review|done|archived` 不会被隐式改写。


---

## 7. 步骤与执行计划（Step / Execution Plan）

步骤（Step）是父任务内部的有序执行步骤，不是阻塞依赖关系。Step 可以是普通文本，
也可以链接到另一个普通任务作为上下文。链接任务不会自动创建
`task_dependencies` 边，也不会根据所链接任务的状态自动完成 step；step 自己有独立的
`todo | done | skipped` 状态。

### 7.1 步骤

表：`task_steps`

数据库结构通过复合外键保证 `parent_task_id` 属于该行的 `board_id`。可选的
`linked_task_id` 只有指向 `tasks(id)` 的普通外键；服务与诊断或导入门禁必须另外保证
它属于同一看板，且不能等于 `parent_task_id`。服务还必须拒绝已归档的父任务、
已归档的链接任务、空白标题和跨看板链接。

字段：

| 字段 | 说明 |
|---|---|
| `id` | Step ID，格式为 `step_...`。 |
| `board_id` | 所属看板。 |
| `parent_task_id` | 被规划的父任务。 |
| `position` | 父任务内步骤排序键。 |
| `title` | 步骤标题。 |
| `body` | 可选说明文本。 |
| `linked_task_id` | 可选的上下文任务。 |
| `required` | 是否阻止父任务完成或归档。 |
| `status` | `todo`、`done` 或 `skipped`。 |
| `resolution_note` | 完成、跳过或重新打开的说明。 |
| `resolved_by` | 最近一次处理的操作者。 |
| `resolved_at` | 最近一次处理时间。 |
| `created_by` | 创建者。 |
| `created_at` | 创建时间。 |
| `updated_by` | 最近更新者。 |
| `updated_at` | 最近更新时间。 |

索引：

- `idx_steps_parent_position(parent_task_id, position)`
- `idx_steps_linked_task(linked_task_id)`
- `idx_steps_board_status(board_id, status)`

语义：

```text
父任务包含有序步骤
可选的 linked_task_id 只提供任务上下文
```

Step 不会直接驱动 `dependency_blocked` 或 `unfinished_parent_count`。必需步骤
只参与执行计划守卫：父任务不能完成或归档，直到所有必需步骤
都是 `done` 或 `skipped`。

### 7.2 执行计划

表：`task_execution_plans`

字段：

| 字段 | 说明 |
|---|---|
| `board_id` | 所属看板。 |
| `task_id` | 被规划的任务。 |
| `state` | `unplanned`、`planned` 或 `not_required`。 |
| `reason` | `not_required` 的说明。 |
| `updated_by` | 最近更新者。 |
| `updated_at` | 最近更新时间。 |

索引：

- `idx_execution_plans_board_state(board_id, state)`

派生口径：

```text
步骤数量 > 0 => planned
存在明确的 not_required 行且没有步骤 => not_required
其他情况 => unplanned
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

## 8. 执行记录（Run）

表：`task_runs`

数据库结构不变量：`task_id` 必须属于该行的 `board_id`。这保证一次 run 尝试
不能在 SQLite 层跨看板指向任务。

执行记录（Run）表示一次执行尝试。

### 8.1 Run 状态

```text
running | succeeded | failed | canceled | expired
```

### 8.2 字段

| 字段 | 说明 |
|---|---|
| `id` | 带 `r_` 前缀的 ID。 |
| `board_id` | 所属看板。 |
| `task_id` | 关联任务。 |
| `status` | run 状态。 |
| `worker_profile` | worker 配置名称。 |
| `worker_pid` | 可选、预留的本机 PID；当前内部实验性 dispatcher 不填充此字段。 |
| `claim_token` | 对应的领取凭证，格式为 `claim_...`。 |
| `claim_owner` | 本次领取的操作者。 |
| `claim_expires_at` | 本次领取的过期时间。 |
| `started_at` | run 开始。 |
| `last_heartbeat_at` | 最近心跳时间。 |
| `finished_at` | run 结束。 |
| `exit_code` | worker 退出码。 |
| `summary` | 简短摘要。 |
| `error` | 错误文本。 |
| `log_path` | stdout/stderr 日志路径。 |
| `metadata_json` | 执行元数据。 |

### 8.3 约束

- 当前为 `running` 的任务必须有当前 run。
- 一个任务可以有多个历史 run。
- 同一任务同时最多有一个 `running` run。

最后一条不由 SQLite 直接强制，需要服务层和事务共同保证。

---

## 9. 事件（Event）

表：`task_events`

事件（Event）是只追加的事实记录。

### 9.1 事件类型

API/SSE 当前已类型化的 39 个已知类型：

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

### 9.2 载荷示例

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{
  "claim_owner": "alice",
  "metadata": {}
}
```

`task_events.kind/payload_json` 的 SQLite 存储允许未来出现未知类型。事件 API 与 SSE
对上面 39 个已知类型使用精确的同级载荷契约，已知类型不匹配时按失败关闭处理；未知
类型的合法 JSON 载荷保持无损。外层 `task_id`、`run_id`、`actor` 都是
必需但可空的字段。可移植 JSONL 的事件载荷仍是不透明 JSON，不复用这组类型化联合。

### 9.3 使用场景

- 任务详情时间线。
- SSE 事件流。
- 调试领取与执行记录。
- CLI `kanban events`。
- 导出与导入。

---

## 10. 评论（Comment）

表：`task_comments`

字段：

| 字段 | 说明 |
|---|---|
| `id` | 评论 ID。 |
| `task_id` | 关联任务。 |
| `board_id` | 关联看板。 |
| `author` | 操作者字符串。 |
| `author_type` | `user` / `agent`，表示评论作者身份；本地操作者是 `user`，其他自动化来源是 `agent`。 |
| `agent_type` | 可选的开放文本，仅用于 `author_type=agent`，例如 `executor` / `reviewer`。 |
| `body` | Markdown 文本。 |
| `kind` | `note` / `decision` / `signal`，表示评论内容语义，不表示作者身份。`signal` 是信号账本的反向链接。 |
| `metadata_json` | `kind` 对应的结构化载荷；默认 `{}`，必须是合法 JSON 对象。`kind=decision` 时必须符合决策结构。`kind=signal` 的反向链接元数据包含 `type:"signal_link"`、`signal_id`、`observation_id`、`signal_kind`、`signal_status`。 |
| `created_at` | 创建时间。 |

`(task_id, board_id)` 通过复合外键关联 `tasks(id, board_id)`，因此评论不能跨看板挂接任务。

旧评论行或 JSONL 导入记录会迁移到新语义：旧 `human` 变为 `user`，旧 `agent/system` 或 `worker/system` 来源变为 `agent`，旧 `text/system/worker` 内容变为 `note`。没有结构化元数据的旧 `decision` 也按 `note` 保留正文作为回退。

创建评论时也会写入一条 `task_events(kind='task.comment.created')`。

`metadata_json` 是 SQLite 的权威存储列；CLI/API 响应会把它解码成自然、无损的
`metadata` 对象。普通 `note`/`signal` 元数据保持开放。只有服务生成的反向链接
完整结构由 `SignalLinkMetadataOutput` 独立证明，不能把用户自定义的同名键碰撞当成协议。

决策评论的元数据结构：

- `options`：非空数组。
- 每个选项都是对象，且包含非空字符串 `slug`、`title`、`detail`。
- `slug` 必须是稳定的小写 ASCII 短名：以小写字母或数字开头，只包含小写字母、数字和 `-`；在同一决策内唯一。
- `selected`：非空字符串，必须匹配某个选项的 `slug`。
- `reason`：非空字符串。
- `risk` / `verification`：可选；如果出现，必须是非空字符串。
- 未知顶层字段允许保留，但不参与状态机、内部实验性 dispatcher 或事件语义。

---

## 11. 附件（Attachment）

二进制内容不存入数据库。

附件默认保存在数据库目录下：

```text
<db_dir>/attachments/<board_id>/<task_id>/<attachment_id>/<filename>
```

例如，在使用常见 Linux 默认数据库目录时，路径通常为
`~/.local/share/kb/attachments/<board_id>/<task_id>/<attachment_id>/<filename>`。

数据库记录：

| 字段 | 说明 |
|---|---|
| `id` | 附件 ID。 |
| `task_id` | 关联任务。 |
| `board_id` | 关联看板。 |
| `filename` | 原始文件名。 |
| `rel_path` | 相对数据目录的路径。 |
| `content_type` | MIME 类型。 |
| `size_bytes` | 大小。 |
| `sha256` | 内容哈希。 |
| `created_by` | 操作者。 |
| `created_at` | 上传时间。 |

`(task_id, board_id)` 通过复合外键关联 `tasks(id, board_id)`，因此附件不能跨看板挂接任务。

安全要求：

- `filename` 必须经过安全清理。
- `rel_path` 必须位于数据目录内。
- 不允许通过 `../` 进行路径穿越。

---

## 12. 标签（Label）

标签（Label）用于轻量分类。

字段：

| 字段 | 说明 |
|---|---|
| `id` | 标签 ID。 |
| `board_id` | 所属看板。 |
| `name` | 标签名。 |
| `color` | UI 颜色标记。 |
| `created_at` | 创建时间。 |
| `updated_at` | 更新时间。 |

同一看板内标签名唯一。

任务与标签的关联通过 `task_labels(task_id, label_id, board_id, created_at)` 关联表表达。
两条复合外键分别约束任务和标签都属于 `board_id` 指定的看板，不能跨看板绑定。
标签只用于分类、过滤和展示；添加或移除标签不改变 `tasks.status`，
不触发依赖重新计算，也不会让内部实验性 dispatcher 领取 `review` 或其他非
`ready` 状态。

### 12.1 标签语义

`labels` 仍是权威的标签身份表：名称、颜色和看板作用域由 `labels`
定义。`task_labels` 仍是任务最终绑定标签的事实。语义推荐和向量检索使用
额外的事实表，不替代这两张表。
`labels` 的身份增删改查属于基础词表登记，不写入本体变更账本；
`label delete` 不会隐式删除语义或 atom，必须先通过受 CAS 保护的语义清理流程
清空语义。

表：`label_semantics`

| 字段 | 说明 |
|---|---|
| `label_id` | 关联 `labels(id)`，一个标签最多有一条语义记录。 |
| `board_id` | 冗余的看板作用域，用复合外键保证标签与看板一致。 |
| `description` | 标签的自然语言说明。 |
| `applies_when` | JSON 字符串数组，正向适用条件。 |
| `excludes_when` | JSON 字符串数组，反向排除条件。 |
| `positive_examples` | JSON 字符串数组，正向示例。 |
| `negative_examples` | JSON 字符串数组，反向示例。 |
| `created_at` / `updated_at` | 语义记录时间。 |

表：`label_atoms`

`label_atoms` 是从 `label_semantics` 与标签名展开的 SQLite 物化投影。
它保存 `positive` 与 `negative` 两种极性，供后续 Group OMP/NNLS 标签求解器和
LanceDB atom 检索使用；它随语义变更在同一事务内重建，不是独立于
`label_semantics` 的第二份语义事实。

| 字段 | 说明 |
|---|---|
| `id` | 稳定的 `la_...` atom ID。 |
| `label_id` / `board_id` | 关联权威标签与看板。 |
| `polarity` | 极性：`positive` / `negative`。 |
| `kind` | `name`、`description`、`applies_when`、`positive_example`、`excludes_when`、`negative_example`；有说明时，`description` atom 是 `label: {name}\ndescription: {description}` 形式的权威 atom，没有说明时才使用 `name` 回退 atom。 |
| `text` | 去除首尾空白并规范化空白后的 atom 文本；每个非空行内部的空白会折叠，权威行分隔保留，空文本不入库。 |
| `ordinal` | 同一标签展开后的顺序；同语义的重复 atom 去重时保留首次出现的 `ordinal`。 |
| `content_hash` | atom 语义内容哈希，用于派生层判断变化；输入为 `label_id + polarity + kind + normalized_text`，不包含 `ordinal`。 |
| `created_at` / `updated_at` | 投影行时间。 |

派生向量表：`kb_label_atoms`

`kb_label_atoms` 是 LanceDB 中可重建的标签 atom 向量表，独立于任务分块表
`kb_chunks`。它按 `board_id`、`embedding_model`、`polarity` 查询 atom 证据，
返回 `label_id`、atom ID、`polarity`、`kind`、`text` 和 LanceDB 原始
`_distance` 等字段。语义标签候选会使用返回的 atom 向量，在本地重新计算
查询向量与残差的余弦相似度，不把距离当作求解器分数。派生表损坏或缺少
提供方时，只会让标签 atom 索引降级，不影响普通标签增删改查、`task_labels` 绑定
或任务状态机。

### 12.2 通用信号账本

通用信号账本保存 agent 或产品在 kanban 工作流中发现的通用问题，
例如 CLI 参数使用不顺、提示误导、参数设计不符合 agent 惯用方式，或操作者发现的
产品反馈。它是看板作用域内的审计账本和只读收件箱数据源，不替代 `tasks.status`、
任务评论、run、事件或标签本体账本。

- `signal_observations` 保存一次观察的来源、操作者、task/run/comment 关联和原始证据。
- `signals` 保存一个可以独立审查的通用信号，并指向对应 observation。
- 通用信号与 `label_ontology_signals` 分离；本体信号仍只服务于标签
  语义、atom、提案审查和变更来源追踪。
- 当前公开 HTTP 接口只读取通用信号；生命周期写操作仍由 CLI/runtime
  的信号记录流程负责。
- 看板作用域内的列表和审查接口只通过 board 路由读取：
  `/api/v1/boards/{board}/signals*`。单条详情
  `GET /api/v1/signals/{signal_id}` 是面向操作者的全局详情查询，用于从
  反向链接或收件箱行直接打开已知信号；它不改变信号的 `board_id`
  事实，也不会把信号混入其他看板的列表。
- `signal_observations.task_id`、`run_id`、`comment_id` 是用于来源和历史的
  软引用。当前一致性由服务写入路径、doctor 和导入最终门禁维护；
  这些引用允许保留历史来源语义。未来如需把全部来源关系硬化，可迁移为
  带看板作用域的复合外键。

表：`signal_observations`

一行表示一次 agent 或操作者的观察。Observation 可关联 task、run 或 comment；
这些关联用于定位来源，不改变对应实体状态。

| 字段 | 说明 |
|---|---|
| `id` | `obs_...` 观察记录 ID。 |
| `board_id` | 来源看板作用域。 |
| `task_id` / `task_ref_snapshot` | 可空。来源任务与捕获时的人类可读引用快照；任务后续改动不影响快照。 |
| `run_id` | 可空。来源执行 run。 |
| `comment_id` | 可空。来源评论。 |
| `actor` / `agent_type` | 捕获者名称与可选的 agent 类型。 |
| `source` | 可空。信号来源，例如 `codex-hook`、`cli` 或 `operator`。 |
| `evidence_json` | JSON 对象字符串，保存命令、stderr、上下文片段、hook 提示等原始证据。 |
| `created_at` | 创建时间。 |

表：`signals`

一行表示一个可以独立进入操作者收件箱的通用信号。它只描述发现的问题和审查
生命周期，不直接触发修复，也不修改权威工作流。

| 字段 | 说明 |
|---|---|
| `id` | `sig_...` 信号 ID。 |
| `board_id` / `observation_id` | 看板作用域与来源 observation。 |
| `kind` | 通用信号类型，例如 `agent_cli_friction`。 |
| `title` / `summary` | 面向操作者的短标题与摘要。 |
| `severity` | 文本严重度，例如 `info`、`medium` 或 `high`。 |
| `status` | `open`、`confirmed`、`resolved`、`rejected`、`superseded`。 |
| `dedupe_key` | 可空。用于调用方聚合相似信号。 |
| `superseded_by_signal_id` | 可空。指向同一看板中的替代信号。 |
| `reviewed_by` / `reviewed_at` / `review_reason` | 生命周期审查记录。 |
| `created_at` / `updated_at` | 创建与更新时间。 |

默认审查队列只读取 `open` 与 `confirmed` 信号；完整历史需要明确设置
`include_all` 或指定状态。

### 12.3 标签本体账本

标签本体账本记录任务标注过程中的证据、分歧信号、审查与操作历史
以及验证结果。它是可查询的审计账本，不替代权威事实：

- `labels` / `task_labels` 仍决定任务当前实际绑定哪些标签。
- `label_semantics` 决定标签的权威语义；`label_atoms` 是它的 SQLite
  物化 atom 投影。
- `label_semantic_proposals` 仍负责新标签提案的生命周期。
- 本体账本覆盖语义和 atom 变更；基础 `labels` 身份的增删改查位于
  账本之外，只写普通事件。

这些表在标签系统中承担不同角色，不是六个严格独立的存储层。`label suggest` 是计算结果，
`kb_label_atoms` 是可重建的检索投影，提案和账本是需要持久审计的 SQLite 记录；
它们都不能直接替代 `task_labels` 的当前绑定事实。

表：`label_ontology_observations`

一行表示一次完整的任务标签判断过程。它保存当时的任务快照、agent 候选、
`label suggest` 快照、最终选择和由快照派生的求解器指标；即使任务、标签或
atom 后续变化，仍能还原当时为什么产生信号。Observation 是只读的来源记录：
写入记录不会修改 `task_labels`、`label_semantics`、`label_atoms`、标签 atom 索引或
提案。

| 字段 | 说明 |
|---|---|
| `id` | `lor_...` 观察记录 ID。 |
| `board_id` / `task_id` | 来源看板与任务。 |
| `task_ref_snapshot` | 捕获时的人类 ref，例如 `default#42`。 |
| `task_snapshot_json` | 捕获时的任务标题、说明、标签、版本和哈希等快照。 |
| `suggest_input_hash` | 可空。按标签建议输入（规范化标题 + 说明）计算的窄哈希，用于验证可比性；旧 observation 缺失时按旧版不可比较处理，不能静默标记为通过。 |
| `agent_candidates_json` | agent 原始候选标签、置信度和理由。 |
| `suggestion_snapshot_json` | 完整的建议输出、参数、模型和索引状态快照；新的捕获路径要保存未经改写的原始快照。 |
| `final_decision_json` | 对最终接受、拒绝和未采用标签的判断。 |
| `suggest_coverage` / `suggest_coverage_cosine` / `suggest_residual_norm` | 可查询的求解器指标。新的捕获路径从 `suggestion_snapshot_json` 派生这些值；调用方不应重复手写。`suggest_coverage = clamp(1 - suggest_residual_norm, 0.0, 1.0)`，二者不是独立证据；`suggest_coverage_cosine` 是查询向量与拟合向量的余弦相似度，可作为补充指标。 |
| `suggest_needs_new_label` / `suggest_degraded` | 捕获时的建议状态。新的捕获路径从 `suggestion_snapshot_json` 派生这些值。`suggest_needs_new_label` 是覆盖审查的兼容字段，不等于自动发现词表缺口；判断是否需要新标签还要结合原因代码、证据、诊断和人工语义判断。 |
| `diagnostics_json` | 建议诊断数组。新的捕获路径从快照的 `diagnostics` 派生；冲突的重复输入会被拒绝。 |
| `capture_fingerprint` | 同一看板内的幂等指纹。 |
| `created_by` / `created_by_type` / `agent_type` | 捕获者身份。 |
| `created_at` | 创建时间。 |

表：`label_ontology_signals`

一行只表达一个可独立审查的本体问题，例如某个已有标签漏选、
建议误选、存在词表缺口或标签边界、名称问题。

| 字段 | 说明 |
|---|---|
| `id` | `los_...` 信号 ID。 |
| `observation_id` / `board_id` | 来源 observation 与看板作用域。 |
| `kind` | `false_negative`、`false_positive`、`vocabulary_gap`、`name_issue`、`boundary_issue`、`structure_issue`。 |
| `status` | `open`、`confirmed`、`resolved`、`rejected`、`superseded`。 |
| `target_label_id` / `target_label_name_snapshot` | 已有标签目标；名称快照用于历史解释。 |
| `related_labels_json` | 拆分、合并等多标签关系快照。 |
| `proposed_action` | `observe`、`add_positive_atom`、`add_negative_atom`、`update_semantics`、`bootstrap_label`、`rename_label`、`split_label`、`merge_labels`。 |
| `candidate_atom_polarity` / `candidate_atom_kind` / `candidate_text` | 建议 atom 的极性、类型和泛化文本。 |
| `candidate_content_hash` | 按 `label_id + polarity + kind + normalized_text` 计算的聚合键。 |
| `proposed_label_name` / `proposed_label_name_normalized` | 词表缺口或重命名候选。 |
| `proposal_json` | 新标签或结构变更的候选语义快照。 |
| `agent_selected` / `suggest_state` / `suggest_score` / `suggest_rank` / `final_selected` | agent、建议与最终判断之间的分歧证据。 |
| `rationale` / `confidence` | 可审查理由和可选置信度。 |
| `signal_key` | observation 内的幂等键。 |
| `superseded_by_signal_id` / `status_reason` | 关闭或替代原因。 |
| `created_at` / `updated_at` / `reviewed_at` / `closed_at` | 生命周期时间。 |

`label ontology review`（标签本体审查）是基于信号的只读聚合投影，不是新的权威事实，也不是
新的可持久化派生存储。分组键来自调用方选择的维度：`label` 使用目标标签，
`proposed-label` 使用规范化后的候选标签名，`candidate-atom` 优先使用
`candidate_content_hash`。没有候选
atom 的信号不会进入一个全局空值分组；回退键会带上信号类型、目标
标签或候选标签，以及候选操作，例如
`no-candidate-atom|kind:vocabulary_gap|proposed:ontology ledger|action:bootstrap_label`。
因此一个分组的含义是“这些信号共享同一个审查键”，不是“这些信号已被证明
来自同一个根因”。

`cluster` 是需要明确启用的重复信号审查辅助功能；它默认关闭，不写入权威 atom，不会自动
确认、应用、验证或变更，也不成为 SQLite 事实。每次审查查询时，聚类键都会从
已有信号文本和审查范围重建：键始终包含信号类型、候选操作、目标
标签快照（或 ID 回退）以及候选标签范围，再依次附加词法规范化后的候选文本、
候选标签、理由，最后回退到纯范围组合。这个范围前缀避免把文本相同但标签边界或操作
不同的信号强制合并；输出中的 `cluster_key` 和 `cluster_reason` 只用于解释辅助分组来源。

审查队列默认使用不同来源任务数（`task_count`）作为主要热度指标，
再按已确认数量、最近信号时间和键排序。`signal_count` 只是分组内的原始
信号行数；同一任务可以贡献多条信号，所以它不能单独代表模型错误率、准确率、
召回率或标签建议质量。需要质量指标时必须另有分母，例如一致性队列
或固定评估集。

`label ontology quality` 是一个只读分析投影，不新增表，也不写权威事实。
它把 `label_ontology_observations` 作为分母来源，并在输出中记录来源、不同
任务数、observation 数、一致或降级的 observation 数、时间范围和任务引用样本；
同时把 `label_ontology_signals` 作为原始分歧分子来源，按类型和状态
给出原始信号数量。只有当分母中存在一致的 observations 时，才会给出
`disagreement_task_rate`；只有信号的数据集会明确返回比率不可用，避免把分歧
记录误称为错误率。准确率和召回率仍需要带预期标签的独立评估队列，当前
账本信号不能单独提供这些指标。

长期标签本体回归语料集属于测试和评估基础设施，不是新的 SQLite 事实。
当前固定语料集测试使用临时数据库和内存标签 atom 索引，跟踪重要标签的已知
正向和负向对照任务，并比较 `label suggest` 选中的标签、分数与证据 atom。
语料集运行本身应只读权威本体；只有测试中明确模拟的临时语义或 atom 变更，
才用于证明比较能够发现回归。真实数据库上的长期语料集需要等稳定任务集积累后再扩展，
不应替代账本信号、可信验证或人工审查。

当前没有标签本体专属的图投影。`label_ontology_*` 表本身就是 SQLite
来源事实；`kanban graph` / Oxigraph 只投影知识派生底座的
`entity_relations`，不保存也不拥有标签本体操作或信号事实。若未来出现明确的
重命名、拆分、合并或来源关系查询需求，新增投影必须从
`labels`、`label_semantics`、`label_atoms`、`label_semantic_proposals` 和
`label_ontology_*` 重建，并通过 `index_outbox` / `derived_store_state` 表达标脏、同步、
重建和错误状态；删除或损坏图存储不得改变权威的标签、本体或账本行。

表：`label_ontology_actions`

Action 是只追加的历史，表示审查者或 agent 实际确认、拒绝、修改本体或
记录验证的操作。直接修改标签语义或接受提案时，来源信息
也写成操作记录。

| 字段 | 说明 |
|---|---|
| `id` | `loa_...` 操作 ID。 |
| `board_id` | 看板作用域。 |
| `parent_action_id` | 验证等后续操作指向被验证的变更操作。 |
| `action_type` | `confirm`、`reject`、`supersede`、`resolve_no_change`、`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、`update_semantics`、`create_label_proposal`、`bootstrap_label`、`rename_label`、`split_label`、`merge_labels`、`validate`、`revert_ontology_mutation`。 |
| `reason` | 必填的人工或 agent 理由。 |
| `target_label_id` / `result_label_id` | 修改目标与结果标签。 |
| `result_atom_id` / `result_atom_content_hash` | 新增或采用 atom 的软引用和稳定哈希。 |
| `result_proposal_id` | 关联的 `label_semantic_proposals`。 |
| `canonical_before_hash` / `canonical_after_hash` | 修改前后权威语义的哈希。 |
| `change_json` | 修改前、修改后、差异或其他可解释的变更快照。 |
| `validation_requirement` | `none`、`required`、`unsupported`。表示父级变更是否需要类型化验证策略；不改写历史尝试结果。 |
| `validation_status` | `not_required`、`pending`、`passed`、`failed`、`partial`。对父级变更表示历史兼容或基础状态；对 `validate` 操作表示一次尝试结果。 |
| `validation_json` | 验证证据封装；服务会包装调用方提供或工具采集的载荷、来源信号用例、任务快照可比性、父级操作结果引用和摘要。公开的提供或采集载荷只保存在顶层 `manual`；生成的 `cases[]` 用 `after.manual_case_ref` 指向 `manual.cases[]` 中对应信号的证据，避免把同一载荷复制到每个用例。`failed` / `partial` 可保存外部或人工证明的诊断。`passed` 操作只能来自工具采集的 `trusted_automated` 证据（采集器来源、嵌入模型、求解器选项、干净的 atom 索引状态与代次、每个信号修改前后的用例），并按父级操作校验正向 atom、负向 atom、标签引导创建，以及负向正例对照或豁免策略；调用方手写 JSON 或自称 `automated` 不构成可信来源。 |
| `created_by` / `created_by_type` / `agent_type` | 操作者身份。 |
| `created_at` | 创建时间。 |

`validation_effective_outcome` 是读取 DTO 时归并计算的结果，不是独立存储列。它按
`validation_requirement` 和最近的验证子操作（`created_at,id`）计算：
`not_required`、`unsupported`、`pending`、`passed`、`failed` 或 `partial`。只有
`required + trusted passed` 会处理已链接的来源信号；`unsupported` 可以记录
外部的失败或部分成功诊断，但拒绝 `passed`。

`label_ontology_action_atom_effects` 连接一条根变更操作与本次实际新增或删除的
atom 快照。它保存 `board_id`、`action_id`、`label_id_snapshot`、`atom_id_snapshot`、
`atom_content_hash`、`polarity`、`kind`、`text`、`effect` 和 `created_at`；`effect` 只允许
`added` / `removed`，唯一约束为 `(action_id, atom_content_hash, effect)`。操作记录使用
带看板作用域的复合外键；atom 快照不使用实时外键，因为 `label_atoms` 会随投影
重建。

`result_atom_id` 有意不使用强外键。`label_atoms` 会随语义重建而删除再插入；
历史操作和影响记录依赖 `result_atom_content_hash`、影响行与 `change_json` 中的 atom
快照保持可解释。Atom 解释查询会优先使用
`label_ontology_action_atom_effects`，也允许用旧版 `result_atom_id` /
`result_atom_content_hash` 兼容旧数据。`adopt_existing_atom` 表示新的来源信号采用了当前已存在的 atom，
不代表权威内容新增。已有 atom 如果来自旧语义写入而没有任何本体操作引用，
查询结果只标记 `legacy_untracked=true`，不会伪造来源记录。

同一 `(board_id, result_proposal_id)` 只能有一条 `create_label_proposal` 操作；接受提案
生成的 `bootstrap_label` 操作通过 `parent_action_id` 指向这条创建
操作，从而让“创建提案 → 引导接受”的来源链路保持无歧义。

`revert_ontology_mutation` 是只追加的回滚历史：它不会修改或删除原变更
操作，而是用 `parent_action_id` 指向被撤销操作，并把权威语义恢复到该
操作的 `change_json.before` / `canonical_before_hash` 快照。当前实现只覆盖
标签作用域内的语义或 atom 变更（`add_positive_atom`、`add_negative_atom`、
`update_semantics`），成功后标脏标签 atom 索引并保持验证待定；引导创建产生的
标签身份或任务绑定回滚不由该操作类型表达。

当前建设性本体变更路径的责任边界如下：

- `label_semantics` 是权威本体事实；`label_atoms` 是它的 SQLite 物化
  投影；`label_ontology_actions` 是只追加的来源记录，不是第二份事实。
- `update_semantics`、`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
  `create_label_proposal` 和 `bootstrap_label` 操作只能由专用服务路径写入。
  `adopt_existing_atom` 是只记录来源的路径，修改前后哈希相同，只把新的
  来源信号连接到已有 atom，不修改权威语义或 atom，也不标脏 atom
  索引；其他建设性变更与对应的权威写入位于同一 SQLite 事务。
- 每个语义或 atom 变更事务只写一条根变更操作；`change_json`
  只保存一次修改前后的语义快照。实际新增或删除的 atom 写入
  `label_ontology_action_atom_effects`；仅修改说明的补丁写入零条影响记录，无实际变化的补丁不写
  操作、影响记录，也不标脏索引。
- 人工变更可以没有来源信号，但仍必须记录操作者、理由、修改前后
  哈希和变更快照。信号驱动的变更会额外写入
  `label_ontology_action_signals` 链接。
- `label semantics upsert` 默认使用补丁与 CAS 路径：`expected_semantics_hash` 防止
  更新丢失；缺省字段不清空旧语义；只有 `replace=true` 才执行完整替换，并将缺省
  数组解释为空集合。
- 直接从任务标签引导创建与接受提案共用同一采用原语。任务标签
  引导创建可以新建或复用没有语义的同名权威标签；接受提案当前会先拒绝
  任何已有的规范化名称冲突，因此成功路径会创建新的权威标签。二者都会写入
  语义和 atom、标脏标签 atom 索引，并写一个 `bootstrap_label` 根操作和新增的
  atom 影响记录；接受提案
  不写 `task_labels`，任务标签引导创建会绑定来源任务。失败时权威写入与
  来源操作一起回滚。
- `rename_label`、`split_label`、`merge_labels` 仍可作为信号的 `proposed_action` 或旧版
  操作读取；当前公开服务、CLI 和 HTTP 不再写新的结构规划变更操作。旧
  结构规划操作的验证要求解释为 `unsupported`。
- `legacy_untracked=true` 只表示当前 atom 没有可匹配的本体操作，例如旧数据或
  破坏性清理后的历史缺口；新的建设性变更不应依赖这种兼容路径来解释
  来源。

表：`label_ontology_action_signals`

多对多连接操作与信号。多个信号可以支持一次 atom 修改；同一个信号
也可以先被确认，随后关联变更操作和验证操作。

默认审查队列只读取 `open` 与 `confirmed` 信号；完整历史需要明确包含全部状态。
变更操作写入后通常保持来源信号为 `confirmed`。只有可信自动化
`passed` 验证会把已链接的来源信号转为 `resolved`；外部或人工
证明、`failed` 或 `partial` 验证只追加历史，不删除信号，也不把问题
伪装成已验证关闭。

### 12.4 标签语义提案

表：`label_semantic_proposals`

`label_semantic_proposals` 保存新增标签提案的生命周期，不是权威的
标签事实。它只记录“现有标签 atom 建议覆盖不足时，外部或人工提供方
给出的候选语义”。明确接受之前，不会创建 `labels`、`label_semantics`、
`label_atoms` 或 `task_labels`。

| 字段 | 说明 |
|---|---|
| `id` | `lp_...` 提案 ID。 |
| `board_id` / `task_id` | 提案来源任务。 |
| `status` | `proposed` / `accepted` / `rejected`。提供方不可用不会写成状态，而是返回降级尝试。 |
| `name` / `description` / `applies_when` / `excludes_when` / `positive_examples` / `negative_examples` | 候选标签语义。数组字段为 JSON 字符串数组。 |
| `heuristic_coverage` / `heuristic_coverage_cosine` / `heuristic_residual_norm` | 来自当前残差标签建议求解器的覆盖与残差元数据，用于记录提案创建时现有标签 atom 的覆盖程度；`heuristic_coverage = clamp(1 - heuristic_residual_norm, 0.0, 1.0)`，二者不是独立证据；`heuristic_coverage_cosine` 是查询向量与拟合向量的余弦相似度。 |
| `top1_existing_label_id` / `top1_existing_label_name` | 当前启发式排序第一的已有标签。 |
| `diagnostics_json` | JSON 字符串数组，包含降级、冲突或验证诊断。 |
| `decision_reason` / `resolved_label_id` / `decided_at` | 接受或拒绝的决策信息；接受后 `resolved_label_id` 指向新建的权威标签。 |

只有 `proposed` 状态的提案可以被接受。接受操作通过共享的采用原语创建同一看板内的
权威 `labels` 行，并写入对应的 `label_semantics` / `label_atoms`，同时标脏
`lancedb_label_atoms` 派生存储，写入 `bootstrap_label` 来源操作，并让
`resolved_label_id` 指向结果标签；提案状态、权威写入与来源操作
在同一事务内提交。它不写入 `task_labels`，不会把新标签自动绑定到来源
任务。

拒绝操作会把提案标记为 `rejected`。与现有标签发生规范化名称冲突的
候选会持久化为 `rejected`，诊断信息包含 `near_duplicate_label_conflict`。
规范化名称冲突是一种确定性的近似重复启发式判断，会忽略大小写、空白和标点。

---

## 13. 看板列（Column）

看板列（Column）属于 UI 展示层。

字段：

| 字段 | 说明 |
|---|---|
| `id` | 看板列 ID。 |
| `board_id` | 所属看板。 |
| `status` | 映射的权威状态。 |
| `title` | UI 名称。 |
| `position` | UI 排序。 |
| `hidden` | 是否隐藏。 |
| `wip_limit` | 可选的在制任务数量限制。 |

当前最小实现中，一个状态对应一个看板列。

---

## 14. 知识派生底座（Knowledge Substrate）

知识派生底座相关表只支持实体身份、关系镜像、派生发件箱和派生存储健康状态。SQLite 中的任务、执行记录、评论和事件仍是运行时权威事实来源。

### 14.1 实体登记

表：`entities`

字段：

| 字段 | 说明 |
|---|---|
| `uri` | 稳定的 `kb://...` 实体 URI。 |
| `kind` | 开放文本；当前自动投影使用 `board`、`column`、`task`、`run`、`event`、`comment`、`attachment`、`label`、`task_label`、`setting`。 |
| `source_table` | 来源 SQLite 表。 |
| `source_id` | 来源行 ID。 |
| `board_id` | 可选的看板作用域。 |
| `task_id` | 可选的任务作用域。 |
| `title` | 展示标题。 |
| `summary` | 简短摘要。 |
| `content_hash` | 内容哈希，用于派生层判断变化。 |
| `created_at` / `updated_at` / `archived_at` | 生命周期时间。 |

### 14.2 关系图镜像

表：`relation_predicates`、`entity_relations`

`relation_predicates` 定义受控谓词；`entity_relations` 保存可重建的关系镜像。关系层用于图与上下文查询，不改变任务状态机。状态机仍以 `tasks.status`、`task_dependencies` 和服务事务为准。

### 14.3 索引发件箱

表：`index_outbox`

字段：

| 字段 | 说明 |
|---|---|
| `id` | 自增作业 ID。 |
| `source_event_id` | 来源 `task_events.id`，允许事件被删除/导入时置空。 |
| `target` | `tantivy` / `oxigraph` / `lancedb` / `all`。 |
| `projection_store` | 可空的精确 store selector；当前只允许 `target=lancedb` 时使用 `lancedb_label_atoms`。`NULL` 保持旧路由语义。 |
| `entity_uri` | 目标实体。 |
| `action` | `upsert` / `delete` / `rebuild`。 |
| `payload_json` | 有界的作业载荷。 |
| `status` | `pending` / `running` / `done` / `failed`。 |
| `attempts` | 尝试次数。 |
| `last_error` | 最近失败原因。 |
| `created_at` / `updated_at` | 作业时间。 |

`index_outbox` 是至少执行一次语义的派生作业接口。任务变更事务只写 SQLite 权威事实、事件、实体和发件箱记录，不直接写 Tantivy、Oxigraph 或 LanceDB。

### 14.4 派生存储状态

表：`derived_store_state`

字段：

| 字段 | 说明 |
|---|---|
| `store_name` | 派生存储名称，例如 `tantivy_tasks`、`oxigraph_relations`、`lancedb_chunks`、`lancedb_label_atoms`。 |
| `schema_version` | 存储结构或契约版本。 |
| `last_event_id` | 存储已成功提交的全局 `task_events.id` 水位。 |
| `dirty` | 是否仍有未完成的发件箱记录、失败的发件箱记录，或最近一次存储更新失败。 |
| `last_rebuild_at` | 最近成功重建时间。 |
| `last_sync_at` | 最近成功同步时间。 |
| `last_error` | 最近失败证据。 |
| `updated_at` | 状态更新时间。 |

`last_event_id` 是存储全局成功处理水位，不是单个看板的局部水位。成功同步或重建只能单调推进这个值；当一个看板同步完成、但其他看板仍有 `pending`、`running` 或 `failed` 发件箱记录时，`dirty` 必须保持 `true`。`dirty=false` 只表示同一存储目标当前没有未完成的发件箱记录，而且最近一次存储更新没有失败。

`last_error` 在成功后清空，失败时保留错误证据并保持 `dirty=true`。操作者应通过 `kanban derived status`、`kanban doctor`、维护 API 和对应的同步或重建命令恢复派生层；派生存储损坏或落后不会改变 SQLite 中的任务事实。

### 13.5 Projection v2 consistency domain

表：`projection_database`、`projection_store_state`、`projection_deliveries`、
`projection_maintenance_owner`

`projection_database` 为一份 SQLite 文件保存稳定 `database_instance_id` 和 projection
protocol version。`projection_store_state` 为每个物理 store 保存 schema version、
legacy/v2 control-plane owner、连续 checkpoint、active/previous/building generation、
provider/model fingerprint、canonical 与 delivery 双 coverage digest、单调 fence epoch、
lease 和 lifecycle/error 状态。
`projection_deliveries` 把一个 `index_outbox` row 展开为各 store 的 board-scoped delivery；
唯一键是 `(outbox_id, store_name)`，每个 delivery 同时携带不可空 `board_id`、连续
store cursor、claim token、lease token、fence epoch 和 target generation。
`projection_maintenance_owner` 是 singleton database runtime lease，保存 owner、opaque
token、mode、expiry、heartbeat、已编译的 store capability 集和可追溯 build identity；
public status 不返回 token。Migration 028 会使无法证明 capability/build identity 的旧
lease 失效。lease 获取、续约与释放都比较 owner + token + canonical capability JSON +
build identity，过期、被篡改或来自另一构建的 owner 无法续约，也无法清除后继 owner。
`maintenance status` 会把当前二进制未编译的 backend 标为 `unavailable`；若活动 owner
没有声明某个 store capability，则该 store 标为 `unverified` 并提供 fallback reason，
不能因为 singleton owner 仍活跃就推断所有 store 都在被维护。

Migration 026 在 fanout/backfill 前验证每个相关 outbox row 能从 source event 或 entity
得到唯一 board；无法解析、orphan source event 或 event/entity board 冲突时 fail closed，
不会以 nullable/global delivery 绕过隔离。旧 `index_outbox.status=done` 只映射为
`legacy_done`，不伪造 v2 checkpoint 或 generation coverage。

Migration 029 以 additive `index_outbox.projection_store` selector 把
`lancedb_label_atoms` 接入同一 delivery domain，且不重建 `index_outbox` 或
`projection_deliveries`、不改变旧 ID/cursor。旧的 `target=lancedb|all` 且 selector 为
`NULL` 的 row 仍只路由到 `lancedb_chunks`；精确
`target=lancedb, projection_store=lancedb_label_atoms` 只路由到标签 atom store。
selector 与 target 在插入后不可改变；exact selector row 还被 SQLite 约束为
`source_event_id=NULL`、`kb://board/{board_id}` 实体、`action=rebuild` 和精确 payload
`{"scope":"board","version":1}`。所有 legacy LanceDB chunks 的 pending/complete/fail、
dirty 与 doctor count 查询都显式要求 `projection_store IS NULL`，不能凭共享
`target=lancedb` 吞入 label atom work。标签规范 mutation 在同一 canonical SQLite
事务里把 `label_atom_index_boards` 标脏，并由 trigger 原子写入 board-scoped
`rebuild` delivery。事务回滚时 dirty 标记、outbox 和 delivery 一并回滚；已有
pending/failed board rebuild 会合并，running rebuild 期间的新 mutation或 provider
failure 会留下新的 pending delivery，provider failure 即使没有旧 delivery 也必须生成
可恢复 work。迁移时已有的 dirty board 会逐板 backfill，不清空错误、旧 outbox 或
watermark。

Projection v2 的 snapshot 流程先固定 cursor，并按 store 从 canonical SQLite 读取完整、
稳定排序且强制携带 board scope 的 corpus：task search/chunk 投影包含 task 及其 comments、
runs、events；graph 投影包含 relation；label atom 投影包含 atom。每条 record 具有稳定
identity、payload 与 content hash；manifest 同时保存 canonical corpus 和 cursor 内
delivery 集合的 count + stable digest，并绑定 provider/model fingerprint。Provider 必须
实际消费 records，返回的 artifact evidence 必须匹配
database/protocol/schema/provider/generation/fence/cursor/两组 coverage；提交 snapshot
acknowledgement 的 transaction 会再次读取 canonical corpus 和 delivery coverage，任一
变化或存在 running claim 都拒绝批量完成。增量 batch 的 receipt 还必须精确匹配 lease、
fence、provider、generation、claim token 和 item count。

`lancedb_label_atoms` 的 canonical mutation 已通过 Migration 029 进入
`projection_deliveries`；`label_atom_index_boards` 在迁移期继续提供 per-board
dirty/error 兼容状态。generation 仍必须在 runtime/backend 的 provider fingerprint、
coverage、lease/fence 和物理 generation publish 门禁全部成立后才能发布，不能因为
delivery seam 已存在就绕过这些证据。

只有物理 store 完成 generation pointer CAS、active read-back 匹配，并证明上一物理
generation（若存在）仍可按 generation id 读取，SQLite 才原子发布 active/previous
metadata。若进程在物理 pointer swap 后退出，新 fence owner 可检查同一 generation 的
artifact evidence 并 reconcile SQLite publish。
若 logical active 的物理 artifact 已不可读，正常 publish CAS 仍 fail closed。只有
maintenance 的显式 recovery 路径可在新 snapshot/catch-up、当前 database/provider
binding 与 fenced lease 均成立时发布替代 generation；SQLite previous metadata 改为
实际可读且被物理 backend 保留的 generation，而不是伪造已丢失 artifact 的保留证据。

`derived_store_state` 和 `index_outbox` 在迁移期保留为 v1 compatibility projection。
generation begin 即把 store 切到 v2 control plane；legacy 与 v2 writer 在完整物理写周期
共享 per-database/per-store barrier，database replace 同时取得所有 store barrier，因此
旧 Tantivy/Oxigraph/LanceDB writer、v2 pointer swap 和 replace 不会交错。v2 reducer
只在 delivery 获得真实 generation coverage 后更新 legacy dirty/outbox 摘要，避免双控制面
永久 dirty 或虚假 clean。

表：`label_atom_index_boards`

`label_atom_index_boards` 只跟踪可重建的 `lancedb_label_atoms` 派生层在各看板
上的刷新状态，不是标签事实。`label_semantics` / `label_atoms` 更新会把对应
看板标脏；单个看板的标签 atom 重建成功，只会清理该看板的 `dirty` 标记。
只有该存储下所有看板都不再标脏时，`derived_store_state.dirty` 才能变为
`false`。

## 15. 常用查询

### 15.1 看板任务列表

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

### 15.2 就绪队列

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

### 15.3 已过期的领取

```sql
SELECT *
FROM tasks
WHERE status = 'running'
  AND claim_expires_at IS NOT NULL
  AND claim_expires_at <= ?;
```

### 15.4 事件流

```sql
SELECT *
FROM task_events
WHERE board_id = ?
  AND id > ?
ORDER BY id ASC
LIMIT ?;
```

---

## 16. 导出与导入格式

JSONL 导出与导入使用可移植的看板快照格式：

```bash
kanban export --board default --format jsonl --out board.jsonl
kanban import --input board.jsonl --replace
```

每行：

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{"type":"task","data":{...}}
{"type":"event","data":{...}}
{"type":"comment","data":{...}}
{"type":"dependency","data":{...}}
```

通用信号账本使用稳定的记录类型：

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{"type":"signal_observation","data":{...}}
{"type":"signal","data":{...}}
```

标签本体账本使用稳定的记录类型：

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{"type":"label_ontology_observation","data":{...}}
{"type":"label_ontology_signal","data":{...}}
{"type":"label_ontology_action","data":{...}}
{"type":"label_ontology_action_atom_effect","data":{...}}
{"type":"label_ontology_action_signal","data":{...}}
```

可移植描述符的权威定义共覆盖 21 个辨别字段值；输入和输出各有精确根结构，共
42 个 Draft 2020-12 结构定义。每行 `data` 都是闭合对象，必需但可空的键必须存在，
但可以明确为 `null`。真实的导出生产者与导入消费者使用同一描述符和 fixture 登记表。
SQLite 中的 `evidence_json`、`related_labels_json`、`proposal_json`、`change_json`、
`validation_json` 等仍是权威存储列；公开适配器只暴露去掉 `_json` 后的自然 JSON。

导入另有一条只向前兼容的迁移，用于读取采用自然 JSON 契约之前、
由上一版导出器生成的数据库原生 JSONL 快照。该格式通过 `column.hidden=0|1`
以及 `metadata_json` / `payload_json` 等真实 SQLite 列形状识别；同一快照必须保持
单一格式，不能混用数据库原生记录与自然 JSON 记录。同一记录只要同时出现自然 JSON
重命名键和数据库原生重命名键，就会在规范化前被拒绝，不能让旧版
值静默覆盖自然 JSON 值。导入器只会把结构一致的上一版本记录中的 JSON 文本列和整数
布尔值转换为当前自然 JSON 记录，再执行同一精确契约验证，以及下述事务和最终
一致性门禁。当前及后续导出始终只写自然 JSON，不再产生数据库原生键；
这不是长期双轨公开契约。

导入时会在同一事务中先插入各行，再运行最终一致性门禁。基础关系表
会检查 `task_labels`、`task_dependencies`、`task_runs`、`task_comments`、
`signal_observations`、`signals`、`task_events`、`task_attachments` 的行所属看板与
被引用任务、标签、执行记录、评论和观察记录所属看板是否一致；失败时整个
`--replace` 导入事务回滚，不提交部分数据。

本体相关行也在同一事务中插入，并延迟回填
`label_ontology_signals.superseded_by_signal_id` 与
`label_ontology_actions.parent_action_id`，避免依赖同表自引用行在文件中的顺序。导入完成前会校验本体账本的看板隔离：观察记录与信号所属看板、操作
父级所属看板、操作与信号链接所属看板、标签或提案软引用所属看板必须一致；
孤立的操作与信号链接、替代关系环和操作父级环会导致导入
失败。

通用信号的 `signals.superseded_by_signal_id` 同样会延迟回填，避免依赖同表自引用行在文件中的顺序。

`kanban doctor --json` 对上述基础关系表、SQLite `PRAGMA foreign_key_check`、本体
账本一致性和通用信号账本的看板一致性规则做只读巡检。
基础关系表问题返回 `consistency_errors`、`consistency_warnings`、
`consistency_issues[]`；本体账本问题返回 `ontology_ledger_errors`、
`ontology_ledger_warnings`、`ontology_ledger_issues[]`。问题项包含 `severity`、
`code`、`message`、`record_ids`，用于定位损坏行；基础关系表消息包含
`table`、`row`、`row_board` 和 `referenced_board`，外键问题会记录表、
rowid、父表和外键索引。严重错误包括行所属看板不匹配、
缺失 v12 本体表、跨看板链接、孤立的操作与信号或操作与影响链接、通用
信号孤立或跨看板上下文、通用信号替代关系环、父级或替代关系异常、标签、提案或任务所属看板不匹配、
替代关系环和操作父级环；错误数非零会让 `ok=false`。警告保留给仍可解释或可重建的软引用，例如历史操作的
`result_atom_id` 已被当前 `label_atoms` 重建删除。

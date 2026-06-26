# State Machine

本文件定义 canonical task status、合法 transition、guard 与 side effects。

---

## 1. Status Enum

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

建议 Rust 表示：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
    Running,
    Blocked,
    Review,
    Done,
    Archived,
}
```

---

## 2. 状态职责

| Status | 是否可编辑 | 是否可 claim | 是否默认展示 | 是否参与 promotion | 说明 |
|---|---:|---:|---:|---:|---|
| `triage` | 是 | 否 | 是 | 否 | 待澄清。 |
| `todo` | 是 | 否 | 是 | 否 | 已定义，但依赖未完成，或尚未被人工提升到 ready。 |
| `scheduled` | 是 | 否 | 是 | 否 | 等时间到；到期后仍需显式 promote 才进入 ready。 |
| `ready` | 是 | 是 | 是 | 否 | 已显式进入可执行队列。 |
| `running` | 部分 | 否 | 是 | 否 | 正在执行。 |
| `blocked` | 是 | 否 | 是 | 否 | 阻塞。 |
| `review` | 是 | 否 | 是 | 否 | 待检查。 |
| `done` | 部分 | 否 | 默认可隐藏 | 否 | 已完成。 |
| `archived` | 否 | 否 | 默认隐藏 | 否 | 归档。 |

---

## 3. Transition Commands

### 3.1 Create

```text
none -> triage | todo | scheduled | ready
```

Initial status 计算：

```text
if input.status explicitly provided and valid for creation:
    use explicit status
else if required spec missing:
    triage
else if scheduled_at > now:
    scheduled
else if parent dependencies exist and not all parents are done or archived:
    todo
else:
    ready
```

允许显式创建状态：

```text
triage | todo | scheduled | ready
```

不允许直接创建：

```text
running | review | done | archived
```

Side effects：

- insert `tasks`
- insert `task_events(kind='task.created')`

---

### 3.2 Specify

```text
triage -> todo | scheduled | ready
```

Guard：

- title 非空。
- description/spec 满足本地校验。
- 如果 `scheduled_at > now`，目标必须是 `scheduled`。
- 如果 parent dependencies 未全部进入 `done` 或 `archived`，目标必须是 `todo`。
- 否则可进入 `ready`。

Side effects：

- update task fields。
- insert `task_events(kind='task.specified')`。

---

### 3.3 Promote

```text
todo -> ready
scheduled -> ready
```

Guard：

- 所有 parent dependency 都是 `done` 或 `archived`。
- execution plan 不是 `unplanned`：必须有 required subtask 形成 `planned`，或显式标记 `not_required` 并填写 reason。
- task 未 archived。
- 对 `scheduled`，必须 `scheduled_at <= now`。

Side effects：

- update status。
- insert `task_events(kind='task.promoted')`。

Promote 是显式 ready 意图，通常由人工 CLI/Web action 触发：

```bash
kanban task promote t_xxx
```

---

### 3.4 Claim / Start

```text
ready -> running
```

Guard：

- task.status == `ready`。
- `claim_token IS NULL`。
- 所有 parent dependency 都是 `done` 或 `archived`。
- execution plan 不是 `unplanned`。
- task 未 archived。

Atomic side effects in one transaction：

1. CAS update `tasks`：
   - `status = 'running'`
   - `claim_token = <new token>`
   - `claim_owner = <actor>`
   - `claim_expires_at = now + ttl`
   - `last_heartbeat_at = now`
   - `started_at = COALESCE(started_at, now)`
   - `lock_version = lock_version + 1`
2. insert `task_runs(status='running')`
3. update `tasks.current_run_id`
4. insert `task_events(kind='task.claimed')`

Failure：

- 若 affected rows = 0，返回 `claim_conflict` 或 `dependency_blocked`。

---

### 3.5 Heartbeat

```text
running -> running
```

Guard：

- task.status == `running`。
- claim token 匹配。
- claim 未被 force reclaimed。

Side effects：

- extend `claim_expires_at`。
- update `last_heartbeat_at`。
- update active `task_runs.last_heartbeat_at`。
- insert `task_events(kind='task.heartbeat')` 可采样写入，避免过多事件。

建议：

- 默认每次 heartbeat 更新 run/task。
- event 可每 N 次或每 60s 写一次。

---

### 3.6 Complete

```text
running -> done
review -> done
```

Guard：

- `running -> done` 必须 claim token 匹配，除非 `force=true`。
- `review -> done` 不需要 claim token。
- 如果存在 required direct subtasks，它们必须全部为 `done` 或 `archived`；optional subtasks 不阻塞 parent complete。

Side effects：

- update task status `done`。
- set `completed_at = now`。
- clear claim fields。
- update active run status `succeeded`。
- insert `task_events(kind='task.completed')`。
- 不自动 promote child tasks；child 保持 `todo`，由 derived dependency state 表示是否仍被 parent 阻塞。

---

### 3.7 Submit Review

```text
running -> review
```

Guard：

- claim token 匹配，除非 `force=true`。

Side effects：

- update task status `review`。
- clear claim fields。
- update active run status `succeeded` with `outcome='review'`。
- insert `task_events(kind='task.submitted_for_review')`。

---

### 3.8 Block

```text
triage | todo | scheduled | ready | running | review -> blocked
```

Guard：

- reason 非空。
- 若从 `running` block，必须 claim token 匹配，除非 `force=true`。

Side effects：

- update status `blocked`。
- set `status_reason`。
- if running: close active run as `failed` or `canceled` depending input。
- clear claim fields。
- insert `task_events(kind='task.blocked')`。

---

### 3.9 Unblock

```text
blocked -> triage | todo | scheduled | ready
```

目标状态计算：

```text
if spec incomplete:
    triage
else if scheduled_at > now:
    scheduled
else if parents not all done:
    todo
else:
    ready
```

Side effects：

- clear `status_reason`。
- update status to computed target。
- insert `task_events(kind='task.unblocked')`。

---

### 3.10 Reclaim

```text
running -> ready | blocked
```

Guard：

任一条件满足：

- `claim_expires_at <= now`。
- worker PID 已不存在。
- run 超过 max runtime。
- 人工 `force=true`。

目标状态：

- 默认 `ready`。
- 如果 retry_count >= max_retries，则 `blocked`。

Side effects：

- close active run as `expired` or `canceled`。
- clear claim fields。
- increment retry_count if appropriate。
- insert `task_events(kind='task.reclaimed')`。

---

### 3.11 Archive

```text
triage | todo | scheduled | ready | blocked | review | done -> archived
```

默认不允许直接 archive `running`，除非 `force=true`。

Side effects：

- set `archived_at = now`。
- set status `archived`。
- clear claim fields if force。
- insert `task_events(kind='task.archived')`。

---

### 3.11.1 Board archive

Board archive is a board lifecycle operation, not a task status transition.

Rules：

- Set `boards.archived_at = now`。
- Insert `task_events(kind='board.archived')`。
- Do not rewrite tasks on that board.
- Reject archive if the board has any `running` task or any `running` task_run.
- After archive, ordinary task/comment/dispatcher mutations against that board are rejected.
- Read-only history queries for events, runs, and comments remain available for audit.

---

### 3.12 Reopen

```text
done -> ready | todo | scheduled
archived -> previous non-archived status or triage
review -> ready
```

MVP 可不实现 reopen。若实现，必须写 event，并重新检查依赖和 schedule。

---

## 4. Transition Matrix

| From \ To | triage | todo | scheduled | ready | running | blocked | review | done | archived |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| none | create | create | create | create | - | - | - | - | - |
| triage | - | specify | specify | specify | - | block | - | - | archive |
| todo | - | - | schedule | promote/manual | - | block | - | - | archive |
| scheduled | - | unschedule | - | promote | - | block | - | - | archive |
| ready | - | demote | schedule | - | claim | block | - | - | archive |
| running | - | - | - | reclaim | - | block | submit_review | complete | force_archive |
| blocked | unblock | unblock | unblock | unblock | - | - | - | - | archive |
| review | - | - | - | reopen | - | block | - | complete | archive |
| done | - | - | - | reopen | - | - | - | - | archive |
| archived | restore | restore | restore | restore | - | - | - | - | - |

`demote`、`schedule`、`reopen`、`restore` 可作为 v1+ 命令；MVP 可以只实现 create/specify/promote/claim/heartbeat/complete/block/unblock/reclaim/archive。

---

## 5. Dependency Rules

### 5.1 依赖语义

```text
parent_task_id -> child_task_id
```

表示 child 被 parent 阻塞。只有 parent 为 `done` 或 `archived` 时，child 才能进入 `ready` 或 `running`。归档 parent 会满足 hard dependency guard，但不会删除 dependency edge，也不会自动 promote child。

### 5.2 规则

1. parent != child。
2. 新增依赖不能产生环。
3. 如果给一个 `ready` child 增加未完成 parent（不是 `done` 或 `archived`），child 必须降级为 `todo`。
4. 如果 parent 从 `done` 被 reopen，所有依赖它的 child 必须重新评估；若 child 不是 terminal/running，可降级为 `todo`。
5. `running` child 不应被新增未完成依赖；除非 force，并且需要 block/reclaim。

---

## 6. UI Column Mapping

UI column 不是状态真相，只是展示配置。

默认列：

| Column | Status |
|---|---|
| Triage | `triage` |
| Todo | `todo` |
| Scheduled | `scheduled` |
| Ready | `ready` |
| Running | `running` |
| Blocked | `blocked` |
| Review | `review` |
| Done | `done` |

`archived` 默认隐藏。

拖拽行为：

- 从 `ready` 拖到 `running`：调用 claim/start。
- 从 `running` 拖到 `done`：调用 complete，需 active claim 或 force。
- 从任意非 terminal 拖到 `blocked`：弹窗要求 reason，调用 block。
- 从 `blocked` 拖到其他列：调用 unblock，不直接设目标状态。
- 拖到 `archived`：调用 archive。

---

## 7. Testing Requirements

必须覆盖：

1. transition matrix 单元测试。
2. dependency cycle detection。
3. `ready -> running` 并发 claim 只有一个成功。
4. expired claim reclaim。
5. block/unblock 重新计算目标状态。
6. completion 后 child 保持 `todo`，并清除 derived dependency-blocked state。
7. archived task 不被 dispatcher 处理。
8. `unplanned` task 不能 promote/claim，dispatcher 也不能 claim。
9. required subtask 未完成时 parent 不能 complete。
10. illegal direct transition 返回 `invalid_transition`。

# 状态机规范

本文件定义权威任务状态、合法转换、守卫条件与副作用。

---

## 1. 状态枚举

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

| 状态 | 是否可编辑 | 是否可领取 | 是否默认展示 | 说明 |
|---|---:|---:|---:|---|
| `triage` | 是 | 否 | 是 | 待澄清。 |
| `todo` | 是 | 否 | 是 | 已定义，但依赖或执行计划尚未允许进入 `ready`。 |
| `scheduled` | 是 | 否 | 是 | 等时间到；时间到本身不会改变状态。 |
| `ready` | 是 | 是 | 是 | 已进入可执行队列。 |
| `running` | 部分 | 否 | 是 | 正在执行。 |
| `blocked` | 是 | 否 | 是 | 阻塞。 |
| `review` | 是 | 否 | 是 | 待检查。 |
| `done` | 部分 | 否 | 默认可隐藏 | 已完成。 |
| `archived` | 否 | 否 | 默认隐藏 | 归档。 |

---

## 3. 状态转换命令

### 3.1 创建（`create`）

```text
none -> triage | todo | scheduled | ready
```

候选初始状态按以下顺序计算：

1. 如果调用方显式提供了允许创建的 `input.status`，使用它作为候选状态。
2. 否则，如果必需规格不完整，候选状态为 `triage`。
3. 否则，如果 `scheduled_at > now`，候选状态为 `scheduled`。
4. 否则，如果存在尚未进入 `done` 或 `archived` 的父依赖，候选状态为 `todo`。
5. 否则，候选状态为 `ready`。

候选状态为 `ready` 时，创建服务仍会保存为 `todo`，因为新任务的执行计划从
`unplanned` 开始。

允许显式创建状态：

```text
triage | todo | scheduled | ready
```

这里的 `ready` 是允许请求并通过基础守卫的候选状态，不表示新任务会直接保存为
`ready`。新任务还没有执行计划，因此服务会把候选 `ready` 保存为
`todo`，计划状态为 `unplanned`。添加第一个 step 使计划成为 `planned`，或者通过
`kanban task step not-required` 填写原因后标记为 `not_required`；其它守卫也满足
时，重新计算才会进入 `ready`。

删除最后一个 step 会使派生计划回到 `unplanned`，并把可重新计算的任务退回
`todo`。当前删除操作只写入 `task.step.removed`，不会额外写入
`task.execution_plan.unplanned` 事件。

不允许直接创建：

```text
running | blocked | review | done | archived
```

副作用：

- 向 `tasks` 插入记录。
- 写入 `task_events(kind='task.created')`。

---

### 3.2 明确规格（`specify`）

```text
triage -> todo | scheduled | ready
```

守卫条件：

- `title` 非空。
- `description` / 规格满足本地校验。
- 如果 `scheduled_at > now`，目标必须是 `scheduled`。
- 如果父依赖未全部进入 `done` 或 `archived`，目标必须是 `todo`。
- 如果执行计划仍为 `unplanned`，目标必须是 `todo`。
- 否则可进入 `ready`。

副作用：

- 更新任务字段。
- 写入 `task_events(kind='task.specified')`。

---

### 3.3 提升为可执行（`promote`）

```text
todo -> ready
scheduled -> ready
```

守卫条件：

- 所有父依赖都是 `done` 或 `archived`。
- 执行计划不是 `unplanned`：必须有 step 形成 `planned`，或显式标记
  `not_required` 并填写原因。
- 任务未归档。
- 对 `scheduled`，必须 `scheduled_at <= now`。

副作用：

- 更新状态。
- 写入 `task_events(kind='task.promoted')`。

`promote` 表示显式进入 `ready` 的意图，通常由人工 CLI 或 Web 操作触发：

```bash
kanban task promote t_xxx
```

执行计划或规格变更也可能触发活动状态重算。例如添加第一个 step 或标记
`not_required` 后，满足其它条件的 `todo` 会通过 `task.recomputed` 进入 `ready`，
而不是写入 `task.promoted`。排期时间到达本身不会触发这种重算。

---

### 3.4 领取 / 开始（`claim` / `start`）

```text
ready -> running
```

守卫条件：

- `task.status == 'ready'`。
- `claim_token IS NULL`。
- 所有父依赖都是 `done` 或 `archived`。
- 执行计划不是 `unplanned`。
- 任务未归档。

同一事务内的原子副作用：

1. 以 CAS 更新 `tasks`：
   - `status = 'running'`
   - `claim_token = <new_token>`
   - `claim_owner = <actor>`
   - `claim_expires_at = now + ttl`
   - `last_heartbeat_at = now`
   - `started_at = COALESCE(started_at, now)`
   - `lock_version = lock_version + 1`
2. 插入 `task_runs(status='running')`。
3. 更新 `tasks.current_run_id`。
4. 写入 `task_events(kind='task.claimed')`。

失败：

- 如果受影响行数为 0，返回 `claim_conflict` 或 `dependency_blocked`。

---

### 3.5 心跳（`heartbeat`）

```text
running -> running
```

守卫条件：

- `task.status == 'running'`。
- 领取凭证匹配。
- 领取尚未被强制回收。

副作用：

- 延长 `claim_expires_at`。
- 更新任务的 `last_heartbeat_at`。
- 更新活动 run 的 `claim_expires_at` 与 `last_heartbeat_at`。
- 每次显式心跳都写入 `task_events(kind='task.heartbeat')`。

对于 `running` 任务，后续有效的任务级事件（例如评论、step 或 label 变更）也会作为
隐式存活信号：服务层刷新任务与活动 run 的领取期限和最后心跳时间，但不额外写入
`task.heartbeat` 事件。board 级事件或没有 `task_id` 的事件不会刷新领取期限。

---

### 3.6 完成（`complete`）

```text
running -> done
review -> done
```

守卫条件：

- `running -> done` 必须匹配领取凭证，除非 `force=true`。
- `review -> done` 不需要领取凭证。
- 如果存在必需 step，它们必须全部为 `done` 或 `skipped`；可选 step 不阻塞父任务完成。

副作用：

- 把任务状态更新为 `done`。
- 设置 `completed_at = now`。
- 清除领取字段。
- 把活动 run 状态更新为 `succeeded`。
- 写入 `task_events(kind='task.completed')`。
- 不自动改写或提升子任务；子任务保持原状态。此前为 `todo` 的仍是 `todo`，派生依赖
  状态会更新为不再受该父任务阻塞。

---

### 3.7 提交审核（`review`）

```text
running -> review
```

守卫条件：

- 领取凭证匹配，除非 `force=true`。

副作用：

- 把任务状态更新为 `review`。
- 清除领取字段。
- 把活动 run 状态更新为 `succeeded`。
- 写入 `task_events(kind='task.submitted_for_review')`。

---

### 3.8 阻塞（`block`）

```text
triage | todo | scheduled | ready | running | review -> blocked
```

守卫条件：

- `reason` 非空。
- 从 `running` 进入 `blocked` 时必须匹配领取凭证，除非 `force=true`。

副作用：

- 把状态更新为 `blocked`。
- 设置 `status_reason`。
- 如果原状态为 `running`，把活动 run 关闭为 `failed`，并记录退出码 `1`。
- 清除领取字段。
- 写入 `task_events(kind='task.blocked')`。

---

### 3.9 解除阻塞（`unblock`）

```text
blocked -> triage | todo | scheduled | ready
```

目标状态按以下顺序计算：

1. 规格不完整时进入 `triage`。
2. 否则，`scheduled_at > now` 时进入 `scheduled`。
3. 否则，父依赖未全部完成时进入 `todo`。
4. 否则，执行计划仍为 `unplanned` 时进入 `todo`。
5. 否则进入 `ready`。

副作用：

- 清除 `status_reason`。
- 更新为计算出的目标状态。
- 写入 `task_events(kind='task.unblocked')`。

---

### 3.10 回收领取（`reclaim`）

```text
running -> ready | todo | blocked
```

守卫条件：

- 批量自动回收只扫描 `running` 且 `claim_expires_at <= now` 的任务。
- 指定任务回收要求任务为 `running`，并且领取已过期或显式传入 `force=true`。
- 当前实现不检查工作进程 PID，也没有最长运行时间回收。

目标状态：

- 默认 `ready`。
- 如果回收后的 `retry_count` 达到 `max_retries`，则进入 `blocked`。
- 如果目标原本为 `ready`，但执行计划守卫不再满足，则降级为 `todo`。

副作用：

- 把活动 run 关闭为 `expired` 或 `canceled`。
- 清除领取字段。
- 增加 `retry_count`。
- 写入 `task_events(kind='task.reclaimed')`。

---

### 3.11 归档（`archive`）

```text
triage | todo | scheduled | ready | blocked | review | done -> archived
```

默认不允许直接归档 `running`，除非 `force=true`。非强制归档还要求所有必需 step
均已完成或跳过；`force=true` 才会绕过该守卫。

副作用：

- 设置 `archived_at = now`。
- 把状态更新为 `archived`。
- 强制归档时清除领取字段。
- 写入 `task_events(kind='task.archived')`。

---

#### 3.11.1 看板归档

看板归档属于 board 生命周期操作，不是任务状态转换。

规则：

- 设置 `boards.archived_at = now`。
- 写入 `task_events(kind='board.archived')`。
- 不改写该看板上的任务。
- 如果看板上存在 `running` 任务或 `running` run 记录，则拒绝归档。
- 归档后，拒绝针对该看板的普通任务、评论和内部实验性 dispatcher 写操作。
- 事件、run 记录与评论的只读历史查询仍然可用，以便审计。

---

### 3.12 重新打开（`reopen`）

```text
done -> triage | todo | scheduled | ready
```

守卫条件：

- 只允许重新打开 `done` 任务；`review`、`archived` 和其它状态必须拒绝。
- `reason` 必须非空。

目标状态由服务端重新计算，不由调用方指定：

1. 规格不完整时进入 `triage`。
2. 否则，`scheduled_at > now` 时进入 `scheduled`。
3. 否则，父依赖未全部进入 `done` 或 `archived` 时进入 `todo`。
4. 否则，执行计划尚不可执行时进入 `todo`。
5. 否则进入 `ready`。

副作用：

- 清除 `completed_at`。
- 保留 `result_summary` / `result_json`。
- 写入 `task_events(kind='task.reopened')`，payload 包含 `from`、`to`、`reason`、
  `original_completed_at`。
- 直接依赖该任务的子任务中，仅 `triage | todo | scheduled | ready` 会按可执行条件
  重新计算；`running | blocked | review | done | archived` 不会被隐式改写。

---

## 4. 当前已实现的显式转换矩阵

| 来源 \ 目标 | triage | todo | scheduled | ready | running | blocked | review | done | archived |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| none | create | create | create | 请求 `ready` 后保存为 `todo` | - | - | - | - | - |
| triage | - | specify | specify | specify | - | block | - | - | archive |
| todo | - | - | - | promote | - | block | - | - | archive |
| scheduled | - | - | - | promote | - | block | - | - | archive |
| ready | - | - | - | - | claim | block | - | - | archive |
| running | - | reclaim | - | reclaim | - | block / reclaim | submit_review | complete | force_archive |
| blocked | unblock | unblock | unblock | unblock | - | - | - | - | archive |
| review | - | - | - | - | - | block | - | complete | archive |
| done | reopen | reopen | reopen | reopen | - | - | - | - | archive |
| archived | - | - | - | - | - | - | - | - | - |

表中只列出当前已有的显式转换服务。由规格、依赖或执行计划变化触发的状态重算不作为
独立命令列入矩阵；它们通过 `task.recomputed` 进入计算出的活动状态。任务级 `reopen`
当前只实现从 `done` 进入服务端重新计算出的活动状态。

### 4.1 未实现候选

以下命令目前没有实现，不属于当前转换矩阵：

- `schedule`
- `unschedule`
- `demote`
- `restore`

---

## 5. 依赖规则

### 5.1 依赖语义

```text
parent_task_id -> child_task_id
```

表示子任务被父任务阻塞。只有父任务为 `done` 或 `archived` 时，子任务才能进入
`ready` 或 `running`。归档父任务会满足强依赖守卫，但不会删除依赖边，也不会自动
提升子任务。

### 5.2 规则

1. `parent_task_id != child_task_id`。
2. 新增依赖不能产生环。
3. 如果给一个 `ready` 子任务增加未完成父任务（不是 `done` 或 `archived`），子任务必须
   降级为 `todo`。
4. 父任务从 `done` 被重新打开时，仅直接子任务中的可重新计算活动状态
   （`triage | todo | scheduled | ready`）会按可执行条件重新计算；
   `blocked | review | running | done | archived` 不会被隐式改写。
5. 不允许给 `running` 子任务新增未完成依赖；当前接口没有强制例外。必须先通过阻塞或
   回收让子任务退出 `running`，再新增依赖。

---

## 6. UI 列映射

UI 列不是状态真相，只是展示配置。

默认列：

| 默认显示名 | 状态 |
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

- 从 `ready` 拖到 `running`：调用 `claim` / `start`。
- 从 `running` 拖到 `done`：调用 `complete`，需要活动领取或显式强制。
- 从任意非终态拖到 `blocked`：弹窗要求填写原因，然后调用 `block`。
- 从 `blocked` 拖到其他列：调用 `unblock`，不直接设置目标状态。
- 拖到 `archived`：调用 `archive`。

---

## 7. 测试要求

必须覆盖：

1. 状态转换矩阵单元测试。
2. 依赖环检测。
3. `ready -> running` 并发领取只有一个成功。
4. 过期领取回收。
5. `block` / `unblock` 重新计算目标状态。
6. 完成父任务后不自动改写子任务状态，并更新派生的依赖阻塞状态。
7. 内部实验性 dispatcher 不处理已归档任务。
8. `unplanned` 任务不能 `promote` 或 `claim`，内部实验性 dispatcher 也不能领取。
9. 必需 step 未完成时，父任务不能 `complete`。
10. 非法直接转换返回 `invalid_transition`。

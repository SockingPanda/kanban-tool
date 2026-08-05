# 任务状态机

状态机由 `kanban-core` 的 readiness/claim/finish 规则和
`kanban-service::ApplicationService` 统一执行。HTTP、CLI、MCP、Desktop 与
dispatcher 都调用这些显式 command；没有通用的“传入任意目标状态”入口。

## 1. Canonical 状态

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

| 状态 | 含义 | 可被 dispatcher claim |
|---|---|---:|
| `triage` | 规格不完整 | 否 |
| `todo` | 已定义但尚未满足执行条件 | 否 |
| `scheduled` | `scheduled_at` 尚未到期 | 否 |
| `ready` | 依赖、规格和执行计划均允许执行 | 是 |
| `running` | 持有 active claim/run | 否 |
| `blocked` | 明确记录阻塞原因 | 否 |
| `review` | 执行完成，等待审查 | 否 |
| `done` | 已完成 | 否 |
| `archived` | 只读历史状态 | 否 |

`tasks.status` 是唯一状态真相；`board_columns` 只映射展示。进入 `ready` 的共同
就绪判断顺序是：标题/描述完整、排期已到期、所有父依赖为 `done` 或 `archived`，
并且执行计划为 `planned` 或 `not_required`。状态写入使用 `lock_version` CAS，
过期调用必须失败且不能留下部分副作用。

## 2. 创建和执行计划

### `task.create`

允许请求的初始状态为 `triage|todo|scheduled|ready`。服务先按规格、排期和依赖
计算候选状态；`ready` 候选因为新任务的计划初始为 `unplanned`，实际保存为
`todo`，同时创建 `task_execution_plans(state='unplanned')` 和
`task.created` 事件。`running|blocked|review|done|archived` 不能作为初始状态。

### `task.plan.not_required`

此 command 不改变 `tasks.status`，只将没有任何 step 的任务计划写为
`not_required` 并保存非空原因。归档任务/看板或已有 step 时拒绝；计划状态第一次
变为 `not_required` 时写入 `task.execution_plan.not_required`。计划为 `unplanned`
时，`task.promote`、`task.claim` 和 `task.release` 都必须拒绝。

创建第一个 step 会把计划写为 `planned`。step/dependency 的现有写操作若影响活动
任务，只能在 `triage|todo|scheduled|ready` 范围内按同一 readiness 规则重算，并与
关系写入处于同一事务；它们不能绕过下列显式命令直接设置任意状态。

## 3. 显式任务 commands

### `task.promote`

```text
todo -> ready
scheduled -> ready
```

要求规格完整、`scheduled_at <= now`、父依赖全部满足、任务和看板未归档、计划已
可执行。服务和 store 都重新读取事实并检查 `lock_version`；成功更新任务并写入
`task.promoted`。任何守卫失败都不修改任务、计划或事件。

### `task.claim`

```text
ready -> running
```

要求任务确实为 `ready`、没有任何 claim 字段、依赖已满足、计划可执行且未归档。
同一 immediate transaction 内：

1. 以 `status='ready'`、空 claim 字段和预期 `lock_version` CAS 更新任务为
   `running`，写入新 token、owner、expiry、heartbeat、`current_run_id` 和
   `started_at`；
2. 插入唯一的 `task_runs(status='running')`；
3. 写入 `task.claimed` 事件。

并发调用恰好一个成功；其他调用得到冲突，不能产生第二个 run/event。普通 claim
不要求日志路径；dispatcher claim 可在同一 command 中指定 worker profile、metadata
和受控 log root。

### `task.heartbeat`

```text
running -> running
```

只接受 active running claim，且 actor 与 token 必须匹配。事务同时延长 task/run
的 `claim_expires_at`、更新 `last_heartbeat_at` 和 task `lock_version`，并写入
`task.heartbeat`。owner 或 token 不匹配时整批拒绝。

### `task.release`

```text
running -> ready
```

仅 active claim 的 owner 携 matching token 可主动释放。服务重新验证规格、排期、
依赖和执行计划仍允许 `ready`；否则拒绝，不产生部分写入。成功事务同时：将 active
run 标为 `canceled`、清除 task claim/heartbeat/current run、回到 `ready`，并写入
`task.released`。

### `task.review`

```text
running -> review
```

默认要求 claim owner 和 matching token；受控 dispatcher 可使用 `force`。事务将
active run 标为 `succeeded`（exit code 0，可写 summary），清除 task 的 claim 字段，
保留 current run 作为历史关联，将任务设为 `review`，并写入
`task.submitted_for_review`。`review` 任务不能再有 active running run。

### `task.done`

```text
running -> done
review -> done
```

running 来源要求 active claim、owner/token（除非 force）；review 来源要求其 current
run 已 succeeded。所有 `required=1` 的 step 必须为 `done` 或 `skipped`。成功事务把
running run 标为 `succeeded`，设置 `completed_at`，保存可选 summary/result JSON，
清除 task claim 字段，设为 `done`，并写入 `task.completed`。不自动改写其他任务。

### `task.block`

```text
triage | todo | scheduled | ready | running | review -> blocked
```

必须提供非空 reason。running 来源要求 active claim 的 owner/token（除非 force）；
其他来源不能有 active running run。running 来源的 run 在同一事务中标为 `failed`、
`exit_code=1` 并记录 error；任务清除 claim 字段、保存 `status_reason`、设为
`blocked`，写入 `task.blocked`。任何校验或 CAS 失败都会回滚 run 和 task 两侧。

## 4. Lease reclaim 与 opt-in dispatcher

`kanban serve` 默认不启动 dispatcher。只有提供 profile 时才在同一进程启动一个
worker loop；profile 固定声明 board、worker command、poll interval、claim TTL、
heartbeat interval、success/failure policy 和 log directory。

每轮顺序为：

1. 调用 application 的 `reclaim_expired(board, 'dispatcher')`；
2. 只查询 `status='ready'` 的任务（按 priority），尝试复用 `task.claim`；
3. 单 worker 执行 command，并周期性复用 `task.heartbeat`；
4. 按 profile 复用 `task.done`/`task.review`，失败复用 `task.block` 或
   `task.release`。

dispatcher 绝不自动 claim `review`、`todo`、`scheduled` 或 `triage`，也不直接写
`tasks`、`task_runs`、`task_events`。

### `reclaim_expired`

这是 dispatcher 的内部 application operation，不是 adapter 自由调用的状态设置。
它只扫描 `running` 且 `claim_expires_at <= now` 的任务，并以 claim owner/token、
run ID 和 lock version 做 CAS。成功时同一事务：

- active run -> `expired`，写入结束时间和 `claim expired` error；
- 清除任务 claim、heartbeat、current run；
- `retry_count += 1`；达到 `max_retries` 时任务为 `blocked`，否则按规格、排期、
  依赖和计划重新计算为 `triage|todo|scheduled|ready`；
- 写入 `task.reclaimed`。

新一轮 polling 停止后，graceful shutdown 允许当前 worker 正常结束；force shutdown
才终止当前 worker。所有 lease、run 和 event 变化继续走 ApplicationService 的同一
事务路径。

## 5. 可验证的不变量

- 任何 mutation 都必须经过 ApplicationService 与本状态机；adapter 不得提交 SQL。
- `ready -> running` 是原子 claim，单任务最多一个 active run。
- owner/token、expiry 和 `lock_version` 共同保护 heartbeat/release/review/done/block。
- 依赖环、跨看板关系、未完成 required step 和未满足执行计划必须在事务提交前拒绝。
- 每次成功的状态或 lease mutation 都有对应 event；失败不得留下孤立 event/run。

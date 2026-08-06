# 任务状态机

状态机由 `kanban-core` 的 readiness/claim/lease 规则和 `kanban-service::ApplicationService` 统一执行。HTTP、CLI、MCP、Desktop 与 dispatcher 只调用显式 command；adapter 不提供任意 `transition(target_status)`，也不直接写 `tasks.status`。

## 1. Canonical 状态

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

| 状态 | 语义 | dispatcher 可 claim |
| --- | --- | ---: |
| `triage` | 规格尚不完整 | 否 |
| `todo` | 已定义但条件尚未满足 | 否 |
| `scheduled` | 等待 `scheduled_at` 到期 | 否 |
| `ready` | 规格、排期、依赖、execution plan 均可执行 | 是 |
| `running` | 持有 active claim/run/lease | 否 |
| `blocked` | 记录了阻塞原因 | 否 |
| `review` | 执行结果等待审查 | 否 |
| `done` | 已完成 | 否 |
| `archived` | 只读历史状态 | 否 |

`tasks.status` 是唯一状态事实；`board_columns` 仅做展示映射。ready 判定重新读取 canonical facts，并检查标题/规格、排期、父依赖、execution plan、board/task archived 和 `lock_version`。guard 失败时不写 task、run、event 或 projection job。

CLI 的 `board columns` 只读取展示映射；它不会创建新的状态或 transition。任务状态仍由下方
显式 lifecycle command 驱动，wire method/path 以 protocol endpoint catalog 为准。

## 2. 创建、规格和 execution plan

### `task.create`

允许初始请求 `triage|todo|scheduled|ready`。新任务默认 execution plan 为 `unplanned`；即使请求 `ready`，只要计划/依赖/排期不满足，service 也会保存重新计算后的 `triage|todo|scheduled`。`running|blocked|review|done|archived` 不是合法初始状态。

### `task.specify`

`triage → todo|scheduled|ready` 的规格补全使用显式 `POST /transitions/specify`/CLI `task specify`/MCP `task_specify`。它只更新允许的描述/排期字段，然后由 service 重算状态并写 `task.specified`/对应事件；不能借 PATCH 直接设 status。

### `task.step.*` 与 `task.plan.not_required`

第一条 step 将 execution plan 变为 `planned`；`task step not-required` 将无 step 任务变为 `not_required` 并要求非空 reason。step `done|skip|reopen|remove|update` 均由 service 校验 task scope、required 规则、linked task board 和 event，不直接改 task status。计划或 step 变化后，活动任务按同一 readiness 规则重新计算。

## 3. Lifecycle commands

### `task.promote`

```text
todo|scheduled → ready
```

要求规格完整、排期已到期、父依赖满足、board/task 未归档且计划为 `planned|not_required`。成功写 `task.promoted`；失败无部分写入。

### `task.claim`

```text
ready → running
```

service 以 `status='ready'`、空 claim、预期 `lock_version` 做 CAS，并在同一 immediate transaction 中：

1. 写入 token、owner、expiry、heartbeat、`current_run_id`；
2. 插入唯一 active `task_runs(status='running')`；
3. 写 `task.claimed` event 和 projection job。

并发 claim 恰好一个成功；冲突调用不产生第二个 run/event。

### `task.heartbeat`

```text
running → running
```

只接受 active claim 的 owner/token，事务同时延长 task/run lease、更新 heartbeat 和 lock version，并写 `task.heartbeat`。错误 token 不留下任何更新。

### `task.release`

```text
running → ready
```

matching owner/token 才能主动 release。service 重新验证 ready 条件后，在同一事务取消 active run、清除 claim/heartbeat/current run、恢复 task 为 `ready`，并写 `task.released`。

### `task.review`

```text
running → review
```

默认要求 owner/token；受控 dispatcher 可使用 `force`。active run 变为 `succeeded`，claim 清除，任务进入 `review`，写 `task.submitted_for_review`。review 不保留 active run。

### `task.done`

```text
running|review → done
```

running 来源要求 owner/token（除非 force），review 来源要求 current run 已成功；所有 required step 必须 `done|skipped`。成功写完成时间、summary/result，结束 run/claim，并写 `task.completed`。

### `task.block`

```text
triage|todo|scheduled|ready|running|review → blocked
```

要求非空 reason。running 来源校验 owner/token（除非 force），并把 run 标为 `failed`；其他来源不得有 active running run。task、run、event、projection job 一起提交。

### `task.unblock`

```text
blocked → triage|todo|scheduled|ready
```

解除阻塞后由 service 重新计算规格、排期、依赖和计划，不能盲目写 `ready`。成功写 `task.unblocked`，失败不清除原 reason 或部分更新。

### `task.reopen`

```text
done|review → triage|todo|scheduled|ready
```

保留历史 result/run/event，清空 completion timestamp，按 canonical facts 重算目标状态，并写 `task.reopened`。不得直接删除完成审计或把子任务无条件改为 ready。

### `task.reclaim`

显式 reclaim 只处理过期或 force 的 running claim；service 使用 owner/token、run ID 和 lock version CAS，在一事务内结束 run、清除 claim、增加 retry、按 facts 重算 `triage|todo|scheduled|ready` 或达到上限后设 `blocked`，并写 `task.reclaimed`。dispatcher 复用同一 operation。

### `task.archive`

只能通过显式 archive guard 进入 `archived`。active run、未满足必要条件或已归档 board 会被拒绝；成功写 `task.archived`。默认 list/search 隐藏 archived，历史 events/runs 仍可读。

## 4. Dispatcher 与 lease

`kanban serve` 默认不启动 dispatcher；只有 `--dispatcher-profile <path>` 才启动同进程单 worker。每轮：

1. service reclaim expired claims；
2. 只查询 `ready` 并原子 claim；
3. worker command 期间复用 heartbeat；
4. 成功调用 `done|review`，失败调用 `block|release`。

dispatcher 绝不 claim `review`、`todo`、`scheduled` 或 `triage`，也不直接写 `tasks`、`task_runs`、`task_events`。停止时先停止 polling，再等待当前 worker；第二次中断才 force stop。

## 5. 不变量与证据

- 任一 mutation 都经过 `ApplicationService`、`kanban-core` guard 和 Turso transaction。
- `ready → running` 是原子 claim；单任务最多一个 active run。
- owner/token、expiry、`lock_version` 共同保护 heartbeat/release/review/done/block/reclaim。
- board-scoped FK、dependency cycle、required step、plan/排期 guard 在提交前拒绝。
- 成功的状态/lease mutation 都有对应 event；失败不留下孤立 run、event 或 projection job。

已有 service evidence 包括 `claim_task_concurrent_callers_have_exactly_one_winner`、`release_task_returns_ready_and_cancels_run_atomically`、`submit_review_task_moves_running_task_and_run_atomically`、`explicit_reclaim_expires_run_in_one_transaction_and_increments_retry`、`specify_task_recomputes_unplanned_task_to_todo`、`unblock_task_recomputes_blocked_task_without_forcing_ready`、`reopen_task_clears_completion_but_preserves_result_and_recomputes_children` 和 `archive_task_sets_archived_state_and_event`。完整 adoption/full gate 仍需独立运行并记录结果。

# Dispatcher SPEC

Dispatcher 是本地可选调度器。它只处理本机 SQLite DB，不处理远程 worker，不处理多用户协作。

---

## 1. 目标

Dispatcher 负责：

1. reclaim：回收超时、崩溃或失联的 `running` 任务。
2. claim：从 `ready` 队列选择任务并进入 `running`。
3. run：执行本地 worker profile。
4. heartbeat：维持 claim。
5. finish：根据 worker 结果写回 `done/review/blocked/ready`。

Dispatcher 不负责：

- 远程执行。
- 多机协调。
- 权限控制。
- 长期日志存储。

---

## 2. 运行方式

### 2.1 单次运行

```bash
kanban dispatch --once
```

执行一轮：

1. reclaim expired。
2. claim up to capacity。
3. 对已 claim task 启动 worker。

### 2.2 常驻运行

```bash
kanban dispatch
kanban dispatch --max-iterations 10
```

前台循环执行。`--max-iterations` 用于测试、脚本或受控 smoke；不传时持续运行直到进程收到外部停止信号。

### 2.3 与 server 同进程

后续扩展。当前实现先提供独立 `kanban dispatch` 前台 loop；`kanban serve` 不启动 dispatcher。

### 2.4 Worker profile config

```bash
kanban dispatch --worker-profile backend --profile-config ./workers.toml
```

最小配置格式：

```toml
[workers.backend]
command = "cargo nextest run -p kanban-sqlite --no-fail-fast"
claim_ttl_ms = 300000
heartbeat_interval_ms = 30000
on_success = "done"
on_failure = "blocked"
log_dir = ".kb/logs/runs"
```

当前 CLI 只读取被 `--worker-profile` 选中的 section。支持字段：

- `command`
- `claim_ttl_ms`
- `heartbeat_interval_ms`
- `on_success`: `done|review|blocked|ready`
- `on_failure`: `done|review|blocked|ready`
- `log_dir`

`log_dir` 必须位于受信任 run log 根目录内：平台默认 run log 目录、
`<db_dir>/logs`，或 `<db_dir>/.kb/logs`。Dispatcher 在 claim task 之前拒绝
其他路径，避免写出后续 `kanban run logs` 和 `doctor` 会判定为可疑的 run log。

---

## 3. Dispatcher Loop

伪代码：

```rust
loop {
    let now = clock.now_ms();

    reclaim_expired(now)?;
    while running_count() < max_concurrency {
        match claim_next_ready_task(now)? {
            Some(claimed) => spawn_worker(claimed)?,
            None => break,
        }
    }

    sleep(poll_interval);
}
```

---

## 4. Promotion

Dispatcher 不执行 `todo/scheduled -> ready` promotion。`ready` 表示显式人工 promote 意图；依赖完成或计划到期只会改变查询返回的 derived state，不会把 task 放入 ready 队列。

---

## 5. Ready Queue Selection

默认排序：

```sql
ORDER BY priority ASC, created_at ASC
```

`priority` is the implemented P0-P3 integer level where `0` (P0) is highest and
`3` (P3) is lowest/default, so dispatcher/frontier claim order selects P0 first
among tasks that are already `ready`.

Priority does not place work into the ready queue. P0 means incident, current
blocker, or must-handle-immediately work; P1 is near-term focus; P2 is important
follow-up; P3 is ordinary backlog/low/default. Ordinary ready tasks should remain
P1/P2/P3 unless they are truly immediate blockers. A P0 task in `todo`,
`scheduled`, or `triage` is still not claimable until the normal state-machine
guards allow explicit promotion to `ready`.

可选后续扩展：

- assignee/profile matching。
- due_at 优先。
- label filter。
- WIP limit。

MVP selection 输入：

```text
board_id
worker_profile optional
limit
```

如果 task.assignee 不为空：

- 当 worker profile 与 assignee 匹配时可 claim。
- 人工 CLI start 可忽略 worker profile，但 actor 写入 claim_owner。

---

## 6. Claim Algorithm

Claim 必须原子执行。

伪 SQL：

```sql
BEGIN IMMEDIATE;

SELECT id
FROM tasks
JOIN boards ON boards.id = tasks.board_id
WHERE tasks.board_id = ?
  AND boards.archived_at IS NULL
  AND status = 'ready'
  AND claim_token IS NULL
  AND (assignee IS NULL OR assignee = ?)
  AND NOT EXISTS (
    SELECT 1
    FROM task_dependencies d
    JOIN tasks p ON p.id = d.parent_task_id
    WHERE d.child_task_id = tasks.id
      AND p.status != 'done'
  )
ORDER BY priority ASC, created_at ASC
LIMIT 1;

UPDATE tasks
SET status = 'running',
    claim_token = ?,
    claim_owner = ?,
    claim_expires_at = ?,
    last_heartbeat_at = ?,
    started_at = COALESCE(started_at, ?),
    updated_at = ?,
    lock_version = lock_version + 1
WHERE id = ?
  AND status = 'ready'
  AND claim_token IS NULL;

INSERT INTO task_runs (...);
UPDATE tasks SET current_run_id = ? WHERE id = ?;
INSERT INTO task_events (...);

COMMIT;
```

如果 update affected rows = 0，说明被其他进程抢先 claim，重新选择下一个。

---

## 7. Worker Profile

配置示例：

```toml
[workers.default]
command = "./scripts/run-task.sh"
concurrency = 1
claim_ttl_ms = 300000
heartbeat_interval_ms = 30000
max_runtime_ms = 3600000
on_success = "done"   # done | review
on_failure = "blocked" # blocked | ready

[workers.codegen]
command = "kb-agent --task $KB_TASK_ID"
concurrency = 2
on_success = "review"
on_failure = "blocked"
```

### 7.1 环境变量

Worker process 获得：

| Env | 说明 |
|---|---|
| `KB_DB_PATH` | SQLite DB path。 |
| `KB_BOARD_ID` | board id。 |
| `KB_BOARD_SLUG` | board slug。 |
| `KB_TASK_ID` | task id。 |
| `KB_TASK_SEQ` | task seq。 |
| `KB_TASK_TITLE` | title。 |
| `KB_CLAIM_TOKEN` | claim token。 |
| `KB_RUN_ID` | run id。 |
| `KB_ACTOR` | dispatcher/worker actor。 |

Worker 可通过 CLI 回写：

```bash
kanban --db "$KB_DB_PATH" task heartbeat "$KB_TASK_ID" --claim-token "$KB_CLAIM_TOKEN"
kanban --db "$KB_DB_PATH" task done "$KB_TASK_ID" --claim-token "$KB_CLAIM_TOKEN" --summary "..."
```

也可以让 dispatcher wrapper 根据进程退出码自动 complete/block。

---

## 8. Heartbeat

默认：dispatcher wrapper 负责 heartbeat，不要求 worker 自己做。

规则：

- 每 `heartbeat_interval_ms` 更新一次。
- heartbeat TTL 延长至 `now + claim_ttl_ms`。
- 若 heartbeat 失败，dispatcher 应终止 worker 或等待 reclaim。

---

## 9. Finish Policy

### 9.1 Success

Worker exit code = 0。

根据 profile：

| `on_success` | Transition |
|---|---|
| `done` | `running -> done` |
| `review` | `running -> review` |

### 9.2 Failure

Worker exit code != 0。

根据 profile：

| `on_failure` | Transition |
|---|---|
| `blocked` | `running -> blocked` with reason。 |
| `ready` | reclaim to ready and increment retry。 |

如果 `retry_count >= max_retries`，强制进入 `blocked`。

### 9.3 Timeout

如果 run 超过 `max_runtime_ms`：

- 尝试 terminate worker。
- close run as `expired`。
- 根据 retry policy 进入 `ready` 或 `blocked`。

---

## 10. Reclaim

Reclaim 条件：

1. `claim_expires_at <= now`。
2. worker_pid 不存在。
3. run 超时。
4. manual force。

Reclaim side effects：

- task status: `ready` 或 `blocked`。
- clear claim fields。
- close active run as `expired/canceled`。
- insert `task_events(kind='task.reclaimed')`。

---

## 11. PID Checking

因为只支持单机，可以检查 PID。

限制：

- PID 可能复用。
- 只能作为辅助信号，claim TTL 仍是主机制。
- 跨平台实现需要抽象。

建议：

- Linux/macOS：检查 pid 是否存在。
- Windows：后续实现，MVP 可只依赖 TTL。

---

## 12. Logs

Worker stdout/stderr 不全量写 DB。

默认路径：

```text
~/.local/state/kb/logs/r_<run_id>.log
```

DB 记录：

- `task_runs.log_path`
- `task_runs.summary`
- `task_runs.error`

CLI：

```bash
kanban run logs r_01HX...
```

---

## 13. Failure Cases

| Case | 行为 |
|---|---|
| Dispatcher 崩溃 | running task claim 过期后被下次 dispatcher reclaim。 |
| Worker 崩溃 | heartbeat 停止，claim 过期，reclaim。 |
| SQLite busy | 等待 busy_timeout；仍失败则记录错误并下轮重试。 |
| Task 被人工 block | Dispatcher 不再处理。 |
| Board 被归档 | Dispatcher 不再 claim/reclaim 该 board；若仍有 running task/run，board archive 本身会被拒绝。 |
| Task 被人工 force complete | Worker 后续 complete 失败，因 token/run 已关闭。 |
| DB integrity failed | Dispatcher 停止，提示运行 `kanban doctor`。 |

---

## 14. MVP Scope

MVP dispatcher 必须实现：

- claim one ready task。
- spawn command。
- heartbeat。
- complete/block based on exit code。
- reclaim expired claims。

MVP 可暂不实现：

- profile concurrency > 1。
- complex worker matching。
- Windows PID checking。
- per-label routing。
- cron-like recurring tasks。

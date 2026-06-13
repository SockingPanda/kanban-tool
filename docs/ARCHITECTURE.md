# Architecture

本架构面向本地单机运行：Rust core、SQLite-only、CLI、localhost Web server、可选 dispatcher。

---

## 1. 总体架构

```text
             ┌───────────────┐
             │    Web UI     │
             └───────┬───────┘
                     │ HTTP/SSE localhost
             ┌───────▼───────┐
             │ kanban-server │
             └───────┬───────┘
                     │ Command Service
┌───────────────┐    │
│  kanban-cli   ├────┤
└───────────────┘    │
                     ▼
             ┌───────────────┐
             │ kanban-core   │
             │ state machine │
             │ commands      │
             └───────┬───────┘
                     │ repository
             ┌───────▼───────┐
             │ kanban-sqlite │
             └───────┬───────┘
                     │
             ┌───────▼───────┐
             │   SQLite WAL  │
             └───────────────┘

             ┌───────────────┐
             │ dispatcher    │
             │ local worker  │
             └───────┬───────┘
                     │ Command Service
                     ▼
             same core/sqlite path
```

---

## 2. Crate 结构

推荐仓库结构：

```text
crates/
  kanban-core/
    src/
      domain/
      commands/
      state_machine.rs
      errors.rs
      clock.rs
      id.rs

  kanban-sqlite/
    src/
      db.rs
      migrations.rs
      repositories.rs
      transactions.rs
      queries.rs

  kanban-cli/
    src/
      main.rs
      commands/
      output.rs

  kanban-server/
    src/
      main.rs
      routes/
      dto.rs
      sse.rs

  kanban-dispatcher/
    src/
      loop.rs
      worker.rs
      reclaim.rs
      profiles.rs
```

### 2.1 `kanban-core`

职责：

- 定义领域类型：`Task`、`Board`、`Status`、`Run`、`Event`、`Dependency`。
- 定义 command input/output。
- 实现状态机与 transition guard。
- 定义错误类型。
- 定义 service 层接口。
- 不依赖 SQLite、HTTP、CLI、前端。

示例：

```rust
pub enum Status {
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

pub enum Command {
    CreateTask(CreateTask),
    UpdateTask(UpdateTask),
    ClaimTask(ClaimTask),
    Heartbeat(HeartbeatTask),
    CompleteTask(CompleteTask),
    BlockTask(BlockTask),
    UnblockTask(UnblockTask),
    ArchiveTask(ArchiveTask),
}
```

### 2.2 `kanban-sqlite`

职责：

- SQLite 连接初始化。
- migrations。
- transaction 封装。
- repository 实现。
- 复杂查询。
- CAS claim。
- append event。

关键要求：

- 所有状态变化必须在 transaction 内完成。
- claim 必须使用 `BEGIN IMMEDIATE` 或等价机制抢写锁。
- 不允许业务层执行裸 SQL 更新状态。

### 2.3 `kanban-cli`

职责：

- 解析命令。
- 构造 command input。
- 调用 core service。
- 输出 human table 或 JSON。
- 返回稳定 exit code。

CLI 可以直接打开 SQLite DB 调用 service，不需要 server 常驻。

### 2.4 `kanban-server`

职责：

- localhost HTTP API。
- 静态 Web UI hosting 或 API-only。
- SSE event stream。
- 请求 DTO 转 command input。
- 错误格式统一。

默认只监听：

```text
127.0.0.1:8721
```

### 2.5 `kanban-dispatcher`

职责：

- claim。
- heartbeat。
- reclaim。
- worker profile 执行。
- run result 写回。

Dispatcher 可以嵌入 server，也可以由 CLI 单独运行：

```bash
kanban dispatch
kanban dispatch --once
```

---

## 3. 数据流

### 3.1 创建 task

```text
CLI/Web
  -> CreateTask command
  -> validate input
  -> compute initial status
  -> insert tasks
  -> insert task_events(kind='task.created')
  -> return task snapshot
```

初始状态计算：

```text
if spec incomplete           -> triage
else if scheduled_at > now   -> scheduled
else if dependencies exist   -> todo
else                         -> ready
```

### 3.2 Claim task

```text
CLI/Web/Dispatcher
  -> ClaimTask command
  -> BEGIN IMMEDIATE
  -> verify task.status == ready
  -> verify no unfinished parent dependencies
  -> CAS update tasks to running
  -> insert task_runs(status='running')
  -> update tasks.current_run_id
  -> insert task_events(kind='task.claimed')
  -> COMMIT
```

### 3.3 Complete task

```text
Worker/CLI/Web
  -> CompleteTask command
  -> BEGIN IMMEDIATE
  -> verify running/review
  -> if running: verify claim token unless force=true
  -> update task_runs
  -> update tasks to done or review
  -> clear claim fields
  -> insert task_events(kind='task.completed')
  -> children remain todo; derived dependency state reflects whether they are still blocked
  -> COMMIT
```

### 3.4 Web live update

```text
State-changing command
  -> insert task_events with monotonically increasing id
  -> server SSE loop polls or subscribes to events
  -> browser receives event
  -> browser fetches changed task or applies patch
```

---

## 4. Process 模型

### 4.1 无 server 模式

```bash
kanban task create "..."
kanban task list
```

CLI 直接打开 SQLite DB。

适用：脚本、本地开发、快速使用。

### 4.2 server 模式

```bash
kanban serve
```

启动：

- localhost HTTP server。
- Web UI。

适用：日常看板 UI。

### 4.3 dispatcher 模式

```bash
kanban dispatch
```

启动本地调度循环。与 server 同进程运行 dispatcher 是后续扩展；当前 CLI 使用独立 `kanban dispatch` 前台 loop。

---

## 5. Config

默认配置文件：

```text
~/.config/kb/config.toml
```

示例：

```toml
[data]
db_path = "~/.local/share/kb/kb.db"
data_dir = "~/.local/share/kb"
attachments_dir = "~/.local/share/kb/attachments"
logs_dir = "~/.local/state/kb/logs"

[server]
listen = "127.0.0.1:8721"
open_browser = true

[defaults]
board = "default"
actor = "auto" # auto = OS username or hostname/user

[dispatcher]
enabled = false
poll_interval_ms = 2000
claim_ttl_ms = 300000
max_concurrency = 1

[workers.default]
command = "echo Task $KB_TASK_ID: $KB_TASK_TITLE"
concurrency = 1
on_success = "done" # done | review
on_failure = "blocked" # blocked | ready
```

CLI 还支持项目级 active board 配置：

```text
<project>/.kb/config.toml
```

当前版本只写入一个顶层字段：

```toml
board = "agent-work"
```

Active board 解析顺序是 `--board`、`KB_BOARD`、向上查找最近 `.kb/config.toml`、最后 fallback 到 `default`。项目配置只选择同一个全局 SQLite DB 内的 board，不表示每个项目一个 DB。

---

## 6. Concurrency

### 6.1 SQLite 写入策略

- 使用 WAL。
- 使用短 transaction。
- 对 claim/reclaim/complete 使用 `BEGIN IMMEDIATE`。
- 使用 optimistic lock：`lock_version`。
- 并发 claim 同一 task 时，只有一个 `UPDATE ... WHERE status='ready' AND claim_token IS NULL` 成功。

### 6.2 不做的事情

- 不引入分布式锁。
- 不用网络文件系统共享 DB。
- 不允许多个机器同时写同一 SQLite 文件。

### 6.3 同机多进程

允许：

- 多个 CLI 命令。
- 一个 server。
- 一个 dispatcher。

SQLite WAL 和 busy timeout 负责排队。业务层仍需保证 transaction 短小。

---

## 7. Error Model

核心错误类型：

| code | 说明 |
|---|---|
| `not_found` | 对象不存在。 |
| `invalid_input` | 输入不合法。 |
| `invalid_transition` | 状态转换非法。 |
| `dependency_blocked` | 依赖未完成。 |
| `claim_conflict` | task 已被其他 actor claim。 |
| `claim_expired` | claim token 已过期。 |
| `claim_token_mismatch` | token 不匹配。 |
| `cycle_detected` | dependency 形成环。 |
| `db_busy` | SQLite 忙且超过 timeout。 |
| `internal` | 未分类内部错误。 |

---

## 8. Observability

本地工具仍需要基本可观测性：

- `task_events` 是第一审计来源。
- server 输出结构化日志。
- dispatcher 对每次 run 写入 `task_runs`。
- worker stdout/stderr 可写入本地 log 文件，DB 只存路径和摘要。
- `kanban doctor` 检查 DB、WAL、schema、integrity、orphan run，并报告 Knowledge Substrate 的 `index_outbox` backlog、derived store dirty/error 状态和 per-store last_error。派生层异常不改变 SQLite task truth；operator 通过 sync/rebuild 恢复 Tantivy/Oxigraph/LanceDB。

---

## 9. Security Boundary

因为不做多用户/远程：

- 默认只绑定 `127.0.0.1`。
- 不提供登录系统。
- 不提供远程访问配置。
- 不在 API 中执行任意 shell，worker profile 只能来自本地 config。
- 附件路径必须限制在 data dir 内，防止 path traversal。

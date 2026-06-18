# Kanban Tool SPEC Bundle

本文档由以下文件合并而成：

- README.md
- docs/SPEC.md
- docs/ARCHITECTURE.md
- docs/STATE_MACHINE.md
- docs/DATA_MODEL.md
- docs/CLI_SPEC.md
- docs/API_SPEC.md
- docs/DISPATCHER_SPEC.md
- docs/IMPLEMENTATION_PLAN.md
- docs/ADR.md
- migrations/001_initial.sql
- migrations/003_comment_author_identity.sql

`docs/SPEC.md`、`docs/CLI_SPEC.md`、`docs/API_SPEC.md`、`docs/DISPATCHER_SPEC.md` 等分主题文档是当前行为的权威来源；本文件是这些源文档的同步快照，便于一次性阅读和离线传递。



---

# File: README.md

# Kanban Tool 文档包

本文档包面向一个 **Rust 核心实现、SQLite-only、本地单机运行、同时提供 Web 与 CLI 能力** 的 Kanban 工具。

本项目不是 Trello 的简单复制品，而是一个更接近 Hermes Kanban 的本地可执行工作队列：

- Kanban UI 负责可视化与人工操作。
- CLI 负责脚本化、本地开发流与 agent/automation 入口。
- SQLite 负责持久化任务、状态、依赖、评论、事件、运行记录。
- Rust core 负责状态机与一致性约束。
- Dispatcher 是可选本地调度器，用于 claim 显式 `ready` 任务、heartbeat、reclaim 和执行 worker profile；不自动提升 `todo/scheduled`。

## 范围约束

明确包含：

- 单机本地运行。
- SQLite 作为唯一数据库。
- Web 端与 CLI。
- 多 board/project，但不是多租户。
- 单用户语义；actor 只是审计字段，不是权限主体。
- 本地 dispatcher/worker 能力。
- append-only events + tasks snapshot。

明确不包含：

- 多用户协作。
- 多租户。
- 远程 worker。
- PostgreSQL/MySQL/MongoDB 后端。
- RBAC、组织、团队、邀请、审计权限模型。
- 云同步或网络文件系统共享 SQLite。

## 文档索引

| 文件 | 内容 |
|---|---|
| [`docs/SPEC.md`](docs/SPEC.md) | 产品与技术总 SPEC |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Rust crate、进程、数据流与配置架构 |
| [`docs/STATE_MACHINE.md`](docs/STATE_MACHINE.md) | 状态定义、转换表、不变量 |
| [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md) | 领域对象、ID、时间、事件、附件、查询模型 |
| [`docs/CLI_SPEC.md`](docs/CLI_SPEC.md) | CLI 命令、参数、输出、退出码 |
| [`docs/API_SPEC.md`](docs/API_SPEC.md) | 本地 Web API 与 SSE 事件流 |
| [`docs/DISPATCHER_SPEC.md`](docs/DISPATCHER_SPEC.md) | 本地 dispatcher / worker 调度规格 |
| [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) | 分阶段实现计划、测试策略、验收标准 |
| [`docs/ADR.md`](docs/ADR.md) | 关键架构决策记录 |
| [`migrations/001_initial.sql`](migrations/001_initial.sql) | SQLite 初始 schema |

## 推荐仓库结构

```text
kanban-tool/
  Cargo.toml
  crates/
    kanban-core/
    kanban-sqlite/
    kanban-cli/
    kanban-server/
    kanban-dispatcher/
  web/
  docs/
  migrations/
```

## 默认二进制名

本文档中使用 `kanban` 作为 CLI binary 名称。


---

# File: docs/SPEC.md

# Kanban Tool SPEC

版本：0.1  
范围：Rust core + SQLite-only + Web + CLI + local dispatcher  
约束：无多用户、无多租户、无远程同步、无 PostgreSQL 后端

---

## 1. 产品定位

本工具是一个本地优先的 Kanban 工作系统。它既能作为人类使用的看板，也能作为自动化任务、agent 工作流或本地脚本的 durable work queue。

核心目标：

1. **持久化**：任务、状态、依赖、评论、事件、运行历史必须落盘。
2. **可恢复**：本地进程崩溃后，任务可以通过 claim TTL / heartbeat / reclaim 恢复。
3. **可审计**：每次关键变化写入 `task_events`。
4. **多入口一致**：Web、CLI、dispatcher 必须走同一套 Rust command service，不允许绕过状态机直接写状态。
5. **SQLite-only**：第一版只支持 SQLite，不设计 PostgreSQL/MongoDB backend。
6. **单用户本地语义**：actor 是操作来源字符串，用于审计，不用于鉴权。

一句话定义：

> 一个 SQLite 驱动的本地 Kanban 状态机，暴露 CLI 和 localhost Web API，并可选运行本地 dispatcher 来执行任务。

---

## 2. 非目标

以下能力不进入当前设计：

- 多用户实时协作。
- 用户表、团队表、权限表、邀请机制。
- 多租户隔离。
- SaaS 部署。
- 跨机器 dispatcher/worker。
- SQLite 文件放在 NFS、Dropbox、iCloud Drive 等同步盘上共享写入。
- 任意自定义 workflow editor。
- 任意自定义字段数据库。
- 复杂自动化规则引擎。

---

## 3. 核心对象

| 对象 | 说明 |
|---|---|
| Board | 本地 project/board。不是租户。一个 SQLite DB 内可以有多个 board。 |
| Task | 看板卡片，也是可执行工作单元。 |
| Status | canonical 状态。UI column 只是状态的展示映射。 |
| Dependency | parent task 阻塞 child task。 |
| Comment | 人或自动化留下的协作文本。 |
| Event | append-only 事件流，用于审计、SSE、调试。 |
| Run | 一次执行 attempt。只有 claim/start 后才产生。 |
| Attachment | 附件元数据，blob 存文件系统。 |
| Label | 本地标签。 |
| Column | UI 展示配置，映射到 status。 |

---

## 4. 状态模型

Canonical status：

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

### 4.1 状态语义

| 状态 | 语义 |
|---|---|
| `triage` | 待澄清、待补全规格、尚不可执行。 |
| `todo` | 已定义，但依赖未完成，或尚未进入 ready 队列。 |
| `scheduled` | 已定义，但 `scheduled_at` 在未来。 |
| `ready` | 可被人工或 dispatcher claim。 |
| `running` | 已被某个 actor/worker claim，正在执行。 |
| `blocked` | 因外部依赖、失败、人工输入等原因阻塞。 |
| `review` | 执行完成但需要人工检查。 |
| `done` | 完成。 |
| `archived` | 归档，不参与默认列表和调度。 |

### 4.2 关键原则

1. `running` 只能通过 `claim/start` transition 进入。
2. `ready -> running` 必须在单个 SQLite transaction 中完成 CAS update、创建 run、写 event。
3. `blocked -> ready` 不能盲目设置，必须重新检查依赖与 schedule。
4. UI 拖拽到列时，本质上调用 transition，不是直接 update `tasks.status`。
5. CLI 也不能绕过 transition service。

完整转换表见 [`STATE_MACHINE.md`](STATE_MACHINE.md)。

---

## 5. 存储模型

### 5.1 SQLite 文件位置

默认路径遵循 XDG 目录约定：

```text
~/.local/share/kb/kb.db
~/.local/share/kb/attachments/
~/.local/state/kb/logs/
~/.config/kb/config.toml
```

也支持项目本地模式：

```text
.project/
  .kb/
    kb.db
    attachments/
```

通过 CLI 指定：

```bash
kanban --db .kb/kb.db task list
```

### 5.2 SQLite 配置

每个连接初始化时必须执行：

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 120000;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;
```

### 5.3 存储策略

采用：

```text
tasks 当前快照 + task_events append-only 事件流
```

不采用纯 event sourcing。原因：

- 查询当前看板需要快照表，不能每次重放事件。
- 事件流用于审计、实时推送、调试、增量同步到 Web UI。
- 快照与事件必须在同一 transaction 内更新。

初始 schema 见 [`../migrations/001_initial.sql`](../migrations/001_initial.sql)。

---

## 6. Web 端能力

Web 端是 localhost UI，不是远程协作服务。

默认监听：

```text
127.0.0.1:8721
```

主要页面：

1. Board 看板页。
2. Task detail drawer。
3. Comments。
4. Event timeline。
5. Runs / execution history。
6. Filter/search。
7. Settings。

Web 端只调用 HTTP API，不直接访问 SQLite。

API 见 [`API_SPEC.md`](API_SPEC.md)。

---

## 7. CLI 能力

CLI 是一等入口，必须覆盖核心生命周期：

```bash
kanban init
kanban board list
kanban task create "实现 SQLite schema"
kanban task list --status ready
kanban task show t_xxx
kanban task start t_xxx
kanban task heartbeat t_xxx --claim-token <token>
kanban task block t_xxx "等待接口确认"
kanban task unblock t_xxx
kanban task done t_xxx --claim-token <token>
kanban task archive t_xxx
kanban events t_xxx
kanban runs t_xxx
kanban serve
kanban dispatch --once
```

CLI 必须支持：

- `--json`：机器可读输出。
- `--db <path>`：指定 SQLite DB。
- `--board <slug>`：指定 board。
- `--actor <name>`：覆盖 actor。
- 稳定退出码。

CLI 见 [`CLI_SPEC.md`](CLI_SPEC.md)。

---

## 8. Dispatcher 能力

Dispatcher 是本地可选组件。它不负责多人协作，只负责本地自动化：

1. 从 `ready` 中 claim 任务。
3. 为 claim 创建 `task_runs`。
3. 运行 worker profile。
4. 周期性 heartbeat。
5. 超时或崩溃后 reclaim。
6. 根据 worker exit status 写入 `done/review/blocked/ready`。

`ready` 表示显式人工 promote 意图；parent 完成、dependency 移除或 schedule 到期不会被 dispatcher 自动提升到 `ready`。

Dispatcher 见 [`DISPATCHER_SPEC.md`](DISPATCHER_SPEC.md)。

---

## 9. 核心不变量

实现必须保证：

1. 一个 task 同时最多一个 active claim。
2. 一个 active claim 必须有一个 active run。
3. `running` task 必须有 `claim_token`、`claim_owner`、`claim_expires_at`。
4. task 不能依赖自己。
5. dependency graph 不能形成环。
6. 有未完成 parent 的 child 不得进入 `ready/running`。
7. `archived` task 不参与默认 list、promotion、claim。
8. `done` 和 `archived` 是 terminal-like 状态；默认不再被 dispatcher 修改。
9. 每次状态变化必须写 `task_events`。
10. task snapshot 与对应 event 必须同 transaction 提交。

---

## 10. 成功标准

MVP 完成时必须满足：

- 可以通过 CLI 初始化 DB、创建 task、查看 board、claim、complete、block、unblock。
- 可以通过 Web UI 完成同样操作。
- 状态转换不允许非法路径。
- 并发 claim 同一 task 时只能一个成功。
- 依赖未完成时 child 不会被提升到 `ready`。
- crash/timeout 后可以 reclaim。
- task events 能完整解释 task 当前状态是如何来的。
- SQLite migration 可重复测试。
- 所有核心命令有单元测试或集成测试。


---

# File: docs/ARCHITECTURE.md

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

- claim 显式 `ready` 任务。
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
- `kanban doctor` 检查 DB、WAL、schema、integrity、orphan run。

---

## 9. Security Boundary

因为不做多用户/远程：

- 默认只绑定 `127.0.0.1`。
- 不提供登录系统。
- 不提供远程访问配置。
- 不在 API 中执行任意 shell，worker profile 只能来自本地 config。
- 附件路径必须限制在 data dir 内，防止 path traversal。



---

# File: docs/STATE_MACHINE.md

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
| `todo` | 是 | 否 | 是 | 是 | 已定义，但依赖或人工排序未 ready。 |
| `scheduled` | 是 | 否 | 是 | 是 | 等时间到。 |
| `ready` | 是 | 是 | 是 | 是 | 可执行。 |
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
else if parent dependencies exist and not all parents done:
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
- 如果 parent dependencies 未全部 done，目标必须是 `todo`。
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

- 所有 parent dependency 都是 `done`。
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
- 所有 parent dependency 都是 `done`。
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

表示 child 被 parent 阻塞。只有 parent 为 `done` 时，child 才能进入 `ready` 或 `running`。

### 5.2 规则

1. parent != child。
2. 新增依赖不能产生环。
3. 如果给一个 `ready` child 增加未完成 parent，child 必须降级为 `todo`。
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
6. completion 后 child 保持 `todo`，需要显式 promote。
7. archived task 不被 dispatcher 处理。
8. illegal direct transition 返回 `invalid_transition`。



---

# File: docs/DATA_MODEL.md

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

---

## 5. Task

Task 是核心对象，既是看板卡片，也是可执行工作单元。

### 5.1 字段分组

#### Identity

| 字段 | 说明 |
|---|---|
| `id` | Task ID。 |
| `board_id` | 所属 board。 |
| `seq` | board 内递增数字，便于显示 `#12`。 |

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
| `priority` | 数值越大越优先。 |
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
parent done => child may become ready
parent not done => child cannot be ready/running
```

添加依赖时必须做环检测。

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
comment.added
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
| `author_type` | `human` / `agent` / `system`，表示评论作者身份；旧请求按 `kind` 推断。 |
| `agent_type` | 可选 open text，仅用于 `author_type=agent`，例如 `executor` / `reviewer`。 |
| `body` | Markdown 文本。 |
| `kind` | `text` / `system` / `worker`，保留为兼容展示/来源分类，不等同于作者身份。 |
| `created_at` | 创建时间。 |

Comment 创建时也写一条 `task_events(kind='task.comment.created')`。

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
| `color` | UI color token。 |

同一 board 内 label name 唯一。

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

## 13. 常用查询

### 13.1 Board task list

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

### 13.2 Ready queue

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
      AND p.status != 'done'
  )
ORDER BY t.priority ASC, t.created_at ASC
LIMIT ?;
```

### 13.3 Expired claims

```sql
SELECT *
FROM tasks
WHERE status = 'running'
  AND claim_expires_at IS NOT NULL
  AND claim_expires_at <= ?;
```

### 13.4 Event stream

```sql
SELECT *
FROM task_events
WHERE board_id = ?
  AND id > ?
ORDER BY id ASC
LIMIT ?;
```

---

## 14. Export Format

建议支持 JSONL export：

```bash
kanban export --board default --format jsonl > board.jsonl
```

每行：

```json
{"type":"task","data":{...}}
{"type":"event","data":{...}}
{"type":"comment","data":{...}}
{"type":"dependency","data":{...}}
```

MVP 可先只 export，不做 import。


---

# File: docs/CLI_SPEC.md

# CLI SPEC

默认 binary 名称：`kanban`

CLI 是一等入口；它与 Web 使用同一套 command service 和 SQLite schema。

---

## 1. Global Options

```bash
kanban [GLOBAL_OPTIONS] <COMMAND>
```

| Option | 说明 |
|---|---|
| `--db <path>` | 指定 SQLite DB。默认从 config 读取。 |
| `--board <slug-or-id>` | 显式指定 active board，优先级最高。 |
| `--actor <name>` | 操作 actor。默认 OS username。 |
| `--json` | JSON 输出。 |

Active board 解析顺序：

1. `--board <slug-or-id>`。
2. `KB_BOARD` 环境变量。
3. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `board = "<slug>"`。
4. fallback 到 `default`。

`kanban board use <board>` 会把当前目录写成项目级 `.kb/config.toml`；后续子目录自动继承该 active board。该配置只选择本地项目的 board，不创建新 DB。

### 1.1 Shell completions

```bash
kanban completions <shell>
kanban __complete <kind> [prefix]
```

`kanban completions <shell>` writes a completion script to stdout. Supported
shells:

```text
bash | zsh | fish | powershell | elvish
```

Static command and option completion is generated for all supported shells.
Bash and zsh scripts additionally include dynamic hooks that call the hidden
internal `kanban __complete` helper for DB-backed candidates:

- task refs for task, comment, event, run, and dependency commands;
- board slugs for `--board` and board identity arguments;
- status values for `--status`;
- comment kind values for `comment add --kind`.

`kanban __complete` is an internal newline-delimited helper for shell scripts
and tests. It accepts:

```text
task-ref | dependency-task-ref | board | status | comment-kind
```

The helper must be quiet for completion use: missing DB files, uninitialized
DBs, missing board config, or read/query failures return success with no
candidates and no stderr. Static completion generation itself does not open or
create the SQLite database.

---

## 2. Exit Codes

| Code | 含义 |
|---:|---|
| 0 | 成功。 |
| 1 | 通用错误。 |
| 2 | 参数错误。 |
| 3 | 未找到对象。 |
| 4 | 非法状态转换。 |
| 5 | claim 冲突。 |
| 6 | dependency blocked。 |
| 7 | SQLite busy/locked。 |
| 8 | integrity check failed。 |

---

## 3. Init

### 3.1 `kanban init`

初始化本地 DB、默认 board、默认 columns。

```bash
kanban init
kanban init --db .kb/kb.db
kanban init --force
```

输出：

```text
Initialized Kanban database at ~/.local/share/kb/kb.db
Default board: default
```

JSON：

```json
{
  "data": {
    "db_path": "/home/alice/.local/share/kb/kb.db",
    "board": "default"
  }
}
```

---

## 4. Board Commands

### 4.1 List boards

```bash
kanban board list [--include-archived]
```

### 4.2 Create board

```bash
kanban board create <slug> --name <name> [--description <text>]
```

Example：

```bash
kanban board create agent-work --name "Agent Work"
```

### 4.3 Show board

```bash
kanban board show <slug>
```

### 4.4 Use board

```bash
kanban board use <slug-or-id>
```

Writes:

```toml
board = "agent-work"
```

to `.kb/config.toml` in the current directory.

### 4.5 Current board

```bash
kanban board current
```

Shows the resolved active board after applying `--board`, `KB_BOARD`, project config, and fallback precedence.

### 4.6 Archive board

```bash
kanban board archive <slug>
```

Archived boards are hidden from `kanban board list` unless `--include-archived` is passed. Ordinary task writes against archived boards are rejected. Audit history remains readable through task/event/run/comment history commands when the task or board can be resolved explicitly. Archiving a board with active `running` work is rejected; finish, block, or reclaim that work first.

---

## 5. Task Commands

### 5.1 Create task

```bash
kanban task create <title> [OPTIONS]
```

Options：

| Option | 说明 |
|---|---|
| `--description <text>` | Markdown 描述。 |
| `--status <status>` | 显式初始状态：triage/todo/scheduled/ready。 |
| `--assignee <name>` | assignee/worker profile。 |
| `--priority <int>` | 优先级，默认 0。 |
| `--scheduled-at <epoch_ms>` | 计划时间，Unix epoch milliseconds。 |
| `--due-at <epoch_ms>` | 截止时间，Unix epoch milliseconds。 |
| `--max-retries <n>` | worker 失败或 reclaim 后最多重试次数。 |
| `--metadata <json>` | 扩展 JSON。 |

Examples：

```bash
kanban task create "实现状态机" --priority 10
kanban task create "明早检查报告" --scheduled-at 1780640400000
```

Human output：

```text
agent-work#12 t_01HX... [ready] 实现状态机
```

JSON output：

```json
{
  "data": {
    "id": "t_01HX...",
    "board_id": "b_01HX...",
    "board_slug": "agent-work",
    "ref": "agent-work#12",
    "seq": 12,
    "status": "ready",
    "title": "实现状态机"
  }
}
```

### 5.2 List tasks

```bash
kanban task list [OPTIONS]
```

Options：

| Option | 说明 |
|---|---|
| `--status <status>` | 按状态过滤，可重复。 |
| `--assignee <name>` | 按 assignee。 |
| `--search <query>` | title/description 模糊搜索。 |
| `--include-archived` | 包含 archived。 |
| `--limit <n>` | 限制数量。 |
| `--offset <n>` | 分页偏移。 |
| `--sort <field>` | priority/created/updated/position。 |

Examples：

```bash
kanban task list
kanban task list --status ready --status running
kanban task list --assignee agent-default --json
```

### 5.3 Show task

```bash
kanban task show <task_ref>
kanban task show <task_ref> --details
```

Default human output remains the compact one-line task summary:

```text
agent-work#12 t_01HX... [ready] 实现状态机
```

`--details` switches only human output to a readable field list. It includes the
task ref/id/status/title, full multiline description, assignee, priority,
scheduled_at, due_at, created_at, updated_at, and other task snapshot fields when
available. `--json task show` returns the same `TaskRecord` envelope with or
without `--details`.

`task_ref` 支持：

- `t_...`：全局 task id，忽略 active board。
- `12`：当前 active board 内的 seq。
- `#12`：当前 active board 内的 seq；shell 中需要引号，例如 `'#12'`。
- `agent-work#12`：显式 board slug + seq。
- `agent-work/#12`：兼容 alias/#seq 形式。
- `b_01HX...#12`：显式 board id + seq。

裸 `12` / `#12` 依赖 active board；显式 `board#seq` 和 `t_...` 可跨 active board 使用。跨 board dependency 在当前版本中会被拒绝。

### 5.4 Update task fields

```bash
kanban task update <task_ref> [OPTIONS]
```

允许更新：

- title
- description
- assignee
- priority
- scheduled_at
- due_at
- max_retries
- metadata

不允许通过 update 修改 status；status 必须通过 transition command。

Examples：

```bash
kanban task update 12 --priority 20
kanban task update t_01HX --description "新的规格"
kanban task update t_01HX --max-retries 2
kanban task update t_01HX --clear-max-retries
```

---

## 6. Transition Commands

### 6.1 Promote

```bash
kanban task promote <task_ref>
```

手动尝试 `todo/scheduled -> ready`。

### 6.2 Start / Claim

```bash
kanban task start <task_ref> [OPTIONS]
kanban task claim <task_ref> [OPTIONS]
```

`start` 是 `claim` 的人类友好 alias。

Options：

| Option | 说明 |
|---|---|
| `--ttl-ms <ms>` | claim TTL。默认 300000。 |

Output：

```text
Claimed t_01HX... token=ct_01HX...
```

JSON：

```json
{
  "data": {
    "task_id": "t_01HX...",
    "run_id": "r_01HX...",
    "claim_token": "ct_01HX...",
    "claim_expires_at": 1717520000000
  }
}
```

### 6.3 Heartbeat

```bash
kanban task heartbeat <task_ref> --claim-token <token>
```

Options：

| Option | 说明 |
|---|---|
| `--ttl-ms <ms>` | 延长 TTL。 |

### 6.4 Done / Complete

```bash
kanban task done <task_ref> --claim-token <token>
kanban task complete <task_ref> --claim-token <token>
```

Options：

| Option | 说明 |
|---|---|
| `--claim-token <token>` | active claim token。 |
| `--force` | 强制完成 running task；仅本地人工修复使用。 |

### 6.5 Submit Review

```bash
kanban task review <task_ref> --claim-token <token>
```

使 task 从 `running` 到 `review`。

### 6.6 Block

```bash
kanban task block <task_ref> <reason>
```

Options：

| Option | 说明 |
|---|---|
| `--claim-token <token>` | running task block 时需要。 |
| `--force` | 强制 block。 |

### 6.7 Unblock

```bash
kanban task unblock <task_ref>
```

不会盲目进入 ready，而是根据 spec、schedule、dependencies 重新计算目标状态。

### 6.8 Reclaim

```bash
kanban task reclaim --expired
kanban task reclaim
```

当前 CLI reclaim 处理 active board 内 expired claims；裸 `kanban task reclaim` 与 `kanban task reclaim --expired` 等价。

### 6.9 Archive

```bash
kanban task archive <task_ref>
```

Options：

| Option | 说明 |
|---|---|
| `--force` | 允许 archive running task，并关闭 active run。 |

---

## 7. Dependency Commands

```bash
kanban dep add <parent_ref> <child_ref>
kanban dep remove <parent_ref> <child_ref>
kanban dep list <task_ref>
```

添加 dependency 后：

- 如果 child 当前是 `ready` 且 parent 未完成，child 降级为 `todo`。
- 如果产生环，返回 exit code 6 或 invalid input。
- 当前版本拒绝跨 board dependency，即使 parent/child 通过全局 `t_...` 或显式 `board#seq` 解析成功。

---

## 8. DAG Commands

```bash
kanban dag show
kanban dag show --json
```

`kanban dag show` returns an LLM-friendly snapshot for the active board. The
CLI calls the SQLite service query; it does not assemble graph SQL in the CLI.
Human output is a concise summary. JSON output is the stable contract and uses
the standard envelope:

```json
{
  "data": {
    "board": {
      "id": "b_...",
      "slug": "default",
      "name": "Default"
    },
    "snapshot": {
      "generated_at": 1717520000000,
      "node_count": 3,
      "edge_count": 1,
      "sort": [
        "priority asc",
        "due_at asc nulls last",
        "scheduled_at asc nulls last",
        "dependency fan-out desc",
        "created_at asc",
        "ref asc",
        "id asc"
      ]
    },
    "raw": {
      "nodes": [
        {
          "id": "t_...",
          "ref": "default#1",
          "seq": 1,
          "title": "Implement state machine",
          "status": "ready",
          "priority": 10,
          "due_at": null,
          "scheduled_at": null,
          "created_at": 1717520000000,
          "archived_at": null,
          "why": "default#1 is currently ready"
        }
      ],
      "edges": [
        {
          "parent": "t_parent",
          "child": "t_child",
          "why": "t_parent must finish before t_child can run"
        }
      ]
    },
    "derived": {
      "blocked_by": [
        {
          "task_id": "t_child",
          "tasks": ["t_parent"],
          "why": "default#2 is blocked by default#1"
        }
      ],
      "unblocks": [
        {
          "task_id": "t_parent",
          "tasks": ["t_child"],
          "why": "default#1 unblocks default#2"
        }
      ],
      "actionable": [
        {
          "task_id": "t_ready",
          "ref": "default#3",
          "why": "default#3 is ready with no unfinished parent dependencies"
        }
      ],
      "frontier": [
        {
          "task_id": "t_ready",
          "ref": "default#3",
          "why": "default#3 is frontier because it is ready and all parent dependencies are done or absent"
        }
      ]
    }
  }
}
```

Frontier v1 includes only unarchived `todo` and `ready` tasks with no unfinished
parent dependencies. It excludes `done`, `archived`, `blocked`, `running`, and
`review` tasks. Nodes and frontier entries use the documented stable sort:
priority ascending (P0 -> P3), due date ascending with nulls last, scheduled time
ascending with nulls last, dependency fan-out descending, created time
ascending, then task ref and id.

---

## 9. Comment Commands

```bash
kanban comment add <task_ref> <body> [--kind text|system|worker] [--author-type human|agent|system] [--agent-type <type>]
kanban comment list <task_ref>
```

`--actor` supplies the comment author display identity. If `--kind` is omitted,
the service default is `text`. If `--author-type` is omitted, the service infers
`worker -> agent`, `system -> system`, and otherwise `human`. `--agent-type` is
allowed only with `--author-type agent`.

Human output is compact and includes comment id, task id, created_at, kind,
author identity, author_type, optional agent_type, and body:

```text
c_01HX... task=t_01HX... created_at=1717520000000 [text] alice (human): ready for review
c_01HX... task=t_01HX... created_at=1717520000100 [worker] worker-a (agent/root): tests passed
```

JSON output uses the standard envelope and returns `CommentRecord` for `add` or
`Vec<CommentRecord>` for `list`. Creating a comment writes
`task_events(kind='task.comment.created')`.

---

## 10. Event Commands

```bash
kanban events <task_ref>
kanban events --board default
```

不传 `<task_ref>` 时按 active board 列出 events。Archived board 的 events 仍可通过显式 `--board` 读取。

---

## 11. Run Commands

```bash
kanban runs <task_ref>
kanban run show <run_id>
kanban run logs <run_id>
kanban run logs <run_id> --tail-bytes 65536
```

`kanban run logs` 默认最多读取 256 KiB。传 `--tail-bytes` 时只返回 log 末尾指定字节数。`task_runs.log_path` 必须解析到受信任日志目录且文件名匹配 `<run_id>.log`；可疑路径会被拒绝。

---

## 12. Dispatcher / Server Commands

```bash
kanban serve
kanban serve --search-sync-interval-ms 5000

kanban dispatch
kanban dispatch --once
kanban dispatch --worker-profile default
kanban dispatch --worker-profile backend --profile-config ./workers.toml
kanban dispatch --max-iterations 10 --poll-interval-ms 1000
```

`kanban dispatch` is a foreground loop. Use `--once` for one pass, or `--max-iterations`
for bounded scripts/tests. `--profile-config` reads the selected `[workers.<name>]`
section and can set `command`, `claim_ttl_ms`, `heartbeat_interval_ms`,
`on_success`, `on_failure`, and `log_dir`. Dispatcher log directories must be
inside a trusted run-log root: the platform default run log directory,
`<db_dir>/logs`, or `<db_dir>/.kb/logs`.

`kanban serve` starts a conservative background search sync loop when the binary is
built with `tantivy-backend`. The loop makes one prompt startup attempt and then
calls `sync_search_index` every `--search-sync-interval-ms` milliseconds
(default `5000`). Use `--search-sync-interval-ms 0` to disable it. Without
`tantivy-backend`, the flag is accepted and no background index task is started.

---

## 13. Search Commands

### 13.1 `kanban search`

```bash
kanban search <query> [--status ready] [--status review] [--assignee worker-a] [--include-archived] [--limit 20] [--offset 0] [--json]
```

默认实现使用 SQLite fallback，不依赖外部/派生索引。启用 `tantivy-backend` feature 且 `index/v1/tasks/` 存在可读 Tantivy 索引时，`kanban search` 使用 Tantivy；缺失或损坏时回落 SQLite，并在 meta 中标记 stale。搜索匹配 task title、description、comments、run summary/error、event kind/payload。

Human output compactly includes seq/id, status, score, title, and snippet when available:

```text
#12 t_01HX... [ready] score=60.0 实现状态机 - ready spec needle
```

JSON output:

```json
{
  "data": {
    "hits": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "score": 60.0,
        "snippet": "ready spec needle",
        "task": {
          "id": "t_01HX...",
          "seq": 12,
          "status": "ready",
          "title": "实现状态机"
        }
      }
    ],
    "meta": {
      "backend": "sqlite",
      "stale": false,
      "index_version": null,
      "last_event_id": 42,
      "index_lag_events": 0
    }
  }
}
```

### 13.2 `kanban index`

```bash
kanban index status
kanban index doctor
kanban index rebuild
kanban index sync
```

默认 backend 是 SQLite fallback。启用 `tantivy-backend` feature 时，Tantivy index 是可重建 derived cache：

- `status` returns backend/meta.
- `doctor` returns the same fallback health meta for scripts.
- `rebuild` builds/replaces `index/v1/tasks/` beside the SQLite DB and stores a clean high-watermark state in `app_settings`.
- `sync` consumes `task_events.id` after the stored high-watermark, delete+reindexes affected task aggregates, then advances the high-watermark only after a successful commit.
- Task mutations do not update Tantivy inside their transactions; run `kanban index sync` after changes, rely on `kanban serve` background sync for local server/desktop sessions, or use `kanban index rebuild` to replace the derived index.

The persisted setting key is board-scoped as `search.tasks.state.<board_id>`. Its JSON contains `schema_version`, `index_version`, `backend`, `index_name`, `board_id`, `last_event_id`, `dirty`, `updated_at`, and optional `message`; it is included in JSONL export/import through existing `app_settings` handling.

JSON data shape:

```json
{
  "data": {
    "backend": "sqlite",
    "derived_index": false,
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0,
    "message": "SQLite fallback search is active; no derived index exists yet"
  }
}
```

With Tantivy enabled after rebuild, `backend` is `tantivy`, `derived_index` is `true`, and `index_version` is `tasks-v1`.
When the current `MAX(task_events.id)` is greater than the stored `last_event_id`, `stale=true` and `index_lag_events` reports the event lag. Search falls back to SQLite while stale to preserve current-result correctness.
Background sync errors do not make search fail open to stale Tantivy results; the next search still reports stale/fallback metadata and returns current SQLite results when the derived index is behind or unusable.

---

## 14. Maintenance Commands

```bash
kanban doctor
kanban stats
kanban backup --out backup.sqlite
kanban export --format jsonl --out board.jsonl
kanban import --input board.jsonl --replace
kanban vacuum
kanban checkpoint

kanban entity list [--kind task] [--limit 50]
kanban entity show kb://task/t_...
kanban outbox list [--status pending] [--limit 50]
kanban derived status
kanban graph status
kanban graph neighbors kb://task/t_... [--predicate depends_on] [--limit 50]
kanban vector status
kanban context build t_... [--lexical-limit 5]
```

`kanban stats --json` 返回 status counts、过期 running claim 列表和 blocked reason 聚合，用于本地 operator recovery。

`kanban backup` 使用 SQLite `VACUUM INTO` 创建一致备份；目标文件已存在时失败，避免覆盖。
`kanban export --format jsonl` 导出数据库记录；目标文件已存在时失败，避免覆盖旧 snapshot。JSONL 不复制 `task_runs.log_path` 指向的外部日志文件，导出的 run 记录会清空 `log_path`；导出中的 live `running` task 会清除 claim 并恢复为 `ready`，对应 running run 会落为 `canceled`，并追加 `task.export_sanitized` 事件解释这次 portable snapshot 改写。需要完整可恢复副本时使用 `kanban backup`。
`kanban import` 是替换式恢复入口，必须显式传 `--replace`；导入文件必须至少包含一个 board，且每个 board 必须包含 columns。`kanban import --replace` 是 offline-only 操作；运行前必须停止 `kanban serve` 和常驻 `kanban dispatch`，如果检测到 active runtime lock 会直接拒绝。
`kanban entity`、`kanban outbox`、`kanban derived` 是 Knowledge Substrate 的只读维护入口。SQLite 仍是事实源；这些命令只报告统一 entity registry、派生索引 outbox 和 derived store 状态，不改变 task 状态或 claim。
`kanban graph` 和 `kanban vector` 是 feature-gated 派生层入口：未启用 `graph-oxigraph` / `vector-lancedb` 或缺少 embedding provider 时返回 disabled/degraded status；启用后仍只作为可重建 relation/vector store，不参与 task 状态事务。
`kanban context build` 通过 SQLite hydrate canonical task，并合并 lexical、graph、vector hits。graph/vector 不可用或失败时返回 degraded markers；失败原因通过有界 diagnostics 暴露，context pack 本身仍可用。

`kanban derived status` 中的 `last_event_id` 是 store 级成功处理水位，不是当前 board 的局部水位。`dirty=true` 表示该 store 仍有任意 board 的 pending/running/failed outbox，或最近一次派生更新失败；board-scoped `kanban index sync`、`kanban graph sync`、`kanban vector sync` 只清理当前 board 的 job，不能因为本 board clean 就强制清掉全局 dirty。

### 14.1 `kanban doctor`

检查：

- DB 文件存在。
- migrations 完整。
- `PRAGMA integrity_check`。
- orphan active run。
- running task 是否缺 claim。
- expired claim 数量。
- dependency cycle。
- archived dependency edge。
- 缺失 run log 文件。
- 可疑 run log 路径。
- `ready/running` task 带有未完成 parent dependency。
- `ready/running` task 缺少可执行 spec。
- `ready/running` task 带有未来 `scheduled_at`。
- `index_outbox` backlog：`outbox_pending`、`outbox_running`、`outbox_failed`。
- derived store health：`derived_dirty_stores`、`derived_error_stores`、`derived_stores[]`，每个 store 包含 `dirty`、`last_error` 和按 store target 聚合的 pending/running/failed outbox 计数。

`dirty` / pending outbox 表示派生层需要 sync/rebuild，不会改变 SQLite task truth；failed outbox 或 `last_error` 用于 operator 判断是否需要 `kanban index sync`、`kanban graph sync/rebuild` 或 `kanban vector sync/rebuild`。`derived_stores[].last_event_id` 表示对应 store 已成功提交的全局 event watermark；当 `dirty=true` 时，它仍然只是“已成功处理到哪里”的摘要，不代表所有 board 都已经干净。

---

## 15. JSON Output Contract

成功：

```json
{
  "data": {},
  "meta": {}
}
```

失败：

```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot claim task from status todo",
    "details": {
      "task_id": "t_...",
      "status": "todo"
    }
  }
}
```

stderr：

- human 模式：错误写 stderr。
- JSON 模式：错误 JSON 写 stdout 或 stderr 需要固定；建议 stderr，stdout 保持空。

---

# File: docs/API_SPEC.md

# Local Web API SPEC

本 API 只面向 localhost Web UI 和本地脚本。它不是远程协作 API。

默认监听：

```text
127.0.0.1:8721
```

Base path：

```text
/api/v1
```

---

## 1. 通用约定

### 1.1 Content Type

Request：

```http
Content-Type: application/json
```

Response：

```http
Content-Type: application/json
```

SSE：

```http
Content-Type: text/event-stream
```

### 1.2 Actor

因为没有多用户系统，actor 是审计字段。

来源优先级：

1. Request body `actor`。
2. Header `X-KB-Actor`。
3. Server 默认 actor。
4. OS username。

### 1.3 Success Response

```json
{
  "data": {},
  "meta": {}
}
```

### 1.4 Error Response

```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot claim task from status todo",
    "details": {
      "task_id": "t_...",
      "status": "todo"
    }
  }
}
```

### 1.5 HTTP Status Mapping

| Error code | HTTP status |
|---|---:|
| `invalid_input` | 400 |
| `not_found` | 404 |
| `invalid_transition` | 409 |
| `dependency_blocked` | 409 |
| `claim_conflict` | 409 |
| `claim_token_mismatch` | 403 |
| `db_busy` | 503 |
| `internal` | 500 |

---

## 2. Health

### `GET /health`

Response：

```json
{
  "data": {
    "ok": true,
    "db": "ok",
    "version": "1.3.0"
  }
}
```

---

## 3. Boards

### 3.1 List boards

```http
GET /api/v1/boards
```

Response：

```json
{
  "data": [
    {
      "id": "b_01HX...",
      "slug": "default",
      "name": "Default",
      "description": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000,
      "archived_at": null
    }
  ]
}
```

### 3.2 Create board

```http
POST /api/v1/boards
```

Request：

```json
{
  "slug": "agent-work",
  "name": "Agent Work",
  "description": "Local agent board",
  "actor": "alice"
}
```

### 3.3 Get board

```http
GET /api/v1/boards/{board_slug_or_id}
```

### 3.4 Archive board

```http
POST /api/v1/boards/{board}/archive
```

---

## 4. Tasks

### 4.1 List tasks

```http
GET /api/v1/boards/{board}/tasks
```

Query params：

| Param | 说明 |
|---|---|
| `status` | 可重复：`?status=ready&status=running`。 |
| `assignee` | 按 assignee。 |
| `label` | 按 label。 |
| `q` | title/description 搜索。 |
| `include_archived` | bool。 |
| `limit` | 默认 100。 |
| `offset` | 分页 offset。 |
| `sort` | `position` / `priority` / `created_at` / `updated_at`。 |

Response：

```json
{
  "data": [
    {
      "id": "t_01HX...",
      "seq": 12,
      "board_id": "b_01HX...",
      "title": "实现状态机",
      "description": "...",
      "status": "ready",
      "priority": 10,
      "position": 1024,
      "assignee": null,
      "scheduled_at": null,
      "due_at": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000
    }
  ],
  "meta": {
    "limit": 100,
    "offset": 0
  }
}
```

### 4.2 Create task

```http
POST /api/v1/boards/{board}/tasks
```

Request：

```json
{
  "title": "实现状态机",
  "description": "Markdown spec",
  "status": "ready",
  "assignee": "local-worker",
  "priority": 10,
  "scheduled_at": null,
  "due_at": null,
  "depends_on": ["t_01HX..."],
  "labels": ["core"],
  "metadata": {},
  "actor": "alice"
}
```

Notes：

- `status` 只能是 `triage|todo|scheduled|ready`。
- 若不传 `status`，服务端计算初始状态。
- 若存在未完成 dependencies，不能创建为 `ready`。

### 4.3 Get task

```http
GET /api/v1/tasks/{task_id}
```

Query params：

| Param | 说明 |
|---|---|
| `include` | `comments,runs,events,dependencies,labels`。 |

### 4.4 Update task fields

```http
PATCH /api/v1/tasks/{task_id}
```

允许字段：

```json
{
  "title": "新的标题",
  "description": "新的描述",
  "assignee": "worker-a",
  "priority": 20,
  "scheduled_at": 1717520000000,
  "due_at": 1717600000000,
  "metadata": {},
  "actor": "alice",
  "expected_lock_version": 7
}
```

禁止字段：

- `status`
- `claim_token`
- `current_run_id`
- `completed_at`

状态必须通过 transition endpoint 修改。

---

## 5. Transitions

### 5.1 Specify

```http
POST /api/v1/tasks/{task_id}/transitions/specify
```

Request：

```json
{
  "description": "补全后的规格",
  "scheduled_at": null,
  "actor": "alice"
}
```

### 5.2 Promote

```http
POST /api/v1/tasks/{task_id}/transitions/promote
```

Request：

```json
{
  "actor": "dispatcher"
}
```

### 5.3 Claim / Start

```http
POST /api/v1/tasks/{task_id}/transitions/claim
```

Request：

```json
{
  "actor": "alice",
  "ttl_ms": 300000,
  "worker_profile": null,
  "metadata": {}
}
```

Response：

```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "status": "running"
    },
    "run": {
      "id": "r_01HX...",
      "status": "running"
    },
    "claim_token": "ct_01HX...",
    "claim_expires_at": 1717520300000
  }
}
```

### 5.4 Heartbeat

```http
POST /api/v1/tasks/{task_id}/transitions/heartbeat
```

Request：

```json
{
  "claim_token": "ct_01HX...",
  "ttl_ms": 300000,
  "note": "still running",
  "actor": "worker-default"
}
```

### 5.5 Complete

```http
POST /api/v1/tasks/{task_id}/transitions/complete
```

Request：

```json
{
  "claim_token": "ct_01HX...",
  "summary": "实现完成，测试通过",
  "result": {},
  "force": false,
  "actor": "worker-default"
}
```

### 5.6 Submit Review

```http
POST /api/v1/tasks/{task_id}/transitions/submit-review
```

Request：

```json
{
  "claim_token": "ct_01HX...",
  "summary": "等待人工检查",
  "actor": "worker-default"
}
```

### 5.7 Block

```http
POST /api/v1/tasks/{task_id}/transitions/block
```

Request：

```json
{
  "reason": "等待 API schema 确认",
  "claim_token": null,
  "force": false,
  "actor": "alice"
}
```

### 5.8 Unblock

```http
POST /api/v1/tasks/{task_id}/transitions/unblock
```

Request：

```json
{
  "actor": "alice"
}
```

Response target 由服务端计算，不由客户端指定。

### 5.9 Reclaim

```http
POST /api/v1/tasks/{task_id}/transitions/reclaim
```

Request：

```json
{
  "force": false,
  "to_status": "ready",
  "reason": "claim expired",
  "actor": "dispatcher"
}
```

### 5.10 Archive

```http
POST /api/v1/tasks/{task_id}/transitions/archive
```

Request：

```json
{
  "force": false,
  "actor": "alice"
}
```

---

## 6. Dependencies

### 6.1 Add dependency

```http
POST /api/v1/tasks/{child_task_id}/dependencies
```

Request：

```json
{
  "parent_task_id": "t_01HX...",
  "actor": "alice"
}
```

### 6.2 Remove dependency

```http
DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}
```

### 6.3 List dependencies

```http
GET /api/v1/tasks/{task_id}/dependencies
```

Response：

```json
{
  "data": {
    "parents": [],
    "children": []
  }
}
```

---

## 7. Comments

### 7.1 List comments

```http
GET /api/v1/tasks/{task_id}/comments
```

### 7.2 Add comment

```http
POST /api/v1/tasks/{task_id}/comments
```

Request：

```json
{
  "body": "这里需要确认边界条件。",
  "kind": "text",
  "author_type": "human",
  "agent_type": null,
  "actor": "alice"
}
```

Notes：

- `kind` 默认为 `text`，当前允许 `text|system|worker`。
- `author_type` marks who produced the comment and allows `human|agent|system`. If omitted, the service infers `worker -> agent`, `system -> system`, and all other kinds as `human`.
- `agent_type` is optional open text for `author_type=agent` comments, such as `executor` or `reviewer`. Non-empty `agent_type` with `author_type=human` or `system` is rejected as `400 invalid_input`.

---

## 8. Runs

### 8.1 List task runs

```http
GET /api/v1/tasks/{task_id}/runs
```

### 8.2 Get run

```http
GET /api/v1/runs/{run_id}
```

---

## 9. Events

### 9.1 List events

```http
GET /api/v1/events?board=default&after=0&limit=100
```

Response：

```json
{
  "data": [
    {
      "id": 123,
      "event_id": "e_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "kind": "task.claimed",
      "actor": "alice",
      "payload": {},
      "created_at": 1717520000000
    }
  ],
  "meta": {
    "next_after": 123
  }
}
```

### 9.2 SSE stream

```http
GET /api/v1/stream/events?board=default&after=123
```

SSE event：

```text
event: task.claimed
id: 124
data: {"id":124,"event_id":"e_...","task_id":"t_...","kind":"task.claimed","payload":{}}
```

Reconnect：

- Browser 使用 Last-Event-ID。
- Server 读取 `after` 或 Last-Event-ID 后继续发送。
- 若 event 已被压缩/清理，客户端重新 fetch board snapshot。

---

## 10. Columns / UI Settings

### 10.1 List columns

```http
GET /api/v1/boards/{board}/columns
```

### 10.2 Update columns

```http
PATCH /api/v1/boards/{board}/columns
```

Request：

```json
{
  "columns": [
    {"id": "col_triage", "title": "Triage", "position": 10, "hidden": false},
    {"id": "col_done", "title": "Done", "position": 80, "hidden": false}
  ]
}
```

MVP 不允许 column 改变 canonical status。

---

## 11. Labels

```http
GET /api/v1/boards/{board}/labels
POST /api/v1/boards/{board}/labels
POST /api/v1/tasks/{task_id}/labels
DELETE /api/v1/tasks/{task_id}/labels/{label_id}
```

---

## 12. Maintenance

### 12.1 Doctor

```http
POST /api/v1/maintenance/doctor
```

### 12.2 Checkpoint

```http
POST /api/v1/maintenance/checkpoint
```

### 12.3 Backup

MVP 建议只提供 CLI backup，不开放 HTTP backup。

---

## 13. Web UI Interaction Rules

1. 拖拽列时调用 transition endpoint。
2. 普通字段编辑调用 `PATCH /tasks/{id}`。
3. Web UI 不显示 claim_token，除非 debug 模式。
4. running task 的 complete/block 操作，若无 token，则 UI 走 `force=true` 并要求确认。
5. blocked task unblock 后目标列由服务端返回，前端不要预设。
6. SSE 收到 event 后，优先 refetch affected task，避免客户端状态机漂移。



---

# File: docs/DISPATCHER_SPEC.md

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
WHERE board_id = ?
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

# File: migrations/003_comment_author_identity.sql

```sql
-- Add explicit comment author identity while preserving existing kind values.

BEGIN;

ALTER TABLE task_comments
  ADD COLUMN author_type TEXT NOT NULL DEFAULT 'human'
  CHECK(author_type IN ('human', 'agent', 'system'));

UPDATE task_comments
SET author_type = CASE kind
  WHEN 'worker' THEN 'agent'
  WHEN 'system' THEN 'system'
  ELSE 'human'
END
WHERE author_type = 'human';

ALTER TABLE task_comments
  ADD COLUMN agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (3, '003_comment_author_identity', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

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
| Task 被人工 force complete | Worker 后续 complete 失败，因 token/run 已关闭。 |
| DB integrity failed | Dispatcher 停止，提示运行 `kanban doctor`。 |

---

## 14. MVP Scope

MVP dispatcher 必须实现：

- claim one explicit `ready` task。
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


---

# File: docs/IMPLEMENTATION_PLAN.md

# Implementation Plan

本文档给出分阶段实现计划、验收标准和测试策略。

---

## Phase 0：Repository Skeleton

目标：建立 Rust workspace 和基本工程纪律。

交付：

- `Cargo.toml` workspace。
- crates：`kanban-core`、`kanban-sqlite`、`kanban-cli`。
- migrations 目录。
- lint/format/test workflow。
- error type。
- ID/time utility。

验收：

- `cargo test` 通过。
- `cargo fmt --check` 通过。
- `cargo clippy` 无关键 warning。

---

## Phase 1：SQLite Schema + Core Domain

目标：数据结构和 migration 可用。

交付：

- 执行 `001_initial.sql`。
- `kanban init`。
- 默认 board。
- 默认 columns。
- 领域类型：Board、Task、Status、Run、Event。
- status enum 与 parse/serialize。

验收：

- 新建 DB 后 `PRAGMA integrity_check` 返回 ok。
- 重复运行 `kanban init` 不破坏已有 DB。
- schema version 可查询。

测试：

- migration test。
- schema smoke test。
- enum roundtrip test。

---

## Phase 2：Task CRUD + Events

目标：任务可创建、查询、更新，事件可记录。

交付：

- `kanban task create/list/show/update`。
- `task_events` 写入。
- `--json` 输出。
- `expected_lock_version` 乐观锁。

验收：

- 创建 task 后有 `task.created` event。
- update 不允许修改 status。
- board task list 默认隐藏 archived。

测试：

- create/list/show integration test。
- event transaction test。
- invalid input test。

---

## Phase 3：State Machine Transitions

目标：核心状态机可用。

交付：

- specify。
- promote。
- claim/start。
- heartbeat。
- complete/done。
- block。
- unblock。
- reclaim。
- archive。

验收：

- 非法 transition 被拒绝。
- 每个 transition 写 event。
- running task 必须有 claim token。
- complete 后清理 claim fields。

测试：

- transition matrix unit tests。
- block/unblock target recomputation tests。
- token mismatch tests。

---

## Phase 4：Dependencies

目标：支持 parent/child 依赖和显式 manual promotion。

交付：

- `kanban dep add/remove/list`。
- cycle detection。
- dependency-aware create/promote/claim。
- parent complete 后不自动 promote children；child 保持 `todo`，由 derived dependency fields 表达是否仍被阻塞。

验收：

- child 依赖未完成 parent 时不能 ready/running。
- parent 完成后 child 可 promotion。
- cycle 添加失败。

测试：

- direct cycle。
- indirect cycle。
- child demotion when dependency added。
- promotion after completion。

---

## Phase 5：Runs + Dispatcher MVP

目标：可执行任务并恢复崩溃任务。

交付：

- `task_runs` 写入。
- `kanban dispatch --once`。
- `kanban dispatch` loop。
- worker profile command。
- heartbeat wrapper。
- expired reclaim。
- run logs。

验收：

- ready task 被 dispatcher claim 并执行。
- command exit 0 后 task done/review。
- command exit non-zero 后 task blocked/ready。
- worker timeout 后 reclaim/block。
- dispatcher crash 后 task 可被 reclaim。

测试：

- claim race test，多线程同时 claim 同 task 只有一个成功。
- worker success integration。
- worker failure integration。
- expired claim reclaim test。

---

## Phase 6：Local Web API

目标：Web 端可通过 HTTP 操作 board/task。

交付：

- `kanban serve`。
- REST endpoints。
- unified error response。
- SSE event stream。
- health endpoint。

验收：

- Web API 能完成 CLI 同等生命周期。
- SSE 收到 task events。
- API 不能 PATCH status。
- 默认只监听 127.0.0.1。

测试：

- route integration tests。
- API transition tests。
- SSE reconnect test。

---

## Phase 7：Web UI MVP

目标：基本看板 UI 可用。

交付：

- Board columns。
- Task cards。
- Task detail drawer。
- Create/update task。
- Drag/drop 调用 transition。
- Comments。
- Event timeline。
- Run history。
- SSE live refresh。

验收：

- UI 不直接修改 status。
- blocked unblock 后根据服务端返回移动列。
- running complete force 时有确认。

---

## Phase 8：Maintenance & Hardening

目标：本地数据可靠性。

交付：

- `kanban doctor`。
- `kanban backup`。
- `kanban checkpoint`。
- `kanban vacuum`。
- JSONL export。
- orphan run 检查。

验收：

- backup 可恢复。
- integrity check 可报告问题。
- expired running task 可列出并 reclaim。

---

## Testing Strategy

### Unit Tests

- status parse/format。
- transition guard。
- initial status computation。
- unblock target computation。
- dependency cycle detection。

### SQLite Integration Tests

- migration fresh DB。
- migration idempotency。
- FK constraints。
- JSON validation constraints。
- CAS claim affected rows。

### Concurrency Tests

- 10/50/100 concurrent claim attempts on one task。
- CLI + server simultaneous writes。
- busy timeout behavior。

### Dispatcher Tests

- successful worker。
- failed worker。
- timeout worker。
- heartbeat extension。
- reclaim expired。

### CLI Golden Tests

- human output stable enough。
- JSON output exact contract。
- exit code mapping。

### API Tests

- task lifecycle。
- invalid transition HTTP 409。
- error body format。
- SSE stream ordering。

---

## Definition of Done for MVP

MVP 完成定义：

1. `kanban init` 创建可用 DB。
2. `kanban task create/list/show/update` 可用。
3. `kanban task start/heartbeat/done/block/unblock/archive` 可用。
4. dependencies 可用。
5. events 可用。
6. runs 可用。
7. dispatcher 可执行本地命令。
8. web API 覆盖核心 lifecycle。
9. web UI 可视化 board。
10. 并发 claim 测试稳定通过。
11. `kanban doctor` 能发现基本数据异常。

---

## Recommended First Milestone

最小可用 milestone：

```text
Phase 0 + Phase 1 + Phase 2 + Phase 3 + 部分 Phase 4
```

即：

- SQLite schema。
- CLI task lifecycle。
- 状态机。
- events。
- dependencies 基础。

先不要做 Web UI 和 dispatcher，直到状态机与 schema 稳定。



---

# File: docs/ADR.md

# Architecture Decision Records

本文件记录当前 SPEC 的关键架构决策。

---

## ADR-0001：SQLite-only

### Status

Accepted

### Context

项目明确不考虑多用户、多租户、团队协作和远程 worker。核心运行环境是本地单机，同时需要 CLI 和 Web。

### Decision

只支持 SQLite。

默认 DB：

```text
~/.local/share/kb/kb.db
```

可通过 `--db <path>` 指定项目本地 DB。

### Consequences

优点：

- 单 binary 易分发。
- CLI 使用成本低。
- 备份简单。
- 本地事务足够强。
- WAL 支持 reader/writer 并发。

代价：

- 不支持跨机器共享写入。
- 不做 server cluster。
- 一次只有一个 writer。
- 需要控制 transaction 长度。

---

## ADR-0002：Status Enum 是真相，Column 是视图

### Status

Accepted

### Context

传统 Trello-like 工具常把 list/column 视为状态。但本项目需要 dispatcher、claim、heartbeat、reclaim、run history。`running` 不是普通视觉列，而是 claim 成功后的执行状态。

### Decision

`tasks.status` 是 canonical truth。`board_columns` 只是 UI 展示映射。

### Consequences

优点：

- Web、CLI、dispatcher 遵循同一状态机。
- 可保护 `ready -> running`。
- 能支持 review/scheduled/blocked 等非纯视觉状态。

代价：

- 拖拽列不能简单 PATCH status。
- Web UI 需要根据目标列调用 transition endpoint。

---

## ADR-0003：Snapshot + Append-only Events，不做纯 Event Sourcing

### Status

Accepted

### Context

看板 UI 高频查询当前任务列表。纯 event sourcing 会让当前状态查询复杂化，需要重放事件或额外投影。

### Decision

采用：

```text
tasks snapshot + task_events append-only
```

状态变化时，snapshot update 与 event insert 必须在同一 transaction 内完成。

### Consequences

优点：

- 当前 board 查询简单。
- 事件仍可用于审计、SSE、debug。
- 实现复杂度可控。

代价：

- 需要保证 snapshot/event 一致。
- 事件不是唯一事实源。

---

## ADR-0004：CLI 可以直接访问 SQLite，但必须走 Core Service

### Status

Accepted

### Context

如果 CLI 必须依赖常驻 server，会降低本地工具可用性。直接访问 SQLite 更适合脚本和开发流。

### Decision

CLI 可以直接打开 SQLite DB，但只能调用 `kanban-core` service / `kanban-sqlite` repository，不允许绕过状态机执行裸 SQL 修改状态。

### Consequences

优点：

- 不需要 server 即可使用。
- 脚本友好。
- 和 Web 行为一致。

代价：

- 需要处理 CLI/server/dispatcher 同机并发。
- 所有状态逻辑必须集中在 core。

---

## ADR-0005：Actor 是审计字符串，不是用户模型

### Status

Accepted

### Context

项目不做多用户和权限，但仍需要知道某个操作来自谁或哪个 worker。

### Decision

保留 `actor`、`created_by`、`claim_owner` 字段。它们是字符串，不关联 users 表。

### Consequences

优点：

- 保留审计能力。
- 支持 CLI、Web、dispatcher、worker profile 区分来源。
- 不引入 RBAC 复杂度。

代价：

- 不提供权限隔离。
- actor 可被本地调用者伪造，这是预期边界。

---

## ADR-0006：Worker stdout/stderr 存文件，DB 只存摘要与路径

### Status

Accepted

### Context

运行日志可能很大。把日志 blob 放进 SQLite 会影响性能和备份体积。

### Decision

日志写入：

```text
~/.local/state/kb/logs/r_<run_id>.log
```

DB 只存：

- `log_path`
- `summary`
- `error`
- `exit_code`

### Consequences

优点：

- SQLite 保持轻量。
- 日志可直接 tail。
- 备份策略可分开处理 DB 和 logs。

代价：

- 移动 DB 时需要同时移动 logs/attachments。
- log path 需要 doctor 检查。

---

## ADR-0007：默认只监听 localhost

### Status

Accepted

### Context

不做远程服务和多用户登录。暴露到局域网会制造安全边界问题。

### Decision

`kanban serve` 默认并且建议只监听：

```text
127.0.0.1:8721
```

MVP 不提供 `0.0.0.0` 远程模式。

### Consequences

优点：

- 无需登录系统。
- 降低误暴露风险。

代价：

- 不能多人访问。
- 不能远程手机/浏览器访问。

---

## ADR-0008：状态变化必须有专用 Transition Command

### Status

Accepted

### Context

直接 PATCH `status` 容易绕过 claim/run/event/dependency guard。

### Decision

禁止普通 update 修改 status。所有状态变化都使用 command：

- specify
- promote
- claim
- heartbeat
- complete
- submit_review
- block
- unblock
- reclaim
- archive

### Consequences

优点：

- 状态机可验证。
- run/claim/event 一致。
- Web/CLI/dispatcher 行为一致。

代价：

- API 数量更多。
- UI 拖拽逻辑更复杂。


---

# File: migrations/001_initial.sql

```sql
-- Kanban Tool initial SQLite schema
-- Time convention: INTEGER unix epoch milliseconds UTC.
-- JSON convention: TEXT with CHECK(json_valid(...)).

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 120000;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;

BEGIN;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS boards (
  id TEXT PRIMARY KEY CHECK(id LIKE 'b_%'),
  slug TEXT NOT NULL UNIQUE CHECK(length(trim(slug)) > 0),
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  description TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER
);

CREATE TABLE IF NOT EXISTS board_columns (
  id TEXT PRIMARY KEY CHECK(id LIKE 'col_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  position INTEGER NOT NULL,
  hidden INTEGER NOT NULL DEFAULT 0 CHECK(hidden IN (0, 1)),
  wip_limit INTEGER CHECK(wip_limit IS NULL OR wip_limit >= 0),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, status),
  UNIQUE(board_id, position)
);

CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY CHECK(id LIKE 't_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL,

  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  description TEXT,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  status_reason TEXT,

  assignee TEXT,
  priority INTEGER NOT NULL DEFAULT 0,
  position INTEGER NOT NULL DEFAULT 0,

  scheduled_at INTEGER,
  due_at INTEGER,

  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  started_at INTEGER,
  completed_at INTEGER,
  archived_at INTEGER,

  claim_token TEXT,
  claim_owner TEXT,
  claim_expires_at INTEGER,
  last_heartbeat_at INTEGER,
  current_run_id TEXT,

  retry_count INTEGER NOT NULL DEFAULT 0 CHECK(retry_count >= 0),
  max_retries INTEGER CHECK(max_retries IS NULL OR max_retries >= 0),

  result_summary TEXT,
  result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),

  lock_version INTEGER NOT NULL DEFAULT 0 CHECK(lock_version >= 0),

  UNIQUE(board_id, seq),
  CHECK(
    (status != 'running') OR
    (claim_token IS NOT NULL AND claim_owner IS NOT NULL AND claim_expires_at IS NOT NULL)
  )
);

CREATE TABLE IF NOT EXISTS task_dependencies (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  child_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id)
);

CREATE TABLE IF NOT EXISTS task_runs (
  id TEXT PRIMARY KEY CHECK(id LIKE 'r_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,

  status TEXT NOT NULL CHECK(status IN ('running', 'succeeded', 'failed', 'canceled', 'expired')),
  worker_profile TEXT,
  worker_pid INTEGER,

  claim_token TEXT NOT NULL,
  claim_owner TEXT NOT NULL,
  claim_expires_at INTEGER NOT NULL,

  started_at INTEGER NOT NULL,
  last_heartbeat_at INTEGER,
  finished_at INTEGER,

  exit_code INTEGER,
  summary TEXT,
  error TEXT,
  log_path TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json))
);

CREATE TABLE IF NOT EXISTS task_comments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author TEXT NOT NULL,
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'text' CHECK(kind IN ('text', 'system', 'worker')),
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK(event_id LIKE 'e_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  actor TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_attachments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'a_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  filename TEXT NOT NULL CHECK(length(trim(filename)) > 0),
  rel_path TEXT NOT NULL CHECK(length(trim(rel_path)) > 0),
  content_type TEXT,
  size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
  sha256 TEXT,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS labels (
  id TEXT PRIMARY KEY CHECK(id LIKE 'l_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  color TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, name)
);

CREATE TABLE IF NOT EXISTS task_labels (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(task_id, label_id)
);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL CHECK(json_valid(value_json)),
  updated_at INTEGER NOT NULL
);

-- Indexes: tasks
CREATE INDEX IF NOT EXISTS idx_tasks_board_status_position
  ON tasks(board_id, status, position);

CREATE INDEX IF NOT EXISTS idx_tasks_board_priority_created
  ON tasks(board_id, priority DESC, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_tasks_assignee_status
  ON tasks(board_id, assignee, status);

CREATE INDEX IF NOT EXISTS idx_tasks_scheduled
  ON tasks(board_id, status, scheduled_at);

CREATE INDEX IF NOT EXISTS idx_tasks_claim_expiry
  ON tasks(board_id, status, claim_expires_at);

CREATE INDEX IF NOT EXISTS idx_tasks_updated
  ON tasks(board_id, updated_at DESC);

-- Indexes: dependencies
CREATE INDEX IF NOT EXISTS idx_deps_child
  ON task_dependencies(child_task_id);

CREATE INDEX IF NOT EXISTS idx_deps_parent
  ON task_dependencies(parent_task_id);

-- Indexes: runs
CREATE INDEX IF NOT EXISTS idx_runs_task_started
  ON task_runs(task_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_runs_status
  ON task_runs(board_id, status, started_at DESC);

-- Indexes: comments
CREATE INDEX IF NOT EXISTS idx_comments_task_created
  ON task_comments(task_id, created_at ASC);

-- Indexes: events
CREATE INDEX IF NOT EXISTS idx_events_board_id
  ON task_events(board_id, id ASC);

CREATE INDEX IF NOT EXISTS idx_events_task_created
  ON task_events(task_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_events_kind_created
  ON task_events(kind, created_at DESC);

-- Indexes: labels
CREATE INDEX IF NOT EXISTS idx_task_labels_label
  ON task_labels(label_id, task_id);

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at)
VALUES (1, '001_initial', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
```

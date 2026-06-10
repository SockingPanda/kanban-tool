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
kanban board create agent-work --name "Agent Work"
kanban board use agent-work
kanban task create "实现 SQLite schema"
kanban task list --status ready
kanban task show agent-work#1
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
- `--board <slug-or-id>`：显式指定 active board。
- `--actor <name>`：覆盖 actor。
- 稳定退出码。

Active board 选择顺序是 `--board`、`KB_BOARD`、最近 `.kb/config.toml`、`default`。`kanban board use <board>` 写入项目级 `.kb/config.toml`，但仍使用同一个全局 SQLite DB。Task ref 必须支持全局 `t_...`、当前 board 的裸 seq / `#seq`、以及显式 `board#seq` / `board/#seq`；CLI 和 API 输出应带可复制的 `board_slug#seq` ref。

CLI 见 [`CLI_SPEC.md`](CLI_SPEC.md)。

---

## 8. Dispatcher 能力

Dispatcher 是本地可选组件。它不负责多人协作，只负责本地自动化：

1. 把满足条件的 `todo/scheduled` 提升为 `ready`。
2. 从 `ready` 中 claim 任务。
3. 为 claim 创建 `task_runs`。
4. 运行 worker profile。
5. 周期性 heartbeat。
6. 超时或崩溃后 reclaim。
7. 根据 worker exit status 写入 `done/review/blocked/ready`。

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
9. Archived board 不接受普通 task/comment/dispatcher 写入；只读 events/runs/comments 历史仍可审计。
10. Board archive 不会改变 task 状态；如果 board 上仍有 `running` task/run，必须拒绝 archive。
11. 每次状态变化必须写 `task_events`。
12. task snapshot 与对应 event 必须同 transaction 提交。

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

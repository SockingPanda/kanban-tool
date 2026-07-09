# Architecture

本架构面向本地单机运行：Rust workspace、SQLite-only、CLI、localhost Web server、可选 dispatcher。

---

## 1. 总体架构

```text
Web UI
  -> kanban-server handlers/DTO
        \
kanban-cli \
dispatcher  -> kanban-application API / DTO contracts
                     | implemented by kanban-sqlite::api / SqliteApplication
                     | uses kanban-core pure state-machine helpers
                     v
                canonical SQLite WAL
                     |
                     | task_events / index_outbox / dirty-generation markers
                     v
                rebuildable derived stores
                (Tantivy / Oxigraph / LanceDB)
```

当前实现已经把 adapter-facing DTO/port 合同抽到 `kanban-application`。CLI、HTTP
server、desktop 和 dispatcher 通过 `kanban_sqlite::api` 或 `SqliteApplication` 进入同一组
SQLite-backed use cases；`kanban-sqlite::service` 仍是 transaction、状态机 guard、canonical
writes、events、runs、outbox 和 provenance 的 implementation owner。`kanban-core` 承载
`TaskStatus`、ID/error/clock 和纯状态机 helper，不拥有持久化 records。

`kanban-sqlite` crate root 不再 re-export DB/init/service 符号。生产 adapter 必须导入
`kanban_sqlite::api`、`kanban_sqlite::application::SqliteApplication`，或显式的
`kanban_sqlite::db` / `kanban_sqlite::init` 基础设施模块。测试 raw inspection 入口集中到
`kanban-test-support`，crate 内部测试可使用显式 `db` / `init` 模块。

可把系统按六个运行平面理解：

| 平面 | 当前内容 | 写权限边界 |
|---|---|---|
| Interaction/adapters | `kanban-cli`、`kanban-server`、desktop、dispatcher 入口 | 转换输入/输出和 locale/message 渲染，不直接写 SQLite truth |
| Application contracts | `kanban-application` DTO/port API，SQLite 实现位于 `kanban-sqlite` | adapters 依赖稳定 API/DTO，不直接依赖 root legacy path |
| Domain/state machine | `kanban-core` 的 status、guard 和 recompute helper | 纯逻辑，不访问 SQLite/HTTP/CLI |
| Canonical SQLite truth | tasks/status、dependencies、labels、semantics、proposals、ontology ledger | 只能由 service path 写入 |
| Propagation/control plane | `task_events`、`index_outbox`、dirty/generation/status markers | 记录同步水位和恢复入口，不替代 truth |
| Rebuildable derived stores | Tantivy、Oxigraph、LanceDB `kb_chunks` / `kb_label_atoms` | 可删除重建，无 canonical write path |

---

## 2. Crate 结构

当前主要仓库结构（省略 tests、scripts、生成文件和部分支持文件）：

```text
crates/
  kanban-core/
    src/
      domain/
      state_machine.rs
      error.rs
      clock.rs
      id.rs

  kanban-sqlite/
    src/
      db.rs
      init.rs
      service.rs
      service/
        sql.rs
        transaction.rs
        boards.rs
        tasks.rs
        transitions.rs
        dispatch.rs
        search.rs
        ...

  kanban-cli/
    src/
      main.rs
      commands/
      output.rs

  kanban-server/
    src/
      dto.rs
      handlers/
      router.rs
      state.rs

  kanban-context/
  kanban-entity/
  kanban-graph/
  kanban-indexer/
  kanban-labels/
  kanban-local/
  kanban-search/
  kanban-vector/

apps/
  desktop/
```

Desktop package 由 Tauri 构建，内置 `kanban-vector-lancedb` 与
`kanban-graph-oxigraph` helper sidecars。Desktop 启动 embedded server 时把已存在的
bundled helper path 注入 `kanban-server::AppState`；CLI `.deb` 仍由
`scripts/package-cli-linux.sh` 独立安装 `/usr/bin/kanban` 与 `/usr/lib/kanban/` helpers。

### 2.1 `kanban-core`

职责：

- 定义基础领域类型：`Board`、`BoardColumn`、`TaskStatus`。
- 提供 typed ID、clock 和统一错误类型。
- 实现纯状态机、readiness recompute 与 transition guard helper。
- 提供轻量 locale 与 message rendering helper；只渲染用户可见文案，不翻译 canonical status、ID、JSON key 或数据库值。
- 不依赖 SQLite、HTTP、CLI、前端。
- 当前不定义完整 command input/output，也不定义 application service interface。
  这些 use-case orchestration 和持久化 records 主要在 `kanban-sqlite::service`。

示例：

```rust
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

pub fn initial_status(...) -> TaskStatus;
pub fn recompute_ready_status(...) -> TaskStatus;
pub fn can_promote_from(status: TaskStatus) -> bool;
pub fn can_complete_from(status: TaskStatus) -> bool;
```

### 2.2 `kanban-sqlite`

职责：

- SQLite 连接初始化。
- migrations。
- transaction 封装。
- application/service orchestration 与 repository 实现。
- 复杂查询。
- CAS claim。
- append event。
- task/comment/dependency/run/label/ontology use cases。
- label proposal validation / persistence，以及 `LabelProposalProvider` trait 边界。

Public API 边界：

- `kanban_sqlite::service` 是 implementation owner，负责 transaction、状态机 guard、
  canonical writes、events、runs 和 provenance。
- `kanban_sqlite::api` 是 adapter-facing facade，用于 CLI、server、desktop 和 dispatcher
  contract path 复用稳定 use case、query、record 和 provenance 类型。它不拥有新的
  orchestration 语义，也不导出 DB connection helper 或 init helper。
- crate root 不再提供 `kanban_sqlite::*` legacy re-export；旧 root path 是 breaking change，
  并由 `tests/ui/root_legacy_reexport_removed.rs` 负向 compile contract 锁定。
- `kanban_sqlite::application::SqliteApplication` 实现 `kanban-application` 的 backend port，
  用于需要以 application API 组合 use case 的 adapter/benchmark 路径。

关键要求：

- 所有状态变化必须在 transaction 内完成。
- claim 必须使用 `BEGIN IMMEDIATE` 或等价机制抢写锁。
- 不允许业务层执行裸 SQL 更新状态。
- `kanban-sqlite` 不直接依赖 LLM SDK、HTTP AI client、runtime credentials 或外部模型
  provider。真实 label proposal provider 只能在 `kanban-server`、`kanban-cli` 本地
  runtime、或单独 `kanban-ai` / `kanban-llm` crate 中实现，再通过
  `LabelProposalProvider` trait 注入 SQLite service。

### 2.3 `kanban-cli`

职责：

- 解析命令。
- 构造 command input。
- 调用 `kanban-sqlite::service` 中的 shared use-case 函数；状态判断复用
  `kanban-core` 的纯状态机 helper。
- 输出 human table 或 JSON。
- 返回稳定 exit code。
- `--locale` / `KANBAN_LOCALE` 只选择 human 输出语言；脚本契约仍以 `--json` 为准。

CLI 可以直接打开 SQLite DB 调用 service，不需要 server 常驻。

### 2.4 `kanban-server`

职责：

- localhost HTTP API。
- 静态 Web UI hosting 或 API-only。
- SSE event stream。
- 请求 DTO 转 command input。
- 错误格式统一。
- 根据 `Accept-Language` 渲染 `error.message`；`error.code` 和 JSON shape 保持稳定。
- 通过 `AppState` 接收可选 graph/vector helper binary path；缺失时 graph/vector
  status endpoint 返回 degraded diagnostics，而不是把 helper-heavy crates 编进 server。

默认只监听：

```text
127.0.0.1:8721
```

### 2.5 Dispatcher path

职责：

- claim。
- heartbeat。
- reclaim。
- worker profile 执行。
- run result 写回。

当前没有独立 `kanban-dispatcher` crate。Dispatcher 入口由 CLI 提供，
执行路径复用同一套 `kanban-sqlite::service` 语义；`kanban serve`
不启动 dispatcher，server 同进程运行 dispatcher 仍是后续扩展。

CLI 入口：

```bash
kanban dispatch
kanban dispatch --once
```

### 2.6 `kanban-vector`

职责：

- 定义可重建向量派生层的数据结构和错误模型。
- `EmbeddingProvider` 只表示外部 embedding provider 的文本向量化能力。
- `ChunkVectorStore` 表示 task chunk derived index 的 upsert/delete/query 能力。
- `LabelAtomVectorStore` 表示 label atom derived index 的 upsert/delete/query 能力，并提供 suggestion/proposal 所需的 query-text embedding。
- `VectorStore` 只是兼容组合 trait；`LanceDbStore` 可以同时实现 chunk 和 label atom 能力，但上层服务应按实际能力依赖更窄的 trait。

边界要求：

- chunk context/rebuild 路径只依赖 `ChunkVectorStore`。
- label suggestion/proposal/atom-index 路径只依赖 `LabelAtomVectorStore`，不依赖 chunk store 语义。
- CLI/server no-heavy 路径通过 subprocess helper adapter 连接 graph/vector 派生层；
  context chunk 查询走 chunk commands，label suggestion/proposal、bootstrap staged verification
  和 label atom status/rebuild/query 走 label atom 专用 helper commands。label atom helper 在
  helper 进程内使用真实 `LanceDbStore` 写 `lancedb_label_atoms`，并通过 `kanban-derived-io` 的窄
  SQLite IO 更新 `LANCEDB_LABEL_ATOMS_STORE` / `label_atom_index_boards` 状态；server/CLI 不把
  chunk store `status` 当作 label atom 状态。
- label atom 场景获取 model 名称时使用通用 `VectorStoreBackend::embedding_model()`；`chunk_embedding_model()` 仅作为 chunk 路径的兼容入口。
- LanceDB 表仍按 derived store 隔离：task chunks 写入 `kb_chunks`，label atoms 写入 `kb_label_atoms`。

### 2.7 Label proposal provider boundary

Semantic label proposals 分成两层：

```text
upper provider layer
  - manual/offline candidate input
  - future local LLM / AI runtime integration
  - credentials, model config, HTTP/client concerns
        ↓ LabelProposalProvider
kanban-sqlite
  - task/suggestion context lookup
  - deterministic validation
  - residual top1+margin gate
  - proposal persistence and accept/reject lifecycle
```

`kanban-sqlite` 只接受 `LabelProposalProvider` trait object，不拥有真实 LLM provider。
默认 `DisabledLabelProposalProvider` 只产生 degraded attempt；`ManualLabelProposalProvider`
用于 CLI/API 显式传入的本地/offline candidate。未来真实 provider 的候选位置是
`kanban-server`、本地 runtime、或独立 `kanban-ai` / `kanban-llm` crate，并且必须保持
SQLite service 不知道 credentials、HTTP transport、prompt 模板或外部 SDK。

### 2.8 Label ontology roles

Label 系统有六个角色，但不是六个严格独立的存储层：

1. `labels` / `task_labels`：canonical label identity 与 task 当前绑定事实；base identity
   CRUD 是 vocabulary registry，不写 ontology ledger。
2. `label_semantics`：canonical ontology semantics；`label_atoms` 是从 semantics 与 label
   name 展开的 SQLite materialized projection。
3. `kb_label_atoms` / `label_atom_index_boards`：可重建 label atom derived retrieval。
4. `label suggest`：基于当前 task、atoms 和 vector evidence 的计算/诊断，不是持久 truth。
5. `label_semantic_proposals`：候选新 label 的 lifecycle 记录，accept 前不改变当前 task-label truth。
6. `label_ontology_*` ledger：observation、signal、action、validation provenance。

Proposal 与 ledger 是 SQLite canonical records，因为它们需要审计和可查询历史；但它们不替代
`task_labels` 的当前绑定事实，也不替代 `label_semantics` 的 ontology semantics。
Ledger 覆盖 semantics/atom mutation provenance；`labels` identity create/delete 位于
ledger 之外。
正式文档使用 `canonical truth`、`derived retrieval`、`proposal workflow` 和
`ontology provenance` 这些边界词；不要把未定义的内部简称写成架构术语。

### 2.9 Label ontology graph boundary

当前没有 label-ontology 专属 graph projection。`kanban graph` / Oxigraph 只镜像
`entity_relations` 中已有的 Knowledge Substrate 关系，例如 task-board 与 task dependency；
label ontology 的 query surface 仍是 SQLite ledger、proposal、semantics、`label ontology
review`、`label atom explain` 和 validation history。

在 rename/split/merge provenance 查询或跨 action 关系查询出现明确需求前，不新增
ontology graph store、ontology RDF schema 或后台 projection。若后续确实需要，它必须复用
Knowledge Substrate 的派生层边界：

- SQLite `labels` / `label_semantics` / `label_atoms` / `label_ontology_*` 仍是事实来源；
  其中 `label_atoms` 是 materialized projection，不是独立 semantic truth。
- Graph projection 只能从 SQLite 快照和 outbox 重建，可删除重建。
- Graph API 只能查询 relation/provenance，不提供 canonical ontology mutation path。
- Graph 故障、dirty 或删除不会改变 task labels、semantics、atoms、signals 或 actions。

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

### 3.4 Reopen task

```text
CLI/Web
  -> ReopenTask command
  -> BEGIN IMMEDIATE
  -> verify task.status == done
  -> verify reason is non-empty
  -> recompute target from spec, schedule, dependencies, and execution plan
  -> clear completed_at while preserving result_summary/result_json
  -> insert task_events(kind='task.reopened')
  -> recompute direct active children; leave running/blocked/review/done/archived children unchanged
  -> COMMIT
```

### 3.5 Web live update

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
~/.config/kanban/config.toml
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
- `kanban doctor` 检查 DB、WAL、schema、integrity、orphan run、基础关系表
  board consistency、label ontology ledger consistency，并报告 Knowledge Substrate 的
  `index_outbox` backlog、derived store dirty/error 状态和 per-store last_error。派生层
  异常不改变 SQLite task truth；operator 通过 sync/rebuild 恢复 Tantivy/Oxigraph/LanceDB。

### 8.1 Board scope 与 schema/service/doctor 分工

Board 是本地 project/board，不是 tenant。正常写路径的隔离边界在 service 层：
CLI、HTTP、desktop 和 dispatcher 通过 `kanban-sqlite::service` resolve board/task/label/run，
再在同一 transaction 中写 canonical SQLite truth。Derived stores 只消费 SQLite/outbox
投影，不拥有 canonical write 权限。

关键关系表已经使用包含 `board_id` 的 composite FK 或 trigger。`task_labels`、
`task_dependencies`、`task_runs`、`task_comments`、`task_attachments` 在 SQLite 层直接
保证 row board 与 referenced task/label/run board 一致；`task_events` 保留 nullable
task/run refs 与 `ON DELETE SET NULL` 历史语义，通过 INSERT/UPDATE triggers 校验非空
refs 的 board scope。Ontology action-signal 使用 board-scoped composite FK；nullable
ontology refs、parent/supersede links、proposal resolved label 等用 triggers 保护；historical
atom refs 保持 soft ref。

- service guard 是普通 CLI/API/Desktop/dispatcher 写入的主防线；
- `kanban doctor` 是现有 DB 的只读巡检层，发现 cross-board relationship rows 或
  `PRAGMA foreign_key_check` violation 时让 `ok=false`；
- JSONL import 在 replace transaction 提交前运行同类 consistency/FK gate，失败会回滚整个
  import。

---

## 9. Security Boundary

因为不做多用户/远程：

- 默认只绑定 `127.0.0.1`。
- 不提供登录系统。
- 不提供远程访问配置。
- 不在 API 中执行任意 shell，worker profile 只能来自本地 config。
- 附件路径必须限制在 data dir 内，防止 path traversal。

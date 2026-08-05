# Kanban Tool 规范合集

本文档由以下文件合并而成：

- README.md
- docs/SPEC.md
- docs/ARCHITECTURE.md
- docs/STATE_MACHINE.md
- docs/DATA_MODEL.md
- docs/CLI_SPEC.md
- docs/API_SPEC.md
- docs/SCHEMA_CONTRACTS.md
- docs/ADR.md
- crates/kanban-store-turso/src/schema.rs

`docs/SPEC.md`、`docs/CLI_SPEC.md`、`docs/API_SPEC.md`、`docs/STATE_MACHINE.md` 和 `docs/SCHEMA_CONTRACTS.md` 等分主题文档是当前行为的权威来源；本文件是这些源文档的同步快照，便于一次性阅读和离线传递。


---

# 文件：README.md

# Kanban Tool

Kanban Tool 是一个本地优先、单用户的看板与 durable work queue。任务、依赖、评论、执行记录和事件保存在本机的 canonical Turso 数据库中；CLI、MCP 和 Desktop 看到的是同一份事实。

当前产品刻意只有一条执行路径：

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP
    kanban serve
        ↓
ApplicationService + state machine
        ↓
  kanban-store-turso
        ↓
~/.local/share/kb/kanban.db
```

只有 `kanban serve` 可以打开、初始化和关闭数据库。其他入口永远通过本机 typed client 调用 host；server 不可用时返回 `server_unavailable`，不会退回直开数据库，也不会创建数据库文件。

## 快速开始

从源码安装 CLI：

```bash
cargo install --path crates/kanban-cli --bin kanban
```

先启动唯一的 application host：

```bash
kanban serve
```

默认数据库是 `~/.local/share/kb/kanban.db`，可以只在 host 上通过 `--db <path>` 或 `KANBAN_DB` 覆盖。默认 HTTP 地址为 `http://127.0.0.1:8721`；客户端命令可以用 `--server-url` 或 `KANBAN_SERVER_URL` 指定地址。

在另一个终端完成第一条 walking-skeleton 链路：

```bash
kanban board list
kanban task create "整理项目首页" --description "让第一次访问的人看懂项目"
kanban task list
kanban task step not-required default#1 --reason "单步任务"
kanban task promote default#1
kanban task show default#1
```

所有 mutation 都由 host 的 `ApplicationService` 校验并写入同一事务。任务状态由状态机决定，不能从 CLI、MCP 或 Desktop 直接修改数据库列。

## 三个入口

- **CLI**：通过 `kanban-client` 使用 localhost HTTP。`kanban init` 以及尚未迁移的旧命令会稳定返回 `feature_not_available`。
- **MCP**：`kanban-mcp` 是最小 stdio server，只提供 tools；它不会拉起 `kanban serve`，所有 `tools/call` 都调用同一个 typed client。
- **Desktop**：Tauri 应用是外部 host 的薄前端。`RuntimeConfig` 只包含 `apiBaseUrl`、`actor` 和 `board`，不包含 `dbPath`，也不内嵌 SQLite 或 Axum server。可通过 `KANBAN_SERVER_URL` 指向 host。

## 当前能力

能力按纵向 operation 切片推进；每条切片都闭合 store、application、HTTP、client 和入口适配器。

| 阶段 | operation |
| --- | --- |
| Walking skeleton | `board.list`、`task.create`、`task.list`、`task.show`、`task.plan.not_required`、`task.promote` |
| Durable queue | `task.claim`、`task.heartbeat`、`task.release`、`task.review`、`task.done`、`task.block`；可选单 worker dispatcher |
| 协作信息 | `comment.create/list`、`step.create/list/update`、`dependency.create/list/remove`、`run.list/show/log`、`event.list` |

健康检查、board columns、stats 和 task selector 是 Desktop 支持所需的只读 query，也通过同一个 application path 提供。Run 不是独立 mutation surface：claim 创建 run，heartbeat/release/review/done/block 在同一事务中更新 run 和事件；入口只读 run。

## 状态与一致性

任务的 canonical status 为：

```text
triage → todo / scheduled → ready → running → review → done
                         ↘ blocked ↗
```

`board_columns` 只是展示映射，不能形成第二套状态机。关键边界包括：

- `ready → running` 通过原子 claim 完成；同一任务最多一个 active run。
- claim、heartbeat、release、review、done、block 复用同一状态机和 application transaction。
- 外键、board isolation、唯一约束、依赖环检查和 `task_events` 由 canonical store 保证。
- `task.create`、`comment.create`、`step.create` 支持实体范围内的 `idempotency_key`；dependency create 由复合唯一约束自然幂等。
- server 重启后 task、plan、run 和 event 仍从同一数据库恢复。

## Dispatcher

`kanban serve` 默认不消费队列。只有显式传入 `--dispatcher-profile <path>` 才会在同一 host 进程中运行最小单 worker loop。profile 固定声明 board、worker command、poll interval、claim TTL、heartbeat interval、success/failure policy 和 log directory。

dispatcher 只扫描 `ready`，并通过 `ApplicationService` 完成 claim、heartbeat 和 finish；不会自动 claim `review`、`todo` 或 `scheduled`。没有 daemon 注册、named pipe、Job Object 或自动 server supervision。

## 数据与配置

canonical 数据库：

```text
~/.local/share/kb/kanban.db
```

Host 启动时幂等执行 embedded schema migration，并 seed `default` board 与固定 status columns。当前 baseline 包含 boards、board_columns、tasks、task_execution_plans、task_steps、task_dependencies、task_runs、task_comments 和 task_events。

actor 是审计字符串，不是用户或权限模型。HTTP 只绑定 loopback，不提供公网、多用户、RBAC 或远程 worker。

## 明确非目标

本轮不提供，也不会以 fallback 形式重新引入：

- SQLite/Turso 双 backend、CLI/MCP/Desktop 直开数据库；
- `kanban-runtime*`、framed IPC、named pipe、capability negotiation 或通用 mutation receipt；
- 自动 server supervision、跨进程/跨机器数据库写入和 `multiprocess_wal`；
- SQLite importer、旧 API 完整兼容和 v2/v3 runtime 恢复协议；
- labels、signals、semantic search、graph、vector、Tantivy/LanceDB/Oxigraph projection 以及 derived control plane；
- 这些能力未来可以单独设计，但不属于当前 canonical path。

## 文档与开发

- 产品范围和阶段能力：[`docs/SPEC.md`](docs/SPEC.md)
- 进程、crate 和数据流：[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- 状态机：[`docs/STATE_MACHINE.md`](docs/STATE_MACHINE.md)
- HTTP contract：[`docs/API_SPEC.md`](docs/API_SPEC.md)
- CLI contract：[`docs/CLI_SPEC.md`](docs/CLI_SPEC.md)
- 数据模型：[`docs/DATA_MODEL.md`](docs/DATA_MODEL.md)
- 架构决策：[`docs/ADR.md`](docs/ADR.md)

修改前请阅读 [`AGENTS.md`](AGENTS.md)，并按受影响的 package 运行最小 `just` 检查。项目只做本地 commit，不自动 push 或创建 PR。

## 许可证

Kanban Tool 使用 [Apache License 2.0](LICENSE) 开源。


---

# 文件：docs/SPEC.md

# Kanban Tool 产品规范

文档类型：当前实现规范

Kanban Tool 是本地优先、单用户的看板与 durable work queue。任务事实只保存在 canonical Turso 数据库中；CLI、MCP、Desktop 和可选 dispatcher 共享同一个 application service、状态机、事务和错误语义。

## 1. 固定执行路径

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP
      kanban serve
        ↓
ApplicationService + state machine
        ↓
   kanban-store-turso
        ↓
   canonical kanban.db
```

硬性规则：

1. 只有 `kanban serve` 可以打开、初始化和关闭 Turso 数据库。
2. CLI、MCP、Desktop 只能依赖 `kanban-client`（或 Desktop 的 TS HTTP client），不得依赖 `kanban-store-turso`、SQLite 或任何 DB-owning crate。
3. 所有 mutation 必须进入同一组 typed application command；adapter 只解析输入、调用 client、映射结果或渲染输出。
4. 不存在“server 运行时走 RPC、server 停止时直开数据库”的 fallback。
5. 本轮不引入自定义 IPC、runtime protocol、capability catalog、通用 receipt 或第二 backend。

Host 默认监听 `http://127.0.0.1:8721`，默认数据库为 `~/.local/share/kb/kanban.db`。`--db`/`KANBAN_DB` 只对 `kanban serve` 生效；其他命令只接受 `--server-url`/`KANBAN_SERVER_URL`。

## 2. 产品目标与非目标

### 2.1 目标

- 将 task、plan、依赖、评论、steps、runs 和 events 持久化为可重启恢复的 canonical 数据。
- 由同一状态机保护状态转换、原子 claim、lease/heartbeat 和 run/event 一致性。
- 让 CLI、MCP、Desktop 对同一 task 立即看到相同结果。
- 保持本地单用户语义；`actor` 只用于审计，不承担鉴权。

### 2.2 非目标

以下能力不属于当前 canonical path：

- 多用户、团队、邀请、RBAC、多租户、SaaS、云同步或远程 worker；
- SQLite/Turso 双 backend、旧 SQLite importer、旧 API 完整 parity；
- `kanban-runtime*`、framed IPC、named pipe、跨版本握手、capability negotiation、mutation receipt 和 crash matrix；
- 自动 server supervision、系统服务注册、Windows Job Object、`multiprocess_wal`；
- labels、signals、semantic search、graph、vector、Tantivy/LanceDB/Oxigraph projection、derived control plane；
- 为未来部署方式预先建设兼容层或通用 backend abstraction。

这些项目可以作为独立后续工作，但不阻塞当前三阶段链路。

## 3. 当前公开 operation

每个 operation 按纵向切片闭合 store → application → HTTP → typed client → adapter → test。

| 阶段 | operation |
| --- | --- |
| Walking skeleton | `board.list`、`board.columns`、`task.create`、`task.list`、`task.show`、`task.plan.not_required`、`task.promote` |
| Durable queue | `task.claim`、`task.heartbeat`、`task.release`、`task.review`、`task.done`、`task.block`；opt-in dispatcher |
| 协作信息 | `comment.create/list`、`step.create/list/update`、`dependency.create/list/remove`、`run.list/show/log`、`event.list` |

`health`、`board.columns`、`stats` 和 task selector query 是 Desktop 支撑所需的只读 query，同样通过 `ApplicationService` 提供。run 不提供独立 create/update mutation；claim 同事务创建 run，后续 lifecycle command 同事务更新 run 和 event。

MCP 使用 `board_list`、`task_*`、Stage 2 lifecycle tools 和 Stage 3 collaboration tools。所有 MCP `tools/call` 只调用 typed localhost client；MCP 不启动 host。

## 4. 状态模型

权威 `tasks.status` 为：

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

`board_columns` 只是展示映射。应用层不提供任意 `transition(target_status)`；每个公开 mutation 都是有明确前置条件和输入的 typed command。

| 状态 | 语义 |
| --- | --- |
| `triage` | 规格尚不完整，不能执行。 |
| `todo` | 已定义，但计划、依赖或其他条件尚未满足。 |
| `scheduled` | 等待未来排期。 |
| `ready` | 满足条件，可由人工或显式 dispatcher claim。 |
| `running` | 已 claim，拥有 active run 和 lease。 |
| `blocked` | 因外部依赖、失败或人工输入暂停。 |
| `review` | 执行结果待人工确认。 |
| `done` | 已完成。 |
| `archived` | 隐藏的历史记录，不进入默认调度。 |

`task.plan.not_required` 和 `task.promote` 共同完成 walking-skeleton 的 `todo → ready` 检查；plan、依赖、排期和 archived 保护由 service 重新计算，adapter 不自行判断。

Durable queue 的不变量：

- `ready → running` 只能通过原子 `task.claim`；两个调用方并发 claim 同一 task 时恰好一个成功。
- `running` 必须有 `claim_token`、`claim_owner`、`claim_expires_at` 和一个 active run。
- `heartbeat`、`release`、`review`、`done`、`block` 先校验 task、owner、token 和 lease，再在同一事务更新 task、run、event。
- `release` 只允许 matching claim owner/token 主动释放；成功后 task 回到 `ready`、active run 变为 `canceled`，并写 `task.released`。
- dispatcher 只扫描 `ready`，绝不自动 claim `review`、`todo` 或 `scheduled`。

完整转换表与 guard 见 [`STATE_MACHINE.md`](docs/STATE_MACHINE.md)。

## 5. Canonical 数据与 schema

`kanban-store-turso` 使用 `turso = 0.7.2`、`default-features = false`。schema 是 embedded SQL，使用简单的 `schema_migrations` 表和事务性、可重复执行的初始化。首次 host 启动幂等 seed `default` board 与固定 status columns。

canonical baseline 包含：

- `boards`、`board_columns`；
- `tasks`、`task_execution_plans`、`task_steps`；
- `task_dependencies`；
- `task_runs`；
- `task_comments`；
- `task_events`。

数据库必须保证：

- status、run status、step status、priority 等 CHECK；
- board 与 task/comment/step/dependency/run/event 的复合外键和 board isolation；
- `(board_id, seq)`、实体 id、active run 和实体本地 `idempotency_key` 唯一约束；
- dependency 自身约束、禁止 self-dependency；
- task snapshot 与对应 event 在同一事务提交。

canonical 数据是业务事实。搜索、图、向量、缓存和 projection 如果未来恢复，只能从 canonical 数据重建，不能成为 mutation path。

## 6. Adapter 规则

### 6.1 CLI

`kanban serve [--db <path>] [--dispatcher-profile <path>]` 是唯一 DB owner。其他命令通过 `kanban-client` 访问默认 `http://127.0.0.1:8721`，支持 `--json`、`--board`、`--actor` 和 board-local/global task selector。`kanban init` 与未迁移命令在触库前返回 `feature_not_available`；host 不可用返回 `server_unavailable`。

### 6.2 MCP

MCP 是最小 Rust stdio server，使用官方 `rmcp` tools/stdio transport；不提供 resources/prompts，不拉起 host，不解释状态转换。工具名与 operation 一一对应，参数和响应复用 `kanban-contract` DTO。

### 6.3 Desktop

Desktop 保留已有页面结构和 TS `KanbanApi`，只通过 external host HTTP 工作。`RuntimeConfig` 只有 `apiBaseUrl`、`actor`、`board`；claim token 只在当前会话内保存，不写入磁盘。labels/signals/search/neighborhood 等未迁移视图必须隐藏或禁用，不发送失败请求。

## 7. Dispatcher

dispatcher 是 `kanban serve` 内的 opt-in、单 worker loop，不是独立 daemon。`--dispatcher-profile <path>` profile 固定包含：

```text
board
worker command
poll interval
claim TTL
heartbeat interval
success/failure policy
log directory
```

loop 停止新 polling 后等待当前 worker 正常结束；第二次中断才强制退出。worker 的 claim、heartbeat、finish 必须调用同一 application command，stdout/stderr 仅写 profile 指定日志路径，数据库保存摘要和路径。

## 8. 配置、错误与重启

- server 只监听 loopback；不提供远程访问和登录。
- `KANBAN_DB`、`--db` 只配置 host 的数据库；`KANBAN_SERVER_URL`、`--server-url` 只配置 client。
- host 每个 operation 在同一进程中按需获取 Turso connection；不启用 `multiprocess_wal`。
- `error.code`、DTO 和 HTTP status 映射由 `kanban-contract`/server/client 共同维护；adapter 不重新解释 domain error。
- 关闭并重启 host 后，boards、tasks、plans、comments、steps、dependencies、runs 和 events 从 canonical DB 继续可读；不会由 adapter 创建第二个数据库。

## 9. 验收基线

Stage 1 的最小验收：CLI 创建 task，Desktop 从同一 host 读到，MCP 调用 `task_plan_not_required` 和 `task_promote`，CLI show 得到 `ready`；停止并重启 host 后 task、plan、event 仍存在。

Stage 2 的最小验收：并发 claim 恰好一次成功；正确/错误 token 的 heartbeat、release、review、done、block 保持 task/run/event 原子一致；release 后可以再次 claim，dispatcher 不 claim `review`。

Stage 3 的最小验收：comment、step、dependency、run 和 event 在 CLI、MCP、Desktop 之间一致；cross-board、FK、唯一约束和 dependency cycle 被拒绝；重启后历史仍可读。

详细 HTTP/CLI wire contract 分别见 [`API_SPEC.md`](docs/API_SPEC.md) 和 [`CLI_SPEC.md`](docs/CLI_SPEC.md)。


---

# 文件：docs/ARCHITECTURE.md

# 架构

kanban-tool 的稳定运行边界是本地优先、单机、单用户和单一 canonical Turso owner。
本次重构收敛 crate、存储和维护方式，但不收缩产品能力：queue/history、labels、ontology、
signals、entities、relations、search、vector、graph、context、maintenance、migration 以及
Desktop 历史视图都必须在同一条 service path 上恢复。当前代码仍是过渡结构，文档分别标出
“当前已 wiring”“schema 已就绪”和“目标 owner”，不把 roadmap 当成已完成实现。

```text
CLI / MCP / Desktop
        │
        ▼
typed localhost HTTP / SSE client
        │
        ▼
kanban serve（唯一 host）
  Axum routes + ApplicationService
  state machine + transaction boundary
  dispatcher + projection worker（目标）
        │
        ▼
Turso canonical database
  facts / events / ontology / entities
  FTS / vector / graph projection（可重建）
```

## 1. 进程与 ownership

只有 `kanban serve`（`kanban-server` host）可以打开、初始化、迁移、备份和关闭 Turso
数据库。默认路径为 `~/.local/share/kb/kanban.db`，默认绑定 `127.0.0.1:8721`。CLI 的
`serve` 子命令负责装配 host；普通 CLI、MCP 和 Desktop 只构造 typed localhost client，
不会直接打开数据库。server 不可用时返回 `server_unavailable`，没有 embedded DB fallback。

同一 host 进程可以按 operation 获取独立 connection，但跨进程直开 canonical 文件不属于
产品路径；不启用 `multiprocess_wal`。数据库连接必须开启 `PRAGMA foreign_keys = ON`，
board isolation、外键、唯一约束、CAS 和事务边界由共享 service/store path 保护。

目标 host 内包含两个共享 worker：

- dispatcher 只通过 service 原子 claim `ready`，执行 worker command，并复用 heartbeat、
  finish、run 和 event 语义；不得自动 claim `review`。
- projection worker 从 canonical transaction 接收 `projection_jobs`，在 host 内处理 Turso
  FTS、Turso vector32、Ollama embedding、entities/relations 派生和可重建 context。它不
  通过 helper subprocess、framed protocol 或第二个数据库写事实。

3b61aa5 已建立 migration、projection job/state、retrieval、ontology/signal、import journal
和 attachment staging 的 schema readiness，并探测 `fts`/`vector32` capabilities；service
use case、worker loop、完整管理 API 和 SQLite importer 仍需按 parity ledger 接通。

## 2. Workspace crate 边界

### 2.1 当前过渡结构

当前 workspace 仍包含：

```text
kanban-core              领域类型、状态机和纯校验
kanban-application       typed use case、ApplicationService 和 store port
kanban-store-turso      Turso schema、migration 和 persistence
kanban-contract          HTTP/CLI/MCP DTO、错误 envelope、schema 描述
kanban-client            typed localhost HTTP client
kanban-server            Axum host、routes、dispatcher 装配
kanban-cli               参数解析、输出和 serve wrapper
kanban-mcp               stdio adapter
apps/desktop             Tauri shell、React UI、TS client
xtask                    publish = false 的 schema/dependency/AGENTS 工具
```

`kanban-store-turso` 是当前唯一直接依赖 `turso = 0.7.2` 的 canonical persistence owner；
被 workspace exclude 的旧 `search`、`vector`、`vector-lancedb`、`graph`、`graph-oxigraph`、
`helper-protocol` 等不能再成为 active mutation path。它们的能力必须先由完整 parity ledger
映射到目标 owner，才允许删除目录和依赖。

### 2.2 目标结构

最终产品单元是 7 个 Rust crate、Desktop 和 `xtask`：

| 单元 | 目标职责 |
|---|---|
| `kanban-core` | 纯领域类型、不变量、状态机、claim/lease、labels/ontology/signals、entity URI 和 board isolation；不依赖 Turso、HTTP 或异步 runtime |
| `kanban-service` | 合并 `kanban-application` 与 `kanban-store-turso`；use case、schema/migration、repository、事务、projection、Ollama provider 和 SQLite 只读 importer |
| `kanban-protocol` | 取代 `kanban-contract`；HTTP/SSE DTO、统一 error envelope、operation catalog 和 machine-readable schema |
| `kanban-client` | typed localhost HTTP/SSE client；不持有领域规则和数据库依赖 |
| `kanban-server` | Axum routes、唯一 host 生命周期、dispatcher、projection worker 和管理操作 |
| `kanban-cli` | 参数解析、人类/JSON 输出；普通命令调用 client，`serve` 负责装配 server |
| `kanban-mcp` | 由 operation catalog 绑定完整领域工具；只通过 client，不暴露 host-admin migration/backup/compaction |
| Desktop | Tauri/Web UI 通过 localhost API 恢复 board/list/map/events/runs/signals/ontology/maintenance/health/settings |
| `xtask` | 私有离线工具；schema、dependency、AGENTS 和 witness，不进入 runtime |

依赖方向为：

```text
kanban-core ◄── kanban-service ◄── kanban-server ◄── kanban-cli::serve
      ▲              ▲                  │
      │              └── kanban-protocol ◄── kanban-client ◄── CLI / MCP / Desktop
      │                                      ▲
      └──────────────────────────────────────┘

xtask ──► kanban-protocol[schema] + cargo metadata
```

迁移期间保留 `kanban-application`/`kanban-store-turso` 和 `kanban-contract` 的现有路径，
但新能力不应再制造另一套 port、DTO 或 database adapter。合并完成前，不能把目标 crate
名称写成已经存在的包，也不能以兼容 shim 掩盖未闭合的 service wiring。

## 3. 共享 service path 与 mutation boundary

所有 adapter 都使用同一条路径：

```text
adapter input
  → kanban-client request / Axum extractor
  → ApplicationService（目标为 kanban-service）
  → kanban-core 状态机与 board resolution
  → Turso transaction
  → canonical snapshot + event + projection job
  → shared protocol DTO / error envelope
```

`tasks.status` 是事实；列只做展示映射。`ready → running` 必须在同一事务内原子 claim，
并同时创建 run 与对应 event；heartbeat、release、review、complete、block、reopen、reclaim、
archive 复用 owner/token、lease 和 lock version。依赖环、ontology CAS、signal review、
attachment checksum 和 import journal phase 都不能由 adapter 自己实现。

目标 service 还负责 entities/relations 的 board-scoped BFS、检索 context pack、label atom
相似度、projection generation/fingerprint、Ollama outage 降级和 rebuild。派生结果写回
`projection_jobs`/`projection_state` 或 retrieval 表，不反向改变 canonical facts。

## 4. Turso schema、search、vector 和 graph

`kanban-store-turso` 当前实现 `kanban.turso` family 的 v1/v2 lineage：v2 包含 queue/history、
labels/ontology/signals、entities/relations、projection、retrieval、import journal 和
attachment staging。启动时比较精确 table/column/index/trigger/constraint witness，并将
v2 SQL SHA-256 写入 migration ledger；v1 原地升级先通过 sibling `VACUUM INTO` 备份验证，
再开始事务 migration。完整字段和指纹见 [`DATA_MODEL.md`](docs/DATA_MODEL.md)。

派生能力统一由 Turso/host 提供：

- **FTS**：`retrieval_documents` 上的 Turso `USING fts` index；service 查询使用
  `fts_match`、`fts_score`、`fts_highlight`，移除外部 Tantivy 作为事实/检索 owner。
- **Vector**：Ollama 只作为 host 内 provider；embedding 的批处理、重试、model/dimension/
  fingerprint、缓存和降级语义由 service/worker 管理；向量和 cosine 查询在 Turso
  `vector32` 中完成，移除 LanceDB。
- **Graph**：`entities`、`relation_predicates`、`entity_relations` 是 canonical；Turso
  0.7.2 不依赖 recursive CTE，service 执行有深度上限、环检测和 board isolation 的批量
  BFS，移除 Oxigraph。
- **Projection**：canonical mutation 在事务中写事实、event 和 projection job；FTS、vector、
  graph/context 都可删除后重建，不接受旧 helper subprocess/protocol 或独立 control plane。

当前 schema 能力已经有 capability probe 和 fail-closed shape validation，但完整 service
查询、worker、HTTP/CLI/MCP/停机恢复和 Desktop 视图仍按纵向 slice 实现；文档不把 schema
存在等同于用户入口可用。

## 5. 迁移与主机管理

目标 `kanban-service` 内置两条数据路径：

1. **Turso v1 原地升级**：host 独占数据库；先生成并校验 sibling backup，再执行事务化
   migration。失败保留旧 schema/data，可再次启动；重复启动幂等。
2. **SQLite v30 逻辑导入**：CLI 通过 localhost 管理 API 请求运行中的 host；service 只读
   打开源 SQLite，目标 Turso 必须为空。先做 schema、计数、引用、board、attachment 和
   checksum preflight，再按 `import_journal`/`attachment_staging` 做 staging、commit 后
   原子发布和崩溃 resume。派生 FTS/vector 不迁移，导入后重建；源文件不修改，重复
   fingerprint 返回已完成结果。

备份、portable export/import、maintenance、rebuild、cleanup、native compaction 或“导出
到新 Turso 后校验并原子替换”都属于 host 管理面。它们不能成为 MCP tool；CLI、HTTP 和
Desktop 管理入口必须复用 host，不能打开第二个数据库。

## 6. HTTP、CLI、MCP 与 Desktop adapter

### 6.1 HTTP/SSE 与 client

`kanban-server` handler 只解析输入、调用 service、映射 protocol DTO/error，并提供完整
领域及 host 管理 catalog；SSE 事件从 append-only `task_events` 游标读取。`kanban-client`
只负责 typed localhost HTTP/SSE，不复制状态机、SQL 或 fallback。

当前 active 路由仍以 `kanban-contract` 和现有 API 文档为准；完整约 84 个 operation 的
surface 必须逐项进入 parity ledger，不能用旧路径兼容或“暂不支持”关闭迁移。新 operation
先在 protocol catalog 定义，再由 server/client/CLI/MCP/Desktop 逐面绑定。

### 6.2 CLI

普通命令只调用 client，输出人类文本或稳定 JSON；`serve` 才接受 `--db` 并装配 host。
`init`、迁移、backup、export/import、maintenance 和 dispatcher 等命令必须明确区分普通
领域操作与 host 管理操作。server 不可用时返回可执行的 `server_unavailable` 提示，不
尝试直接打开数据库。

### 6.3 MCP

`kanban-mcp` 通过 stdio 绑定 operation catalog 的完整领域工具和只读 resource，覆盖 queue、
labels/ontology/signals、entities/relations/search/vector/context、runs/events/stats 等
能力。迁移、备份、vacuum/compaction、数据库替换等 host-admin 操作只保留在 HTTP/CLI/
Desktop 管理面。MCP 不拥有 store/server 依赖，也不在 host 不可用时 fallback。

### 6.4 Desktop

Desktop 只保留 `apiBaseUrl`、actor、board 等运行时配置，通过 localhost API 恢复 board/list/
map/events/runs/signals/ontology/maintenance/health/settings 视图和操作。Tauri command 不
拥有数据库、嵌入 Axum 或 DB lookup；claim token 仅存当前会话。现有页面与新 API 的 wiring
必须按 parity ledger 恢复，不能把隐藏/禁用未迁移视图当成能力完成。

## 7. Dispatcher 与 projection worker

dispatcher 是 host 内 opt-in 单 worker loop：

```text
kanban serve --dispatcher-profile profile.toml
```

它只扫描 `ready`，用共享 service claim/heartbeat/finish，写 run 摘要与可信 log path，
关闭时等待当前 worker，第二次中断才 force-stop。projection worker 使用同一个 host 生命周期
和维护 lease，按 job 的 generation/fingerprint 处理 FTS、vector、relations、context，
支持重试、失败降级、rebuild 和恢复；Ollama provider 不可用时保留 canonical mutation，
仅标记 projection degraded。

这两类 worker 都不能自行打开第二个数据库、维护第二个状态机或直接更新 adapter DTO。

## 8. 收敛和验收边界

旧 crate 删除前必须完成：

1. parity ledger 为所有旧表、operation、CLI/MCP/Desktop surface 和维护操作指定目标 owner、
   入口、迁移规则和验收测试；
2. 每个纵向 slice 闭合 `core/service → protocol/server → client → CLI/MCP/Desktop → tests/docs`；
3. 证明 FTS/vector/graph/context 都能从 canonical 数据重建，且无直接 DB adapter、helper
   protocol、LanceDB/Oxigraph/Tantivy active dependency；
4. 通过迁移、失败回滚、崩溃恢复、重复执行、board isolation、claim/event 原子性和完整
   surface acceptance。

当前 `kanban-application`、`kanban-store-turso` 和 `kanban-contract` 仍是实现过渡态；最终
分别合并为 `kanban-service` 和 `kanban-protocol`。在合并和删除发生前，所有缺口都必须在
任务 ledger 中保持可审计，不能以“功能收缩”或新的兼容路径结束迁移。


---

# 文件：docs/STATE_MACHINE.md

# 任务状态机

状态机由 `kanban-core` 的 readiness/claim/finish 规则和
`kanban-application::ApplicationService` 统一执行。HTTP、CLI、MCP、Desktop 与
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


---

# 文件：docs/DATA_MODEL.md

# Canonical 数据模型

本文件描述 `kanban-store-turso` 当前提交所建立的 Turso schema，以及完整功能
迁移时必须保留的 canonical/derived 边界。权威实现是
`crates/kanban-store-turso/src/schema.rs` 与 `migration.rs`；应用服务负责领域规则，
数据库负责外键、唯一性、`CHECK`、board isolation 和事务约束。

这里的“schema 已就绪”不等于所有 service、HTTP、CLI、MCP、Desktop 或 SQLite
importer 已经完成。公开 surface 仍以对应 contract 和 parity ledger 为准；本文件不把
尚未接通的入口写成已实现能力。

## 1. Schema family、lineage 和精确指纹

数据库必须属于 `kanban.turso` family。数字 migration version 与 lineage 分开记录，
因此一个恰好写着 `version = 1` 的其他数据库不会被自动采用。

| lineage | migration | 作用 | 精确指纹 |
|---|---|---|---|
| `v1` | `001_canonical_baseline` | 旧的窄 Turso baseline，也是原地升级的唯一 v1 输入 | 10 张表、114 个列名；`columns-sha256:c235e96f250e780f62241b55a9721b14b5ebe9244172e01a5655e16af6d18d00` |
| `v2` | `002_turso_full_feature_baseline` | 完整功能 schema；fresh install 和 v1 upgrade 都落到这里 | 38 张表、443 个列名、45 个普通 index、22 个 trigger；SQL `sql-sha256:6367687a26d9658f1f3e5454f45f784a2e9806c818eb8c2fa9c7506c2f620bfb` |

`FULL_EXACT_COLUMNS` 是 v2 的精确 table/column manifest。启动时会比较完整的表集合
和每张表的列集合，缺列、多列、未知表都会失败关闭。普通 index 的 required manifest
由 `FULL_REQUIRED_INDEXES` 固定；另外包含一个 Turso FTS index
`idx_retrieval_documents_fts`，因此启用 FTS 后实际 index 对象为 46 个。trigger
由 `FULL_REQUIRED_TRIGGERS` 固定为 22 个。

约束指纹由 `FULL_REQUIRED_SQL_FRAGMENTS` 的 20 个 SQL 片段、`PRAGMA foreign_key_check`、
board-isolation preflight 和上述 trigger manifest 共同组成。它覆盖关键 `CHECK`、复合
外键、lease/CAS 形状、自依赖拒绝、JSON 合法性和附件路径边界；不接受只看表名的“近似
schema”。

`schema_migrations` 保存每个版本的 `name`、`checksum`、`applied_at` 和
`schema_family`；`schema_identity` 保存当前 family、lineage、version、schema
fingerprint 与 migration checksum；`schema_capabilities` 保存运行时探测到的 `fts` 和
`vector32` 能力。v2 的 `schema_identity.migration_checksum` 与 version 2 ledger 的
checksum 都是上述 SQL SHA-256，而不是可变的运行时状态。

## 2. Canonical 与 derived 分类

canonical 表保存业务事实或不可由索引重建的迁移事实；derived 表、索引和 worker 状态
都必须可以删除后从 canonical 事实重建。`projection_jobs` 虽然是 durable work queue，
也不是业务事实来源。

| 分类 | canonical 表 | derived/运行时表或索引 |
|---|---|---|
| 看板、任务和历史 | `boards`、`board_columns`、`tasks`、`task_execution_plans`、`task_steps`、`task_dependencies`、`task_runs`、`task_comments`、`task_events`、`task_attachments`、`app_settings` | 无；事件是追加审计事实，不是第二套状态机 |
| labels、ontology、signals | `labels`、`task_labels`、`label_semantics`、`label_atoms`、`label_semantic_proposals`、`label_ontology_observations`、`label_ontology_signals`、`label_ontology_actions`、`label_ontology_action_signals`、`label_ontology_action_atom_effects`、`signal_observations`、`signals` | `label_atom_index_boards` 是 label atom 相似度索引的可重建状态 |
| entities、relations | `entities`、`relation_predicates`、`entity_relations` | 图邻居、neighborhood、task map 和 BFS 结果均由这些事实重新计算 |
| projection | 无业务事实表 | `projection_jobs`、`projection_state`、`projection_maintenance_owner` 是 host worker 的 job、generation、fingerprint、lease 和维护状态 |
| 检索 | 无业务事实表 | `retrieval_documents`、`retrieval_vectors` 以及 Turso FTS index；内容、embedding、model/dimension/fingerprint 都可重建 |
| 导入耐久性 | `import_journal` 记录 source fingerprint、阶段和 resume 所需的事务事实 | `attachment_staging` 记录 staging 文件的 checksum/size/发布阶段；staging 文件本身不是 canonical attachment |
| schema 元数据 | `schema_migrations`、`schema_identity`、`schema_capabilities` | capability probe 结果可刷新，不改变业务事实 |

旧 SQLite v30 的导入目标必须是空的 canonical Turso 数据库。v2 已提供 journal 和
attachment staging 的持久化结构，但 importer、管理 API 和文件发布流程仍需在 service
纵向切片中接通；不能因为表已经存在就声称导入已完成。

## 3. ID、时间和 JSON

实体 ID 使用带固定前缀的 ULID 字符串：

| 实体 | 前缀 |
|---|---|
| board | `b_` |
| task | `t_` |
| step | `step_` |
| run | `r_` |
| comment | `c_` |
| event | `e_` |
| column | `col_` |
| attachment | `a_` |
| label | `l_` |
| label atom | `la_` |
| semantic proposal | `lp_` |
| ontology observation/signal/action | `lor_` / `los_` / `loa_` |
| generic observation/signal | `obs_` / `sig_` |
| entity/document/vector | `kb://` / `doc_` / `vec_` |
| import journal/staging row | `ij_` / `as_` |

`task_events.id` 是 `INTEGER PRIMARY KEY AUTOINCREMENT` 游标，`event_id` 是公开的
`e_...` 身份。时间列是 UTC Unix epoch milliseconds，Rust 边界使用 `i64`。JSON 存为
`TEXT`，由 `CHECK(json_valid(...))` 保护；对象和数组字段还会检查 `json_type`。未知
事件载荷必须保留合法原始 JSON。

## 4. Schema migration、备份和回滚

### 4.1 fresh install 与 v1 upgrade

`kanban serve` 的数据库 owner 在 immediate transaction 中执行 embedded schema，写入
ledger、identity、projection seed 和默认 board/columns。重复启动必须幂等：不重建业务
表、不改变已有事实、不生成第二份升级备份。

识别为 `kanban.turso`/`v1` 后，升级顺序是：

1. 比较 v1 的精确表集合、列集合、ledger name/checksum/family。
2. 在任何 schema 写入前，通过 Turso `VACUUM INTO` 在同一文件系统的数据库旁生成
   `<database>.pre-v2-<timestamp>[-n].turso-backup`。
3. 重新打开备份，检查 lineage、`PRAGMA integrity_check`，并逐表比较 v1 表行数。
4. 通过可选 host backup hook 后，才开始 v2 migration transaction。
5. migration 失败时由事务回滚；旧 v1 schema/data 和已验证 sibling backup 保持可再次
   启动的状态。

未知 family、未知 table、列 drift、constraint/trigger drift、foreign key 或 board
   isolation preflight 失败时，启动 fail closed。导入源文件不会由 migration 修改。

### 4.2 capabilities

初始化完成后 host 探测：

- `vector32` 与 `vector_distance_cos`：向量保存为 Turso vector32 BLOB，service 另行校验
  model、dimension、content fingerprint；维度不匹配必须失败。
- `fts`：在 `retrieval_documents(content)` 上建立 Turso-native FTS index，并使用
  `fts_match`、`fts_score`、`fts_highlight`。FTS 只服务检索，不能成为事实来源。

能力不可用会写入 `schema_capabilities.available = 0` 和可诊断的 detail；不能静默回退到
外部 Tantivy、LanceDB 或第二个数据库。

## 5. 看板、任务和历史

### `boards` 与 `board_columns`

`boards` 保存 `id`、`slug`、`name`、`description`、创建/更新时间和归档时间；`id`
匹配 `b_%`，slug/name 非空且 slug 唯一。`board_columns` 保存 status 展示映射、排序、
hidden 和 WIP 限制；status 只能是
`triage|todo|scheduled|ready|running|blocked|review|done|archived`。默认 seed 为九列，
`archived` 默认隐藏。列不是第二套状态机。

### `tasks`

`tasks.status` 是唯一 canonical 状态真相。字段分为身份（`id`、`board_id`、`seq`、
`idempotency_key`）、内容（title/description/result/metadata）、排序（priority、
position、scheduled/due）、操作者（created_by、assignee）、claim/lease、生命周期、
重试和并发版本。`running` 必须同时有 claim token、owner 和 expiry；`(board_id,seq)`、
实体身份和局部 idempotency key 提供唯一性。

### plans、steps、dependencies、runs

`task_execution_plans` 记录 `unplanned|planned|not_required`；`task_steps` 保存父任务内
的位置、required、resolution 和可选同板 linked task；`task_dependencies` 以复合外键
连接同板任务，应用 service 负责可达路径检查和依赖环拒绝。`task_runs` 保存 claim、worker、
heartbeat、完成状态、摘要、错误和可信相对 log path；每个任务最多一个 active running
run。claim、run、event 必须同事务提交。

### comments、attachments、events

`task_comments` 支持 `note|decision|signal`，并保留 author/agent、metadata 与本地
idempotency key；signal comment 是 generic signal 的可追溯回链。`task_attachments` 只
保存 metadata、相对路径、size 和可选 SHA-256；路径 trigger 拒绝绝对路径和 `..` 穿越。
文件复制和发布由导入/附件 service 负责，数据库字段不能成为任意文件读取入口。

`task_events` 是 append-only 审计和 SSE 游标事实，task/run 引用以 board-scoped 复合外键
保护；事件与对应 snapshot mutation 同事务提交。事件不是另一套 event-sourcing 状态来源。

## 6. Labels、ontology 和 signals

`labels`/`task_labels` 保存 board 范围的标签绑定；`label_semantics` 保存 description、
applies/excludes 条件和正负例；`label_atoms` 保存带 polarity/kind/ordinal/content hash
的原子语义。`label_semantic_proposals` 保留 coverage、residual、top-1 label、diagnostics、
decision reason 和更新时间，支持提议、采纳或拒绝。

ontology ledger 由 observation、signal、action 及其 signal/atom effect 关系组成。表中
保留 task snapshot、agent candidates、suggestion/final decision、CAS hash、review/close
时间、status reason、validation JSON、actor type 和 atom 内容快照，以支持
review/confirm/reject/resolve/supersede、apply/adopt、validate/revert/undo 的完整审计。
generic `signal_observations`/`signals` 记录非 label 产品信号，signal 生命周期和
comment backlink 不依赖 ontology。

所有 ontology/signal 外键、supersede 引用和 target label 都必须保持同 board；应用层
负责 CAS/hash guard 和状态转换，数据库 trigger 负责拒绝跨 board 引用。

## 7. Entities、relations、graph 和 task map

`entities` 以 `kb://...` URI 保存实体 kind、source table/id、board/task 归属、标题摘要和
content hash；`relation_predicates` 定义谓词域、范围、cardinality 和说明；`entity_relations`
保存 subject/predicate/object、graph URI、source event、metadata 和更新时间。subject/object
使用复合 board 外键，不能跨 board。

图数据库不是事实来源。service 从 canonical relations 执行带深度上限、环检测和 board
isolation 的批量 BFS，生成 neighborhood/task map/context graph；任何 graph projection
删除后都能重建。

## 8. Projection、FTS 和 vector retrieval

`projection_jobs` 是 host 内部 worker 的 durable queue：`target` 可为 `fts`、`vector_tasks`、
`vector_label_atoms`、`relations` 或 `all`；job 保存 operation、dedupe key、attempt、lease、
fence epoch、generation 和错误。`projection_state` 保存每个 projection 的 lifecycle、active/
building generation、fingerprint、provider/model/dimension、corpus fingerprint、last event、
lease 和失败状态。`projection_maintenance_owner` 串行化 rebuild/compact/import/backup 管理
操作。它们是派生控制记录，不能取代业务事实。

`retrieval_documents` 是可重建的文本语料，`retrieval_vectors` 保存对应的 embedding、
dimension、model 和 content hash。Ollama provider 的批处理、重试、降级和 fingerprint 由
service/host worker 实现；provider outage 只能使 projection degraded，不能丢失 canonical
task、label、ontology、signal 或 entity 数据。

## 9. Import journal 与 attachment staging

`import_journal` 支持 `jsonl|sqlite_v30` source fingerprint 和
`prepared|staged|validated|published|completed|failed` 阶段，保存源路径、staged database/
attachment root、canonical root、manifest、previous identity 和错误。`attachment_staging`
按 journal/attachment 保存源路径、staged 路径、期望/观测 size 与 SHA-256，以及
`planned|copied|verified|published|failed` 阶段。

目标流程是只读打开 SQLite v30，先做 schema、计数、引用、attachment checksum 和 board
isolation preflight，再将附件复制到同文件系统 staging；DB commit 后原子发布，崩溃后按
journal resume。源文件永不修改，重复 fingerprint 返回已完成结果。当前 schema 已提供
这些耐久结构，但 importer/service/HTTP/CLI 管理入口仍是待闭合的 parity slice。

## 10. 事务、约束和当前 ownership

1. `kanban serve` 是唯一 canonical DB owner，开启 `PRAGMA foreign_keys = ON`；CLI、MCP、
   Desktop 不直接打开数据库。
2. 所有 mutation 使用 immediate transaction；task snapshot、run、event、labels/ontology/
   signals 和 projection job enqueue 必须整批提交或整批回滚。
3. claim、heartbeat、release、review、complete、block、ontology CAS 和导入阶段都使用
   owner/token、lock version 或 fingerprint guard。
4. board-scoped 外键、唯一约束、idempotency、attachment path guard、foreign-key check 和
   schema shape validator 共同构成数据库边界。
5. FTS/vector/graph/context/缓存及其他索引始终可删除和重建；它们不能反向写 canonical
   事实，也不能成为导入计数或业务状态的依据。

当前这些 schema/migration 能力仍位于 `kanban-store-turso`；application、server、client
和各 adapter 尚未完成全功能 wiring。目标结构是把 application 与 Turso repository 合并
为 `kanban-service`，再由 `kanban-protocol`、`kanban-client`、`kanban-server`、CLI、MCP 和
Desktop 共享同一 service path。迁移删除旧 backend 前，必须由 parity ledger 和逐项测试证明
全部业务语义及旧数据已经有 owner。


---

# 文件：docs/CLI_SPEC.md

# `kanban` CLI 规范

`kanban` 是 canonical localhost application host 的薄适配器。除 `kanban serve` 外，
所有命令都只创建 `kanban-client` 并调用 `http://127.0.0.1:8721`；CLI 不打开、初始化或
fallback 到任何数据库。

当前实现只覆盖 board、task、comment、dependency、run 和 event；这不是最终功能边界。
labels、signals、ontology、search、graph、vector、context、projection、maintenance、
init/import 等能力必须按 parity ledger 恢复为 localhost client 命令。旧的直接数据库
执行路径不会恢复。

## 1. 全局选项

```text
kanban [OPTIONS] <COMMAND>
```

| 选项 | 环境变量 | 默认值 | 作用 |
|---|---|---|---|
| `--server-url <URL>` | `KANBAN_SERVER_URL` | `http://127.0.0.1:8721` | client 访问的 loopback host；仅允许 `http://` loopback URL |
| `--board <SLUG-OR-ID>` | `KB_BOARD` | `default` | board-scoped selector 的上下文 |
| `--actor <NAME>` | `KANBAN_ACTOR` | `USER` → `USERNAME` → `local` | CLI 解析后作为 `X-KB-Actor` 发送并用于审计 |
| `--json` | — | 关闭 | 输出稳定 JSON envelope |

`--db` 不是全局选项，只能由 `serve` 使用。相同参数可放在命令前，clap 将其作为 global
option 解析。

### JSON 成功与错误

`--json` 成功输出为 `{ "data": ... }`；CLI 使用自己的 output DTO，不保证保留 HTTP
response 的 pagination/cursor `meta`。运行期错误输出为：

<!-- schema-doc-ignore: CLI runtime error 的说明性示例，不绑定某个具体 command output contract -->
```json
{
  "error": {
    "code": "server_unavailable",
    "message": "server unavailable: connection refused",
    "exit_code": 9
  }
}
```

脚本只依赖 `error.code` 和进程退出码，不解析 message。clap 参数解析失败仍由 clap 写入
stderr 并退出 `2`，不会输出运行期 JSON error。

当前 adapter 重点错误码：

| code | exit code | 语义 |
|---|---:|---|
| `invalid_input` / `invalid_response` | 2 | 参数、selector 或 host 响应无效 |
| `not_found` | 3 | 资源不存在 |
| `invalid_transition`、`execution_plan_required`、`steps_incomplete`、`dependency_cycle` | 4 | application/state machine 拒绝操作 |
| `claim_conflict`、`claim_token_mismatch`、`idempotency_conflict` | 5 | claim 或实体幂等冲突 |
| `dependency_blocked` | 6 | 依赖阻止状态转换 |
| `server_unavailable` | 9 | `kanban serve` 未运行或 loopback 连接失败 |
| `feature_not_available` | 10 | 命令明确未迁移，且未触碰数据库 |

## 2. 唯一数据库 owner：`serve`

```text
kanban serve [--db <PATH>] [--dispatcher-profile <PATH>]
             [--host <LOOPBACK-IP>] [--port <PORT>]
```

- `--db <PATH>` 或 `KANBAN_DB` 只配置 host 使用的 canonical Turso 文件。
- 未指定时，Linux 默认路径为 `~/.local/share/kb/kanban.db`（通过平台 data-local 目录解析）。
- `--host` 默认 `127.0.0.1`，非 loopback 地址直接返回 `invalid_input`。
- `--port` 默认 `8721`。
- 没有 `--dispatcher-profile` 时不自动消费 queue；传入 profile 才启用同进程单 worker dispatcher。
- Ctrl-C 第一次 graceful shutdown，第二次 force stop。

启动后 host 负责初始化 schema、打开/关闭 Turso 和提供全部 HTTP route。其他 CLI 命令在 host
未启动时只返回 `server_unavailable`，不会创建数据库文件。

## 3. Board

```text
kanban board list [--include-archived]
kanban board columns [BOARD]
```

两者都是只读 client query：`board list` 返回 `ListBoardsResponse`，`board columns` 返回
`ListBoardColumnsResponse`。当前 CLI 没有 board create/use/archive/config 命令。

## 4. Task

### 4.1 创建、列表、详情

```text
kanban task create <TITLE>
  [--description <TEXT>] [--status triage|todo|scheduled|ready]
  [--assignee <NAME>] [--priority <0..=3>]
  [--scheduled-at <MS>] [--due-at <MS>] [--max-retries <N>]
  [--metadata <JSON-OBJECT>] [--idempotency-key <KEY>] [--task-id <T_ID>]

kanban task list
  [--status triage|todo|scheduled|ready|running|blocked|review|done|archived]...
  [--priority <0..=3>]...
  [--plan-filter plan_needed|has_steps|incomplete_required_steps]...
  [--assignee <NAME>] [--query <TEXT>] [--include-archived]
  [--limit <N>] [--offset <N>] [--sort <SORT>]

kanban task show <TASK_SELECTOR>
```

`TASK_SELECTOR` 可为全局 `t_...`、`board#seq`、`#seq` 或数字 seq；client 在发 HTTP mutation
前将 board-local ref 解析为全局 ID。`task show --details` 需要 deferred ontology
projection，当前暂时返回 `feature_not_available`；ontology 切片恢复后必须改为真实查询。

当前 `task create` 尚未接通 labels/dependencies，`task list` 也尚未接通 label filter；
这些都是 parity ledger 中必须恢复的 CLI surface。
`--search` 是 `--query` 的隐藏兼容 alias。成功输出分别使用 task create、CLI task-list
（只含 `data`，不保留 HTTP pagination `meta`）和 task show 的公开 output DTO。

`--sort` 使用 HTTP wire 值：`seq|-seq|title|-title|status|-status|position|-position|
priority|-priority|assignee|-assignee|scheduled_at|-scheduled_at|due_at|-due_at|
created_at|-created_at|updated_at|-updated_at`，默认值为 `position`。负号开头的值需要写成
`--sort=-updated_at`，避免被 clap 解释为另一个 option。

### 4.2 Execution plan

```text
kanban task step not-required <TASK_SELECTOR> --reason <TEXT>
```

这会调用 `POST /api/v1/tasks/{task_id}/execution-plan/not-required`，返回
`MarkExecutionPlanNotRequiredResponse`。它是 promote 前的显式 plan gate。

### 4.3 State machine transitions

```text
kanban task promote <TASK_SELECTOR>
kanban task claim <TASK_SELECTOR> [--ttl-ms <MS>]
kanban task heartbeat <TASK_SELECTOR> --claim-token <TOKEN>
    [--ttl-ms <MS>] [--note <TEXT>]
kanban task release <TASK_SELECTOR> --claim-token <TOKEN>
kanban task review <TASK_SELECTOR> [--claim-token <TOKEN>] [--force]
kanban task done <TASK_SELECTOR> [--claim-token <TOKEN>] [--force]
kanban task block <TASK_SELECTOR> [<REASON>|--reason-file <PATH|->]
    [--claim-token <TOKEN>] [--force]
```

`done` 有 visible alias `complete`。claim 是原子 `ready -> running`，并在同一 application
path 创建 run；heartbeat/release/review/done/block 继续复用该 path。release 仅接受当前
owner 的 token，成功后任务回到 ready；错误 token 返回 `claim_token_mismatch`/相应稳定错误码。
`task block` 的 inline reason 与 `--reason-file` 互斥；`-` 从 stdin 读取，输入上限为 1 MiB。

每个命令的人类输出只显示精简 task 状态；`--json` 输出对应 contract response。CLI 不自行
解释状态机或直接修改 run。

### 4.4 Steps

```text
kanban task step add <TASK_SELECTOR> <TITLE>
  [--body <TEXT>] [--link-task <TASK_SELECTOR>] [--position <N>]
  [--required|--optional] [--idempotency-key <KEY>]
kanban task step list <TASK_SELECTOR>
kanban task step update <TASK_SELECTOR> <STEP_SELECTOR>
  [--title <TEXT>] [--body <TEXT>] [--link-task <TASK_SELECTOR>|--unlink-task]
  [--position <N>] [--required|--optional]
```

`STEP_SELECTOR` 可为全局 `step_...` 或该 task 下的 `S<n>`。add/list/update 返回同一
`ApiTaskSteps` shape；add 的 idempotency key 仅在实体 task 内生效。

## 5. Comment

```text
kanban comment add <TASK_SELECTOR> <BODY>
  [--kind note|decision|signal] [--author <NAME>]
  [--author-type user|agent] [--agent-type <TYPE>]
  [--metadata-json <JSON-OBJECT>] [--idempotency-key <KEY>]
kanban comment list <TASK_SELECTOR>
```

add 是 mutation，list 是 query。add 的 key 属于 task；相同 key 与相同 payload 可安全重放，
不同 payload 返回 `idempotency_conflict`。未指定 author 时由 host actor 规则填充。

## 6. Dependency

顶层命令名为 `dep`，`dependency` 是 visible alias：

```text
kanban dep add <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
kanban dep list <TASK_SELECTOR>
kanban dep remove <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
```

add/remove 是 mutation，list 是 query。client 先解析两个 selector；server 负责同 board、FK、
唯一约束和 cycle 检查。dependency create 没有额外 receipt/idempotency flag。

## 7. Runs 与 events（只读）

```text
kanban runs <TASK_SELECTOR>
kanban run show <RUN_ID>
kanban run logs <RUN_ID>
kanban events [TASK_SELECTOR] [--after <ID>] [--limit <N>]
```

- `RUN_ID` 必须是全局 `r_...`。
- `run logs` 有 visible alias `log`，没有 `--tail-bytes` 或其他 tail 参数。
- log 始终读取固定 256 KiB bounded snapshot；JSON 输出包含 `run_id`、`content`、`truncated`
  和兼容 DTO 中固定为 `null` 的 `tail_bytes`，但用户不能指定 tail 大小。
- run 不能由 CLI 创建或修改；只能通过 task claim/heartbeat/release/review/done/block 产生和更新。
- `events` 可按当前 board 和 task selector 过滤，JSON 输出包含事件 `data`，但当前 CLI
  output 不保留 HTTP `meta.next_after`；事件 payload 对未知 kind 保持原 JSON。

## 8. 未迁移命令与停止行为

```text
kanban init
kanban <未列出的旧命令>
```

`init` 和未知的顶层 clap external subcommand 稳定返回：

<!-- schema-doc-ignore: 迁移期间 feature_not_available 的说明性错误示例 -->
```json
{
  "error": {
    "code": "feature_not_available",
    "message": "...",
    "exit_code": 10
  }
}
```

该路径不读取配置数据库、不创建文件、不 fallback 到 SQLite。未注册的嵌套子命令（例如
`kanban task archive`）由 clap 在发起任何 HTTP/DB 操作前以参数错误退出 `2`；它们不使用
runtime JSON envelope。尚未完成的 labels、signals、search、maintenance、projection、
graph、vector 等顶层 surface 暂时返回 `feature_not_available`；只要该临时路径仍存在，
对应 parity 项就不能标记完成。host 停止或端口不可达时，已迁移命令返回
`server_unavailable`（exit code `9`），而不是切换到第二条执行路径。


---

# 文件：docs/API_SPEC.md

# 本地 HTTP API 规范

本 API 是 `kanban serve` 提供的本机应用服务入口。CLI、MCP 和 Desktop
只能通过 typed localhost client 调用它；它们不打开数据库，也不各自实现业务状态转换。

默认监听地址为 `http://127.0.0.1:8721`，只接受 loopback 绑定。所有产品路由的基础路径为
`/api/v1`；健康检查是 `/health`。

本文件先描述当前已接入的 single-host 路由；它不是最终功能边界。labels、signals、
ontology、search、graph、vector、context、projection、maintenance 与 SSE 必须按 parity
ledger 恢复到同一 localhost API，并随每个纵向切片补齐本规范。旧的直接数据库路径不会
恢复。

## 1. 通用契约

### 1.1 HTTP

- JSON 请求使用 `Content-Type: application/json`。
- JSON 响应使用 `Content-Type: application/json`。
- GET 查询只使用各 endpoint 列出的 URL query 参数。task list 与 event list 会严格拒绝
  未知参数；没有 query contract 的 endpoint 不把未知参数解释为新能力。
- 服务端只绑定 loopback；client 也拒绝非 loopback URL。
- 正常响应使用 `{ "data": ... }`；事件列表额外包含 `meta`。

<!-- schema-doc-ignore: 通用 data envelope 的说明性最小示例，不代表具体 endpoint contract -->
```json
{ "data": {} }
```

分页/游标响应使用：

<!-- schema-doc-ignore: 分页 meta 的说明性示例，具体 endpoint 仍以各自 fixture 为准 -->
```json
{ "data": [], "meta": { "limit": 100, "offset": 0, "total": 0 } }
```

事件列表的 `meta` 形状为 `{ "next_after": 123 }`。

### 1.2 actor

mutation 请求中的 actor 由服务端按以下顺序解析：

1. JSON body 的 `actor`（如果该 request DTO 有此字段）；
2. `X-KB-Actor` 请求头；
3. `kanban serve` 启动时配置的默认 actor。

actor 会被写入 canonical mutation/event 审计记录。只读请求不需要 body；client 会发送
`X-KB-Actor`，服务端不会据此改变查询结果。comment create 是命名上的例外：
`CreateCommentRequest.author` 占据 body actor 的优先级，并作为 comment author/event
actor；body 未提供 author 时才回退到 header 和 host 默认值。

### 1.3 错误封装与状态码

由 handler/ApplicationService 返回的产品错误使用：

<!-- schema-doc-ignore: 错误 envelope 的说明性示例，code/message 用于解释状态码映射 -->
```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot promote task from status done"
  }
}
```

`code` 是稳定机器契约，`message` 只供人阅读，调用方不得解析 message。Axum 在 route
匹配之前产生的 malformed path、method-not-allowed 等框架级 4xx 不属于该产品 error
envelope；typed client 不会主动构造这类请求。

| `error.code` | HTTP | 用途 |
|---|---:|---|
| `invalid_input` | 400 | handler 已接收的 JSON、path/query value 或字段值无效 |
| `not_found` | 404 | board、task、step、dependency 或 run 不存在 |
| `conflict` | 409 | 一般业务冲突或唯一性冲突 |
| `idempotency_conflict` | 409 | 同一实体 key 重放但 canonical payload 不同 |
| `dependency_cycle` | 409 | dependency 会形成环 |
| `execution_plan_required` | 409 | promote 前没有计划或 not-required 标记 |
| `steps_incomplete` | 409 | review/done 的必需 step 尚未完成 |
| `dependency_blocked` | 409 | 依赖阻止进入目标状态 |
| `claim_conflict` | 409 | 并发 claim 或 claim 条件冲突 |
| `invalid_transition` | 409 | 状态机拒绝转换 |
| `claim_token_mismatch` | 403 | claim owner/token 不匹配 |
| `feature_not_available` | 501 | 当前 single-host 尚未提供该 surface |
| `internal` | 500 | storage 或 host 内部错误 |

### 1.4 ID 与 task selector

HTTP path 中的 task 必须使用 canonical 全局 `t_...` ID；run 使用全局 `r_...` ID，step
使用全局 `step_...` ID。typed `kanban-client` 对 `t_...` 直接透传；对 board 上下文中的
`board#seq`、`#seq`、数字 seq，先通过 task list 解析为全局 ID，再发起后续请求。服务端
不在 mutation handler 内实现第二套 selector 语义。

## 2. 健康与看板（只读）

### `GET /health`

返回 `HealthResponse`：`data.ok`、`data.db`（当前为 `"turso"`）、`data.version`、
`data.db_path` 和 `data.db_fingerprint`。该路由只检查已打开 host 的健康状态，不创建备用数据库。

### `GET /api/v1/boards`

Query：`include_archived`（默认 `false`）。返回 `ListBoardsResponse`，即 `data` 为
`ApiBoard[]`。

### `GET /api/v1/boards/{board}/columns`

返回 `ListBoardColumnsResponse`，即固定看板列的 `ApiBoardColumn[]`。列的 status 使用
`triage`、`todo`、`scheduled`、`ready`、`running`、`blocked`、`review`、`done`、`archived`。

当前 host 没有 board create/get/archive route；调用这些旧路径应视为未提供功能，而不是
另起一个数据库路径。

### `GET /api/v1/stats`

Query：`board`（默认 `default`）。返回 `StatsResponse`（`data: QueueStats`）：
`board_id`、`generated_at`、按 status 计数、过期 running claim、blocked reason 计数、
未规划 active task 数，以及仍有未完成 required step 的 active parent 数。该 query 通过
ApplicationService 读取 canonical Turso snapshot，不执行 claim/reclaim 或其他 mutation。

## 3. Task 读取与创建

### `GET /api/v1/boards/{board}/tasks`（只读）

返回 `ListTasksResponse`：`data: ApiTask[]` 与
`meta: { limit, offset, total }`。

支持的 query 参数：

- `status`：可重复，任务状态枚举；
- `priority`：可重复，`0..=3`；
- `plan_filter`：`plan_needed`、`has_steps`、`incomplete_required_steps`；
- `assignee`、`q`、`include_archived`；
- `limit`（默认 100，最大 1000）、`offset`（默认 0）；
- `sort`：`seq`、`-seq`、`title`、`-title`、`status`、`-status`、
  `position`、`-position`、`priority`、`-priority`、`assignee`、
  `-assignee`、`scheduled_at`、`-scheduled_at`、`due_at`、`-due_at`、
  `created_at`、`-created_at`、`updated_at`、`-updated_at`（默认 `position`）。

当前 task list 尚未接通 label filter；传入 `label` 会暂时返回
`feature_not_available`。labels 切片完成后必须恢复该 query contract。

### `POST /api/v1/boards/{board}/tasks`（mutation）

请求为 `CreateTaskRequest`：`title` 必填；可选 `task_id`、`idempotency_key`、
`description`、`status`（`triage|todo|scheduled|ready`）、`assignee`、`priority`、
`scheduled_at`、`due_at`、`max_retries`、`metadata`、`actor`。`labels` 和 `depends_on`
字段当前必须为空；queue/labels 切片必须在共享 application transaction 中恢复这两个
surface。

成功返回 HTTP `201` 与 `CreateTaskResponse { data: ApiTask }`。同一 board 内相同
`idempotency_key` 与相同 canonical payload 返回已有 task；payload 不同返回
`idempotency_conflict`。请求中的 `status` 是期望初始状态；ApplicationService 仍会应用
execution-plan、依赖与排期 guard，例如尚未满足 ready 条件时返回的 task 会处于 `todo`。

### `GET /api/v1/tasks/{task_id}`（只读）

`task_id` 必须是全局 `t_...`。返回 `GetTaskResponse`：`data: ApiTask`，当前不带 ontology
`meta`。`include=ontology` 当前尚未接通，ontology 切片完成后必须恢复。

## 4. Execution plan 与 task state machine

所有以下 endpoint 都调用同一个 ApplicationService/state machine；不存在通用的
`POST .../transitions/{target_status}`。

### `POST /api/v1/tasks/{task_id}/execution-plan/not-required`（mutation）

请求：`{ "reason": string, "actor": string|null }`。返回 `MarkExecutionPlanNotRequiredResponse`
（`data: ApiExecutionPlan`）。这是 walking skeleton 中显式完成计划前置条件的操作。

### `POST /api/v1/tasks/{task_id}/transitions/promote`（mutation）

请求：`{ "actor": string|null }`。只允许状态机认可的 todo/scheduled 到 ready 转换；返回
`PromoteTaskResponse`（`data: ApiTask`）。

### `POST /api/v1/tasks/{task_id}/transitions/claim`（mutation）

请求字段：`actor`、`ttl_ms`（默认 300000）、可选 `worker_profile`、`metadata`。这是原子
`ready -> running` claim，同时创建 active run 和 event。返回 `ClaimTaskResponse`：
`data.task`、`data.run`、`data.claim_token`、`data.claim_expires_at`。竞争调用恰有一个成功，
失败者收到 `claim_conflict`。

### `POST /api/v1/tasks/{task_id}/transitions/heartbeat`（mutation）

请求：`claim_token` 必填，`ttl_ms`（默认 300000），可选 `actor`、`note`。token/owner 不匹配
返回 `claim_token_mismatch`；成功返回 `HeartbeatTaskResponse`（`data: ApiTask`）。

### `POST /api/v1/tasks/{task_id}/transitions/release`（mutation）

请求：`claim_token` 必填，可选 `actor`。只有 active running claim owner 能调用；事务内将 task
回到 ready、清除 claim、取消 active run 并写 `task.released`。成功返回 `ReleaseTaskResponse`
（`data: ApiTask`），失败不会留下部分写入。

### `POST /api/v1/tasks/{task_id}/transitions/submit-review`（mutation）

请求：可选 `actor`、`claim_token`、`summary`，以及 `force`（默认 false）。返回
`SubmitReviewTaskResponse`（`data: ApiTask`）。

### `POST /api/v1/tasks/{task_id}/transitions/complete`（mutation）

请求：可选 `actor`、`claim_token`、`summary`、`result`，以及 `force`（默认 false）。返回
`CompleteTaskResponse`（`data: ApiTask`）。

### `POST /api/v1/tasks/{task_id}/transitions/block`（mutation）

请求：`reason` 必填；可选 `actor`、`claim_token`、`force`（默认 false）。返回
`BlockTaskResponse`（`data: ApiTask`）。

review、complete、block 的 claim 校验、required steps、依赖检查与 run 更新均在同一
application transaction 中完成。

## 5. Comments

### `GET /api/v1/tasks/{task_id}/comments`（只读）

返回 `ListCommentsResponse`（`data: ApiComment[]`）。

### `POST /api/v1/tasks/{task_id}/comments`（mutation）

请求为 `CreateCommentRequest`：`body` 必填；可选 `idempotency_key`、`author`、`kind`
（wire enum 为 `note|decision|signal`）、`author_type`（`user|agent`）、`agent_type`、
`metadata`。当前 canonical path 只接受 `note` 与 `decision`；`signal` 稳定返回
`feature_not_available`。signals 切片必须恢复 `signal` 并保持 comment backlink。
成功返回 HTTP `201` 与 `CreateCommentResponse`（`data: ApiComment`）。idempotency key
属于 task；相同 key/相同 payload 重放返回已有 comment，不同 payload 返回
`idempotency_conflict`。

## 6. Steps 与 execution plan

### `GET /api/v1/tasks/{task_id}/steps`（只读）

返回 `ListStepsResponse`（`data.task_id`、`data.steps`、`data.execution_plan`）。

### `POST /api/v1/tasks/{task_id}/steps`（mutation）

请求：`title` 必填；可选 `idempotency_key`、`body`、`linked_task_ref`、`position`、
`required`（默认 true）、`actor`。`linked_task_ref` 在 HTTP contract 中必须是全局
`t_...` ID；board-local selector 由 typed adapter 先解析。成功返回 HTTP `201` 与
`CreateStepResponse`。step create 的 key 只在当前 task 内幂等。

### `PATCH /api/v1/tasks/{task_id}/steps/{step_id}`（mutation）

请求可更新 `title`、`body`、`linked_task_ref`/`unlink_task`、`position`、`required`、
`actor`；不改变 step status。返回 `UpdateStepResponse`（同一 `ApiTaskSteps` 形状）。

## 7. Dependencies

### `GET /api/v1/tasks/{task_id}/dependencies`（只读）

返回 `ListDependenciesResponse`（`data.task`、`parents`、`children`、`edges`）。

### `POST /api/v1/tasks/{task_id}/dependencies`（mutation）

请求：`parent_task_id` 必须是同一 board 的全局 task ID，可选 `actor`。返回
`AddDependencyResponse`。复合唯一约束保证重复 add 幂等；跨 board、未知 task 和 dependency
cycle 拒绝。

### `DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}`（mutation）

删除同一 board 的 parent edge，返回 `RemoveDependencyResponse`（当前 dependencies 快照）。
目标 task 存在但 edge 已不存在时是成功 no-op，不追加 remove event。

## 8. Runs 与 log（run 不是独立 mutation surface）

run 只能由 task claim 创建，并由 heartbeat/release/review/complete/block 同事务更新。HTTP
没有 run create/update endpoint；以下全部是只读查询。

### `GET /api/v1/tasks/{task_id}/runs`

返回 `ListRunsResponse`（`data: ApiRun[]`）。

### `GET /api/v1/runs/{run_id}`

返回 `GetRunResponse`（`data: ApiRun`）。

### `GET /api/v1/runs/{run_id}/log`

返回 `GetRunLogResponse`：`data.run_id`、`data.content`、`data.truncated`。读取使用固定
256 KiB 的文件尾部 snapshot；超过上限时返回最后 256 KiB 并设置 `truncated=true`。
typed client 不发送 `tail` query，服务端也不会把未知 query 解释为可配置读取范围；没有
任意文件路径输入或第二种 log 协议。

## 9. Events

### `GET /api/v1/events`

Query：`board`（默认 `default`）、可选全局 `task_id`、`after`（默认 0）、`limit`（默认
100，服务端上限 1000，超过时收敛到 1000）。返回 `ListEventsResponse`：
`data: StreamEventData[]` 与 `meta.next_after`。
known event kind 使用 typed payload；未知 kind 保留原 JSON payload，不被 adapter 丢弃。

event list 是只读；所有 mutation 通过 ApplicationService 写 canonical event。

## 10. 停止路径

服务停止后，client 返回 `server_unavailable`，不得 fallback 到嵌入式数据库、旧 SQLite
路径或另一个 host。迁移期间尚未接通的 labels/signals/search/maintenance 等命令暂时返回
`feature_not_available`，不会触碰数据库；只要该临时响应仍存在，对应 parity 项就不能
标记完成。


---

# 文件：docs/SCHEMA_CONTRACTS.md

# JSON Schema 与机器契约

## 1. 适用范围与权威来源

本文件只描述 wire contract、schema artifact、surface catalog 以及它们的校验方式。
它不定义业务状态机，也不把 schema 校验当作事务或权限校验。

当前单 Host 产品路径是：

```text
CLI / MCP / Desktop
        ↓ typed localhost client
kanban serve (HTTP)
        ↓
ApplicationService + State Machine
        ↓
kanban-store-turso → canonical Turso database
```

`kanban-contract` 是公开 DTO、事件 payload、错误 envelope、operation inventory 和
transport descriptor 的 Rust 权威来源；只有根目录私有 `xtask` 生成和校验 JSON Schema
artifact。`kanban-server`、`kanban-client`、CLI、MCP 和 Desktop 是运行时 producer/consumer，
不能各自复制一套 DTO 或业务错误解释。

语义分工如下：

- DTO/schema：字段、类型、必填/可选、未知字段策略和基础值域。
- `endpoint_catalog()` 与 `surface_operation_catalog()`：机器契约的 source inventory。
  当前仍含尚未迁入单 Host 路径的历史条目；这些条目是完整功能 parity 义务。HTTP、CLI、
  MCP 身份必须同时由真实 router/adapter 证明，catalog 本身不创建 route，也不能单独证明
  adoption。
- ApplicationService、`kanban-core` 状态机和 `kanban-store-turso`：事务、CAS、board
  isolation、依赖和 run/event 一致性。
- `docs/API_SPEC.md`、`docs/CLI_SPEC.md`：用户可见的 operation、HTTP/退出码和输出行为。

`schemas/` 中的文件是生成/提交产物，不是新的事实来源。若 source inventory、真实
server route、adapter 或测试与 committed artifact 冲突，以当前 source 和运行时为准，
先修正 contract source，再运行 `just schema-generate`；不得手工改 generated JSON 来掩盖漂移。

## 2. 契约状态与当前迁移边界

`operation_inventory()` 的每一项必须使用以下状态：

| 状态 | 含义 | 证据要求 |
| --- | --- | --- |
| `planned` | 已确定精确边界，尚未生成 root | 不得填写伪 schema、fixture 或采用证据 |
| `generated` | DTO、root 和 fixture 已生成 | 不得声称运行时已经采用 |
| `adopted` | 真实 producer/consumer 使用同一 DTO/contract | fixture、producer witness、consumer witness 和精确测试 |
| `excluded` | 明确不是稳定 JSON contract | 非空排除理由；不得同时声称有 schema/fixture |

`adoption` witness 必须同时记录 `operation`、`contract_id`、`surface`、`direction`、
`package`、`test_target` 和 `exact_test`。request/input 的 producer 必须由真实 DTO
序列化；consumer 必须从 committed fixture 进入真实 router/handler。response/output 的
producer 必须来自真实 adapter 响应路径。producer 和 consumer 不得共用一个高层 exercise
helper。

当前 committed `schemas/json-schema/draft-2020-12/operations.json` 与
`surface-operations.json` 仍可见历史的 SQLite、projection、label、signal、search、
graph/vector 和维护命令条目。它们记录必须恢复的完整产品能力，但不能仅凭 catalog 解释为
当前单 Host runtime 已经采用。尚未接入真实 adapter 的条目必须保持 `planned` 或
`generated`，并在对应纵向切片完成 DTO、fixture、route/tool/command 与 exact witness 后
才能改为 `adopted`。除非某条记录只是重复 wire 表达且业务能力已有明确的新 owner，不得把
它改成 `excluded` 来缩小功能范围。在所有 parity 项有真实证据前，不能声称
`just schema-audit-closed` 或整个 schema catalog 已完全收口。

SQLite backend、`kanban-sqlite`/`kanban-local`、Tantivy/LanceDB/Oxigraph projection 以及
helper subprocess 不属于目标 runtime 架构；旧源码只作为迁移证据，不得重新接入 active
workspace。它们承载的 labels、signals、ontology、search、graph、vector、context 与维护
语义必须迁入 `kanban-core`、`kanban-service`、`kanban-protocol` 及各 adapter 后，旧目录
才允许删除。

## 3. 当前 active operation 的精确 contract

下面列出本轮已接入真实 HTTP、typed client 与 adapter 的 run/event contract。每个路径
使用独立的 path/query/success root；共享 headers 仍由各 endpoint 的 descriptor 明确引用。

| Operation | HTTP path | contract id |
| --- | --- | --- |
| stats | `GET /api/v1/stats` | `api.get-stats.query`, `api.get-stats.response` |
| run list | `GET /api/v1/tasks/:task_id/runs` | `api.list-runs.path`, `api.list-runs.response` |
| run show | `GET /api/v1/runs/:run_id` | `api.get-run.path`, `api.get-run.response` |
| run log | `GET /api/v1/runs/:run_id/log` | `api.get-run-log.path`, `api.get-run-log.response` |
| event list | `GET /api/v1/events` | `api.list-events.query`, `api.list-events.response` |

对应的 schema root 和正例 fixture 位于：

```text
schemas/json-schema/draft-2020-12/
schemas/fixtures/api/list-runs-*.v1.valid.json
schemas/fixtures/api/get-run-*.v1.valid.json
schemas/fixtures/api/get-run-log-*.v1.valid.json
schemas/fixtures/api/list-events-*.v1.valid.json
```

CLI 的当前读取面是 `events`、`runs`、`run show`、`run logs`，其 output contract 分别为
`cli.events.output`、`cli.runs.output`、`cli.run-show.output` 和 `cli.run-logs.output`。
`run logs` 不再接受 `--tail-bytes`；固定的 `ApiRunLog` 返回 `run_id`、完整或截断的
`content` 和 `truncated`，CLI JSON 保留现有 nullable `tail_bytes` 字段但新路径返回
`null`。MCP 使用 `event_list`、`run_list`、`run_show`、`run_log`，只调用
`kanban-client`；Desktop 复用同一 typed HTTP client，不打开数据库。

`ListEventsResponse` 的 `meta.next_after` 是游标；已知 event kind 的 payload 必须通过
对应 typed payload 校验，未知 kind 保留任意 JSON value（包括数组、标量和嵌套对象），
不能静默丢字段。run/event 是只读 adapter surface；run 的创建和更新仍由 task claim、
heartbeat、release、review、done、block 的共享 ApplicationService mutation path 完成。

## 4. Wire 规则

- 方言固定为 JSON Schema Draft 2020-12；request/input 使用
  `SchemaSettings::for_deserialize()`，response/output 使用 `for_serialize()`。
- root ID 使用 `urn:kanban-tool:schema:<surface>:<semantic-name>:v1`；root 版本与 crate
  或 API 版本独立，破坏性 wire change 必须提升 root 版本并删除被替代 artifact，不保留
  双轨输出。
- schema 必须自包含，只允许本地 `#/$defs/...` `$ref`；产物不得包含时间戳、绝对路径或
  网络 resolver。
- `#[serde(deny_unknown_fields)]` 与 `required-nullable` 必须反映真实 DTO 行为。显式
  `null` 与省略字段不是同一语义；不能在 schema 中把 required-nullable 改成 optional。
- 基础 JSON 校验不覆盖跨字段业务规则。状态转换、claim token、依赖 cycle、board
  isolation、幂等 key、事务原子性和错误 code 必须由 ApplicationService/store 测试证明。
- 不为 schema tooling 引入 HTTP、文件 resolver、TLS、OpenAPI 或生产 runtime validator。

## 5. 依赖边界与单 Host gate

active workspace 只保留 `kanban-core`、`kanban-application`、`kanban-contract`、
根目录私有 `xtask`、`kanban-store-turso`、`kanban-client`、`kanban-cli`、`kanban-mcp`、
`kanban-server` 和 Desktop Tauri host。数据库依赖方向固定为：

```text
kanban-server → kanban-store-turso → turso
kanban-cli / kanban-mcp / Desktop → kanban-client → localhost HTTP
```

CLI、MCP、Desktop、test fixture 和 test-support 不得依赖 `kanban-store-turso`、
`kanban-sqlite`、`kanban-local`、`rusqlite` 或任何数据库-owning path；只有
`kanban-server` 可以初始化、打开和关闭 Turso。该限制覆盖 normal、dev、build、target-
specific dependency 和测试 fixture，不只检查源码 import。

`scripts/check-single-host-dependencies.py` 是单 Host manifest gate；它拒绝 legacy package
进入 workspace、projection helper 进入 active workspace，以及任意 adapter 的 forbidden
dependency alias。schema tooling 另有独立边界：根目录私有 `xtask` 只能作为离线生成/
校验工具，不能进入产品 runtime graph。`kanban-mcp` 会启用 `kanban-contract/schema`
来生成 RMCP tool input schema；这不授权它依赖 `xtask`、`jsonschema` runtime 或数据库
crate。

## 6. Artifact 目录与生成规则

```text
schemas/
  fixtures/
    api/ cli/ jsonl/ metadata/ sse/
  json-schema/draft-2020-12/
    operations.json
    surface-operations.json
    manifest.json
    api/ cli/ jsonl/ metadata/ sse/
```

`operations.json` 记录 semantic contract；`surface-operations.json` 记录精确传输操作；
`manifest.json` 记录 root、fixture 和 hash。连续生成必须 byte-identical。公开 Markdown
中的完整 JSON 示例如需进入 schema docs gate，必须由 `schema-doc` marker 绑定到
manifest-owned fixture；片段、伪值或解释性 payload 使用 `schema-doc-ignore` 并给出理由。

transport descriptor 由 `endpoint_catalog()` 生成，至少区分：

- `Path`、`Query`、`Headers`、`Body`、`Success`、`Error` 和 `Sse` location；
- `RequiredOne`、`OptionalOne`、`RepeatedOrdered` cardinality；
- endpoint-specific exact contract 与可复用的 `SharedComponent`。

route、CLI leaf command 或 MCP tool 新增/删除/重命名时，必须同步 source inventory、
descriptor、fixture、adoption witness 和 generated artifact；不能用 family/wildcard 记录
来绕过审计。

## 7. Schema recipes 与验证顺序

根目录 `xtask/` 是 `publish = false` 的私有 workspace leaf。其命令面如下：

```text
xtask schema generate
xtask schema check
xtask schema audit
xtask schema witnesses
xtask deps check
xtask agents check
```

`schema generate` 生成并随后校验 committed schema tree；`schema check` 只读检查重新生成
结果、fixture、manifest 和 hash 漂移；`schema audit` 校验 operation/surface catalog，可
附加 `--require-closed` 要求没有未闭合项；`schema witnesses` 在 audit 后输出 `adopted`
operation inventory 的 JSON；`deps check` 运行 schema dependency policy、cargo tree 隔离
和 single-host 依赖检查；`agents check` 检查根 `AGENTS.md`、技能包结构以及 active
recipe/package 映射。

开发者入口仍由 `justfile` 提供：`just schema-generate`、`just schema-check` 和
`just schema-audit-closed` 使用 `xtask`，通用的 `just schema ...`、`just deps ...` 和
`just agents ...` 透传对应命令。`xtask` 不反向调用 `just`；它只直接调用必要的脚本和
自身的 schema/catalog 逻辑。

当前 `justfile` 仍提供以下 schema 入口：

```text
just schema-generate
just schema-check
just schema-docs
just schema-fmt
just schema-tool
just schema-dependency-isolation-self-test
just schema-dependency-isolation
just schema-adoption-witness-self-test
just schema-adoption-witness
just schema-surface-audit
just schema-contract
just schema-audit-closed
```

- `schema-generate` 生成 source inventory 对应的 committed tree；`schema-check` 只读检查
  fresh generation、fixture、manifest 和 hash 漂移。
- `schema-tool` 保留现有 recipe 名称，但实际对私有 `xtask` 执行 check、test 和 clippy，
  不再调用已迁移的 `kanban-schema-tool`。
- `schema-docs` 检查 spec bundle、marker、JSON fence 与 fixture 映射；它不把 prose 示例
  变成新的 contract。
- `schema-surface-audit` 的目标是对照真实 server route 与 CLI Clap leaf command；当前
  recipe 的历史 filter 仍待迁移，不能把“0 tests passed”当作 single-host surface 证明。
  MCP inventory 以实际 tool router 测试为准。
- `schema-adoption-witness` 先按 `(package, test_target)` 分组列出并执行 exact witness，
  再报告 producer/consumer；缺失、重复、ignored 或未执行均失败。
- `just schema-contract` 仍是现有 schema-contract composite gate，继续组合
  `just schema-dependency-isolation`、`just schema-fmt`、`just feature-p kanban-contract schema`、
  `just schema-tool`、`just schema-check`、`just schema-docs`、`just schema-surface-audit` 和
  `just schema-adoption-witness`；它没有被 `xtask` 替代，也不会被 `xtask` 反向调用。
- `schema-dependency-isolation`、`schema-surface-audit`、`schema-adoption-witness` 和
  `schema-contract` 仍包含旧 catalog/registry closure 的收口责任；在 legacy source 与
  artifact 被重新分类并由真实单 Host surface 接管前，它们不是完整功能完成证明，也不得
  为使其通过而伪造 witness 或重新接入 legacy/projection crate。
- `schema-audit-closed` 仅用于 source inventory 已清理且没有 `planned`/`generated`/未闭合
  endpoint obligation 的阶段性收口。本分支当前仍有 legacy artifact，不能据此宣称 closed。

所有会写 Cargo target 的命令必须经仓库 build lock 和上述 recipe；不要为 schema 文档任务
单独设置 target/cache，也不要运行与当前 contract 无关的 full/release/projection gate。

## 8. 新 operation 的最小闭环

新增 operation 时按以下顺序完成一个纵向 slice：

1. 在 `kanban-contract` 定义精确 DTO、schema root、inventory 和 endpoint/surface descriptor。
2. 添加 valid/invalid fixture，并为真实 producer 与 consumer 各提供独立 exact witness。
3. 在 `kanban-store-turso`、ApplicationService、server、`kanban-client` 和所需 adapter
   中接通同一 operation；adapter 不得直连 store。
4. 运行受影响 package tests、contract tests、`just schema-check`、
   `just schema-surface-audit`、`just schema-adoption-witness` 和 `just diff-check`。
5. 若 operation 实际被取消，状态改为 `excluded` 并写明理由；不得留下看似 adopted 的
   fixture 或 route。

这套闭环适用于完整单 Host 产品路径。labels、signals、ontology、search、graph、vector、
context、projection、运维、旧 SQLite importer 与 Desktop 历史视图按 parity ledger
分片恢复；每一片都必须同步 contract catalog 与真实 surface。旧 wire/CLI 兼容不是目标，
但不得借此删除业务能力或数据迁移语义。


---

# 文件：docs/ADR.md

# 架构决策记录

本文件按时间记录 SPEC 的关键架构决策。每条 ADR 的背景、统计值和迁移状态都是
决策时快照，不会随着实现自动改写；当前行为和实时契约覆盖以对应的
`docs/*_SPEC.md` 与 `docs/SCHEMA_CONTRACTS.md` 为准。

ADR-0001 和 ADR-0004 保留为历史记录，但已被当前的 single-host 决策
（ADR-0022）取代。它们中关于 CLI 直开数据库或旧文件名的内容不再描述当前产品。

---

## ADR-0001：仅使用 SQLite

### 状态

已被 ADR-0022 取代（历史决策）

### 后续决策

见 [ADR-0022：Turso single-host canonical application host](#adr-0022turso-single-host-canonical-application-host)。

### 背景

项目明确不考虑多用户、多租户、团队协作和远程 worker。核心运行环境是本地单机，同时需要 CLI 和 Web。

### 决策

只支持 SQLite。

默认数据库：

```text
~/.local/share/kb/kb.db
```

可通过 `--db <path>` 指定项目本地数据库。

### 影响

优点：

- 单一二进制文件易分发。
- CLI 使用成本低。
- 备份简单。
- 本地事务足够强。
- WAL 支持读写并发。

代价：

- 不支持跨机器共享写入。
- 不做 server 集群。
- 同一时刻只有一个写入者。
- 需要控制事务长度。

---

## ADR-0002：Status 枚举是事实，Column 是视图

### 状态

已接受

### 背景

传统的类 Trello 工具常把 list/column 视为状态。但本项目需要 dispatcher、claim、heartbeat、reclaim 和 run 历史。`running` 不是普通视觉列，而是 claim 成功后的执行状态。

### 决策

`tasks.status` 是权威事实。`board_columns` 只是界面展示映射。

### 影响

优点：

- Web、CLI、dispatcher 遵循同一状态机。
- 可保护 `ready -> running`。
- 能支持 review/scheduled/blocked 等非纯视觉状态。

代价：

- 拖拽列不能简单 PATCH status。
- Web 界面需要根据目标列调用状态转换端点。

---

## ADR-0003：快照 + 只追加事件，不做纯事件溯源

### 状态

已接受

### 背景

看板界面会高频查询当前任务列表。纯事件溯源会让当前状态查询复杂化，需要重放事件或额外投影。

### 决策

采用：

```text
tasks snapshot + task_events append-only
```

状态变化时，快照更新与事件插入必须在同一事务内完成。

### 影响

优点：

- 当前 board 查询简单。
- 事件仍可用于审计、SSE、调试。
- 实现复杂度可控。

代价：

- 需要保证快照/事件一致。
- 事件不是唯一事实源。

---

## ADR-0004：CLI 可以直接访问 SQLite，但必须走统一服务路径

### 状态

已被 ADR-0022 取代（历史决策）

### 后续决策

见 [ADR-0022：Turso single-host canonical application host](#adr-0022turso-single-host-canonical-application-host)。

### 背景

如果 CLI 必须依赖常驻 server，会降低本地工具可用性。直接访问 SQLite 更适合脚本和开发流程。

### 决策

CLI 可以直接打开 SQLite 数据库，但只能调用统一 Rust 服务路径；当前实现主要是
`kanban-sqlite::service` 用例函数，并复用 `kanban-core` 的纯状态机辅助函数。
CLI 不允许绕过状态机执行裸 SQL 修改状态。

### 影响

优点：

- 不需要 server 即可使用。
- 脚本友好。
- 和 Web 行为一致。

代价：

- 需要处理 CLI/server/dispatcher 同机并发。
- 所有状态逻辑必须集中在共享的 service/state-machine 路径，避免 CLI、server 或
  dispatcher 各自实现一套状态转换。

---

## ADR-0005：Actor 是审计字符串，不是用户模型

### 状态

已接受

### 背景

项目不做多用户和权限，但仍需要知道某个操作来自谁或哪个 worker。

### 决策

保留 `actor`、`created_by`、`claim_owner` 字段。它们是字符串，不关联用户表。

### 影响

优点：

- 保留审计能力。
- 支持 CLI、Web、dispatcher、worker profile 区分来源。
- 不引入 RBAC 复杂度。

代价：

- 不提供权限隔离。
- actor 可被本地调用者伪造，这是预期边界。

---

## ADR-0006：Worker stdout/stderr 存文件，数据库只存摘要与路径

### 状态

已接受

### 背景

运行日志可能很大。把日志数据放进 SQLite 会影响性能和备份体积。

### 决策

日志写入：

```text
~/.local/state/kb/logs/runs/<run_id>.log
```

数据库只存：

- `log_path`
- `summary`
- `error`
- `exit_code`

### 影响

优点：

- SQLite 保持轻量。
- 日志可直接用 `tail` 查看。
- 备份策略可分开处理数据库和日志。

代价：

- 移动数据库时需要同时移动日志/附件。
- 日志路径需要由 doctor 检查。

---

## ADR-0007：默认只监听 localhost

### 状态

已接受

### 背景

不做远程服务和多用户登录。暴露到局域网会制造安全边界问题。

### 决策

`kanban serve` 默认并且建议只监听：

```text
127.0.0.1:8721
```

MVP 不提供 `0.0.0.0` 远程模式。

### 影响

优点：

- 无需登录系统。
- 降低误暴露风险。

代价：

- 不能多人访问。
- 不能远程手机/浏览器访问。

---

## ADR-0008：状态变化必须使用专用转换命令

### 状态

已接受

### 背景

直接 PATCH `status` 容易绕过 claim/run/event/dependency 保护。

### 决策

禁止普通 update 修改 status。所有状态变化都使用专用命令：

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

### 影响

优点：

- 状态机可验证。
- run/claim/event 一致。
- Web/CLI/dispatcher 行为一致。

代价：

- API 数量更多。
- 界面拖拽逻辑更复杂。

---

## ADR-0009：Knowledge Substrate 派生层

### 状态

已被 ADR-0022 取代（projection lane 非 active）

### 背景

后续搜索、关系扩展、agent 上下文、artifact 来源和向量召回需要跨 task/run/comment/artifact/skill 的统一身份与派生索引，但不能削弱 SQLite 状态机、claim 和依赖保护。

### 决策

SQLite 继续作为运行事实源。新增：

- `entities`：跨库统一的 `kb://...` 身份注册表。
- `relation_predicates` / `entity_relations`：受控 predicate 与可重建关系镜像。
- `index_outbox`：派生存储的至少一次任务入口。
- `derived_store_state`：Tantivy/Oxigraph/LanceDB 等派生层健康和水位。

Tantivy、Oxigraph、LanceDB 都是可重建的派生存储，不参与状态机事务。

`derived_store_state` 的语义是存储全局状态，不是 board 局部状态：

- `last_event_id` 表示该存储已成功处理并提交的全局 task event 高水位。成功同步/重建只能把它单调推进，不能倒退。
- `dirty=true` 表示该存储仍有未完成 outbox、失败 outbox 或最近一次派生更新失败；即使某个 board 已完成同步/重建，其他 board 仍有待处理/失败任务时也必须保持 dirty。
- board 范围的同步/重建只清理当前 board 的 outbox 任务；是否把 `dirty` 置回 false，取决于同一存储目标是否还存在任何 board 的未完成 outbox。
- `last_error` 记录最近一次存储级失败证据。成功处理会清除 `last_error`，失败会保持 `dirty=true` 并保留/标记相关 outbox 失败状态。
- `index_outbox` 是恢复和重放入口；`derived_store_state` 是操作者使用的健康/水位摘要。两者都不能使派生层成为事实源。

### 影响

优点：

- 后续图/向量/context broker 可以接同一实体/关系契约。
- SQLite 状态机边界保持清楚。
- 派生存储损坏时可回退/重建。
- `kanban doctor` / maintenance API 汇总 outbox 积压、脏存储、last_error 和失败 outbox，供本地操作者判断是否同步/重建，而不是让派生层参与 SQLite 事务。

代价：

- 需要维护实体回填/outbox/派生状态。
- `derived_store_state` 是派生存储的主健康/水位记录；Tantivy 的旧 `app_settings` 搜索状态仅保留为兼容元数据。

---

## ADR-0010：单数据库多 board 与 CLI task 引用

### 状态

已接受

### 背景

本地项目需要不同 board/project，但未来也需要聚合视图和跨 board 审计。如果每个项目拆一个 SQLite 数据库，聚合、搜索、事件和 dispatcher 恢复都会变复杂。另一方面，裸 `#12` 在 shell 中容易被当作注释，且 board 内的 seq 不能跨 board 唯一。

### 决策

继续使用单个 SQLite 数据库内的多个 board：

- `tasks.id` 是全局唯一 `t_...`。
- `tasks.seq` 只在 `board_id` 内唯一。
- CLI/API 展示可复制的 task 引用：`board_slug#seq`。
- CLI task 引用支持全局 `t_...`、当前 board 的 `12` / `#12`、显式 `board#12` / `board/#12` / `b_...#12`。
- 当前 board 的解析顺序是 `--board`、`KB_BOARD`、最近的 `.kb/config.toml`、`default`。
- `.kb/config.toml` 只记录当前项目选择的 board，不表示项目拥有独立数据库。
- Board slug 禁用保留 ID 前缀和会破坏引用语法的字符。

已归档 board 默认不可写；归档只标记 board，不改 task 状态，并拒绝仍有 `running` task/run 的 board。只读 events/runs/comments 历史保留可查，作为审计入口。

### 影响

优点：

- 保留未来聚合 board / 仪表盘的数据基础。
- `t_...` 可作为脚本稳定全局引用。
- `board#seq` 对人和 shell 都更可复制。
- 项目级当前 board 不破坏单数据库备份、搜索和 dispatcher 语义。

代价：

- CLI 必须维护 task 引用的解析/解析目标逻辑。
- 已归档 board 需要区分只读历史与变更保护。
- 裸 `#12` 只能作为兼容输入，文档和输出不能依赖它。

---

## ADR-0011：Schema 批次边界：status、type、labels、dependency type 与 decision comments

### 状态

提议中

### 背景

`kanban-tool` 接下来会进入一组 schema/model 扩展：

- `task_type`：表达任务是什么类型。
- `dependency_type`：表达任务之间是什么关系。
- labels：表达可搜索、可筛选、可推荐的多维标签。
- comments：承载人和 agent 的协作记录。
- decision comment：记录人或 LLM/agent 在多个方案之间做出的选择。

当前 comment 模型里的 `kind` 混用了两类概念：

- 谁写的：system / worker / agent / user。
- 写的是什么：普通记录 / 决策记录。

这会让后续结构化 decision comment 变得混乱。需要先把模型边界切开：

- 作者/来源轴：谁留下了这条 comment。
- 内容类型轴：这条 comment 表达什么语义。

项目早期只面向本地单用户场景，不需要为早期评论结构保留沉重兼容层。可以直接修改模型，
只要迁移清晰，并让 CLI、API 与 Desktop 同步跟上。

### 决策

保留现有核心原则：

- `tasks.status` 继续是唯一的权威工作流状态。
- hard dependency 继续是状态机和 dispatcher 保护的事实来源。
- `task_events` 继续是只追加审计轨迹。
- comments 继续承载协作记录，但 comment schema 要拆清楚作者和内容语义。
- 新字段默认不改变状态机、dispatcher claim 或 ready 资格，除非本 ADR 明确允许。

### 字段职责

| 字段 / 模型 | 责任 | 是否影响状态机 | 是否影响 dispatcher | 是否影响依赖/搜索/上下文展示 | 是否用于搜索/上下文/界面 |
|---|---|---:|---:|---:|---:|
| `status` | 权威工作流状态 | 是 | 是 | 是 | 是 |
| `priority` | ready/dispatcher 的排序权重 | 否 | 是，排序 | 是，列表和推荐排序 | 是 |
| `scheduled_at` | 计划时间，参与 scheduled/ready guard | 是 | 是 | 是，列表和上下文排序 | 是 |
| `due_at` | 截止时间，只展示、筛选、排序 | 否 | 可排序 | 可排序 | 是 |
| `task_type` | 任务类别，例如 bug/feature/research/ops/follow_up | 否 | 否 | 可用于展示/排序，不改变执行资格 | 是 |
| labels | 多标签分类、搜索、推荐和界面分组 | 否 | 否 | 否，除非未来显式配置排序策略 | 是 |
| `dependency_type` | 依赖边语义，区分硬阻塞和软关系 | 仅硬阻塞 | 仅硬阻塞 | 是，但必须区分硬/软关系 | 是 |
| `comment.author_type` | 评论作者角色：`user` 或 `agent` | 否 | 否 | 否 | 是 |
| `comment.author` | 展示名，例如 `alice`、`codex` | 否 | 否 | 否 | 是 |
| `comment.agent_type` | 可选 agent 细分，例如 `codex`、`executor`、`dispatcher` | 否 | 否 | 否 | 是 |
| `comment.kind` | 内容语义：`note` 或 `decision` | 否 | 否 | 否 | 是 |
| `comment.metadata_json` | `comment.kind` 对应的结构化 payload | 否 | 否 | 否 | 是 |
| `event.kind` | 只追加审计事件类型 | 否，event 是结果不是输入 | 否 | 否 | 是 |

### 工作流状态

`status` 仍然是任务是否可执行、是否被 claim、是否 blocked/review/done 的唯一事实来源。

任何新字段都不能隐式表达状态：

- `task_type=bug` 不表示高优先级。
- label `blocked` 不表示 task 处于 blocked。
- decision 的选中项不表示 task 处于 done。
- comment 中写 “blocked” 不改变 task status。

状态变化只能通过状态转换命令完成。

### 任务类型

`task_type` 表达“这个 task 是什么工作类别”，不表达“它现在处于什么执行状态”。

建议第一批 task 类型：

```text
bug | feature | research | ops | docs | refactor | test | follow_up
```

`task_type` 可以用于：

- Desktop/List/Board 筛选。
- 搜索/上下文过滤。
- 依赖、搜索和上下文解释。
- 未来排序加权。

`task_type` 不用于：

- dispatcher 领取资格。
- 状态机转换保护。
- 硬依赖判断。
- 替代 labels。

枚举策略：

- 第一版使用受控枚举。
- 后续如需要开放扩展，再单独做 ADR。
- 未知类型应被拒绝，而不是静默写入。

### 标签

labels 表达多维、可叠加的分类。一个 task 可以有多个 label。

labels 适合表达：

- 区域：`desktop`、`cli`、`sqlite`
- 领域：`search`、`dispatcher`、`comments`
- 语义组：`llm-facing`、`release-risk`
- 用户临时整理方式

labels 不适合表达：

- 工作流状态
- 硬依赖
- 执行所有权
- 决策结果

未来的语义 label 推荐器可以推荐 label，但推荐结果必须显式保存后才成为 task label。

### 依赖类型

现有 dependency 的核心语义是硬前置条件：

```text
parent done or archived => child may become ready
parent neither done nor archived => child cannot be ready/running
```

引入 `dependency_type` 后，必须保留 hard dependency 的清晰语义。

建议第一批 dependency 类型：

| 类型 | 语义 | 是否阻塞子任务 |
|---|---|---:|
| `blocks` | 父任务是子任务的硬前置条件 | 是 |
| `relates_to` | 相关任务，仅用于导航/search/context | 否 |
| `informs` | 父任务提供背景、设计输入或决策依据 | 否 |
| `spawned_from` | 子任务在父任务执行过程中被发现 | 否 |
| `duplicates` | 重复或替代关系 | 否 |

只有 `blocks` 参与：

- 依赖阻塞判断
- promote 保护
- claim 保护
- dispatcher 执行资格
- 硬依赖阻塞

软依赖可以进入 Desktop 展示、搜索和上下文，但不能让任务变成 blocked，也不能阻止 claim。

### 评论作者模型

comment 的作者模型只表达“谁写的”。

本项目面向本地单用户场景，不建立用户系统。作者角色只保留两类：

```text
user | agent
```

规则：

- `user`：本地操作者写入的内容。
- `agent`：自动化主体写入的内容。
- `author`：展示名，例如 `alice`、`codex`。
- `agent_type`：仅当 `author_type=agent` 时可用，例如 `codex`、`executor`、`reviewer`、`dispatcher`。
- 不引入 users table、identity table、RBAC 或权限模型。

这意味着不再使用 comment kind 表示 `system`、`worker` 或 `agent`。这些都属于作者/来源轴。

### 评论类型模型

`comment.kind` 只表达“这条 comment 的内容语义”。

第一版只保留两类：

```text
note | decision
```

后续 Generic Signal Ledger 决策把 `signal` 加入该集合，用于指向通用 signal ledger；
当前完整约束以 `docs/DATA_MODEL.md` 和 `docs/API_SPEC.md` 为准。

#### `note`

普通协作记录。包括：

- 进展说明
- 交接记录
- 执行总结
- 问题描述
- 审查者反馈
- 验证记录
- 人或 agent 的普通回复

“遇到的问题”默认也是 `note`。如果问题真的阻塞任务，应该同时通过状态转换命令把 task 变成 `blocked`，并写入 `status_reason`。

#### `decision`

结构化选择记录。用于表达：

- 有多个选项。
- 最终选择了其中一个。
- 有选择理由、风险和验证方式。

decision 不是 task status，不是 event，也不是 ADR 的替代品。

### 评论元数据

`comment.metadata_json` 是 `comment.kind` 的结构化 payload。

规则：

- `kind=note` 时，metadata 默认 `{}`。
- `kind=decision` 时，metadata 必须符合 decision schema。
- metadata 非法 JSON 或 schema 不匹配时拒绝写入。
- metadata 不参与状态机。
- metadata 不替代 event。
- metadata 不应该变成随意塞字段的长期垃圾桶。

### 决策评论 Schema

建议第一版结构：

<!-- schema-doc-ignore: 说明性或不完整 payload；已提交的 schema fixture 仍是可执行权威 -->
```json
{
  "options": [
    {
      "slug": "comment-metadata",
      "title": "使用评论元数据",
      "detail": "把结构化决策数据保存在 task_comments.metadata_json 中。"
    },
    {
      "slug": "decision-table",
      "title": "创建决策表",
      "detail": "创建独立的 task_decisions 表并保存各个选项。"
    }
  ],
  "selected": "comment-metadata",
  "reason": "让决策紧邻任务讨论，避免产生平行时间线。",
  "risk": "metadata schema 需要严格验证。",
  "verification": "CLI/API/Desktop 测试覆盖创建、读取、渲染和非法元数据拒绝。"
}
```

验证规则：

- `options` 必须非空。
- 每个 option 必须是 object，且有非空字符串 `slug`、`title`、`detail`。
- 每个 option 必须有唯一 `slug`。
- `selected` 必须匹配某个 option slug。
- `reason` 必填且非空。
- `risk` 可选但推荐；如果出现，必须是非空字符串。
- `verification` 可选但推荐；如果出现，必须是非空字符串。
- `slug` 使用稳定小写 ASCII slug，必须以小写字母或数字开头，只包含小写字母、数字和 `-`，便于 CLI、JSON 和前端引用。
- `detail` 可以是 Markdown 文本，但 Desktop 渲染必须遵守安全 Markdown 规则。

### Desktop 渲染规则

Desktop TaskDetail 评论列表：

- `note`：按普通 Markdown 评论渲染。
- `decision`：
  - 展示 comment body 作为自然语言摘要，例如“已决定继续使用 comment metadata 承载结构化决策信息，正文保留为简短结论，方便没有结构化渲染的环境阅读。”
  - 展示所有选项 slug。
  - 选中项使用明确的绿色/selected 状态。
  - 点击选项展开 `title` 和 `detail`。
  - 展示 reason、risk、verification。
  - 如果 decision metadata 无效，不应该静默当作 selected；应显示错误状态或降级 note。

### CLI / API 规则

CLI：

```bash
kanban comment add <task-ref> "<body>"
kanban comment add <task-ref> "<body>" --kind note
kanban comment add <task-ref> "<body>" --kind decision --metadata-json '<json>'
```

`kind=decision` 的 body 是自然语言回退摘要，不重复完整选项表；`options`、`selected`、`reason`、`risk` 和 `verification` 只放在 `metadata_json` 中，由 Desktop 在正文下方结构化渲染。

可后续增加更友好的命令：

```bash
kanban decision add <task-ref> ...
```

但第一版不要求。

API：

- comment 创建请求显式包含：
  - `body`
  - `author_type`
  - `author`
  - `agent_type`
  - `kind`
  - `metadata`
- comment 响应返回同样字段。
- 不再把 `system/worker` 作为 kind 返回。

### 事件类型

`event.kind` 只记录系统事实：

- `comment.added`
- `task.created`
- `task.updated`
- `task.claimed`
- `task.completed`
- `dependency.added`

event 不承载 decision 本体。添加 decision comment 时，event 只记录 `comment.added`，decision 内容在 comment 快照中。

### Dispatcher 与候选集规则

Dispatcher 领取资格只能看：

- `status`
- 硬依赖（`dependency_type=blocks`）
- `scheduled_at`
- claim token / 租约
- board 归档状态
- 负责人 / worker profile

Dispatcher 排序可以看：

- `priority`
- `created_at`
- 未来显式的 dispatcher 策略

Dispatcher 不看：

- `task_type`
- labels
- `comment.kind`
- `comment.metadata_json`
- decision 选中项
- 软依赖

候选集可以展示和解释更多字段，但不得把软字段解释成硬阻塞条件。

### 迁移策略

项目当时仍处于本地单用户的早期版本，不做沉重兼容层。采用直接结构迁移批次：

1. 本 ADR 固定边界。
2. 修改 `task_comments`：
   - 增加 `author_type`
   - 保留/明确 `author`
   - 增加 `agent_type`
   - 收窄 `kind` 为 `note | decision`
   - 增加 `metadata_json`
3. 更新 Rust domain/API/CLI/Desktop 类型。
4. 迁移现有 comment：
   - 不是用户本人写的，一律 `author_type=agent`
   - 用户本人写的，`author_type=user`
   - 普通历史 comment 一律 `kind=note`
   - 已有 decision comment 若能识别则 `kind=decision`，否则 `note`
5. 实现 decision metadata 验证。
6. 实现 Desktop decision 渲染。
7. 后续再做 `task_type`、`dependency_type`、labels 扩展。

### 影响

优点：

- comment 模型语义清楚：作者归作者，内容类型归内容类型。
- decision comment 可以成为真正结构化对象。
- Desktop 渲染会简单很多。
- LLM/agent 做选择时可以留下可索引、可展开、可复盘的记录。
- 不再让 `system/worker` 这类来源概念污染内容类型。

代价：

- 需要 schema migration。
- 需要一次性更新 CLI/API/Desktop。
- 旧 comment JSON shape 会改变。
- 需要认真做 decision metadata 验证，避免 `metadata_json` 变成任意垃圾桶。
- 全局 `kanban-tool` skill 需要同步，因为 CLI/API/comment JSON 行为会变化。

### 非目标

- 不引入多用户系统。
- 不引入 RBAC、团队、组织、邀请或云同步。
- 不用 decision comment 替代 ADR。
- 不让 comment metadata 影响 dispatcher claim。
- 不让 labels/type/metadata 变成隐式 status。
- 不把 `task_dependencies` 改成完整知识图谱。
- 不在本 ADR 中实现具体 migration。

---

## ADR-0012：Label Proposal Provider 边界

### 状态

已接受

### 背景

语义 label 建议的日常路径应保持确定性：SQLite 保存权威的
`labels` / `task_labels` / `label_semantics` / `label_atoms`，LanceDB 只是
`kb_label_atoms` 派生索引，求解器只做本地向量计算。Label proposal 是“覆盖不足时建议新 label
semantics”的可选流程，它可以由人工、离线工具或未来本地 LLM provider 产生候选项。

真实 LLM provider 如果直接放进 `kanban-sqlite`，会把外部 SDK、HTTP client、prompt、
credential 和 runtime 配置拖入 SQLite service。这样会破坏本项目的本地优先 / 仅 SQLite
边界，也会让 proposal 验证与模型调用耦合过深。

### 决策

`kanban-sqlite` 只定义并消费 `LabelProposalProvider` trait：

- `DisabledLabelProposalProvider`：默认 provider 不可用，返回降级尝试，不写入权威 label。
- `ManualLabelProposalProvider`：接收 CLI/API 显式传入的本地/离线候选项。
- `propose_task_label_with_store`：从 SQLite 读取 task 和建议上下文，调用 provider，
  然后执行确定性验证、残差 top1+margin 门禁、proposal 持久化和
  accept/reject 生命周期。

真实 LLM provider 不属于 `kanban-sqlite`。可选实现位置是：

- `kanban-server`：当 localhost server 显式配置本地 provider/runtime 时注入 trait object。
- `kanban-cli` 或本地 runtime：当命令显式读取本地/离线候选项或未来本机模型输出时注入。
- 独立 `kanban-ai` / `kanban-llm` crate：承载 SDK、HTTP client、prompt 和 credential 读取，
  再向上层暴露实现 `LabelProposalProvider` 的适配器。

### 影响

优点：

- SQLite service 不依赖 LLM SDK、HTTP AI client、runtime credential 或外部模型配置。
- proposal 生命周期仍由确定性的 SQLite service 守住，不会因为 provider 类型不同而绕过
  残差验证或 accept/reject 门禁。
- 日常 `label suggest` 不依赖 proposal provider；provider 不可用只会产生降级的 proposal 尝试。
- 未来 provider 可以替换或禁用，不需要修改权威 label 事实或 task label 绑定语义。

代价：

- 真实 provider 需要在上层做适配器和配置装配。
- server/CLI 需要明确区分“候选项生成失败”和“SQLite 验证拒绝候选项”。
- 需要持续避免把 prompt、credential、HTTP 重试等关注点下沉进 `kanban-sqlite`。

### 非目标

- 本 ADR 不实现真实 LLM provider。
- 不上传本地 task 数据到远程服务。
- 不让 provider 自动绑定 task label。
- 不改变 proposal accept 后才创建 label semantics / atoms 的生命周期。

---

## ADR-0013：暂不引入 label ontology 图投影

### 状态

已接受

### 背景

当前 label ontology 已有 SQLite 权威事实与查询面：

- `labels` / `task_labels` 表达当前 task label 绑定事实。
- `label_semantics` 表达权威 ontology semantics；`label_atoms` 是从 semantics 与
  label name 展开的 SQLite 物化投影。
- `label_semantic_proposals` 表达新 label proposal lifecycle。
- `label_ontology_observations` / `signals` / `actions` / `action_signals` 表达
  来源、审查、变更和验证历史。
- `label ontology review`、`label atom explain`、JSONL export/import 和 doctor 已经从
  SQLite records 直接回答第一批 review/provenance 问题。

项目也已有通用 Knowledge Substrate 图：`entity_relations` 作为 SQLite 镜像，
Oxigraph 作为可重建派生存储，`index_outbox` / `derived_store_state` 管理 dirty、
同步和重建。这个图当前覆盖 task-board、task dependency 等通用实体
关系，不覆盖 label ontology 账本。

第一版账本还没有明确的关系查询需求需要 ontology 专属图。过早投影 signal、
action、atom 和 proposal 会增加 schema、outbox、query API 和重建复杂度，并提高把
图误当第二事实源的风险。

### 决策

暂不实现 label ontology 图投影。

在 rename/split/merge、跨 action 来源、atom 谱系或审查工作台出现明确
关系查询需求前，ontology 查询继续走 SQLite service/API：

- `label ontology review`
- `label ontology show`
- `label atom explain`
- `label proposal list/show`
- JSONL export/import 与 doctor

未来若新增 ontology graph projection，它必须满足：

- SQLite `labels`、`task_labels`、`label_semantics`、`label_atoms`、proposal 和
  `label_ontology_*` 仍是事实来源；`label_atoms` 是 projection，不是独立 semantic truth。
- 投影只能从 SQLite 快照/outbox 派生，可删除重建。
- 投影状态通过 `index_outbox` 和 `derived_store_state` 或等价派生层控制面表达。
- graph API 只能查询关系/来源，不提供 confirm/apply/validate/revert/bootstrap
  或其它权威变更写入口。
- 图 dirty、error、删除或重建失败不改变 task status、task labels、semantics、atoms、
  proposal 或账本记录。

### 影响

优点：

- 第一版 ontology workflow 保持简单，避免过早增加第二个 provenance 表达。
- SQLite ledger/review/explain 继续作为可审计事实来源。
- 未来如果确有查询需求，可以复用已存在的 Knowledge Substrate derived-store contract。
- graph 故障不会影响 ontology mutation、validation 或 review 的 canonical state。

代价：

- 复杂 lineage / relationship traversal 暂时需要通过 SQLite query、review grouping、
  `atom explain` 或导出后离线分析完成。
- 未来若要支持 ontology graph，需要单独设计 projection schema、outbox fanout 和 rebuild
  测试。

### 非目标

- 本 ADR 不新增 ontology RDF schema。
- 不把 `label_ontology_*` rows 写入 `entity_relations`。
- 不扩展 `kanban graph` 为 ontology mutation API。
- 不用 graph 替代 label ontology review、show、atom explain 或 validation history。

---

## ADR-0014：标签本体收口契约

### 状态

已接受

### 背景

标签身份增删改查、任务标签绑定、语义变更、提案接受、引导初始化、验证与审查生命周期
曾经混用来源语义。最危险的问题是普通任务采集可以隐式创建词汇，删除标签身份可以隐式
删除语义与 atom，语义变更会拆成多条逐 atom 操作，受信验证的原始 JSON 可能绕过采集器，
引导验证也曾依赖提交后的补偿操作。

### 决策

采用收窄后的 closure contract：

- `labels` identity CRUD 是基础 vocabulary registry，不写 ontology mutation action；task
  label binding 只绑定已存在 label，写普通 task event。
- `label delete` 永不隐式删除 `label_semantics` / `label_atoms`；force 只允许移除 task
  bindings 后删除空 identity。
- `label_semantics` / `label_atoms` canonical mutation 一次 transaction 只写一条 root
  mutation action；实际 atom delta 写入 `label_ontology_action_atom_effects` 的
  `added` / `removed` rows。No-op 不写 action/effects，也不标脏 index。
- Semantics clear 继续使用 `update_semantics` action type，必须有 actor、非空 reason 和
  `expected_semantics_hash`。
- Atom explain 优先读取 effect rows；legacy per-atom actions 只做兼容读取，不回写压缩历史。
- Trusted automated validation 只能由 CLI collector 生成，表示 current hash/index
  generation 和指定 cases/controls 机械通过，不表示全局语义正确。
- CLI bootstrap verify 是 pre-commit staged verification；失败、provider unavailable 或
  verify/commit 间 state 变化时零 canonical 写入。
- `validation_requirement` 与 validation attempt outcome 分离；effective outcome 是查询
  reducer 结果。Unsupported parent 可记录 external failed/partial 诊断，但不能 passed。
- Public structure plan write 入口关闭；rename/split/merge 暂仅可作为 review signal 或
  legacy action 读取。

### 影响

优点：

- Routine task capture 不能再绕过 vocabulary adoption。
- Ledger 行数随真实 mutation 数线性增长，atom explain 粒度来自 effect rows。
- Destructive semantics clear 有 CAS、reason 和 revertable root action。
- Trusted/external validation 边界由 Rust visibility、collector entry 和 tests 锁住。

代价：

- 旧 per-atom action 保留历史噪声，需要 explain/revert 的 legacy compatibility。
- Base label identity delete 需要用户先显式 clear semantics。
- Structure mutation 需要未来单独 typed apply、binding migration 和 validation policy。

### 非目标

- 不新增 action type、signal type、validation status 或 graph/dashboard projection。
- 不回写或压缩历史 per-atom actions。
- 不实现 rename/split/merge canonical mutation。

## ADR-0015：通用信号账本

### 状态

已被 ADR-0022 取代（signal surface 非 active）

### 背景

Agent/Product 的故障和观察需要一个持久的审查生命周期；它不能局限于 label，也不能只是自由格式的评论元数据。

### 决策

新增 board 范围的 `signal_observations` 和 `signals` 表。`kanban signal record` 写入 observation 与 signal 记录；存在 task 上下文时，还会在同一 SQLite 事务中写入简短的 `task_comments.kind = signal` 回链。生命周期审查支持 `open -> confirmed|rejected|superseded|resolved` 和 `confirmed -> resolved`；supersede 要求替代 signal 与原 signal 属于同一 board，并防止成环。V1 不会自动创建后续任务。

### 影响

Signal 账本成为通用 agent/product 信号的权威存储。Label ontology 账本仍然只服务于 label，不复用于通用产品信号。


## ADR-0016：API/SSE 传输描述符作为唯一 method/path 权威

### 状态

已被 ADR-0022 取代

当前 Axum router 直接注册 active single-host paths；descriptor 是待清理的机器契约来源，
不是运行时 route factory。

### 背景

此前 `SurfaceOperation` 与 router 各自手写 method/path；即使已有一致性测试，仍存在双写漂移面。

### 决策

在 `kanban-contract` 默认 feature 中保存 API/SSE 描述符；server router 以
`operation_id` + 显式 `adapter_id` 绑定真实 handler，并读取描述符的 method/path。

### 影响

`SurfaceOperation` 的 API/SSE 记录改为投影；CLI/JSONL 保持独立清单。schema root 使用
`contract_id`，不与端点 `operation_id` 混淆。DTO/schema 采用不在本决策中提前完成。

## ADR-0017：B1-A 错误与删除响应的 wire 收口边界

### 状态

已接受

### 背景

稳定错误码与固定删除确认响应已具备可验证的 wire 形状；把任意
`String`/`Value` 留在公开边界会削弱 schema、类型化 consumer 与漂移门禁。

### 决策

`ErrorBody.code` 使用闭合的 `ApiErrorCode`，server 适配器显式将 `KanbanError` 映射为
枚举；label semantics 删除 handler 使用 `DeleteResponse`/`DeleteResult`，不再公开
`DataEnvelope<serde_json::Value>`。

### 影响

该决策只拥有 wire/schema 证据。HTTP status、locale 消息、service 保护、状态机、
CAS、事务与 SQLite 继续由 adapter/service/core 负责。决策时删除端点的
其它义务尚未建模；其后续当前状态以 `docs/SCHEMA_CONTRACTS.md` 为准。

## ADR-0018：B1-C0 传输位置、基数与精确/共享绑定

### 状态

已接受

### 背景

仅有 contract ID 与 input/output 方向无法区分 path/query/header/body/2xx success/shared
error/SSE，也无法证明 query 重复值顺序、path placeholder 映射或共享错误 envelope 的
真实复用关系。

### 决策

API/SSE 语义 contract 显式声明 `Http { operation_key, location, parameters }`，非 HTTP
contract 显式声明 `NoTransport`；参数基数只允许
`RequiredOne|OptionalOne|RepeatedOrdered`。`Success` 只表示 2xx success；非 2xx `Error`
只允许用于 `SharedComponent`。任意 `Adopted` contract 和端点精确引用都必须是
`granularity=Exact`。

端点精确绑定不维护全局第二绑定映射。method/path 唯一、精确
`operation_key` 和单一 location 共同推出合法绑定唯一；公开面目录中的重复精确
引用仍单独失败关闭。

`SharedComponent` 可以跨多个端点复用且不计入精确/采用覆盖。
generated/adopted shared 必须至少有显式链接，或同一公开面的真实采用 witness。

### 影响

验证器对未知/`Planned`/`Excluded` 引用，以及错误的
binding/granularity/location/direction/operation/surface 失败关闭。本 ADR 保存的是
迁移当时的边界和冻结值，不代表当前覆盖；实时状态见 `docs/SCHEMA_CONTRACTS.md`。

## ADR-0019：B1-C1 Task-read 精确 path/query 契约与单一有序解析器

### 状态

已接受

### 背景

两个 task-read 端点需要证明各自精确消费 path/query，同时避免 handler 或多个 parser
重复拥有 raw query。

### 决策

`GET /api/v1/boards/:board/tasks` 与 `/tasks/by-status` 分别拥有独立 path/query DTO，
  形成 4 个 `Adopted` 精确 contract。两个 server 本地类型化 Axum extractor 分别绑定对应
  `Path<...>`，并各自从 `parts.uri.query()` 读取一次 raw URI 后进入共享有序解析器；handler
  只接收已绑定的 request，不持有 `RawQuery`、`Query<T>` 或第二个 raw source。
- Query 语法：只有 `status`、`priority`、`label`、`plan_filter` 是
  `RepeatedOrdered`；其余标量重复、未知 key 与旧 `search` 别名均返回
  `400 invalid_input`。54 对上限由 9/4/3/32 个重复参数预算加 6 个标量参数推导；
  raw query 上限为 8192 字节。`q` 是唯一文本搜索 key。label 会 trim Unicode 边缘空白，
  但纯 Unicode 空白失败关闭；percent/UTF-8、枚举、priority、limit、offset 和 sort 边界由
  真实 router URI 矩阵固定。
每个 contract 都有独立的 DTO-to-fixture producer 和 fixture-to-real-router consumer；
  非默认 board 哨兵证明真实 path 消费。AST 测试锁定 DTO 所有权、类型化
  extractor、两个 raw URI 消费点及 handler `&path.board` 到 `list_tasks_page` 的实参，并以显式
  变异覆盖别名、私有 DTO、错误 extractor、双重来源、第二个 raw parser，以及两个
  handler 各自的 `path.board -> default`。producer/consumer 区域保护只证明当前源码区域直接
  分离，不把任意未来共同 helper 间接层夸大为变异完备证明。

### 影响

Desktop/Web/CLI 的 HTTP 调用方必须使用上述语法；现有 Desktop 调用方已使用 `q`
  并保留重复参数顺序。SQLite service 的防御性上限直接引用唯一 application 权威，
  server 相等性门禁覆盖该实际 service 路径；service 查询行为与 core 状态机不变。本文保留
  决策时的迁移边界；当前采用状态以 `docs/SCHEMA_CONTRACTS.md` 为准。

## ADR-0020：B1-C2b task-read 成功响应决策

### 状态

已接受

### 背景

共享响应 envelope 会掩盖两个 task-read 端点的精确响应差异。

### 决策

让两个 task-read 端点分别拥有闭合响应 contract，只复用 `ApiTask`、`ApiLabel` 与既有
pagination primitives。

### 影响

行为细节以 [API_SPEC](docs/API_SPEC.md#4-任务) 和
[SCHEMA_CONTRACTS](docs/SCHEMA_CONTRACTS.md#2-契约状态) 为准。

## ADR-0021：Oxigraph quick-xml 安全临时 vendor patch

### 状态

已被 ADR-0022 取代

该例外只适用于已经退出 active workspace 的 Oxigraph projection lane。当前根
`Cargo.toml` 不再包含该 `[patch.crates-io]`；以下内容保留为历史决策记录，不描述当前
产品依赖图。

### 背景

`oxrdfxml 0.2.3` 与 `sparesults 0.3.3` 的 crates.io 版本仍解析到受
RUSTSEC-2026-0194/RUSTSEC-2026-0195 影响的 `quick-xml < 0.41`；仓库当前通过 root
`Cargo.toml` 与对应的 `vendor/` 目录使用上游修复源码，并统一到 `quick-xml 0.41.0`。

### 决策

允许根目录 `Cargo.toml` 中唯一的 `[patch.crates-io]` 例外，且仅接受 `oxrdfxml`/`sparesults` 两个精确仓内 vendor 路径、package name/version 与普通文件目标。`schema_dependency_policy` 对额外 key、非精确 source/path、path traversal、symlink、全部 `[replace]` 保持失败关闭；schema-tool 注册表闭包不变，产品依赖图继续禁止 schema tooling 泄漏。

由安全负责人维护，待 crates.io 上游版本发布并确认 `quick-xml >= 0.41` 后移除 vendor、`[patch]`、lockfile 变更及本 ADR；advisory、provenance 或 vendor digest 变化必须重新审查。复核期限：2026-10-12。

---

## ADR-0022：Turso single-host canonical application host

### 状态

已接受（single-host ownership 仍为当前架构；初始功能收敛条款已由完整功能迁移计划接续）

### 后续决策

本 ADR 继续约束唯一 host、typed localhost client、共享 mutation path、Turso ownership 和
dispatcher 边界。第 6 条以及“非目标”中的 labels、signals、search、graph、vector、projection
和 importer，是当时为先完成 host 收敛而设的阶段性后置项，不是删除用户能力或永久不支持的
产品决定。当前锁定的完整功能 Turso 重构计划要求逐项恢复这些能力、旧数据迁移和 Desktop
历史 surface；具体 owner、入口、迁移规则和验收测试以
[`docs/migration/turso-full-feature-parity.md`](docs/migration/turso-full-feature-parity.md)、
[`ARCHITECTURE.md`](docs/ARCHITECTURE.md) 和 [`DATA_MODEL.md`](docs/DATA_MODEL.md) 为准。

在过渡期，`kanban-application` + `kanban-store-turso` 仍然是当前实现；目标是合并为
`kanban-service`，并将 `kanban-contract` 收敛为 `kanban-protocol`。这些名称变化不会改变
single-host ownership，也不能用“暂不支持”替代 parity ledger 的闭合。

### 背景

CLI、MCP 和 Desktop 曾分别持有 storage/runtime 入口。即使它们读写同一份数据库文件，
也可能各自解释 task transition、claim、comment 和 event，造成语义漂移。Turso 默认还要求
同一本地数据库文件由一个 OS 进程 owner 打开；继续维护多进程直连、runtime framing 或兼容
fallback 会把数据库 ownership 问题扩展成另一套产品协议。

### 决策

1. `kanban serve` 是唯一 application host，也是唯一可以打开、初始化和关闭 Turso 数据库的
   进程。默认路径是 `~/.local/share/kb/kanban.db`，默认监听 `127.0.0.1:8721`。
2. CLI、MCP、Desktop 统一通过 typed localhost HTTP client 调用 host。server 不可用时返回
   `server_unavailable`；不允许“有 server 走 HTTP、没 server 直开数据库”的双路径。
3. 所有 mutation 进入同一个 `ApplicationService`，由同一状态机、事务、board isolation、
   CAS claim、owner/token 校验和 error contract 保护。adapter 只负责解析、调用和展示。
4. 使用 `turso = 0.7.2`、`default-features = false`；不启用 `multiprocess_wal`。同一 host 进程
   内按 operation 获取 connection，数据库文件不由其他入口或其他进程直接打开。
5. dispatcher 只作为 `kanban serve --dispatcher-profile <path>` 的同进程 opt-in 单 worker
   loop，复用 application commands；默认不自动消费队列。
6. 不建设或保留自定义 framed IPC、named pipe、runtime protocol、capability negotiation、
   generalized mutation receipt、projection control plane 或旧 API 兼容层。未迁移的 labels、
   signals、search、graph、vector、projection 和 importer 留给独立后续工作。

> 历史范围说明：这里的“未迁移”描述 ADR-0022 作出时的阶段，不代表完整功能重构可以删去
> 这些能力；后续迁移仍必须复用本 ADR 定义的 single-host application path。

### 影响

优点：

- 三个入口共享同一条可验证的 command/query path，业务错误不会在 adapter 间分叉。
- 单一 DB owner 简化 Turso 生命周期、重启恢复和并发边界；HTTP client 保持 adapter 薄。
- 每个 operation 可以独立完成 store → application → HTTP → client → adapter 的纵向切片。

代价：

- 使用 CLI、MCP 或 Desktop 前必须先运行 `kanban serve`。
- host 是本机单用户服务，不提供离线直连、多进程数据库访问或公网 API。
- 未迁移能力会显式返回 `feature_not_available`，不以兼容 shim 掩盖未完成迁移。

### 非目标

本决策不定义 SQLite importer、自动 server supervision、跨机器 worker、备份/恢复产品、
projection rebuild 或未来 backend。它只收敛当前 canonical application path；任何新增能力
必须先证明不会引入第二条 mutation path。完整功能迁移计划已经将 SQLite v30 import、
projection rebuild、FTS/vector/graph/search、labels/ontology/signals、backup/maintenance
和 Desktop 历史 surface 纳入后续实现；这些能力的完成状态由 parity ledger 和对应验收测试
判断，而不是由本 ADR 的历史“非目标”段落推断。


---

# 文件：crates/kanban-store-turso/src/schema.rs

```rust
pub(crate) const CANONICAL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL DEFAULT '',
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
  idempotency_key TEXT,
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  description TEXT,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  status_reason TEXT,
  assignee TEXT,
  priority INTEGER NOT NULL DEFAULT 3 CHECK(priority BETWEEN 0 AND 3),
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
  UNIQUE(board_id, id),
  UNIQUE(id, board_id),
  UNIQUE(board_id, seq),
  CHECK(
    (status != 'running') OR
    (claim_token IS NOT NULL AND claim_owner IS NOT NULL AND claim_expires_at IS NOT NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_idempotency
  ON tasks(board_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_execution_plans (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  state TEXT NOT NULL CHECK(state IN ('unplanned', 'planned', 'not_required')),
  reason TEXT,
  updated_by TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(task_id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_steps (
  id TEXT PRIMARY KEY CHECK(id LIKE 'step_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  idempotency_key TEXT,
  position INTEGER NOT NULL,
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  body TEXT,
  linked_task_id TEXT,
  required INTEGER NOT NULL DEFAULT 1 CHECK(required IN (0, 1)),
  status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo', 'done', 'skipped')),
  resolution_note TEXT,
  resolved_by TEXT,
  resolved_at INTEGER,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_by TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(parent_task_id, idempotency_key),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(linked_task_id, board_id) REFERENCES tasks(id, board_id),
  CHECK(linked_task_id IS NULL OR parent_task_id != linked_task_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_steps_idempotency
  ON task_steps(parent_task_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_dependencies (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL,
  child_task_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(child_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_runs (
  id TEXT PRIMARY KEY CHECK(id LIKE 'r_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
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
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_task_runs_task_started
  ON task_runs(task_id, started_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_runs_one_active
  ON task_runs(task_id)
  WHERE status = 'running';

CREATE TABLE IF NOT EXISTS task_comments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  idempotency_key TEXT,
  author TEXT NOT NULL,
  author_type TEXT NOT NULL DEFAULT 'user' CHECK(author_type IN ('user', 'agent')),
  agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL),
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'note' CHECK(kind IN ('note', 'decision')),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  UNIQUE(task_id, idempotency_key)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_comments_idempotency
  ON task_comments(task_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK(event_id LIKE 'e_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT,
  run_id TEXT,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  actor TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  created_at INTEGER NOT NULL,
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id),
  FOREIGN KEY(run_id, board_id) REFERENCES task_runs(id, board_id)
);

CREATE INDEX IF NOT EXISTS idx_task_events_board_created
  ON task_events(board_id, id DESC);

CREATE INDEX IF NOT EXISTS idx_task_events_task_created
  ON task_events(task_id, id DESC);
"#;

pub(crate) const SCHEMA_VERSION: i64 = 1;
pub(crate) const SCHEMA_NAME: &str = "001_canonical_baseline";

/// schema family 与数字 migration version 有意分离。`version = 1` 但 family 不匹配时，
/// 即使表名为 `schema_migrations` 也不是 Turso 数据库，不能被自动采用。
pub(crate) const SCHEMA_FAMILY: &str = "kanban.turso";
pub(crate) const SCHEMA_LINEAGE: &str = "v1";

/// 对 v1 的精确 table/column 清单计算 SHA-256。`migration::validate_v1_shape` 会逐表拒绝
/// 缺列和多余列，因此该字面量既是 lineage 标识，也是升级前备份的 shape witness。
pub(crate) const CURRENT_V1_SCHEMA_FINGERPRINT: &str =
    "columns-sha256:c235e96f250e780f62241b55a9721b14b5ebe9244172e01a5655e16af6d18d00";

pub(crate) const FULL_SCHEMA_VERSION: i64 = 2;
pub(crate) const FULL_SCHEMA_NAME: &str = "002_turso_full_feature_baseline";

/// 完整 feature migration 新增的表。这里使用 Turso-native additive migration；本 crate
/// 不执行旧 SQLite v30 的 table-rebuild 脚本。
pub(crate) const FULL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS schema_identity (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  family TEXT NOT NULL,
  lineage TEXT NOT NULL,
  version INTEGER NOT NULL CHECK(version >= 1),
  fingerprint TEXT NOT NULL,
  migration_checksum TEXT NOT NULL,
  upgraded_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_capabilities (
  capability TEXT PRIMARY KEY,
  available INTEGER NOT NULL CHECK(available IN (0, 1)),
  detail TEXT NOT NULL,
  checked_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_attachments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'a_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  filename TEXT NOT NULL CHECK(length(trim(filename)) > 0),
  rel_path TEXT NOT NULL CHECK(length(trim(rel_path)) > 0),
  content_type TEXT,
  size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
  sha256 TEXT,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS labels (
  id TEXT PRIMARY KEY CHECK(id LIKE 'l_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  color TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, name),
  UNIQUE(id, board_id)
);

CREATE TABLE IF NOT EXISTS task_labels (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  label_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(task_id, label_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL CHECK(json_valid(value_json)),
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_subtasks (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL,
  child_task_id TEXT NOT NULL,
  position INTEGER NOT NULL,
  required INTEGER NOT NULL DEFAULT 1 CHECK(required IN (0, 1)),
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(child_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS entities (
  uri TEXT PRIMARY KEY CHECK(uri LIKE 'kb://%'),
  kind TEXT NOT NULL,
  source_table TEXT NOT NULL,
  source_id TEXT NOT NULL,
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT,
  title TEXT,
  summary TEXT,
  content_hash TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER,
  UNIQUE(source_table, source_id),
  UNIQUE(uri, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS relation_predicates (
  name TEXT PRIMARY KEY,
  domain_kind TEXT,
  range_kind TEXT,
  cardinality TEXT NOT NULL DEFAULT 'many',
  authoritative_store TEXT NOT NULL DEFAULT 'turso',
  description TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS entity_relations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  subject_uri TEXT NOT NULL REFERENCES entities(uri) ON DELETE CASCADE,
  predicate TEXT NOT NULL REFERENCES relation_predicates(name) ON DELETE RESTRICT,
  object_uri TEXT NOT NULL REFERENCES entities(uri) ON DELETE CASCADE,
  graph_uri TEXT NOT NULL CHECK(graph_uri LIKE 'kb://%'),
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  authoritative_store TEXT NOT NULL DEFAULT 'turso',
  source_table TEXT,
  source_id TEXT,
  source_event_id INTEGER,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(subject_uri, predicate, object_uri, graph_uri),
  FOREIGN KEY(subject_uri, board_id) REFERENCES entities(uri, board_id) ON DELETE CASCADE,
  FOREIGN KEY(object_uri, board_id) REFERENCES entities(uri, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS projection_jobs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  source_event_id INTEGER REFERENCES task_events(id) ON DELETE SET NULL,
  target TEXT NOT NULL CHECK(target IN ('fts', 'vector_tasks', 'vector_label_atoms', 'relations', 'all')),
  entity_uri TEXT CHECK(entity_uri IS NULL OR entity_uri LIKE 'kb://%'),
  dedupe_key TEXT,
  operation TEXT NOT NULL CHECK(operation IN ('upsert', 'delete', 'rebuild')),
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'running', 'done', 'failed')),
  attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0),
  max_attempts INTEGER NOT NULL DEFAULT 10 CHECK(max_attempts > 0),
  lease_owner TEXT,
  lease_token TEXT,
  lease_expires_at INTEGER,
  fence_epoch INTEGER NOT NULL DEFAULT 0 CHECK(fence_epoch >= 0),
  generation TEXT,
  next_attempt_at INTEGER,
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  CHECK(operation = 'rebuild' OR entity_uri IS NOT NULL),
  CHECK((status = 'running') = (lease_owner IS NOT NULL AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS projection_state (
  projection TEXT PRIMARY KEY CHECK(projection IN ('fts', 'vector_tasks', 'vector_label_atoms', 'relations')),
  lifecycle_status TEXT NOT NULL DEFAULT 'bootstrap_required' CHECK(lifecycle_status IN ('bootstrap_required', 'idle', 'rebuilding', 'ready', 'degraded', 'error')),
  active_generation TEXT,
  active_fingerprint TEXT,
  previous_generation TEXT,
  previous_fingerprint TEXT,
  building_generation TEXT,
  building_fingerprint TEXT,
  provider TEXT,
  provider_fingerprint TEXT,
  corpus_schema TEXT,
  corpus_fingerprint TEXT,
  embedding_model TEXT,
  embedding_dimensions INTEGER,
  last_event_id INTEGER NOT NULL DEFAULT 0 CHECK(last_event_id >= 0),
  dirty INTEGER NOT NULL DEFAULT 1 CHECK(dirty IN (0, 1)),
  lease_owner TEXT,
  lease_token TEXT,
  lease_expires_at INTEGER,
  fence_epoch INTEGER NOT NULL DEFAULT 0 CHECK(fence_epoch >= 0),
  last_success_at INTEGER,
  last_error TEXT,
  updated_at INTEGER NOT NULL,
  CHECK(embedding_dimensions IS NULL OR embedding_dimensions > 0),
  CHECK((lease_owner IS NULL AND lease_token IS NULL AND lease_expires_at IS NULL) OR (lease_owner IS NOT NULL AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL)),
  CHECK(previous_generation IS NULL OR previous_generation != active_generation),
  CHECK(building_generation IS NULL OR building_generation != active_generation)
);

CREATE TABLE IF NOT EXISTS label_semantics (
  label_id TEXT PRIMARY KEY,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  description TEXT,
  applies_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(applies_when) AND json_type(applies_when) = 'array'),
  excludes_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(excludes_when) AND json_type(excludes_when) = 'array'),
  positive_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(positive_examples) AND json_type(positive_examples) = 'array'),
  negative_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(negative_examples) AND json_type(negative_examples) = 'array'),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS label_atoms (
  id TEXT PRIMARY KEY CHECK(id LIKE 'la_%'),
  label_id TEXT NOT NULL,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  polarity TEXT NOT NULL CHECK(polarity IN ('positive', 'negative')),
  kind TEXT NOT NULL CHECK(kind IN ('name', 'description', 'applies_when', 'positive_example', 'excludes_when', 'negative_example')),
  text TEXT NOT NULL CHECK(length(trim(text)) > 0),
  ordinal INTEGER NOT NULL DEFAULT 0 CHECK(ordinal >= 0),
  content_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(label_id, polarity, kind, ordinal),
  UNIQUE(label_id, content_hash),
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS label_atom_index_boards (
  store_name TEXT NOT NULL,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  dirty INTEGER NOT NULL DEFAULT 1 CHECK(dirty IN (0, 1)),
  last_rebuild_at INTEGER,
  last_error TEXT,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(store_name, board_id)
);

CREATE TABLE IF NOT EXISTS label_semantic_proposals (
  id TEXT PRIMARY KEY CHECK(id LIKE 'lp_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed', 'accepted', 'rejected')),
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  description TEXT,
  applies_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(applies_when)),
  excludes_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(excludes_when)),
  positive_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(positive_examples)),
  negative_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(negative_examples)),
  heuristic_coverage REAL NOT NULL DEFAULT 0.0 CHECK(heuristic_coverage BETWEEN 0 AND 1),
  heuristic_residual_norm REAL NOT NULL DEFAULT 1.0 CHECK(heuristic_residual_norm BETWEEN 0 AND 1),
  heuristic_coverage_cosine REAL CHECK(heuristic_coverage_cosine IS NULL OR heuristic_coverage_cosine BETWEEN 0 AND 1),
  top1_existing_label_id TEXT,
  top1_existing_label_name TEXT,
  diagnostics_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(diagnostics_json)),
  created_by TEXT NOT NULL,
  decision_reason TEXT,
  resolved_label_id TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  decided_at INTEGER,
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(top1_existing_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  FOREIGN KEY(resolved_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  UNIQUE(id, board_id)
);

CREATE TABLE IF NOT EXISTS label_ontology_observations (
  id TEXT PRIMARY KEY CHECK(id LIKE 'lor_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  task_ref_snapshot TEXT NOT NULL CHECK(length(trim(task_ref_snapshot)) > 0),
  task_snapshot_json TEXT NOT NULL CHECK(json_valid(task_snapshot_json)),
  agent_candidates_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(agent_candidates_json)),
  suggestion_snapshot_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(suggestion_snapshot_json)),
  final_decision_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(final_decision_json)),
  suggest_coverage REAL,
  suggest_coverage_cosine REAL,
  suggest_residual_norm REAL,
  suggest_needs_new_label INTEGER NOT NULL DEFAULT 0 CHECK(suggest_needs_new_label IN (0, 1)),
  suggest_degraded INTEGER NOT NULL DEFAULT 0 CHECK(suggest_degraded IN (0, 1)),
  diagnostics_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(diagnostics_json)),
  capture_fingerprint TEXT NOT NULL CHECK(length(trim(capture_fingerprint)) > 0),
  suggest_input_hash TEXT,
  created_by TEXT NOT NULL CHECK(length(trim(created_by)) > 0),
  created_by_type TEXT NOT NULL CHECK(created_by_type IN ('user', 'agent')),
  agent_type TEXT,
  created_at INTEGER NOT NULL,
  UNIQUE(board_id, capture_fingerprint),
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS label_ontology_signals (
  id TEXT PRIMARY KEY CHECK(id LIKE 'los_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  observation_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('false_negative', 'false_positive', 'vocabulary_gap', 'name_issue', 'boundary_issue', 'structure_issue')),
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'confirmed', 'resolved', 'rejected', 'superseded')),
  target_label_id TEXT,
  target_label_name_snapshot TEXT,
  related_labels_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(related_labels_json)),
  proposed_action TEXT NOT NULL CHECK(proposed_action IN ('observe', 'add_positive_atom', 'add_negative_atom', 'update_semantics', 'bootstrap_label', 'rename_label', 'split_label', 'merge_labels')),
  candidate_atom_polarity TEXT,
  candidate_atom_kind TEXT,
  candidate_text TEXT,
  candidate_content_hash TEXT,
  proposed_label_name TEXT,
  proposed_label_name_normalized TEXT,
  proposal_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(proposal_json)),
  agent_selected INTEGER NOT NULL DEFAULT 0 CHECK(agent_selected IN (0, 1)),
  suggest_state TEXT CHECK(suggest_state IS NULL OR suggest_state IN ('selected', 'candidate', 'absent', 'unavailable')),
  suggest_score REAL,
  suggest_rank INTEGER,
  final_selected INTEGER NOT NULL DEFAULT 0 CHECK(final_selected IN (0, 1)),
  rationale TEXT NOT NULL CHECK(length(trim(rationale)) > 0),
  confidence REAL CHECK(confidence IS NULL OR confidence BETWEEN 0 AND 1),
  signal_key TEXT NOT NULL CHECK(length(trim(signal_key)) > 0),
  superseded_by_signal_id TEXT,
  status_reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  reviewed_at INTEGER,
  closed_at INTEGER,
  UNIQUE(observation_id, signal_key),
  UNIQUE(id, board_id),
  FOREIGN KEY(observation_id, board_id) REFERENCES label_ontology_observations(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(target_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  FOREIGN KEY(superseded_by_signal_id) REFERENCES label_ontology_signals(id) ON DELETE SET NULL,
  CHECK(id != superseded_by_signal_id)
);

CREATE TABLE IF NOT EXISTS label_ontology_actions (
  id TEXT PRIMARY KEY CHECK(id LIKE 'loa_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_action_id TEXT,
  action_type TEXT NOT NULL CHECK(action_type IN ('confirm', 'reject', 'supersede', 'resolve_no_change', 'add_positive_atom', 'add_negative_atom', 'adopt_existing_atom', 'update_semantics', 'create_label_proposal', 'bootstrap_label', 'rename_label', 'split_label', 'merge_labels', 'validate', 'revert_ontology_mutation')),
  reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
  target_label_id TEXT,
  result_label_id TEXT,
  result_atom_id TEXT,
  result_atom_content_hash TEXT,
  result_proposal_id TEXT,
  canonical_before_hash TEXT,
  canonical_after_hash TEXT,
  change_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(change_json)),
  validation_status TEXT NOT NULL DEFAULT 'not_required' CHECK(validation_status IN ('not_required', 'pending', 'passed', 'failed', 'partial')),
  validation_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(validation_json)),
  validation_requirement TEXT NOT NULL DEFAULT 'none' CHECK(validation_requirement IN ('none', 'required', 'unsupported')),
  created_by TEXT NOT NULL CHECK(length(trim(created_by)) > 0),
  created_by_type TEXT NOT NULL CHECK(created_by_type IN ('user', 'agent')),
  agent_type TEXT,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(parent_action_id) REFERENCES label_ontology_actions(id) ON DELETE SET NULL,
  FOREIGN KEY(target_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  FOREIGN KEY(result_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  FOREIGN KEY(result_proposal_id) REFERENCES label_semantic_proposals(id) ON DELETE SET NULL,
  UNIQUE(id, board_id)
);

CREATE TABLE IF NOT EXISTS label_ontology_action_signals (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  action_id TEXT NOT NULL,
  signal_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(action_id, signal_id),
  FOREIGN KEY(action_id, board_id) REFERENCES label_ontology_actions(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(signal_id, board_id) REFERENCES label_ontology_signals(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS label_ontology_action_atom_effects (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  action_id TEXT NOT NULL,
  label_id_snapshot TEXT NOT NULL CHECK(length(trim(label_id_snapshot)) > 0),
  atom_id_snapshot TEXT NOT NULL CHECK(atom_id_snapshot LIKE 'la_%'),
  atom_content_hash TEXT NOT NULL CHECK(length(trim(atom_content_hash)) > 0),
  polarity TEXT NOT NULL CHECK(polarity IN ('positive', 'negative')),
  kind TEXT NOT NULL CHECK(kind IN ('name', 'description', 'applies_when', 'positive_example', 'excludes_when', 'negative_example')),
  text TEXT NOT NULL CHECK(length(trim(text)) > 0),
  effect TEXT NOT NULL CHECK(effect IN ('added', 'removed')),
  created_at INTEGER NOT NULL,
  UNIQUE(action_id, atom_content_hash, effect),
  FOREIGN KEY(action_id, board_id) REFERENCES label_ontology_actions(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS signal_observations (
  id TEXT PRIMARY KEY CHECK(id LIKE 'obs_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT,
  run_id TEXT,
  comment_id TEXT,
  task_ref_snapshot TEXT,
  actor TEXT NOT NULL CHECK(length(trim(actor)) > 0),
  agent_type TEXT,
  source TEXT,
  evidence_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(evidence_json) AND json_type(evidence_json) = 'object'),
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE SET NULL,
  FOREIGN KEY(run_id) REFERENCES task_runs(id) ON DELETE SET NULL,
  FOREIGN KEY(comment_id) REFERENCES task_comments(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS signals (
  id TEXT PRIMARY KEY CHECK(id LIKE 'sig_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  observation_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  summary TEXT NOT NULL CHECK(length(trim(summary)) > 0),
  severity TEXT NOT NULL DEFAULT 'info',
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'confirmed', 'rejected', 'superseded', 'resolved')),
  dedupe_key TEXT,
  superseded_by_signal_id TEXT,
  reviewed_by TEXT,
  reviewed_at INTEGER,
  review_reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  UNIQUE(observation_id, board_id),
  FOREIGN KEY(observation_id, board_id) REFERENCES signal_observations(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(superseded_by_signal_id) REFERENCES signals(id) ON DELETE SET NULL,
  CHECK(id != superseded_by_signal_id)
);

CREATE TABLE IF NOT EXISTS projection_maintenance_owner (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  owner TEXT,
  lease_token TEXT,
  mode TEXT CHECK(mode IS NULL OR mode IN ('rebuild', 'compact', 'import', 'backup')),
  lease_expires_at INTEGER,
  fence_epoch INTEGER NOT NULL DEFAULT 0 CHECK(fence_epoch >= 0),
  capabilities_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(capabilities_json)),
  build_identity TEXT,
  started_at INTEGER,
  last_heartbeat_at INTEGER,
  updated_at INTEGER NOT NULL,
  CHECK((owner IS NULL AND lease_token IS NULL AND lease_expires_at IS NULL AND mode IS NULL) OR (owner IS NOT NULL AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL AND mode IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS retrieval_documents (
  id TEXT PRIMARY KEY CHECK(id LIKE 'doc_%'),
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  entity_uri TEXT REFERENCES entities(uri) ON DELETE CASCADE,
  source_kind TEXT NOT NULL,
  content TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, entity_uri, source_kind),
  UNIQUE(id, board_id),
  FOREIGN KEY(entity_uri, board_id) REFERENCES entities(uri, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS retrieval_vectors (
  id TEXT PRIMARY KEY CHECK(id LIKE 'vec_%'),
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  entity_uri TEXT REFERENCES entities(uri) ON DELETE CASCADE,
  document_id TEXT REFERENCES retrieval_documents(id) ON DELETE CASCADE,
  embedding BLOB NOT NULL,
  dimensions INTEGER NOT NULL CHECK(dimensions > 0),
  embedding_model TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(document_id, embedding_model),
  FOREIGN KEY(entity_uri, board_id) REFERENCES entities(uri, board_id) ON DELETE CASCADE,
  FOREIGN KEY(document_id, board_id) REFERENCES retrieval_documents(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS import_journal (
  id TEXT PRIMARY KEY CHECK(id LIKE 'ij_%'),
  source_kind TEXT NOT NULL CHECK(source_kind IN ('jsonl', 'sqlite_v30')),
  source_path TEXT NOT NULL,
  snapshot_fingerprint TEXT NOT NULL,
  phase TEXT NOT NULL CHECK(phase IN ('prepared', 'staged', 'validated', 'published', 'completed', 'failed')),
  staged_database_path TEXT,
  staged_attachment_root TEXT,
  canonical_attachment_root TEXT,
  manifest_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(manifest_json)),
  previous_identity_json TEXT CHECK(previous_identity_json IS NULL OR json_valid(previous_identity_json)),
  error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS attachment_staging (
  id TEXT PRIMARY KEY CHECK(id LIKE 'as_%'),
  journal_id TEXT NOT NULL REFERENCES import_journal(id) ON DELETE CASCADE,
  attachment_id TEXT NOT NULL,
  source_rel_path TEXT NOT NULL,
  staged_rel_path TEXT NOT NULL,
  expected_sha256 TEXT,
  expected_size_bytes INTEGER NOT NULL CHECK(expected_size_bytes >= 0),
  observed_sha256 TEXT,
  observed_size_bytes INTEGER,
  phase TEXT NOT NULL DEFAULT 'planned' CHECK(phase IN ('planned', 'copied', 'verified', 'published', 'failed')),
  error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(journal_id, attachment_id)
);

CREATE INDEX IF NOT EXISTS idx_task_attachments_task_created ON task_attachments(task_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_task_labels_label ON task_labels(label_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_subtasks_parent_position ON task_subtasks(parent_task_id, position);
CREATE INDEX IF NOT EXISTS idx_entities_board_kind ON entities(board_id, kind, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_entities_task ON entities(task_id);
CREATE INDEX IF NOT EXISTS idx_entity_relations_subject ON entity_relations(subject_uri);
CREATE INDEX IF NOT EXISTS idx_entity_relations_object ON entity_relations(object_uri);
CREATE INDEX IF NOT EXISTS idx_projection_jobs_ready ON projection_jobs(status, next_attempt_at, updated_at ASC);
CREATE INDEX IF NOT EXISTS idx_projection_jobs_board ON projection_jobs(board_id, status, updated_at ASC);
CREATE INDEX IF NOT EXISTS idx_projection_jobs_lease ON projection_jobs(lease_owner, lease_expires_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_projection_jobs_dedupe ON projection_jobs(target, dedupe_key) WHERE dedupe_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_projection_state_dirty ON projection_state(dirty, updated_at ASC);
CREATE INDEX IF NOT EXISTS idx_label_semantics_board_updated ON label_semantics(board_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_label_atoms_board_kind ON label_atoms(board_id, polarity, kind, ordinal);
CREATE INDEX IF NOT EXISTS idx_label_atoms_label_ordinal ON label_atoms(label_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_label_proposals_board_status ON label_semantic_proposals(board_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_label_proposals_task_status ON label_semantic_proposals(task_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_observation_task ON label_ontology_observations(task_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_signal_status ON label_ontology_signals(board_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_signal_label_kind ON label_ontology_signals(board_id, target_label_id, kind, status);
CREATE INDEX IF NOT EXISTS idx_ontology_signal_candidate_atom ON label_ontology_signals(board_id, candidate_content_hash, status) WHERE candidate_content_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ontology_signal_proposed_label ON label_ontology_signals(board_id, proposed_label_name_normalized, status) WHERE proposed_label_name_normalized IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ontology_action_created ON label_ontology_actions(board_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_action_label ON label_ontology_actions(board_id, target_label_id, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_ontology_action_create_proposal ON label_ontology_actions(board_id, result_proposal_id) WHERE action_type='create_label_proposal' AND result_proposal_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ontology_action_signals_signal ON label_ontology_action_signals(signal_id, action_id);
CREATE INDEX IF NOT EXISTS idx_ontology_action_atom_effects_hash ON label_ontology_action_atom_effects(board_id, atom_content_hash, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_action_atom_effects_label ON label_ontology_action_atom_effects(board_id, label_id_snapshot, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_signal_observation_created ON signal_observations(board_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_signal_observation_task ON signal_observations(board_id, task_id, created_at DESC) WHERE task_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_signals_status ON signals(board_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_signals_observation ON signals(observation_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_signals_dedupe_key ON signals(board_id, dedupe_key) WHERE dedupe_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_retrieval_documents_board ON retrieval_documents(board_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_retrieval_vectors_board ON retrieval_vectors(board_id, embedding_model);
CREATE INDEX IF NOT EXISTS idx_import_journal_phase ON import_journal(phase, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_import_journal_fingerprint ON import_journal(source_kind, snapshot_fingerprint);
CREATE INDEX IF NOT EXISTS idx_attachment_staging_phase ON attachment_staging(journal_id, phase);
CREATE TRIGGER IF NOT EXISTS task_events_board_guard_insert
BEFORE INSERT ON task_events
WHEN (NEW.task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE id = NEW.task_id AND board_id = NEW.board_id
)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_runs WHERE id = NEW.run_id AND board_id = NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'task_events reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS task_events_board_guard_update
BEFORE UPDATE OF board_id, task_id, run_id ON task_events
WHEN (NEW.task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE id = NEW.task_id AND board_id = NEW.board_id
)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_runs WHERE id = NEW.run_id AND board_id = NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'task_events reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_semantic_proposals_board_guard_insert
BEFORE INSERT ON label_semantic_proposals
WHEN (NEW.top1_existing_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.top1_existing_label_id AND board_id=NEW.board_id
)) OR (NEW.resolved_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.resolved_label_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_semantic_proposals reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_semantic_proposals_board_guard_update
BEFORE UPDATE OF board_id, top1_existing_label_id, resolved_label_id ON label_semantic_proposals
WHEN (NEW.top1_existing_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.top1_existing_label_id AND board_id=NEW.board_id
)) OR (NEW.resolved_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.resolved_label_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_semantic_proposals reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_ontology_signals_board_guard_insert
BEFORE INSERT ON label_ontology_signals
WHEN (NEW.target_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.target_label_id AND board_id=NEW.board_id
)) OR (NEW.superseded_by_signal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_ontology_signals
  WHERE id=NEW.superseded_by_signal_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_signals reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_ontology_signals_board_guard_update
BEFORE UPDATE OF board_id, target_label_id, superseded_by_signal_id ON label_ontology_signals
WHEN (NEW.target_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.target_label_id AND board_id=NEW.board_id
)) OR (NEW.superseded_by_signal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_ontology_signals
  WHERE id=NEW.superseded_by_signal_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_signals reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_ontology_actions_board_guard_insert
BEFORE INSERT ON label_ontology_actions
WHEN (NEW.parent_action_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_ontology_actions WHERE id=NEW.parent_action_id AND board_id=NEW.board_id
)) OR (NEW.target_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.target_label_id AND board_id=NEW.board_id
)) OR (NEW.result_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.result_label_id AND board_id=NEW.board_id
)) OR (NEW.result_proposal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_semantic_proposals
  WHERE id=NEW.result_proposal_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_actions reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_ontology_actions_board_guard_update
BEFORE UPDATE OF board_id, parent_action_id, target_label_id, result_label_id, result_proposal_id ON label_ontology_actions
WHEN (NEW.parent_action_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_ontology_actions WHERE id=NEW.parent_action_id AND board_id=NEW.board_id
)) OR (NEW.target_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.target_label_id AND board_id=NEW.board_id
)) OR (NEW.result_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.result_label_id AND board_id=NEW.board_id
)) OR (NEW.result_proposal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_semantic_proposals
  WHERE id=NEW.result_proposal_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_actions reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS signal_observations_board_guard_insert
BEFORE INSERT ON signal_observations
WHEN (NEW.task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE id=NEW.task_id AND board_id=NEW.board_id
)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_runs WHERE id=NEW.run_id AND board_id=NEW.board_id
)) OR (NEW.comment_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_comments WHERE id=NEW.comment_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'signal_observations reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS signal_observations_board_guard_update
BEFORE UPDATE OF board_id, task_id, run_id, comment_id ON signal_observations
WHEN (NEW.task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE id=NEW.task_id AND board_id=NEW.board_id
)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_runs WHERE id=NEW.run_id AND board_id=NEW.board_id
)) OR (NEW.comment_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_comments WHERE id=NEW.comment_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'signal_observations reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS signals_board_guard_insert
BEFORE INSERT ON signals
WHEN NEW.superseded_by_signal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM signals WHERE id=NEW.superseded_by_signal_id AND board_id=NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'signals superseded reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS signals_board_guard_update
BEFORE UPDATE OF board_id, superseded_by_signal_id ON signals
WHEN NEW.superseded_by_signal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM signals WHERE id=NEW.superseded_by_signal_id AND board_id=NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'signals superseded reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS entity_relations_board_guard_insert
BEFORE INSERT ON entity_relations
WHEN NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.subject_uri AND board_id IS NEW.board_id
) OR NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.object_uri AND board_id IS NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'entity_relations reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS entity_relations_board_guard_update
BEFORE UPDATE OF board_id, subject_uri, object_uri ON entity_relations
WHEN NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.subject_uri AND board_id IS NEW.board_id
) OR NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.object_uri AND board_id IS NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'entity_relations reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS projection_jobs_board_guard_insert
BEFORE INSERT ON projection_jobs
WHEN (NEW.source_event_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_events WHERE id = NEW.source_event_id AND board_id IS NEW.board_id
)) OR (NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'projection_jobs reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS projection_jobs_board_guard_update
BEFORE UPDATE OF board_id, source_event_id, entity_uri ON projection_jobs
WHEN (NEW.source_event_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_events WHERE id = NEW.source_event_id AND board_id IS NEW.board_id
)) OR (NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'projection_jobs reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS retrieval_documents_board_guard_insert
BEFORE INSERT ON retrieval_documents
WHEN NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'retrieval_documents entity board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS retrieval_documents_board_guard_update
BEFORE UPDATE OF board_id, entity_uri ON retrieval_documents
WHEN NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'retrieval_documents entity board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS retrieval_vectors_board_guard_insert
BEFORE INSERT ON retrieval_vectors
WHEN (NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
)) OR (NEW.document_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM retrieval_documents WHERE id = NEW.document_id AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'retrieval_vectors reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS retrieval_vectors_board_guard_update
BEFORE UPDATE OF board_id, entity_uri, document_id ON retrieval_vectors
WHEN (NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
)) OR (NEW.document_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM retrieval_documents WHERE id = NEW.document_id AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'retrieval_vectors reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS task_attachments_path_guard_insert
BEFORE INSERT ON task_attachments
WHEN NEW.rel_path LIKE '/%'
  OR NEW.rel_path LIKE '\%'
  OR instr('/' || replace(NEW.rel_path, '\', '/') || '/', '/../') > 0
BEGIN
  SELECT RAISE(ABORT, 'attachment rel_path escapes database directory');
END;

CREATE TRIGGER IF NOT EXISTS task_attachments_path_guard_update
BEFORE UPDATE OF rel_path ON task_attachments
WHEN NEW.rel_path LIKE '/%'
  OR NEW.rel_path LIKE '\%'
  OR instr('/' || replace(NEW.rel_path, '\', '/') || '/', '/../') > 0
BEGIN
  SELECT RAISE(ABORT, 'attachment rel_path escapes database directory');
END;
"#;

/// FTS index 独立于 canonical schema checksum，但 `kanban-store-turso` 必须启用
/// Turso 的 `fts` feature；初始化若不能创建该索引，会把 capability 记录为不可用。
pub(crate) const FTS_SCHEMA: &str = "CREATE INDEX IF NOT EXISTS idx_retrieval_documents_fts ON retrieval_documents USING fts (content);";

pub(crate) const DEFAULT_COLUMNS: [(&str, &str, i64, bool); 9] = [
    ("triage", "Triage", 10, false),
    ("todo", "Todo", 20, false),
    ("scheduled", "Scheduled", 30, false),
    ("ready", "Ready", 40, false),
    ("running", "Running", 50, false),
    ("blocked", "Blocked", 60, false),
    ("review", "Review", 70, false),
    ("done", "Done", 80, false),
    ("archived", "Archived", 90, true),
];
```

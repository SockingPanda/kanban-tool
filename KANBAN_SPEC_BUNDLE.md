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
- crates/kanban-service/src/schema.rs

`docs/SPEC.md`、`docs/CLI_SPEC.md`、`docs/API_SPEC.md`、`docs/STATE_MACHINE.md` 和 `docs/SCHEMA_CONTRACTS.md` 等分主题文档是当前行为的权威来源；本文件是这些源文档的同步快照，便于一次性阅读和离线传递。


---

# 文件：README.md

# Kanban Tool

Kanban Tool 是本地优先、单机、单用户的看板与 durable work queue。任务、依赖、评论、执行记录、事件、labels、ontology、signals、entities、relations、检索和派生状态都归属于同一个 canonical Turso 数据库。

产品只有一条运行路径：

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP / SSE
    kanban serve（唯一 host）
        ↓
ApplicationService + 状态机 + 事务
        ↓
kanban-service（唯一 Turso owner）
        ↓
canonical Turso database
```

只有 `kanban serve` 可以打开、初始化、迁移、备份、替换和关闭数据库。其他入口不会直开数据库，也没有 host 不可用时的 embedded/SQLite fallback。

## 快速开始

```bash
cargo install --path crates/kanban-cli --bin kanban
kanban serve
```

默认 host 为 `http://127.0.0.1:8721`，默认数据库为 `$XDG_DATA_HOME/kb/kanban.db`（未设置时通常为 `~/.local/share/kb/kanban.db`）。数据库路径只在 `serve` 或配置解析中使用，可由 `--db`、`KANBAN_DB` 或兼容变量 `KB_DB` 覆盖。

项目 shell 命令不触碰数据库：

```bash
kanban init
kanban config show
kanban board use agent-work
kanban board current
```

启动 host 后可以使用完整的任务和知识路径：

```bash
kanban board list
kanban board columns default
kanban task create "整理项目首页" --description "让第一次访问的人看懂项目"
kanban task step not-required default#1 --reason "单步任务"
kanban task specify default#1 --description "补充可执行规格"
kanban task promote default#1
kanban task claim default#1
kanban search "项目首页"
kanban index rebuild
kanban index sync
kanban entity upsert --uri 'kb://task/t_example' --kind task --source-table tasks --source-id t_example
kanban context build default#1
kanban graph neighborhood default#1
kanban --board default graph map
kanban vector status
```

所有 mutation 都由 host 的 `ApplicationService` 和同一事务边界校验；adapter 只负责 typed 请求、错误映射和展示。

## 入口和功能面

- **CLI**：普通命令通过 `kanban-client` 访问 localhost；`serve`、`init`、配置/board 选择、completion 和 Codex hook 是本地 shell 或 host 装配命令。
- **MCP**：`kanban-mcp` 使用 stdio 和 `rmcp`。公开工具由
  `kanban-protocol::MCP_OPERATION_CATALOG`（`mcp_operation_catalog()`）机器可读目录固定，
  共 102 个 tool，覆盖全部 101 个非 host-admin HTTP operation；
  `MCP_HOST_ADMIN_OPERATION_IDS` 明确禁止 12 个 host-admin operation。所有 tool 都调用
  typed client，不启动 host、不直接写数据库。search/graph/vector 与 label atom-index 的
  domain `rebuild`/`sync` 不属于这 12 个 host-admin operation，仍由 catalog 覆盖。
- **Desktop**：Tauri/React shell 通过 typed HTTP 使用 `board`、`list`、`map`、`events`、`runs`、`signals`、`ontology`、`maintenance`、`health`、`settings` 十个导航视图；task detail、attachments、steps、comments、dependencies、context 和 maintenance 继续复用同一 host。

CLI 的 canonical leaf 和 HTTP 的 method/path 由 `kanban-protocol` 的
`surface_operation_catalog()`/`endpoint_catalog()` 固定；可见 alias 只改善交互，不增加第二条
contract operation。当前知识面包含 board columns、entity upsert、task specify、graph
neighborhood/map，以及 search index 的 status/doctor/rebuild/sync；完整参数和 wire 形状见
[`docs/CLI_SPEC.md`](docs/CLI_SPEC.md) 与 [`docs/API_SPEC.md`](docs/API_SPEC.md)。

跨入口的领域面包括：

| 领域 | 当前 owner / 派生边界 |
| --- | --- |
| 看板、任务、计划、步骤、依赖、评论、runs、events | `kanban-service` canonical facts、状态机和 event transaction |
| labels、ontology、signals | service canonical ledger；ontology atom index 和 provider 结果可重建 |
| search | Turso FTS `task_search_fts`；未 ready/stale 时由 service 回退 canonical SQL |
| graph | `entities`、`relation_predicates`、`entity_relations` canonical；service 执行有深度上限、环检测和 board isolation 的 BFS |
| vector/context | Turso `vector32`、host 内 Ollama provider 和 bounded context merge；provider 不可用时返回 degraded diagnostics |
| maintenance/migration | host-owned doctor、checkpoint、backup、portable import/export、vacuum、`/api/v1/maintenance/rebuild`、`/api/v1/maintenance/cleanup`，以及可选 `legacy-sqlite-import` v30 importer |

FTS、vector、graph、context 和 projection 均可从 canonical facts 删除后重建，不能反向改变业务事实。

## 状态和 dispatcher

权威状态为 `triage|todo|scheduled|ready|running|blocked|review|done|archived`；`board_columns` 只是展示映射。`ready → running` 只能通过原子 claim，并与 active run、lease 和 `task.claimed` event 同事务提交。heartbeat、release、review、done、block、specify、unblock、reopen、reclaim 和 archive 都是显式 service commands，不提供任意 `transition(target_status)`。

`kanban serve` 默认不消费队列。只有 `--dispatcher-profile <path>` 才在同一 host 进程启动单 worker dispatcher；它只 claim `ready`，复用共享 claim/heartbeat/finish path，不会 claim `review`、`todo` 或 `scheduled`。

## 数据迁移

当前支持两条互补路径：

1. **Turso v1 → v2 原地升级**：host 先验证 schema family、shape、foreign keys 和 board isolation，创建已验证 sibling backup，再在事务内升级；失败回滚并保持旧数据库可启动。
2. **portable/legacy 导入**：portable JSONL 只导入 canonical facts，提交后入队 FTS/vector/graph rebuild；`import-v30` 读取 legacy SQLite v30，只在显式启用 `legacy-sqlite-import` feature 的 host 上执行，默认构建 fail-closed。

两条路径都经 host-owned `import_journal`、fingerprint、staging 和 recovery 语义；CLI、HTTP 和 Desktop 只请求 host，MCP 不暴露数据库替换、backup、vacuum 或 migration 管理命令。

## 工作区和文档

产品单元是七个 Rust crate（`kanban-core`、`kanban-service`、`kanban-protocol`、`kanban-client`、`kanban-server`、`kanban-cli`、`kanban-mcp`）、Desktop Tauri package `kanban-desktop` 和私有 `xtask`。旧 backend/helper sidecar 已从 active workspace 删除；相关 release/projection runbook 仅作为历史归档，不是当前 runtime 或 release gate。

核心事实源：

- [`docs/SPEC.md`](docs/SPEC.md)：产品范围和当前行为；
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)：进程、crate、ownership 和派生边界；
- [`docs/STATE_MACHINE.md`](docs/STATE_MACHINE.md)：状态、transition、claim、lease 和 dispatcher；
- [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md)：canonical schema、约束和导入；
- [`docs/API_SPEC.md`](docs/API_SPEC.md)、[`docs/CLI_SPEC.md`](docs/CLI_SPEC.md)：HTTP/CLI contract；
- [`docs/SCHEMA_CONTRACTS.md`](docs/SCHEMA_CONTRACTS.md)：protocol/schema/adoption evidence；
- [`docs/migration/turso-full-feature-parity.md`](docs/migration/turso-full-feature-parity.md)：`6ea277` baseline 到当前 owner/surface/test/gate 的 parity ledger。

文档同步使用 `just diff-check`、`just spec-bundle-check` 和受影响的 schema marker 检查。adoption、full、release、push、PR、merge 和发布均不因文档更新自动视为通过或执行。

## 明确边界

本项目不提供多用户/RBAC/云同步/远程访问、第二 canonical backend、第二 mutation path、自定义 IPC、自动 server supervision 或发布/PR 工作流。旧 Tantivy/LanceDB/Oxigraph/helper sidecar 只在历史迁移证据中保留，当前功能使用 Turso FTS、`vector32` 和 service BFS。

## 许可证

Kanban Tool 使用 [Apache License 2.0](LICENSE) 开源。


---

# 文件：docs/SPEC.md

# Kanban Tool 产品规范

文档类型：当前实现规范。代码、`kanban-protocol` catalog、真实 router/adapter 和测试优先于历史快照；尚未运行的 adoption/full gate 不在本文中标记为通过。

Kanban Tool 是本地优先、单机、单用户的看板与 durable work queue。canonical 事实只存于 host-owned Turso 数据库；CLI、MCP、Desktop 和 dispatcher 共享同一个 `ApplicationService`、状态机、事务和错误语义。

## 1. 固定执行路径

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP / SSE
      kanban serve（唯一 host）
        ↓
ApplicationService + kanban-core 状态机
        ↓
kanban-service（Turso、schema、migration、projection provider）
        ↓
canonical Turso database
```

硬性规则：

1. 只有 `kanban serve` 可以打开、初始化、迁移、备份、替换和关闭 Turso。
2. CLI、MCP、Desktop 只依赖 `kanban-client` 或 Desktop 的 TS HTTP client；它们不依赖数据库-owning crate，也不在 host 停止时 fallback。
3. 所有 mutation/query 都进入共享 service path。adapter 只解析输入、发送 typed request、映射 error 或渲染输出。
4. `tasks.status` 是事实，board column 只是展示；没有任意 `transition(target_status)`。
5. canonical facts、event 和 projection job enqueue 必须在规定的同一事务边界内提交或回滚。

默认 host 为 `http://127.0.0.1:8721`，默认数据库为 `$XDG_DATA_HOME/kb/kanban.db`；`--db`、`KANBAN_DB`、`KB_DB` 只由 `serve`/配置解析使用。`config show`、`init`、`board use/current`、completion 和 Codex hook 是不触库的本地 shell。

## 2. 产品范围

### 2.1 当前功能域

| 域 | canonical owner | 当前入口 |
| --- | --- | --- |
| boards/tasks/plans/steps/dependencies/comments/runs/events | `kanban-service` | HTTP、typed client、CLI、MCP、Desktop detail/board |
| labels、ontology、signals | `kanban-service` ledger | HTTP、CLI、MCP、Desktop workbench/detail |
| search/index | Turso FTS + host projection worker | HTTP、CLI、MCP、Desktop list/context |
| entities/relations/graph | `kanban-service` canonical relations + bounded BFS | HTTP、CLI、MCP、Desktop map/context |
| vector/context | Turso `vector32` + host Ollama provider + service merge | HTTP、CLI、MCP、Desktop typed API |
| host maintenance/migration | host-owned service/worker | HTTP、CLI、Desktop Maintenance/Health；MCP 不暴露管理写操作 |

以上功能共享同一条事实路径。FTS/vector/graph/context/projection 是可重建派生结果，不是第二事实源。

### 2.2 非目标

- 多用户、团队、邀请、RBAC、多租户、SaaS、云同步、公网监听或远程 worker；
- 第二 canonical backend、CLI/MCP/Desktop 直开数据库、embedded fallback 或自定义 IPC；
- 外部 Tantivy/LanceDB/Oxigraph/helper sidecar 作为 active runtime；旧 sidecar 仅在历史归档与迁移证据中保留；
- 自动 server supervision、`multiprocess_wal`、通用 mutation receipt、发布、push、PR、merge 或 release cohort。

`legacy-sqlite-import` 是 host 的显式可选 feature，不等于第二 runtime backend；它只读 legacy SQLite v30，并把事实导入 canonical Turso。

## 3. 状态与队列

canonical 状态集合：

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

`ready` 表示规格、排期、依赖和 execution plan 均允许执行；只有 `task.claim` 能把它原子地变为 `running`。claim 同事务创建 active run 和 event，最多一个 active run。`heartbeat`、`release`、`review`、`done`、`block`、`specify`、`unblock`、`reopen`、`reclaim`、`archive` 都是显式 typed command；dispatcher 只扫描 `ready`。

状态机完整 guard 和 event 见 [`STATE_MACHINE.md`](docs/STATE_MACHINE.md)。

## 4. Host、adapter 与 worker

### 4.1 HTTP/SSE

`kanban-server` 合并 boards、tasks、steps、comments、attachments、dependencies、entities、graph、search、context、labels、ontology、signals、runs、events、stats、maintenance 和 vector routers；`/health` 与 `/api/v1/stream/events` 也由同一 host 提供。`kanban-client` 只做 typed localhost HTTP/SSE，不复制 SQL 或 fallback。

### 4.2 CLI

CLI 顶层包括 `serve`、board/config/task/label/comment/context/attachment/dep/entity/graph/events/runs/run/search/index/signal/vector、doctor/stats/backup/export/import/import-v30/checkpoint/vacuum/maintenance、init/completions/__complete/hook。所有 domain/host-admin 命令（`serve` 除外）通过 client 请求 host；`import-v30` 未启用 feature 时返回 `feature_not_available`。

当前 canonical leaf 以 `kanban-protocol::surface_operation_catalog()` 与 Clap 的
`get_name()` 对齐。看板列使用 `board columns`，实体写入使用 `entity upsert`，triage
规格补全使用 `task specify`；graph 提供 `neighborhood`/`map`，search index 提供
`index rebuild`/`index sync`。visible alias 不单独产生 contract operation；已清理的旧
projection/admin、独立 lifecycle leaf 和 task-read 旧路径不属于 active surface。

### 4.3 MCP

`kanban-mcp` 是 Rust stdio server。公开工具由
`kanban-protocol::MCP_OPERATION_CATALOG`（`mcp_operation_catalog()`）机器可读目录固定，共
102 个 tool，覆盖全部 101 个非 host-admin HTTP operation；
`MCP_HOST_ADMIN_OPERATION_IDS` 明确禁止 12 个 host-admin operation。MCP 不启动 host、不打开
数据库、不提供 migration/backup/vacuum/replace 管理命令；search/graph/vector 与 label
atom-index 的 domain `rebuild`/`sync` 不属于这 12 个禁止项。

### 4.4 Desktop

Desktop 通过 `KanbanApi` 使用 HTTP，只保留 `apiBaseUrl`、actor、board 等运行时配置。十个导航视图为 `board`、`list`、`map`、`runs`、`events`、`signals`、`ontology`、`maintenance`、`health`、`settings`；task detail 继续承载 attachments、comments、dependencies、steps、runs/events、labels 和 context 的 typed API。危险 maintenance 操作需要二次确认，并展示 `server_unavailable`/`restart_required` 等稳定结果。

### 4.5 Worker

- dispatcher 是 `kanban serve --dispatcher-profile <path>` 启动的同进程单 worker；只 claim `ready`，复用 lifecycle commands。
- projection worker 在同一个 host 生命周期内处理 FTS、vector32/Ollama、relations 和 context 的 `projection_jobs`，支持 generation、fingerprint、lease、重试、degraded 和 rebuild。

两个 worker 都不得打开第二个数据库、维护第二套状态机或直接写 adapter DTO。

## 5. 数据与迁移

schema family 为 `kanban.turso`，当前 lineage 为 v1/v2：queue/history、labels/ontology/signals、entities/relations、retrieval、projection、import journal 和 attachment staging 均由 `kanban-service` 持有。canonical facts 包括 boards/tasks/plans/steps/dependencies/runs/comments/events、labels/ontology/signals、entities/relations、attachment metadata 和导入事实；`retrieval_documents`、`retrieval_vectors`、FTS、vector、BFS 结果及 projection control rows 可删除后重建。

支持两条迁移路径：

1. **Turso v1 → v2 原地升级**：host 校验 family、exact shape、constraints、foreign keys 和 board isolation；创建 verified sibling backup 后运行事务 migration，失败 rollback，重复启动幂等。
2. **portable/legacy 导入**：portable JSONL 只写 canonical facts，按 `import_journal` 记录 fingerprint/staging/phase，提交后入队 derived rebuild；`import-v30` 只读 legacy SQLite v30，attachment 先 staging、checksum/board isolation preflight，显式 feature 未启用则 fail-closed。

backup、export/import、checkpoint、vacuum、`/api/v1/maintenance/rebuild|cleanup` 和数据库
替换都由 host 管理；这里的 maintenance operation 不应与 MCP 可调用的 search/graph/vector
或 label atom-index domain `rebuild`/`sync` 混同。MCP 不承载前述 host-admin 命令。

## 6. 契约和错误

`kanban-protocol` 是 DTO、event payload、错误 envelope、endpoint/surface catalog 和 JSON Schema 的权威来源。错误以稳定 `error.code` 表示，message 不属于机器契约；常见 code 包括 `invalid_input`、`not_found`、`conflict`、`idempotency_conflict`、`dependency_cycle`、`claim_conflict`、`claim_token_mismatch`、`invalid_transition`、`feature_not_available`、`server_unavailable` 和 `internal`。

HTTP/API、CLI output、MCP tool schema 和 Desktop parser 必须引用同一 protocol DTO。schema adoption witness、surface audit 和完整测试尚未在本次文档任务中运行，不能因为 catalog 已有 `adopted` 条目就宣称所有 runtime gate 完成。

## 7. 验收边界

每个功能域的“已接入”至少需要：

1. canonical schema/constraint 与 service operation；
2. HTTP route + typed client；
3. 需要的 CLI/MCP/Desktop entry；
4. producer/consumer fixture、真实 route/adapter 测试和 board/事务负向测试；
5. FTS/vector/graph/context 可从 canonical facts rebuild，且旧 sidecar 不在 active workspace。

本文件只记录当前源码和已有测试事实；最终 adoption/full/schema gate、release、push 和 PR 由单独任务执行并独立报告。

详细 wire 行为见 [`API_SPEC.md`](docs/API_SPEC.md) 与 [`CLI_SPEC.md`](docs/CLI_SPEC.md)；逐域 baseline、owner、入口、迁移规则、实际测试和未完成 gates 见 [`migration/turso-full-feature-parity.md`](docs/migration/turso-full-feature-parity.md)。


---

# 文件：docs/ARCHITECTURE.md

# 架构

kanban-tool 的稳定边界是本地优先、单机、单用户和单一 canonical Turso owner。当前代码把 use case、Turso store、事务、projection provider 和 host wiring 收敛在 `kanban-service`/`kanban-server`；所有产品功能通过同一 service path 暴露。

```text
CLI / MCP / Desktop
        │ typed localhost HTTP / SSE
        ▼
kanban serve（唯一 host）
  Axum routes + ApplicationService
  kanban-core 状态机 + Turso transaction
  dispatcher + projection worker
        │
        ▼
kanban-service / canonical Turso
  facts + events + FTS/vector32/relations projection
```

## 1. Process 与 ownership

只有 `kanban serve`（`kanban-server` host）可以打开、初始化、迁移、备份、替换和关闭 Turso。默认绑定 `127.0.0.1:8721`，默认数据库为 `~/.local/share/kb/kanban.db`。普通 CLI、MCP、Desktop 只构造 typed localhost client；host 不可用时返回 `server_unavailable`，不会创建备用数据库。

`kanban-service` 是唯一直接持有 Turso connection、schema/migration、repository、transaction 和 provider 的产品 crate。server 负责 host 生命周期、HTTP/SSE、路由合并、dispatcher 和 projection worker 装配，但不直接解释 row 或状态机。所有 adapter 都不能提交 SQL；Vector 的 status/query/configure/rebuild/sync/enqueue 以及 embedding/provider coordination 统一由 `ApplicationService` 的 service API 编排。

同一 host 内的 operation 可以按需获取 connection；不启用 `multiprocess_wal`，canonical 文件不允许由其他产品入口或第二进程直开。`PRAGMA foreign_keys = ON`、复合 FK、CHECK、唯一约束、CAS、board isolation 和 service transaction 共同形成边界。

## 2. Workspace 边界

active 产品由七个 Rust crate、Desktop Tauri package 和私有 `xtask` 组成：

| 单元 | 职责 |
| --- | --- |
| `kanban-core` | 领域 ID、枚举、状态机、readiness/claim/lease 和纯校验；不依赖 Turso、HTTP 或 runtime |
| `kanban-service` | `ApplicationService`、Turso schema/migration、store operations、事务、FTS/vector32/Ollama、BFS、projection、portable/legacy import |
| `kanban-protocol` | HTTP/CLI/MCP DTO、event payload、error envelope、endpoint/surface catalog 和 JSON Schema |
| `kanban-client` | typed localhost HTTP/SSE client、selector 解析和响应映射；不持有 DB |
| `kanban-server` | Axum host、全部 API routers、唯一 host lifecycle、dispatcher/projection worker wiring |
| `kanban-cli` | clap 命令、JSON/human output、host wrapper 和不触库的 config/init/completion/hook shell |
| `kanban-mcp` | `rmcp` stdio tool adapter；只调用 `kanban-client` |
| Desktop (`kanban-desktop`) | Tauri host + React/TypeScript UI；只调用 localhost API |
| `xtask` | `publish = false` 的离线 schema/dependency/witness 工具，不进入 runtime |

旧的 backend/helper sidecar（包括历史搜索、vector、graph helper/protocol 和外部 projection 进程）已从 active workspace 删除。旧目录、lockfile provenance 和 recovery runbook 仅作为迁移/历史证据；不能被重新声明为 runtime crate 或第二 mutation path。

依赖方向固定为：

```text
kanban-core ◄── kanban-service ◄── kanban-server ◄── kanban-cli::serve
      ▲              ▲                  │
      │              └── kanban-protocol ◄── kanban-client ◄── CLI / MCP / Desktop
      └──────────────────────────────────────────────────────────┘

xtask ──► kanban-protocol[schema] + cargo metadata
```

## 3. Shared service path

```text
adapter input
  → typed client / Axum extractor
  → ApplicationService（kanban-service）
  → kanban-core readiness、CAS、board isolation
  → Turso transaction
  → canonical snapshot + event + projection job
  → protocol DTO / error envelope
```

`tasks.status` 是唯一事实；`board_columns` 仅展示。`ready → running` 在一个 immediate transaction 中 CAS claim，创建 active run 和 event。heartbeat、release、review、done、block、specify、unblock、reopen、reclaim、archive、label/ontology/signal review、attachment publish 和 import journal 都复用 service operation，adapter 不重新解释 guard。

`kanban-service` 同时提供实体/关系写入、board-scoped BFS、bounded context merge、label atom index、projection generation/fingerprint、Ollama outage degraded 和重建。派生状态只写 `projection_jobs`/`projection_state`/retrieval 表，不反向改变 canonical facts。

Vector service API 返回独立的 command/result DTO。`kanban-server` 的 router 与 dispatcher 只持有 `HostApplicationService`，不得导入 `TursoApplicationStore`、`TursoStore`、`StoreError` 或 projection row record；provider 调用和查询 embedding 也不能在 host adapter 中重新编排。

## 4. Turso schema 与派生能力

schema family 为 `kanban.turso`，v1/v2 lineage 与 exact shape 由 `schema.rs`、`migration.rs` 和 schema identity 校验。v2 包含 queue/history、labels/ontology/signals、entities/relations、retrieval、projection、import journal 和 attachment staging；字段、约束、fingerprint 见 [`DATA_MODEL.md`](docs/DATA_MODEL.md)。

### FTS

`retrieval_documents` 上的 `task_search_fts` 是 Turso `USING fts` index。task/comment/run/event mutation 在同一事务 enqueue projection job；host worker 用 `fts_match`、`fts_score`、`fts_highlight`。FTS 未 ready、落后或失败时，service 使用 canonical SQL fallback，并在 search meta/diagnostics 标记 stale/degraded。

### Vector

Ollama 只负责 host 内 embedding provider；model、dimension、fingerprint、重试和降级由 service/worker 管理。向量保存和 cosine query 使用 Turso `vector32`。provider outage 不丢失 canonical task、label、ontology 或 entity，只返回 degraded 状态并保留可重建 job。

### Graph / context

`entities`、`relation_predicates`、`entity_relations` 是 canonical relation facts。Turso 不依赖 recursive CTE；service 执行带深度上限、环检测、去重和 board isolation 的批量 BFS，提供 neighbors/query/neighborhood/task-map。context pack 合并 subject、FTS lexical、graph 和 vector 候选，按 budget/rank/provenance 去重；graph/vector 不可用时保留可用 lexical 结果。

### Projection

canonical mutation 写 facts、event 和 `projection_jobs`；FTS、vector、relations、graph/context 都可删除后 rebuild。`projection_state` 保存 generation、fingerprint、provider/corpus、lag/error；`projection_maintenance_owner` 串行化 rebuild/cleanup/import/backup。没有 helper subprocess、framed protocol 或独立 control plane。

## 5. Migration 与 host-admin

1. **Turso v1 → v2 原地升级**：host 先检查 schema family、table/column/index/trigger/constraint、foreign keys 和 board isolation，创建 verified sibling backup，然后在事务内升级；失败 rollback，重复启动幂等。
2. **portable/legacy 导入**：portable JSONL 只导入 canonical facts，`import_journal` 记录 fingerprint、staging 和 phase，提交后 enqueue derived rebuild；`import-v30` 只读 legacy SQLite v30，attachment 先做 schema/计数/checksum/board preflight，在显式 `legacy-sqlite-import` feature 下由 service 执行，默认构建 fail-closed。

backup、export/import、checkpoint、vacuum、`/api/v1/maintenance/rebuild|cleanup` 和 database
replace 都是 host-admin operation。CLI、HTTP、Desktop 管理入口复用 host；MCP 只承载领域
query/mutation，不承载数据库替换或迁移管理。search/graph/vector 与 label atom-index 的
domain `rebuild`/`sync` 不属于 host-admin surface。

## 6. Surface 与 worker

`kanban-server/src/http/operations` 当前合并 boards、tasks、steps、comments、attachments、dependencies、entities、graph、search、context、labels、ontology、signals、runs、events、stats、maintenance 和 vector routers；`/health`、`/api/v1/stream/events` 也由 host 提供。真实 route 与 `kanban-protocol::endpoint_catalog()` 必须同步，但 catalog/adoption descriptor 不能替代 actual route test。

CLI domain 命令、MCP protocol machine-readable catalog（102 个 tool，覆盖 101 个非
host-admin HTTP operation）和 Desktop 十个导航视图都通过 typed client 接入；catalog 明确
拒绝 12 个 host-admin operation。Desktop Tauri command 不持有 Turso；claim token 仅在会话
状态中保存。维护操作显示 host 返回的 phase、degraded 和 `restart_required`，不凭 UI 状态
推断 canonical 成功。

dispatcher 只有传入 `--dispatcher-profile` 才启动，轮询 `ready`、复用 claim/heartbeat/finish、优雅停止等待当前 worker；projection worker 与 host 共用 lifecycle/maintenance lease，按 job generation 处理 FTS/vector/relations/context，并通过共享 `ApplicationService::vector_worker_tick` 协调 Vector provider。两类 worker 都不打开第二数据库、不维护第二状态机、不直接改 adapter DTO。

## 7. 证据与停止边界

每个纵向 slice 必须闭合 `core/service → protocol/server → client → CLI/MCP/Desktop → tests/docs`，并提供 schema/fixture、真实 producer/consumer、负向约束和 rebuild/fallback 证据。旧 sidecar 删除后只保留历史归档，不以 shim 恢复。

本次文档任务不运行或伪造 adoption/full/schema-audit/release gate；ledger 中将它们标为待运行，并记录已有测试名称。发布、push、PR、merge 不属于架构收敛范围。


---

# 文件：docs/STATE_MACHINE.md

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


---

# 文件：docs/DATA_MODEL.md

# Canonical 数据模型

本文件描述 `kanban-service` 当前持有的 Turso schema、canonical/derived 边界和两条导入路径。权威实现为 `crates/kanban-service/src/schema.rs`、`migration.rs`、`maintenance.rs`、`legacy_import.rs` 以及 entities/graph/search/vector operations；文档不创造第二份 schema inventory。

## 1. Schema family 与 lineage

数据库必须属于 `kanban.turso` family。migration version 与 lineage 分开记录，未知 family/shape 一律 fail-closed。

| lineage | migration | 当前事实 |
| --- | --- | --- |
| `v1` | `001_canonical_baseline` | queue/history 的窄 Turso 输入；启动时精确检查 10 张表和列形状 |
| `v2` | `002_turso_full_feature_baseline` | full-feature schema；包含 38 张表、443 个列、45 个普通 index、22 个 trigger，另有 Turso FTS `task_search_fts` |

`FULL_EXACT_COLUMNS`、`FULL_REQUIRED_INDEXES`、`FULL_REQUIRED_TRIGGERS`、SQL fragment 和 schema identity 共同构成 exact shape。`schema_migrations` 保存 name/checksum/applied_at/family；`schema_identity` 保存 family/lineage/version/fingerprint；`schema_capabilities` 保存运行时 `fts`/`vector32` probe。仅表名相同但 family、列、约束或 trigger 不同的数据库不能被采用。

## 2. Canonical 与 derived

canonical 是不可由索引重建的业务事实；derived、缓存和 worker control rows 可以删除后从 canonical facts 重建。

| 分类 | canonical facts | derived/运行时 |
| --- | --- | --- |
| board/task/history | `boards`、`board_columns`、`tasks`、`task_execution_plans`、`task_steps`、`task_dependencies`、`task_runs`、`task_comments`、`task_events`、`task_attachments`、`app_settings` | 无；event 是追加审计事实，不是第二状态机 |
| labels/ontology/signals | `labels`、`task_labels`、`label_semantics`、`label_atoms`、`label_semantic_proposals`、`label_ontology_observations`、`label_ontology_signals`、`label_ontology_actions`、`label_ontology_action_signals`、`label_ontology_action_atom_effects`、`signal_observations`、`signals` | `label_atom_index_boards` 可重建 |
| entities/relations | `entities`、`relation_predicates`、`entity_relations` | neighbors、neighborhood、task map 和 BFS 结果按需重算 |
| projection | 无业务事实 | `projection_jobs`、`projection_state`、`projection_maintenance_owner` |
| retrieval | 无业务事实 | `retrieval_documents`、`retrieval_vectors`、`task_search_fts`、vector query cache |
| migration/attachments | `import_journal` 的导入阶段事实 | `attachment_staging` 和 staging 文件；发布后文件 metadata 归 `task_attachments` |
| schema metadata | `schema_migrations`、`schema_identity` | `schema_capabilities` probe 可刷新 |

`projection_jobs` 是 durable worker queue，不是业务事实；projection failure 只能让 search/vector/graph/context degraded，不得回滚或改写 task/label/signal/entity facts。

## 3. ID、时间和 JSON

实体 ID 使用固定前缀的 ULID 字符串：

| 实体 | 前缀 |
| --- | --- |
| board/task/step/run/comment/event/column/attachment | `b_`、`t_`、`step_`、`r_`、`c_`、`e_`、`col_`、`a_` |
| label/atom/proposal | `l_`、`la_`、`lp_` |
| ontology observation/signal/action | `lor_`、`los_`、`loa_` |
| generic observation/signal | `obs_`、`sig_` |
| entity/document/vector | `kb://`、`doc_`、`vec_` |
| import/staging | `ij_`、`as_` |

`task_events.id` 是 `INTEGER PRIMARY KEY AUTOINCREMENT` 游标，公开 `event_id` 使用 `e_...`。时间是 UTC Unix epoch milliseconds，Rust 边界使用 `i64`。JSON 存为 `TEXT` 并由 `json_valid`/`json_type` 约束；未知 event payload 必须保留原始合法 JSON。

## 4. Queue、lifecycle 与 relations

`tasks.status` 是唯一状态真相；`board_columns` 只描述列顺序、hidden 和 WIP。task 身份包含全局 `t_...`、board-local `seq` 和实体范围的 idempotency key；`(board_id, seq)`、实体 id、active run 和 composite FK 提供唯一性与 board isolation。

`task_execution_plans` 为 `unplanned|planned|not_required`；`task_steps` 保存 position、required、resolution 和可选同板 linked task；`task_dependencies` 由 service 做可达路径/环检查；`task_runs` 保存 claim、worker、heartbeat、summary/error、exit code 和受控相对 log path，单任务最多一个 active run。

`task_comments` 支持 `note|decision|signal`；signal backlink 由 signal record 事务生成。`task_events` append-only，snapshot、run 和 event 在同一事务写入。attachment metadata 只存 host root 下的相对路径、size、content type、SHA-256；服务端 staging、fsync、原子发布和 `.trash/` 恢复语义禁止任意文件读写。

## 5. Labels、ontology、signals

`labels`/`task_labels` 保存 board 范围身份和绑定；`label_semantics`/`label_atoms` 保存语义和可解释的 atom；`label_semantic_proposals` 记录提议、采纳/拒绝和 diagnostics。ontology ledger 的 observation/signal/action 及 atom effect rows 保留 CAS hash、actor、source signal、validation JSON、review/revert 信息；generic `signal_observations`/`signals` 记录非 ontology 产品信号与 comment backlink。

所有 label/ontology/signal 引用必须同 board。应用层负责 CAS、状态转换、proposal review 和 validation；FK、CHECK、trigger 负责跨 board、自引用和唯一性。

## 6. Entities、graph、search、vector、context

`entities` 用 `kb://...` URI 保存 kind、source、board/task、摘要和 content hash；`relation_predicates` 定义谓词；`entity_relations` 保存 subject/predicate/object、source event 和 metadata。它们是 canonical facts，service 通过有限深度、环检测、去重和 board isolation 的 BFS 提供 graph query、neighborhood 和 task map。

`retrieval_documents` 是可重建文本语料；`task_search_fts` 由 Turso `fts` 提供 match/score/highlight。搜索在 projection 不可用时回退 canonical SQL，并在 response meta 标记 `stale`/`fallback_reason`。

`retrieval_vectors` 保存 vector32 embedding、model、dimension、content/provider fingerprint。Ollama 是 host 内 provider；Turso `vector32` 完成向量存储和 cosine query。provider outage 不影响 canonical writes，仅产生 degraded status 和可重试 job。context pack 合并 lexical/graph/vector 候选，按 subject、budget、rank、provenance 去重；所有候选先做 board isolation。

## 7. Migration、backup 和 import journal

### 7.1 Turso v1 → v2 原地升级

`kanban serve` 在 immediate transaction 中：

1. 检查 family、exact tables/columns/indexes/triggers/constraints、foreign keys 和 board isolation；
2. 通过 Turso `VACUUM INTO` 创建并重新打开 verified sibling backup，比较 integrity 和逐表计数；
3. 执行 v2 migration，写 schema ledger/identity/projection seed/default board；
4. 失败时 rollback，保持旧 v1 facts 和 backup 可再次启动；重复启动幂等。

未知 family、列 drift、constraint/trigger drift、FK/board guard 失败都 fail-closed；migration 不修改输入源文件。

### 7.2 portable JSONL 与 legacy SQLite v30

portable export/import 只包含 canonical facts；目标 host 可以是仅含 bootstrap board/columns 的 fresh v2 数据库。`import_journal` 记录 `jsonl|sqlite_v30` source fingerprint、manifest、staging、previous identity、`prepared|staged|validated|published|completed|failed` phase 和 error。`replace=true` 先 verified backup，再在 host-owned handle 内按 FK 顺序替换 canonical facts；事务错误 rollback，重复 fingerprint 幂等，提交后 enqueue FTS/vector/graph rebuild。

`import-v30` 只读 legacy SQLite v30：先 schema/计数/reference/board isolation preflight，再将附件复制到同文件系统 staging，校验 size/SHA-256，事务插入 canonical facts，按 journal resume。它由 `legacy-sqlite-import` feature 提供并经 host-admin HTTP/CLI 调用；未启用 feature 时返回 `feature_not_available`，默认 runtime 不包含第二 SQLite backend。

## 8. 约束和 ownership

1. `kanban serve` 是唯一 DB owner，并开启 `PRAGMA foreign_keys = ON`。
2. mutation 使用 immediate transaction；task snapshot、run、event、labels/ontology/signals 和 projection enqueue 要么全提交，要么全回滚。
3. claim/lease、CAS hash、lock version、idempotency key、dependency cycle、attachment path 和 import fingerprint 都由 service + Turso 约束保护。
4. FTS/vector/graph/context、cache 和 projection control rows 始终可删可重建，不能成为业务状态或导入计数依据。
5. 旧 Tantivy/LanceDB/Oxigraph/helper sidecar 不在 active workspace；其历史 schema/恢复文档只作为 migration evidence，不改变当前 owner。

详细 HTTP/CLI 入口见 [`API_SPEC.md`](docs/API_SPEC.md)、[`CLI_SPEC.md`](docs/CLI_SPEC.md)；按 baseline 映射的 migration/test/gate 见 [`migration/turso-full-feature-parity.md`](docs/migration/turso-full-feature-parity.md)。


---

# 文件：docs/CLI_SPEC.md

# `kanban` CLI 规范

`kanban` 是 canonical localhost application host 的薄适配器。除 `serve`、配置/init、completion 和 Codex hook 外，命令都创建 `kanban-client` 并请求 `http://127.0.0.1:8721`；CLI 不打开、初始化或 fallback 到数据库。

## 1. 全局选项与错误

```text
kanban [OPTIONS] <COMMAND>
```

| 选项 | 来源/默认 | 作用 |
| --- | --- | --- |
| `--server-url <URL>` | `KANBAN_SERVER_URL` / `http://127.0.0.1:8721` | loopback host |
| `--board <SLUG-OR-ID>` | `KB_BOARD` / `default` | board-scoped selector context |
| `--db <PATH>` | `KANBAN_DB` → `KB_DB` → XDG data-local | 只供 `serve`/配置解析 |
| `--locale <auto|zh-CN|en>` | `KANBAN_LOCALE` / system | 人类输出语言 |
| `--actor <NAME>` | `KANBAN_ACTOR` / local user | `X-KB-Actor` 审计值 |
| `--json` | 关闭 | 稳定 JSON output |

JSON 成功通常为 `{ "data": ... }`；运行期错误使用 `error.code` 和 exit code，clap 参数错误仍退出 `2`。常见 exit code：`not_found=3`、状态/依赖 guard `=4`、claim/idempotency conflict `=5`、dependency blocked `=6`、`server_unavailable=9`、`feature_not_available=10`。

<!-- schema-doc-ignore: CLI 错误 envelope 说明性示例。 -->
```json
{"error":{"code":"server_unavailable","message":"服务端不可用：请检查服务端 URL，并确认已运行 `kanban serve`","exit_code":9}}
```

task selector 支持全局 `t_...`、`board#seq`、`#seq` 和当前 board 数字 seq；typed client 在需要全局 path 时先解析 selector。`--board` > `KB_BOARD` > 最近项目 `.kb/config.toml` > `default`；`--db` > `KANBAN_DB` > `KB_DB` > 项目/global config > XDG 默认路径。

## 2. 唯一 host 与本地 shell

```text
kanban serve [--db <PATH>] [--host <LOOPBACK-IP>] [--port <PORT>]
             [--dispatcher-profile <PATH>]
kanban init [--force]
kanban config show
kanban board use <BOARD>
kanban board current
kanban completions bash|zsh|fish|powershell|elvish
kanban __complete ...
```

只有 `serve` 打开/初始化/迁移/关闭 Turso。默认 `--host=127.0.0.1`、`--port=8721`；非 loopback 直接 `invalid_input`。无 profile 不启动 dispatcher；有 profile 才运行同进程单 worker。

`init` 幂等创建/复用 `.kb/config.toml`，`config show` 只解析值和来源，`board use/current` 只读写本地选择；它们不校验或创建 canonical board。completion 和隐藏 `__complete` 只处理静态/本地候选，不触库。

Codex hook：

```text
kanban hook codex install|status|uninstall
kanban hook codex handle failure|task-create
```

managed marker/fingerprint、原子写入和 handler stdin/stdout 协议由 CLI 负责；handler 不直接写 Turso。

## 3. Board、task、steps、dependency

```text
kanban board list [--include-archived]
kanban board columns [BOARD]
kanban board create <SLUG> <NAME>
kanban board show <BOARD>
kanban board archive <BOARD>

kanban task create <TITLE> [--description <TEXT>] [--status <STATUS>]
  [--assignee <NAME>] [--priority <0..=3>] [--scheduled-at <MS>] [--due-at <MS>]
  [--max-retries <N>] [--metadata <JSON>] [--labels <NAME>...]
  [--depends-on <TASK_SELECTOR>...] [--idempotency-key <KEY>] [--task-id <T_ID>]
kanban task list [filters...] [--limit <N>] [--offset <N>] [--sort <SORT>]
kanban task show <TASK_SELECTOR> [--details]
kanban task update <TASK_SELECTOR> [fields...]
```

`task.status` 只能通过显式 lifecycle command 改变；`task update` 只更新 service 允许的内容/排期/metadata 字段。list 支持 status、priority、plan_filter、assignee、query、archive 和完整 sort contract。

execution plan：

```text
kanban task step add <TASK_SELECTOR> <TITLE> [--body <TEXT>] [--link-task <TASK_SELECTOR>]
kanban task step list <TASK_SELECTOR>
kanban task step update <TASK_SELECTOR> <STEP_SELECTOR> [fields...]
kanban task step done <TASK_SELECTOR> <STEP_SELECTOR> --note <TEXT>
kanban task step skip <TASK_SELECTOR> <STEP_SELECTOR> --reason <TEXT>
kanban task step reopen <TASK_SELECTOR> <STEP_SELECTOR>
kanban task step remove <TASK_SELECTOR> <STEP_SELECTOR>
kanban task step not-required <TASK_SELECTOR> --reason <TEXT>
```

`STEP_SELECTOR` 为全局 `step_...` 或 task-local `S<n>`。step required/linked-task/position/status 都由 service 校验。

```text
kanban dep add <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
kanban dep list <TASK_SELECTOR>
kanban dep remove <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
```

`dependency` 是 `dep` visible alias；server 负责同 board、FK、唯一约束和 cycle 检查。

## 4. Lifecycle

```text
kanban task promote <TASK_SELECTOR>
kanban task specify <TASK_SELECTOR> [--description <TEXT>] [--scheduled-at <MS>]
kanban task claim <TASK_SELECTOR> [--ttl-ms <MS>] [--worker-profile <PROFILE>]
kanban task heartbeat <TASK_SELECTOR> --claim-token <TOKEN> [--ttl-ms <MS>] [--note <TEXT>]
kanban task release <TASK_SELECTOR> --claim-token <TOKEN>
kanban task review <TASK_SELECTOR> [--claim-token <TOKEN>] [--force]
kanban task done <TASK_SELECTOR> [--claim-token <TOKEN>] [--force]
kanban task block <TASK_SELECTOR> [<REASON>|--reason-file <PATH|->] [--claim-token <TOKEN>] [--force]
kanban task unblock <TASK_SELECTOR> [--reason <TEXT>]
kanban task reopen <TASK_SELECTOR> [--reason <TEXT>]
kanban task reclaim <TASK_SELECTOR> [--force]
kanban task archive <TASK_SELECTOR> [--force]
```

claim 是原子 `ready → running`，同时创建 run/event；heartbeat/release/review/done/block/reclaim 复用 owner/token、lease、CAS 和同一 transaction。`block` reason inline 与 `--reason-file` 互斥。

## 5. Labels、ontology、signals

```text
kanban label|labels|ontology list|create|add|remove
kanban label semantics list|show|upsert|delete
kanban label atoms list|explain
kanban label atom-index status|rebuild|query
kanban label suggest <TASK_SELECTOR>
kanban label propose <TASK_SELECTOR>
kanban label proposals list|show|accept|reject
kanban label ontology record|list|show|review|quality|confirm|reject|resolve|supersede|apply|revert|validate
```

label identity/binding、semantics CAS、atom index、proposal 和 ontology ledger 都经 host typed API；atom/index 是可重建 derived state，CAS/review/action event 由 service 负责。

```text
kanban signal record [--input <JSON-FILE|->]
kanban signal list [filters...]
kanban signal show <SIGNAL_ID>
kanban signal review [filters...]
kanban signal confirm|reject|resolve <SIGNAL_ID>... --reason <TEXT>
kanban signal supersede <SIGNAL_ID>... --by <SIGNAL_ID> --reason <TEXT>
```

signal record/backlink/review/lifecycle 共享一条事务 path；`--json` output 使用 `kanban-protocol` DTO。

## 6. Search、graph、vector、context

```text
kanban search <TEXT> [--status <STATUS>...] [--label <NAME>...] [--assignee <NAME>]
  [--include-archived] [--limit <N>] [--offset <N>]
kanban index status|doctor|rebuild|sync

kanban entity list|show|upsert ...
kanban graph status|neighbors|query|neighborhood|map|rebuild|sync ...

kanban vector configure|status|rebuild|sync|query-chunks|query-label-atoms ...
kanban context build [SUBJECT] [--task <TASK_ID>] [--reference <REF>] [--query <TEXT>]
  [--depth <N>] [--lexical-limit <N>] [--graph-limit <N>] [--vector-limit <N>]
  [--max-items <N>] [--budget <N>]
```

search 使用 Turso FTS，未 ready/stale 时回退 canonical SQL；graph 使用 canonical relations 的 bounded BFS；vector 使用 Turso `vector32` + host Ollama；context 是只读 bounded pack，按 provenance/rank/budget 去重，provider degraded 仍返回可用 lexical/canonical 结果。

## 7. Comments、attachments、runs、events

```text
kanban comment add <TASK_SELECTOR> <BODY> [--kind note|decision|signal] [options...]
kanban comment list <TASK_SELECTOR>

kanban attachment add <TASK_SELECTOR> <FILE> [--filename <NAME>] [options...]
kanban attachment list <TASK_SELECTOR>
kanban attachment download <TASK_SELECTOR> <ATTACHMENT_ID> --out <PATH>
kanban attachment remove <TASK_SELECTOR> <ATTACHMENT_ID>

kanban runs <TASK_SELECTOR>
kanban run show <RUN_ID>
kanban run logs|log <RUN_ID>
kanban events [TASK_SELECTOR] [--after <ID>] [--limit <N>]
```

attachment add/remove 只请求 host，download 写 raw bytes 到用户指定 output；run 不能独立 create/update，log 是固定 256 KiB bounded snapshot；events 保留未知 payload，CLI 可显示 task/event data。

## 8. Host-admin maintenance

```text
kanban doctor
kanban stats [--board <BOARD>]
kanban backup --path <PATH>
kanban export --path <PATH>
kanban import --path <PATH> [--replace]
kanban import-v30 --path <PATH> [--attachment-root <PATH>]
kanban checkpoint
kanban vacuum
kanban maintenance status
kanban maintenance run|rebuild|cleanup [--owner <OWNER>]
```

这些命令只通过 host 管理 API 执行。portable import/replace 使用 `import_journal`、verified backup、atomic transaction 和 derived rebuild；`import-v30` 未启用 `legacy-sqlite-import` 时 typed 返回 `feature_not_available`。MCP 不提供这些命令。

## 9. 停止行为和 gate 边界

host 停止或端口不可达时，已注册 command 返回 `server_unavailable`（exit `9`）；未知顶层 command 使用 external catch-all 返回 `feature_not_available`（exit `10`），不触碰存储。没有直接 DB fallback。

`kanban-protocol` 的 operation/surface catalog、fixture 和 adoption witness 是机器契约；本文件只描述实际 clap adapter。schema surface audit、adoption/full、Desktop package、release、push 和 PR 不因 CLI 文档同步自动运行或变绿。

### Canonical leaf 口径

`kanban-protocol::surface_operation_catalog()` 与 Clap 的 canonical `get_name()` 一一对应；
visible alias 不会产生第二个 leaf contract。当前新增并已接入的 leaf 包括 `board columns`、
`entity upsert`、`task specify`、`graph neighborhood`、`graph map`、`index rebuild` 和
`index sync`。旧 projection/admin、独立 lifecycle leaf 与旧 task-read path 不在 active
catalog 中；完整 exact 列表以 `schemas/json-schema/draft-2020-12/surface-operations.json`
和对应源代码为准。


---

# 文件：docs/API_SPEC.md

# 本地 HTTP API 规范

`kanban serve` 提供本机 application API。CLI、MCP、Desktop 只能通过 typed localhost HTTP/SSE client 调用它；它们不打开数据库，也不各自实现状态转换。默认地址为 `http://127.0.0.1:8721`，产品路由前缀为 `/api/v1`，健康路由为 `/health`。

## 1. 通用契约

- 请求/响应使用 JSON；成功 envelope 为 `{ "data": ... }`，列表可带 `meta`。
- server 只绑定 loopback；client 拒绝非 loopback URL。host 停止时返回 `server_unavailable`，不会 fallback。
- mutation actor 依次来自 body `actor`、`X-KB-Actor`、host 默认 actor；comment `author` 是命名上的 body 优先级例外。
- `error.code` 是稳定机器字段，`message` 只供人阅读。常见 code：`invalid_input`（400）、`not_found`（404）、`conflict`/`idempotency_conflict`/`dependency_cycle`/`claim_conflict`/`invalid_transition`（409）、`claim_token_mismatch`（403）、`feature_not_available`（501）、`internal`（500）。
- path 中 task 使用全局 `t_...`，run 使用 `r_...`，step 使用 `step_...`；board-local selector 由 typed client 先解析，不在 handler 里复制第二套语义。

host 未启动或 URL 不可达时，client 返回稳定的 `server_unavailable`；人类消息保持可行动，
例如“服务端不可用：请检查服务端 URL，并确认已运行 `kanban serve`”。

<!-- schema-doc-ignore: envelope 说明性示例，不绑定具体 endpoint root。 -->
```json
{"data": {}}
```

## 2. Host、health、boards 和 stats

| Method | Path | 语义 |
| --- | --- | --- |
| `GET` | `/health` | host、Turso path/fingerprint 和版本健康 |
| `GET` | `/api/v1/boards` | board 列表，可 `include_archived` |
| `POST` | `/api/v1/boards` | 创建 board，返回 `CreateBoardResponse` |
| `GET` | `/api/v1/boards/:board` | board 详情 |
| `POST` | `/api/v1/boards/:board/archive` | 显式归档 board，拒绝 active running work |
| `GET` | `/api/v1/boards/:board/columns` | 固定 status columns |
| `GET` | `/api/v1/stats` | board-scoped queue/claim/step 统计 |

`board_columns` 只做展示，不写第二套状态机。

## 3. Host-admin maintenance

以下操作都在唯一 host 的 Turso handle 内执行；backup/export 先 staging、校验、原子 rename，不覆盖既有目标或 symlink。

| Method | Path | 结果 |
| --- | --- | --- |
| `GET` | `/api/v1/maintenance/doctor` | schema/FK/task/run/projection diagnostics |
| `POST` | `/api/v1/maintenance/checkpoint` | WAL/checkpoint report |
| `POST` | `/api/v1/maintenance/backup` | verified backup、SHA-256、bytes |
| `POST` | `/api/v1/maintenance/export` | portable canonical JSONL |
| `POST` | `/api/v1/maintenance/import` | portable import；`replace=true` 走 verified backup + atomic canonical replace |
| `POST` | `/api/v1/maintenance/import-v30` | legacy SQLite v30 importer；需 `legacy-sqlite-import` feature，否则 `feature_not_available` |
| `POST` | `/api/v1/maintenance/vacuum` | host-owned compaction |
| `GET` | `/api/v1/maintenance/status` | owner lease、generation、dirty/error、job count |
| `POST` | `/api/v1/maintenance/run` | action `run|compact|rebuild|cleanup` |
| `POST` | `/api/v1/maintenance/rebuild` | projection rebuild |
| `POST` | `/api/v1/maintenance/cleanup` | 仅清理可重建派生内容 |

`projection_maintenance_owner` 保护并发；portable import 只写 canonical facts，提交后 enqueue
FTS/vector/graph rebuild。MCP 不提供这些 host-admin mutations；这不限制 MCP 对 search/graph/
vector 或 label atom-index domain `rebuild`/`sync` 的调用。

## 4. Boards、tasks、plans、steps、dependencies

### Board/task

| Method | Path | 语义 |
| --- | --- | --- |
| `GET` | `/api/v1/boards/:board/tasks` | status/priority/plan/assignee/query/pagination/sort 过滤 |
| `POST` | `/api/v1/boards/:board/tasks` | task create；支持 task/idempotency key、metadata、labels、depends_on |
| `GET` | `/api/v1/tasks/:task_id` | task aggregate；`include=details` 可返回 ontology/labels/dependencies/steps/runs/event meta |
| `PATCH` | `/api/v1/tasks/:task_id` | 只更新 service 允许的内容/排期/metadata 字段，不直接改 status |
| `POST` | `/api/v1/tasks/:task_id/execution-plan/not-required` | 显式 plan gate |

task list/search 默认排除 `archived`；所有 selector、board isolation、idempotency 和 dependency guard 由 service 处理。
状态筛选属于 `/api/v1/boards/:board/tasks` 的 query contract；搜索按状态的只读结果使用
`/api/v1/search/tasks/by-status`，两者不是第二套 task mutation path。

### Lifecycle

| Method | Path | 语义 |
| --- | --- | --- |
| `POST` | `/api/v1/tasks/:task_id/transitions/promote` | `todo|scheduled → ready` |
| `POST` | `/api/v1/tasks/:task_id/transitions/specify` | 补充 triage 规格并重算状态 |
| `POST` | `/api/v1/tasks/:task_id/transitions/claim` | 原子 `ready → running`，创建 run + event |
| `POST` | `/api/v1/tasks/:task_id/transitions/heartbeat` | matching token 延长 lease |
| `POST` | `/api/v1/tasks/:task_id/transitions/release` | matching token 取消 run 并回到 ready |
| `POST` | `/api/v1/tasks/:task_id/transitions/submit-review` | running → review，结束 active run |
| `POST` | `/api/v1/tasks/:task_id/transitions/complete` | running/review → done；校验 required steps |
| `POST` | `/api/v1/tasks/:task_id/transitions/block` | 非空 reason，必要时结束 run |
| `POST` | `/api/v1/tasks/:task_id/transitions/unblock` | blocked 后按 canonical facts 重算目标 |
| `POST` | `/api/v1/tasks/:task_id/transitions/reopen` | 保留完成历史并重算活动状态 |
| `POST` | `/api/v1/tasks/:task_id/transitions/reclaim` | expired/force claim 的显式回收 |
| `POST` | `/api/v1/tasks/:task_id/transitions/archive` | 显式归档 guard |

所有 lifecycle mutation 共享 `ApplicationService`；不提供任意目标状态 endpoint。

### Steps/dependencies

| Method | Path | 语义 |
| --- | --- | --- |
| `GET` | `/api/v1/tasks/:task_id/steps` | list execution steps |
| `POST` | `/api/v1/tasks/:task_id/steps` | create execution step |
| `PATCH` | `/api/v1/tasks/:task_id/steps/:step_id` | update step |
| `DELETE` | `/api/v1/tasks/:task_id/steps/:step_id` | remove step |
| `POST` | `/api/v1/tasks/:task_id/steps/:step_id/done` | mark step done |
| `POST` | `/api/v1/tasks/:task_id/steps/:step_id/skip` | skip step |
| `POST` | `/api/v1/tasks/:task_id/steps/:step_id/reopen` | reopen step |
| `GET` | `/api/v1/tasks/:task_id/dependencies` | list same-board parent edges |
| `POST` | `/api/v1/tasks/:task_id/dependencies` | add same-board parent edge |
| `DELETE` | `/api/v1/tasks/:child_task_id/dependencies/:parent_task_id` | remove edge；cycle/FK 在 service 拒绝 |

## 5. Comments、attachments、runs、events

| Method | Path | 语义 |
| --- | --- | --- |
| `GET` | `/api/v1/tasks/:task_id/comments` | list note/decision/signal comments |
| `POST` | `/api/v1/tasks/:task_id/comments` | create comment；task-local idempotency |
| `GET` | `/api/v1/tasks/:task_id/attachments` | list attachment metadata |
| `POST` | `/api/v1/tasks/:task_id/attachments` | staged file publish；checksum/path guard |
| `GET` | `/api/v1/tasks/:task_id/attachments/:attachment_id` | 重新校验 size/SHA 后下载 raw bytes |
| `DELETE` | `/api/v1/tasks/:task_id/attachments/:attachment_id` | `.trash/` reversible delete + event |
| `GET` | `/api/v1/tasks/:task_id/runs` | task runs |
| `GET` | `/api/v1/runs/:run_id` | run detail |
| `GET` | `/api/v1/runs/:run_id/log` | 固定 256 KiB bounded log snapshot |
| `GET` | `/api/v1/events` | append-only event list，`after` + `limit` cursor |
| `GET` | `/api/v1/stream/events` | SSE finite event snapshot/stream |

run 没有独立 create/update API；claim 创建 run，后续 lifecycle 同事务更新 run/event。未知 event kind 的 JSON payload 原样保留。

## 6. Labels、ontology、signals

### Labels

| Method | Path | 语义 |
| --- | --- | --- |
| `GET` | `/api/v1/boards/:board/labels` | board label list |
| `POST` | `/api/v1/boards/:board/labels` | board label create |
| `GET` | `/api/v1/tasks/:task_id/labels` | task label list |
| `POST` | `/api/v1/tasks/:task_id/labels` | task label add |
| `DELETE` | `/api/v1/tasks/:task_id/labels/:label_id` | task label remove |

### Ontology/atom/proposal

语义、atoms、atom-index、suggestions、proposals 和 ontology ledger 的当前 exact paths 为：

| Method | Path |
| --- | --- |
| `GET` | `/api/v1/boards/:board/labels/semantics` |
| `GET` | `/api/v1/boards/:board/labels/:label_id/semantics` |
| `PUT` | `/api/v1/boards/:board/labels/:label_id/semantics` |
| `DELETE` | `/api/v1/boards/:board/labels/:label_id/semantics` |
| `GET` | `/api/v1/boards/:board/labels/atoms` |
| `GET` | `/api/v1/boards/:board/labels/atoms/:atom_ref/explain` |
| `GET` | `/api/v1/boards/:board/labels/atom-index/status` |
| `POST` | `/api/v1/boards/:board/labels/atom-index/rebuild` |
| `GET` | `/api/v1/boards/:board/labels/atom-index/query` |
| `GET` | `/api/v1/tasks/:task_id/labels/suggestions` |
| `GET` | `/api/v1/tasks/:task_id/label-proposals` |
| `POST` | `/api/v1/tasks/:task_id/label-proposals` |
| `POST` | `/api/v1/tasks/:task_id/label-ontology/observations` |
| `GET` | `/api/v1/boards/:board/label-ontology/signals` |
| `GET` | `/api/v1/boards/:board/label-ontology/review` |
| `POST` | `/api/v1/boards/:board/label-ontology/actions` |
| `POST` | `/api/v1/boards/:board/label-ontology/apply/atom` |
| `POST` | `/api/v1/boards/:board/label-ontology/revert` |
| `POST` | `/api/v1/boards/:board/label-ontology/validate` |
| `GET` | `/api/v1/label-ontology/signals/:signal_id` |
| `GET` | `/api/v1/label-proposals/:proposal_id` |
| `POST` | `/api/v1/label-proposals/:proposal_id/accept` |
| `POST` | `/api/v1/label-proposals/:proposal_id/reject` |

每个 action 由 service 做 CAS、board guard、atom effects、review/validate/revert 和 event；index 是可重建派生状态。

### Generic signals

```text
GET  /api/v1/boards/:board/signals
POST /api/v1/boards/:board/signals
GET  /api/v1/boards/:board/signals/review
POST /api/v1/boards/:board/signals/confirm
POST /api/v1/boards/:board/signals/reject
POST /api/v1/boards/:board/signals/resolve
POST /api/v1/boards/:board/signals/supersede
GET  /api/v1/signals/:signal_id
```

record、backlink comment、review transition 和 `signal.reviewed` event 在同一事务提交。

## 7. Search、FTS、graph、vector、context

### Search

```text
GET  /api/v1/search/tasks
GET  /api/v1/search/tasks/by-status
GET  /api/v1/search/status
POST /api/v1/search/index/rebuild
POST /api/v1/search/index/sync
```

普通文本在 Turso FTS `task_search_fts` ready 时使用 match/score/highlight；exact `t_...`/`board#seq`/`#seq` 走 canonical selector。FTS stale/unavailable 时 service 回退 canonical SQL，并在 `search_meta` 标记 `backend`、`generation`、`fallback_reason`。

### Graph/entity

```text
GET  /api/v1/entities
PUT  /api/v1/entities
GET  /api/v1/entities/:uri
GET  /api/v1/graph/status
GET  /api/v1/graph/neighbors
GET  /api/v1/graph/query
POST /api/v1/graph/rebuild
POST /api/v1/graph/sync
GET  /api/v1/tasks/:task_id/neighborhood
GET  /api/v1/boards/:board/task-map
```

`PUT /api/v1/entities` 是 entity upsert；graph query 使用 canonical `entities`/relations 的
bounded BFS，包含 depth、dedup、cycle 和 board isolation。`neighborhood` 和 `task-map` 都是
只读聚合，不创建新的 graph facts。

### Vector/context

```text
GET  /api/v1/vector/status
POST /api/v1/vector/configure
POST /api/v1/vector/rebuild
POST /api/v1/vector/sync
GET  /api/v1/vector/query-chunks
GET  /api/v1/vector/query-label-atoms
GET  /api/v1/tasks/:task_id/context
```

vector32 查询使用 host Ollama embedding provider；provider outage 返回 typed degraded diagnostics。context pack 按 subject、lexical、graph、vector、budget 和 provenance 稳定合并；不可用 provider 不阻断可用 lexical/canonical 结果。

## 8. Contract 与验证边界

`kanban-protocol::endpoint_catalog()` 是 method/path/DTO/schema descriptor 的权威来源；真实 router、typed client、CLI/MCP/Desktop 和 fixture/adoption witness 必须逐项绑定。catalog 中的 `adopted` 只表示 protocol surface contract 已闭合，不能单独证明运行时 full/adoption gate 已运行。

本文件按领域解释语义；逐项 exact method/path 以 `endpoint_catalog()` 及生成的
`schemas/json-schema/draft-2020-12/surface-operations.json` 为准。`/api/v1/search/tasks/by-status`
是当前搜索查询 surface，task board 列表本身仍通过 `/api/v1/boards/:board/tasks` 的 query
完成状态筛选。

已有 server/service evidence 包括 task lifecycle、label round-trip、ontology action/revert、signal ledger、FTS capability、graph BFS/rebuild、vector fixture/degraded、context merge、maintenance import/replace 和 Desktop contract tests。完整 schema adoption、surface audit、full package、release 和 PR gate 不由本文档更新自动执行，结果见 parity ledger 的待验收清单。


---

# 文件：docs/SCHEMA_CONTRACTS.md

# JSON Schema 与机器契约

本文件描述 `kanban-protocol` 的 wire DTO、schema artifact、endpoint/surface catalog、fixture 和 adoption 证据边界。它不定义业务状态机、事务或权限；这些规则属于 `kanban-core`/`kanban-service`。

当前单 Host 路径为：

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP / SSE
kanban serve（唯一 host）
        ↓
kanban-service + kanban-core
        ↓
canonical Turso
```

## 1. 权威来源和状态

- `kanban-protocol`：DTO、event payload、error envelope、`endpoint_catalog()`、`surface_operation_catalog()` 和 schema registry 的唯一 Rust source。
- `kanban-server`：真实 HTTP/SSE producer/consumer；`kanban-client`：typed transport consumer；CLI/MCP/Desktop：入口 adapter。
- `schemas/`：由 `xtask`/脚本生成的提交产物，不手工编辑。
- `docs/API_SPEC.md`、`docs/CLI_SPEC.md`：用户可见 wire/output 行为；`docs/migration/turso-full-feature-parity.md`：跨 surface 事实和 gate ledger。

| `MigrationState` | 含义 | 最小证据 |
| --- | --- | --- |
| `planned` | 已定义边界但尚未生成 root | 精确 owner/operation |
| `generated` | root/fixture 已生成 | committed schema/fixture |
| `adopted` | 真实 producer/consumer 使用同一 contract | fixture、exact witness、route/adapter test |
| `excluded` | 不是 JSON document contract | 非空排除理由；不能同时声称有 schema |

`adopted` 是协议/表面 contract 状态，不等于整个 runtime、Desktop、schema-audit 或 full package gate 已运行。真实 route、CLI leaf、MCP tool 和 Desktop API 必须与 catalog 一一核对；catalog 本身不创建 route。

## 2. Active catalog 范围

当前 catalog 覆盖：

- API：health、boards（含 columns）、tasks/lifecycle（含 specify）、steps、comments、attachments、dependencies、entities（含 upsert）、graph（含 neighborhood/map）、search/context（含 index rebuild/sync）、labels/ontology、signals、runs/events、maintenance 和 vector；
- CLI：board/config/task/label/comment/context/attachment/dep/entity/graph/search/index/signal/vector、run/event、host-admin、hook/init/completion；原始 bytes 下载、completion、hook stdin/stdout、serve daemon 等非 JSON 输出保持 `excluded`。canonical leaf 以 Clap `get_name()` 为准，visible alias 不重复登记；
- MCP：`kanban-protocol::MCP_OPERATION_CATALOG` 的 machine-readable catalog，共 102 个
  tool，覆盖全部 101 个非 host-admin HTTP operation；`MCP_HOST_ADMIN_OPERATION_IDS` 明确
  禁止 12 个 host-admin operation。search/graph/vector 与 label atom-index 的 domain
  `rebuild`/`sync` 仍属于允许的 MCP surface；
- JSONL/metadata/config：portable import/export、decision/signal/ontology metadata、project config。

旧 backend/helper/projection 名称若仍出现在 historical catalog、migration fixture 或 archive 文档中，只表示 baseline/data lineage；active owner 已是 `kanban-service` 的 Turso FTS/vector32/BFS/worker，不能据此重新引入第二 backend。

## 3. Wire rules

1. 方言固定 JSON Schema Draft 2020-12；request/input 使用 deserialize settings，response/output 使用 serialize settings。
2. root ID 使用 `urn:kanban-tool:schema:<surface>:<semantic-name>:v1`；破坏性 wire change 提升 root version，不保留双轨输出。
3. schema 必须自包含，只允许本地 `#/$defs/...` `$ref`，不得有时间戳、绝对路径或网络 resolver。
4. `deny_unknown_fields`、required-nullable、显式 `null` 与省略字段必须反映真实 DTO。schema validation 不替代跨字段业务 guard。
5. path/query/headers/body/success/sse、cardinality（`RequiredOne`、`OptionalOne`、`RepeatedOrdered`）由 endpoint descriptor 精确记录；不使用 wildcard/family 逃避审计。
6. status transition、claim token、dependency cycle、board isolation、idempotency、transaction atomicity 和 provider degraded 由 service/server/client tests 证明，而不是由 JSON Schema 推断。

## 4. Artifacts 和 adoption witness

```text
schemas/
  fixtures/{api,cli,jsonl,metadata,sse}/
  json-schema/draft-2020-12/
    operations.json
    surface-operations.json
    manifest.json
    api/ cli/ jsonl/ metadata/ sse/
```

`operations.json` 是 semantic contract inventory；`surface-operations.json` 是精确 transport/adapter inventory；`manifest.json` 绑定 root、fixture 和 digest。连续生成必须 byte-identical。

每条 adoption witness 记录 `operation`、`contract_id`、`surface`、`direction`、`package`、`test_target`、`exact_test`。producer 必须来自真实 DTO 序列化；consumer 必须从 committed fixture 进入真实 route/handler/adapter；不能用一个高层 exercise helper 同时代替两侧证据。

当前代码已有 protocol schema tests、server fixture adoption tests、CLI contract tests、MCP inventory test、Desktop contract tests 和 service capability/lifecycle tests。尚未执行的 `schema-surface-audit`、`schema-adoption-witness`、`schema-audit-closed` 或 full package gate 必须按实际输出更新 ledger，不能从 artifact 生成成功推断为 green。

## 5. 依赖与单 Host gate

active runtime 依赖方向：

```text
kanban-server → kanban-service → turso
kanban-cli / kanban-mcp / Desktop → kanban-client → localhost HTTP
xtask → kanban-protocol[schema] + cargo metadata
```

CLI、MCP、Desktop、fixture 和 test-support 不得依赖 `kanban-service` 的数据库-owning path、SQLite importer feature、第二 store 或 `xtask` runtime。`legacy-sqlite-import` 只在 host/service feature 中编译，执行的是只读 legacy source import，不是 active SQLite backend。

`scripts/check-single-host-dependencies.py` 与 schema dependency policy 负责 package/feature/target-specific 依赖隔离；失败时修复 manifest/source，不通过删除契约或伪造 witness 掩盖。

## 6. Recipe 和验证顺序

```text
just schema-generate
just schema-check
just schema-docs
just schema-surface-audit
just schema-adoption-witness
just schema-contract
```

`xtask schema generate/check/audit/witnesses` 是离线工具；`just schema-docs` 还检查 `KANBAN_SPEC_BUNDLE.md`、JSON fence marker 和 fixture 映射。只改文档时至少运行 `just diff-check`、`just spec-bundle-check`；schema marker/fixture 变化再运行 `just schema-docs`。写 Cargo target 的 recipe 统一经 `scripts/cargo-build-lock.sh`。

`schema-audit-closed` 只有在 source inventory 无 `planned/generated`/未闭合 obligation 且所有 exact witness 实际通过时才可称 closed；本次文档任务不预先宣称该状态。release/package/full gate、push、PR 和发布不属于 schema 文档同步。

## 7. 新 operation 闭环

1. 在 `kanban-protocol` 定义 DTO、root、endpoint/surface descriptor 和 migration state；
2. 添加 valid/invalid fixture 与 producer/consumer exact witness；
3. 贯通 service → server → client → 需要的 CLI/MCP/Desktop adapter；
4. 运行受影响 package tests、`just schema-check`、`just schema-surface-audit`、`just schema-adoption-witness` 和 `just diff-check`；
5. 若取消 operation，改为 `excluded` 并保留理由，不伪造 route 或 fixture。

完整 parity 还要求 FTS/vector/graph/context 能从 canonical facts rebuild、migration/rollback/recovery 可验证、旧 sidecar 不在 active workspace；具体 owner/test/gate 见 [`migration/turso-full-feature-parity.md`](docs/migration/turso-full-feature-parity.md)。


---

# 文件：docs/ADR.md

# 架构决策记录

本文件按时间记录 SPEC 的关键架构决策。每条 ADR 的背景、统计值和迁移状态都是
决策时快照，不会随着实现自动改写；当前行为和实时契约覆盖以对应的
`docs/*_SPEC.md`、`docs/SCHEMA_CONTRACTS.md` 与 `docs/migration/turso-full-feature-parity.md`
为准。ADR-0023 记录当前最终 Turso 架构；历史 ADR 中的旧 SQLite/backend/helper 名称不
表示 active workspace 仍保留这些路径。

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
- done
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

历史决策（外部 projection lane 已退出 active workspace）

当前 `entities`、`relation_predicates`、`entity_relations` 和检索/向量派生语义由
`kanban-service` 在 Turso host 内提供；旧 Tantivy/Oxigraph/LanceDB/helper 仅保留为
迁移证据。当前 owner 与 rebuild 规则见 ADR-0023、`DATA_MODEL.md`。

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

历史 provider 边界；当前 ontology/label proposal owner 已收敛到 `kanban-service`，
Turso `vector32`/host Ollama 只提供可降级、可重建的派生能力。文中
`kanban-sqlite`、LanceDB 和旧 runtime 名称只描述决策快照。

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

历史决策（不建立独立 ontology graph）；当前仍不建立 ontology 专属 graph mutation path，
但通用 entities/relations 的 canonical BFS 已由 `kanban-service` 提供。Ontology ledger
继续由 Turso facts/service actions 负责，graph 故障不会改变 canonical ontology。

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
- 删除 label identity 时永不隐式删除 `label_semantics` / `label_atoms`；force 只允许移除 task
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

在 `kanban-protocol` 默认 feature 中保存 API/SSE 描述符；server router 以
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

历史快照；当前 task-read surface 以 ADR-0023、`docs/API_SPEC.md` 和
`kanban-protocol::endpoint_catalog()` 为准

### 背景

两个 task-read 端点需要证明各自精确消费 path/query，同时避免 handler 或多个 parser
重复拥有 raw query。

### 决策

`GET /api/v1/boards/:board/tasks` 使用唯一的 task-list path/query DTO；状态筛选通过同一
query contract 表达，搜索按状态的只读结果由 `/api/v1/search/tasks/by-status` 负责。server
使用类型化 Axum extractor 从一次 raw URI 进入共享有序解析器；handler 只接收已绑定的
request，不持有第二个 raw source。
- Query 语法：只有 `status`、`priority`、`label`、`plan_filter` 是
  `RepeatedOrdered`；其余标量重复、未知 key 与旧 `search` 别名均返回
  `400 invalid_input`。54 对上限由 9/4/3/32 个重复参数预算加 6 个标量参数推导；
  raw query 上限为 8192 字节。`q` 是唯一文本搜索 key。label 会 trim Unicode 边缘空白，
  但纯 Unicode 空白失败关闭；percent/UTF-8、枚举、priority、limit、offset 和 sort 边界由
  真实 router URI 矩阵固定。
contract 仍有 DTO-to-fixture producer 和 fixture-to-real-router consumer；非默认 board 哨兵
证明真实 path 消费。当前 route/catalog 对齐测试覆盖类型化 extractor、raw URI 单一来源、
`&path.board` 到 `list_tasks_page` 的实参和 query 边界。producer/consumer 区域保护只证明
当前源码区域直接分离，不把任意未来共同 helper 间接层夸大为变异完备证明。

### 影响

Desktop/Web/CLI 的 HTTP 调用方必须使用上述语法；现有 Desktop 调用方已使用 `q`
  并保留重复参数顺序。Turso service 的防御性上限直接引用唯一 application 权威，
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

已接受（当前唯一 host/owner 决策）

### 后续决策

本 ADR 继续约束唯一 host、typed localhost client、共享 mutation path、Turso ownership 和
dispatcher 边界。它作出时的“阶段性后置项”已经由完整功能重构接入当前 service path；具体
owner、入口、迁移规则、已有测试和未运行 gates 以
[`docs/migration/turso-full-feature-parity.md`](docs/migration/turso-full-feature-parity.md)、
[`ARCHITECTURE.md`](docs/ARCHITECTURE.md) 和 [`DATA_MODEL.md`](docs/DATA_MODEL.md) 为准。

当前实现已将 application service 与 Turso persistence 合并为 `kanban-service`，并保持
`kanban-protocol` 作为独立 wire/schema crate。该收敛不改变 single-host ownership，也不能用
“暂不支持”替代 parity ledger 的闭合。

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
   generalized mutation receipt、第二 projection control plane 或旧 API 兼容层。labels、
   signals、search、graph、vector、context、projection 和 importer 若提供能力，必须复用
   当前 `kanban-service`/Turso host path。

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
- 未注册或显式 feature-disabled 的路径才返回 `feature_not_available`；当前已接入的 labels、
  signals、search、graph、vector、context、projection 和 maintenance 不得用该错误码
  代替实现。最终 adoption/full 状态仍以 parity ledger 的实际 gate 为准。

### 非目标

本决策不定义多用户/RBAC/云同步、公网 host、自动 server supervision、跨机器 worker 或
发布/PR 工作流。`legacy-sqlite-import` 只是 host 的显式只读导入 feature，不是第二
canonical backend。FTS/vector/graph/context/projection rebuild、backup/maintenance 和
Desktop 历史 surface 已有当前 owner，但完成状态仍由 parity ledger 的实际测试/gate 判断，
不能从本 ADR 的决策文字推断为 release ready。

---

## ADR-0023：最终 Turso 全功能 workspace 收敛

### 状态

已接受；schema adoption、surface audit、full package 和 Desktop check 仍须以实际命令结果
单独记录，不由本 ADR 标为通过。

### 背景

baseline `6ea277` 同时存在窄 queue、外部 projection/helper、独立 backend、多个 adapter
路径和十个 Desktop 视图。继续保留这些进程会让 canonical owner、状态机、依赖和导入语义
出现第二套事实。当前实现已经把完整业务域接回 Turso single-host，文档需要固定最终边界
并明确未运行的验收 gates。

### 决策

1. active 产品单元固定为七个 Rust crate：`kanban-core`、`kanban-service`、
   `kanban-protocol`、`kanban-client`、`kanban-server`、`kanban-cli`、`kanban-mcp`，加
   Desktop `kanban-desktop` 和私有 `xtask`；不新增 backend/helper sidecar。
2. `kanban serve` 是唯一 Turso host/owner。CLI、MCP、Desktop 只能经 typed localhost
   HTTP/SSE；所有 mutation/query 经过 `ApplicationService`、`kanban-core` guard、Turso
   transaction 和 protocol DTO。
3. 搜索使用 Turso FTS `task_search_fts`，向量使用 Turso `vector32` + host Ollama provider，
   图和 context 使用 `kanban-service` canonical relation + bounded BFS/merge。FTS/vector/
   graph/context/projection 可删可重建，provider/index 故障只产生 degraded diagnostics。
4. 数据迁移保留两条路径：Turso v1→v2 原地升级（verified sibling backup + transaction
   rollback）和 portable JSONL/legacy SQLite v30 导入（`import_journal`、staging、fingerprint、
   derived rebuild；v30 仅显式 `legacy-sqlite-import` feature）。导入不创建第二 runtime backend。
5. baseline 的旧 backend、external projection、helper protocol 和 sidecar 已删除；相关
   release/projection 文档只允许标为 historical archive，不得成为 active runbook 或 release
   gate。
6. release、push、PR、merge、发布和外部协调不是本次 architecture/parity 文档的验收条件。
7. API method/path 以 `kanban-protocol::endpoint_catalog()` 为准，CLI canonical leaf 以
   Clap `get_name()` 与 `surface_operation_catalog()` 对齐。当前新增的 board columns、entity
   upsert、task specify、graph neighborhood/map 和 search index rebuild/sync 都属于 active
   surface；visible alias 不形成第二条 contract operation。

### 影响

- 七个 crate、Desktop 和 `xtask` 的依赖方向可由 manifest 与 single-host dependency gate
  审计；不存在第二产品 runtime。
- HTTP、CLI、MCP、Desktop 的 domain surface 可以按 parity ledger 逐域核验，catalog adopted
  不再被误读为 full runtime green。
- canonical Turso 事实、event、run/claim 和导入 journal 的一致性优先于 projection
  availability；FTS/vector/graph/context 的重建和 degraded 状态可观察、可回放。

### 非目标

不引入多用户/RBAC/云同步、公网访问、自动 server supervision、自定义 IPC 或另一套
projection control plane。历史 sidecar 文档不承诺兼容行为，也不替代实际 gate 结果。


---

# 文件：crates/kanban-service/src/schema.rs

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
  kind TEXT NOT NULL DEFAULT 'note' CHECK(kind IN ('note', 'decision', 'signal')),
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
CREATE UNIQUE INDEX IF NOT EXISTS idx_projection_jobs_dedupe ON projection_jobs(target, dedupe_key);
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
) AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE NEW.entity_uri = 'kb://task/' || id AND board_id IS NEW.board_id
) AND NOT EXISTS (
  SELECT 1 FROM label_atoms WHERE NEW.entity_uri = 'kb://label-atom/' || id AND board_id IS NEW.board_id
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
) AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE NEW.entity_uri = 'kb://task/' || id AND board_id IS NEW.board_id
) AND NOT EXISTS (
  SELECT 1 FROM label_atoms WHERE NEW.entity_uri = 'kb://label-atom/' || id AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'projection_jobs reference board mismatch');
END;

-- canonical task event 成功写入后，自动为可重建 vector projection 留下 job。
-- 该触发器只写 projection_jobs/projection_state，不改变 tasks 的事实状态。
CREATE TRIGGER IF NOT EXISTS task_events_vector_projection_enqueue
AFTER INSERT ON task_events
WHEN NEW.task_id IS NOT NULL
BEGIN
  INSERT INTO projection_jobs(
    board_id, source_event_id, target, entity_uri, dedupe_key, operation,
    payload_json, status, attempts, max_attempts, next_attempt_at,
    created_at, updated_at
  ) VALUES (
    NEW.board_id, NEW.id, 'vector_tasks',
    'kb://task/' || NEW.task_id,
    'vector_tasks:kb://task/' || NEW.task_id || ':upsert',
    'upsert',
    json_object('task_id', NEW.task_id), 'pending', 0, 10, NULL,
    NEW.created_at, NEW.created_at
  )
  ON CONFLICT(target, dedupe_key) DO UPDATE SET
    source_event_id=excluded.source_event_id,
    payload_json=excluded.payload_json,
    status=CASE WHEN projection_jobs.status='done' THEN 'pending' ELSE projection_jobs.status END,
    next_attempt_at=NULL,
    updated_at=excluded.updated_at;
  UPDATE projection_state
  SET dirty=1, lifecycle_status=CASE WHEN lifecycle_status='ready' THEN 'degraded' ELSE lifecycle_status END,
      updated_at=NEW.created_at
  WHERE projection = 'vector_tasks' OR projection = 'vector_label_atoms';
END;

CREATE TRIGGER IF NOT EXISTS label_atoms_vector_projection_enqueue
AFTER INSERT ON label_atoms
BEGIN
  INSERT INTO projection_jobs(
    board_id, source_event_id, target, entity_uri, dedupe_key, operation,
    payload_json, status, attempts, max_attempts, next_attempt_at,
    created_at, updated_at
  ) VALUES (
    NEW.board_id, NULL, 'vector_label_atoms',
    'kb://label-atom/' || NEW.id,
    'vector_label_atoms:kb://label-atom/' || NEW.id || ':upsert',
    'upsert',
    json_object('atom_id', NEW.id), 'pending', 0, 10, NULL,
    NEW.created_at, NEW.created_at
  )
  ON CONFLICT(target, dedupe_key) DO UPDATE SET
    payload_json=excluded.payload_json,
    status=CASE WHEN projection_jobs.status='done' THEN 'pending' ELSE projection_jobs.status END,
    next_attempt_at=NULL,
    updated_at=excluded.updated_at;
  UPDATE projection_state
  SET dirty=1, lifecycle_status=CASE WHEN lifecycle_status='ready' THEN 'degraded' ELSE lifecycle_status END,
      updated_at=NEW.updated_at
  WHERE projection = 'vector_label_atoms';
END;

CREATE TRIGGER IF NOT EXISTS label_atoms_vector_projection_update
AFTER UPDATE OF text, content_hash, board_id ON label_atoms
BEGIN
  INSERT INTO projection_jobs(
    board_id, source_event_id, target, entity_uri, dedupe_key, operation,
    payload_json, status, attempts, max_attempts, next_attempt_at,
    created_at, updated_at
  ) VALUES (
    NEW.board_id, NULL, 'vector_label_atoms',
    'kb://label-atom/' || NEW.id,
    'vector_label_atoms:kb://label-atom/' || NEW.id || ':upsert',
    'upsert',
    json_object('atom_id', NEW.id), 'pending', 0, 10, NULL,
    NEW.created_at, NEW.updated_at
  )
  ON CONFLICT(target, dedupe_key) DO UPDATE SET
    payload_json=excluded.payload_json,
    status=CASE WHEN projection_jobs.status='done' THEN 'pending' ELSE projection_jobs.status END,
    next_attempt_at=NULL,
    updated_at=excluded.updated_at;
  UPDATE projection_state
  SET dirty=1, lifecycle_status=CASE WHEN lifecycle_status='ready' THEN 'degraded' ELSE lifecycle_status END,
      updated_at=NEW.updated_at
  WHERE projection = 'vector_label_atoms';
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

/// FTS index 独立于 canonical schema checksum，但 `kanban-service` 必须启用
/// Turso 的 `fts` feature；初始化若不能创建该索引，会把 capability 记录为不可用。
pub(crate) const FTS_SCHEMA: &str =
    "CREATE INDEX IF NOT EXISTS task_search_fts ON retrieval_documents USING fts (content);";

/// v2 已有数据库也必须补上 canonical event -> FTS outbox 的触发器。它们不改变
/// canonical table shape，因此初始化时可幂等执行，不需要另起 migration version。
pub(crate) const PROJECTION_TRIGGER_SCHEMA: &str = r#"
DROP TRIGGER IF EXISTS projection_jobs_board_guard_insert;
DROP TRIGGER IF EXISTS projection_jobs_board_guard_update;
CREATE TRIGGER projection_jobs_board_guard_insert
BEFORE INSERT ON projection_jobs
WHEN (NEW.target != 'fts' AND NEW.source_event_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_events WHERE id = NEW.source_event_id AND board_id IS NEW.board_id
)) OR (NEW.target != 'fts' AND NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
) AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE NEW.entity_uri = 'kb://task/' || id AND board_id IS NEW.board_id
) AND NOT EXISTS (
  SELECT 1 FROM label_atoms WHERE NEW.entity_uri = 'kb://label-atom/' || id AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'projection_jobs reference board mismatch');
END;
CREATE TRIGGER projection_jobs_board_guard_update
BEFORE UPDATE OF board_id, source_event_id, entity_uri ON projection_jobs
WHEN (NEW.target != 'fts' AND NEW.source_event_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_events WHERE id = NEW.source_event_id AND board_id IS NEW.board_id
)) OR (NEW.target != 'fts' AND NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
) AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE NEW.entity_uri = 'kb://task/' || id AND board_id IS NEW.board_id
) AND NOT EXISTS (
  SELECT 1 FROM label_atoms WHERE NEW.entity_uri = 'kb://label-atom/' || id AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'projection_jobs reference board mismatch');
END;
CREATE TRIGGER IF NOT EXISTS task_events_projection_job_insert
AFTER INSERT ON task_events
WHEN NEW.task_id IS NOT NULL
BEGIN
  INSERT OR IGNORE INTO projection_jobs(
    board_id, source_event_id, target, entity_uri, dedupe_key, operation,
    payload_json, status, attempts, max_attempts, created_at, updated_at
  ) VALUES (
    NEW.board_id, NEW.id, 'fts', 'kb://task/' || NEW.task_id,
    'fts:' || NEW.board_id || ':' || NEW.task_id || ':' || NEW.id,
    'upsert', json_object('task_id', NEW.task_id), 'pending', 0, 10,
    NEW.created_at, NEW.created_at
  );
END;
CREATE TRIGGER IF NOT EXISTS tasks_projection_job_delete
BEFORE DELETE ON tasks
BEGIN
  INSERT OR IGNORE INTO projection_jobs(
    board_id, target, entity_uri, dedupe_key, operation, payload_json,
    status, attempts, max_attempts, created_at, updated_at
  ) VALUES (
    OLD.board_id, 'fts', 'kb://task/' || OLD.id,
    'fts:' || OLD.board_id || ':' || OLD.id || ':delete',
    'delete', json_object('task_id', OLD.id), 'pending', 0, 10,
    strftime('%s','now') * 1000, strftime('%s','now') * 1000
  );
END;
"#;

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

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
和 attachment staging 的 schema readiness，并探测 `fts`/`vector32` capabilities；本维护切片
已把 doctor/checkpoint/verified backup/portable export-import/vacuum 以及 projection
owner、generation、recovery status 接入 application → HTTP → typed client → CLI。SQLite v30
importer 的 typed host-admin route/client/CLI wiring 已接入，但实际只读 importer 逻辑由
`legacy-sqlite-import` feature 提供；未启用 feature 时 fail-closed。真实 projection worker
和 Desktop maintenance controls 仍需后续纵向切片。

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
再开始事务 migration。完整字段和指纹见 [`DATA_MODEL.md`](DATA_MODEL.md)。

派生能力统一由 Turso/host 提供：

- **FTS**：`retrieval_documents` 上的 `task_search_fts` Turso `USING fts` index；canonical
  task event/delete 在同一事务写入 `projection_jobs`，host projection service 使用
  `fts_match`、`fts_score`、`fts_highlight`，移除外部 Tantivy 作为事实/检索 owner。查询在
  projection 未 ready、落后或 provider 失败时回退 canonical SQL，并通过 search meta 标记
  stale/fallback reason。
- **Vector**：Ollama 只作为 host 内 provider；embedding 的批处理、重试、model/dimension/
  fingerprint、缓存和降级语义由 service/worker 管理；向量和 cosine 查询在 Turso
  `vector32` 中完成，移除 LanceDB。
- **Graph**：`entities`、`relation_predicates`、`entity_relations` 是 canonical；Turso
  0.7.2 不依赖 recursive CTE，service 执行有深度上限、环检测和 board isolation 的批量
  BFS，移除 Oxigraph。
- **Projection**：canonical mutation 在事务中写事实、event 和 projection job；FTS、vector、
  graph/context 都可删除后重建，不接受旧 helper subprocess/protocol 或独立 control plane。

当前 schema 能力已经有 capability probe 和 fail-closed shape validation；task search 的
store → application → HTTP → typed client → CLI/MCP 纵向切片已接通；maintenance 查询和
host-admin mutation 由 `kanban-server` 唯一 owner 串行执行。其他领域 service、worker 和
Desktop 视图仍按纵向 slice 实现，文档不把 schema 存在等同于用户入口可用。

## 5. 迁移与主机管理

目标 `kanban-service` 内置两条数据路径：

1. **Turso v1 原地升级**：host 独占数据库；先生成并校验 sibling backup，再执行事务化
   migration。失败保留旧 schema/data，可再次启动；重复启动幂等。
2. **portable/SQLite 导入**：当前 CLI 通过 localhost 管理 API 请求运行中的 host，portable
   JSONL 只导入 canonical facts，目标保留 host bootstrap board/columns；事务阶段写入
   `import_journal`，失败标记 `failed`，派生 FTS/vector 不迁移，导入后由 rebuild 处理。
   SQLite v30 的 route/client/CLI 入口已预留；只读 schema、attachment staging、原子文件发布和
   崩溃 resume 仅在显式启用 `legacy-sqlite-import` feature 后提供，默认构建保持不可用。

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

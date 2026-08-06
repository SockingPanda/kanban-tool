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

schema family 为 `kanban.turso`，v1/v2 lineage 与 exact shape 由 `schema.rs`、`migration.rs` 和 schema identity 校验。v2 包含 queue/history、labels/ontology/signals、entities/relations、retrieval、projection、import journal 和 attachment staging；字段、约束、fingerprint 见 [`DATA_MODEL.md`](DATA_MODEL.md)。

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

CLI domain 命令、MCP protocol machine-readable catalog（103 个 tool，覆盖 102 个非
host-admin HTTP operation）和 Desktop 十个导航视图都通过 typed client 接入；catalog 明确
拒绝 12 个 host-admin operation。Desktop Tauri command 不持有 Turso；claim token 仅在会话
状态中保存。维护操作显示 host 返回的 phase、degraded 和 `restart_required`，不凭 UI 状态
推断 canonical 成功。

dispatcher 只有传入 `--dispatcher-profile` 才启动，轮询 `ready`、复用 claim/heartbeat/finish、优雅停止等待当前 worker；projection worker 与 host 共用 lifecycle/maintenance lease，按 job generation 处理 FTS/vector/relations/context，并通过共享 `ApplicationService::vector_worker_tick` 协调 Vector provider。两类 worker 都不打开第二数据库、不维护第二状态机、不直接改 adapter DTO。

## 7. 证据与停止边界

每个纵向 slice 必须闭合 `core/service → protocol/server → client → CLI/MCP/Desktop → tests/docs`，并提供 schema/fixture、真实 producer/consumer、负向约束和 rebuild/fallback 证据。旧 sidecar 删除后只保留历史归档，不以 shim 恢复。

本次文档任务不运行或伪造 adoption/full/schema-audit/release gate；ledger 中将它们标为待运行，并记录已有测试名称。发布、push、PR、merge 不属于架构收敛范围。

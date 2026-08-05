# Turso full-feature parity ledger

本账本比较两个明确快照：

- feature baseline：`6ea277583e51ea010aa6739a53091337676b4cff`（下文简称
  baseline）；
- implementation starting point：`62c9adf`（下文简称起点）；
- 当前实施状态：以本文件所在工作树为准，schema 层已包含提交 `3b61aa5`。

`current` 表示当前实现有真实公开入口并走 Turso single-host `kanban-service` path；`partial`
表示只剩内部实现、孤立 adapter/contract 或仅保留了一个子能力；`missing` 表示没有
当前公开入口。`new` 表示 baseline 没有而当前实现新增的公开能力。

## 计数口径与证据

| 面 | baseline | 当前实现 | 计数口径 |
|---|---:|---:|---|
| Turso 表 | 35 | 38 | baseline 按 `migrations/001..030` replay 后的最终表名，忽略 `_new` 瞬态表；当前实现按 `crates/kanban-store-turso/src/schema.rs` 的最终 schema 统计。38 张表还包含 schema identity、capability、import journal 等单主机治理表，因此不能用数量相等代替逐表语义验收。 |
| 普通 index | 61 | 45 | baseline 按最终 `CREATE/DROP INDEX` 状态解析；当前实现另有 1 个 Turso FTS index。名称与形状按目标查询契约重新设计，不要求复刻旧 index 名。 |
| trigger | 13 | 22 | 当前 trigger 负责 nullable 关联的 board isolation、附件路径和跨表防护；其余不变量由 FK、CHECK、唯一约束与 service transaction 共同承担。 |
| HTTP operation | 84 | 27 | baseline 取 `crates/kanban-contract/src/endpoint.rs` 的 `EndpointDescriptor`；当前实现取 `crates/kanban-server/src/http/operations/**` 的真实 `route(method,path)`。catalog descriptor 不能当作已注册 route。 |
| CLI leaf | 126 | 27（其中 26 可用） | 递归展开 baseline `crates/kanban-cli/src/args.rs` 和当前 clap enum；声明的 alias/variant 按命令名计，`Init` 计入但实际返回 `feature_not_available`，`FeatureNotAvailable` 外部 catch-all 不计 leaf。 |
| MCP tool | 0 | 24 | baseline 没有 `crates/kanban-mcp`；当前实现按 `#[tool(name = ...)]` 及 `main.rs` 稳定 inventory 测试。 |
| Desktop view | 10 | 6 可导航 | baseline `OperatorView` 全部 10 项；当前实现以 `NavigableOperatorView`/`sidebarViews` 为公开导航，4 项仍被排除。 |

可靠提取命令（均只读）：

```bash
git show 6ea277583e51ea010aa6739a53091337676b4cff:crates/kanban-contract/src/endpoint.rs \
  | rg 'operation_id:|method:|path:'
rg -n 'route\s*\(' crates/kanban-server/src/http/operations
git grep -n -E 'CREATE (TABLE|INDEX|TRIGGER)|DROP (TABLE|INDEX|TRIGGER)|ALTER TABLE' \
  6ea277583e51ea010aa6739a53091337676b4cff -- migrations
rg -n '#\[tool\(|name = "' crates/kanban-mcp/src
git show 6ea277583e51ea010aa6739a53091337676b4cff:apps/desktop/src/features/navigation/view-types.ts
nl -ba apps/desktop/src/features/navigation/view-types.ts
```

## 1. 数据库表与 schema objects

### 1.1 保留的 canonical core（10/10，`current`）

| legacy feature | target owner / public entry | migration rule | acceptance | status |
|---|---|---|---|---|
| `schema_migrations`, `boards`, `board_columns`, `tasks`, `task_execution_plans`, `task_steps`, `task_dependencies`, `task_runs`, `task_comments`, `task_events` | `crates/kanban-store-turso/src/schema.rs:2-200`; `TursoStore::initialize` in `crates/kanban-store-turso/src/db.rs` | 目标 owner：`kanban-service` schema/operation；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop。沿 Turso lineage v1→full migration 逐版推进；baseline 旧 v30 由 `kanban-service` legacy SQLite importer（经 host-admin HTTP/CLI，不在 `xtask`）作为 logical export/import 输入回填事实，不把旧 SQLite 版本当 Turso runtime migration chain。保留表名不等于保留全部列、约束或行为。 | `TursoStore::initialize` 成功创建 10 张表、9 个默认 column；service/server/CLI/MCP/Desktop 的 retained rows 能读写并通过单 host。 | current |

### 1.2 baseline 扩展表的 schema 已恢复，公开能力仍待闭合（`partial`）

| legacy feature（逐项） | legacy evidence | target owner / public entry | migration rule | acceptance | status |
|---|---|---|---|---|---|
| `app_settings`, `task_attachments`, `task_subtasks` | `migrations/001_initial.sql`, `022_task_subtasks_execution_plans.sql`, baseline `docs/DATA_MODEL.md` | 目标 owner：`kanban-service` configuration/attachment/step facts；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop detail | Turso v2 已建立三张表、FK/路径 guard 与 attachment staging/import journal；attachment list/add/download/remove 已闭合 canonical host path，旧 v30 逻辑导入仍独立推进。 | 新表、FK、DTO、各 attachment adapter operation 和 metadata/file round-trip、checksum、坏路径、缺失文件、重复 ID 与 recoverable delete 测试齐全；legacy importer 继续保持独立边界。 | current |
| `labels`, `task_labels` | `migrations/001_initial.sql`；baseline `crates/kanban-sqlite/src/service/tasks.rs` | 目标 owner：`kanban-service` label facts/transactions；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop task detail（对应 `/api/v1/boards/:board/labels` 与 task label routes） | Turso v2 已恢复 identity/binding、board composite FK 和查询 index；下一步实现 service transaction、`kanban-protocol` DTO adoption 与各 adapter。 | label create/list/add/remove 的真实 route、命令、tool、UI mutation 与跨 board/幂等测试齐全。 | partial |
| `label_semantics`, `label_atoms`, `label_atom_index_boards`, `label_semantic_proposals` | `migrations/004..011`; baseline `service/label_semantics.rs`, `label_proposals.rs` | 目标 owner：`kanban-service` ontology facts + Turso vector projection；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop ontology | Turso v2 已恢复 semantics/atom/proposal 事实表；atom index 仍是可重建派生层，导入时先导入事实再重建 index。 | 语义 CAS、proposal lifecycle、atom explain/index rebuild/query 均有 service path、`kanban-protocol` fixture/adoption tests。 | partial |
| `label_ontology_observations`, `label_ontology_signals`, `label_ontology_actions`, `label_ontology_action_signals`, `label_ontology_action_atom_effects` | `migrations/012..021`, baseline `service/label_ontology.rs` | 目标 owner：`kanban-service` ontology ledger；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop ontology workbench | Turso v2 已按 action/validation/source-signal 依赖恢复事实表、约束、index 与 board guard；v30 importer 和 mutation adapter 尚未闭合。 | action/validation/revert/retarget 与 ledger back-link 事务测试、schema fixture 和 Desktop review acceptance 齐全。 | partial |
| `entities`, `relation_predicates`, `entity_relations`, `index_outbox`, `derived_store_state` | `migrations/002_knowledge_substrate.sql`, `024..025`, baseline `service/entities.rs`, `projections.rs` | 目标 owner：`kanban-service` entity/relation facts、Turso BFS/vector/FTS 与 host projection worker；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop map/context | Turso v2 已恢复 entity/relation 事实；旧 `index_outbox`/`derived_store_state` 不复制为第二套协议，改由 `projection_jobs`/`projection_state` 承担单主机 worker 状态。 | projection replay、Turso BFS/vector/FTS rebuild、derived status 与 context fallback 在完整 single-host/host-admin acceptance 中闭合。 | partial |
| `signal_observations`, `signals` | `migrations/024_signal_ledger.sql`, `025_generic_signal_ledger.sql`; baseline `service/signals.rs` | 目标 owner：`kanban-service` signal ledger；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop Signals workbench | Turso v2 已恢复 observation/signal 事实、comment backlink、状态约束和 board guard；record/list/review/show 与四个 lifecycle action 均复用同一事务 path。 | signal record/list/review/confirm/reject/resolve/supersede、HTTP/CLI/MCP DTO、backlink round-trip 与跨 board/幂等测试齐全；Desktop workbench 接入仍由 shell lane 收尾。 | current/partial |
| `projection_database`, `projection_store_state`, `projection_deliveries`, `projection_maintenance_owner` | `migrations/026..030_projection_*.sql`; baseline `service/projection_v2.rs`, `maintenance_runtime.rs` | 目标 owner：`kanban-service` projection worker + `host-admin` maintenance；public entry：`kanban-server`/`kanban-client`、CLI、Desktop Maintenance view | 不迁移 helper subprocess 的 database/delivery 协议；Turso v2 以 `projection_jobs`、`projection_state`、`projection_maintenance_owner` 建立唯一 host 内部 worker 模型。旧派生状态由 canonical facts 重建。 | owner lease、generation publish/recovery、cleanup/doctor 与 projection capability acceptance 全部通过。 | partial |

### 1.3 objects、约束与形状变化（`partial` 或 `missing`）

| legacy feature | legacy evidence | target owner / public entry | migration rule | acceptance | status |
|---|---|---|---|---|---|
| 61 baseline indexes → 45 Turso 普通 index + 1 FTS index | baseline migration replay；当前 `schema.rs` v2 schema | owner：`kanban-service` schema/query layer；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop retained queries and restored feature indexes | index 按目标查询契约重新设计；FTS 使用 Turso `USING fts`，其余索引服务 canonical/worker 查询，不复刻外部 Tantivy/LanceDB/Oxigraph 的内部对象。 | capability tests 已证明 FTS 写入、更新、删除、排序与 highlight；每个恢复 feature 仍需独立查询与 adapter acceptance。 | partial |
| 13 baseline triggers → 22 Turso trigger | baseline `migrations/020..030`; 当前 `schema.rs` v2 schema | owner：`kanban-service` schema + transaction guards；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop retained mutation and restored relation/projection operations | nullable 跨表引用和附件路径使用 trigger fail closed；其余保护由 FK/CHECK/唯一约束和同一事务 guard 承担，不追求 trigger 数量相同。 | schema tests 已证明关键跨 board、附件路径、第二 active run 与 running claim 约束；公开 mutation 仍需逐项负向测试。 | partial |
| label/signal/projection 派生对象 | baseline migration object list | owner：`kanban-service` label/signal/projection facts + host projection worker；public entry：`kanban-server`/`kanban-client`、CLI、Desktop surface | Turso v2 已建立事实约束与 worker 队列表；下一步完成 replay/backfill、derived rebuild 和公开入口，不创建 helper-protocol 兼容空壳。 | 每个对象有 source lineage、rebuild/repair acceptance 和 adapter witness；在公开切片闭合前保持 partial。 | partial |

## 2. 状态机与生命周期

canonical 状态集合在两端都是 `triage|todo|scheduled|ready|running|blocked|review|done|archived`。
baseline 公开 transitions 为 11 个；HEAD 公开 transitions 为 7 个，另有一个新的
`release`。HEAD 的内部 `reclaim_expired` 不是 adapter 自由调用的状态设置。

| legacy transition / operation | baseline evidence | target owner / public entry | migration rule | acceptance | status |
|---|---|---|---|---|---|
| `promote` (`todo|scheduled -> ready`) | baseline `service/transitions.rs`; `api.promote-task`; current `crates/kanban-store-turso/src/operations/lifecycle/promote.rs` | `kanban-service` lifecycle operation；`kanban-server` HTTP `POST /api/v1/tasks/:task_id/transitions/promote`；`kanban-client`/CLI/MCP `task_promote`；Desktop | 保留 readiness、plan、dependency、CAS 守卫。 | same task promote success/failure semantics and event. | current |
| `claim` (`ready -> running`) | baseline `api.claim-task`; current `lifecycle/claim.rs` | `kanban-service` lifecycle operation；`kanban-server` HTTP `/transitions/claim`；`kanban-client`/CLI `task claim`；MCP `task_claim`；dispatcher reuse | 必须原子 CAS + run + event，单任务最多一个 active run。 | concurrent claim exactly one success; run/event are same transaction. | current |
| `heartbeat` (`running -> running`) | baseline `api.heartbeat-task`; current `lifecycle/heartbeat.rs` | `kanban-service` lifecycle operation；`kanban-server`/`kanban-client` HTTP/CLI；MCP heartbeat；Desktop | owner/token/expiry/lock_version semantics unchanged. | mismatched token leaves task/run unchanged. | current |
| `complete`/`done` (`running|review -> done`) | baseline `api.complete-task`; current `lifecycle/done.rs` | `kanban-service` lifecycle operation；`kanban-server` HTTP `/transitions/complete`；`kanban-client`/CLI `task done` (`complete` alias)；MCP `task_done`；Desktop | keep required-step and active-run guards. | incomplete required step rejected; successful run/event/task updates atomic. | current |
| `submit-review` (`running -> review`) | baseline `api.submit-review-task`; current `lifecycle/review.rs` | `kanban-service` lifecycle operation；`kanban-server`/`kanban-client` HTTP/CLI/MCP review；Desktop | preserve owner/token or dispatcher force policy. | run becomes succeeded, task has no active claim and enters review. | current |
| `block` (`triage|todo|scheduled|ready|running|review -> blocked`) | baseline `api.block-task`; current `lifecycle/block.rs` | `kanban-service` lifecycle operation；`kanban-server`/`kanban-client` HTTP/CLI/MCP block；Desktop | non-empty reason and running-run failure must remain atomic. | bad reason/token produces no partial run/task/event mutation. | current |
| `task.plan.not_required` | baseline `api.mark-execution-plan-not-required`; current `lifecycle/plan_not_required.rs` | `kanban-service` plan operation；`kanban-server`/`kanban-client` HTTP/CLI/MCP `task_plan_not_required`；Desktop | keep explicit plan gate; no direct arbitrary status setter. | plan transition is idempotent/guarded and promote/claim reject unplanned. | current |
| `release` (`running -> ready`) | no baseline endpoint/CLI/MCP tool; HEAD `lifecycle/release.rs`, HTTP route and clients | `kanban-service` lifecycle operation；`kanban-server` HTTP `/transitions/release`；`kanban-client`/CLI `task release`；MCP `task_release`；Desktop | New operation: only matching active owner/token, cancel active run, clear claim, emit event. | successful release returns ready and canceled run; wrong token has no side effects. | new/current |
| `specify` | baseline `api.specify-task`, `service::specify_task` | owner：`kanban-service` lifecycle/specification operation；public entry：`kanban-server`/`kanban-client` HTTP `/transitions/specify`、CLI `task specify`、MCP `task_specify`、Desktop task edit | 新增显式 service command 与 `kanban-protocol` contract；迁移 specification 字段/event，再注册四类 adapter；不把 PATCH 当作状态转换。 | route、CLI、MCP、Desktop detail 均可 specify，且 event/状态守卫测试通过。 | missing |
| `unblock` | baseline `api.unblock-task`, `service::unblock_task` | owner：`kanban-service` lifecycle operation；public entry：`kanban-server`/`kanban-client` HTTP `/transitions/unblock`、CLI `task unblock`、MCP `task_unblock`、Desktop drag/action | 实现依赖、排期、execution-plan 重算，按 `triage|todo|scheduled|ready` 选择目标，再同步注册 adapters。 | blocked task 的 guard/recompute/event acceptance 完整，不能盲设 ready。 | missing |
| `reopen` | baseline `api.reopen-task`, `service::reopen_task` | owner：`kanban-service` lifecycle operation；public entry：`kanban-server`/`kanban-client` HTTP `/transitions/reopen`、CLI `task reopen`、MCP `task_reopen`、Desktop task action | 恢复 completion history/result 保留、completed_at 清空及受控子任务重算，并加入 `kanban-protocol` operation contract。 | done/review reopen 的 reason、CAS、event、child-scope 测试通过。 | missing |
| `reclaim` (explicit expired claim) | baseline `api.reclaim-task`, `service::reclaim_task`/`reclaim_expired` | owner：`kanban-service` reclaim operation + `host-admin` dispatcher；public entry：`kanban-server`/`kanban-client` HTTP `/transitions/reclaim`、CLI `task reclaim`、MCP `task_reclaim`；internal dispatcher path remains `serve --dispatcher-profile` | 将 expired run/task CAS、retry/max-retries、target status 和 event 封装为 service operation；公开入口与 dispatcher 复用同一事务。 | explicit reclaim 与 dispatcher reclaim 的 owner/token/run/event/retry acceptance 完整；当前仅 internal path，公开入口 missing。 | partial |
| `archive` (task) | baseline `api.archive-task`, `service::archive_task` | owner：`kanban-service` archive operation；public entry：`kanban-server`/`kanban-client` HTTP `/transitions/archive`、CLI `task archive`、MCP `task_archive`、Desktop board/task action | 增加显式 archive guard、board/task archive event 与读写隔离；禁止通过 generic update 改 status。 | archive guard、默认隐藏、active run 拒绝和 event acceptance 完整。 | missing |

## 3. HTTP route parity

### 3.1 Retained routes (27/84, `current`)

| legacy evidence / operation | target owner / public entry | migration rule | acceptance | status |
|---|---|---|---|---|
| `GET /health` (`api.health`) | `crates/kanban-server/src/http/operations/health.rs` | keep loopback host health contract | health returns valid data envelope | current |
| `GET /api/v1/boards` (`api.list-boards`) | `.../boards/list.rs` | board list only | route + response DTO contract test | current |
| `GET /api/v1/boards/:board/columns` (`api.list-board-columns`) | `.../boards/columns.rs` | fixed canonical columns | columns match 9 default statuses | current |
| `GET/POST /api/v1/boards/:board/tasks` (`api.list-tasks`, `api.create-task`) | `.../tasks/list.rs`, `create.rs` | shared task list/create path | list/create and idempotency acceptance | current |
| `GET /api/v1/tasks/:task_id` (`api.get-task`) | `.../tasks/show.rs` | selector resolved by host | global id/board selector response exact | current |
| `GET /api/v1/tasks/:task_id/context` | `.../context.rs` + `adapter/operations/context.rs` | bounded subject/reference/query pack；Turso FTS、canonical relation BFS、vector provider 均经 application merge | subject-first stable dedup/rank、budget、board isolation、provider degraded/fallback and response contract tests | current |
| `POST /api/v1/tasks/:task_id/execution-plan/not-required` | `.../tasks/plan_not_required.rs` | explicit plan gate | unplanned task can opt out with reason | current |
| `POST /api/v1/tasks/:task_id/transitions/{block,claim,complete,heartbeat,promote,submit-review}` | `.../tasks/{block,claim,done,heartbeat,promote,review}.rs` | all mutations use the shared `kanban-service` transaction path | route-specific lifecycle tests and event/run invariants | current |
| `POST /api/v1/tasks/:task_id/transitions/release` | `.../tasks/release.rs` | new release contract; see §2 | release acceptance above | new/current |
| `GET/POST /api/v1/tasks/:task_id/comments` | `.../comments/{list,create}.rs` | note/decision comments only | comment idempotency and DTO contract | current |
| `GET/POST /api/v1/tasks/:task_id/dependencies`, `DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id` | `.../dependencies/{list,create,remove}.rs` | board FK + cycle guard | add/list/remove acceptance | current |
| `GET/POST /api/v1/tasks/:task_id/steps`, `PATCH /api/v1/tasks/:task_id/steps/:step_id` | `.../steps/{list,create,update}.rs` | create first step plans task; update has no status mutation | step CRUD/plan acceptance | current |
| `GET /api/v1/tasks/:task_id/runs`, `GET /api/v1/runs/:run_id`, `GET /api/v1/runs/:run_id/log` | `.../runs/{list,show,log}.rs` | bounded read-only run/log surface | run/list/show/log tests; bounded 256 KiB log | current |
| `GET /api/v1/events` (`api.list-events`) | `.../events/list.rs` | event list only, no direct event write | cursor/board/task filter acceptance | current |
| `GET /api/v1/stats` (`api.get-stats`) | `.../stats.rs` | canonical queue stats query | status counts/stale claims/blocked reasons response | current |

### 3.2 删除的 baseline routes（57/84，全部 `missing`）

以下逐项来自 baseline `EndpointDescriptor`；HEAD `http/operations` 没有对应 route：

```text
GET  /api/v1/boards/:board
POST /api/v1/boards
POST /api/v1/boards/:board/archive
GET  /api/v1/boards/:board/tasks/by-status
GET  /api/v1/boards/:board/task-map
PATCH /api/v1/tasks/:task_id
GET  /api/v1/tasks/:task_id/neighborhood
GET  /api/v1/tasks/:task_id/labels
POST /api/v1/tasks/:task_id/labels
DELETE /api/v1/tasks/:task_id/labels/:label_id
POST /api/v1/tasks/:task_id/labels/bootstrap
GET  /api/v1/tasks/:task_id/labels/suggestions
GET  /api/v1/tasks/:task_id/label-proposals
POST /api/v1/tasks/:task_id/label-proposals
POST /api/v1/tasks/:task_id/label-ontology/observations
GET  /api/v1/boards/:board/labels
POST /api/v1/boards/:board/labels
GET  /api/v1/boards/:board/labels/semantics
GET  /api/v1/boards/:board/labels/:label_id/semantics
PUT  /api/v1/boards/:board/labels/:label_id/semantics
DELETE /api/v1/boards/:board/labels/:label_id/semantics
GET  /api/v1/boards/:board/labels/atoms
GET  /api/v1/boards/:board/labels/atoms/:atom_ref/explain
GET  /api/v1/boards/:board/labels/atom-index/status
POST /api/v1/boards/:board/labels/atom-index/rebuild
GET  /api/v1/boards/:board/labels/atom-index/query
GET  /api/v1/boards/:board/signals
GET  /api/v1/boards/:board/signals/review
GET  /api/v1/signals/:signal_id
POST /api/v1/boards/:board/signals
POST /api/v1/boards/:board/signals/confirm
POST /api/v1/boards/:board/signals/reject
POST /api/v1/boards/:board/signals/resolve
POST /api/v1/boards/:board/signals/supersede
GET  /api/v1/boards/:board/label-ontology/signals
GET  /api/v1/boards/:board/label-ontology/review
GET  /api/v1/label-ontology/signals/:signal_id
POST /api/v1/boards/:board/label-ontology/actions
POST /api/v1/boards/:board/label-ontology/apply/atom
POST /api/v1/boards/:board/label-ontology/revert
POST /api/v1/boards/:board/label-ontology/validate
GET  /api/v1/label-proposals/:proposal_id
POST /api/v1/label-proposals/:proposal_id/accept
POST /api/v1/label-proposals/:proposal_id/reject
DELETE /api/v1/tasks/:task_id/steps/:step_id
POST /api/v1/tasks/:task_id/steps/:step_id/done
POST /api/v1/tasks/:task_id/steps/:step_id/skip
POST /api/v1/tasks/:task_id/steps/:step_id/reopen
POST /api/v1/tasks/:task_id/transitions/specify
POST /api/v1/tasks/:task_id/transitions/reopen
POST /api/v1/tasks/:task_id/transitions/reclaim
POST /api/v1/tasks/:task_id/transitions/unblock
POST /api/v1/tasks/:task_id/transitions/archive
GET  /api/v1/search/tasks
GET  /api/v1/search/tasks/by-status
GET  /api/v1/search/status
GET  /api/v1/graph/status
GET  /api/v1/graph/neighbors
GET  /api/v1/vector/status
GET  /api/v1/stream/events
POST /api/v1/maintenance/doctor
POST /api/v1/maintenance/checkpoint
```

目标 owner/public entry：上列领域路径都由 `kanban-service` operation 负责事实语义，
`kanban-server` 注册 HTTP handler，`kanban-client`/CLI/MCP/Desktop 作为真实 consumer；
维护类路径由 `host-admin` 与 `kanban-service` 共同负责，仅暴露给 `kanban-server`/`kanban-client`、CLI、Desktop，
MCP 只承载领域 query/resource。迁移规则是先加 Turso schema/service operation，再注册 route、
`kanban-client`/CLI/MCP/Desktop，并补齐 schema/adoption witness；旧路径的兼容窗口
只允许返回明确 404/feature-unavailable，不写入 Turso。Acceptance 是 route inventory 与
真实 handler、operation、consumer 一一对应；58 个路径在此之前保持 missing。

## 4. CLI parity

### 4.1 current 可用 leaf（26）与 new/retired 差异

| legacy/current operation | public entry | migration rule | acceptance | status |
|---|---|---|---|---|
| `serve` | `kanban serve [--db] [--dispatcher-profile] [--host] [--port]` | `kanban-service` host 由 serve 打开/初始化 Turso；所有 `kanban-client` 命令走 loopback | `crates/kanban-cli/src/server.rs` loopback/dispatcher tests；host down 返回 `server_unavailable` | current |
| board `list`, `columns` | `kanban board list`, `kanban board columns` | baseline `board list` 保留；`columns` 是 HEAD 新 query；create/show/use/current/archive 目标 owner：`kanban-service` board facts；public entry：`kanban-server`/`kanban-client`、CLI/config、Desktop board settings | exact board/columns DTO and no direct DB | current/new |
| task `create`, `list`, `show` | `kanban task create|list|show` | labels/dependencies/detail projection 不在当前 CLI surface | create/list/show client contract tests | current |
| task step `add`, `list`, `update`, `not-required` | `kanban task step ...` | 保留执行计划最小 CRUD；done/skip/reopen/remove 暂不迁移 | step list/create/update and plan gate tests | current |
| task `promote`, `claim`, `heartbeat`, `review`, `done`, `block` | `kanban task ...` | 复用 §2 lifecycle；`done` 的 visible alias `complete` 只是别名 | CLI output/error and state/event acceptance | current |
| task `release` | `kanban task release` | HEAD 新增，matching token only | release route/client acceptance | new/current |
| comment `add`, `list`; dependency/dep `add`, `list`, `remove`; events; runs; run `show`, `logs` | `kanban comment ...`, `kanban dep|dependency ...`, `kanban events`, `kanban runs`, `kanban run ...` | 只调用 client；run logs 无 baseline `--tail-bytes` 参数 | each command returns stable JSON/error envelope | current |
| `init` | `kanban init` | CLI 项目 shell；幂等创建/复用 `.kb/config.toml`，不初始化 DB | config output、权限/原子写入和 no-storage-touch tests | current |

### 4.2 baseline leaf groups removed（baseline 126 → HEAD 26 usable）

| legacy group / leaves | baseline evidence | target owner / public entry | migration rule | acceptance | status |
|---|---|---|---|---|---|
| board `create`, `show`, `use`, `current`, `archive` | baseline `args.rs::BoardCommand` | `create/show/archive` owner：`kanban-service` board facts + localhost HTTP；`use/current` owner：CLI `.kb/config.toml` shell | 先迁移 board mutation/archive guard；`use/current` 只读写项目配置，不校验或打开 DB。 | config-side DTO/schema/adoption、权限/原子写入和 HTTP board operation 分别验收。 | partial |
| task `update`, `reopen`, `start`, `unblock`, `reclaim`, `archive`; step `done`, `skip`, `reopen`, `remove` | baseline `args.rs::TaskCommand`/`TaskStepCommand` | owner：`kanban-service` task/step lifecycle；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop detail/drag actions | 为每个动作增加显式 service operation/`kanban-protocol` contract，迁移 event/run/step guards，再注册 adapter；status 不走 generic setter。 | 每个 leaf 有 route/client/CLI/MCP/Desktop acceptance；当前 nested commands 在 DB 前以 usage/unavailable 结束。 | missing |
| signal `record`, `list`, `show`, `review`, `confirm`, `reject`, `resolve`, `supersede` | baseline `args.rs`, `commands/signal.rs` | owner：`kanban-service` signal ledger；public entry：`kanban-server`/`kanban-client` signals HTTP、`kanban signal`、MCP signal tools、Desktop Signals | signal tables/backlinks、共享 ledger operation、typed client、CLI 和 MCP 已接通；UI 继续保持同一 host path。 | signal lifecycle、backlink、JSON contract、跨 board/幂等与 Desktop review tests 完整。 | current/partial |
| hook `codex install`, `status`, `uninstall`, `handle failure`, `handle task-create` | baseline `args.rs`, `commands/hook.rs` | owner：host-admin/CLI hook adapter；public entry：`kanban hook codex ...` 与 managed hook files | 只写 `CODEX_HOME/hooks.json` 和 XDG prompt config；managed marker/fingerprint、原子写入和卸载边界由 CLI 保证；handler 不打开 DB。 | install/status/uninstall/handle fixtures、权限、fingerprint tamper 和 recovery tests。 | current |
| config `show` | baseline `args.rs`, `commands/config.rs` | owner：CLI config resolver/host runtime；public entry：`kanban config show`, `.kb/config.toml` and Desktop runtime settings | 共享完整 db/board/locale precedence；命令只读解析并输出 source DTO，不触碰 DB。 | config show 只读、坏配置错误封装、无 DB touch 的 tests 完整。 | current |
| label leaves `list/create/bootstrap/delete/add/remove`, `semantics list/show/upsert/delete`, `atoms list/explain`, `atom-index status/rebuild/query`, `suggest`, `propose`, `proposals list/show/accept/reject`, `ontology record/list/show/review/quality/confirm/reject/supersede/resolve/apply atom/revert/validate` | baseline `args.rs`, `commands/label.rs` | owner：`kanban-service` label/ontology facts + Turso vector projection；public entry：`kanban-server`/`kanban-client`、CLI、MCP、Desktop ontology/detail | 按 §1 表→service→derived index→adapter 顺序迁移，每个 leaf 绑定 schema/adoption witness。 | 所有列出的 leaf 有稳定 JSON/error、事务/回滚、Turso vector degraded 和 Desktop workbench acceptance。 | missing |
| search/index `search`; `index status/doctor/rebuild/sync` | baseline `args.rs`, `commands/search.rs`, `commands/index.rs` | owner：`kanban-service` Turso FTS + host projection worker；public entry：`kanban-server`/`kanban-client`、CLI search/index、Desktop list query | 迁移 FTS facts/metadata/outbox；legacy SQLite importer 作为 service feature，经 host-admin HTTP/CLI 运行，不在 `xtask`；再完成 rebuild/sync/doctor，并让 query adapter 只读 service projection。 | FTS missing/corrupt/degraded、query filters/pagination、rebuild/sync tests 完整；importer 不经 `xtask` 写 canonical。 | missing |
| substrate `entity list/show`, `outbox list`, `derived status` | baseline `commands/substrate.rs` | owner：`kanban-service` entity/outbox/derived facts + host projection worker；public entry：`kanban-server`/`kanban-client`、CLI、MCP substrate queries、Desktop diagnostics | 迁移 entity/outbox/derived tables并定义 event replay cursor，再注册只读 adapters。 | entity/outbox/derived DTO、cursor、lag/error diagnostics acceptance 完整。 | missing |
| graph `status/neighbors/rebuild/sync/query`; vector `status/configure/rebuild/sync/query-chunks/query-label-atoms` | baseline `args.rs`, `commands/substrate.rs` | owner：`kanban-service` Turso BFS/vector/FTS + host projection worker；public entry：`kanban-server`/`kanban-client`、CLI、Desktop map/detail | 恢复 service 内 capability/lease/config、derived rebuild/sync 与 context 外的维护操作；legacy SQLite importer 仅作为 service feature 通过 host-admin HTTP/CLI，不在 `xtask`，再开放命令。 | service unavailable/degraded/recovery、BFS/vector/FTS query and maintenance tests 完整。 | missing |
| context `build` | baseline `args.rs`, `commands/substrate.rs` | owner：`kanban-application` context merge + Turso FTS/BFS/vector adapters；public entry：`kanban-server`/`kanban-client`、CLI `context build`、MCP `context_build`、Desktop typed API | read-only context pack；subject/reference/query selectors、budget/depth、provider capability/degraded diagnostics、lexical-only fallback 和 board isolation 均走 canonical host | application merge、HTTP/client/CLI/MCP/fixture/adoption witness and provider outage tests complete | current |
| hidden `dispatch`, `completions`, `__complete` | baseline `args.rs` | owner：host-admin dispatcher + CLI completion adapter；public entry：`kanban serve --dispatcher-profile`, `kanban completions`, hidden `__complete` | dispatcher 绑定 serve host/claim path；completion 只生成静态脚本和本地枚举/配置候选，不重新打开 DB。 | profile/claim/shutdown、completion no-storage-touch tests 完整。 | partial |
| baseline maintenance/diagnostics: `doctor`, `stats`, `backup`, `export`, `import`, `checkpoint`, `vacuum` | baseline `args.rs`, `commands/maintenance.rs`, `app.rs` | owner：`host-admin` + `kanban-service` maintenance；public entry：CLI `kanban doctor|stats|backup|export|import|checkpoint|vacuum` 与等价 `kanban-server`/`kanban-client` HTTP、Desktop | 先定义 Turso backup/import format、doctor/checkpoint/lock/recovery contracts，再注册 CLI/HTTP/Desktop。 | each leaf has no-data-loss, recovery, lock and JSON contract acceptance；当前没有对应 CLI leaf。 | missing |

## 5. MCP surface

baseline 没有 MCP crate/tool；HEAD 新增 25 个 stdio tools，均通过 `KanbanClient` 调用
loopback host（`crates/kanban-mcp/src/shared.rs`），不直接打开 Turso。

| tool set / names | legacy evidence | target owner / public entry | migration rule | acceptance | status |
|---|---|---|---|---|---|
| `board_list` | baseline absent | `crates/kanban-mcp/src/tools/boards/list.rs` | map to retained board list HTTP operation | tool schema + client call test | new/current |
| `task_create`, `task_list`, `task_show`, `task_plan_not_required`, `task_promote` | baseline absent | `tools/tasks/*.rs` | use shared task/application DTO; no labels/legacy detail expansion | independent tool locatability tests | new/current |
| `task_claim`, `task_heartbeat`, `task_release`, `task_review`, `task_done`, `task_block` | baseline absent | `tools/lifecycle/*.rs` | use same state machine, owner/token/CAS semantics as HTTP/CLI | each tool has route-equivalent acceptance | new/current |
| `comment_create`, `comment_list` | baseline absent | `tools/comments/*.rs` | note/decision only; no signal ledger | comment contract acceptance | new/current |
| `dependency_create`, `dependency_list`, `dependency_remove` | baseline absent | `tools/dependencies/*.rs` | same-board FK/cycle guards | relation operation acceptance | new/current |
| `step_create`, `step_list`, `step_update` | baseline absent | `tools/steps/*.rs` | no old step done/skip/reopen/remove until route exists | step contract acceptance | new/current |
| `run_list`, `run_show`, `run_log`, `event_list` | baseline absent | `tools/runs/*.rs`, `tools/events/list.rs` | read-only run/event paths; bounded log | tool inventory + bounded log/event tests | new/current |
| `context_build` | baseline absent | `tools/context.rs` | bounded read-only context pack；不提供 rebuild/sync/admin | tool locatability + typed client/provider degraded acceptance | new/current |

HEAD inventory is locked by `crates/kanban-mcp/src/main.rs:38-74` and lists exactly 25 names.

## 6. Desktop views and task workbench

| view / surface | baseline evidence | target owner / public entry | migration rule | acceptance | status |
|---|---|---|---|---|---|
| `board`, `list`, `events`, `runs`, `health`, `settings` | baseline `view-types.ts`; HEAD `view-types.ts:5-14`, `AppShell.tsx:941-985` | `apps/desktop/src/app/AppShell.tsx` | keep only views whose API calls are in §3.1; all mutations still go through host client | navigation renders six views and uses retained routes | current |
| `map` | baseline `primaryViews/sidebarViews` and `BoardTaskMapView.tsx` | owner：`kanban-service` Turso BFS/entity projection + Desktop task-map；public entry：`kanban-server`/`kanban-client` `GET /api/v1/boards/:board/task-map`、Desktop Map view、CLI/MCP graph query | 迁移 entity/relation projection、bounded graph route 和 task-map state，再把 component 接回 AppShell navigation。 | map route/data/layout/selection acceptance 完整；当前 view 被排除，status partial/retired。 | partial/retired |
| `signals` | baseline `SignalsWorkbench.tsx` and navigation | owner：`kanban-service` signal ledger + Desktop Signals；public entry：`kanban-server`/`kanban-client` signals HTTP、CLI `kanban signal`、MCP signal tools、Signals workbench | 迁移 signal tables/backlinks、review/action routes 和 client DTO，再把 workbench 接回导航。 | signal lifecycle/JSON/UI review acceptance 完整；当前 view 被排除，status partial/retired。 | partial/retired |
| `ontology` | baseline `OntologyReviewWorkbench.tsx` and navigation | owner：`kanban-service` ontology facts + Turso vector projection + Desktop Ontology；public entry：`kanban-server`/`kanban-client` label-ontology HTTP、CLI、MCP、Ontology workbench | 迁移 ontology ledger/action/validation 及 atom index，再注册 workbench navigation。 | ontology action/validation/revert/UI review acceptance 完整；当前 view 被排除，status partial/retired。 | partial/retired |
| `maintenance` | baseline `MaintenanceView.tsx` and navigation | owner：`host-admin` + `kanban-service` maintenance worker + Desktop Maintenance；public entry：`kanban-server`/`kanban-client` maintenance HTTP、CLI and Maintenance view | 迁移 owner lease/generation/doctor/checkpoint/recovery，再把 Maintenance view 接回导航。 | maintenance run/rebuild/doctor/recovery/UI acceptance 完整；当前 view 被排除，status partial/retired。 | partial/retired |
| task detail panels: description, execution plan, comments, dependencies, runs/events, metadata | baseline/current `features/task-detail/*` | `TaskDetail.tsx` and retained task/run/comment/dependency/step APIs | keep panel only for retained routes; no direct SQL | detail sheet can read/update only supported operations | current |
| context pack typed API | baseline task context surface | `apps/desktop/src/lib/api/context.ts`, `KanbanApi.buildContext` | expose the read-only context contract without adding a second data path; UI adoption remains scoped to a later detail panel | TypeScript API types and route encoding compile; no direct database access | current/partial |
| task labels/label suggestions and old graph detail | baseline `TaskLabelsPanel.tsx`, label API methods | current cutline test `task-detail-capability-cutline.test.ts` removes label panel and label mutations from selected detail | do not infer parity from orphan files/types | test proves no label panel/add/remove/suggestion call in active detail path | partial/missing |

## 7. host、dispatcher 与维护命令

| legacy maintenance capability | legacy evidence | target owner / public entry | migration rule | acceptance | status |
|---|---|---|---|---|---|
| single host DB ownership | baseline direct SQLite `kanban-cli` paths and `kanban serve`; HEAD `docs/CLI_SPEC.md` §2, `crates/kanban-cli/src/server.rs` | owner：`kanban-service` host；`kanban serve` is the only process allowed to open/initialize Turso | all CLI/MCP/Desktop requests use loopback client; no fallback DB open | stop host → `server_unavailable`; client does not create DB | current |
| opt-in dispatcher | baseline hidden `dispatch` and service dispatcher; HEAD `ServeArgs::dispatcher_profile`, `crates/kanban-server/src/dispatcher.rs` | in-process dispatcher started only by `kanban serve --dispatcher-profile` | dispatcher claims only ready, calls shared reclaim/claim/heartbeat/done/review/block/release path | profile absent means dispatcher disabled; expired reclaim is internal and transactional | current/partial |
| HTTP `POST /api/v1/maintenance/doctor`, `POST /api/v1/maintenance/checkpoint` | baseline `api.doctor`, `api.checkpoint`; baseline server router | owner：`host-admin` + `kanban-service` maintenance operations + `kanban-server`; public entry：same HTTP paths, `kanban-client`/CLI doctor/checkpoint and Desktop Health/Maintenance | 以 Turso schema/host identity、WAL/checkpoint、FK/derived diagnostics 为 operation contract，注册 HTTP/client/CLI/Desktop。 | route inventory、doctor report、checkpoint lock/recovery and JSON contract tests complete；当前 route 数为 0。 | missing |
| CLI `maintenance run/status/rebuild/cleanup-legacy inventory/apply/verify/restore` (7 leaves) | baseline `args.rs::MaintenanceCommand`; `commands/maintenance.rs` | owner：`host-admin` + `kanban-service` projection worker；public entry：CLI `kanban maintenance ...`、`kanban-server` maintenance HTTP、Desktop Maintenance | 迁移 projection-v2 owner lease/generation/recovery/cleanup protocol，随后注册每个 leaf 与 schema/adoption witness。 | run/status/rebuild/cleanup-legacy 的 no-data-loss、owner exclusion、resume/restore tests complete。 | missing |
| CLI `doctor`, `stats`, `backup`, `export`, `import`, `checkpoint`, `vacuum` (7 leaves) | baseline `args.rs`, `commands/app.rs` | owner：`host-admin` + `kanban-service` maintenance operations；public entry：CLI、`kanban-server`/`kanban-client` maintenance HTTP、Desktop Health | 定义 backup/import format、replace journal、doctor/checkpoint/lock/recovery 后注册 CLI；stats 与 HTTP stats 共用 `kanban-protocol` DTO。legacy SQLite importer 是 service feature，经 host-admin HTTP/CLI 运行，不在 `xtask`。 | 每个 leaf 有 JSON/error、data-loss guard、recovery and read-only acceptance；当前 CLI leaf 缺失。 | missing |
| search/graph/vector/index maintenance | baseline `commands/search.rs`, `commands/index.rs`, `commands/substrate.rs`, projection migrations | owner：`kanban-service` Turso vector/FTS/BFS + host projection worker；public entry：`kanban-server`/`kanban-client`、CLI、Desktop map/detail，host-admin 负责维护调用 | 由 service feature 提供 legacy SQLite importer，通过 host-admin HTTP/CLI 运行，绝不在 `xtask`；恢复 Turso derived metadata/outbox/capability，再执行 rebuild/sync/repair；canonical Turso 只保存事实。 | service unavailable/degraded、rebuild/sync/repair and cross-surface acceptance complete。 | missing |
| desktop maintenance/health boundary | baseline MaintenanceView plus HTTP doctor/checkpoint; HEAD HealthView only | owner：`host-admin` + `kanban-service` maintenance worker + Desktop；public entry：`kanban-server`/`kanban-client` maintenance HTTP、CLI maintenance 与 Desktop Maintenance；`HealthView` 保留为只读 host health | 恢复 Maintenance view 导航及完整 host-admin 操作（doctor/checkpoint、owner lease、generation、rebuild、restore/recovery）；所有写操作经 service/host-admin path，不直接开 Turso。 | `HealthView` 只展示 host 状态；Maintenance view 可导航并覆盖完整 maintenance acceptance，且与 §7 的 HTTP/CLI 操作一致。 | partial |

本节 host-admin 维护路径不注册 MCP；MCP 只保留领域 query/resource，不提供 rebuild、migration、backup、compaction 或 replacement。

## 8. Acceptance gate for this ledger

Parity is not closed by the presence of a stale contract descriptor, orphan frontend file or
private implementation detail. A feature is `current` only when all of the following are true:

1. canonical Turso schema has the needed fact tables/constraints;
2. a shared `kanban-service` operation owns the mutation/query;
3. every claimed public adapter entry (HTTP, CLI, MCP, Desktop) reaches that operation;
4. route/command/tool/view inventory and contract tests cover the entry;
5. deleted features have no compatibility path that writes canonical data.

Current HEAD therefore records an implementation-starting-point gap: the 10-table task queue and
26 retained HTTP/CLI operations are current; release/dispatcher/MCP are new current slices; labels,
signals, search/graph/vector/projection, old maintenance and four Desktop workbenches are still
missing or partial in this ledger, with target owners and explicit acceptance paths above.

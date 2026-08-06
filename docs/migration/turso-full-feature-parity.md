# Turso 全功能 parity ledger

本账本以 `6ea277583e51ea010aa6739a53091337676b4cff` 为功能基线（下文简称 baseline），以当前
工作树的 `kanban-service`/`kanban-server`/`kanban-protocol` 实现为事实快照；最终集成 HEAD 与
full gates 仍按实际执行结果记录，保持 `pending`。它回答四个问题：baseline 每个领域现在由谁
拥有、HTTP/CLI/MCP/Desktop 通过什么入口、数据/语义如何迁移、哪些测试已经存在以及最终 gates
还缺什么。

本账本不把 catalog、孤立文件或一次未运行的命令标为 green。状态含义：

| 状态 | 含义 |
| --- | --- |
| `implemented-evidence` | 当前代码和代表性测试已显示纵向路径；仍需按 §5 的最终 gate 复核 |
| `protocol-adopted` | protocol/schema surface 已标为 `adopted`，但不能替代 runtime/full gate |
| `pending-gate` | 已有 owner/实现或局部测试，尚未有本轮完整 adoption/full 证据 |
| `historical` | baseline/旧 sidecar 的迁移证据；不属于 active runtime |

## 0. 快照与证据口径

baseline 的 machine source 是 `6ea277` 中的 migration、`crates/kanban-contract`、CLI args、Desktop navigation 和 helper/backend crates；当前 source 是 `Cargo.toml`、`crates/kanban-service/src/schema.rs`/`service.rs`、`crates/kanban-server/src/http/operations/**`、`crates/kanban-client/src/**`、`crates/kanban-cli/src/main.rs`、`crates/kanban-protocol` catalog、`crates/kanban-mcp/src/main.rs` 的 adapter 对齐测试和 `apps/desktop/src`。

legacy SQLite v30 的表清单以 `crates/kanban-service/src/legacy_import.rs::{LEGACY_TABLES, CANONICAL_TABLES}` 为 machine source：共 35 张源表，其中 27 张是 canonical facts，另外 8 张只是旧 projection/outbox/control-plane 的 shape witness。这里的表数不等同于旧 workspace 的 backend/helper crate 数；后者仅作为历史 source map 保留。

可复核命令：

```bash
git show 6ea277583e51ea010aa6739a53091337676b4cff:migrations/001_initial.sql
git show 6ea277583e51ea010aa6739a53091337676b4cff:apps/desktop/src/features/navigation/view-types.ts
rg -n '\.route\(' crates/kanban-server/src/http/operations crates/kanban-server/src/vector.rs
rg -n 'MCP_OPERATION_CATALOG|MCP_HOST_ADMIN_OPERATION_IDS|mcp_operation_catalog' crates/kanban-protocol/src/mcp.rs crates/kanban-mcp/src/main.rs
```

当前 active workspace 明确是七个产品 Rust crate（`kanban-core`、`kanban-service`、`kanban-protocol`、`kanban-client`、`kanban-server`、`kanban-cli`、`kanban-mcp`）、Desktop Tauri package 和私有 `xtask`。旧 backend/helper sidecar 不再是 workspace/runtime 成员；它们只在 baseline source map、历史 release/projection 文档或迁移 fixture 中作为证据保留。

## 1. 数据域映射

| baseline 域/事实 | 当前 owner | HTTP / CLI / MCP / Desktop | 迁移规则 | 实际测试证据（代表性） | 状态 / 剩余 |
| --- | --- | --- | --- | --- | --- |
| `boards`、`board_columns` | `kanban-service` board operations + Turso schema | `GET/POST /api/v1/boards`、board detail/archive/columns；`kanban board list/create/show/archive/columns`；`board_list/create/show/archive`；Desktop board/list/settings | 保留 board identity、columns、archive guard；board 归档不改 task status，拒绝 active running work | `board_routes_cover_create_duplicate_archive_and_active_only_get`；`board_queries_use_the_initialized_host_database`；CLI board contract tests | `implemented-evidence`; 需跑完整 surface/adoption gate |
| `tasks`、plan、steps、dependencies | service task/step/dependency operations + state machine | `/boards/:board/tasks`、`/boards/:board/tasks/by-status`、`/tasks/:id`、`/steps`、`/dependencies`；CLI `task`/`dep`；MCP task/step/dependency tools（含 `task_list_by_status`）；Desktop board/list/detail | 保留 global ID、board seq、plan gate、same-board FK、cycle guard、idempotency；状态只走显式 command；by-status 每个窗口复用 canonical list filters/sort/pagination | `suite_tasks_crud_and_reads_use_committed_fixtures_through_router`；`list_tasks_*`；`claim_task_*`；`step_create_and_list_use_application_path_and_entity_local_idempotency`；`dependency_create_and_list_use_the_shared_application_path`；CLI task/board/maintenance contract tests | `implemented-evidence`; 完整 cross-surface gate pending |
| `task_runs`、logs | service lifecycle/run read operations | `/tasks/:id/runs`、`/runs/:id`、`/runs/:id/log`；`kanban runs/run show/run logs`；MCP run tools；Desktop Runs/detail | run 只能由 claim 创建；固定 256 KiB log snapshot；lifecycle 和 run/event 同事务；不引入独立 run mutation | `run_list_uses_application_path_and_preserves_run_contract`；`run_show_reads_the_run_created_by_the_canonical_claim_path`；`run_log_route_uses_the_application_and_returns_contract_shape` | `implemented-evidence`; bounded log + package gate pending |
| `task_comments`、decision metadata、signal backlink | service comment/signal transaction | `/tasks/:id/comments`、`/boards/:board/signals`；`kanban comment`/`signal`；MCP comment/signal；Desktop detail/Signals | comment idempotency 归 task；signal record/backlink/event 同事务；保留 unknown metadata JSON | server comment/signal tests；`signal_tools_are_independently_locatable`；Desktop comments/signals contract tests | `implemented-evidence`; full adoption witness pending |
| `task_attachments`、staging | service attachment + host filesystem root | `/tasks/:id/attachments` list/create/download/delete；CLI/MCP attachment；Desktop TaskAttachmentsPanel | metadata 进 Turso；content 先 staging+fsync+SHA，再原子 publish；删除移 `.trash/`，事务失败恢复；portable/v30 由 journal 关联 | attachment server/desktop contract tests；service attachment/path guard capability test | `implemented-evidence`; v30 end-to-end gate pending |
| `labels`、`task_labels` | service label facts/transactions | `/boards/:board/labels`、`/tasks/:id/labels`；CLI `label`；MCP label tools；Desktop detail/ontology | identity 与 binding 保留 board composite FK；task create/list/query 通过 service labels 参数；不隐式删除 semantics/atoms | `labels_round_trip_is_idempotent_and_emits_add_remove_events`；`label_commands_trim_and_dedupe_names`；CLI label contract tests；MCP label inventory | `implemented-evidence`; Desktop end-to-end/adoption gate pending |
| `label_semantics`、`label_atoms`、proposal | service ontology facts + Turso vector atom index | task proposal：`/api/v1/tasks/:task_id/label-proposals`；board-wide proposal：`GET /api/v1/boards/:board/label-proposals`（可选 `status`）；CLI `label semantics/atoms/atom-index/suggest/propose/proposals`（`--task-ref` 省略时按 board 列出）；MCP `label_proposals_list`；Desktop Ontology workbench | semantics CAS/hash、atom effect、proposal accept/reject 保留；atom index 可删可 rebuild；provider degraded 不写错误 canonical；task/board proposal 查询都经 `KanbanService` | `delete_semantics_removes_derived_atoms`；`apply_and_revert_keep_canonical_hashes_and_atoms_in_sync`；`board_proposal_list_uses_the_board_scoped_service_path`；`label_proposal_routes_consume_typed_fixtures_and_persist_real_proposal`；CLI label contract tests | `implemented-evidence`; final index/adoption gate pending |
| ontology ledger (`observations/signals/actions/effects`) | service ontology ledger | `/label-ontology/*`；CLI `label ontology ...`；MCP ontology tools；Desktop Ontology | record/review/quality/confirm/reject/resolve/supersede/apply/revert/validate 共用 CAS、board guard、event；不将 graph projection 当事实 | `crates/kanban-service/src/store_operations/ontology_tests.rs`；server ontology route fixtures；CLI ontology contract tests | `implemented-evidence`; full semantic acceptance pending |
| generic `signal_observations`、`signals` | service signal ledger | `/boards/:board/signals{,/review,/confirm...}`、`/signals/:id`；CLI `signal`；MCP signal tools；Desktop Signals | record、backlink、review transition 与 event 原子提交；同 board dedupe key idempotent | server signal record/review tests；MCP `signal_tools_are_independently_locatable`；Desktop SignalsWorkbench tests | `implemented-evidence`; full cross-surface gate pending |
| `entities`、`relation_predicates`、`entity_relations`、baseline substrate | service canonical relation store | `/entities`；`entity list/show/upsert`；MCP graph/task map；Desktop Map/context | relation facts 迁入 Turso；board composite FK；旧派生控制面改为 host `projection_jobs/state`，不保留第二 control plane | `entity_upsert_list_and_show_are_available_on_host`；`unknown_entity_is_not_found`；`graph_reads_canonical_relations_with_cycle_safe_bounded_bfs`；`graph_neighbors_enforces_board_isolation` | `implemented-evidence`; full graph rebuild/repair gate pending |
| baseline search/index state | Turso FTS + service search operations | `/search/tasks`、`/search/tasks/by-status`、`/search/status`、`/search/index/rebuild`、`/search/index/sync`；CLI `search/index status|doctor|rebuild|sync`；MCP search；Desktop list/context | `retrieval_documents` + `task_search_fts` 从 canonical task/comment/run/event 重建；FTS stale/error 回退 canonical SQL；旧 external index 不迁移为 owner | `fts_capability_exercises_insert_update_delete_score_and_highlight`；`rebuilds_turso_fts_and_falls_back_for_exact_references`；server search route tests；CLI index contract tests | `implemented-evidence`; FTS/full surface gate pending |
| vector task chunks / label atoms | Turso `vector32` + host Ollama provider | `/vector/status/configure/rebuild/sync/query-*`；CLI vector；MCP read-only vector；Desktop context/ontology typed API | 导入只保留 canonical source；embedding/model/dimension/fingerprint 作为 derived metadata；provider outage 设 degraded 并保留 job | `vector32_roundtrip_dimension_and_cosine_are_real_turso_capabilities`；`vector_routes_use_typed_envelopes_and_degraded_query_error`；vector fixture producer/consumer tests；MCP vector inventory | `implemented-evidence`; Ollama integration/adoption/full gate pending |
| graph/BFS/task map | service bounded BFS over canonical relations | `/graph/status/neighbors/query/rebuild/sync`、`/tasks/:id/neighborhood`、`/boards/:board/task-map`；CLI graph；MCP graph；Desktop Map | BFS depth/cycle/dedup/board isolation；graph projection 删除后 rebuild；不恢复 Oxigraph/helper protocol | `graph_status_and_task_map_routes_are_adopted`；`graph_maintenance_routes_publish_generation_and_counts`；Desktop TaskGraph tests | `implemented-evidence`; full repair/rebuild gate pending |
| context pack | service context merge + FTS/BFS/vector adapters | `/tasks/:id/context`；CLI `context build`；MCP `context_build`；Desktop typed `KanbanApi.buildContext` | subject/reference/query selector；budget/depth/lexical/graph/vector limit；按 provenance 去重；provider degraded 保留可用部分 | `context_merge_is_stable_deduplicated_and_ranked`；`context_merge_enforces_budget_and_board_isolation`；server context route tests；CLI/MCP context contract tests | `implemented-evidence`; Desktop UI adoption/full gate pending |
| projection state/jobs/maintenance owner | service host projection worker + maintenance lease | `/maintenance/status/run/rebuild/cleanup`、doctor/checkpoint/backup/export/import/vacuum；CLI host-admin；Desktop Maintenance/Health；MCP 不提供 host-admin maintenance | job generation/fingerprint/lease/retry/degraded；derived 可删可 rebuild；cleanup 不得删 canonical；domain search/graph/vector/atom-index `rebuild`/`sync` 仍走各自 operation | `maintenance_status_and_run_release_owner_lease`；`maintenance_rebuild_executes_search_graph_and_leaves_unavailable_vector_pending`；`maintenance_cleanup_does_not_delete_canonical_facts`；`maintenance_lease_competition_is_fail_closed` | `implemented-evidence`; final maintenance/full gate pending |
| schema/migration metadata | service schema/migration | host startup + maintenance doctor；CLI/HTTP/Desktop host-admin | v1→v2 exact shape + verified sibling backup + transaction rollback；portable JSONL 与 legacy SQLite v30 走独立 journal/fingerprint | `fresh_database_records_full_turso_lineage`；`current_v1_fixture_is_adopted_and_upgraded`；`unknown_same_number_schema_is_rejected_without_adoption`；`migration_failure_rolls_back_schema_and_ledger_changes`；portable replace/rollback tests | `implemented-evidence`; v30 feature gate/adoption pending |

## 2. Lifecycle parity

| baseline operation | 当前 owner / surfaces | 迁移规则 | 证据 | 状态 |
| --- | --- | --- | --- | --- |
| `promote`、baseline `start`/当前 `claim`、`heartbeat`、`review`、`done`、`block` | service lifecycle；HTTP `/transitions/*`；CLI task；MCP lifecycle；Desktop task actions | 保留 plan/依赖/排期/owner/token/CAS；claim + run + event 同事务；baseline `start` 词法折叠为 `claim`，不是第二条状态路径 | claim concurrency/guard、review/done/block server tests；service lifecycle suites；CLI completion 明确不再建议 `task start` | `implemented-evidence`; exact `start` spelling compatibility remains `pending-gate` |
| `release` | service release；HTTP/CLI/MCP/Desktop | 新增 matching token release：cancel run、clear lease、回 ready、写 event | `release_task_returns_ready_and_cancels_run_atomically`；server release route test | `implemented-evidence` |
| `specify`、`unblock`、`reopen`、`reclaim`、`archive`、task `update` | service explicit operations；HTTP/CLI/MCP/Desktop task detail/actions | 不允许 generic status setter；按 canonical facts 重算目标；保留历史与 retry/event | `specify_task_recomputes_unplanned_task_to_todo`；`unblock_task_recomputes_blocked_task_without_forcing_ready`；`reopen_task_clears_completion_but_preserves_result_and_recomputes_children`；`explicit_reclaim_expires_run_in_one_transaction_and_increments_retry`；`archive_task_sets_archived_state_and_event` | `implemented-evidence`; full surface gate pending |
| steps `done/skip/reopen/remove` | service step lifecycle；HTTP/CLI/MCP/Desktop detail | required step、linked task board、parent plan guard；状态和 event 同事务 | `step_lifecycle_routes_share_one_application_and_store_path`；Desktop steps contract tests | `implemented-evidence`; full adoption gate pending |

## 3. Surface parity

### HTTP / typed client

`kanban-server/src/http/operations` 当前覆盖 board/task/lifecycle/steps/comments/attachments/dependencies/entities/graph/search/context/labels/ontology/signals/runs/events/stats/maintenance；`crates/kanban-server/src/vector.rs` 注册 vector 路径。`kanban-client` 的 operations 与 server route 对应，失败映射统一 protocol error。Ontology router 已同时注册 task-scoped 与 board-wide label proposal list；后者是 `GET /api/v1/boards/:board/label-proposals`，可选 `status` query。

实际 evidence：server route tests、vector fixture producer/consumer tests、`kanban-protocol` endpoint catalog/schema tests。`schema-surface-audit`、`schema-adoption-witness` 和完整 package/full tests 的当前状态以 §5 为准，不能从 route inventory 数量推断完成。

### CLI

当前 Clap 顶层覆盖 `serve`、board/config/task/label/comment/context/attachment/dep/entity/graph/events/runs/run/search/index/signal/vector、doctor/stats/backup/export/import/import-v30/checkpoint/vacuum/maintenance、init/completions/__complete/hook。canonical leaf 已包含 `board columns`、`entity upsert`、`task specify`、`graph neighborhood`、`graph map`、`index rebuild` 和 `index sync`；CLI contract tests 已覆盖 board/task/label/maintenance/config 等 adapter；`surface.rs` 中非 JSON 输出（serve、completion、raw attachment、hook handler）明确 `excluded`，不算 JSON adoption。visible alias 不单独形成 surface operation。

迁移规则：普通 command 只调用 client；host-admin 命令只调用 host；不恢复 baseline direct DB path。剩余 gate 是 clap leaf 与 protocol surface inventory 的精确审计，以及未在本轮运行的 exact witness/full tests。

`label proposals list` 的 `--task-ref` 是可选的；省略时按当前 board 调用 board-wide proposal API，不能误写为只支持 task scope。

### MCP

baseline 没有 MCP；当前 `kanban-protocol::MCP_OPERATION_CATALOG` 是唯一 machine-readable
source，共 103 个 tool，绑定全部 103 个非 host-admin HTTP operation。`MCP_HOST_ADMIN_OPERATION_IDS`
明确禁止 12 个 host-admin operation；search/graph/vector 与 label atom-index 的 domain
`rebuild`/`sync` 不在禁止项内。MCP 只调用 `KanbanClient`，不启动 host、不提供
migration/backup/vacuum/replace。

实际 evidence：protocol `catalog_is_valid_and_has_unique_sorted_tool_names`、
`host_admin_operations_are_never_bound`，以及 `kanban-mcp` 的 `tool_inventory_is_stable`、各
family independent locatability tests 和 protocol tool schema。剩余 gate 是 MCP
package/adoption witness 的实际执行与 Desktop/MCP cross-surface acceptance。

### Desktop

baseline 的十个 `OperatorView` 现在全部存在并由 `primaryViews/sidebarViews` 导航：`board`、`list`、`map`、`events`、`runs`、`signals`、`ontology`、`maintenance`、`health`、`settings`。task detail 还有 comments、dependencies、steps、runs/events、attachments、labels/context typed API。

实际 evidence：`task-detail-capability-cutline.test.ts`、`MaintenanceView`/`SignalsWorkbench`/`OntologyReviewWorkbench`/`BoardTaskMapView` tests、API contract tests、`layout-scroll-contract`。剩余 gate 是运行 `just desktop-check` 与完整 UI/API cross-surface adoption；未运行前不标 green。

### Baseline surface closure

baseline HTTP catalog 还有一个当前没有 owner/entry 的真实缺口：`api.bootstrap-task-label`，即
`POST /api/v1/tasks/:task_id/labels/bootstrap`。baseline 的
`crates/kanban-contract/src/endpoint.rs`、`handlers::tasks::bootstrap_task_label`、route
registration 和 adoption tests 都存在；当前 route、service operation、typed client、CLI、MCP
和 Desktop 均没有绑定。`BootstrapTaskLabelRequest`/`BootstrapTaskLabelData` 仍在
`kanban-protocol` 中只是 orphan DTO，不能算 runtime parity。它需要 canonical service
transaction、HTTP/client adapter 以及各入口是否暴露的明确决策；本账本不把它标为
`historical` 或“暂不支持”，状态保持 `pending-gate`。

baseline CLI leaf 的差异也逐项记账，避免从“没有同名命令”误推断已闭合：

| baseline leaf | 当前事实 / 迁移 | 状态 |
| --- | --- | --- |
| `task start` | 语义由 `task claim` 承载；completion test 明确不再建议 `task start`，未保留词法 alias | `pending-gate`（若验收要求 exact spelling） |
| `label bootstrap` | 与缺失的 `api.bootstrap-task-label` 是同一能力缺口，当前无 service/HTTP/client/CLI/MCP/Desktop 纵向路径 | `pending-gate` |
| identity `label delete` | 当前只有 label-semantics delete；没有 baseline identity delete 的 HTTP/CLI owner | `pending-gate`（需决定是否保留该 baseline capability） |
| `outbox list`、`derived status` | 旧 control plane 已由 host `maintenance status`/`doctor` 的 `projection_jobs`/`projection_state` 取代；不复制旧表 rows | `historical` semantic replacement；exact leaf 若仍是验收项则 `pending-gate` |
| hidden `dispatch` | 语义并入 canonical host `kanban serve --dispatcher-profile`，只 claim `ready`；有真实 serve/dispatcher adoption test | `implemented-evidence` semantic migration |
| `maintenance cleanup-legacy` | 当前 `maintenance cleanup` 只清理 projection；旧 sidecar cleanup 没有同名入口，保留为历史 decision | `historical`；exact compatibility 若要求则 `pending-gate` |

baseline 没有 MCP，因此当前 103 个 domain tools 是新增 surface，不构成 baseline 缺失；MCP
catalog 的 exact binding 仍以 §5 gate 为准。

## 4. 双迁移路径和停止条件

### Turso v1 → v2

host 先精确检查 schema family/shape/constraints/FK/board guard，创建并验证 sibling backup，再事务升级；任何 drift、backup/integrity 失败或 migration error 都 fail-closed/rollback。现有 db tests 已覆盖 fresh、v1 adoption、unknown same-number rejection、backup hook failure、migration rollback 和 drift rejection。

### portable JSONL / legacy SQLite v30

portable path 导出/导入 canonical facts，`replace=true` 在 host 独占窗口中 verified backup + atomic canonical transaction，提交后 enqueue rebuild。legacy v30 path 只读 SQLite source、preflight schema/attachment/checksum/board isolation，显式 `legacy-sqlite-import` feature 下执行；默认构建未加载 importer，不能称第二 backend。

现有 maintenance tests 覆盖 portable round-trip、checksum failure、explicit IDs/relations、replace commit/rollback/writer lock/idempotency；legacy importer unit tests 覆盖 v30 manifest、source preflight、attachment staging。待运行的是 feature-enabled host/CLI/Desktop end-to-end adoption 和完整 recovery gate。

### 旧 SQLite v30 表逐项闭合

`LEGACY_TABLES` 的 35 张表已经按 source shape 校验；`CANONICAL_TABLES` 的 27 张表才进入
canonical import：

- canonical facts（27）：`boards`、`board_columns`、`tasks`、`task_execution_plans`、`task_steps`、`task_dependencies`、`task_runs`、`task_comments`、`task_events`、`task_attachments`、`labels`、`task_labels`、`app_settings`、`task_subtasks`、`entities`、`relation_predicates`、`entity_relations`、`label_semantics`、`label_atoms`、`label_semantic_proposals`、`label_ontology_observations`、`label_ontology_signals`、`label_ontology_actions`、`label_ontology_action_signals`、`label_ontology_action_atom_effects`、`signal_observations`、`signals`。
- source-only shape witness（8）：`derived_store_state`、`index_outbox`、`label_atom_index_boards`、`projection_database`、`projection_deliveries`、`projection_maintenance_owner`、`projection_store_state`、`schema_migrations`。

旧 control-plane 到当前 host state 的映射是显式的，而不是旧 row 的隐式复制：

- `derived_store_state` + `projection_store_state` → 当前 `projection_state`；
- `index_outbox` + `projection_deliveries` → 当前 `projection_jobs`；
- `projection_database` → 当前 `schema_identity`/host metadata，重新建立 identity，不复制旧 projection database row；
- `schema_migrations` → 当前 schema migration metadata，重新校验 lineage，不作为业务事实导入；
- `label_atom_index_boards`、`projection_maintenance_owner` → 当前可重建的 atom-index/maintenance lease host state，不进入 canonical facts。

因此 importer 只发布上述 27 张 canonical fact 表；8 张 source-only 表既不被静默忽略，也不被当作第二
control plane。证据是 `legacy_import.rs` 的 `v30_manifest_has_exact_table_and_migration_shape`、
`source_preflight_rejects_non_v30_without_touching_source` 和 staging checksum/size tests；feature
enabled 的 host/CLI/Desktop end-to-end 仍按 §5 保持 `pending-gate`。

### 停止条件

- 缺少 schema/constraint、service owner、真实 HTTP/client 或所需 adapter 时，领域保持 `pending-gate`；
- schema catalog 的 `adopted` 不等于 server/CLI/MCP/Desktop/full gate 已通过；
- FTS/vector/graph/context 的 provider/index/worker 故障只能标记 degraded，不能改 canonical facts；
- old sidecar/backend/helper 已删除，不建立 compatibility shim；历史文档必须标为 archive；
- release、package、push、PR、merge 和发布不属于本账本的完成条件。

## 5. Final gates（本次未宣称通过）

| gate | 目的 | 本次状态 |
| --- | --- | --- |
| `just docs-check` | 文档链接、rustdoc include、crate README 和 ADR index | 本次尝试受并发代码编译失败影响，未形成通过证据；稳定主线需复跑 |
| `just diff-check` | 文档空白/冲突检查 | 本次运行 exit 0 |
| `just schema-check` | protocol schema/catalog 一致性 | 本任务未运行，保持 `pending-gate` |
| `just schema-surface-audit` | 实际 HTTP/CLI surface 与 catalog 对齐 | 本工作树已实际执行：server exact route 与 CLI exact leaf tests 均通过；最终集成 HEAD 仍需复跑，状态 `pending-gate` |
| `just schema-adoption-witness` | exact producer/consumer witness | 本任务未运行，保持 `pending-gate` |
| final HEAD / full gate | 集成后的最终 revision 与完整 runtime/full 证据 | pending；本账本不预先宣称通过 |
| 受影响 Rust/CLI/MCP/Desktop package tests | 纵向行为和 UI contract | 本任务未运行 package/full gate；已有代表性测试名仅作证据索引，未运行项保持 `unknown` |
| release/package/PR | 发布/外部协调 | 明确不在本任务范围 |

本 ledger 的最终状态必须在上述 gates 实际执行后更新；未运行的 gate 永远保持 `pending-gate`/`unknown`，不能用文档或生成物替代测试证据。

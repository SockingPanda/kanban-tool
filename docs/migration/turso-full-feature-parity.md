# Turso 全功能 parity ledger

本账本以 `6ea277583e51ea010aa6739a53091337676b4cff` 为功能基线（下文简称 baseline），以当前
HEAD `7c6fa714` 的 `kanban-service`/`kanban-server`/`kanban-protocol` 实现为事实快照。它回答
四个问题：baseline 每个领域现在由谁拥有、HTTP/CLI/MCP/Desktop 通过什么入口、数据/语义如何
迁移，以及哪些测试为闭合提供了可审计证据。

这里的“功能闭合”指每个 baseline 业务事实都有一个 canonical Turso owner、共享 application
path 和可核对的公开 surface；wire 或 CLI 的旧 spelling 不兼容单独记为兼容性差异，不再被误判
为功能缺失。更宽的 package/full/release gate 若未在本次文档变更中重跑，只在 §5 标为
`not-run`，不改变已闭合的实现结论。

本账本不把 catalog、孤立文件或一次未运行的命令替代 runtime 证据。状态含义：

| 状态 | 含义 |
| --- | --- |
| `implemented-evidence` | 当前 HEAD 已有 canonical owner、公开入口和代表性 runtime/adoption witness；无待补的功能纵向切片 |
| `protocol-adopted` | protocol/schema surface 已标为 `adopted`；与 owner/runtime 证据一起构成功能闭合，不等于本次重跑所有 package gate |
| `not-run` | 本次文档任务没有重跑该较宽验证；不是功能缺口 |
| `historical` | baseline/旧 sidecar 的迁移证据；不属于 active runtime |

## 0. 快照与证据口径

baseline 的 machine source 是 `6ea277` 中的 migration、`crates/kanban-contract`、CLI args、Desktop navigation 和 helper/backend crates；当前 source 是 `Cargo.toml`、`crates/kanban-service/src/schema.rs`/`service.rs`、`crates/kanban-server/src/http/operations/**`、`crates/kanban-client/src/**`、`crates/kanban-cli/src/main.rs`、`crates/kanban-protocol` catalog、`crates/kanban-mcp/src/main.rs` 的 adapter 对齐测试和 `apps/desktop/src`。

legacy SQLite v30 的表清单以 `crates/kanban-service/src/legacy_import.rs::{LEGACY_TABLES, CANONICAL_TABLES}` 为 machine source：共 35 张源表，其中 27 张是 canonical facts，另外 8 张只是旧 projection/outbox/control-plane 的 shape witness。这里的表数不等同于旧 workspace 的 backend/helper crate 数；后者仅作为历史 source map 保留。

可复核命令：

```bash
git show 6ea277583e51ea010aa6739a53091337676b4cff:migrations/001_initial.sql
git show 6ea277583e51ea010aa6739a53091337676b4cff:apps/desktop/src/features/navigation/view-types.ts
rg -n '\.route\(' crates/kanban-server/src/http/operations crates/kanban-server/src/vector.rs
rg -n 'mcp_operation_catalog|mcp_host_admin_operation_ids|project_mcp_policy' crates/kanban-protocol/src/mcp.rs crates/kanban-mcp/src/main.rs
```

当前 active workspace 明确是七个产品 Rust crate（`kanban-core`、`kanban-service`、`kanban-protocol`、`kanban-client`、`kanban-server`、`kanban-cli`、`kanban-mcp`）、Desktop Tauri package 和私有 `xtask`。旧 backend/helper sidecar 不再是 workspace/runtime 成员；它们只在 baseline source map、历史 release/projection 文档或迁移 fixture 中作为证据保留。

HEAD 的闭合线索可由 Git history 和 owner 测试复核：`f5bcb1f0`/`804840f4`/`6d6c29fb`/`818d8b73`
恢复 label bootstrap 的 service、HTTP/client、CLI 和 protocol；`e216df6b`/`6d9496ef`/`8a8f316b`
恢复 identity label delete 的 service、HTTP/client 和 CLI；`97a93bfb`、`f7fd376e`、`ba2a54a8`
等提交将 endpoint、CLI、MCP surface 收敛到 declaration source。当前
`crates/kanban-protocol/tests/foundation.rs::current_train_freeze_requires_closed_authority`
断言 endpoint obligation 没有 `Todo`、没有 `Planned`/`Generated` contract 或 surface，且所有 API
endpoint 均为 `adopted`；`crates/kanban-server/src/router.rs::contract_catalog_tests::api_route_catalog_matches_exact_contract_catalog`
与 `crates/kanban-cli/src/main.rs::tests::clap_leaf_commands_match_exact_contract_catalog` 分别核对真实 route/Clap
leaf。

## 1. 数据域映射

| baseline 域/事实 | 当前 owner | HTTP / CLI / MCP / Desktop | 迁移规则 | 实际测试证据（代表性） | 状态 |
| --- | --- | --- | --- | --- | --- |
| `boards`、`board_columns` | `kanban-service` board operations + Turso schema | `GET/POST /api/v1/boards`、board detail/archive/columns；`kanban board list/create/show/archive/columns`；`board_list/create/show/archive`；Desktop board/list/settings | 保留 board identity、columns、archive guard；board 归档不改 task status，拒绝 active running work | `board_routes_cover_create_duplicate_archive_and_active_only_get`；`board_queries_use_the_initialized_host_database`；CLI board contract tests | `implemented-evidence` |
| `tasks`、plan、steps、dependencies | service task/step/dependency operations + state machine | `/boards/:board/tasks`、`/boards/:board/tasks/by-status`、`/tasks/:id`、`/steps`、`/dependencies`；CLI `task`/`dep`；MCP task/step/dependency tools（含 `task_list_by_status`）；Desktop board/list/detail | 保留 global ID、board seq、plan gate、same-board FK、cycle guard、idempotency；状态只走显式 command；by-status 每个窗口复用 canonical list filters/sort/pagination | `suite_tasks_crud_and_reads_use_committed_fixtures_through_router`；`list_tasks_*`；`claim_task_*`；`step_create_and_list_use_application_path_and_entity_local_idempotency`；`dependency_create_and_list_use_the_shared_application_path`；CLI task/board/maintenance contract tests | `implemented-evidence` |
| `task_runs`、logs | service lifecycle/run read operations | `/tasks/:id/runs`、`/runs/:id`、`/runs/:id/log`；`kanban runs/run show/run logs`；MCP run tools；Desktop Runs/detail | run 只能由 claim 创建；固定 256 KiB log snapshot；lifecycle 和 run/event 同事务；不引入独立 run mutation | `run_list_uses_application_path_and_preserves_run_contract`；`run_show_reads_the_run_created_by_the_canonical_claim_path`；`run_log_route_uses_the_application_and_returns_contract_shape` | `implemented-evidence` |
| `task_comments`、decision metadata、signal backlink | service comment/signal transaction | `/tasks/:id/comments`、`/boards/:board/signals`；`kanban comment`/`signal`；MCP comment/signal；Desktop detail/Signals | comment idempotency 归 task；signal record/backlink/event 同事务；保留 unknown metadata JSON | server comment/signal tests；`signal_tools_are_independently_locatable`；Desktop comments/signals contract tests | `implemented-evidence` |
| `task_attachments`、staging | service attachment + host filesystem root | `/tasks/:id/attachments` list/create/download/delete；CLI/MCP attachment；Desktop TaskAttachmentsPanel | metadata 进 Turso；content 先 staging+fsync+SHA，再原子 publish；删除移 `.trash/`，事务失败恢复；portable/v30 由 journal 关联 | attachment server/desktop contract tests；service attachment/path guard capability test | `implemented-evidence` |
| `labels`、`task_labels` | service label facts/transactions | `/api/v1/boards/:board/labels`、`/api/v1/tasks/:id/labels`、`POST /api/v1/tasks/:id/labels/bootstrap`、`DELETE /api/v1/boards/:board/labels/:label_id`；CLI `label`（含 `bootstrap`/`delete`）；MCP label tools；Desktop detail/ontology | identity 与 binding 保留 board composite FK；task create/list/query 通过 service labels 参数；bootstrap、delete 和 remove 都是显式事务，不隐式删除 semantics/atoms | `labels_round_trip_is_idempotent_and_emits_add_remove_events`；`knowledge_adoption::bootstrap_task_label_request_and_response_fixtures_reach_real_host`；`suite::labels_adoption::delete_board_label_response_fixture_is_produced_by_real_router`；`bootstrap_label_flow_through_real_cli`；`label_delete_flow_through_real_cli` | `implemented-evidence` |
| `label_semantics`、`label_atoms`、proposal | service ontology facts + Turso vector atom index | task proposal：`/api/v1/tasks/:task_id/label-proposals`；board-wide proposal：`GET /api/v1/boards/:board/label-proposals`（可选 `status`）；CLI `label semantics/atoms/atom-index/suggest/propose/proposals`（`--task-ref` 省略时按 board 列出）；MCP `label_proposals_list`；Desktop Ontology workbench | semantics CAS/hash、atom effect、proposal accept/reject 保留；atom index 可删可 rebuild；provider degraded 不写错误 canonical；task/board proposal 查询都经 `KanbanService` | `delete_semantics_removes_derived_atoms`；`apply_and_revert_keep_canonical_hashes_and_atoms_in_sync`；`board_proposal_list_uses_the_board_scoped_service_path`；`label_proposal_routes_consume_typed_fixtures_and_persist_real_proposal`；CLI label contract tests | `implemented-evidence` |
| ontology ledger (`observations/signals/actions/effects`) | service ontology ledger | `/label-ontology/*`；CLI `label ontology ...`；MCP ontology tools；Desktop Ontology | record/review/quality/confirm/reject/resolve/supersede/apply/revert/validate 共用 CAS、board guard、event；不将 graph projection 当事实 | `crates/kanban-service/src/store_operations/ontology_tests.rs`；server ontology route fixtures；CLI ontology contract tests | `implemented-evidence` |
| generic `signal_observations`、`signals` | service signal ledger | `/boards/:board/signals{,/review,/confirm...}`、`/signals/:id`；CLI `signal`；MCP signal tools；Desktop Signals | record、backlink、review transition 与 event 原子提交；同 board dedupe key idempotent | server signal record/review tests；MCP `signal_tools_are_independently_locatable`；Desktop SignalsWorkbench tests | `implemented-evidence` |
| `entities`、`relation_predicates`、`entity_relations`、baseline substrate | service canonical relation store | `/entities`；`entity list/show/upsert`；MCP graph/task map；Desktop Map/context | relation facts 迁入 Turso；board composite FK；旧派生控制面改为 host `projection_jobs/state`，不保留第二 control plane | `entity_upsert_list_and_show_are_available_on_host`；`unknown_entity_is_not_found`；`graph_reads_canonical_relations_with_cycle_safe_bounded_bfs`；`graph_neighbors_enforces_board_isolation` | `implemented-evidence` |
| baseline search/index state | Turso FTS + service search operations | `/search/tasks`、`/search/tasks/by-status`、`/search/status`、`/search/index/rebuild`、`/search/index/sync`；CLI `search/index status|doctor|rebuild|sync`；MCP search；Desktop list/context | `retrieval_documents` + `task_search_fts` 从 canonical task/comment/run/event 重建；FTS stale/error 回退 canonical SQL；旧 external index 不迁移为 owner | `fts_capability_exercises_insert_update_delete_score_and_highlight`；`rebuilds_turso_fts_and_falls_back_for_exact_references`；server search route tests；CLI index contract tests | `implemented-evidence` |
| vector task chunks / label atoms | Turso `vector32` + host Ollama provider | `/vector/status/configure/rebuild/sync/query-*`；CLI vector；MCP read-only vector；Desktop context/ontology typed API | 导入只保留 canonical source；embedding/model/dimension/fingerprint 作为 derived metadata；provider outage 设 degraded 并保留 job | `vector32_roundtrip_dimension_and_cosine_are_real_turso_capabilities`；`vector_routes_use_typed_envelopes_and_degraded_query_error`；vector fixture producer/consumer tests；MCP vector inventory | `implemented-evidence` |
| graph/BFS/task map | service bounded BFS over canonical relations | `/graph/status/neighbors/query/rebuild/sync`、`/tasks/:id/neighborhood`、`/boards/:board/task-map`；CLI graph；MCP graph；Desktop Map | BFS depth/cycle/dedup/board isolation；graph projection 删除后 rebuild；不恢复 Oxigraph/helper protocol | `graph_status_and_task_map_routes_are_adopted`；`graph_maintenance_routes_publish_generation_and_counts`；Desktop TaskGraph tests | `implemented-evidence` |
| context pack | service context merge + FTS/BFS/vector adapters | `/tasks/:id/context`；CLI `context build`；MCP `context_build`；Desktop typed `KanbanApi.buildContext` | subject/reference/query selector；budget/depth/lexical/graph/vector limit；按 provenance 去重；provider degraded 保留可用部分 | `context_merge_is_stable_deduplicated_and_ranked`；`context_merge_enforces_budget_and_board_isolation`；server context route tests；CLI/MCP context contract tests | `implemented-evidence` |
| projection state/jobs/maintenance owner | service host projection worker + maintenance lease | `/maintenance/status/run/rebuild/cleanup`、doctor/checkpoint/backup/export/import/vacuum；CLI host-admin；Desktop Maintenance/Health；MCP 不提供 host-admin maintenance | job generation/fingerprint/lease/retry/degraded；derived 可删可 rebuild；cleanup 不得删 canonical；domain search/graph/vector/atom-index `rebuild`/`sync` 仍走各自 operation | `maintenance_status_and_run_release_owner_lease`；`maintenance_rebuild_executes_search_graph_and_leaves_unavailable_vector_pending`；`maintenance_cleanup_does_not_delete_canonical_facts`；`maintenance_lease_competition_is_fail_closed` | `implemented-evidence` |
| schema/migration metadata | service schema/migration | host startup + maintenance doctor；CLI/HTTP/Desktop host-admin | v1→v2 exact shape + verified sibling backup + transaction rollback；portable JSONL 与 legacy SQLite v30 走独立 journal/fingerprint | `fresh_database_records_full_turso_lineage`；`current_v1_fixture_is_adopted_and_upgraded`；`unknown_same_number_schema_is_rejected_without_adoption`；`migration_failure_rolls_back_schema_and_ledger_changes`；portable replace/rollback tests | `implemented-evidence` |

## 2. Lifecycle parity

| baseline operation | 当前 owner / surfaces | 迁移规则 | 证据 | 状态 |
| --- | --- | --- | --- | --- |
| `promote`、baseline `start`/当前 `claim`、`heartbeat`、`review`、`done`、`block` | service lifecycle；HTTP `/transitions/*`；CLI task；MCP lifecycle；Desktop task actions | 保留 plan/依赖/排期/owner/token/CAS；claim + run + event 同事务；baseline `start` 词法折叠为 `claim`，不是第二条状态路径 | claim concurrency/guard、review/done/block server tests；service lifecycle suites；CLI completion 明确不再建议 `task start` | `implemented-evidence`；旧 `start` spelling 仅是不兼容的 CLI compatibility 差异 |
| `release` | service release；HTTP/CLI/MCP/Desktop | 新增 matching token release：cancel run、clear lease、回 ready、写 event | `release_task_returns_ready_and_cancels_run_atomically`；server release route test | `implemented-evidence` |
| `specify`、`unblock`、`reopen`、`reclaim`、`archive`、task `update` | service explicit operations；HTTP/CLI/MCP/Desktop task detail/actions | 不允许 generic status setter；按 canonical facts 重算目标；保留历史与 retry/event | `specify_task_recomputes_unplanned_task_to_todo`；`unblock_task_recomputes_blocked_task_without_forcing_ready`；`reopen_task_clears_completion_but_preserves_result_and_recomputes_children`；`explicit_reclaim_expires_run_in_one_transaction_and_increments_retry`；`archive_task_sets_archived_state_and_event` | `implemented-evidence` |
| steps `done/skip/reopen/remove` | service step lifecycle；HTTP/CLI/MCP/Desktop detail | required step、linked task board、parent plan guard；状态和 event 同事务 | `step_lifecycle_routes_share_one_application_and_store_path`；Desktop steps contract tests | `implemented-evidence` |

## 3. Surface parity

### HTTP / typed client

`kanban-server/src/http/operations` 当前覆盖 board/task/lifecycle/steps/comments/attachments/dependencies/entities/graph/search/context/labels/ontology/signals/runs/events/stats/maintenance；`crates/kanban-server/src/vector.rs` 注册 vector 路径。`kanban-client` 的 operations 与 server route 对应，失败映射统一 protocol error。Ontology router 已同时注册 task-scoped 与 board-wide label proposal list；后者是 `GET /api/v1/boards/:board/label-proposals`，可选 `status` query。labels router 还注册了 bootstrap task label 与 identity label delete。

实际 evidence：`crates/kanban-server/src/router.rs::contract_catalog_tests::api_route_catalog_matches_exact_contract_catalog`、各 family 的
fixture/adoption tests、vector fixture producer/consumer tests，以及 `kanban-protocol` 的
`current_train_freeze_requires_closed_authority`。当前 non-SSE endpoint descriptor 为 117 个，均由
catalog 标为 `adopted`；未在本次文档任务重跑的宽 gate 见 §5，不构成功能缺口。

### CLI

当前 Clap 顶层覆盖 `serve`、board/config/task/label/comment/context/attachment/dep/entity/graph/events/runs/run/search/index/signal/vector、doctor/stats/backup/export/import/import-v30/checkpoint/vacuum/maintenance、init/completions/__complete/hook。canonical leaf 已包含 `board columns`、`entity upsert`、`task specify`、`graph neighborhood`、`graph map`、`index rebuild`、`index sync`、`label bootstrap` 和 `label delete`；CLI contract/adoption tests 已覆盖 board/task/label/maintenance/config 等 adapter；`surface.rs` 中非 JSON 输出（serve、completion、raw attachment、hook handler）明确 `excluded`，不算 JSON adoption。visible alias 不单独形成 surface operation。

迁移规则：普通 command 只调用 client；host-admin 命令只调用 host；不恢复 baseline direct DB path。
`crates/kanban-cli/src/main.rs::tests::clap_leaf_commands_match_exact_contract_catalog` 将真实 leaf 与 catalog 精确比对；
`task start` 未保留是有意的 spelling compatibility 差异，语义由 `task claim` 承载。

`label proposals list` 的 `--task-ref` 是可选的；省略时按当前 board 调用 board-wide proposal API，不能误写为只支持 task scope。

### MCP

baseline 没有 MCP；当前 `kanban-protocol::mcp_operation_catalog()` 是唯一 machine-readable
source，共 105 个 tool，绑定全部 105 个非 host-admin API operation。declaration policy 明确
隔离 12 个 host-admin operation；search/graph/vector 与 label atom-index 的 domain
`rebuild`/`sync` 不在禁止项内。MCP 只调用 `KanbanClient`，不启动 host、不提供
migration/backup/vacuum/replace；`task_label_bootstrap`、`label_delete` 等恢复的 label capability
也由同一 policy 投影。

实际 evidence：protocol `catalog_is_valid_and_has_unique_sorted_tool_names`、
`host_admin_operations_are_never_bound`，以及 `kanban-mcp` 的 `tool_inventory_is_stable`、各
family independent locatability tests 和 protocol tool schema。catalog 的 declaration source
与真实 router 共享 operation ID，因此不存在旧 MCP 未有导致的功能缺口。

### Desktop

baseline 的十个 `OperatorView` 现在全部存在并由 `primaryViews/sidebarViews` 导航：`board`、`list`、`map`、`events`、`runs`、`signals`、`ontology`、`maintenance`、`health`、`settings`。task detail 还有 comments、dependencies、steps、runs/events、attachments、labels/context typed API。

实际 evidence：`task-detail-capability-cutline.test.ts`、`MaintenanceView`/`SignalsWorkbench`/`OntologyReviewWorkbench`/`BoardTaskMapView` tests、API contract tests、`layout-scroll-contract`。Desktop 继续通过 typed API 复用 host；本 ledger 不把 Desktop 是否提供某个 CLI-only leaf 当作第二套 surface。

### Baseline surface closure

baseline 的“缺口”在当前 HEAD 已逐项闭合；下表同时记录 exact spelling/wire 的兼容性边界。旧
spelling 不兼容只意味着调用者需要迁移到当前 contract，不意味着 service capability 缺失。

| baseline leaf / surface | 当前 owner 与公开 surface | 迁移 / 兼容性 | 验收证据 | 状态 |
| --- | --- | --- | --- | --- |
| `api.bootstrap-task-label`、`label bootstrap` | `kanban-service` `BootstrapTaskLabelCommand`；`POST /api/v1/tasks/:task_id/labels/bootstrap`；typed `KanbanClient::bootstrap_task_label`；CLI `kanban label bootstrap`；MCP `task_label_bootstrap`；Desktop 保留 ontology action/read contract，但没有独立 CLI-only leaf | 原子创建 semantics、atoms、binding、ontology action/event；staged verification 可选且 provider degraded 不写错误 canonical | `knowledge_adoption::bootstrap_task_label_request_and_response_fixtures_reach_real_host`；`bootstrap_label_flow_through_real_cli`；`labels_catalog` 的 adopted path/header/request/response contracts | `implemented-evidence` |
| identity `label delete` | `kanban-service` `DeleteBoardLabelCommand`；`DELETE /api/v1/boards/:board/labels/:label_id`；typed `KanbanClient::delete_board_label`；CLI `kanban label delete`；MCP `label_delete`；Desktop 不复制 CLI-only destructive leaf | `force` 只显式移除 task bindings；semantics/atoms 删除结果由 service 返回并记录，不走 semantics delete 的第二路径 | `suite::labels_adoption::delete_board_label_response_fixture_is_produced_by_real_router`；`label_delete_flow_through_real_cli`；`labels_catalog` adopted contracts | `implemented-evidence` |
| `task start` | service claim lifecycle；`POST /api/v1/tasks/:task_id/transitions/claim`；CLI/MCP/Dispatcher 使用 `claim` | baseline CLI spelling 未保留 alias；completion test 明确不再建议 `task start`。这是有意的 CLI compatibility 差异，claim/run/event 语义已闭合 | `claim_task_*`、`release_task_returns_ready_and_cancels_run_atomically`；`dispatcher_profile_is_consumed_by_real_serve_and_only_claims_ready`；completion negative assertion | `implemented-evidence`（old spelling incompatible） |
| `outbox list`、`derived status` | service `projection_jobs`/`projection_state`；`GET /api/v1/maintenance/status`、`GET /api/v1/maintenance/doctor`；CLI `maintenance status`/`doctor`；Desktop Maintenance/Health | 旧 control-plane rows 不复制；状态、失败、dirty、pending/running/failed outbox 计数由 host doctor/status 暴露。旧 CLI leaf 不兼容，但语义由 host-admin owner 承载 | `maintenance_status_and_run_release_owner_lease`；`doctor_response_maps_real_non_default_report_before_fixture_normalization`；Desktop maintenance contract tests | `implemented-evidence`（semantic replacement） |
| hidden `dispatch` | `kanban-server` dispatcher + `kanban serve --dispatcher-profile`；CLI admin adoption；共享 service claim path | 只 claim `ready`，不 claim `review`；不保留 baseline hidden command 或 direct DB worker | `dispatcher_profile_is_consumed_by_real_serve_and_only_claims_ready`；CLI `cli_admin_adoption` dispatcher flow | `implemented-evidence`（semantic migration） |
| `maintenance cleanup-legacy` | 旧 sidecar 已由 workspace/runtime 删除；当前 service owner 提供 `maintenance cleanup`（projection jobs）和 `import-v30`（只读 legacy source） | 无旧 sidecar 运行时对象可清理，因此 exact leaf 退役不是功能缺失；保留为历史 source-map spelling，不建立 compatibility shim | `a6489cc3` retired backend removal；`maintenance_cleanup_does_not_delete_canonical_facts`；`legacy_import` v30 preflight/attachment tests | `historical`（retired spelling） |

baseline 没有 MCP；当前 105 个 domain tools 是新增 adapter surface，同时通过 declaration policy
绑定全部非 host-admin API operation。它们不会把 baseline 的 absence 误写成缺失，也不会绕过
`KanbanClient` 建立第二条 mutation path。

## 4. 双迁移路径和停止条件

### Turso v1 → v2

host 先精确检查 schema family/shape/constraints/FK/board guard，创建并验证 sibling backup，再事务升级；任何 drift、backup/integrity 失败或 migration error 都 fail-closed/rollback。现有 db tests 已覆盖 fresh、v1 adoption、unknown same-number rejection、backup hook failure、migration rollback 和 drift rejection。

### portable JSONL / legacy SQLite v30

portable path 导出/导入 canonical facts，`replace=true` 在 host 独占窗口中 verified backup + atomic canonical transaction，提交后 enqueue rebuild。legacy v30 path 只读 SQLite source、preflight schema/attachment/checksum/board isolation，显式 `legacy-sqlite-import` feature 下执行；默认构建未加载 importer，不能称第二 backend。

验收证据覆盖 portable round-trip、checksum failure、explicit IDs/relations、replace commit/rollback/
writer lock/idempotency；`crates/kanban-server/src/suite/portable_adoption.rs` 还验证 rich fixture
导出、导入和 replace 后 canonical facts 相等。`legacy_adoption.rs` 在
`legacy-sqlite-import` feature 下通过真实 HTTP `/api/v1/maintenance/import-v30` 验证 27 张事实表
中的代表性 rows、依赖、附件 staging 和 source preflight；默认 feature 则 fail-closed 返回
`NOT_IMPLEMENTED`，不会暗中启用第二 backend。

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
`source_preflight_rejects_non_v30_without_touching_source` 和 staging checksum/size tests，以及
server `suite::maintenance_adoption::legacy_import_v30_request_producer`、
`suite::maintenance_adoption::legacy_import_v30_request_consumer`、
`suite::maintenance_adoption::legacy_import_v30_response_producer`、
`suite::maintenance_adoption::legacy_import_v30_response_consumer` adoption fixtures；feature-enabled
path 的更宽 package/full rerun 状态记录在 §5，不作为 capability gap。

### 迁移边界

- 当前 HEAD 已有 schema/constraint、service owner、真实 HTTP/client 以及需要的 CLI/MCP/Desktop
  adapter；旧 spelling 不兼容时按上表迁移，不重新建立第二条 mutation path。
- schema catalog 的 `adopted` 与 route/Clap/runtime witness 一起证明 surface 闭合；更宽 package/full
  gate 是否在本次重跑只见 §5，不用文档或生成物冒充执行结果。
- FTS/vector/graph/context 的 provider/index/worker 故障只能标记 degraded，不能改 canonical facts；
  这是 active runtime 不变量，不是迁移缺口。
- old sidecar/backend/helper 已删除，不建立 compatibility shim；其名称只在 baseline source map、
  migration fixture 或 historical release/projection 文档中保留。
- release、package、push、PR、merge 和发布不属于本账本的完成条件。

## 5. 验证记录与边界

本次只为 ledger 做文档闭合；“`not-run`”表示本次没有重复执行较宽 gate，不表示实现仍缺
能力。协议 source 自带的 `current_train_freeze_requires_closed_authority`、server route catalog、
CLI leaf catalog、MCP projection 和各 family adoption witness 仍是功能闭合的可核对证据。

| gate | 目的 | 本次状态 |
| --- | --- | --- |
| `just docs-check` | 文档链接、rustdoc include、crate README 和 ADR index | 通过（`export PS1=''; just docs-check`，exit 0；仓库 shell 启动有既有 `PS1` warning，不影响 gate） |
| `just diff-check` | 文档空白/冲突检查 | 通过（`export PS1=''; just diff-check`，exit 0） |
| `just schema-check` | protocol schema/catalog 一致性 | `not-run`；HEAD 的 foundation closure test 已断言无 `Todo`/未迁移 surface |
| `just schema-surface-audit` | 实际 HTTP/CLI surface 与 catalog 对齐 | `not-run`；对应精确测试 locator 已写入 §0 和 protocol catalog |
| `just schema-adoption-witness` | exact producer/consumer witness | `not-run`；adopted contract 的 producer/consumer locator 由 declaration source 持有 |
| final HEAD / full gate | 集成后的最终 revision 与完整 runtime/full 证据 | `not-run`；属于 CI/发布层，不是本次 ledger 的 capability 判断 |
| 受影响 Rust/CLI/MCP/Desktop package tests | 纵向行为和 UI contract | `not-run`；代表性测试名和 owner 已在 §1–§4 索引 |
| release/package/PR | 发布/外部协调 | 明确不在本任务范围 |

因此本账本不再列出 bootstrap、identity delete 或其他 baseline domain 的待补功能；它们的
owner、route/client/CLI/MCP surface、迁移规则和 adoption evidence 已在上文闭合。精确 wire/CLI
spelling 的不兼容性已单独标注，不作为功能缺失。

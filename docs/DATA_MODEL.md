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

详细 HTTP/CLI 入口见 [`API_SPEC.md`](API_SPEC.md)、[`CLI_SPEC.md`](CLI_SPEC.md)；按 baseline 映射的 migration/test/gate 见 [`migration/turso-full-feature-parity.md`](migration/turso-full-feature-parity.md)。

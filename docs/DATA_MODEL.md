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
`task_search_fts`，因此启用 FTS 后实际 index 对象为 46 个。trigger
由 `FULL_REQUIRED_TRIGGERS` 固定为 22 个；host 启动后会重建两个 projection guard，并追加
task event/delete 的 FTS outbox trigger。后四个运行时 trigger 不改变 v2 canonical 指纹。

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

portable JSONL 导入目标可以是只含 host bootstrap board/columns 的新 Turso 数据库，且不
把 projection/FTS/vector/graph 派生表当作业务事实；事务阶段写入 `import_journal`，失败
会记录 `failed`。旧 SQLite v30 的 schema/attachment preflight、staging 和原子文件发布
仍需专门 importer 逻辑（默认 feature 不启用）；typed host-admin 入口可以在启用
`legacy-sqlite-import` 后调用它，不能因为表已经存在就声称 SQLite 导入已完成。

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
保存 metadata、相对路径、size 和可选 SHA-256；路径 trigger 拒绝绝对路径和 `..` 穿越，
runtime service 进一步要求路径位于 `{board_id}/{task_id}/` 目录。文件复制和发布由
host-owned attachment service 负责：同文件系统 staging 写入、文件与目录 `fsync` 后原子
发布，数据库字段不能成为任意文件读取入口。删除先移动到 attachment root 的 `.trash/`，
数据库事务失败时恢复 canonical path；trash 是可恢复删除证据，不是 canonical 事实。

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

公开 attachment API 已由 `kanban-server`/`kanban-client`、CLI、MCP 接通；Desktop task detail
可直接复用 typed client endpoint。目标导入流程仍是只读打开 SQLite v30，先做 schema、计数、引用、attachment checksum 和 board
isolation preflight，再将附件复制到同文件系统 staging；DB commit 后原子发布，崩溃后按
journal resume。源文件永不修改，重复 fingerprint 返回已完成结果。当前 schema 与
`kanban-store-turso` 已提供 portable JSONL service/HTTP/CLI 管理入口；SQLite v30 importer
与 attachment 文件发布仍是待闭合的 parity slice。

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

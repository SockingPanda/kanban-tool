# Architecture

`POST /api/v1/boards/:board/tasks` 的公开 path/request/success wire DTO 由
`kanban-contract` 单一拥有；server adapter 显式映射到 `kanban_sqlite::api::CreateTask`，并继续
以一次 `create_task_with_labels_and_dependencies` 调用进入 canonical transaction。Contract
status 只表达 create 输入允许的 `triage|todo|scheduled|ready`，metadata 只表达 opaque object
shape；initial-status recompute、ready 降级、labels/dependencies、retry policy、events 与 rollback
仍由 SQLite service/core 拥有。

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

当前实现已经把一组已选择的 adapter-facing DTO/port vertical slice 抽到 `kanban-application`；它不是完整 application service，也不拥有 SQLite transaction。CLI、HTTP
server、desktop 和 dispatcher 通过 `kanban_sqlite::api` 或 `SqliteApplication` 进入同一组
SQLite-backed use cases；`kanban-sqlite::service` 仍是 transaction、状态机 guard、canonical
writes、events、runs、outbox 和 provenance 的 implementation owner。`kanban-core` 承载
`TaskStatus`、ID/error/clock 和纯状态机 helper，不拥有持久化 records。

`kanban-sqlite` crate root 不再 re-export DB/init/service 符号。生产 adapter 必须导入
`kanban_sqlite::api`、`kanban_sqlite::application::SqliteApplication`，或显式的
`kanban_sqlite::db` / `kanban_sqlite::init` 基础设施模块。测试 raw inspection 入口集中到
`kanban-test-support`，crate 内部测试可使用显式 `db` / `init` 模块。

可把系统按八个运行平面理解：

| 平面 | 当前内容 | 写权限边界 |
|---|---|---|
| Interaction/adapters | `kanban-cli`、`kanban-server`、desktop、dispatcher 入口 | 转换输入/输出和 locale/message 渲染，不直接写 SQLite truth |
| Wire contracts | `kanban-contract` 的候选 Serde DTO、精确 surface catalog、operation inventory 与 schema root registry | 只定义公开机器契约候选；只有 `Adopted` 条目表示运行时采用，不拥有 service guard、SQLite record 或 runtime validation |
| Schema tooling | `kanban-schema-tool` 的 `kanban-schema` binary、metaschema/fixture 校验、manifest/hash 和 drift gate | 独立 leaf tool，不进入产品 runtime graph，也不能充当 adoption witness |
| Application contracts | `kanban-application` selected use-case DTO/port API，SQLite 实现位于 `kanban-sqlite` | adapters 逐步依赖稳定 API/DTO；该 crate 不是完整 application service |
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

  kanban-contract/
    src/
      wire.rs
      inventory.rs
      schema.rs

  kanban-schema-tool/
    src/
      lib.rs
      bin/kanban-schema.rs

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

### 2.1a `kanban-contract`

职责：

- 为逐步迁入的公开 API、CLI、JSONL、SSE、structured metadata、config 和 helper
  wire DTO 提供唯一候选归属；adapter 迁移时负责 application/SQLite record 到 wire DTO
  的显式映射。
- 默认 feature 只包含轻量 Serde 类型；唯一 additive `schema` feature 启用 `schemars`
  并公开 schema root registry。该 crate 不拥有 binary、`jsonschema`、SHA-256 或 drift tooling。
- 用精确 surface catalog 枚举实际 Axum method/path、Clap leaf command 和 JSONL
  discriminator；对应测试从真实声明生成 key，新增公开入口而未登记时自动失败。
- 对 API/SSE contract 显式记录 `operation_key`、`Path|Query|Headers|Body|Success|Error|Sse`
  location 和参数 cardinality；非 HTTP surface 显式记录 `NoTransport`。`Success` 只表达 2xx
  success，`Error` 只表达 `SharedComponent` 非 2xx response，且不新增第七 endpoint obligation。
  transport validator 负责 direction/location、operation/surface、granularity、path placeholder 和
  重复/缺失参数的 fail-closed 拓扑校验，不承担 HTTP status 或业务语义。
- 用 operation inventory 明确每个公开 surface 的方向、strictness、fixture、schema ID
  或 exclusion，并区分 `Planned`、`Generated`、`Adopted`、`Excluded`。`Generated`
  只表示离线 schema/fixture 就绪；`Adopted` 还必须绑定 direction-correct evidence：
  request/input producer 由 contract DTO 程序化序列化并精确匹配 committed fixture，consumer
  从 fixture 经真实 runtime handler；response/output producer 来自真实 adapter。双方包含
  operation/contract/surface/direction 和精确 Cargo test locator，且不共用同一个高层 exercise
  helper。Endpoint 的整体 migration state 与六项 obligation 分开收敛：
  `Generated` endpoint 可以先把已迁移的 body 声明为 adopted exact contract，但其它
  obligation 仍为 `Todo` 时不能提升为 `Adopted`；审计要求该 contract 与 runtime operation
  唯一、双向且精确绑定。witness gate 以 canonical manifest 和 Cargo package ID 锁定
  当前 workspace `kanban-contract`，要求 unconditional non-optional normal declaration 与 default
  resolve edge，并以 `--all-features --target all --edges normal,features --locked` 扫描 adopter runtime
  leakage，随后真实执行双方测试；registry/git/其它 path 的同名 package 不构成 adoption。最终 closure gate 只允许
  `Adopted` 或 `Excluded`。
- 生成显式 Draft 2020-12、离线
  `urn:kanban-tool:schema:<surface>:<semantic-name>:v1` root；schema bytes 从候选 wire type
  确定生成。fixtures 是手写正负样例，用于验证 schema 与当前候选 wire shape；
  它们本身不构成运行时采用证据。

该 crate 不依赖 `kanban-sqlite`、`kanban-server`、`kanban-cli`、desktop、
dispatcher 或 helper-heavy backend。JSON Schema 只验证 wire shape/value domain，
不能替代状态机、CAS、dependency、recompute、transaction 或 comment semantic guard。
详细生成与验证契约见 [`SCHEMA_CONTRACTS.md`](SCHEMA_CONTRACTS.md)。

### 2.1b `kanban-schema-tool`

职责：

- 独占 `kanban-schema` binary、离线 inventory audit、metaschema/fixture 校验、
  committed artifact 写入/漂移检查和 SHA-256 manifest。
- direct dependency 必须且只能是 `jsonschema`、`kanban-contract/schema`、`serde`、
  `serde_json` 与 `sha2` 这 5 条 normal edge；不得声明 dev、build、optional、alias 或
  target-specific edge，也不得依赖任何产品或内部 workspace crate。
- `autolib`、`autobins`、`autoexamples`、`autotests`、`autobenches` 与 auto build script
  全部关闭；只允许显式声明的一个 lib、一个 bin 与一个 integration test。contract 同样
  只允许一个 lib 和两个显式 integration tests；metadata 与普通文件/symlink gate 锁定
  target name、kind、lexical `src_path` 和仓库内归属。
- dependency policy 从 tool manifest 运行 full locked `cargo metadata`，锁定
  `resolve.root`、canonical tool/contract package ID 与 manifest path、五条 resolved
  direct edge、启用 `kanban-contract/schema` 后批准的逻辑 registry
  `schemars 1.2.1` edge，以及 `jsonschema=[]`、
  `schemars=[derive,schemars_derive,std]` effective feature union，并拒绝
  tool-root reachable closure 中的其它 path/git override。
- `policy/schema-tool-registry-closure.json` 是独立治理数据边界，只包含当前
  tool-root closure 的 registry packages；两个 canonical workspace path packages 不进入
  snapshot。policy 解析真实 `Cargo.lock`，要求每个 reachable registry package 唯一映射到
  64 位小写十六进制 checksum，并与按 `(name, version, source)` canonical 排序的 committed
  `{name, version, source, checksum}` 集合双向完全一致。普通 gate 只比较，禁止自动写入或
  bless；该检查证明 committed lockfile 相对 approval 的漂移，crate 内容仍由 Cargo
  fetch/build 按 registry index `cksum` 验证。
- Cargo metadata 的 `SourceId` 是 opaque identity；这里锁定的是 pinned toolchain 下
  本项目批准的 logical SourceId 字符串，不宣称其中 URL 是 Cargo 通用 canonical network
  URL。物理 index/download 可由 Cargo source replacement mirror 提供，不要求直连
  crates.io origin。
- 除该 tool 自身外，任何 workspace member 都不得以任何 dependency kind、alias、optional
  或 target-specific direct edge 引用它；六个产品 runtime graph 另由 all-features/all-target
  cargo tree gate 扫描传递性 tooling 泄漏。
- 作为 workspace leaf crate 排除在 default/core/helper/full 产品门禁之外。产品 `fmt`
  （及 `fmt-check` alias）精确选择 core packages，`fmt-full` 精确选择 core + helper，
  `schema-fmt` 则只选择 `kanban-contract` + `kanban-schema-tool`，并且必须在 schema
  dependency preflight 之后执行；不存在 workspace-wide fmt 旁路。
- 真实 `just --dump-format json --dump` parser AST hash 与 fake nested
  `just`/build-lock/cargo/python/script 有序 JSONL trace 形成双门禁，锁定上述 fmt lane、
  full/rust/test 分支、schema 子 gate、`schema-audit-closed` 的 adoption + locked audit，
  以及 `release` 从 affected self-test 到 diff-check 的 13 步精确顺序。leaf 仅由独立
  schema gates 执行格式、check、tests、clippy、生成和校验；witness gate 显式拒绝该
  tooling owner 冒充 runtime adopter。

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
- `kanban_sqlite::api` root 是 adapter/product use-case curated facade，用于 CLI、server、desktop
  和 dispatcher contract path 复用已允许的 use case、query、record 和 provenance 类型。它不拥有新的
  orchestration 语义，不是 `service::*` broad re-export，也不导出 DB connection helper、init
  helper、runtime lifecycle guard、provider/vector-store seam，或未列入 allowlist 的 service-only
  implementation helper。
- `kanban_sqlite::api::provider` 承载 adapter/test 需要显式注入 provider 或 vector store 的 seam，
  包括 `LabelProposalProvider`、manual/disabled proposal provider、`*_with` label suggestion/proposal
  helpers、label atom/vector-store status/query/rebuild/sync helpers，以及 trusted-suggestion validation DTO。
  这些符号不从 `api` root 暴露。
- `kanban_sqlite::api::lifecycle` 承载进程 runtime/replace lifecycle plumbing：
  `DatabaseRuntimeGuard`、`DatabaseReplaceGuard`、`begin_database_runtime` 和
  `begin_database_replace`。这些 guard 是 binary/runtime owner 的基础设施，不是普通 product use-case。
- `kanban_sqlite::db` 和 `kanban_sqlite::init` 仍是显式基础设施模块；`connect_file`、
  `init_database` 不从 `api` root 暴露。
- crate root 不再提供 `kanban_sqlite::*` legacy re-export；旧 root path 是 breaking change，
  并由 `tests/ui/root_legacy_reexport_removed.rs` 负向 compile contract 锁定。`api` root、
  `api::provider`、`api::lifecycle` 和显式 `db` / `init` 边界由 `public_api` trybuild contract 锁定。
- `kanban_sqlite::application::SqliteApplication` 实现 `kanban-application` 的 backend port，
  用于需要以 application API 组合 selected use-case slice 的 adapter/benchmark 路径。
- `kanban-application` DTO/trait 演进遵循 additive-first 策略：优先新增可选字段、option
  struct 或 extension trait；破坏性 DTO/trait 变更必须和 adapter 更新、public API compile
  contract 同步提交。

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
- 调用 `kanban_sqlite::api` root 中的 shared use-case 函数；需要 provider/vector-store seam 时显式使用
  `kanban_sqlite::api::provider`，需要 runtime guard 时显式使用 `kanban_sqlite::api::lifecycle`；状态判断复用
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

公开 error wire vocabulary 由 `kanban-contract::ApiErrorCode` 作为唯一闭合集合 owner。
HTTP status 映射与 operation-level transport 说明仅在 `docs/API_SPEC.md` 的
“HTTP Status Mapping”表中维护；架构文档不复制 code 表，避免与 server adapter 的实际
`KanbanError -> ApiErrorCode` 映射漂移。

`error.message` 仍是面向人的 locale-dependent 文案；状态机、service guard、CAS、
transaction 与 SQLite 错误 authority 不转移给 wire contract。

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


### Transport descriptor boundary

`kanban-contract` 是 localhost transport 的 method/path authority：其 default feature 无 runtime HTTP dependency，仍可被 leaf schema tool 离线使用。`kanban-server::router::registered_api_routes()` 仅提供显式 `adapter_id` 和真实 handler；path/method 从 contract descriptor 读取。这样 CLI/JSONL inventory 与 API/SSE transport identity 分层，server 不能自行复制 transport strings。

每个 API/SSE semantic contract 还必须显式声明 HTTP location；其它 surface 必须声明
`NoTransport`。任意 `Adopted` contract 与 endpoint exact reference 都必须保持
`granularity=Exact`。唯一 method/path、精确 `operation_key` 和单一 location 共同保证一个
`ExactSurface` contract 不可能合法绑定两个 endpoint obligations，因此不保留不可达的全局
second-binding guard。`SharedComponent` 允许被多个 endpoint 显式链接，或由同 surface 的真实
adoption witness 证明非 orphan；这两个条件是 OR。shared 永远不计入 endpoint exact coverage，
也不单独决定 endpoint migration state。

B1-C1 已把两个 board task-read endpoint 的 path/query transport 收口为 4 个 endpoint-specific
exact contract。两个 server-local typed Axum extractor 各自绑定对应 path/query DTO，并且各自只从
`parts.uri.query()` 调用一次共享 ordered parser；handler 不再持有 `Path`、`RawQuery` 或第二套
`Query<T>` extractor。parser 以 8192 bytes 为 raw 总预算；pair cap 由 9/4/3/32 repeated
budgets 加 6 个 scalar 参数推导为 54。只有 `status`、`priority`、`label`、`plan_filter` 可重复，
不同值保留首次出现顺序；重复语义值、纯 Unicode 空白 label、未知 key、旧 `search` alias、
scalar duplicate 及各字段预算越界均失败关闭。wire limit 由
`kanban-contract::MAX_TASK_READ_LIMIT` 拥有；`kanban-sqlite::service::MAX_TASK_LIST_LIMIT` 直接引用
唯一 application authority，server 对这个实际 defensive path 建立编译期相等门禁。该边界只
负责 wire grammar 与 DTO 到既有 application option 的显式映射；service 查询行为与
`kanban-core` 状态机语义未改变。两个 endpoint 的 path/query
obligation 已是 `Contract`，GET body 是 `NotApplicable`；headers 和 success response 仍为
`Todo`，因此 endpoint migration state 保持 `Generated`。


## B1-C2b task-read 响应边界

`kanban-contract` 拥有共享 `ApiTask`/`ApiLabel` 与既有 pagination primitives，两个 endpoint 各自拥有闭合 response root；server adapter 与 Desktop consumer 不另建 wire DTO。精确 wire 行为见 [API_SPEC](API_SPEC.md#b1-c2b-task-read-成功响应契约)，schema/adoption 证据见 [SCHEMA_CONTRACTS](SCHEMA_CONTRACTS.md#b1-c2b-task-read-成功响应契约)。

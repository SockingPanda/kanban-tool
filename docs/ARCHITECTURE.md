# 架构

Kanban Tool 把本机 SQLite 作为唯一权威事实来源。CLI、Tauri 桌面端和本机 HTTP API
共享同一组 Rust 用例与状态机；搜索、图和向量存储都是可重建的派生层。

本架构面向本地单机运行：Rust 工作区、SQLite、CLI、本机 HTTP API、Tauri 桌面端，
以及暂不作为公开支持能力的实验性 dispatcher 入口。

---

## 1. 总体架构

```text
Tauri Desktop
  -> kanban-server handler/DTO
        \
kanban-cli \
dispatcher  -> kanban-application API / DTO 契约
                     | 由 kanban-sqlite::api / SqliteApplication 实现
                     | 使用 kanban-core 的纯状态机辅助函数
                     v
                权威 SQLite WAL
                     |
                     | task_events / index_outbox / 脏代际标记
                     v
                可重建派生存储
                (Tantivy / Oxigraph / LanceDB)
```

当前实现已经把一组已选择的面向适配器的 DTO/port 垂直切片抽到 `kanban-application`；它不是完整的 application service，也不拥有 SQLite 事务。CLI、HTTP
server、desktop 和 dispatcher 通过 `kanban_sqlite::api` 或 `SqliteApplication` 进入同一组
SQLite 支持的用例；`kanban-sqlite::service` 仍是事务、状态机保护、权威
写入、events、runs、outbox 和来源记录的实现 owner。`kanban-core` 承载
`TaskStatus`、ID/error/clock 和纯状态机辅助函数，不拥有持久化记录。

`kanban-sqlite` crate 根模块不再重新导出数据库/init/service 符号。生产适配器必须导入
`kanban_sqlite::api`、`kanban_sqlite::application::SqliteApplication`，或显式的
`kanban_sqlite::db` / `kanban_sqlite::init` 基础设施模块。测试原始检查入口集中到
`kanban-test-support`，crate 内部测试可使用显式 `db` / `init` 模块。

可把系统按八个运行平面理解：

| 平面 | 当前内容 | 写权限边界 |
|---|---|---|
| 交互/适配器 | `kanban-cli`、`kanban-server`、desktop、dispatcher 入口 | 转换输入/输出和 locale/message 渲染，不直接写 SQLite 事实 |
| Wire 契约 | `kanban-contract` 的候选 Serde DTO、精确公开面目录、操作清单与 schema 根注册表 | 只定义公开机器契约候选；只有 `Adopted` 条目表示运行时采用，不拥有 service 保护、SQLite 记录或运行时验证 |
| Schema 工具 | `kanban-schema-tool` 的 `kanban-schema` 二进制程序、metaschema/fixture 校验、manifest/hash 和漂移门禁 | 独立叶子工具，不进入产品运行时依赖图，也不能充当采用 witness |
| 应用契约 | `kanban-application` 已选择的用例 DTO/port API，SQLite 实现位于 `kanban-sqlite` | 适配器逐步依赖稳定 API/DTO；该 crate 不是完整 application service |
| 领域/状态机 | `kanban-core` 的 status、保护和重新计算辅助函数 | 纯逻辑，不访问 SQLite/HTTP/CLI |
| 权威 SQLite 事实 | tasks/status、dependencies、labels、semantics、proposals、ontology 账本 | 只能由 service 路径写入 |
| 传播/控制平面 | `task_events`、`index_outbox`、dirty/generation/status 标记 | 记录同步水位和恢复入口，不替代事实 |
| 可重建派生存储 | Tantivy、Oxigraph、LanceDB `kb_chunks` / `kb_label_atoms` | 可删除重建，无权威写入路径 |

---

## 2. Crate 结构

当前主要仓库结构（省略测试、脚本、生成文件和部分支持文件）：

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

  kanban-application/
    src/
      lib.rs

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

Desktop 包由 Tauri 构建，内置 `kanban-vector-lancedb` 与
`kanban-graph-oxigraph` 辅助进程。Desktop 启动内嵌 server 时把已有的
内置辅助进程路径注入 `kanban-server::AppState`；CLI `.deb` 仍由
`scripts/package-cli-linux.sh` 独立安装 `/usr/bin/kanban` 与 `/usr/lib/kanban/` 下的辅助程序。

### 2.1 `kanban-core`

职责：

- 定义基础领域类型：`Board`、`BoardColumn`、`TaskStatus`。
- 提供类型化 ID、clock 和统一错误类型。
- 实现纯状态机、ready 重新计算与状态转换保护辅助函数。
- 提供轻量 locale 与消息渲染辅助函数；只渲染用户可见文案，不翻译权威 status、ID、JSON key 或数据库值。
- 不依赖 SQLite、HTTP、CLI、前端。
- 当前不定义完整命令输入/输出，也不定义 application service 接口。
  这些用例编排和持久化记录主要在 `kanban-sqlite::service`。

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

- 为逐步迁入的公开 API、CLI、JSONL、SSE、结构化元数据、配置和辅助进程
  wire DTO 提供唯一候选归属；适配器迁移时负责 application/SQLite 记录到 wire DTO
  的显式映射。
- 默认 feature 只包含轻量 Serde 类型；唯一增量 `schema` feature 启用 `schemars`
  并公开 schema 根注册表。该 crate 不拥有二进制程序、`jsonschema`、SHA-256 或漂移工具。
- 用精确公开面目录枚举实际 Axum method/path、Clap 叶子命令和 JSONL
  discriminator；对应测试从真实声明生成 key，新增公开入口而未登记时自动失败。
- 对 API/SSE contract 显式记录 `operation_key`、`Path|Query|Headers|Body|Success|Error|Sse`
  location 和参数 cardinality；非 HTTP surface 显式记录 `NoTransport`。`Success` 只表达 2xx
  success，`Error` 只表达 `SharedComponent` 非 2xx response，且不新增第七 endpoint obligation。
  传输验证器负责 direction/location、operation/surface、granularity、path placeholder 和
  重复/缺失参数的失败关闭拓扑校验，不承担 HTTP status 或业务语义。
- 用操作清单明确每个公开面的方向、严格性、fixture、schema ID
  或 exclusion，并区分 `Planned`、`Generated`、`Adopted`、`Excluded`。`Generated`
  只表示离线 schema/fixture 就绪；`Adopted` 还必须绑定 direction-correct evidence：
  request/input producer 由 contract DTO 程序化序列化并精确匹配已提交 fixture，consumer
  从 fixture 经真实运行时 handler；response/output producer 来自真实适配器。双方包含
  operation/contract/surface/direction 和精确 Cargo test locator，且不共用同一个高层 exercise
  helper。端点整体迁移状态与六项义务分开收敛：
  `Generated` 端点可以先把已迁移的 body 声明为 adopted 精确 contract，但其它
  义务仍为 `Todo` 时不能提升为 `Adopted`；审计要求该 contract 与运行时 operation
  唯一、双向且精确绑定。witness gate 以 canonical manifest 和 Cargo package ID 锁定
  当前 workspace `kanban-contract`，要求无条件、非可选的普通依赖声明与默认
  resolve edge，并以 `--all-features --target all --edges normal,features --locked` 扫描采用者的运行时
  泄漏，随后真实执行双方测试；registry/git/其它 path 的同名 package 不构成采用。最终收口门禁只允许
  `Adopted` 或 `Excluded`。
- 生成显式 Draft 2020-12、离线
  `urn:kanban-tool:schema:<surface>:<semantic-name>:v1` root；schema 字节从候选 wire 类型
  确定生成。fixtures 是手写正负样例，用于验证 schema 与当前候选 wire shape；
  它们本身不构成运行时采用证据。

该 crate 不依赖 `kanban-sqlite`、`kanban-server`、`kanban-cli`、desktop、
dispatcher 或重型辅助后端。JSON Schema 只验证 wire 结构/值域，
不能替代状态机、CAS、依赖、重新计算、事务或评论语义保护。
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
  crates.io 原始来源。
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
  以及 `release` 从 affected self-test、显式 Tantivy/Oxigraph Projection cohort 到
  diff-check 的 14 步精确顺序。leaf 仅由独立
  schema gates 执行格式、check、tests、clippy、生成和校验；witness gate 显式拒绝该
  tooling owner 冒充 runtime adopter。

### 2.2 `kanban-sqlite`

职责：

- SQLite 连接初始化。
- migration。
- 事务封装。
- application/service 编排与 repository 实现。
- 复杂查询。
- CAS claim。
- 追加 event。
- task/comment/dependency/run/label/ontology 用例。
- label proposal 验证/持久化，以及 `LabelProposalProvider` trait 边界。

公开 API 边界：

- `kanban_sqlite::service` 是实现 owner，负责事务、状态机保护、
  权威写入、events、runs 和来源记录。
- `kanban_sqlite::api` 根模块是面向适配器/产品用例的精选 facade，用于 CLI、server、desktop
  和 dispatcher contract path 复用已允许的 use case、query、record 和 provenance 类型。它不拥有新的
  编排语义，不是 `service::*` 的宽泛重新导出，也不导出数据库连接辅助函数、init
  辅助函数、运行时生命周期保护、provider/vector-store seam，或未列入允许清单的 service-only
  实现辅助函数。
- `kanban_sqlite::api::provider` 承载 adapter/test 需要显式注入 provider 或 vector store 的 seam，
  包括 `LabelProposalProvider`、manual/disabled proposal provider、`*_with` label suggestion/proposal
  helpers、label atom/vector-store status/query/rebuild/sync helpers，以及 trusted-suggestion validation DTO。
  这些符号不从 `api` root 暴露。
- `kanban_sqlite::api::lifecycle` 承载进程运行时/替换生命周期管线：
  `DatabaseRuntimeGuard`、`DatabaseReplaceGuard`、`begin_database_runtime` 和
  `begin_database_replace`。这些保护是二进制程序/运行时 owner 的基础设施，不是普通产品用例。
- `kanban_sqlite::db` 和 `kanban_sqlite::init` 仍是显式基础设施模块；`connect_file`、
  `init_database` 不从 `api` root 暴露。
- crate 根模块不再提供 `kanban_sqlite::*` 旧版重新导出；旧根路径是破坏性变更，
  并由 `tests/ui/root_legacy_reexport_removed.rs` 负向编译契约锁定。`api` 根模块、
  `api::provider`、`api::lifecycle` 和显式 `db` / `init` 边界由 `public_api` trybuild contract 锁定。
- `kanban_sqlite::application::SqliteApplication` 实现 `kanban-application` 的 backend port，
  用于需要以 application API 组合 selected use-case slice 的 adapter/benchmark 路径。
- `kanban-application` DTO/trait 演进遵循 additive-first 策略：优先新增可选字段、option
  struct 或 extension trait；破坏性 DTO/trait 变更必须和 adapter 更新、public API compile
  contract 同步提交。

关键要求：

- 所有状态变化必须在事务内完成。
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
- 输出人类可读表格或 JSON。
- 返回稳定退出码。
- `--locale` / `KANBAN_LOCALE` 只选择人类可读输出语言；脚本契约仍以 `--json` 为准。

CLI 可以直接打开 SQLite 数据库调用 service，不需要 server 常驻。

### 2.4 `kanban-server`

职责：

- localhost HTTP API。
- 只提供本机 API，不托管浏览器版看板。
- SSE 事件流。
- 请求 DTO 转换为命令输入。
- 错误格式统一。
- 根据 `Accept-Language` 渲染 `error.message`；`error.code` 和 JSON shape 保持稳定。
- 通过 `AppState` 接收可选 graph/vector helper binary path；缺失时 graph/vector
  status endpoint 返回 degraded diagnostics，而不是把 helper-heavy crates 编进 server。

独立运行 `kanban serve` 时默认只监听：

```text
127.0.0.1:8721
```

Tauri Desktop 的内嵌服务器绑定 `127.0.0.1:0`，由操作系统选择可用端口。

### 2.5 `kanban-vector`

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

### 2.6 标签提案 provider 边界

语义 label proposal 分成两层：

```text
上层 provider
  - 人工/离线候选项输入
  - 未来的本地 LLM / AI 运行时集成
  - credential、模型配置与 HTTP/client 关注点
        ↓ LabelProposalProvider
kanban-sqlite
  - task/建议上下文查询
  - 确定性验证
  - 残差 top1+margin 门禁
  - proposal 持久化与 accept/reject 生命周期
```

`kanban-sqlite` 只接受 `LabelProposalProvider` trait object，不拥有真实 LLM provider。
默认 `DisabledLabelProposalProvider` 只产生降级尝试；`ManualLabelProposalProvider`
用于 CLI/API 显式传入的本地/离线候选项。未来真实 provider 的候选位置是
`kanban-server`、本地运行时或独立 `kanban-ai` / `kanban-llm` crate，并且必须保持
SQLite service 不知道 credential、HTTP transport、prompt 模板或外部 SDK。

### 2.7 标签本体角色

Label 系统有六个角色，但不是六个严格独立的存储层：

1. `labels` / `task_labels`：权威 label identity 与 task 当前绑定事实；基础 identity
   CRUD 是词汇表注册表，不写 ontology 账本。
2. `label_semantics`：权威 ontology semantics；`label_atoms` 是从 semantics 与 label
   name 展开的 SQLite 物化投影。
3. `kb_label_atoms` / `label_atom_index_boards`：可重建 label atom 派生检索。
4. `label suggest`：基于当前 task、atom 和向量证据的计算/诊断，不是持久事实。
5. `label_semantic_proposals`：候选新 label 的生命周期记录，accept 前不改变当前 task-label 事实。
6. `label_ontology_*` 账本：observation、signal、action、validation 来源记录。

Proposal 与账本是 SQLite 权威记录，因为它们需要审计和可查询历史；但它们不替代
`task_labels` 的当前绑定事实，也不替代 `label_semantics` 的 ontology semantics。
账本覆盖 semantics/atom 变更来源；`labels` identity create/delete 位于
账本之外。
正式文档使用“权威事实”“派生检索”“proposal 工作流”和
“ontology 来源记录”这些边界词；不要把未定义的内部简称写成架构术语。

### 2.8 标签本体图边界

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

### 3.1 创建任务

```text
CLI/Web
  -> CreateTask 命令
  -> 验证输入
  -> 计算初始状态
  -> 插入 tasks
  -> 插入 task_events(kind='task.created')
  -> 返回 task 快照
```

初始状态计算：

```text
如果规格不完整                  -> triage
否则如果 scheduled_at > 当前时间 -> scheduled
否则如果存在依赖                -> todo
否则                            -> ready 候选
新任务 execution plan=unplanned  -> 将 ready 候选降为 todo
```

添加第一个 step，或明确标记 `not_required` 后，服务才会重新计算是否进入 `ready`。

### 3.2 领取任务

```text
CLI/Web/Dispatcher
  -> ClaimTask 命令
  -> BEGIN IMMEDIATE
  -> 验证 task.status == ready
  -> 验证不存在未完成的父任务依赖
  -> 通过 CAS 把 tasks 更新为 running
  -> 插入 task_runs(status='running')
  -> 更新 tasks.current_run_id
  -> 插入 task_events(kind='task.claimed')
  -> COMMIT
```

### 3.3 完成任务

```text
Worker/CLI/Web
  -> CompleteTask 命令
  -> BEGIN IMMEDIATE
  -> 验证处于 running/review
  -> 如果处于 running：除非 force=true，否则验证 claim token
  -> 更新 task_runs
  -> 把 tasks 更新为 done 或 review
  -> 清除领取字段
  -> 插入 task_events(kind='task.completed')
  -> 子任务保持 todo；派生依赖状态反映它们是否仍被阻塞
  -> COMMIT
```

### 3.4 重新打开任务

```text
CLI/Web
  -> ReopenTask 命令
  -> BEGIN IMMEDIATE
  -> 验证 task.status == done
  -> 验证 reason 非空
  -> 根据规格、计划时间、依赖和执行计划重新计算目标状态
  -> 清除 completed_at，同时保留 result_summary/result_json
  -> 插入 task_events(kind='task.reopened')
  -> 重新计算直接的活跃子任务；running/blocked/review/done/archived 子任务保持不变
  -> COMMIT
```

### 3.5 Web 实时更新

```text
状态变更命令
  -> 插入 ID 单调递增的 task_events
  -> server SSE 循环轮询或订阅事件
  -> 浏览器接收事件
  -> 浏览器获取已变更 task 或应用补丁
```

---

## 4. 进程模型

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
- SSE 事件流。

适用：为 Tauri Desktop 或本机脚本提供 API；该命令本身不提供浏览器看板。

---

## 5. 配置

默认配置文件：

```text
~/.config/kanban/config.toml
```

可解析的项目或全局配置示例：

```toml
board = "default"
db = "/path/to/kb.db"

[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "qwen3-embedding:0.6b"
dimensions = 1024
```

当前配置只接受顶层 `board`、`db` 和可选的 `[vector]`；未知字段会被拒绝。默认操作者
来自 `USER`、`USERNAME` 或回退值 `local`，不是配置字段。

CLI 还支持项目级当前 board 配置：

```text
<project>/.kb/config.toml
```

当前版本只写入一个顶层字段：

```toml
board = "agent-work"
```

当前 board 的解析顺序是 `--board`、`KB_BOARD`、向上查找最近的 `.kb/config.toml`，最后回退到 `default`。项目配置只选择同一个全局 SQLite 数据库内的 board，不表示每个项目使用一个数据库。

---

## 6. 并发

### 6.1 SQLite 写入策略

- 使用 WAL。
- 使用短事务。
- 对 claim/reclaim/complete 使用 `BEGIN IMMEDIATE`。
- 使用乐观锁：`lock_version`。
- 并发 claim 同一 task 时，只有一个 `UPDATE ... WHERE status='ready' AND claim_token IS NULL` 成功。

### 6.2 不做的事情

- 不引入分布式锁。
- 不用网络文件系统共享数据库。
- 不允许多个机器同时写同一 SQLite 文件。

### 6.3 同机多进程

允许：

- 多个 CLI 命令。
- 一个 server。
- 一个 dispatcher。

SQLite WAL 和 busy timeout 负责排队。业务层仍需保证事务短小。

---

## 7. 错误模型

公开错误 wire 词汇由 `kanban-contract::ApiErrorCode` 作为唯一闭合集合 owner。
HTTP status 映射与 operation-level transport 说明仅在 `docs/API_SPEC.md` 的
“HTTP 状态映射”表中维护；架构文档不复制 code 表，避免与 server 适配器的实际
`KanbanError -> ApiErrorCode` 映射漂移。

`error.message` 仍是面向人的 locale 相关文案；状态机、service 保护、CAS、
事务与 SQLite 错误权威不转移给 wire contract。

---

## 8. 可观测性

本地工具仍需要基本可观测性：

- `task_events` 是第一审计来源。
- server 输出结构化日志。
- dispatcher 对每次 run 写入 `task_runs`。
- worker stdout/stderr 可写入本地日志文件，数据库只存路径和摘要。
- `kanban doctor` 检查数据库、WAL、schema、完整性、孤儿 run、基础关系表
  board 一致性、label ontology 账本一致性，并报告 Knowledge Substrate 的
  `index_outbox` 积压、派生存储 dirty/error 状态和各存储的 last_error。派生层
  异常不改变 SQLite task 事实；操作者通过同步/重建恢复 Tantivy/Oxigraph/LanceDB。

统一 Projection v2 maintenance runtime 在数据库级使用 singleton lease，但 lease
同时绑定当前进程实际编译的 store capability 集和运行制品 build identity。status
不会把“存在活动 owner”解释为“所有 store 都可维护”：当前构建缺少 backend 时报告
`unavailable`，活动 owner 未声明 store capability 时报告 `unverified`，两者都附带
稳定 fallback reason，并使 `doctor --strict-derived` fail closed。continuous runtime
必须声明全部 projection store capability 才能领取 singleton lease；feature-limited
制品在 claim 前拒绝，避免部分能力 owner 长期垄断数据库级维护入口。

一次 `run --once` 或 `rebuild --all` 中，store backend/provider/delivery 的局部失败
以闭合的结构化 store result 返回并记录到对应 projection state；runtime 仍按稳定顺序
尝试其余已编译 store。数据库访问、singleton owner、lease/fence 或 shutdown 失败属于
全局错误，会终止本次 pass。错误作用域由运行时的显式结果类型和调用边界决定，不解析
错误文案；任何派生失败都不回滚已经提交的 SQLite 权威 mutation。

### 8.1 Board scope 与 schema/service/doctor 分工

Board 是本地 project/board，不是租户。正常写路径的隔离边界在 service 层：
CLI、HTTP、desktop 和 dispatcher 通过 `kanban-sqlite::service` resolve board/task/label/run，
再在同一事务中写入权威 SQLite 事实。派生存储只消费 SQLite/outbox
投影，不拥有权威写入权限。

关键关系表已经使用包含 `board_id` 的 composite FK 或 trigger。`task_labels`、
`task_dependencies`、`task_runs`、`task_comments`、`task_attachments` 在 SQLite 层直接
保证 row board 与 referenced task/label/run board 一致；`task_events` 保留 nullable
task/run refs 与 `ON DELETE SET NULL` 历史语义，通过 INSERT/UPDATE triggers 校验非空
refs 的 board scope。Ontology action-signal 使用 board-scoped composite FK；nullable
ontology refs、parent/supersede links、proposal resolved label 等用 triggers 保护；historical
atom refs 保持 soft ref。

- service 保护是普通 CLI/API/Desktop/dispatcher 写入的主防线；
- `kanban doctor` 是现有数据库的只读巡检层，发现跨 board 关系记录或
  `PRAGMA foreign_key_check` 违规时让 `ok=false`；
- JSONL import 在替换事务提交前运行同类一致性/外键门禁，失败会回滚整个
  导入。

---

## 9. 安全边界

因为不做多用户/远程：

- 默认只绑定 `127.0.0.1`。
- 不提供登录系统。
- 不提供远程访问配置。
- 不在 API 中执行任意 shell，worker profile 只能来自本地配置。
- 附件路径必须限制在数据目录内，防止路径穿越。


### 传输描述符边界

`kanban-contract` 是 localhost 传输的 method/path 权威：其默认 feature 无运行时 HTTP 依赖，仍可被叶子 schema 工具离线使用。`kanban-server::router::registered_api_routes()` 仅提供显式 `adapter_id` 和真实 handler；path/method 从 contract 描述符读取。这样 CLI/JSONL 清单与 API/SSE 传输标识分层，server 不能自行复制传输字符串。

每个 API/SSE 语义 contract 还必须显式声明 HTTP location；其它公开面必须声明
`NoTransport`。任意 `Adopted` contract 与端点精确引用都必须保持
`granularity=Exact`。唯一 method/path、精确 `operation_key` 和单一 location 共同保证一个
`ExactSurface` contract 不可能合法绑定两个端点义务，因此不保留不可达的全局
第二绑定保护。`SharedComponent` 允许被多个端点显式链接，或由同一公开面的真实
采用 witness 证明不是孤儿；这两个条件取其一。共享 contract 永远不计入端点精确覆盖，
也不单独决定端点迁移状态。

两个 task-read 端点的 path、query、headers 与成功响应都由
端点专属精确 contract 覆盖，当前迁移状态为 `Adopted`。精确 wire 形状、
query 预算、producer/consumer 证据与实时覆盖状态以 `docs/API_SPEC.md`、
`docs/SCHEMA_CONTRACTS.md` 和生成的 schema artifact 为准；架构层只规定传输权威
和共享 service 路径，不复制阶段性冻结统计。

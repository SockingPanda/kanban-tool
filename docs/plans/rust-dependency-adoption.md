# Rust dependency adoption

状态：首轮低风险采用进行中。

## 采用原则

- 新库必须服务于明确问题，不能反向改写本项目的 SQLite-only、本地单机和 service-path 架构。
- canonical write 仍由 `kanban-sqlite::service` transaction/state guard 负责；adapter 和 binary 入口继续通过 `kanban_sqlite::api` / `SqliteApplication`。
- CLI/API 的 JSON envelope、`error.code`、退出码和 stdout/stderr 边界不能因为 human diagnostics、schema tooling 或文件 IO helper 漂移。
- helper-heavy 图、向量、AI 或 schema 依赖不能进入主 SQLite service canonical write path。

## 本轮采用

### `tokio-util`

用途：把 server search-sync background task 和 desktop embedded API shutdown 收敛到 `tokio_util::sync::CancellationToken`。

边界：`CancellationToken` 只存在于 process/runtime lifecycle 层，不进入 SQLite transaction，不改变 `begin_database_runtime` guard 和 import/replace guard 语义。

验证重点：`kanban-server` check/test、`kanban-cli` serve SIGINT 测试、desktop check，以及最终 `just rust-fast`。

### `fs-err`

用途：替换少量 path-heavy runtime IO：`kanban-local` config 读写和 CLI maintenance import/replace 文件操作。收益是 IO error 自动携带操作和路径上下文。

边界：只替换 `std::fs` 的 drop-in 调用，不改变结构化错误类型、JSON error envelope、`error.code` 或 exit code。

验证重点：`kanban-local` tests、`kanban-cli` config/maintenance/import_export/exit_codes tests。

### `assert_fs`

用途：作为 dev-dependency 提供文件系统测试夹具。首轮只用于 `kanban-local` config parse-error 测试，避免机械重写大量既有 `tempfile` 测试。

边界：仅测试依赖，不进入 runtime dependency graph。

验证重点：`kanban-local` tests 和 workspace fast gate。

### `schemars` / `jsonschema`

用途：`schemars 1.2.1` 从 `kanban-contract` 的候选 Serde wire DTO 生成显式 Draft
2020-12 schema；当前 API error response 与 `GET /health` response 已是 `Adopted`，label
semantics delete response 与 decision comment metadata input 保持 `Generated`，合计 2 个
adopted 与 2 个 generated foundation roots。前两项由 producer/consumer 双方共 4 个真实
witness 证明运行时采用；generated 仍只代表离线 schema/fixture 就绪。`jsonschema 0.47.0`
在开发期离线校验 metaschema、手写正负 fixtures 和 committed artifact drift。

边界：`kanban-contract` 只用 optional `schema` feature 启用 `schemars`；独立的
`kanban-schema-tool` leaf crate 通过 normal dependency 启用该 feature，并独占 `jsonschema`、
SHA-256、binary 和 drift tooling。产品 default/core/full 门禁不包含 leaf tool；除该 tool 外，任何 workspace member 都不得通过
normal/dev/build、alias、optional 或 target-specific direct edge 引用它。tool 自身仅允许 5 条
已批准 normal edge，且完整 manifest/metadata signature 由 isolation gate 锁定。desktop 因 Tauri 既有依赖仍独立包含
`schemars 0.8.x`；依赖隔离 gate 专门禁止本次新增的 `schemars 1.x`、`jsonschema`、
`kanban-contract/schema` 或 `kanban-schema-tool` 泄漏到产品 runtime 图，不把 Tauri 的既有
版本误报为本次回归。schema 仍不替代 service/state-machine guard。

验证重点：`just schema-contract`、manifest/hash determinism、手写 fixture parity、
Axum/Clap/JSONL 精确 surface audit、`schema-audit-closed` 和
`just schema-dependency-isolation`。adopted 条目还必须通过结构化 producer/consumer
witness gate，以 canonical manifest/package ID 证明 unconditional non-optional normal dependency
及 default resolve edge 指向当前 workspace contract，并通过
`--all-features --target all --edges normal,features --locked` runtime graph 负向扫描和精确 Cargo test
locator 的真实执行；registry、git 或其它 path 的同名 package 不算采用，平台或 feature-specific witness
当前不支持。

## 暂缓项

### `tracing-appender`

暂缓原因：要形成有价值的 rolling file logging，需要先决定 CLI flag、默认日志路径、desktop embedded runtime log lifecycle 和 stderr/file 双写策略。当前若直接加入会扩大用户可见行为和测试面。

后续建议：单独设计 `kanban serve --log-file` 或配置项，确认不影响 stdout/stderr purity 后再采用。

### `proptest-state-machine`

暂缓原因：适合 `kanban-core` 状态机动作序列模型，但应单独建模 ready/running/blocked/review/archive 迁移和 shrink 策略，避免在 dependency adoption PR 中混入大测试设计。

后续建议：单独测试 lane，先覆盖纯 `kanban-core` 状态机，不触碰 SQLite service。

### `petgraph`

暂缓原因：当前没有足够窄且高收益的 derived/read-only parity 切入点。直接加入容易把 graph library 误带入 canonical write path。

后续建议：只在 `kanban-graph` derived/read-only 算法中试点，并用 parity tests 证明不影响 SQLite `task_dependencies` truth。

### `rusqlite` modernization

暂缓原因：`rusqlite 0.32 -> 0.40` 属于核心 persistence 现代化，可能影响 bundled SQLite、migrations、busy_timeout、transaction 和 error typing，不能与低风险库采用混合。

后续建议：独立分支、独立 kanban task、完整云端 backend gate 和 reviewer gate。

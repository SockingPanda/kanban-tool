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

## 暂缓项

### `tracing-appender`

暂缓原因：要形成有价值的 rolling file logging，需要先决定 CLI flag、默认日志路径、desktop embedded runtime log lifecycle 和 stderr/file 双写策略。当前若直接加入会扩大用户可见行为和测试面。

后续建议：单独设计 `kanban serve --log-file` 或配置项，确认不影响 stdout/stderr purity 后再采用。

### `proptest-state-machine`

暂缓原因：适合 `kanban-core` 状态机动作序列模型，但应单独建模 ready/running/blocked/review/archive 迁移和 shrink 策略，避免在 dependency adoption PR 中混入大测试设计。

后续建议：单独测试 lane，先覆盖纯 `kanban-core` 状态机，不触碰 SQLite service。

### `schemars` / `jsonschema`

暂缓原因：schema 生成和 runtime validation 会影响 API/metadata contract 解释权。需要先选择一个窄 DTO 或 metadata surface，并定义 snapshot ownership。

后续建议：先在 non-canonical DTO 做 schema snapshot pilot；generated schema 只作为 contract aid，不替代 `docs/API_SPEC.md`。

### `petgraph`

暂缓原因：当前没有足够窄且高收益的 derived/read-only parity 切入点。直接加入容易把 graph library 误带入 canonical write path。

后续建议：只在 `kanban-graph` derived/read-only 算法中试点，并用 parity tests 证明不影响 SQLite `task_dependencies` truth。

### `rusqlite` modernization

暂缓原因：`rusqlite 0.32 -> 0.40` 属于核心 persistence 现代化，可能影响 bundled SQLite、migrations、busy_timeout、transaction 和 error typing，不能与低风险库采用混合。

后续建议：独立分支、独立 kanban task、完整云端 backend gate 和 reviewer gate。

# kanban-tool 项目契约

本文件只保留跨任务稳定的产品边界、仓库地图、文档路由和验证边界。当前行为以 owner crate、
代码、生成 artifact 和测试为准；不要把高频库存复制到这里。

## 1. 产品边界

- kanban-tool 是本地优先、单机、单用户的看板与 durable work queue。
- `kanban serve` 是唯一 host 和 canonical Turso owner；其他入口通过 typed localhost HTTP/SSE 工作。
- CLI、MCP、Desktop 和 dispatcher 共享 application service、状态机、事务和错误语义。
- 不引入 SaaS、多租户、远程访问、RBAC、云同步或第二条 canonical mutation path。

## 2. 稳定不变量

- `tasks.status` 是事实；看板列只是展示映射，不得形成第二套状态机。
- 所有 mutation 必须经过共享 application/service path；adapter 不得直接写 canonical 状态。
- `ready -> running` 只能通过原子 claim；claim、run 和对应 event 必须保持一致。
- dispatcher 只 claim `ready`，不得自动 claim `review`。
- 保留 board isolation、外键、唯一约束、idempotency、依赖环检查和事务原子性。
- canonical 数据是业务事实；projection、缓存和派生索引都只能重建，不能反向写事实。

## 3. 工作区地图

- `kanban-core`：领域 ID、枚举、状态机、纯校验和领域错误；不依赖内部 crate、HTTP 或 Turso。
- `kanban-service`：application service、Turso schema/migration、repository、事务、projection、
  provider 和只读 importer；是唯一直接依赖 `turso` 的 canonical persistence owner。
- `kanban-server`：唯一 host、Axum routes、数据库装配和 dispatcher。
- `kanban-protocol`：当前 active wire DTO、error、schema 和 surface catalog；不承载数据库 row 或 store 规则。
- `kanban-client`：typed localhost HTTP client；CLI、MCP 和 Desktop 不直连数据库。
- `kanban-cli`、`kanban-mcp`、`apps/desktop/src-tauri`：薄入口或 shell；`xtask` 仅作离线检查工具。
- 依赖 ownership 和模块边界见 [`docs/architecture.md`](docs/architecture.md) 及 `$style`。

## 4. 任务边界与停止

- 开始修改前明确用户可观察目标、最小调用链、验收方式、非目标和停止条件。
- 只推进当前验收所需的纵向切片；不顺手重构、补邻接 bug 或扩大产品范围。
- 未经当前验收需要，不升级 dependency、重写 lockfile、增加 backend/兼容层或建立新的 abstraction。
- 发现范围外问题只记录；只有数据完整性、安全边界、状态机或主路径不可用时才扩大范围。
- 验收证据和最终 diff 检查完成后停止，不自动开始下一阶段。

## 5. 技能路由

- `$style`：Rust、Cargo、模块组织、依赖边界、错误和测试位置。
- `$prose`：用户可见简体中文文案、README、指南和 ADR 表达。
- `$docs`：事实源、owner placement、文档同步和历史退出边界。
- `$check`：根据当前 `justfile` 选择并报告最小充分验证。
- `$commit`：仅在用户明确授权后创建本地 Conventional Commit。

## 6. 文档地图

- 产品首页、最小使用路径和指南索引：[`README.md`](README.md)。
- 跨 crate 拓扑、依赖方向和 canonical/derived 原则：[`docs/architecture.md`](docs/architecture.md)。
- 状态、readiness、claim、lease 和 dispatcher：[`crates/kanban-core/docs/state_machine.md`](crates/kanban-core/docs/state_machine.md)。
- persistence、migration、maintenance：[`crates/kanban-service/docs/`](crates/kanban-service/docs/)。
- wire DTO、schema 和 catalog：[`crates/kanban-protocol/docs/schema.md`](crates/kanban-protocol/docs/schema.md)。
- CLI、client、server、MCP、Desktop 使用指南：各自 crate/app 的 `README.md`；Desktop layout 见
  [`apps/desktop/docs/layout.md`](apps/desktop/docs/layout.md)。
- 长期跨模块取舍：[`docs/adr/README.md`](docs/adr/README.md)，一项决定一个 ADR。
- 精确 CLI syntax 由 Clap help，精确 HTTP/MCP surface 由 catalog，精确 schema 由 migration/生成 artifact
  持有；测试名称、gate 状态、migration 进度和 baseline 留在任务、CI 或 Git history。

## 7. 验证边界

- `justfile` 是命令入口的唯一事实源；不凭记忆发明 recipe 或参数。
- 文件修改至少运行 `just diff-check`；文档结构改动补 `just docs-check`。
- protocol/schema contract 改动才运行 `just schema-check`；Rust、Web、Desktop 和 package gate 只按真实影响升级。
- 会写 Cargo target 的 recipe 必须经仓库的 build lock；不自设 target/cache、不 `cargo clean`、不并行写 target。
- 未运行的 gate 不得表述为通过、migration closed 或 release ready；失败先判断是否由当前 diff 引起。

## 8. 语言与 Git 边界

- 项目自有文档、skill、代码注释与 rustdoc 以简体中文为主；命令、路径、crate、API/JSON 字段、枚举和库名保留 literal。
- 中文和 Unicode 可用于 prose；代码、shell、TOML、JSON、schema 等机器语法遵循 parser，不做全局 ASCII 化。
- 保护既有 dirty work；diff 聚焦当前任务，不覆盖无关改动，不使用破坏性 reset/checkout。
- 不 push、开 PR、merge、rebase 或发布；创建本地 commit 必须得到当前用户明确授权。

## 9. 维护

- 根文件只做入口和路由；领域行为跟随 owner crate/app，机器 inventory 跟随代码或生成 artifact。
- 修改文档源后直接运行相应 owner/docs gate；不生成聚合快照，不手工维护派生规格。
- 任何长期架构取舍落在对应 ADR；实现进度、review finding 和一次性 workaround 留在任务或 runbook。

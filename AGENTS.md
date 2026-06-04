# kanban-tool 协作规则

本项目是本地优先 Kanban / durable work queue：Rust core + SQLite-only + CLI + localhost Web + 可选 dispatcher。

## 必读文档

开始任何实现前先读：

- `README.md`
- `docs/SPEC.md`
- `docs/ARCHITECTURE.md`
- `docs/STATE_MACHINE.md`
- `docs/DATA_MODEL.md`
- `docs/CLI_SPEC.md`
- `docs/API_SPEC.md`
- `docs/DISPATCHER_SPEC.md`
- `docs/IMPLEMENTATION_PLAN.md`
- `docs/ADR.md`
- `migrations/001_initial.sql`

## 架构边界

- SQLite 是唯一数据库；不要引入 Postgres/MySQL/MongoDB 后端。
- 单机本地语义；不要引入多用户、RBAC、组织、团队、邀请或 SaaS 假设。
- `tasks.status` 是 canonical truth；`board_columns` 只是 UI 展示映射。
- Web、CLI、dispatcher 必须走同一套 core command service，不允许绕过状态机直接写 status。
- `ready -> running` 必须是原子 claim transaction：CAS update + run + event。
- `blocked -> ready` 必须重新计算 spec、schedule、dependency，不允许盲设。
- review 不自动触发执行；dispatcher 不 claim `review`。

## 工程流程

- 当前目录如果不是 git 仓库，先初始化 git；每个方向使用独立分支。
- 默认分支工作流：clean main → feature branch → TDD 实现 → verify → squash/merge → 删除临时分支。
- 复杂实现优先按小任务推进，并使用 worker/reviewer gate；父级负责最终验证。
- 生产代码遵循 TDD：先写失败测试，运行看到 RED，再写最小实现，运行 GREEN。
- 每次完成一个阶段必须运行：
  - `cargo fmt --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 提交语义使用 Conventional Commits。

## Rust workspace 约定

推荐 crate：

- `kanban-core`：领域类型、状态机、command service 接口；不依赖 SQLite/HTTP/CLI。
- `kanban-sqlite`：SQLite 连接、migration、repository、transaction、query。
- `kanban-cli`：`kb` CLI。
- 后续：`kanban-server`、`kanban-dispatcher`、`web/`。

## 本地文档经验

- 项目经验优先写入本仓库的 `AGENTS.md` 或后续 `.agents/` 支持文件，不写成一次性全局技能。
- 全局 skill 只承载可复用模式；本项目特定取舍写在本仓库。

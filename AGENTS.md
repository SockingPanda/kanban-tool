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

## Desktop frontend guidance

- 修改 `apps/desktop` 前先查阅 `docs/plans/desktop-shadcn-dashboard.md`，把 shadcn dashboard 作为方向参考，不要整套复制或做无目标重写。
- Desktop UI 必须保持本地优先、单用户、localhost operator console 语义；不要引入 SaaS、团队协作、RBAC、邀请、云同步或远程 worker 假设。
- 未来 shell/layout 变更优先保持 `AppShell` 作为边界：sidebar/header/global search/actions 属于 shell，Board/List/Event/Run/TaskDetail 等功能视图不要重复实现外层布局。
- Board/List/Event 等数据视图必须保留状态机语义：列只是展示，任务状态变化通过 API/core command service，不直接写 `tasks.status`。

## 工程流程

- 当前目录如果不是 git 仓库，先初始化 git；每个方向使用独立分支。
- 默认分支工作流：clean main → feature branch → TDD 实现 → verify → squash/merge → 删除临时分支。
- 复杂实现优先按小任务推进，并使用 worker/reviewer gate；父级负责最终验证。
- milestone/release 级实现必须在合并前通过独立 spec reviewer + quality reviewer；仅 `fmt/test/clippy/smoke` 通过不能宣称版本完成。
- 如果 reviewer 指出 P0/P1 规格或质量问题，必须在同一方向分支上修正并重新 review，直到 PASS/APPROVED 后再由父级合并。
- 生产代码遵循 TDD：先写失败测试，运行看到 RED，再写最小实现，运行 GREEN。
- 提交语义使用 Conventional Commits。

## 全局 skill 同步

- 任何改动如果改变了用户可见的 kanban CLI、API、data model、workflow、status、task、dependency、comment、JSON、help 或 documentation 使用行为，implementer 必须检查全局 Codex skill `kanban-tool` 是否需要同步。
- 如果该行为影响 skill 使用说明，必须同步更新 `~/.codex/skills/kanban-tool/SKILL.md`，以及必要的相关 agent 或 `openai.yaml` 配置。
- 如果检查后不需要同步，final report 或 task record 必须明确记录 `kanban-tool skill checked: no change`。
- 全局 skill 只能描述已经实现、并且能由 CLI help 或实际命令/API 输出确认的行为；不要把 roadmap、计划中功能或未实现规格写入 skill。

## 验证策略

普通小改动默认跑受影响范围验证，不要把每个阶段都升级成全量 workspace gate。

### 默认使用 just

- 本仓库本地验证优先使用 `just`；会写 Cargo target 的 recipes 已经内置共享 target root 与构建锁。
- 不要直接运行会写 Cargo target 的 raw `cargo build/test/check/clippy/nextest/run`；需要新验证入口时，先加 `just` recipe。
- 验证、review、acceptance gate 必须在用户指定的工作树 / 目录中执行；不要为验证擅自切换到额外 worktree、临时 clone、新路径或隔离目录。
- 不要额外设置 `KANBAN_CARGO_TARGET_ROOT`、`CARGO_TARGET_DIR` 或其它自定义 target/cache 隔离变量；使用本仓库 `just` recipes 内置的共享 target root 与构建锁。
- 如果确实需要任何额外隔离、新路径、临时 target 或不同工作树，必须先得到用户明确授权。
- 查看可用入口：`just --summary`。

常用验证：

- Rust 快速检查：`just fmt`、`just check`、`just test`、`just clippy`、`just rust-fast`。
- 单 crate：`just check-p kanban-cli`、`just test-p kanban-cli`、`just clippy-p kanban-cli`。
- feature 组合：`just feature-p kanban-cli tantivy-backend`。
- Desktop / Web：`just desktop-check`、`just desktop-package`、`just web-test`、`just web-typecheck`、`just web-build`。
- CLI package 与 smoke：`just cli-package`、`just smoke`。
- 脚本 / 文档小改动：`just target-tools`、`just diff-check`。

以下用于本地发布打包 / explicit release gate：

- `just release`

## Rust workspace 约定

当前主要 crate：

- `kanban-core`：领域类型、状态机、command service 接口；不依赖 SQLite/HTTP/CLI。
- `kanban-sqlite`：SQLite 连接、migration/init、service、transaction、query helper。
- `kanban-cli`：`kanban` CLI。
- `kanban-server`：localhost HTTP API / SSE。
- `kanban-context`、`kanban-entity`、`kanban-graph`、`kanban-indexer`、`kanban-labels`、`kanban-local`、`kanban-search`、`kanban-vector`：本地派生层、索引、graph、context、label 和 vector 支持 crate。
- `apps/desktop`：Tauri desktop operator console。

## 本地文档经验

- 项目经验优先写入本仓库的 `AGENTS.md` 或后续 `.agents/` 支持文件，不写成一次性全局技能。
- 全局 skill 只承载可复用模式；本项目特定取舍写在本仓库。

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

- Rust 单 crate 改动：
  - `cargo fmt --check`
  - `cargo check -p <crate> --tests`
  - `cargo nextest run -p <crate> --no-fail-fast <filter>` 或 `cargo test -p <crate> <filter>`
- Rust 跨 crate 改动：
  - `cargo fmt --check`
  - `cargo check --workspace --exclude kanban-desktop --tests`
  - `cargo nextest run --workspace --exclude kanban-desktop --no-fail-fast`
- CLI / server / sqlite 测试优先使用 package 或 integration target：
  - `cargo nextest run -p kanban-cli --test task --no-fail-fast`
  - `cargo nextest run -p kanban-server -E 'test(tasks::)' --no-fail-fast`
  - `cargo nextest run -p kanban-sqlite -E 'test(transitions::)' --no-fail-fast`
- 文档或配置小改动：
  - `git diff --check`
  - 仅运行与该配置直接相关的静态检查或 dry-run。
- 如果本机安装了 `just`，可以使用等价 `just fmt`、`just check-p <crate>`、`just test-p <crate>`；直接 `cargo` 命令仍是 canonical fallback。
- 并行 worktree / agent 开发时，优先把 Cargo 构建缓存放到内置 NVMe 的 lane 目录，避免在外置系统盘下重复冷构建：
  - `CARGO_TARGET_DIR=$HOME/.cache/kanban-tool/cargo-target/main`
  - `CARGO_TARGET_DIR=$HOME/.cache/kanban-tool/cargo-target/cli`
  - `CARGO_TARGET_DIR=$HOME/.cache/kanban-tool/cargo-target/server`
  - `CARGO_TARGET_DIR=$HOME/.cache/kanban-tool/cargo-target/sqlite`
  - `CARGO_TARGET_DIR=$HOME/.cache/kanban-tool/cargo-target/desktop`
- 多个 agent 并行时不要共用同一个 `CARGO_TARGET_DIR`，按 lane 分配，例如：
  - `CARGO_TARGET_DIR=$HOME/.cache/kanban-tool/cargo-target/cli just check-p kanban-cli`
  - `CARGO_TARGET_DIR=$HOME/.cache/kanban-tool/cargo-target/server just check-p kanban-server`
  - `CARGO_TARGET_DIR=$HOME/.cache/kanban-tool/cargo-target/sqlite just test-p kanban-sqlite`

以下只用于 milestone / release / explicit full gate：

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- feature matrix
- `pnpm --dir apps/desktop test`
- `pnpm --dir apps/desktop typecheck`
- `pnpm --dir apps/desktop build`
- `pnpm --dir apps/desktop tauri build`
- `scripts/smoke-v1-local.sh`

## Rust workspace 约定

推荐 crate：

- `kanban-core`：领域类型、状态机、command service 接口；不依赖 SQLite/HTTP/CLI。
- `kanban-sqlite`：SQLite 连接、migration、repository、transaction、query。
- `kanban-cli`：`kb` CLI。
- 后续：`kanban-server`、`kanban-dispatcher`、`web/`。

## 本地文档经验

- 项目经验优先写入本仓库的 `AGENTS.md` 或后续 `.agents/` 支持文件，不写成一次性全局技能。
- 全局 skill 只承载可复用模式；本项目特定取舍写在本仓库。

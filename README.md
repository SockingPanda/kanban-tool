# Kanban Tool 文档包

本文档包面向一个 **Rust workspace 实现、SQLite-only、本地单机运行、同时提供 Web 与 CLI 能力** 的 Kanban 工具。

本项目不是 Trello 的简单复制品，而是一个更接近 Hermes Kanban 的本地可执行工作队列：

- Kanban UI 负责可视化与人工操作。
- CLI 负责脚本化、本地开发流与 agent/automation 入口。
- SQLite 负责持久化任务、状态、依赖、评论、事件、运行记录。
- Rust workspace 负责状态机、SQLite service/transaction 和一致性约束；当前 application orchestration 主要在 `kanban-sqlite::service`，`kanban-core` 提供纯状态机 helper。
- Dispatcher 是可选本地调度器，用于 claim 显式 `ready` 任务、heartbeat、reclaim 和执行 worker profile；不自动提升 `todo/scheduled`。

`docs/SPEC.md`、`docs/CLI_SPEC.md`、`docs/API_SPEC.md`、`docs/DISPATCHER_SPEC.md` 等分主题文档是当前行为的权威来源；`KANBAN_SPEC_BUNDLE.md` 是这些源文档的同步快照，便于一次性阅读和离线传递。

## 范围约束

明确包含：

- 单机本地运行。
- SQLite 作为唯一数据库。
- Web 端与 CLI。
- 多 board/project，但不是多租户。
- 单用户语义；actor 只是审计字段，不是权限主体。
- 本地 dispatcher/worker 能力。
- append-only events + tasks snapshot。

Board/project 使用同一个 SQLite DB。CLI 通过 `--board`、`KB_BOARD` 或项目级 `.kb/config.toml` 选择 active board；`kanban board use <board>` 只写入项目配置，不创建项目独立 DB。Task 的 `t_...` id 全局唯一，board 内序号通过 `board#seq` 展示和复制。

明确不包含：

- 多用户协作。
- 多租户。
- 远程 worker。
- PostgreSQL/MySQL/MongoDB 后端。
- RBAC、组织、团队、邀请、审计权限模型。
- 云同步或网络文件系统共享 SQLite。

## 文档索引

| 文件 | 内容 |
|---|---|
| [`docs/SPEC.md`](docs/SPEC.md) | 产品与技术总 SPEC |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Rust crate、进程、数据流与配置架构 |
| [`docs/STATE_MACHINE.md`](docs/STATE_MACHINE.md) | 状态定义、转换表、不变量 |
| [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md) | 领域对象、ID、时间、事件、附件、查询模型 |
| [`docs/CLI_SPEC.md`](docs/CLI_SPEC.md) | CLI 命令、参数、输出、退出码 |
| [`docs/API_SPEC.md`](docs/API_SPEC.md) | 本地 Web API 与 SSE 事件流 |
| [`docs/DISPATCHER_SPEC.md`](docs/DISPATCHER_SPEC.md) | 本地 dispatcher / worker 调度规格 |
| [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) | 分阶段实现计划、测试策略、验收标准 |
| [`docs/ADR.md`](docs/ADR.md) | 关键架构决策记录 |
| [`docs/V0.5.md`](docs/V0.5.md) | V0.5 已实现范围、验证记录与暂未包含项 |
| [`docs/V1.md`](docs/V1.md) | V1 Local Web API 已实现范围、`kanban serve`、SSE 行为与 smoke 流程 |
| [`docs/V0.6.md`](docs/V0.6.md) | V0.6 invariant hardening：reclaim、migration、query、doctor |
| [`docs/codex-cloud-environment.md`](docs/codex-cloud-environment.md) | Codex Cloud 前端/后端验证环境 setup、maintenance 与验证命令 |
| [`migrations/001_initial.sql`](migrations/001_initial.sql) | SQLite 初始 schema |

## 当前仓库结构

```text
kanban-tool/
  Cargo.toml
  crates/
    kanban-cli/
    kanban-context/
    kanban-core/
    kanban-derived-io/
    kanban-entity/
    kanban-graph/
    kanban-graph-oxigraph/
    kanban-indexer/
    kanban-labels/
    kanban-local/
    kanban-search/
    kanban-server/
    kanban-sqlite/
    kanban-vector/
    kanban-vector-lancedb/
  apps/
    desktop/
  docs/
  migrations/
```

`kanban-derived-io` contains shared derived-store IO helpers; `kanban-graph-oxigraph` and
`kanban-vector-lancedb` are helper-heavy backend crates packaged as helper binaries.

## 默认二进制名

本文档中使用 `kanban` 作为 CLI binary 名称。

## Linux installation and packaging

Linux 发布分为两个独立 `.deb` 包：

- Desktop package：由 Tauri 构建，产物是 desktop app 的 `.deb`。Desktop 包不负责安装 `kanban` CLI。
- CLI package：独立安装 `kanban` 到 `/usr/bin/kanban`，包名是 `kanban-tool-cli`。

当前 dogfood 发布只关注 Debian package 路径；RPM target 暂不启用。

### Install packaged releases

Debian / Ubuntu：

```bash
sudo apt install ./kanban-tool-cli_1.4.1-1_amd64.deb
kanban --help
```

Desktop packages are installed separately from the CLI package. Install both only if
you want both the graphical desktop app and the `kanban` command.

### Build the CLI package from source

The repository includes a local packaging script for the standalone CLI package.
It always builds the release CLI binary first:

```bash
./scripts/package-cli-linux.sh --format deb
```

The `.deb` is written under the shared locked Cargo target root:

```bash
${KANBAN_CARGO_TARGET_ROOT:-/media/kanban-user/Data/cargo-targets/kanban-tool}/release/bundle/cli/deb/
```

Feature flags can be passed through to the cargo build:

```bash
./scripts/package-cli-linux.sh --format deb --no-default-features
./scripts/package-cli-linux.sh --format deb --all-features
```

The default CLI package installs the main `kanban` binary plus helper binaries
for the Oxigraph graph and LanceDB vector backends under `/usr/lib/kanban/`.
The main CLI/server dependency trees stay free of those helper-only heavy
dependencies.

### Install the CLI directly with cargo

For development machines, you can skip OS packages and install only the CLI from
source:

```bash
cargo install --path crates/kanban-cli --bin kanban
kanban --help
```

Generate static shell completions to stdout with:

```bash
kanban completions bash
kanban completions zsh
kanban completions fish
kanban completions powershell
kanban completions elvish
```

Bash and zsh completion scripts also include dynamic local candidates for task
refs, board slugs, statuses, and comment kinds through the internal
`kanban __complete` helper. Other shells currently receive static command and
option completions only.

## Affected validation

Use the affected validation router to choose a focused local validation set from
the current git diff:

```bash
just affected-plan base="main"
just affected-json base="main"
just affected base="main"
```

The router combines the branch diff from `base...HEAD`, staged changes, working
tree changes, and untracked files. It classifies changed paths into docs,
desktop, core, vector-helper, graph-helper, CLI, server/API,
SQLite/core/state-machine, search/graph/vector/context, and
scripts/packaging/release-sensitive groups, then emits matching `just` commands.
Release-sensitive diffs set `full_gate_recommended=true`; `just release` remains
the authoritative full gate.

Rust check gates are split by helper architecture:

```bash
just check-core     # default `just check`; excludes helper-heavy crates
just check-helpers  # checks kanban-vector-lancedb and kanban-graph-oxigraph
just check-full     # check-core followed by check-helpers
```

Rust validation recipes run target-writing Cargo/Tauri commands through
`scripts/cargo-build-lock.sh`. The wrapper serializes shared target writes and
defaults local build/test parallelism to two jobs/threads to avoid swap-heavy
workspace gates. Override with `KANBAN_CARGO_BUILD_JOBS` /
`KANBAN_TEST_THREADS`, or tool-specific `CARGO_BUILD_JOBS`,
`NEXTEST_TEST_THREADS`, and `RUST_TEST_THREADS`. Set the repo-level values to
`auto` to leave the tool-specific variables unset, which is the preferred
Codex Cloud setting.

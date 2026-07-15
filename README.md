# Kanban Tool 文档包

本文档包面向一个 **Rust workspace 实现、SQLite-only、本地单机运行、同时提供 Web 与 CLI 能力** 的 Kanban 工具。

本项目不是 Trello 的简单复制品，而是一个更接近 Hermes Kanban 的本地可执行工作队列：

- Kanban UI 负责可视化与人工操作。
- CLI 负责脚本化、本地开发流与 agent/automation 入口。
- SQLite 负责持久化任务、状态、依赖、评论、事件、运行记录和 Agent/Product signal ledger。
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
| [`docs/SCHEMA_CONTRACTS.md`](docs/SCHEMA_CONTRACTS.md) | 公开 JSON contract inventory、schema 产物与验证边界 |
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
    kanban-contract/
    kanban-schema-tool/
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
sudo apt install ./kanban-tool-cli_2.1.2-1_amd64.deb
kanban --help
```

Desktop packages are installed separately from the CLI package. Install both only if
you want both the graphical desktop app and the `kanban` command. The Desktop
package bundles the `kanban-vector-lancedb` and `kanban-graph-oxigraph` helper
binaries for its embedded localhost API; it does not install the standalone
`kanban` CLI.

### Build the CLI package from source

The repository includes a local packaging script for the standalone CLI package.
It always builds the release CLI binary first:

```bash
./scripts/package-cli-linux.sh --format deb
```

The `.deb` is written under the exact shared target directory managed by the build-lock wrapper:

```bash
$(scripts/cargo-build-lock.sh --print-target-dir)/release/bundle/cli/deb/
```

Packaging holds that single shared build lock through build, provenance verification,
and `.deb` assembly. It invalidates only workspace-package fingerprints (preserving
registry dependencies), rebuilds from the current tree, and requires dep-info to name
the current source root before any shared binary is copied into the package.

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

## SPEC bundle synchronization

`KANBAN_SPEC_BUNDLE.md` is a generated snapshot for one-file reading and offline
handoff. Its header is the canonical ordered source list; the source documents,
including `README.md`, remain authoritative. Regenerate or check the snapshot with:

```bash
just spec-bundle-generate
just spec-bundle-check
```

`just schema-docs` includes the non-writing bundle check, so a source-document
change cannot be hidden with a `schema-doc-ignore` marker while the bundle stays
stale.

Rust validation gates are split by helper architecture. The core set covers the
main CLI/server/SQLite/local crates plus lightweight helper protocol/shared
crates. The helper set covers helper-heavy backend crates
`kanban-vector-lancedb` and `kanban-graph-oxigraph`.

```bash
just check-core       # default `just check`; excludes helper-heavy crates
just fmt              # format-checks only the daily core Rust set
just test             # default core Rust tests; equivalent to test-core
just clippy           # default core Rust clippy; equivalent to clippy-core
just test-core        # tests the daily core Rust set
just clippy-core      # clippy for the daily core Rust set
just bench-check      # compiles criterion benchmark baselines without running measurements
just rust-fast        # core fmt + check-core + test-core + clippy-core

just check-helpers    # checks kanban-vector-lancedb and kanban-graph-oxigraph
just test-helpers     # tests helper-heavy backend crates
just clippy-helpers   # clippy for helper-heavy backend crates

just check-full       # check-core followed by check-helpers
just fmt-full         # format-checks core + helper crates; excludes desktop and schema leaf
just test-full        # workspace Rust tests excluding desktop + schema leaf
just clippy-full      # clippy-core followed by clippy-helpers
just rust-full        # fmt-full + core/helper check, test, and clippy
```

公开 JSON contract 变更还需要运行：

```bash
just schema-contract  # contract/schema + leaf tool tests/clippy + committed artifact/fixture drift check
```

`just test-full` intentionally excludes `kanban-schema-tool`; the leaf is checked,
tested and linted by `just schema-contract`.
`just fmt`（以及 `just fmt-check` alias）显式选择 core package 集；`just
fmt-full` 显式选择 core + helper package 集。两者都不使用 workspace-wide selection，
因此不会遍历 desktop 或 `kanban-schema-tool`。`just schema-contract` 在 dependency
preflight 之后执行 `just schema-fmt`，后者只选择 `kanban-contract` 与
`kanban-schema-tool`。
`policy/schema-tool-registry-closure.json` 是 schema leaf tool 唯一的 registry
closure approval：普通 gate 解析真实 `Cargo.lock`，从 tool-root metadata graph
计算 reachable registry packages，并要求按 `(name, version, source)` 排序的
`{name, version, source, checksum}` 集合与该 committed snapshot 双向完全一致。
gate 只比较，不会自动写入或 bless snapshot。

Cargo metadata 的 `SourceId` 是 opaque identity；本项目锁定的是 pinned toolchain
下批准的 logical SourceId 字符串，不把其中的 URL 字符串解释为 Cargo 的通用
canonical network URL 保证。上述 policy 检测 committed `Cargo.lock` 相对 approval
的 identity/checksum 漂移；Cargo fetch/build 仍独立依据 registry index `cksum`
验证 crate 内容。物理 index 与 crate 下载可使用 rsproxy 等等价 Cargo
source-replacement mirror，不要求直连 crates.io origin。

Use `just test`, `just clippy`, or `just rust-fast` for daily Rust feedback when
the branch does not touch helper-heavy backends. Use `just bench-check` when
benchmark harnesses change; it compiles criterion baselines with `cargo bench
--no-run` and does not collect benchmark measurements. Use `just test-full`,
`just clippy-full`, `just rust-full`, or `just release` for helper backend,
packaging, release-sensitive, or cross-surface changes. `just release` includes
`just bench-check`; `just rust-fast` intentionally does not.

真实 `just` AST 与 fake ordered trace 锁定 `release` 的唯一顺序：
`affected-self-test`、`schema-contract`、`audit`、`rust-full`、`bench-check`、
`target-tools`、`cli-package`、`cli-package-layout`、`desktop-package-config`、
`desktop-package`、`desktop-package-layout`、`smoke`、`diff-check`。
同一 witness 还锁定 `schema-audit-closed` 必须先执行 adoption witness，再通过
build lock 运行 `kanban-schema audit --require-closed`。

Use `just audit` to run the dependency gate. It executes `cargo deny check`
and `cargo audit -D warnings`; these tools inspect Cargo metadata and
`Cargo.lock` without writing the shared Cargo target directory. `just release`
includes this audit gate before the full Rust, packaging, desktop, and smoke
checks.

Desktop validation and packaging prepare Tauri sidecar binaries before checking
or building the app. The static config/sidecar check is separate from the
post-package `.deb` layout check:

```bash
just desktop-check
just desktop-package-config
just desktop-package
just desktop-package-layout
```

`just desktop-package` keeps the Desktop `.deb` separate from the CLI `.deb`,
while bundling helper binaries used by the embedded server. `just
desktop-package-layout` must run after `just desktop-package`; it inspects the
generated Desktop `.deb` with `dpkg-deb -c`.

Rust validation recipes run target-writing Cargo/Tauri commands through
`scripts/cargo-build-lock.sh`. The wrapper serializes target writes with a shared lock and
defaults local build/test parallelism to two jobs/threads to avoid swap-heavy
workspace gates. Override with `KANBAN_CARGO_BUILD_JOBS` /
`KANBAN_TEST_THREADS`, or tool-specific `CARGO_BUILD_JOBS`,
`NEXTEST_TEST_THREADS`, and `RUST_TEST_THREADS`. Set the repo-level values to
`auto` to leave the tool-specific variables unset, which is the preferred
Codex Cloud setting.

# Kanban Tool SPEC Bundle

本文档由以下文件合并而成：

- README.md
- docs/SPEC.md
- docs/ARCHITECTURE.md
- docs/STATE_MACHINE.md
- docs/DATA_MODEL.md
- docs/CLI_SPEC.md
- docs/API_SPEC.md
- docs/DISPATCHER_SPEC.md
- docs/IMPLEMENTATION_PLAN.md
- docs/ADR.md
- migrations/001_initial.sql
- migrations/003_comment_author_identity.sql

`docs/SPEC.md`、`docs/CLI_SPEC.md`、`docs/API_SPEC.md`、`docs/DISPATCHER_SPEC.md` 等分主题文档是当前行为的权威来源；本文件是这些源文档的同步快照，便于一次性阅读和离线传递。


---

# File: README.md

# Kanban Tool 文档包

本文档包面向一个 **Rust workspace 实现、SQLite-only、本地单机运行、同时提供 Web 与 CLI 能力** 的 Kanban 工具。

本项目不是 Trello 的简单复制品，而是一个本地优先的可执行工作队列：

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


---

# File: docs/SPEC.md

# Kanban Tool SPEC

版本：0.1  
范围：Rust core + SQLite-only + Web + CLI + local dispatcher  
约束：无多用户、无多租户、无远程同步、无 PostgreSQL 后端

---

## 1. 产品定位

本工具是一个本地优先的 Kanban 工作系统。它既能作为人类使用的看板，也能作为自动化任务、agent 工作流或本地脚本的 durable work queue。

核心目标：

1. **持久化**：任务、状态、依赖、评论、事件、运行历史必须落盘。
2. **可恢复**：本地进程崩溃后，任务可以通过 claim TTL / heartbeat / reclaim 恢复。
3. **可审计**：每次关键变化写入 `task_events`。
4. **多入口一致**：Web、CLI、dispatcher 必须走同一套 Rust use-case/service path
   （当前主要在 `kanban-sqlite::service`，并复用 `kanban-core` 状态机 helper），
   不允许绕过状态机直接写状态。
5. **SQLite-only**：第一版只支持 SQLite，不设计 PostgreSQL/MongoDB backend。
6. **单用户本地语义**：actor 是操作来源字符串，用于审计，不用于鉴权。

一句话定义：

> 一个 SQLite 驱动的本地 Kanban 状态机，暴露 CLI 和 localhost Web API，并可选运行本地 dispatcher 来执行任务。

---

## 2. 非目标

以下能力不进入当前设计：

- 多用户实时协作。
- 用户表、团队表、权限表、邀请机制。
- 多租户隔离。
- SaaS 部署。
- 跨机器 dispatcher/worker。
- SQLite 文件放在 NFS、Dropbox、iCloud Drive 等同步盘上共享写入。
- 任意自定义 workflow editor。
- 任意自定义字段数据库。
- 复杂自动化规则引擎。

---

## 3. 核心对象

| 对象 | 说明 |
|---|---|
| Board | 本地 project/board。不是租户。一个 SQLite DB 内可以有多个 board。 |
| Task | 看板卡片，也是可执行工作单元。 |
| Status | canonical 状态。UI column 只是状态的展示映射。 |
| Dependency | parent task 阻塞 child task。 |
| Comment | 人或自动化留下的协作文本；`kind=signal` 用作 signal ledger backlink。 |
| Event | append-only 事件流，用于审计、SSE、调试。 |
| Run | 一次执行 attempt。只有 claim/start 后才产生。 |
| Attachment | 附件元数据，blob 存文件系统。 |
| Label | 本地标签。 |
| Label Semantics / Atoms | Label 的 canonical ontology truth，用于本地 suggest 与 review。 |
| Label Proposal / Ontology Ledger | 新 label 候选 lifecycle 与 append-only provenance；它们解释 ontology 演化，但不替代当前 label truth。 |
| Signal Observation / Signal | 通用 Agent/Product signal ledger；记录产品或 agent 操作信号、review lifecycle 和可选 task/run/comment context。 |
| Column | UI 展示配置，映射到 status。 |

---

## 4. 状态模型

Canonical status：

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

### 4.1 状态语义

| 状态 | 语义 |
|---|---|
| `triage` | 待澄清、待补全规格、尚不可执行。 |
| `todo` | 已定义，但依赖未完成，或尚未进入 ready 队列。 |
| `scheduled` | 已定义，但 `scheduled_at` 在未来。 |
| `ready` | 可被人工或 dispatcher claim。 |
| `running` | 已被某个 actor/worker claim，正在执行。 |
| `blocked` | 因外部依赖、失败、人工输入等原因阻塞。 |
| `review` | 执行完成但需要人工检查。 |
| `done` | 完成。 |
| `archived` | 归档，不参与默认列表和调度。 |

### 4.2 关键原则

1. `running` 只能通过 `claim/start` transition 进入。
2. `ready -> running` 必须在单个 SQLite transaction 中完成 CAS update、创建 run、写 event。
3. `blocked -> ready` 不能盲目设置，必须重新检查依赖与 schedule。
4. UI 拖拽到列时，本质上调用 transition，不是直接 update `tasks.status`。
5. CLI 也不能绕过 transition service。

完整转换表见 [`STATE_MACHINE.md`](STATE_MACHINE.md)。

---

## 5. 存储模型

### 5.1 SQLite 文件位置

默认路径遵循 XDG 目录约定：

```text
~/.local/share/kb/kb.db
~/.local/share/kb/attachments/
~/.local/state/kb/logs/
~/.config/kanban/config.toml
```

也支持项目本地模式：

```text
.project/
  .kb/
    kb.db
    attachments/
```

通过 CLI 指定：

```bash
kanban --db .kb/kb.db task list
```

### 5.2 SQLite 配置

每个连接初始化时必须执行：

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 120000;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;
```

### 5.3 存储策略

采用：

```text
tasks 当前快照 + task_events append-only 事件流
```

不采用纯 event sourcing。原因：

- 查询当前看板需要快照表，不能每次重放事件。
- 事件流用于审计、实时推送、调试、增量同步到 Web UI。
- 快照与事件必须在同一 transaction 内更新。

初始 schema 见 [`../migrations/001_initial.sql`](../migrations/001_initial.sql)。

### 5.4 Label ontology truth 与 provenance

Label 系统保持四类角色分离：

1. `labels` / `task_labels` 是任务当前绑定事实。
2. `label_semantics` / `label_atoms` 是 label 的 canonical ontology truth。
3. `lancedb_label_atoms` 等向量索引是可删除重建的派生检索层。
4. `label_semantic_proposals` 与 `label_ontology_*` ledger 记录候选、分歧、review、
   mutation provenance 和 validation history。

当前不引入 label-ontology 专属 graph projection。现有 `kanban graph` / Oxigraph 只消费
Knowledge Substrate 的 `entity_relations` mirror；ontology review、atom explain、proposal
和 validation history 直接从 SQLite truth 读取。未来若为 rename/split/merge 或 provenance
关系查询增加 graph projection，它必须是可删除重建的 derived store，不能拥有 canonical write
path。

Constructive ontology mutation 必须通过专用 service path：semantics patch/replace、
atom apply、task-label bootstrap、proposal create/accept 和 validation 都要在同一 SQLite
transaction 中写入对应 canonical row 与 provenance action，或一起回滚。通用
ontology action endpoint 只允许 lifecycle review action，不能伪造 canonical before/after
hash、result atom/result label/result proposal 或 validation evidence。

已提交的 label-scoped semantics/atom mutation 可以通过专用 revert path 追加
`revert_ontology_mutation` action：它要求当前 canonical hash 仍等于被撤销 action 的
after hash，将 semantics 恢复到 before snapshot，标脏 atom index，并保留原 action
history。该 path 不承担 bootstrap label identity 或 task binding 回滚。

Semantics upsert 默认是 patch，不是 full replace：缺省字段保留当前值，数组字段追加或按
`remove_*` 删除；只有显式 replace 才把缺省数组解释为空。`expected_semantics_hash`
用于防止 lost update。Proposal accept 与单 task bootstrap 共用 new-label adoption
primitive；proposal accept 不自动写 `task_labels`，bootstrap 会绑定来源 task。旧数据或
cleanup 路径中缺少 action provenance 的 atom 只能通过 `legacy_untracked=true` 标记，不应
被当作新的 ontology growth 方式。
当 apply atom 发现同内容 atom 已存在时，只写 `adopt_existing_atom` provenance-only action；
它连接新的 source signals 到 existing atom，不修改 canonical semantics/atoms，也不触发
derived atom index dirty。

---

## 6. Web 端能力

Web 端是 localhost UI，不是远程协作服务。

默认监听：

```text
127.0.0.1:8721
```

主要页面：

1. Board 看板页。
2. Task detail drawer。
3. Comments。
4. Event timeline。
5. Runs / execution history。
6. Filter/search。
7. Settings。

Web 端只调用 HTTP API，不直接访问 SQLite。

API 见 [`API_SPEC.md`](API_SPEC.md)。

---

## 7. CLI 能力

CLI 是一等入口，必须覆盖核心生命周期：

```bash
kanban init
kanban board list
kanban board create agent-work --name "Agent Work"
kanban board use agent-work
kanban task create "实现 SQLite schema"
kanban task list --status ready
kanban task show agent-work#1
kanban task show t_xxx
kanban task start t_xxx
kanban task heartbeat t_xxx --claim-token <token>
kanban task block t_xxx "等待接口确认"
kanban task unblock t_xxx
kanban task done t_xxx --claim-token <token>
kanban task archive t_xxx
kanban events t_xxx
kanban runs t_xxx
kanban serve
kanban dispatch --once
```

CLI 必须支持：

- `--json`：机器可读输出。
- `--db <path>`：指定 SQLite DB。
- `--board <slug-or-id>`：显式指定 active board。
- `--actor <name>`：覆盖 actor。
- 稳定退出码。

Active board 选择顺序是 `--board`、`KB_BOARD`、最近 `.kb/config.toml`、`default`。`kanban board use <board>` 写入项目级 `.kb/config.toml`，但仍使用同一个全局 SQLite DB。Task ref 必须支持全局 `t_...`、当前 board 的裸 seq / `#seq`、以及显式 `board#seq` / `board/#seq`；CLI 和 API 输出应带可复制的 `board_slug#seq` ref。

CLI 见 [`CLI_SPEC.md`](CLI_SPEC.md)。

---

## 8. Dispatcher 能力

Dispatcher 是本地可选组件。它不负责多人协作，只负责本地自动化：

1. 从 `ready` 中 claim 任务。
3. 为 claim 创建 `task_runs`。
3. 运行 worker profile。
4. 周期性 heartbeat。
5. 超时或崩溃后 reclaim。
6. 根据 worker exit status 写入 `done/review/blocked/ready`。

`ready` 表示显式人工 promote 意图；parent 完成、dependency 移除或 schedule 到期不会被 dispatcher 自动提升到 `ready`。

Dispatcher 见 [`DISPATCHER_SPEC.md`](DISPATCHER_SPEC.md)。

---

## 9. 核心不变量

实现必须保证：

1. 一个 task 同时最多一个 active claim。
2. 一个 active claim 必须有一个 active run。
3. `running` task 必须有 `claim_token`、`claim_owner`、`claim_expires_at`。
4. task 不能依赖自己。
5. dependency graph 不能形成环。
6. 有未完成 parent 的 child 不得进入 `ready/running`。
7. `archived` task 不参与默认 list、promotion、claim。
8. `done` 和 `archived` 是 terminal-like 状态；默认不再被 dispatcher 修改。
9. Archived board 不接受普通 task/comment/dispatcher 写入；只读 events/runs/comments 历史仍可审计。
10. Board archive 不会改变 task 状态；如果 board 上仍有 `running` task/run，必须拒绝 archive。
11. 每次状态变化必须写 `task_events`。
12. task snapshot 与对应 event 必须同 transaction 提交。
13. `tasks.status`、label binding truth、label semantics truth、ontology ledger 和派生检索层各自有明确写权限；derived stores 不拥有 canonical write path。
14. 新的 constructive ontology mutation 不通过 generic lifecycle action endpoint；必须由专用 command/API/service 路径同时写 canonical state 与 provenance action；采用已存在 atom 只写 `adopt_existing_atom` provenance action，不伪装成新增 atom。
15. label ontology graph projection 当前不存在；如未来新增，只能从 SQLite truth 派生并重建，不得成为 `labels`、`task_labels`、`label_semantics`、`label_atoms` 或 `label_ontology_*` 的写入口。
16. label ontology longitudinal regression corpus 是测试/评估基础设施：它可比较固定 corpus 的 selected labels、score 和 evidence atoms，但 corpus run 本身不得修改 canonical label/ontology/ledger truth，也不得成为日常 task label 绑定的默认流程。
17. label ontology quality analytics 是只读投影：denominator 来源必须可审计；raw disagreement signal count 不得被命名或解释为模型错误率、precision 或 recall。没有带 expected labels 的独立评估 cohort 时，precision/recall 必须显示为 unavailable。

---

## 10. 成功标准

MVP 完成时必须满足：

- 可以通过 CLI 初始化 DB、创建 task、查看 board、claim、complete、block、unblock。
- 可以通过 Web UI 完成同样操作。
- 状态转换不允许非法路径。
- 并发 claim 同一 task 时只能一个成功。
- 依赖未完成时 child 不会被提升到 `ready`。
- crash/timeout 后可以 reclaim。
- task events 能完整解释 task 当前状态是如何来的。
- SQLite migration 可重复测试。
- 所有核心命令有单元测试或集成测试。


---

# File: docs/ARCHITECTURE.md

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


---

# File: docs/STATE_MACHINE.md

# State Machine

本文件定义 canonical task status、合法 transition、guard 与 side effects。

---

## 1. Status Enum

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

建议 Rust 表示：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
```

---

## 2. 状态职责

| Status | 是否可编辑 | 是否可 claim | 是否默认展示 | 是否参与 promotion | 说明 |
|---|---:|---:|---:|---:|---|
| `triage` | 是 | 否 | 是 | 否 | 待澄清。 |
| `todo` | 是 | 否 | 是 | 否 | 已定义，但依赖未完成，或尚未被人工提升到 ready。 |
| `scheduled` | 是 | 否 | 是 | 否 | 等时间到；到期后仍需显式 promote 才进入 ready。 |
| `ready` | 是 | 是 | 是 | 否 | 已显式进入可执行队列。 |
| `running` | 部分 | 否 | 是 | 否 | 正在执行。 |
| `blocked` | 是 | 否 | 是 | 否 | 阻塞。 |
| `review` | 是 | 否 | 是 | 否 | 待检查。 |
| `done` | 部分 | 否 | 默认可隐藏 | 否 | 已完成。 |
| `archived` | 否 | 否 | 默认隐藏 | 否 | 归档。 |

---

## 3. Transition Commands

### 3.1 Create

```text
none -> triage | todo | scheduled | ready
```

Initial status 计算：

```text
if input.status explicitly provided and valid for creation:
    use explicit status
else if required spec missing:
    triage
else if scheduled_at > now:
    scheduled
else if parent dependencies exist and not all parents are done or archived:
    todo
else:
    ready
```

允许显式创建状态：

```text
triage | todo | scheduled | ready
```

不允许直接创建：

```text
running | review | done | archived
```

Side effects：

- insert `tasks`
- insert `task_events(kind='task.created')`

---

### 3.2 Specify

```text
triage -> todo | scheduled | ready
```

Guard：

- title 非空。
- description/spec 满足本地校验。
- 如果 `scheduled_at > now`，目标必须是 `scheduled`。
- 如果 parent dependencies 未全部进入 `done` 或 `archived`，目标必须是 `todo`。
- 否则可进入 `ready`。

Side effects：

- update task fields。
- insert `task_events(kind='task.specified')`。

---

### 3.3 Promote

```text
todo -> ready
scheduled -> ready
```

Guard：

- 所有 parent dependency 都是 `done` 或 `archived`。
- execution plan 不是 `unplanned`：必须有 step 形成 `planned`，或显式标记 `not_required` 并填写 reason。
- task 未 archived。
- 对 `scheduled`，必须 `scheduled_at <= now`。

Side effects：

- update status。
- insert `task_events(kind='task.promoted')`。

Promote 是显式 ready 意图，通常由人工 CLI/Web action 触发：

```bash
kanban task promote t_xxx
```

---

### 3.4 Claim / Start

```text
ready -> running
```

Guard：

- task.status == `ready`。
- `claim_token IS NULL`。
- 所有 parent dependency 都是 `done` 或 `archived`。
- execution plan 不是 `unplanned`。
- task 未 archived。

Atomic side effects in one transaction：

1. CAS update `tasks`：
   - `status = 'running'`
   - `claim_token = <new token>`
   - `claim_owner = <actor>`
   - `claim_expires_at = now + ttl`
   - `last_heartbeat_at = now`
   - `started_at = COALESCE(started_at, now)`
   - `lock_version = lock_version + 1`
2. insert `task_runs(status='running')`
3. update `tasks.current_run_id`
4. insert `task_events(kind='task.claimed')`

Failure：

- 若 affected rows = 0，返回 `claim_conflict` 或 `dependency_blocked`。

---

### 3.5 Heartbeat

```text
running -> running
```

Guard：

- task.status == `running`。
- claim token 匹配。
- claim 未被 force reclaimed。

Side effects：

- extend `claim_expires_at`。
- update `last_heartbeat_at`。
- update active `task_runs.last_heartbeat_at`。
- insert `task_events(kind='task.heartbeat')` 可采样写入，避免过多事件。

建议：

- 默认每次 heartbeat 更新 run/task。
- event 可每 N 次或每 60s 写一次。
- 对 `running` task，后续有效 task-scoped event（例如 comment、step、label 变更）也可作为 implicit liveness signal：服务层刷新 task `claim_expires_at`、`last_heartbeat_at` 和 active run `last_heartbeat_at`，但不额外写 `task.heartbeat` event，避免递归和轮询噪音。
- board-level event 或没有 `task_id` 的 event 不刷新 running lease。

---

### 3.6 Complete

```text
running -> done
review -> done
```

Guard：

- `running -> done` 必须 claim token 匹配，除非 `force=true`。
- `review -> done` 不需要 claim token。
- 如果存在 required steps，它们必须全部为 `done` 或 `skipped`；optional steps 不阻塞 parent complete。

Side effects：

- update task status `done`。
- set `completed_at = now`。
- clear claim fields。
- update active run status `succeeded`。
- insert `task_events(kind='task.completed')`。
- 不自动 promote child tasks；child 保持 `todo`，由 derived dependency state 表示是否仍被 parent 阻塞。

---

### 3.7 Submit Review

```text
running -> review
```

Guard：

- claim token 匹配，除非 `force=true`。

Side effects：

- update task status `review`。
- clear claim fields。
- update active run status `succeeded` with `outcome='review'`。
- insert `task_events(kind='task.submitted_for_review')`。

---

### 3.8 Block

```text
triage | todo | scheduled | ready | running | review -> blocked
```

Guard：

- reason 非空。
- 若从 `running` block，必须 claim token 匹配，除非 `force=true`。

Side effects：

- update status `blocked`。
- set `status_reason`。
- if running: close active run as `failed` or `canceled` depending input。
- clear claim fields。
- insert `task_events(kind='task.blocked')`。

---

### 3.9 Unblock

```text
blocked -> triage | todo | scheduled | ready
```

目标状态计算：

```text
if spec incomplete:
    triage
else if scheduled_at > now:
    scheduled
else if parents not all done:
    todo
else:
    ready
```

Side effects：

- clear `status_reason`。
- update status to computed target。
- insert `task_events(kind='task.unblocked')`。

---

### 3.10 Reclaim

```text
running -> ready | blocked
```

Guard：

任一条件满足：

- `claim_expires_at <= now`。
- worker PID 已不存在。
- run 超过 max runtime。
- 人工 `force=true`。

目标状态：

- 默认 `ready`。
- 如果 retry_count >= max_retries，则 `blocked`。

Side effects：

- close active run as `expired` or `canceled`。
- clear claim fields。
- increment retry_count if appropriate。
- insert `task_events(kind='task.reclaimed')`。

---

### 3.11 Archive

```text
triage | todo | scheduled | ready | blocked | review | done -> archived
```

默认不允许直接 archive `running`，除非 `force=true`。

Side effects：

- set `archived_at = now`。
- set status `archived`。
- clear claim fields if force。
- insert `task_events(kind='task.archived')`。

---

### 3.11.1 Board archive

Board archive is a board lifecycle operation, not a task status transition.

Rules：

- Set `boards.archived_at = now`。
- Insert `task_events(kind='board.archived')`。
- Do not rewrite tasks on that board.
- Reject archive if the board has any `running` task or any `running` task_run.
- After archive, ordinary task/comment/dispatcher mutations against that board are rejected.
- Read-only history queries for events, runs, and comments remain available for audit.

---

### 3.12 Reopen

```text
done -> triage | todo | scheduled | ready
```

Guard：

- 只允许 `done` task reopen；`review`、`archived` 和非 done task 必须拒绝。
- `reason` 必须非空。

目标状态由服务端重新计算，不由调用方指定：

```text
if spec incomplete -> triage
else if scheduled_at > now -> scheduled
else if parent dependencies not all done/archived -> todo
else if execution plan is not ready -> todo
else -> ready
```

Side effects：

- clear `completed_at`。
- preserve `result_summary` / `result_json`。
- insert `task_events(kind='task.reopened')`，payload 包含 `from`、`to`、`reason`、`original_completed_at`。
- 直接依赖该 task 的 child 中，仅 `triage|todo|scheduled|ready` 会按 readiness 重新计算；`running|blocked|review|done|archived` 不隐式改写。

---

## 4. Transition Matrix

| From \ To | triage | todo | scheduled | ready | running | blocked | review | done | archived |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| none | create | create | create | create | - | - | - | - | - |
| triage | - | specify | specify | specify | - | block | - | - | archive |
| todo | - | - | schedule | promote/manual | - | block | - | - | archive |
| scheduled | - | unschedule | - | promote | - | block | - | - | archive |
| ready | - | demote | schedule | - | claim | block | - | - | archive |
| running | - | - | - | reclaim | - | block | submit_review | complete | force_archive |
| blocked | unblock | unblock | unblock | unblock | - | - | - | - | archive |
| review | - | - | - | - | - | block | - | complete | archive |
| done | reopen | reopen | reopen | reopen | - | - | - | - | archive |
| archived | restore | restore | restore | restore | - | - | - | - | - |

`demote`、`schedule`、`restore` 可作为 v1+ 命令；task-level `reopen` 当前只实现 `done -> recomputed active status`。

---

## 5. Dependency Rules

### 5.1 依赖语义

```text
parent_task_id -> child_task_id
```

表示 child 被 parent 阻塞。只有 parent 为 `done` 或 `archived` 时，child 才能进入 `ready` 或 `running`。归档 parent 会满足 hard dependency guard，但不会删除 dependency edge，也不会自动 promote child。

### 5.2 规则

1. parent != child。
2. 新增依赖不能产生环。
3. 如果给一个 `ready` child 增加未完成 parent（不是 `done` 或 `archived`），child 必须降级为 `todo`。
4. 如果 parent 从 `done` 被 reopen，仅直接 child 中的 active recomputable 状态（`triage|todo|scheduled|ready`）按 readiness 重新计算；`blocked|review|running|done|archived` 不隐式改写。
5. `running` child 不应被新增未完成依赖；除非 force，并且需要 block/reclaim。

---

## 6. UI Column Mapping

UI column 不是状态真相，只是展示配置。

默认列：

| Column | Status |
|---|---|
| Triage | `triage` |
| Todo | `todo` |
| Scheduled | `scheduled` |
| Ready | `ready` |
| Running | `running` |
| Blocked | `blocked` |
| Review | `review` |
| Done | `done` |

`archived` 默认隐藏。

拖拽行为：

- 从 `ready` 拖到 `running`：调用 claim/start。
- 从 `running` 拖到 `done`：调用 complete，需 active claim 或 force。
- 从任意非 terminal 拖到 `blocked`：弹窗要求 reason，调用 block。
- 从 `blocked` 拖到其他列：调用 unblock，不直接设目标状态。
- 拖到 `archived`：调用 archive。

---

## 7. Testing Requirements

必须覆盖：

1. transition matrix 单元测试。
2. dependency cycle detection。
3. `ready -> running` 并发 claim 只有一个成功。
4. expired claim reclaim。
5. block/unblock 重新计算目标状态。
6. completion 后 child 保持 `todo`，并清除 derived dependency-blocked state。
7. archived task 不被 dispatcher 处理。
8. `unplanned` task 不能 promote/claim，dispatcher 也不能 claim。
9. required step 未完成时 parent 不能 complete。
10. illegal direct transition 返回 `invalid_transition`。


---

# File: docs/DATA_MODEL.md

# Data Model

本文件定义领域模型、SQLite 表、ID、时间、JSON、附件、事件与常用查询。

---

## 1. ID 规范

所有 public ID 使用带前缀的 ULID/UUID-like string，便于日志和 CLI 区分。

| 对象 | 前缀 | 示例 |
|---|---|---|
| Board | `b_` | `b_01HY...` |
| Task | `t_` | `t_01HY...` |
| Run | `r_` | `r_01HY...` |
| Comment | `c_` | `c_01HY...` |
| Attachment | `a_` | `a_01HY...` |
| Label | `l_` | `l_01HY...` |
| Column | `col_` | `col_ready` |
| Event | `e_` | `e_01HY...` |

`task_events.id` 同时保留自增 integer，用于 SSE offset 和顺序分页。

---

## 2. 时间规范

所有时间字段使用：

```text
INTEGER unix epoch milliseconds UTC
```

字段命名：

- `created_at`
- `updated_at`
- `scheduled_at`
- `started_at`
- `completed_at`
- `archived_at`
- `claim_expires_at`
- `last_heartbeat_at`

Rust 内部建议使用 `time::OffsetDateTime`，DB 边界转换为 `i64` milliseconds。

---

## 3. JSON 字段规范

SQLite 中 JSON 存 `TEXT`，必须满足：

```sql
CHECK(json_valid(field_name))
```

默认值：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{}
```

用途：

| 字段 | 说明 |
|---|---|
| `tasks.metadata_json` | 轻量扩展信息。 |
| `task_runs.metadata_json` | worker profile、环境、命令摘要等。 |
| `task_events.payload_json` | event payload。 |

禁止把大对象、stdout/stderr 全量日志、附件 blob 放进 JSON。

---

## 4. Board

Board 是本地 project/board，不是 tenant。

主要字段：

| 字段 | 说明 |
|---|---|
| `id` | `b_` prefixed ID。 |
| `slug` | CLI/Web 使用的人类可读短名。 |
| `name` | 展示名。 |
| `description` | 可选说明。 |
| `created_at` | 创建时间。 |
| `updated_at` | 更新时间。 |
| `archived_at` | 归档时间。 |

默认 board：

```text
default
```

Board slug 由 service 层校验：必须唯一、非空、不超过 64 bytes，以小写 ASCII 字母或数字开头，只能包含小写 ASCII 字母、数字、`.`、`_`、`-`，并且不能使用 `b_`、`t_`、`r_`、`c_`、`a_`、`l_`、`col_`、`e_` 等保留前缀。这样可以避免和 public ID、`board#seq` task ref、路径式 alias 语法冲突。

Archived board 默认不出现在 board list，也不接受普通 task/comment/dispatcher 写入。归档只设置 board 的 `archived_at` 并写入 `board.archived` event，不改变 task 状态；如果 board 上仍有 `running` task 或 `running` run，归档会被拒绝。Events、runs、comments 等只读历史仍可通过显式 task/board identity 查询，用于审计。

### 4.1 Board isolation 责任边界

SQLite 是 canonical truth，但 board isolation 由 schema、service 和 diagnostic gate 共同保证：

1. DB constraint：所有 board-scoped rows 都有 `board_id` 并引用 `boards(id)`；
   referenced task / label / run id 也各自有 FK，确保引用对象存在。`task_labels`、
   `task_dependencies`、`task_steps`、`task_execution_plans`、`task_runs`、`task_comments`、`task_attachments` 和较新的
   label semantics / atoms / ontology link 表使用包含 `board_id` 的复合 FK，直接阻止这些
   关系表出现 cross-board row。`task_events` 保留 nullable task/run refs 与
   `ON DELETE SET NULL` 语义，由 INSERT/UPDATE triggers 校验非空 refs 的 board scope。
2. Service guard：CLI、HTTP、desktop 和 dispatcher 的正常写路径必须先在同一 board
   scope 内 resolve task、label、run 等对象，再写关系 row；例如 task label binding、
   dependency、comment、event、run 和 attachment 都不应跨 board 组合。
3. Doctor/import check：`kanban doctor` 和 JSONL import final gate 会只读检查基础关系表
   中 `row.board_id` 与 referenced task / label / run 的 board 是否一致，并运行
   `PRAGMA foreign_key_check`。任一 violation 都会成为 hard-error issue；import 会在
   commit 前回滚整个 replace transaction。

---

## 5. Task

Task 是核心对象，既是看板卡片，也是可执行工作单元。

### 5.1 字段分组

#### Identity

| 字段 | 说明 |
|---|---|
| `id` | Task ID。 |
| `board_id` | 所属 board。 |
| `seq` | board 内递增数字，便于显示 `board#12`。 |

Task public identity 有两层：

- `id` 是全局唯一 `t_...`，可跨 board 直接定位 task。
- `seq` 只在同一 board 内唯一，CLI/API 展示时应组合成 `board_slug#seq`，例如 `agent-work#12`。

#### Content

| 字段 | 说明 |
|---|---|
| `title` | 必填。 |
| `description` | Markdown 文本。 |
| `status_reason` | block 等状态原因。 |
| `result_summary` | 完成摘要。 |
| `metadata_json` | 扩展字段。 |

#### Workflow

| 字段 | 说明 |
|---|---|
| `status` | canonical status。 |
| `priority` | integer enum-like priority level: `0` = P0 highest, `1` = P1, `2` = P2, `3` = P3 lowest/default. DB default is `3` and values are constrained with `CHECK(priority BETWEEN 0 AND 3)` after migrations. Create/update commands reject values outside P0-P3. |
| `position` | UI 排序键。 |
| `scheduled_at` | 计划时间。 |
| `due_at` | 截止时间，仅展示/过滤，不驱动状态机。 |
| `retry_count` | 已 retry 次数。 |
| `max_retries` | 最大 retry 次数。 |

#### Actor / Execution

| 字段 | 说明 |
|---|---|
| `assignee` | 人或 worker profile 名称。 |
| `created_by` | actor string。 |
| `claim_token` | active claim token。 |
| `claim_owner` | active claim actor。 |
| `claim_expires_at` | claim 过期时间。 |
| `last_heartbeat_at` | heartbeat 时间。 |
| `current_run_id` | active/latest run id。 |

#### Timestamps

| 字段 | 说明 |
|---|---|
| `created_at` | 创建。 |
| `updated_at` | 更新。 |
| `started_at` | 首次进入 running。 |
| `completed_at` | 完成。 |
| `archived_at` | 归档。 |

#### Concurrency

| 字段 | 说明 |
|---|---|
| `lock_version` | optimistic lock。 |

### 5.2 Priority 语义

`priority` 表示任务的相对重要性和排序权重，不表示状态机可执行性。`ready`
表示任务已经被人工或服务显式放入可 claim 队列；P0-P3 只影响列表和 dispatcher 在可选任务之间的排序。

优先级约定：

| Priority | 语义 | 示例 |
|---|---|---|
| `0` / P0 | incident、阻断当前目标、必须立即处理的任务。少量使用，不作为普通 ready 默认值。 | 修复导致本地队列无法 claim 的回归；解除发布前 P1/P0 reviewer blocker。 |
| `1` / P1 | 近期待办焦点，当前迭代或当前工作流应优先完成。 | 今天要完成的实现切片；当前 PR 必须补齐的测试。 |
| `2` / P2 | 重要 follow-up，但不阻塞当前主线。 | 整理文档示例；补充非关键 smoke。 |
| `3` / P3 | 普通 backlog、低优先级或默认值。 | 想法、低风险清理、未来可做的体验改进。 |

`ready` 与 P0 不能互相替代：

- 普通可执行任务应是 `ready` + P1/P2/P3，而不是为了进入队列全部标成 P0。
- P0 任务如果仍缺规格、排期未到或依赖未完成，仍不能被 claim；它应保持
  `triage`、`scheduled` 或 `todo`，直到满足状态机 guard 后再 promote 到 `ready`。
- Dispatcher 只 claim `ready` 任务；在多个 `ready` 任务之间，才按 priority 从
  P0 到 P3 排序。

---

## 6. Dependency

表：`task_dependencies`

Schema-level invariant：`parent_task_id` 和 `child_task_id` 必须都属于 row
`board_id`。旧数据库升级到 composite FK schema 前会先检查 existing cross-board rows；
发现不一致时 migration 会失败并要求先用 doctor/repair 清理。

字段：

| 字段 | 说明 |
|---|---|
| `parent_task_id` | 前置任务。 |
| `child_task_id` | 被阻塞任务。 |
| `created_at` | 创建时间。 |

语义：

```text
parent done or archived => child may become ready
parent neither done nor archived => child cannot be ready/running
```

添加依赖时必须做环检测。归档 parent 会满足 hard dependency guard，但 dependency edge 保留为历史，不会自动 promote child。

parent 从 `done` reopen 后，直接 child 中仅 `triage|todo|scheduled|ready` 会按 readiness 重算；`running|blocked|review|done|archived` 不隐式改写。


---

## 7. Step / Execution Plan

Step 是父任务内部的有序执行步骤，不是阻塞依赖关系。Step 可以是普通文本，
也可以链接到另一个普通 task 作为上下文。链接 task 不会自动创建
`task_dependencies` 边，也不会用 linked task 的状态自动完成 step；step 自己有独立的
`todo | done | skipped` 状态。

### 7.1 Steps

表：`task_steps`

Schema-level invariant：`parent_task_id` 必须属于 row `board_id`；可选的
`linked_task_id` 也必须属于同一 board，且不能等于 `parent_task_id`。Service 还必须
拒绝 archived parent、archived linked task、空白标题和 cross-board link。

字段：

| 字段 | 说明 |
|---|---|
| `id` | Step ID。 |
| `board_id` | 所属 board。 |
| `parent_task_id` | 被规划的父任务。 |
| `position` | 父任务内步骤排序键。 |
| `title` | 步骤标题。 |
| `body` | 可选说明文本。 |
| `linked_task_id` | 可选上下文 task。 |
| `required` | 是否阻塞父任务 complete/archive。 |
| `status` | `todo`、`done` 或 `skipped`。 |
| `resolution_note` | done/skip/reopen 的说明。 |
| `resolved_by` | 最近一次 resolution actor。 |
| `resolved_at` | 最近一次 resolution 时间。 |
| `created_by` | 创建 actor。 |
| `created_at` | 创建时间。 |
| `updated_by` | 最近更新 actor。 |
| `updated_at` | 最近更新时间。 |

索引：

- `idx_steps_parent_position(parent_task_id, position)`
- `idx_steps_linked_task(linked_task_id)`
- `idx_steps_board_status(board_id, status)`

语义：

```text
parent task contains ordered step
optional linked_task_id supplies task context only
```

Step 不会直接驱动 `dependency_blocked` 或 `unfinished_parent_count`。Required step
只参与 execution-plan guard：父任务不能 complete/archive，直到所有 required step
都是 `done` 或 `skipped`。

### 7.2 Execution plans

表：`task_execution_plans`

字段：

| 字段 | 说明 |
|---|---|
| `board_id` | 所属 board。 |
| `task_id` | 被规划的 task。 |
| `state` | `unplanned`、`planned` 或 `not_required`。 |
| `reason` | `not_required` 的说明。 |
| `updated_by` | 最近更新 actor。 |
| `updated_at` | 最近更新时间。 |

索引：

- `idx_execution_plans_board_state(board_id, state)`

派生口径：

```text
steps count > 0 => planned
explicit not_required row and no steps => not_required
otherwise => unplanned
```

事件：

```text
task.step.created
task.step.updated
task.step.removed
task.step.done
task.step.skipped
task.step.reopened
task.execution_plan.planned
task.execution_plan.not_required
```

## 8. Run

表：`task_runs`

Schema-level invariant：`task_id` 必须属于 row `board_id`。这保证 run attempt
不能在 SQLite 层跨 board 指向 task。

Run 是一次 execution attempt。

### 8.1 Run status

```text
running | succeeded | failed | canceled | expired
```

### 8.2 字段

| 字段 | 说明 |
|---|---|
| `id` | `r_` prefixed ID。 |
| `task_id` | 关联 task。 |
| `status` | run 状态。 |
| `worker_profile` | worker profile 名。 |
| `worker_pid` | 本机 PID。 |
| `claim_token` | 对应 claim。 |
| `started_at` | run 开始。 |
| `last_heartbeat_at` | 最近 heartbeat。 |
| `finished_at` | run 结束。 |
| `exit_code` | worker 退出码。 |
| `summary` | 简短摘要。 |
| `error` | 错误文本。 |
| `log_path` | stdout/stderr 日志路径。 |
| `metadata_json` | 执行元数据。 |

### 8.3 约束

- active `running` task 必须有 active run。
- 一个 task 可以有多个历史 run。
- 同一 task 同时最多一个 running run。

SQLite 不强制最后一条，需要 service 层和 transaction 保证。

---

## 9. Event

表：`task_events`

Event 是 append-only 事实记录。

### 9.1 Event kind

API/SSE 当前类型化的 39 个 known kind：

```text
board.created
board.archived
dependency.added
dependency.removed
label.created
label.deleted
signal.recorded
signal.reviewed
task.archived
task.blocked
task.claimed
task.comment.created
task.completed
task.created
task.execution_plan.not_required
task.execution_plan.planned
task.execution_plan.unplanned
task.heartbeat
task.label.added
task.label.removed
task.label_proposal.accepted
task.label_proposal.proposed
task.label_proposal.rejected
task.promoted
task.reclaimed
task.recomputed
task.reopened
task.retry_policy.updated
task.specified
task.step.created
task.step.done
task.step.removed
task.step.reopened
task.step.skipped
task.step.updated
task.submitted_for_review
task.unblocked
task.updated
task.export_sanitized
```

### 8.2 Payload 示例

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_owner": "alice",
  "metadata": {}
}
```

`task_events.kind/payload_json` 的 SQLite storage 允许未来 unknown kind。Events API 与 SSE
对上面 39 个 known kind 使用精确 sibling payload contract，known mismatch fail closed；unknown
kind 的合法 JSON payload 保持 lossless。外层 `task_id`、`run_id`、`actor` 都是
required-nullable。portable JSONL 的 event payload 仍是 opaque JSON，不复用该 typed union。

### 8.3 使用场景

- Task detail timeline。
- SSE event stream。
- Debug dispatcher。
- CLI `kanban events`。
- 未来 export/import。

---

## 9. Comment

表：`task_comments`

字段：

| 字段 | 说明 |
|---|---|
| `id` | Comment ID。 |
| `task_id` | 关联 task。 |
| `author` | actor string。 |
| `author_type` | `user` / `agent`，表示评论作者身份；本地操作者是 `user`，其它自动化来源是 `agent`。 |
| `agent_type` | 可选 open text，仅用于 `author_type=agent`，例如 `executor` / `reviewer`。 |
| `body` | Markdown 文本。 |
| `kind` | `note` / `decision` / `signal`，表示 comment 内容语义，不表示作者身份。`signal` 是 signal ledger backlink。 |
| `metadata_json` | `kind` 对应的结构化 payload；默认 `{}`，必须是合法 JSON object。`kind=decision` 时必须符合 decision schema。`kind=signal` backlink metadata 包含 `type:"signal_link"`、`signal_id`、`observation_id`、`signal_kind`、`signal_status`。 |
| `created_at` | 创建时间。 |

旧 comment rows / JSONL import 会迁移到新语义：旧 `human` 变为 `user`，旧 `agent/system` 或 `worker/system` 来源变为 `agent`，旧 `text/system/worker` 内容变为 `note`。没有结构化 metadata 的旧 `decision` 也按 `note` 保留 body fallback。

Comment 创建时也写一条 `task_events(kind='task.comment.created')`。

`metadata_json` 是 SQLite canonical storage 列；CLI/API response 会解码成自然、无损的
`metadata` object。普通 note/signal metadata 保持开放。只有 service-generated backlink 的
完整 shape 由 `SignalLinkMetadataOutput` 独立证明，不能把用户自定义的同名键碰撞当成协议。

Decision comment metadata schema：

- `options`：非空 array。
- 每个 option 是 object，且包含非空 string `slug`、`title`、`detail`。
- `slug` 必须是稳定小写 ASCII slug：以小写字母或数字开头，只包含小写字母、数字和 `-`；同一 decision 内唯一。
- `selected`：非空 string，必须匹配某个 option slug。
- `reason`：非空 string。
- `risk` / `verification`：可选；如果出现，必须是非空 string。
- 未知顶层字段允许保留，但不参与状态机、dispatcher 或 event 语义。

---

## 10. Attachment

Blob 不存 DB。

默认路径：

```text
~/.local/share/kb/attachments/<board_id>/<task_id>/<attachment_id>/<filename>
```

DB 存：

| 字段 | 说明 |
|---|---|
| `id` | Attachment ID。 |
| `task_id` | 关联 task。 |
| `filename` | 原始文件名。 |
| `rel_path` | 相对 data dir 的路径。 |
| `content_type` | MIME。 |
| `size_bytes` | 大小。 |
| `sha256` | 内容 hash。 |
| `created_by` | actor。 |
| `created_at` | 上传时间。 |

安全要求：

- `filename` 必须 sanitize。
- `rel_path` 必须在 data dir 内。
- 不允许 `../` path traversal。

---

## 11. Label

Label 是轻量分类。

字段：

| 字段 | 说明 |
|---|---|
| `id` | Label ID。 |
| `board_id` | 所属 board。 |
| `name` | 标签名。 |
| `color` | UI 颜色 token。 |
| `created_at` | 创建时间。 |
| `updated_at` | 更新时间。 |

同一 board 内 label name 唯一。

Task 与 label 的关联通过 `task_labels(task_id, label_id)` 关联表表达。
Label 只用于分类、过滤和展示；添加或移除 label 不改变 `tasks.status`，
不触发 dependency recompute，也不会让 dispatcher claim `review` 或其他非
`ready` 状态。

### 11.1 Label semantics

`labels` 仍是 canonical label identity：名称、颜色和 board 作用域由 `labels`
定义。`task_labels` 仍是 task 的最终 label 绑定事实。语义推荐和向量检索使用
额外 truth 表，不替代这两张表。
`labels` identity CRUD 是基础 vocabulary registry，不写 ontology mutation ledger；
`label delete` 不会隐式删除 semantics/atoms，必须先通过 CAS-protected semantics clear
清空语义。

表：`label_semantics`

| 字段 | 说明 |
|---|---|
| `label_id` | 关联 `labels(id)`，一条 label 最多一条 semantics。 |
| `board_id` | 冗余 board scope，用复合外键保证 label/board 一致。 |
| `description` | label 的自然语言说明。 |
| `applies_when` | JSON string array，正向适用条件。 |
| `excludes_when` | JSON string array，反向排除条件。 |
| `positive_examples` | JSON string array，正向示例。 |
| `negative_examples` | JSON string array，反向示例。 |
| `created_at` / `updated_at` | 语义记录时间。 |

表：`label_atoms`

`label_atoms` 是从 `label_semantics` 与 label name 展开的 SQLite materialized
projection。它保存 positive 与 negative 两种 polarity，供后续 Group OMP/NNLS label
solver 和 LanceDB atom retrieval 使用；它随 semantics mutation 同事务重建，不是独立于
`label_semantics` 的第二份 semantic truth。

| 字段 | 说明 |
|---|---|
| `id` | 稳定 `la_...` atom id。 |
| `label_id` / `board_id` | 关联 canonical label 与 board。 |
| `polarity` | `positive` / `negative`。 |
| `kind` | `name`、`description`、`applies_when`、`positive_example`、`excludes_when`、`negative_example`；有 description 时，`description` atom 是 `label: {name}\ndescription: {description}` canonical atom，无 description 时才使用 `name` fallback atom。 |
| `text` | trim 且规范化 whitespace 后的 atom 文本；每个非空行内部 whitespace collapse，canonical 行分隔保留，空文本不入库。 |
| `ordinal` | 同一 label 展开后的顺序；同语义重复 atom 去重时保留首次出现的 ordinal。 |
| `content_hash` | atom 语义内容 hash，用于派生层判断变化；输入为 `label_id + polarity + kind + normalized_text`，不包含 `ordinal`。 |
| `created_at` / `updated_at` | projection row 时间。 |

派生向量表：`kb_label_atoms`

`kb_label_atoms` 是 LanceDB 中的可重建 label atom 向量表，独立于 task chunk 表
`kb_chunks`。它按 `board_id`、`embedding_model`、`polarity` 查询 atom evidence，
返回 `label_id`、atom id、`polarity`、`kind`、`text` 和 LanceDB `_distance` 原始
distance 等字段。语义 label 候选会用返回的 atom vector 在本地重新计算
query/residual cosine similarity，不把 distance 当作 solver score。派生表损坏或缺少
provider 时只让 label atom index degraded，不影响普通 label CRUD、`task_labels` 绑定
或 task 状态机。

### 11.2 Generic signal ledger

Generic signal ledger 保存 agent/product 在 kanban 工作流中发现的通用问题信号，
例如 CLI 参数摩擦、提示误导、参数设计不符合 agent 惯用方式，或 operator 发现的
产品反馈。它是 board-scoped 审计账本和只读 inbox 数据源，不替代 `tasks.status`、
task comments、runs、events 或 label ontology ledger。

- `signal_observations` 保存一次观察的来源、actor、task/run/comment 关联和原始证据。
- `signals` 保存一个可独立 review 的通用 signal，并指向对应 observation。
- 通用 signal 与 `label_ontology_signals` 分离；ontology signals 仍只服务 label
  semantics/atom/proposal review 和 mutation provenance。
- 当前 public HTTP surface 只读取通用 signal；lifecycle 写操作仍由 CLI/runtime
  signal record 流程负责。
- Board-scoped list/review surface 只通过 board 路由读取：
  `/api/v1/boards/{board}/signals*`。单条详情
  `GET /api/v1/signals/{signal_id}` 是 operator-wide detail lookup，用于从
  backlink 或 inbox row 直接打开已知 signal；它不改变 signal 的 `board_id`
  truth，也不把 signal 混入其它 board 的列表。
- `signal_observations.task_id`、`run_id`、`comment_id` 是 provenance/history
  soft refs。当前一致性由 service 写入路径、doctor 和 import final gate 维护；
  这些 refs 允许保留历史来源语义，未来如需把全部来源关系硬化，可迁移为
  board-composite FK。

表：`signal_observations`

一行表示一次 agent 或 operator 观察。Observation 可关联 task、run 或 comment；
这些关联用于定位来源，不改变对应实体状态。

| 字段 | 说明 |
|---|---|
| `id` | `obs_...` observation id。 |
| `board_id` | 来源 board scope。 |
| `task_id` / `task_ref_snapshot` | 可空。来源 task 与捕获时的人类 ref 快照；task 后续改动不影响快照。 |
| `run_id` | 可空。来源 execution run。 |
| `comment_id` | 可空。来源 comment。 |
| `actor` / `agent_type` | 捕获者名称与可选 agent type。 |
| `source` | 可空。信号来源，例如 `codex-hook`、`cli` 或 `operator`。 |
| `evidence_json` | JSON object 字符串，保存命令、stderr、上下文片段、hook 提示等原始证据。 |
| `created_at` | 创建时间。 |

表：`signals`

一行表示一个可独立进入 operator inbox 的通用 signal。它只描述发现的问题和 review
lifecycle，不直接触发修复或修改 canonical workflow。

| 字段 | 说明 |
|---|---|
| `id` | `sig_...` signal id。 |
| `board_id` / `observation_id` | board scope 与来源 observation。 |
| `kind` | 通用 signal 类型，例如 `agent_cli_friction`。 |
| `title` / `summary` | 面向 operator 的短标题与摘要。 |
| `severity` | 文本严重度，例如 `info`、`medium` 或 `high`。 |
| `status` | `open`、`confirmed`、`resolved`、`rejected`、`superseded`。 |
| `dedupe_key` | 可空。用于调用方聚合相似 signal。 |
| `superseded_by_signal_id` | 可空。指向同 board 的替代 signal。 |
| `reviewed_by` / `reviewed_at` / `review_reason` | lifecycle review 记录。 |
| `created_at` / `updated_at` | 创建与更新时间。 |

默认 review queue 只读取 `open` 与 `confirmed` signals；完整历史需显式
`include_all` 或指定 status。

### 11.3 Label ontology ledger

Label ontology ledger 记录 task 标注过程里的证据、分歧 signal、review/action 历史
和 validation 结果。它是可查询的审计账本，不替代 canonical truth：

- `labels` / `task_labels` 仍决定 task 当前实际绑定哪些 label。
- `label_semantics` 决定 label 的 canonical 语义；`label_atoms` 是它的 SQLite
  materialized atom projection。
- `label_semantic_proposals` 仍负责新 label proposal lifecycle。
- ontology ledger 覆盖 semantics/atom mutations；base `labels` identity CRUD 位于
  ledger 之外，只写普通 events。

这些表是 label 系统中的不同角色，不是六个严格独立的存储层。`label suggest` 是计算结果，
`kb_label_atoms` 是可重建检索投影，proposal 和 ledger 是需要持久审计的 SQLite records；
它们都不能直接替代 `task_labels` 的当前绑定事实。

表：`label_ontology_observations`

一行表示一次完整的 task label 判断过程。它保存当时的 task 快照、agent 候选、
`label suggest` 快照、最终选择和由 snapshot 派生的 solver 指标；即使 task、label 或
atoms 后续变化，仍能还原当时为什么产生 signal。Observation 是只读 provenance：
record 写入不会修改 `task_labels`、`label_semantics`、`label_atoms`、label atom index 或
proposal。

| 字段 | 说明 |
|---|---|
| `id` | `lor_...` observation id。 |
| `board_id` / `task_id` | 来源 board 与 task。 |
| `task_ref_snapshot` | 捕获时的人类 ref，例如 `default#42`。 |
| `task_snapshot_json` | 捕获时的 task title、description、labels、version/hash 等快照。 |
| `suggest_input_hash` | 可空。按 label suggest 输入（normalized title + description）计算的窄 hash，用于 validation comparability；旧 observation 缺失时按 legacy incomparable 处理，不能静默 passed。 |
| `agent_candidates_json` | agent 原始候选 labels、置信度和理由。 |
| `suggestion_snapshot_json` | 完整 suggestion 输出、参数、模型和 index 状态快照；新 capture path 要保存未改写的原始 snapshot。 |
| `final_decision_json` | 最终接受、拒绝和未采用 labels 的判断。 |
| `suggest_coverage` / `suggest_coverage_cosine` / `suggest_residual_norm` | 可查询的 solver 指标。新 capture path 从 `suggestion_snapshot_json` 派生这些值；调用方不应重复手写。`suggest_coverage = clamp(1 - suggest_residual_norm, 0.0, 1.0)`，二者不是独立证据；`suggest_coverage_cosine` 是 query 与 fitted vector 的 cosine similarity，可作为补充指标。 |
| `suggest_needs_new_label` / `suggest_degraded` | 捕获时 suggestion 状态。新 capture path 从 `suggestion_snapshot_json` 派生这些值。`suggest_needs_new_label` 是 coverage review 兼容字段，不等于自动 vocabulary gap；判断新 label 需要结合 reason codes、evidence、diagnostics 和人工语义判断。 |
| `diagnostics_json` | suggestion diagnostics 数组。新 capture path 从 snapshot 的 `diagnostics` 派生；冲突的重复输入会被拒绝。 |
| `capture_fingerprint` | 同一 board 内幂等 fingerprint。 |
| `created_by` / `created_by_type` / `agent_type` | 捕获者身份。 |
| `created_at` | 创建时间。 |

表：`label_ontology_signals`

一行只表达一个可独立 review 的 ontology 问题，例如某个已有 label 漏选、
suggest 误选、存在 vocabulary gap 或 label 边界/名称问题。

| 字段 | 说明 |
|---|---|
| `id` | `los_...` signal id。 |
| `observation_id` / `board_id` | 来源 observation 与 board scope。 |
| `kind` | `false_negative`、`false_positive`、`vocabulary_gap`、`name_issue`、`boundary_issue`、`structure_issue`。 |
| `status` | `open`、`confirmed`、`resolved`、`rejected`、`superseded`。 |
| `target_label_id` / `target_label_name_snapshot` | 已有 label 目标；名称快照用于历史解释。 |
| `related_labels_json` | split/merge 等多 label 关系快照。 |
| `proposed_action` | `observe`、`add_positive_atom`、`add_negative_atom`、`update_semantics`、`bootstrap_label`、`rename_label`、`split_label`、`merge_labels`。 |
| `candidate_atom_polarity` / `candidate_atom_kind` / `candidate_text` | 建议 atom 的 polarity、kind 和泛化文本。 |
| `candidate_content_hash` | 按 `label_id + polarity + kind + normalized_text` 计算的聚合键。 |
| `proposed_label_name` / `proposed_label_name_normalized` | vocabulary gap 或 rename 候选。 |
| `proposal_json` | 新 label 或结构变更的候选语义快照。 |
| `agent_selected` / `suggest_state` / `suggest_score` / `suggest_rank` / `final_selected` | agent、suggest 与最终判断之间的分歧证据。 |
| `rationale` / `confidence` | 可审查理由和可选置信度。 |
| `signal_key` | observation 内幂等键。 |
| `superseded_by_signal_id` / `status_reason` | 关闭或替代原因。 |
| `created_at` / `updated_at` / `reviewed_at` / `closed_at` | 生命周期时间。 |

`label ontology review` 是基于 signals 的只读聚合投影，不是新的 canonical truth，也不是
新的可持久化 derived store。group key 来自调用方选择的维度：`label` 使用目标 label，
`proposed-label` 使用 normalized proposed label name，`candidate-atom` 优先使用
`candidate_content_hash`。没有 candidate
atom 的 signals 不会进入一个全局空值 bucket；fallback key 会带上 signal kind、target
label 或 proposed label、以及 proposed action，例如
`no-candidate-atom|kind:vocabulary_gap|proposed:ontology ledger|action:bootstrap_label`。
因此一个 group 的含义是“这些 signals 共享同一个 review key”，不是“这些 signals 已被证明
来自同一个根因”。

`cluster` 是 opt-in duplicate-signal review-aid，不默认启用，不写 canonical atoms，不自动
confirm/apply/validate/mutate，也不成为 SQLite truth。cluster key 每次 review 查询时从
已有 signal 文本和 review scope 重建：key 始终包含 signal kind、proposed action、target
label snapshot（或 id fallback）以及 proposed label scope，再附加优先级最高的
lexical-normalized candidate text，其次 proposed label，再其次 rationale，最后退回纯
scope 组合。这个 scope 前缀避免把相同文本但不同 label boundary/action 的 signals 强制
合并；输出中的 `cluster_key` 和 `cluster_reason` 只解释这个辅助分组来源。

Review queue 的默认排序使用 distinct source task count（`task_count`）作为主要热度指标，
再按 confirmed count、latest signal time 和 key 排序。`signal_count` 只是 group 内原始
signal 行数；同一 task 可以贡献多条 signals，所以它不能单独代表模型错误率、precision、
recall 或 label suggest 质量。需要质量指标时必须另有 denominator，例如 agreement cohort
或固定评估集。

`label ontology quality` 是一个只读 analytics 投影，不新增表，也不写 canonical truth。
它把 `label_ontology_observations` 作为 denominator 来源并在输出中记录该来源、distinct
task 数、observation 数、agreement/degraded observation 数、时间范围和 task ref sample；
同时把 `label_ontology_signals` 作为 raw disagreement numerator 来源，按 kind/status
给出原始 signal counts。只有当 denominator 中存在 agreement observations 时，才会给出
`disagreement_task_rate`；只有 signals 的数据集会明确返回 rate unavailable，避免把分歧
记录误称为错误率。Precision/recall 仍需要带 expected labels 的独立评估 cohort，当前
ledger signal 不能单独提供这些指标。

长期 label ontology regression corpus 属于测试/评估基础设施，不是新的 SQLite truth。
当前固定 corpus 测试使用临时 DB 和内存 label atom index 跟踪 important labels 的 known
positive/negative-control tasks，并比较 `label suggest` 的 selected labels、score 与
evidence atoms。Corpus run 本身应保持只读 canonical ontology；只有测试中显式模拟的
临时 semantics/atom 变更才会用于证明 comparison 能发现回归。真实 DB 上的长期 corpus
需要等稳定任务集积累后再扩展，不应替代 ledger signals、trusted validation 或人工 review。

当前没有 label-ontology 专属 graph projection。`label_ontology_*` 表本身就是 SQLite
provenance truth；`kanban graph` / Oxigraph 只投影 Knowledge Substrate 的
`entity_relations`，不保存或拥有 label ontology action/signal truth。若未来出现明确的
rename/split/merge 或 provenance relationship 查询需求，新增 projection 必须从
`labels`、`label_semantics`、`label_atoms`、`label_semantic_proposals` 和
`label_ontology_*` 重建，并通过 `index_outbox` / `derived_store_state` 表达 dirty、sync、
rebuild 和 error 状态；删除或损坏 graph 不得改变 canonical label/ontology/ledger rows。

表：`label_ontology_actions`

Action 是 append-only history，表示 reviewer/agent 实际确认、拒绝、修改 ontology 或
记录 validation 的动作。直接修改 label semantics 或接受 proposal 时，provenance
也写成 action。

| 字段 | 说明 |
|---|---|
| `id` | `loa_...` action id。 |
| `board_id` | board scope。 |
| `parent_action_id` | validation 等后续 action 指向被验证的 mutation action。 |
| `action_type` | `confirm`、`reject`、`supersede`、`resolve_no_change`、`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、`update_semantics`、`create_label_proposal`、`bootstrap_label`、`rename_label`、`split_label`、`merge_labels`、`validate`、`revert_ontology_mutation`。 |
| `reason` | 必填人工或 agent 理由。 |
| `target_label_id` / `result_label_id` | 修改目标与结果 label。 |
| `result_atom_id` / `result_atom_content_hash` | 新增或采用 atom 的软引用和稳定 hash。 |
| `result_proposal_id` | 关联的 `label_semantic_proposals`。 |
| `canonical_before_hash` / `canonical_after_hash` | 修改前后 canonical semantics hash。 |
| `change_json` | before/after/diff 或其它可解释变更快照。 |
| `validation_requirement` | `none`、`required`、`unsupported`。表达 parent mutation 是否需要 typed validation policy；不改写历史 attempt outcome。 |
| `validation_status` | `not_required`、`pending`、`passed`、`failed`、`partial`。对 mutation parent 是历史兼容/base status；对 `validate` action 表示一次 attempt outcome。 |
| `validation_json` | validation evidence envelope；service 会包装 supplied/collected payload、source signal cases、task snapshot comparability、parent action result 引用和 summary。公共 supplied/collected payload 只保存在 top-level `manual`；generated `cases[]` 用 `after.manual_case_ref` 指向 `manual.cases[]` 中对应 signal 的 evidence，避免把同一 payload 复制到每个 case。`failed` / `partial` 可保存 external/manual attestation 诊断。`passed` action 只能来自工具采集的 `trusted_automated` evidence（collector source、embedding model、solver options、clean atom index status/generation、per-signal before/after cases），并按 parent action 校验 positive atom、negative atom、bootstrap label 和 negative positive-control/waiver policy；调用方手写 JSON 或自称 `automated` 不构成可信来源。 |
| `created_by` / `created_by_type` / `agent_type` | action actor。 |
| `created_at` | 创建时间。 |

`validation_effective_outcome` 是读取 DTO 中的 reducer 结果，不是独立存储列。它按
`validation_requirement` 和 latest validation child action（`created_at,id`）计算：
`not_required`、`unsupported`、`pending`、`passed`、`failed` 或 `partial`。只有
`required + trusted passed` 会 resolve linked source signals；`unsupported` 可以记录
external failed/partial 诊断，但拒绝 passed。

`label_ontology_action_atom_effects` 连接一条 root mutation action 与本次实际 added/removed
atom snapshots。它保存 `board_id`、`action_id`、`label_id_snapshot`、`atom_id_snapshot`、
`atom_content_hash`、`polarity`、`kind`、`text`、`effect` 和 `created_at`；`effect` 只允许
`added` / `removed`，唯一约束为 `(action_id, atom_content_hash, effect)`。Action 使用
board-scoped composite FK；atom snapshot 不使用 live FK，因为 `label_atoms` 会随 projection
重建。

`result_atom_id` 故意不是强 FK。`label_atoms` 会随 semantics rebuild delete/insert；
历史 action/effect 依赖 `result_atom_content_hash`、effect row 和 `change_json` 中的 atom
snapshot 保持可解释。Atom explain 查询会优先使用
`label_ontology_action_atom_effects`，也允许用 legacy `result_atom_id` /
`result_atom_content_hash` 兼容旧数据。`adopt_existing_atom` 表示新的 source signal 采用了当前已存在 atom，
不代表 canonical 内容新增。已有 atom 如果来自旧 semantics 写入而没有任何 ontology action 引用，
查询结果只标记 `legacy_untracked=true`，不会伪造 provenance。

`create_label_proposal` action 对同一 `(board_id, result_proposal_id)` 唯一；proposal
accept 生成的 `bootstrap_label` action 通过 `parent_action_id` 指向这条 creation
action，从而让 proposal creation -> bootstrap acceptance provenance 链路保持无歧义。

`revert_ontology_mutation` 是 append-only rollback history：它不会修改或删除原 mutation
action，而是用 `parent_action_id` 指向被撤销 action，并把 canonical semantics 恢复到该
action 的 `change_json.before` / `canonical_before_hash` snapshot。当前实现只覆盖
label-scoped semantics/atom mutations（`add_positive_atom`、`add_negative_atom`、
`update_semantics`），成功后标脏 label atom index 并保持 validation pending；bootstrap
的 label identity / task binding rollback 不由该 action 类型表达。

当前 constructive ontology mutation path 的责任边界如下：

- `label_semantics` 是 canonical ontology truth；`label_atoms` 是它的 SQLite materialized
  projection；`label_ontology_actions` 是 append-only provenance，不是第二份 truth。
- `update_semantics`、`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
  `create_label_proposal` 和 `bootstrap_label` action 只能由专用 service path 写入。
  `adopt_existing_atom` 是 provenance-only path，before/after hash 相同，只连接新的
  source signals 到 existing atom，不修改 canonical semantics/atoms，也不标脏 atom
  index；其它 constructive mutation 与对应 canonical write 位于同一 SQLite transaction。
- 每个 semantics/atom mutation transaction 只写一条 root mutation action；`change_json`
  只保存一次 before/after semantics snapshot。实际 added/removed atoms 写入
  `label_ontology_action_atom_effects`，description-only patch 写零 effect，no-op patch 不写
  action/effect/index dirty。
- Manual mutation 可以没有 source signals，但仍必须记录 actor、reason、before/after
  hash 和 change snapshot。Signal-driven mutation 会额外写入
  `label_ontology_action_signals` links。
- `label semantics upsert` 默认是 patch/CAS path：`expected_semantics_hash` 防止
  lost update；缺省字段不清空旧 semantics；`replace=true` 才执行完整替换，并将缺省
  arrays 解释为空集合。
- Direct task-label bootstrap 与 proposal accept 共用 adoption primitive。Task-label
  bootstrap 可创建或复用无 semantics 的同名 canonical label；proposal accept 当前会先拒绝
  任何 existing normalized-name conflict，因此成功路径创建新 canonical label。二者都会写
  semantics/atoms、标脏 label atom index，并写一个 `bootstrap_label` root action 和 added
  atom effects；proposal accept
  不写 `task_labels`，task-label bootstrap 会绑定来源 task。失败时 canonical writes 与
  provenance action 一起回滚。
- `rename_label`、`split_label`、`merge_labels` 仍可作为 signal proposed_action 或 legacy
  action 读取；当前 public service/CLI/HTTP 不再写新的 structure plan mutation action。旧
  structure plan action 的 validation requirement 解释为 `unsupported`。
- `legacy_untracked=true` 只表示当前 atom 没有可匹配的 ontology action，例如旧数据或
  destructive cleanup 后的历史缺口；新 constructive mutation 不应依赖这种兼容路径来解释
  provenance。

表：`label_ontology_action_signals`

多对多连接 action 与 signals。多个 signals 可以支持一次 atom 修改；同一个 signal
也可以先被 confirm，随后关联 mutation action 和 validation action。

默认 review queue 只读取 `open` 与 `confirmed` signals；完整历史需显式 include all。
Mutation action 写入后通常保持 source signals 为 `confirmed`。只有 trusted automated
`passed` validation 会把 linked source signals 转为 `resolved`；external/manual
attestation、`failed` 或 `partial` validation 只追加历史，不删除 signals，也不把问题
伪装成已验证关闭。

---

## 12. Column

Column 是 UI 展示层。

字段：

| 字段 | 说明 |
|---|---|
| `id` | Column ID。 |
| `board_id` | 所属 board。 |
| `status` | 映射的 canonical status。 |
| `title` | UI 名称。 |
| `position` | UI 排序。 |
| `hidden` | 是否隐藏。 |
| `wip_limit` | 可选 WIP limit。 |

MVP：一个 status 对应一个 column。

---

## 13. Knowledge Substrate

Knowledge Substrate 表只支持实体身份、关系镜像、派生 outbox 和派生 store 健康状态。SQLite task/run/comment/event 仍是 operational source of truth。

### 13.1 Entity registry

表：`entities`

字段：

| 字段 | 说明 |
|---|---|
| `uri` | 稳定 `kb://...` entity URI。 |
| `kind` | `task` / `run` / `comment` / `artifact` / `skill` / `project`。 |
| `source_table` | 来源 SQLite 表。 |
| `source_id` | 来源 row id。 |
| `board_id` | 可选 board scope。 |
| `task_id` | 可选 task scope。 |
| `title` | 展示标题。 |
| `summary` | 简短摘要。 |
| `content_hash` | 内容 hash，用于派生层判断变化。 |
| `created_at` / `updated_at` / `archived_at` | 生命周期时间。 |

### 13.2 Relation graph mirror

表：`relation_predicates`、`entity_relations`

`relation_predicates` 定义受控 predicate；`entity_relations` 存可重建关系镜像。关系层用于 graph/context 查询，不改变 task 状态机。状态机仍以 `tasks.status`、`task_dependencies` 和 service transaction 为准。

### 13.3 Index outbox

表：`index_outbox`

字段：

| 字段 | 说明 |
|---|---|
| `id` | 自增 job id。 |
| `source_event_id` | 来源 `task_events.id`，允许事件被删除/导入时置空。 |
| `target` | `tantivy` / `oxigraph` / `lancedb` / `all`。 |
| `entity_uri` | 目标 entity。 |
| `action` | `upsert` / `delete` / `rebuild`。 |
| `payload_json` | 有界 job payload。 |
| `status` | `pending` / `running` / `done` / `failed`。 |
| `attempts` | 尝试次数。 |
| `last_error` | 最近失败原因。 |
| `created_at` / `updated_at` | job 时间。 |

`index_outbox` 是 at-least-once 派生 job surface。task mutation transaction 只写 SQLite truth、event、entity/outbox 记录，不直接写 Tantivy/Oxigraph/LanceDB。

### 13.4 Derived store state

表：`derived_store_state`

字段：

| 字段 | 说明 |
|---|---|
| `store_name` | 派生 store 名称，例如 `tantivy_tasks`、`oxigraph_relations`、`lancedb_chunks`、`lancedb_label_atoms`。 |
| `schema_version` | store schema/contract 版本。 |
| `last_event_id` | store 已成功提交的全局 `task_events.id` 水位。 |
| `dirty` | 是否仍有未完成 outbox、失败 outbox 或最近一次 store 更新失败。 |
| `last_rebuild_at` | 最近成功 rebuild 时间。 |
| `last_sync_at` | 最近成功 sync 时间。 |
| `last_error` | 最近失败证据。 |
| `updated_at` | 状态更新时间。 |

`last_event_id` 是 store 全局成功处理水位，不是 board 局部水位。成功 sync/rebuild 只能单调推进这个值；当一个 board sync 完成但其他 board 仍有 pending/running/failed outbox 时，`dirty` 必须保持 true。`dirty=false` 只表示同一 store target 当前没有 unfinished outbox 且最近一次 store 更新没有失败。

`last_error` 成功后清空，失败时保留错误证据并保持 `dirty=true`。Operator 应通过 `kanban derived status`、`kanban doctor`、maintenance API 和对应 `sync/rebuild` 命令恢复派生层；派生 store 损坏或落后不改变 SQLite task truth。

表：`label_atom_index_boards`

`label_atom_index_boards` 只跟踪可重建的 `lancedb_label_atoms` 派生层在各 board
上的刷新状态，不是 label truth。`label_semantics` / `label_atoms` 更新会把对应
board 标脏；单个 board 的 label atom rebuild 成功只清理该 board 的 dirty 标记。
只有该 store 下所有 board 都不 dirty 时，`derived_store_state.dirty` 才能变为
`false`。

### 11.2 Label semantic proposals

表：`label_semantic_proposals`

`label_semantic_proposals` 是新增 label 的持久提案生命周期，不是 canonical
label truth。它只记录“现有 label atom suggestion 覆盖不足时，外部/manual provider
给出的候选语义”。未显式 accept 前，不创建 `labels`、`label_semantics`、
`label_atoms` 或 `task_labels`。

| 字段 | 说明 |
|---|---|
| `id` | `lp_...` proposal id。 |
| `board_id` / `task_id` | 提案来源 task。 |
| `status` | `proposed` / `accepted` / `rejected`。provider 不可用不写成 status，而是返回 degraded attempt。 |
| `name` / `description` / `applies_when` / `excludes_when` / `positive_examples` / `negative_examples` | 候选 label semantics。数组字段为 JSON string array。 |
| `heuristic_coverage` / `heuristic_coverage_cosine` / `heuristic_residual_norm` | 来自当前 residual label suggestion solver 的覆盖/残差元数据，用于记录 proposal 创建时现有 label atoms 的覆盖程度；`heuristic_coverage = clamp(1 - heuristic_residual_norm, 0.0, 1.0)`，二者不是独立证据；`heuristic_coverage_cosine` 是 query 与 fitted vector 的 cosine similarity。 |
| `top1_existing_label_id` / `top1_existing_label_name` | 当前启发式 top1 existing label。 |
| `diagnostics_json` | JSON string array，包含 degraded、冲突或 validation 诊断。 |
| `decision_reason` / `resolved_label_id` / `decided_at` | accept/reject 决策信息；accept 后 `resolved_label_id` 指向新建 canonical label。 |

Accept 只允许 `proposed` proposal。accept 通过共享 adoption primitive 创建同 board 的
canonical `labels` 行，并写入对应 `label_semantics` / `label_atoms`，同时标脏
`lancedb_label_atoms` 派生 store，写入 `bootstrap_label` provenance action，并把
`resolved_label_id` 指向 result label；proposal status、canonical writes 与 action
provenance 同 transaction 提交。它不写入 `task_labels`，不会把新 label 自动绑定到来源
task。

Reject 将 proposal 标记为 `rejected`。与现有 label 发生 normalized-name 冲突的
候选会持久化为 `rejected`，diagnostics 包含 `near_duplicate_label_conflict`。
Normalized-name 冲突是忽略大小写、空白和标点后的 deterministic near-duplicate
heuristic。

## 14. 常用查询

### 14.1 Board task list

```sql
SELECT *
FROM tasks
WHERE board_id = ?
  AND status != 'archived'
ORDER BY
  CASE status
    WHEN 'triage' THEN 10
    WHEN 'todo' THEN 20
    WHEN 'scheduled' THEN 30
    WHEN 'ready' THEN 40
    WHEN 'running' THEN 50
    WHEN 'blocked' THEN 60
    WHEN 'review' THEN 70
    WHEN 'done' THEN 80
    ELSE 90
  END,
  position ASC,
  priority ASC,
  created_at ASC;
```

### 14.2 Ready queue

```sql
SELECT *
FROM tasks t
WHERE t.board_id = ?
  AND t.status = 'ready'
  AND t.claim_token IS NULL
  AND NOT EXISTS (
    SELECT 1
    FROM task_dependencies d
    JOIN tasks p ON p.id = d.parent_task_id
    WHERE d.child_task_id = t.id
      AND p.status NOT IN ('done','archived')
  )
ORDER BY t.priority ASC, t.created_at ASC
LIMIT ?;
```

### 14.3 Expired claims

```sql
SELECT *
FROM tasks
WHERE status = 'running'
  AND claim_expires_at IS NOT NULL
  AND claim_expires_at <= ?;
```

### 14.4 Event stream

```sql
SELECT *
FROM task_events
WHERE board_id = ?
  AND id > ?
ORDER BY id ASC
LIMIT ?;
```

---

## 15. Export / Import Format

JSONL export/import 是 portable board snapshot 格式：

```bash
kanban export --board default --format jsonl --out board.jsonl
kanban import --input board.jsonl --replace
```

每行：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"type":"task","data":{...}}
{"type":"event","data":{...}}
{"type":"comment","data":{...}}
{"type":"dependency","data":{...}}
```

Generic signal ledger 使用稳定 record types：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"type":"signal_observation","data":{...}}
{"type":"signal","data":{...}}
```

Label ontology ledger 使用稳定 record types：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"type":"label_ontology_observation","data":{...}}
{"type":"label_ontology_signal","data":{...}}
{"type":"label_ontology_action","data":{...}}
{"type":"label_ontology_action_atom_effect","data":{...}}
{"type":"label_ontology_action_signal","data":{...}}
```

Portable descriptor authority 共覆盖 21 个 discriminator；input/output 各有 exact root，共
42 个 Draft 2020-12 schemas。每行 `data` 闭合，required-nullable key 必须存在但可显式为
`null`，真实 export producer 与 import consumer 使用同一 descriptor/fixture registry。
SQLite 中的 `evidence_json`、`related_labels_json`、`proposal_json`、`change_json`、
`validation_json` 等仍是 canonical storage 列；公开 adapter 只暴露去掉 `_json` 后的自然 JSON。

Import 另有一条仅向前的 compatibility migration，用于读取 natural JSON contract 采用前、
由上一版 exporter 生成的 storage-native JSONL snapshot。该格式以 `column.hidden=0|1`
以及 `metadata_json` / `payload_json` 等真实 SQLite 列形状识别；同一 snapshot 必须保持
单一格式，不能混用 storage-native 与 natural records。Importer 会先把上一版 JSON text
列和 integer boolean 转为当前 natural record，再执行同一 exact contract validation 与下述
transaction/final consistency gates。当前及后续 export 始终只写 natural JSON，不再产生
storage-native keys；这不是长期双轨 public contract。

导入时会在同一 transaction 中先插入 rows，再运行 final consistency gate。基础关系表
会检查 `task_labels`、`task_dependencies`、`task_runs`、`task_comments`、
`signal_observations`、`signals`、`task_events`、`task_attachments` 的 row board 与
referenced task / label / run / comment / observation board 是否一致；失败时整个
`--replace` import transaction 回滚，不提交部分数据。

Ontology rows 也在同一 transaction 中插入，并延迟回填
`label_ontology_signals.superseded_by_signal_id` 与
`label_ontology_actions.parent_action_id`，避免依赖同表自引用 rows 的文件顺序。导入完成前会校验 ontology ledger board isolation：observation/signal board、action
parent board、action-signal link board、label/proposal soft reference board 必须一致；
orphan action-signal links、supersede cycles 和 action parent cycles 会导致 import
失败。

Generic `signals.superseded_by_signal_id` 同样会延迟回填，避免依赖同表自引用 rows 的文件顺序。

`kanban doctor --json` 对上述基础关系表、SQLite `PRAGMA foreign_key_check`、ontology
ledger consistency 和 generic signal ledger board consistency 规则做只读巡检。
基础关系表问题返回 `consistency_errors`、`consistency_warnings`、
`consistency_issues[]`；ontology ledger 问题返回 `ontology_ledger_errors`、
`ontology_ledger_warnings`、`ontology_ledger_issues[]`。Issue 包含 `severity`、
`code`、`message`、`record_ids`，用于定位损坏 row；基础关系表 message 包含
`table`、`row`、`row_board` 和 `referenced_board`，foreign-key issue 会记录 table、
rowid、parent table 和 FK index。Hard error 覆盖 row board mismatch、
missing v12 ontology table、跨 board link、orphan action-signal/action-effect link、generic
signal orphan/cross-board context、generic signal supersede cycle、parent/supersede 异常、label/proposal/task board mismatch、
supersede cycle 和 action parent cycle；非零
error 让 `ok=false`。Warning 保留给仍可解释或可重建的软引用，例如历史 action 的
`result_atom_id` 已被当前 `label_atoms` rebuild 删除。


---

# File: docs/CLI_SPEC.md

# CLI SPEC

默认 binary 名称：`kanban`

CLI 是一等入口；它与 Web 使用同一套 `kanban-sqlite::service` backed service path
和 SQLite schema。

---

## 1. Global Options

```bash
kanban [GLOBAL_OPTIONS] <COMMAND>
```

| Option | 说明 |
|---|---|
| `--db <path>` | 指定 SQLite DB；优先级高于 env、config 和 XDG 默认路径。 |
| `--board <slug-or-id>` | 显式指定 active board，优先级最高。 |
| `--actor <name>` | 操作 actor。默认 OS username。 |
| `--locale <auto|zh-CN|en>` | human 输出语言。默认 `zh-CN`；`auto`/`system` 使用系统 locale。 |
| `--json` | JSON 输出。 |

SQLite DB path 解析顺序：

1. `--db <path>`。
2. `KANBAN_DB` 环境变量。
3. `KB_DB` 环境变量（兼容短名）。
4. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `db = "<path>"`。
5. 用户全局 config `$XDG_CONFIG_HOME/kanban/config.toml`，读取 `db = "<path>"`。
6. fallback 到 XDG data 默认路径，通常是 `~/.local/share/kb/kb.db`。

Active board 解析顺序：

1. `--board <slug-or-id>`。
2. `KB_BOARD` 环境变量。
3. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `board = "<slug>"`。
4. fallback 到 `default`。

`kanban board use <board>` 会把当前目录写成项目级 `.kb/config.toml`；后续子目录自动继承该 active board。该配置只选择本地项目的 board，不创建新 DB。
如果同一配置文件也包含 `db = "<path>"` 或 `[vector]`，`board use` 必须保留这些字段。配置中的相对 DB 路径按配置文件所在目录解析；环境变量和 `--db` 中的相对路径按当前工作目录解析。

Locale 只影响 human-readable 输出和错误消息，不改变 JSON key、状态枚举、task ref、ID、exit code 或机器可读 diagnostics。选择顺序：

1. `--locale <auto|zh-CN|en>`。
2. `KANBAN_LOCALE`。
3. 默认 `zh-CN`。

`auto` / `system` 会按 `LC_ALL`、`LC_MESSAGES`、`LANG` 解析系统 locale；当前只支持中文和英文。脚本和自动化应优先使用 `--json`，不要依赖 human 文案。

### 1.1 Config inspection

```bash
kanban config show [--json]
```

`config show` 输出当前 CLI 会使用的 SQLite DB path、active board 和 locale，以及每个值的来源。该命令用于 agent/operator 排查 precedence，不会打开、初始化或创建 SQLite DB。

`--json` 输出使用普通 `{ "data": ... }` envelope，`data` 结构如下：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "db": {
    "value": "/path/to/kb.db",
    "source": { "kind": "project_config", "path": "/repo/.kb/config.toml", "key": "db" }
  },
  "board": {
    "value": "kanban-tool",
    "source": { "kind": "env", "name": "KB_BOARD" }
  },
  "locale": {
    "value": "zh-CN",
    "input": "auto",
    "source": { "kind": "flag", "name": "--locale" }
  }
}
```

`source.kind` 是脚本可依赖的 ASCII 枚举：

| `source.kind` | 含义 |
|---|---|
| `flag` | 来自显式 CLI flag，例如 `--db`、`--board`、`--locale`。 |
| `env` | 来自环境变量，例如 `KANBAN_DB`、`KB_DB`、`KB_BOARD`、`KANBAN_LOCALE`。 |
| `project_config` | 来自最近的项目级 `.kb/config.toml`。 |
| `global_config` | 来自 `$XDG_CONFIG_HOME/kanban/config.toml`。当前只适用于 DB path。 |
| `default` | 来自 CLI 默认值或 fallback。 |

`locale.value` 是实际解析后的 locale；当输入为 `auto` / `system` 时，`input` 保留原始选择，`value` 保留系统 locale 解析结果。`db.value` 对显式 flag 和环境变量保留调用方传入的路径形态；config 中的相对 DB 路径按 config 文件所在目录解析。

### 1.2 Help output contract

`kanban --help` 和公开 command group 的 `--help` 输出必须为每个公开 command/subcommand 行提供一句简短用途说明；隐藏内部命令（例如 `__complete`）除外。`kanban` 无参或公开 command group 缺少 subcommand 时必须显示同一类简洁帮助，而不是只输出 parser error；这仍属于 clap parse-time 路径，退出码为 2，且不输出 runtime JSON error envelope。全局 options 的 help 必须说明它们影响的是 SQLite DB、active board、actor、locale 或 JSON 输出，不改变 JSON key、状态枚举或 exit code 契约。

关键 agent-facing 输入面必须在命令 help 中优先展示安全路径：多行或 shell-sensitive 文本使用 `--description-file -`、`--body-file -`、`--metadata-json-file <PATH|->`、`--metadata-file <PATH|->` 或 `--input -`，避免 shell expansion / quoting 污染。危险、破坏性或容易误解的 flag 必须在 help 中说明语义，例如 `task archive --force` 绕过普通 archive guard，`import --replace` 是有意 backup/restore flow 的替换式恢复入口；兼容 no-op flag 必须明确写出 no-op。

对 `PATH|-` 文本输入（如 `--reason-file`、`--input`、`--body-file`、`--metadata-json-file`）与其变体，`kanban` 实现上约束单次输入上限为 1MiB。超过上限时返回 `invalid_input`，并在 `--json` 下通过 `error.message` 指明输入长度限制，CLI 端可用更高层分片策略。该约束覆盖 stdin 与文件输入，目的是避免错误输入导致 CLI 服务路径资源异常。

顶层 help 和关键 agent-facing 命令可以包含 `Examples:`，但示例必须保持短小、稳定，并与实际命令语义一致；不要把 CLI_SPEC 的完整说明复制进 help。CLI help contract 由 `crates/kanban-cli/tests/help.rs` 覆盖，防止公开 command 行退化为空描述。

顶层 `kanban --help` 必须包含简洁 `Error codes:` section，覆盖当前公开退出码，帮助 operator 在终端直接发现 parse/runtime error code 边界。该 section 是 human-readable discovery surface；脚本仍应依赖 `--json` 下的 `error.code` 和 `error.exit_code`，不要解析 help 文案。

### 1.3 JSON output contract

所有公开 `--json` 输出使用顶层 envelope：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {},
  "meta": {}
}
```

`meta` 只在需要分页、details 或 diagnostics 时出现。`data` 可以是一个对象，也可以是对象数组；公共输出不得依赖裸 tuple、未命名数组位置、只有内部 id 的临时数组，或只回显输入参数。命令需要表达关系、删除或当前选择时，应返回命名 DTO，例如 `edge.parent`/`edge.child`、`step`、`board`。Task-like DTO 必须带可复制的 `ref`、`id`、`board_id` 或 `board_slug` 中的必要身份字段。

`board current --json` 和 `board use --json` 的 `data.board` 是完整 board 对象；调用方应读取 `data.board.slug`，不要把 `data.board` 当字符串。

#### JSON error output

当 `--json` 已被 clap 成功解析，且错误发生在运行期 service/IO 路径时，CLI 输出稳定错误 envelope 到 stdout，并使用对应 exit code：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "error": {
    "code": "not_found",
    "message": "未找到：board missing",
    "exit_code": 3
  }
}
```

`error.code` 是脚本可依赖的 ASCII 枚举；`message` 是本地化 human-readable 说明；`exit_code` 与进程退出码一致。运行期 `--json` 错误不写 stderr。

`error.code` 不应依赖业务校验 message 文案推断；普通业务层 `KanbanError::InvalidInput` / `InvalidStatus` 都返回稳定 `invalid_input`。已通过 clap 解析后的用户配置 TOML 解析失败也属于 `invalid_input`：例如 `kanban --json config show` 读取 malformed `.kb/config.toml` 或 `$XDG_CONFIG_HOME/kanban/config.toml` 时，输出 runtime JSON error 到 stdout、退出 2、不写 stderr，并且不打开、初始化或创建 SQLite DB。仅对无结构化层外错误（IO、路径、异常第三方 text）以及穿过 `InvalidInput` 的 SQLite/maintenance lock sentinel 使用降级文本分类作为补充，例如 `sqlite_busy`。

参数解析错误发生在 clap 解析阶段，仍由 clap 输出 stderr 并退出 2；这类错误不输出 JSON envelope。没有 `--json` 时，运行期错误继续输出 human-readable stderr。

### 1.3.1 JSONL / NDJSON streaming boundary

JSONL/NDJSON 只适用于 streaming 或 record-oriented surfaces，例如 portable export/import、watch/event stream，或未来逐条输出的长流命令。该类输出必须满足：stdout 中每一行都是独立 valid JSON object，编码为 UTF-8，记录之间仅用 newline 分隔；human diagnostics、progress、warnings 和 runtime errors 不得混入同一个 stdout 数据流。

有限命令仍使用 `--json` 的 `{data, meta?}` 成功 envelope 或 `{error:{code,message,exit_code}}` runtime error envelope。JSONL/NDJSON 不替代有限命令 envelope，也不能成为未设计的全局 `--jsonl` 快捷方式。若某个命令支持 `--out -` JSONL stream，则它不得与 `--json` 共享 stdout；需要结构化错误时，必须在命令级定义 stream error policy，并用 line-by-line JSON、stdout/stderr purity 和退出码测试覆盖。

当前公开错误 code：

| `error.code` | Exit code | 含义 |
|---|---:|---|
| `generic_error` | 1 | 未分类通用错误。 |
| `invalid_input` | 2 | 参数已通过 clap 解析，但业务输入、值域或 validation 无效。 |
| `not_found` | 3 | board、task、label、step、run 等对象未找到。 |
| `invalid_transition` | 4 | 状态机拒绝该转换，或 required execution plan / steps 未满足。 |
| `claim_conflict` | 5 | claim/heartbeat/finish token 或并发 claim 冲突。 |
| `dependency_blocked` | 6 | 依赖未完成导致任务不能进入 ready/running。 |
| `sqlite_busy` | 7 | SQLite busy/locked 或维护/runtime lock 阻塞。 |
| `integrity_check_failed` | 8 | doctor/import/maintenance 发现 integrity 或 consistency hard failure。 |
| `storage_error` | 1 | 其它存储错误；不保证可按 SQLite lock/integrity 自动恢复。 |

### 1.4 Shell completions

```bash
kanban completions <shell>
kanban __complete <kind> [prefix]
```

`kanban completions <shell>` writes a completion script to stdout. Supported
shells:

```text
bash | zsh | fish | powershell | elvish
```

Static command and option completion is generated for all supported shells.
Bash and zsh scripts additionally include dynamic hooks that call the hidden
internal `kanban __complete` helper for DB-backed candidates:

- task refs for task, comment, event, run, and dependency commands;
- board slugs for `--board` and board identity arguments;
- status values for `--status`;
- comment kind values for `comment add --kind` (`note`, `decision`, `signal`).

`kanban __complete` is an internal newline-delimited helper for shell scripts
and tests. It accepts:

```text
task-ref | dependency-task-ref | board | status | comment-kind
```

The helper must be quiet for completion use: missing DB files, uninitialized
DBs, missing board config, or read/query failures return success with no
candidates and no stderr. Static completion generation itself does not open or
create the SQLite database.

### 1.5 Codex hooks

```bash
kanban hook codex install [--handler-command <command-prefix>] [--timeout 30] [--record-signals] [--json]
kanban hook codex status [--json]
kanban hook codex uninstall [--json]
kanban hook codex handle failure [--record-signals]
kanban hook codex handle task-create
```

`kanban hook codex` manages a Codex lifecycle hook for kanban-aware agent
feedback. Hooks are installed at the Codex user config path:
`$CODEX_HOME/hooks.json`, or `~/.codex/hooks.json` when `CODEX_HOME` is not set.
There is no project-scope install mode, because kanban is intended to provide
the same CLI-aware behavior across workspaces.

Hook prompt text is read from the user kanban config path:
`$XDG_CONFIG_HOME/kanban/codex-hooks.json`, normally `~/.config/kanban/codex-hooks.json`.
`install` creates this file with Chinese default prompts when it is missing, and
never overwrites an existing file. If the prompt file is missing, malformed, has
an unsupported `version`, or points a binding at a missing prompt alias, the
handler falls back to the embedded Chinese defaults instead of failing the Codex
hook.

`install` adds two managed `PostToolUse` command hooks under matcher `^Bash$`:
one for failed `kanban ...` command traces and one for successful
`kanban task create ...` follow-up advice. The managed command prefix defaults
to `kanban hook codex handle`; the installed commands are:

```bash
kanban hook codex handle failure --installed-by kanban-hook-codex [--record-signals]
kanban hook codex handle task-create --installed-by kanban-hook-codex
```

`uninstall` removes only hooks with the hidden marker
`--installed-by kanban-hook-codex` and preserves unrelated user hooks. Re-running
`install` is idempotent: it replaces the previous managed hooks before writing
the new ones.

`handle failure` and `handle task-create` are internal hook commands. They read
Codex hook JSON from stdin and emit either no output or a raw Codex hook
response object such as:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"systemMessage":"检测到 kanban CLI 命令失败。\n\n命令：kanban task list --bad-flag\n退出码：2\n\n继续调整。调整成功后，视情况 记录必要的后续工作。"}
```

The `handle` subcommands deliberately do not use the normal `{ "data": ... }`
JSON envelope, because Codex consumes hook stdout directly. The public
management commands `install`, `status`, and `uninstall` do use the normal
`--json` envelope.

Prompt config schema:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "version": 1,
  "codex_hooks": {
    "bindings": {
      "failure": "failure.zh-default",
      "task_create": "task_create.zh-default"
    },
    "prompts": {
      "failure.zh-default": "检测到 kanban CLI 命令失败。\n\n命令：{{command}}\n退出码：{{exit_code}}\n\n继续调整。调整成功后，视情况 记录必要的后续工作。",
      "task_create.zh-default": "检测到 kanban task 创建成功。\n\n命令：{{command}}\n任务：{{task_ref}}\n\n请考虑为该 task 执行 label/signal follow-up；需要标签时先运行 `kanban label suggest {{task_ref}} --json`。不要自动写 label ontology，除非已有完整 suggestion snapshot 和明确 decision payload。"
    }
  }
}
```

Supported placeholders are deliberately small:

- `failure`: `{{command}}`, `{{exit_code}}`;
- `task_create`: `{{command}}`, `{{task_ref}}`.

`stderr` and `stdout` are not prompt placeholders. For `handle failure
--record-signals`, they remain bounded internal evidence in the recorded generic
signal.

V1 behavior:

- non-`Bash` tools and Bash commands that do not invoke `kanban` are no-op;
- `handle failure` only reports failed `kanban ...` commands with a prompt
  rendered from `codex-hooks.json` or the embedded Chinese default;
- `handle failure --record-signals` also records a generic signal with
  `kind="agent_cli_failure"`, `source="kanban-hook-codex"`, and bounded command
  evidence;
- `handle task-create` only reports successful `kanban task create ...` commands
  with a label/signal follow-up prompt rendered from `codex-hooks.json` or the
  embedded Chinese default;
- the hook never silently starts a Codex native subagent and never writes label
  ontology automatically. It only injects advice; the active Codex session must
  decide whether to spawn a native agent or record ontology observations.

---

## 2. Exit Codes

| Code | 含义 |
|---:|---|
| 0 | 成功。 |
| 1 | 通用错误或未分类 storage error。 |
| 2 | clap 参数错误，或运行期 validation / invalid input。 |
| 3 | 未找到对象。 |
| 4 | 非法状态转换、required execution plan 或 required steps 未满足。 |
| 5 | claim/token/concurrent claim 冲突。 |
| 6 | dependency blocked。 |
| 7 | SQLite busy/locked 或 maintenance/runtime lock。 |
| 8 | integrity check failed 或 consistency hard failure。 |

---

## 3. Init

### 3.1 `kanban init`

初始化本地 DB、默认 board、默认 columns。该命令是幂等的；重复执行只会应用缺失 migration 并确保默认数据存在，不会重置或覆盖已有任务数据。`--force` 是兼容旧脚本的 no-op，不改变 `init` 行为。

```bash
kanban init
kanban init --db .kb/kb.db
kanban init --force
```

`--force` 是 deprecated compatibility no-op：保留用于兼容旧脚本，不改变 `init` 行为，不执行 reset/overwrite，也不会绕过 migration 或 schema 校验。

输出：

```text
Initialized Kanban database at ~/.local/share/kb/kb.db
Default board: default
```

JSON：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "db_path": "/home/alice/.local/share/kb/kb.db",
    "board": "default"
  }
}
```

---

## 4. Board Commands

### 4.1 List boards

```bash
kanban board list [--include-archived]
```

### 4.2 Create board

```bash
kanban board create <slug> --name <name> [--description <text>]
```

Example：

```bash
kanban board create agent-work --name "Agent Work"
```

### 4.3 Show board

```bash
kanban board show <slug>
```

### 4.4 Use board

```bash
kanban board use <slug-or-id>
```

Writes:

```toml
board = "agent-work"
```

to `.kb/config.toml` in the current directory.

### 4.5 Current board

```bash
kanban board current
```

Shows the resolved active board after applying `--board`, `KB_BOARD`, project config, and fallback precedence.
Board resolution is independent from DB path resolution: `--db` / `KANBAN_DB` / `KB_DB` choose which SQLite database to open, while `--board` / `KB_BOARD` / `.kb/config.toml` `board` choose the board inside that database.

### 4.6 Archive board

```bash
kanban board archive <slug>
```

Archived boards are hidden from `kanban board list` unless `--include-archived` is passed. Ordinary task writes against archived boards are rejected. Audit history remains readable through task/event/run/comment history commands when the task or board can be resolved explicitly. Archiving a board with active `running` work is rejected; finish, block, or reclaim that work first.

---

## 5. Task Commands

### 5.1 Create task

```bash
kanban task create <title> [OPTIONS]
```

Options：

| Option | 说明 |
|---|---|
| `--description <text>` | Markdown 描述。 |
| `--description-file <PATH|->` | 从文件或 stdin (`-`) 读取 Markdown 描述；与 `--description` 互斥。推荐用于多行或包含 `$`、反引号、JSON 等 shell-sensitive 文本。 |
| `--status <status>` | 显式初始状态：triage/todo/scheduled/ready。 |
| `--assignee <name>` | assignee/worker profile。 |
| `--priority <int>` | Priority level `0..3`: `0` = P0 incident/blocker/must-handle-immediately, `1` = P1 near-term focus, `2` = P2 important follow-up, `3` = P3 ordinary backlog/low/default. Invalid values are rejected. |
| `--scheduled-at <epoch_ms>` | 计划时间，Unix epoch milliseconds。 |
| `--due-at <epoch_ms>` | 截止时间，Unix epoch milliseconds。 |
| `--max-retries <n>` | worker 失败或 reclaim 后最多重试次数。 |
| `--label <name>` | 创建时附加已存在 label，可重复；缺失的 board label 会拒绝整个 create。 |
| `--metadata <json>` | 扩展 JSON。 |
| `--metadata-file <PATH|->` | 从文件或 stdin (`-`) 读取扩展 JSON；与 `--metadata` 互斥。推荐用于避免 JSON shell quoting 问题。 |

Priority 只表达相对重要性和排序，不表达可 claim 状态。`ready` 才表示任务已被显式放入可执行队列；普通 ready 任务通常仍应是 P1/P2/P3，不能为了表示“下一批可做”全部标成 P0。P0 只用于 incident、当前目标 blocker 或必须立即处理的任务；若 P0 task 仍缺规格、排期未到或依赖未完成，它仍保持 `triage` / `scheduled` / `todo`，不能被 claim。

Examples：

```bash
kanban task create "修复 claim 队列阻断回归" --priority 0
kanban task create "实现状态机" --priority 1
kanban task create "补充文档示例" --priority 2
kanban task create "明早检查报告" --scheduled-at 1780640400000
kanban task create "修复 API 回归" --label backend --label p1
```

`--label` 只绑定当前 board 中已存在的 label identity。名称会先 trim；空白名称会被拒绝。
任一 label 缺失时，整个 create 返回 invalid input，且不会写入 `tasks`、`labels`、
`task_labels` 或 `task_events`。需要新 vocabulary identity 时，先显式运行
`kanban label create`，或使用 `kanban label add --create-missing` 这类明确的 identity
创建入口；task create 本身没有 create-missing 模式。

Human output：

```text
agent-work#12 [ready] P1 实现状态机 · plan: planned · steps: 0/0
```

JSON output：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "id": "t_01HX...",
    "board_id": "b_01HX...",
    "board_slug": "agent-work",
    "ref": "agent-work#12",
    "seq": 12,
    "status": "ready",
    "title": "实现状态机",
    "labels": []
  }
}
```

### 5.2 List tasks

```bash
kanban task list [OPTIONS]
```

Options：

| Option | 说明 |
|---|---|
| `--status <status>` | 按状态过滤，可重复。 |
| `--assignee <name>` | 按 assignee。 |
| `--label <name>` | 按 label 名称或 id 过滤，可重复；多个 label 使用 AND 语义。 |
| `--search <query>` | title/description 模糊搜索；task ref 形状按精确匹配处理。 |
| `--include-archived` | 包含 archived。 |
| `--limit <n>` | 限制数量。 |
| `--offset <n>` | 分页偏移。 |
| `--sort <field>` | `seq` / `title` / `status` / `position` / `priority` / `assignee` / `scheduled_at` / `due_at` / `created_at` / `updated_at`。降序可用 `<field>_desc`，也兼容 API 风格 `-<field>`。`priority` sorts P0 -> P3; `priority_desc` / `-priority` sorts P3 -> P0. |
| `--plan-needed` | 只列出 execution plan 仍为 `unplanned` 的 active tasks。 |
| `--has-steps` | 只列出至少有一个 step 的 tasks。 |
| `--incomplete-required-steps` | 只列出存在未完成 required step 的 tasks。 |
| `--plan-filter <filter>` | 可重复：`plan-needed` / `has-steps` / `incomplete-required-steps`。 |

Priority sort does not promote work into `ready`; it only orders tasks within the selected result set.

`--search` 对 task ref 形状使用精确匹配而不是文本 contains 匹配：
纯数字 `12`、`#12` 匹配 active board 内的 seq；`board#12` / `board/#12`
只在该 board 与当前列表请求的 board 相同时匹配；`t_...` 只匹配当前列表请求 board
内的 task id。其他文本仍执行 title/description 模糊搜索。

Examples：

```bash
kanban task list
kanban task list --status ready --status running
kanban task list --label backend --label p1
kanban task list --assignee agent-default --json
kanban task list --plan-needed
kanban task list --plan-filter incomplete-required-steps
```

### 5.3 Show task

```bash
kanban task show <task_ref>
kanban task show <task_ref> --details
```

默认人类可读输出仍是紧凑的单行 task 摘要；默认摘要面向扫描，保留可复制 ref、status、priority、title、labels，以及必要 plan/step 信号，不默认展示内部 `t_...` id：

```text
agent-work#12 [ready] P1 实现状态机 · plan: planned · steps: 0/0
```

`--details` 改变人类可读输出，按 `Task`、`Description`、`Plan`、`Schedule`、`Timestamps`、`Execution`、`Result`、`Metadata` 分组显示易读字段列表。可用时包含 task ref/id/status/title、完整多行 description、assignee、priority、labels、scheduled_at、due_at、created_at、updated_at、execution plan state、required/optional step counts、claim/run、result、metadata 以及其他 task snapshot 字段。
如果该 task 有 label ontology signals，details 输出还会追加紧凑的
`ontology_summary`，列出 signal/status/degraded/stale/action counts、aging 时间和
少量 sample signal ids。

`task show <task_ref> --json` 默认只返回 `{"data": TaskRecord}`。带 `--details`
时，`data` 仍是相同的 `TaskRecord`，但 envelope 会包含
`meta.details.ontology_summary`；没有 ontology signals 时该字段为 `null`。该 summary
只读，不改变 task、labels 或 ontology signal 状态。需要完整 review queue 时继续使用
`label ontology list/show/review`。

`task_ref` 支持：

- `t_...`：全局 task id，忽略 active board。
- `12`：当前 active board 内的 seq。
- `#12`：当前 active board 内的 seq；shell 中需要引号，例如 `'#12'`。
- `agent-work#12`：显式 board slug + seq。
- `agent-work/#12`：兼容 alias/#seq 形式。
- `b_01HX...#12`：显式 board id + seq。

裸 `12` / `#12` 依赖 active board；显式 `board#seq` 和 `t_...` 可跨 active board 使用。跨 board dependency 在当前版本中会被拒绝。

### 5.4 Update task fields

```bash
kanban task update <task_ref> [OPTIONS]
```

允许更新：

- title
- description
- assignee
- priority
- scheduled_at
- due_at
- max_retries
- metadata

不允许通过 update 修改 status；status 必须通过 transition command。允许字段仍由
shared service path 处理，因此修改 description、scheduled_at 等会影响 spec 或
schedule 的字段后，服务会根据 spec、schedule 和当前 dependencies 重新计算
active task 的目标状态并写入对应事件。Dependency edge 通过 `kanban dep`
命令修改；`max_retries` 只更新 retry policy，不是 status recompute 触发器。

Examples：

```bash
kanban task update 12 --priority 1
kanban task update t_01HX --description "新的规格"
kanban task update t_01HX --description-file - <<'EOF'
新的多行规格，保留 $VAR、$(command)、反引号和 JSON 字面量。
EOF
kanban task update t_01HX --max-retries 2
kanban task update t_01HX --clear-max-retries
```

---

## 6. Transition Commands

### 6.1 Promote

```bash
kanban task promote <task_ref>
```

手动尝试 `todo/scheduled -> ready`。

### 6.2 Start / Claim

```bash
kanban task start <task_ref> [OPTIONS]
kanban task claim <task_ref> [OPTIONS]
```

`start` 是 `claim` 的人类友好 alias。

Options：

| Option | 说明 |
|---|---|
| `--ttl-ms <ms>` | claim TTL。默认 300000。 |

Output：

```text
Claimed t_01HX... token=ct_01HX...
```

JSON 返回 canonical claim snapshot：`data.task` 是闭合的 `ApiTask`，`data.run`
是闭合的 `ApiRun`，token 只允许出现在顶层 `data.claim_token`。下面仅节选 identity
与状态字段；实际对象还包含各自 schema 声明的其余字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "status": "running",
      "current_run_id": "r_01HX..."
    },
    "run": {
      "id": "r_01HX...",
      "task_id": "t_01HX...",
      "status": "running"
    },
    "claim_token": "ct_01HX...",
    "claim_expires_at": 1717520000000
  }
}
```

### 6.3 Heartbeat

```bash
kanban task heartbeat <task_ref> --claim-token <token>
```

Options：

| Option | 说明 |
|---|---|
| `--ttl-ms <ms>` | 延长 TTL。 |

显式 heartbeat API 保持兼容。除此之外，`running` task 的有效 task-scoped activity event 也会隐式刷新 lease，可作为 liveness signal；该隐式刷新不会再写 `task.heartbeat` event。board-level event 或没有 `task_id` 的 event 不触发续租。

### 6.4 Done / Complete

```bash
kanban task done <task_ref> --claim-token <token>
kanban task complete <task_ref> --claim-token <token>
```

Options：

| Option | 说明 |
|---|---|
| `--claim-token <token>` | active claim token。 |
| `--force` | 强制完成 running task；仅本地人工修复使用。 |

### 6.5 Submit Review

```bash
kanban task review <task_ref> --claim-token <token>
```

使 task 从 `running` 到 `review`。

### 6.6 Block

```bash
kanban task block <task_ref> [<reason>|--reason-file <PATH|->]
```

Options：

| Option | 说明 |
|---|---|
| `--claim-token <token>` | running task block 时需要。 |
| `--force` | 强制 block。 |
| `--reason-file <PATH|->` | 从文件或 stdin (`-`) 读取 block reason；与 positional `<reason>` 互斥。 |

### 6.7 Unblock

```bash
kanban task unblock <task_ref>
```

不会盲目进入 ready，而是根据 spec、schedule、dependencies 重新计算目标状态。

### 6.8 Reopen

```bash
kanban task reopen <task_ref> [--reason <text>|--reason-file <PATH|->]
```

只允许 reopen `done` task，reason 必填且不能为空，可用 `--reason-file <PATH|->`
从文件或 stdin 读取；它与 inline `--reason` 互斥。Reopen 会清空
`completed_at`，保留 `result_summary` / natural JSON `result`（持久层仍存于 `result_json`），并按 spec、schedule、
dependency 和 execution plan readiness 重新计算目标状态。

如果被 reopen 的 task 是其他 task 的 dependency parent，直接 child 中仅 `triage|todo|scheduled|ready` 会重新计算；`running|blocked|review|done|archived` 不隐式改写。

### 6.9 Reclaim

```bash
kanban task reclaim --expired
kanban task reclaim
```

当前 CLI reclaim 处理 active board 内 expired claims；裸 `kanban task reclaim` 与 `kanban task reclaim --expired` 等价。
JSON 输出固定为 `{"data":{"reclaimed":<u64>}}`，且拒绝未声明字段。

### 6.10 Archive

```bash
kanban task archive <task_ref>
```

Options：

| Option | 说明 |
|---|---|
| `--force` | 允许 archive running task，并关闭 active run。 |

---

### 6.10 Step / Execution Plan

```bash
kanban task step list <task_ref>
kanban task step add <task_ref> <title> [--body <text>|--body-file <PATH|->] [--link-task <task_ref>] [--position <n>] [--required|--optional]
kanban task step update <task_ref> <step_ref> [--title <text>] [--body <text>|--body-file <PATH|->|--clear-body] [--link-task <task_ref>|--unlink-task] [--position <n>] [--required|--optional]
kanban task step done <task_ref> <step_ref> [--note <text>|--note-file <PATH|->]
kanban task step skip <task_ref> <step_ref> [--reason <text>|--reason-file <PATH|->]
kanban task step reopen <task_ref> <step_ref> [--reason <text>|--reason-file <PATH|->]
kanban task step remove <task_ref> <step_ref>
kanban task step not-required <task_ref> [--reason <text>|--reason-file <PATH|->]
```

Step 是 execution plan 的一等结构化项目。它可以是纯文本步骤，也可以通过
`--link-task` 引用同一 board 内的普通 task 作为上下文。链接 task 不等于 dependency，
不会让 linked task 的状态自动完成 step。Step 自己的状态是 `todo`、`done` 或
`skipped`。

`step_ref` 支持 step id，也支持父任务列表里的 `S<n>` 序号。`add` 默认创建
required step；`--required` / `--optional` 互斥。Canonical human form is the bare
flag form, but the CLI also accepts bounded agent-generated values for this
specific flag: `--required true`, `--required=false`, and the matching
`--required=true` / `--required false` forms. Only literal `true` / `false` are
consumed as boolean values; ordinary positional text after `--required` remains
positional, and any other extra value remains a parser error. `--body-file
<PATH|->` 从文件或 stdin 读取长正文，与 `--body` 互斥；`update --clear-body`
也与 `--body-file` 互斥。`update` 只有在显式传 `--required` 或 `--optional`
时才改变 required flag。`done`、`skip` 和 `reopen` 必须记录说明文本。
`--note-file <PATH|->` 和 `--reason-file <PATH|->` 从文件或 stdin 读取长
note/reason，分别与 inline `--note` / `--reason` 互斥。

Human list 输出示例：

```text
Execution plan: planned
Required steps: 1/2 done-or-skipped
Optional steps: 1

S1 st_01HX... [done] required pos=1024 Write tests
S2 st_01HY... [todo] required pos=2048 link=default#13 Verify desktop UI
S3 st_01HZ... [todo] optional pos=3072 Release notes
```

`task step not-required` 只在没有 steps 时可用；它记录 reason 并解除 ready/claim 的
execution-plan gate。已有 step 的 task 不能标记为 `not_required`。

---

## 7. Dependency Commands

```bash
kanban dep add <parent_ref> <child_ref>
kanban dep remove <parent_ref> <child_ref>
kanban dep list <task_ref>
```

`--json` 输出使用 hydrated dependency DTO。`dep list --json` 返回以查询 task 为中心的 snapshot：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_child",
      "board_id": "b_default",
      "board_slug": "default",
      "ref": "default#2",
      "title": "child",
      "status": "todo"
    },
    "parents": [
      {
        "id": "t_parent",
        "board_id": "b_default",
        "board_slug": "default",
        "ref": "default#1",
        "title": "parent",
        "status": "done"
      }
    ],
    "children": [],
    "edges": [
      {
        "parent": {
          "id": "t_parent",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#1",
          "title": "parent",
          "status": "done"
        },
        "child": {
          "id": "t_child",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#2",
          "title": "child",
          "status": "todo"
        }
      }
    ]
  }
}
```

`dep add --json` 和 `dep remove --json` 返回：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "edge": { "parent": {}, "child": {} },
    "dependencies": { "task": {}, "parents": [], "children": [], "edges": [] }
  }
}
```

常用 jq：

```bash
kanban dep list default#2 --json | jq -r '.data.edges[] | "\(.parent.ref) -> \(.child.ref)"'
```

Human output for add/remove is Chinese-first:

```text
已添加依赖：default#1 -> default#2
已移除依赖：default#1 -> default#2
```

添加 dependency 后：

- 如果 child 当前是 `ready` 且 parent 未完成（不是 `done` 或 `archived`），child 降级为 `todo`。
- parent 完成、归档或 dependency 移除后，child 保持 `todo`；需要 `kanban task promote <task_ref>` 才显式进入 `ready`。归档 parent 不会删除 dependency edge。
- parent 从 `done` reopen 后，直接 child 中仅 `triage|todo|scheduled|ready` 会按 readiness 重算；`running|blocked|review|done|archived` 不隐式改写。
- 重复添加同一 parent/child edge 是 idempotent no-op：不追加新的
  `dependency.added` event，也不再次触发 child 状态重算。
- 如果产生环，返回 exit code 6 或 invalid input。
- 当前版本拒绝跨 board dependency，即使 parent/child 通过全局 `t_...` 或显式 `board#seq` 解析成功。

`task list/show --json` 返回 derived dependency fields：`dependency_blocked`
和 `unfinished_parent_count`。未完成 parent 指状态不是 `done` 或 `archived` 的 parent；这些字段用于区分仍被未完成 parent 阻塞的 `todo`
与已解除依赖但尚未人工 promote 的 `todo`。

---

## 8. 标签命令

```bash
kanban label list
kanban label create <name> [--color <color>]
kanban label delete <label> [--force] [--json]
kanban label bootstrap <task_ref> <label> [--description <text>] [--applies-when <text>]... [--excludes-when <text>]... [--positive-example <text>]... [--negative-example <text>]... [--verify] [--min-verify-score 0.50] [--vector-config <toml>] [--json]
kanban label add [--create-missing] <task_ref> <label>...
kanban label remove <task_ref> <label>
kanban label semantics list [--json]
kanban label semantics show <label> [--json]
kanban label semantics upsert <label> [--expected-semantics-hash <hash>] [--replace] [--reason <text>|--reason-file <PATH|->] [--source-signal <signal_id>]... [--description <text>] [--applies-when <text>]... [--excludes-when <text>]... [--positive-example <text>]... [--negative-example <text>]... [--remove-applies-when <text>]... [--remove-excludes-when <text>]... [--remove-positive-example <text>]... [--remove-negative-example <text>]... [--json]
kanban label semantics delete <label> --expected-semantics-hash <hash> [--reason <text>|--reason-file <PATH|->] [--json]
kanban label atoms list [--json]
kanban label atom explain <atom-id-or-content-hash> [--json]
kanban label atom-index status [--vector-config <toml>] [--json]
kanban label atom-index rebuild [--vector-config <toml>] [--json]
kanban label atom-index query <text> [--polarity positive|negative] [--limit 24] [--vector-config <toml>] [--json]

`label atom-index status`、`rebuild` 和 `query` 复用 vector TOML 解析规则：显式 `--vector-config`/`--config` 优先，其次是最近项目 `.kb/config.toml`，最后是全局 config。helper argv 只在显式传入 `--vector-config` 时附带该参数；省略时由 helper 按默认配置解析。
kanban label suggest <task_ref> [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label propose <task_ref> [--proposal-json <path>] [--source-signal <signal_id>]... [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label proposals list [--task <task_ref>] [--status proposed|accepted|rejected] [--json]
kanban label proposals show <proposal_id> [--json]
kanban label proposals accept <proposal_id> [--reason <text>|--reason-file <PATH|->] [--source-signal <signal_id>]... [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label proposals reject <proposal_id> [--reason <text>|--reason-file <PATH|->] [--json]
kanban label ontology record <task_ref> --input <path|-> [--suggestion-snapshot <path|-> | --capture-suggest] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label ontology list [--status open|confirmed|resolved|rejected|superseded]... [--kind false_negative|false_positive|vocabulary_gap|name_issue|boundary_issue|structure_issue]... [--task <task_ref>] [--label <label>] [--proposed-label <name>] [--include-all] [--limit 100] [--json]
kanban label ontology show <signal_id> [--json]
kanban label ontology review [--group-by label|candidate-atom|proposed-label|cluster] [--include-all] [--limit 100] [--json]
kanban label ontology quality [--sample-limit 20] [--json]
kanban label ontology confirm <signal_id>... [--reason <text>|--reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology reject <signal_id>... [--reason <text>|--reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology supersede <signal_id>... --by <signal_id> [--reason <text>|--reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology resolve <signal_id>... --no-change [--reason <text>|--reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology apply atom <signal_id>... --label <label> --kind applies-when|positive-example|excludes-when|negative-example [--text <text>|--text-file <PATH|->] [--reason <text>|--reason-file <PATH|->] [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology revert <action_id> [--reason <text>|--reason-file <PATH|->] [--expected-current-hash <hash>] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --status passed|failed|partial [--reason <text>|--reason-file <PATH|->] --input <PATH|-> [signal_id]... [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --trusted --status passed|failed|partial [--reason <text>|--reason-file <PATH|->] [signal_id]... [--positive-control <TASK_REF>]... [--positive-control-waiver <REASON>|--positive-control-waiver-file <PATH|->] [--vector-config <toml>] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--actor-type user|agent] [--agent-type <type>] [--json]
```

Label semantics/proposal/ontology 命令中的 `--reason-file <PATH|->`、
`--retarget-reason-file <PATH|->`、`--text-file <PATH|->` 和
`--positive-control-waiver-file <PATH|->` 从文件或 stdin 读取对应长文本，并与同名
inline 参数互斥。`label atom-index query <text>` 的 `<text>` 是短查询标量，不提供
file 输入；需要持久 ontology evidence 时使用 `label ontology record --input <path|->`
或 `label ontology validate --input <PATH|->`。

`label create` 创建当前 board 作用域内的 label；如果同一 board 已存在同名
label，返回已有 label。`label add` 接受 task ref 和一个或多个 label 名称；默认
只绑定 task 所属 board 上已经存在的 canonical label。缺失 label 会返回
invalid input，并提示先用 `label create`、`label bootstrap`、proposal/adoption
路径创建，或在明确接受只创建 canonical identity 的情况下传 `--create-missing`。
`--create-missing` 只创建 `labels` identity 并绑定 task，不生成 `label_semantics`
或 `label_atoms`；JSON 输出改为 `{ "task": <TaskRecord>, "created_labels": [...] }`。
`label remove` 接受 task ref 和 label 名称或 id。空白 label 名称会被拒绝。

`label delete <label>` 删除当前 board 上的 canonical label identity，区别于
`label remove <task_ref> <label>` 的 task-level 解绑。Label identity CRUD 不属于
ontology ledger；create/delete 只写普通 board/task event，不写 ontology mutation
action。默认情况下，如果 label 仍绑定任何 task，会拒绝删除并报告绑定数量；显式传
`--force` 时只移除 task bindings 后删除空 label identity。若 label 仍有
`label_semantics` 或 `label_atoms`，即使传 `--force` 也会拒绝；必须先用
`label semantics delete --expected-semantics-hash <hash> --reason <text>` 清空语义。
JSON 返回 `{ "label": <LabelRecord>, "forced": bool, "removed_task_bindings": n,
"removed_semantics": false, "removed_atoms": 0 }`。删除 canonical label 不改变 task
status；被删除 label 会从 `label list`、`task show/list` 的 labels 和后续 suggest truth
中消失。

Label 变更对 task-label 关联保持幂等。只有关联实际变化时，才追加
`task.label.added` / `task.label.removed` event；该操作不改变 task status。
批量 `label add` 会先验证所有 label 名称；如果任一 label 为空白、非法或缺失且未传
`--create-missing`，不会创建 canonical label，也不会留下部分 task-label 绑定。
显式创建模式与单 label add 相同，只创建缺失的 canonical identity，并在输出中列出
本次新建的 labels。

`label bootstrap` 是一次性 new-label adoption helper：在同一 transaction 内创建
当前 task 所属 board 上缺失的 canonical label，或复用没有既有 semantics 的同名
label，写入该 label 的 `label_semantics`，同步重建 SQLite `label_atoms`，标脏派生
的 label atom vector index，并把该 label 绑定到 task。`<label>` 按名称解析；空白
名称会被拒绝。语义输入会 trim 并丢弃空白值，且必须至少提供 `description` 或一个非空
语义数组值。

Bootstrap 默认不会覆盖已有 `label_semantics`。如果同名 label 已经有 semantics，
命令会失败，并要求改用专用 semantics mutation 或 proposal/adoption 路径；重复执行
同一 task/label 只在目标 label 仍无 semantics 时保持 task-label 绑定幂等。JSON
返回 `{ "task": <TaskRecord>, "semantics": <LabelSemanticsRecord>, "verification": null|<Verification> }`。

当前 no-heavy CLI build 已把 label suggestion/proposal、bootstrap staged verification 和
label atom status/rebuild/query 接到 vector helper subprocess adapter；`kanban vector ...` 仍保留
raw chunk / label-atom 查询入口，helper 内部用 label atom 专用 command 处理
`lancedb_label_atoms`，不复用 chunk store status 伪装 label atom 状态。

传入 `--verify` 或 `--vector-config <toml>` 时，CLI 使用 pre-commit staged
verification：先在 canonical DB transaction 外读取当前 task、target label state 和
board ontology digest，并在隔离的临时 atom store 中加载当前 atoms 与 candidate atoms。
随后对来源 task 运行非 degraded `label suggest`，要求新 label 出现在
`selected_labels` 或 `candidates`，且 score 至少达到 `--min-verify-score`（默认
`0.50`）。rebuild、suggest、threshold、provider 或临时 store 失败时不会写
canonical label、semantics、atoms、task-label binding、ontology action、event 或 dirty
marker。如果 vector helper/provider 不可用会返回明确的 verification error；需要离线验收时也可改走 external attestation `--input` 路径。

验证通过后 CLI 才开启短 `BEGIN IMMEDIATE` transaction，重算 task suggest-input hash、
target label state 和 board ontology digest；任一值变化会返回 conflict 且零写入。成功
路径在一个 transaction 中写 canonical label/semantics/atoms、task binding、普通
task-label event、一个 `bootstrap_label` root ontology action 和对应 added atom
effects。Verification summary 会写入 root action change snapshot 和 CLI output；它不等同于
post-commit trusted validation。无可用 vector provider 时，验证会在写入前失败；不需要
本地 vector 验证时省略 `--verify` 和 `--vector-config`。

示例：

```bash
kanban label create backend --color blue
kanban label delete old-label --json
kanban label delete old-label --force --json
kanban label semantics delete old-label --expected-semantics-hash sem_abc123 --reason "Retire obsolete semantics before deleting identity" --json
kanban label bootstrap default#12 database --description "Database persistence work" --applies-when "touches SQLite migrations" --positive-example "new table migration" --json
kanban label bootstrap default#12 database --description "Database persistence work" --applies-when "touches SQLite migrations" --positive-example "new table migration" --vector-config .kb/vector.toml --min-verify-score 0.50 --json
kanban label add default#12 backend
kanban label create api
kanban label add default#12 backend api
kanban label add --create-missing default#12 scratch-label --json
kanban label remove t_01HX... backend
kanban label list --json
```

人类可读输出使用紧凑 label 行：

```text
backend l_01HX... color=blue
```

Task 的人类可读摘要如果存在 labels，会在末尾追加方括号标签列表：

```text
default#12 [ready] P1 修复 API 回归 [backend,p1] · plan: planned · steps: 0/0
```

`label suggest` 返回 task-level label suggestions。带内置 label atom vector store 的
构建会把 task title +
description embedding 作为 query，使用 `lancedb_label_atoms` 按残差多轮检索正向
label atoms，并用原始 query 检索负向 atoms 做 penalty / suppression。solver 在
label group 层执行 Group OMP 选择，再用选中 label 的 top positive atom vectors 做
non-negative refit；`coverage` / `residual_norm` 来自该 atom-level fitted vector，
其中 `coverage = clamp(1 - residual_norm, 0.0, 1.0)`，因此二者不是两份独立
证据；`coverage_cosine` 是原始 query 与 fitted vector 的 cosine similarity，
可作为独立补充指标。
候选 label 只有在 tentative refit 后带来足够 residual norm 降幅才会进入结果；
coverage 或 residual norm 达到停止阈值后，solver 会提前停止而不是凑满
`--max-selected-labels`。candidate group 与已选 label 语义向量过度相似时会被跳过，
以减少重复语义 label 同时出现在 selected labels；这不会合并或删除 canonical
labels。
`needs_new_label` 是兼容字段，只表示存在需要人工 review 的 label coverage
诊断；具体原因必须读取 `reason_codes`，例如 `no_selected_labels`、
`coverage_below_threshold`、`residual_above_threshold`、`unexplained_residual`，
或 degraded 相关原因。不要把 `coverage` 与 `residual_norm` 重复计票，也不要仅凭
`needs_new_label=true` 创建 vocabulary；必须结合 `reason_codes`、evidence atoms、
diagnostics 和人工语义判断。
它不会自动创建新 label，也不会写入 new-label proposal。应用建议时仍使用现有
`label add <task_ref> <label>...` / API attach 流程。

默认 no-heavy CLI 通过 vector helper adapter 运行 label vector 查询；helper/provider 不可用时命令成功返回
degraded 结果而不是失败，且 `needs_new_label=false`。`--vector-config`
使用与 `kanban vector configure/status` 相同的 TOML 解析规则，并把解析出的 embedding model 传给 helper 查询。`LabelAtomHit.distance`
保留 LanceDB `_distance` 的原始语义；suggestion / proposal 的 score 只根据返回
atom vector 与当前 query/residual 在本地计算 cosine similarity，不从 distance 推导。

JSON 输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_01HX...",
    "board_id": "b_01HX...",
    "selected_labels": [],
    "candidates": [],
    "coverage": 0.0,
    "coverage_cosine": 0.0,
    "residual_norm": 1.0,
    "needs_new_label": false,
    "reason_codes": ["degraded_result", "vector_store_disabled"],
    "degraded": true,
    "diagnostics": ["vector_store_disabled"]
  }
}
```

Human output 简洁列出建议 label、score、weight、already_applied；degraded 时追加
diagnostics 行。

`--limit` 只控制最终输出中 `selected_labels` / `candidates` 的最大条数，不会收窄
solver 内部搜索能力。内部能力由 `--candidate-limit`、`--atom-limit` 和
`--max-selected-labels` 分别控制：候选 label group 数、每轮 atom vector 检索上限、
以及最多进入 non-negative refit 的 label 数。所有 limit 参数都必须是
`1..=1000`；`--min-score` 必须在 `0..=1`。

Label ontology 的长期 regression corpus 目前是本地测试基础设施，不是一个会写生产
DB 的 CLI mutation 流程。修改 label solver、semantics/atom 生成、trusted validation
或重要 label ontology 时，可以运行：

```bash
just test-p kanban-sqlite label_ontology_longitudinal_regression
```

该测试在临时 SQLite DB 中建立固定 important labels、known positive tasks 和
negative-control tasks，重建内存 label atom index，保存 baseline `label suggest`
结果，再模拟一次过宽 atom 变更并比较 selected labels、score 和 evidence atoms。它会
断言正常 corpus run 不修改 `labels`、`task_labels`、`label_semantics`、
`label_atoms` 或 ontology ledger rows；真实项目 corpus 应在积累稳定任务后逐步扩展，
但不应成为每个日常 task label 绑定的默认必跑步骤。

`label semantics` 管理当前 board 上已有 label 的语义字典。`<label>` 接受 label
name 或 `l_...` id。`upsert` 默认是 patch：`--description` 只在提供非空值时覆盖当前
description，数组参数会追加到对应集合，`--remove-*` 只删除匹配的既有文本；未提供的
字段不会被解释为清空。传 `--replace` 时才执行完整替换，此时未提供的数组会成为空
数组，并且不能同时传 `--remove-*`。`--expected-semantics-hash <hash>` 是
compare-and-swap guard：hash 不等于当前 semantics hash 时返回 conflict 且不写入。
`--reason` 和 `--source-signal` 会进入 `update_semantics` ontology action；即使没有
source signal，constructive semantics mutation 也会在同一 transaction 写入 before/after
hash、change snapshot 和 actor provenance。`upsert` 会写入 `label_semantics` 并同步重建
该 label 的 `label_atoms`，随后标脏派生的 label atom vector index。数组参数可重复；空白值会被
trim 后丢弃。生成 atoms 时，有 description 的 label 会生成一个 canonical
`description` atom：`label: {name}\ndescription: {description}`；没有 description 时
才使用 `name` fallback atom。atom text 会进一步规范化 whitespace：每个非空行内部
collapse，canonical 行分隔保留。同一 label 下相同
`polarity + kind + normalized_text` 的 atom 会去重并保留首次 ordinal，`id` /
`content_hash` 不包含 ordinal，因此只调整数组顺序不会改变同一文本 atom identity。
`delete` 是 CAS-protected semantics clear：必须传
`--expected-semantics-hash <hash>` 和非空 `--reason <text>`。它删除该 label 的
semantics 与 SQLite atoms，但不删除 canonical label identity 或 task-label 绑定；同一
transaction 会写一个 `update_semantics` root ontology action，after snapshot 为空，
并为实际 removed atoms 写 `removed` atom effects，随后标脏 label atom index。Hash
mismatch 时 canonical、action、effects 和 dirty state 全不变。成功返回
`{ "data": { "deleted": true } }`。需要在清空后删除 label identity 时，先 clear
semantics，再执行 `label delete`。

`label atoms list` 读取 SQLite `label_atoms` materialized projection。这些 atoms 来自
`label semantics upsert`、`label bootstrap`、`label ontology apply atom` 或接受 label
proposal 后生成的 semantics；它们是 `lancedb_label_atoms` 派生索引的输入，不是派生索引本身。

`label atom explain <atom-id-or-content-hash>` 是 `label atoms explain` 的单数别名，
按当前 board 的 atom id 或稳定 `content_hash` 解析现有 atom，并返回当前 atom、
canonical semantics、provenance actions、supporting signals/source tasks 和
validation history。当前 atom 存在但没有 ontology provenance action 引用其 id 或
content hash 时命令成功返回 `legacy_untracked=true` 和 `legacy_reason`；未知 id/hash
返回 not found。JSON 输出是 `LabelAtomExplainRecord`，包含 `query`、`atom`、
`current_semantics`、`provenance_actions`、`supporting_signals`、
`validation_history`、`legacy_untracked` 和 `legacy_reason`。由于 content hash 不含
ordinal，semantics rebuild 后同语义 atom 的 id 改变时仍可用 content hash 解释历史。

`label atom-index status` 返回 label atom vector index 的状态。未配置 provider 或 helper
不可用时仍成功返回 disabled/degraded 状态。JSON 保留兼容字段 `message`，并返回结构化
`diagnostics: string[]`、`dirty: boolean | null`、`board_dirty: boolean | null`；调用方应使用
结构化字段判断 dirty/error，而不要解析 `message` 文案。`status` 通过 helper 的
`label-atoms-status` command 读取 `LANCEDB_LABEL_ATOMS_STORE` 与 `label_atom_index_boards` 语义；
`query` 通过 helper adapter 查询 label atom vector index，`--polarity` 只接受 `positive` 或
`negative`，human 输出和 JSON hit 都把 LanceDB `_distance` 暴露为 `distance`。`rebuild` 通过
helper 的 `rebuild-label-atoms` command 重建 label atom 派生索引；helper/provider 不可用时返回显式
error，不修改 SQLite canonical label truth，也不标记 chunk store success。

`kanban vector query-label-atoms` 是公开 raw helper 查询入口，支持 text 查询和 raw vector 查询。
输入必须且只能选择一种：positional `<text>`、`--text-file <PATH|->`、`--vector-json <JSON>` 或
`--vector-json-file <PATH|->`。`-` 表示从 stdin 读取。示例：
`kanban vector query-label-atoms --text-file query.txt [--polarity positive|negative] [--limit N] [--embedding-model MODEL] [--vector-config <toml>]`，或
`kanban vector query-label-atoms --vector-json-file vector.json [--include-vector] [--embedding-model MODEL] [--polarity positive|negative] [--limit N]`。
`--include-vector` 只对 helper 支持的 raw vector/vector hit 输出有意义。

`label propose` 是独立的新 label semantics 提案流程，不复用或改变 `label suggest`。
它先读取当前 task-level label suggestions 的 `coverage` / `coverage_cosine` / `residual_norm` /
top1 existing label。没有 `--proposal-json` 时默认 provider 不可用，命令成功返回
degraded attempt，不创建 canonical label、`label_semantics`、`label_atoms` 或
`task_labels`。日常 label suggestion 不依赖该 proposal provider。
`--limit` 只截断 proposal attempt 中复用的 suggestion 输出；`--candidate-limit`、
`--atom-limit`、`--max-selected-labels`、`--min-score` 会在 proposal 持久化前调节底层
label suggestion solver，用于计算 coverage、coverage_cosine、residual_norm 和 top1 existing label。
`--vector-config` 使用与 `label suggest` 相同的 TOML 解析规则。默认 no-heavy CLI
通过 vector helper adapter 运行 residual validation；未配置或 helper/provider 不可用时保持
degraded fallback，不写入普通 label 或 task-label 关联。

Provider boundary：CLI 当前只使用 disabled provider 或 `--proposal-json` 显式传入的
本地/offline candidate。真实 LLM provider 不属于 `kanban-sqlite`；未来若接入本机
AI/runtime，应在 CLI/local runtime 或独立 AI crate 中实现 `LabelProposalProvider`
adapter，再把 candidate 交给 SQLite service 做 deterministic validation 和 persistence。

`--proposal-json` 提供本地/offline provider 输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "database",
  "description": "Database persistence work",
  "applies_when": ["touches SQLite migrations"],
  "excludes_when": ["UI-only polish"],
  "positive_examples": ["new table migration"],
  "negative_examples": ["CSS-only tweak"]
}
```

数组字段缺省时按空数组处理。`name` 不能为空，且 description 或任一语义数组至少
需要提供一个非空值。只有当前启发式 coverage 不足时才持久化 proposal。与现有
label 发生 normalized-name 冲突的候选会写成 `rejected` proposal，并在 diagnostics
中返回 `near_duplicate_label_conflict`；该 normalized-name 检查忽略大小写、空白
和标点，是 deterministic near-duplicate heuristic。
coverage 不足的候选还会执行残差 top1+margin 校验：候选语义的 residual score
和现有 label top1 都按返回 atom vector 在本地计算 cosine similarity，不从
LanceDB distance 推导；候选必须超过现有 label top1，且超过幅度达到固定 margin。
校验失败时 attempt 仍会把候选持久化为 `rejected` proposal，diagnostics 包含
`label_proposal_residual_top1_failed` 或
`label_proposal_residual_margin_insufficient`，用于审计为什么没有进入可接受状态。
如果 residual validation 不可用或 degraded，且没有明确通过 top1+margin 校验，
attempt 返回 `degraded=true`、`proposal=null`，不新增 proposal row，也不创建
canonical label、`label_semantics`、`label_atoms` 或 `task_labels`；diagnostics 包含
`label_proposal_residual_validation_unavailable` 和具体原因。
传入 `--source-signal <los_...>` 时，proposal 创建成功后会在同一 transaction 写入
`create_label_proposal` ontology action，并通过 action-signal links 记录该 proposal
由哪些 confirmed vocabulary-gap signals 支持；proposal row 与 provenance action
要么同时写入，要么一起回滚。Source signals 默认必须是同一 board 上 `confirmed`
的 `vocabulary_gap` + `bootstrap_label` signals，且 normalized `proposed_label_name`
必须等于 proposal name。`--actor-type` / `--agent-type` 控制该
`create_label_proposal` action 的 actor provenance；actor name 仍来自全局 `--actor`。
确实需要把 confirmed same-board source signal retarget 到该 proposal 时，必须同时传
`--allow-retarget` 和非空 `--retarget-reason <text>`；reason 和 source signal 原始
target/proposed label 会写入 `change_json.retarget_override`。Override 不放宽
board/status 要求。

`label proposals accept` 只接受 `proposed` proposal。accept 与单 task bootstrap 共用
同一个 adoption primitive：创建 canonical label、`label_semantics` 与 `label_atoms`，
标脏 label atom index，并写入 `bootstrap_label` ontology action；proposal row、
canonical writes 和 action provenance 要么同一 transaction 成功，要么一起回滚。它不会自动
给来源 task 写入 `task_labels`。未传 `--source-signal` 时仍会记录 bootstrap action，
只是没有 action-signal links；传入 `--source-signal <los_...>` 时会通过 links 记录该
new-label bootstrap 的 signal provenance，且这些 source signals 必须是同一 board 上的
`confirmed` signals。`--actor-type` / `--agent-type` 控制该
`bootstrap_label` action 的 actor provenance；actor name 仍来自全局 `--actor`。
默认是 `user`。`--actor-type agent` 必须提供非空 `--agent-type`；`user` 不能提供
`--agent-type`。Source signals 默认还必须是 `vocabulary_gap` +
`bootstrap_label`，且 normalized `proposed_label_name` 必须等于 proposal name。
如果 proposal 已有 `create_label_proposal` action，accept 产生的 `bootstrap_label`
action 会把 `parent_action_id` 指向该 creation action，形成 proposal creation ->
bootstrap acceptance 链路。
确实需要把 confirmed same-board source signal retarget 到该 proposal 时，必须同时传
`--allow-retarget` 和非空 `--retarget-reason <text>`；该 reason、source signal 原始
target/proposed label 和最终 proposal/result label 会写入 bootstrap action
`change_json.retarget_override`。Override 不放宽 board/status 要求。`label proposals reject`
标记 proposal 为 `rejected`，不接受 `--source-signal`。accepted/rejected proposal 不能再次决策。

`label ontology record` 记录一次 label 判断 observation 并写入其中的 child signals。
推荐输入边界是：工具采集或接收未改写的 `label suggest` snapshot，service 从 snapshot
派生 coverage、residual、degraded、diagnostics 等 observation metrics；agent 只提交
候选、最终判断、signals、candidate atom 和 rationale。CLI 可以用
`--capture-suggest` 在 record 前用同一组 suggest options 运行一次真实 `label suggest`，
也可以用 `--suggestion-snapshot <path|->` 读取已保存的原始 suggest JSON。snapshot
可以是直接的 suggest response，也可以是带 `data` wrapper 的 JSON response。

`--input` 只接受 contract-owned natural JSON shape；旧 `_json` compatibility siblings
（例如 `diagnostics_json`、`related_labels_json`）会作为 unknown field 拒绝。新调用方不应重复手写
`suggest_coverage`、`suggest_residual_norm` 或 `diagnostics`。如果 snapshot 中已有
这些字段而输入又提供冲突的标量或 diagnostics，命令会失败。Service 会读取当前 task
snapshot、解析 target label ref、计算 normalized proposed label name、signal key 和
candidate atom content hash；observation 同时保存完整审计用
`task_snapshot_json.content_hash` 和只基于 label suggest 输入（normalized title +
description）的 `suggest_input_hash`。它只写 ledger，不修改 `task_labels`、
`label_semantics`、`label_atoms`、label atom index 或 proposal。

Signal 输入会在写入前做 ontology contract 校验。`candidate_atom` 的
`applies_when` / `positive_example` 只能使用 `positive` polarity，
`excludes_when` / `negative_example` 只能使用 `negative` polarity。
`add_positive_atom` 必须提供 target label 和 positive candidate atom；
`add_negative_atom` 必须提供 target label 和 negative candidate atom；
`update_semantics` 必须提供 target label；`bootstrap_label` 必须提供
`proposed_label_name`；`rename_label` 必须提供 target label 和
`proposed_label_name`；`split_label` / `merge_labels` 必须提供 target label 和非空
`related_labels`。Observation metric `suggest_coverage`、
`suggest_coverage_cosine`、`suggest_residual_norm` 以及 signal metric
`suggest_score` / `confidence` 必须是 finite `0.0..=1.0`；`suggest_rank` 必须为
`null` 或 `>= 1`。
`rename_label` / `split_label` / `merge_labels` 当前只作为 review signal proposed_action
保存，CLI 不提供写入 canonical structure mutation action 或 structure plan action 的命令；
旧 structure-plan rows 只读展示为 unsupported validation requirement。

使用已保存 suggest snapshot 的推荐输入形状：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
  "agent_candidates": [
    {"label": "cli", "reason": "The task changes CLI behavior."}
  ],
  "final_decision": {
    "selected": ["cli"],
    "rejected": []
  },
  "signals": [
    {
      "kind": "false_negative",
      "target_label_ref": "cli",
      "related_labels": [],
      "proposed_action": "add_positive_atom",
      "candidate_atom": {
        "polarity": "positive",
        "kind": "applies_when",
        "text": "extends CLI subcommands, command arguments, help output, or machine-readable JSON behavior"
      },
      "proposal": {},
      "agent_selected": true,
      "suggest_state": "candidate",
      "suggest_score": 0.08,
      "suggest_rank": 6,
      "final_selected": true,
      "rationale": "The task expands the CLI surface."
    }
  ]
}
```

调用示例：

```bash
kanban label suggest default#42 --json > /tmp/default-42-suggest.json
kanban label ontology record default#42 \
  --input /tmp/default-42-record.json \
  --suggestion-snapshot /tmp/default-42-suggest.json \
  --json
```

或者让 CLI 在记录前采集 snapshot：

```bash
kanban label ontology record default#42 \
  --input /tmp/default-42-record.json \
  --capture-suggest \
  --vector-config ./vector.toml \
  --json
```

`label ontology list` 默认只返回 `open` 和 `confirmed` signals。`--include-all`
返回完整历史；`--status`、`--kind` 可重复过滤，`--task`、`--label` 和
`--proposed-label` 用于按来源 task、目标 label 或候选新 label 查询。
`label ontology show` 返回 signal、observation 和关联 actions。`label ontology review`
是只读聚合 review queue 视图，默认只聚合 `open` 和 `confirmed` signals；传
`--include-all` 时包含 resolved/rejected/superseded 历史。`--group-by` 支持按
`label`、`candidate-atom`、`proposed-label` 或 opt-in `cluster` 聚合，`--limit` 限制返回 group
数量。`--json` 每个 group 返回聚合维度、key、相关 label / candidate atom /
proposed label、cluster key/reason（仅 cluster view 有值）、distinct task count、signal/status/degraded/action counts、score
summary、sample task refs、signal ids、action ids 和 proposal ids。排序优先使用
distinct task count，其次 confirmed count、latest signal time 和 key。

Review group 只表示一组 signals 共享同一个聚合键，不证明它们一定来自同一个根因。
`--group-by label` 使用 `target_label_id` 作为 key，缺失目标 label 时使用
`no-target-label`。`--group-by proposed-label` 使用 normalized proposed label name，
缺失候选新 label 时使用 `no-proposed-label`。`--group-by candidate-atom` 优先使用
`candidate_content_hash`；如果 signal 没有 candidate atom，则 key 会包含 signal kind、
target label 或 proposed label、以及 proposed action，例如
`no-candidate-atom|kind:vocabulary_gap|proposed:ontology ledger|action:bootstrap_label`。
这个 fallback 避免把不同 kind、不同 label 或不同 proposed action 的空 candidate
signals 合并到一个全局 bucket。
`--group-by cluster` 是一个只读 review-aid：它不写 canonical atoms，也不会确认、
应用、validate 或关闭 signal。cluster key 每次查询时从已有 signal 文本重建，优先使用
lexical-normalized candidate text，其次 proposed label，再其次 rationale，最后才退回到
kind/action/target/proposed-label scope 组合；所有 cluster key 都带有 signal kind、
proposed action、target label 和 proposed-label scope，避免跨 label/action/boundary 误合并；
`cluster_reason` 说明当前 key 的来源。

`task_count` 是 group 内 distinct source task 数，也是默认热度排序的第一依据；同一 task
上的多条 signals 仍只贡献一个 distinct task。`signal_count` 是原始 signal 行数，
用于判断一组里有多少审查项；它没有 denominator，不能解释为模型错误率、precision
或 recall。`degraded_count`、status counts、score summary 和 sample task refs 只是
reviewer 的排查线索。排序为 `task_count` desc、`confirmed_count` desc、
`latest_signal_at` desc、`key` asc；需要判断是否同一问题时，应继续查看 group 的
sample tasks、signal ids 和 `label ontology show` 详情。

`label ontology quality` 是只读 quality/analytics 报告。它从当前 board 的
`label_ontology_observations` 取得可审计 denominator，并从
`label_ontology_signals` 取得 raw disagreement counts；不会写入 task、label、
semantics、atoms 或 ledger action。JSON 输出包含：

- `denominator.source="label_ontology_observations"`、`observation_count`、
  `distinct_task_count`、agreement/degraded observation counts、时间范围和
  `sample_task_refs`。
- `disagreement.signal_count`、`disagreement.distinct_task_count`、`by_kind`、
  `by_status`。
- `rates.disagreement_task_rate`，只在 denominator 至少包含一个 agreement
  observation 时返回；只有 signals 的历史不会输出伪错误率。
- `precision_recall.available=false`，直到项目有带 expected labels 的独立评估
  cohort。raw signals 只能说明记录过分歧，不能单独证明 precision、recall、miss
  rate 或模型错误率。

Lifecycle commands 写入 action 并同步更新 signal status：

- `confirm`：`open` signal 进入 `confirmed`。
- `reject`：把 signal 标记为 `rejected`。
- `supersede --by`：把重复或过时 signal 标记为 `superseded`；写入前会沿
  replacement `superseded_by_signal_id` 链检查，拒绝会回到任一 source signal 的环。
- `resolve --no-change`：记录无需 ontology 修改的 resolution。

这些 lifecycle commands 只记录 review/status 变化，不接受 canonical mutation
provenance 字段。`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
`update_semantics`、`create_label_proposal`、`bootstrap_label`、`revert_ontology_mutation` 和 `validate` 等 action rows 只能由
`label semantics upsert`、`label ontology apply atom`、`label propose`、
proposal accept、`label bootstrap`、`label ontology revert`、`label ontology validate` 等专用命令/服务路径在同一
transaction 中写入。通用 action command 不能伪造 canonical before/after hash、
result atom/result label/result proposal 或 validation payload。
Lifecycle、apply atom、validate 和带 `--source-signal` 的 proposal accept 都支持
`--actor-type user|agent` 与 `--agent-type <type>`。这些 flag 只控制 ontology action
row 的 `created_by_type` / `agent_type`；action name 仍来自全局 `--actor`。默认
`--actor-type user` 且不写 `agent_type`。`agent` actor 必须提供非空 `--agent-type`，
`user` actor 带 `--agent-type` 会被拒绝。

`label ontology apply atom` 只接受 `confirmed` source signals。它会读取目标 label
当前 semantics，把泛化文本加入对应数组，走现有 semantics upsert/rebuild atoms 路径。
如果 canonical 内容实际新增 atom，会写入 `add_positive_atom` 或 `add_negative_atom`
action，记录生成 atom 的软引用、content hash、before/after hash、单份 change snapshot
和一个 `added` atom effect，并把 `validation_requirement` 置为 `required`。如果同内容 atom 已经存在，则写入
`adopt_existing_atom` provenance-only action，记录 existing atom 软引用、before/after
hash（相同）和 source signal links；该 action 不修改 semantics/atoms、不标脏 atom
index，`validation_requirement=none` 且 effective outcome 为 `not_required`。
默认要求所有带 `target_label_id` 的 source signals 都指向被修改 label；不匹配时拒绝
并列出 offending signal ids。Atom text 不需要逐字等于 source signal 的 candidate
text，reviewer 可以写更泛化的 canonical atom。确实需要 retarget confirmed same-board
signals 时，必须传 `--allow-retarget` 和非空 `--retarget-reason <text>`；action
`change_json.retarget_override` 会记录 reason、source signal 原始 target/proposed label
和最终 target label。Override 不放宽 board/status 要求。
该命令只有在 canonical atom 实际新增时才标脏 label atom index；vector index rebuild
和后续 suggest 验证仍是第二阶段。

`label ontology revert <action_id>` 为已提交的 label-scoped canonical ontology mutation
追加 `revert_ontology_mutation` action，并把目标 label semantics 恢复到被撤销 action
的 `canonical_before_hash` / `change_json.before` snapshot。当前只支持
`add_positive_atom`、`add_negative_atom` 和 `update_semantics`；不处理 bootstrap 的
label identity 或 task binding 回滚。为避免覆盖后续修改，命令要求当前 canonical
semantics hash 仍等于目标 action 的 `canonical_after_hash`；传
`--expected-current-hash <hash>` 时还会先做调用方持有快照的 CAS 检查。成功后会写入
append-only revert action，`parent_action_id` 指向被撤销 action，复制原 action 的
source signal links，记录 before/after revert snapshot，为本次 revert 实际 added/removed
atoms 写 atom effects，标脏 label atom index，并把 `validation_requirement` 置为
`unsupported`。原 mutation
action 不会被修改或删除。

所有 canonical semantics/atom mutation transaction 都遵循 one-root-action 合同：同一
transaction 只写一条 root mutation action，`change_json` 只保存一次 before/after
semantics snapshot；实际新增或删除的 atoms 通过
`label_ontology_action_atom_effects` 记录 `added` / `removed` effects。Description-only
patch 会写一条 root action 和零 atom effects；no-op patch 不写 action/effects，也不标脏
index。Atom explain 优先使用 effect rows；legacy per-atom actions 仍保持兼容读取。

`label ontology validate` 为一个 mutation action 追加 `validate` action。Parent action
必须是同一 board 上 `validation_requirement=required` 的 canonical mutation action，
并携带 canonical result evidence（例如 atom/result label/proposal 引用、canonical hash
和非空 change snapshot）。Parent action 的 `validation_status` 是历史兼容字段，不再单独
表达“是否需要验证”；读取时通过 reducer 暴露 effective outcome：
`not_required|unsupported|pending|passed|failed|partial`。

普通 `--input` 路径是 external attestation：CLI 读取调用方提供的 JSON，service 只把
supplied payload、source signal case 摘要、task snapshot/suggest input hash 对比和
parent action 结果引用包装进 validation envelope。公共 supplied/collected payload
只保存一次在 top-level `manual`；generated `cases[]` 使用 `after.manual_case_ref`
引用 `manual.cases[]` 中对应 signal 的 evidence，不在每个 case 中重复整份 payload。
该路径可记录 `failed` / `partial` 诊断，但不能把 `passed` 写成 trusted proof；即使 JSON 自称
`evidence_type="automated"`，`--status passed` 也会被拒绝，linked signals 不会被
关闭。

`--trusted` 路径才是 trusted automated validation。它不接受 `--input`，也不接受调用方
手写 trusted evidence JSON；CLI 只能走内置 collector。Trusted 表示工具在当前 parent
action、source signals、canonical hash、atom index generation 和指定 cases/controls 上做了
机械采集和检查，不表示 ontology 在全局语义上正确。CLI 必须有可用 label atom vector
workflow adapter（当前 no-heavy CLI 尚未接入；旧内置 `vector-lancedb` build 需可解析 `--vector-config` 或默认 config），先在 SQLite transaction 外 rebuild atom index，再用同一
`--limit` / `--candidate-limit` / `--atom-limit` / `--max-selected-labels` /
`--min-score` options 对 linked source signals 重新运行 `label suggest`，由工具生成
`evidence_type="trusted_automated"`、`collector.source="label_ontology_validate_trusted"`、
`embedding_model`、`solver_options`、clean `index.status` / `index.generation` 和
per-signal `cases[]`。写 action 时 service 会在短 transaction 内重新核验 parent action、
source signals、canonical after hash、atom index dirty/error 状态和 generation，防止
查询后 canonical 或 derived state 已变化。dirty/error/disabled index、缺失 generation
或 stale generation 都不能产生 trusted passed。

`--positive-control <TASK_REF>` 与 `--positive-control-waiver <REASON>` 只用于
negative atom trusted validation，且二者互斥；非 negative parent 携带这些参数会被拒绝。
waiver 只能由 `--actor-type user` 提交，reason 必须非空。Negative atom parent 若两者都
缺失，会在 collection 前失败。

`cases[]` 的 `case_type` 必须匹配 parent action：`positive_atom`、`negative_atom`
或 `bootstrap_label`。Positive atom validation 要求 `after.degraded=false`、
result atom id/content hash 出现在 `after.evidence_atoms[]`、target label selected
或 score >= 0.50，且 score/coverage 不恶化。Negative atom validation 要求 result
atom id/content hash 出现在 `after.negative_evidence_atoms[]`；false-positive task 上
必须证明 `after.target.selected=false`，或 before/after score 都存在且 after score
低于 before score；并且必须提供至少一个 `after.positive_controls[]` 且全部 passed
未 regressed，或提供带非空 reason 的 `after.positive_control_waiver`。Bootstrap
label validation 要求所有 linked source signals 都有 passed case，new/result label
selected 或 score >= 0.50，且 evidence atoms 来自 result label。

Validation comparability 默认使用 observation 的
`suggest_input_hash`；status、`updated_at`、`lock_version` 或 task label binding
只改变完整 snapshot 时写入 `task_metadata_drift` / `label_binding_drift` warning，
不会让 passed validation stale。title/description 变化会写入 `suggest_input_drift`
并使 case incomparable；旧 observation 缺少 `suggest_input_hash` 时写入
`legacy_suggest_input_hash_missing`，不能静默 passed。`--status passed` 会把 linked
source signals 转为 `resolved`；`failed` / `partial` 保留历史和 evidence，source
signals 继续等待后续修正或人工处理。

`label propose --json` 返回结构化 attempt：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_...",
    "board_id": "b_...",
    "proposal": null,
    "degraded": true,
    "diagnostics": ["label_proposal_provider_unavailable", "vector_store_disabled"],
    "heuristic_coverage": 0.0,
    "heuristic_coverage_cosine": 0.0,
    "heuristic_residual_norm": 1.0,
    "top1_existing_label_id": null,
    "top1_existing_label_name": null
  }
}
```

---

## 9. Removed DAG Commands

`kanban dag` is no longer a supported command surface. Dependency management
remains available through `kanban dep`, task status transitions, dispatcher
guards, and the shared SQLite command service. Callers that previously consumed
DAG snapshot, ancestors, actionable, or frontier JSON must switch to the
specific dependency, task list, search, context, or graph APIs that match their
use case.

---

## 10. Comment Commands

```bash
kanban comment add <task_ref> [<body>|--body-file <PATH|->] [--kind note|decision] [--author-type user|agent] [--agent-type <type>] [--metadata-json <json>|--metadata-json-file <PATH|->]
kanban comment list <task_ref>
```

`--actor` supplies the comment author display identity. If `--kind` is omitted,
the service default is `note`. If `--author-type` is omitted, the service default
is `user`; pass `--author-type agent --agent-type <type>` for Codex/dispatcher or
other automated writers. `signal` is a persisted comment kind, but users should
create signal backlink comments through `kanban signal record` rather than
manually using `comment add --kind signal`; this keeps the signal ledger and
backlink comment in one transaction. `--body-file <PATH|->` reads long comment
bodies from files or stdin and is mutually exclusive with inline `<body>`; it is the recommended path for multiline or shell-sensitive comment text.
`--metadata-json` defaults to `{}` and must be a JSON object;
`--metadata-json-file <PATH|->` reads the same JSON payload from a file or stdin, avoids shell quoting issues for structured payloads,
and is mutually exclusive with `--metadata-json`. For `--kind decision`,
metadata is required to satisfy the structured
decision schema: non-empty `options`, unique lowercase ASCII option `slug`
values, `selected` matching one slug, non-empty `reason`, and optional
non-empty `risk` / `verification`.

Agent command failure traces should be recorded as comments instead of being
left only in chat transcripts. Use `comment add --author-type agent --agent-type
<name> --kind note --metadata-json <json>` with the human-readable body as a
short summary and the structured trace in metadata. The minimum trace payload is
an object with these fields:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "tool": "kanban-cli",
  "command": "kanban task step add",
  "argv": ["kanban", "task", "step", "add", "..."],
  "intent": "add a required execution-plan step",
  "why_selected": "agent selected the step command because the task needed execution-plan tracking",
  "actual_error": "unexpected argument 'true' found",
  "repair": "retry with canonical bare --required or supported --required true/false form",
  "product_signal": "agent-facing boolean flag compatibility gap",
  "followup_task": "default#123"
}
```

Callers may add extra fields, but these names are the stable minimum contract for
tooling that mines failed agent commands into parser, docs, skill, or test work.

Agent-facing rich input example:

```bash
kanban comment add default#12 --body-file - <<'EOF'
正文可以安全包含 $VAR、$(command)、`code`、JSON 和多行文本。
EOF
```

Use `--kind decision` for meaningful multi-option choices. Body remains the
human-readable fallback summary, while structured options and selection data
live only in `--metadata-json`:

```text
已决定继续使用 comment metadata 承载结构化决策信息，正文保留为简短结论，方便没有结构化渲染的环境阅读。
```

Decision metadata example:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "options": [
    {
      "slug": "comment-metadata",
      "title": "Use comment metadata",
      "detail": "Store structured decision data in task_comments.metadata_json."
    },
    {
      "slug": "decision-table",
      "title": "Create decision table",
      "detail": "Create a separate task_decisions table with option rows."
    }
  ],
  "selected": "comment-metadata",
  "reason": "Keeps decisions close to task discussion and avoids a parallel timeline.",
  "risk": "metadata schema needs validation discipline.",
  "verification": "CLI/API/Desktop tests cover creation, reading, rendering, and invalid metadata rejection."
}
```

Skip decision comments for trivial naming, formatting, or purely mechanical
choices.

Human output is compact and includes comment id, task id, created_at, kind,
author identity, author_type, optional agent_type, and body:

```text
c_01HX... task=t_01HX... created_at=1717520000000 [note] alice (user): ready for review
c_01HX... task=t_01HX... created_at=1717520000100 [note] codex (agent/root): tests passed
```

JSON output uses the standard envelope and returns the contract comment DTO for `add` or
a list of that DTO for `list`, including natural, lossless `metadata` objects. The input flag
names `--metadata-json` / `--metadata-json-file` remain unchanged. Creating a comment
writes `task_events(kind='task.comment.created')`.

---

## 11. Event Commands

```bash
kanban events <task_ref>
kanban events --board default
```

不传 `<task_ref>` 时按 active board 列出 events。Archived board 的 events 仍可通过显式 `--board` 读取。

---

## 12. Run Commands

```bash
kanban runs <task_ref>
kanban run show <run_id>
kanban run logs <run_id>
kanban run logs <run_id> --tail-bytes 65536
```

`kanban run logs` 默认最多读取 256 KiB。传 `--tail-bytes` 时只返回 log 末尾指定字节数。`task_runs.log_path` 必须解析到受信任日志目录且文件名匹配 `<run_id>.log`；可疑路径会被拒绝。

---

## 13. Dispatcher / Server Commands

```bash
kanban serve
kanban serve --quiet
kanban serve --log-level warn
kanban serve --search-sync-interval-ms 5000

kanban dispatch
kanban dispatch --once
kanban dispatch --worker-profile default
kanban dispatch --worker-profile backend --profile-config ./workers.toml
kanban dispatch --max-iterations 10 --poll-interval-ms 1000
```

`kanban dispatch` is a foreground loop. Use `--once` for one pass, or `--max-iterations`
for bounded scripts/tests. `--profile-config` reads the selected `[workers.<name>]`
section and can set `command`, `claim_ttl_ms`, `heartbeat_interval_ms`,
`on_success`, `on_failure`, and `log_dir`. Dispatcher log directories must be
inside a trusted run-log root: the platform default run log directory,
`<db_dir>/logs`, or `<db_dir>/.kb/logs`.

Ctrl-C/SIGINT is an operator stop for the foreground `kanban dispatch` loop.
The current `dispatch_once` / worker iteration is not actively interrupted; the
loop stops before starting another polling iteration, including during the
inter-iteration wait. The command exits `0` after this graceful stop. With
`--json`, stdout remains the normal success envelope and includes
`data.stop_reason="interrupted"`; operator cancellation diagnostics, if emitted,
go to stderr only. A non-interrupted `--max-iterations` exit omits
`data.stop_reason`. A second Ctrl-C during dispatcher shutdown exits
immediately with code `130`.

`kanban serve` writes startup diagnostics, HTTP request traces, and graceful shutdown notices to stderr by default; stdout remains reserved for explicit machine-readable output and is not used for service logs. Use `--quiet` to suppress serve diagnostics, `--log-level <off|error|warn|info|debug|trace>` for a simple verbosity override, or omit both and set `RUST_LOG` for advanced tracing filters. The default filter is `kanban=info,kanban_cli=info,kanban_server=info,tower_http=info,kanban_desktop=info`.

Ctrl-C/SIGINT triggers graceful shutdown for `kanban serve`, releases the runtime
lock, exits `0`, and writes no stdout. `--quiet` and `--log-level off` suppress
the graceful shutdown notice. A second Ctrl-C during shutdown exits immediately
with code `130`.

`kanban serve` starts a conservative background search sync loop when the binary is
built with `tantivy-backend`. The loop makes one prompt startup attempt and then
calls `sync_search_index` every `--search-sync-interval-ms` milliseconds
(default `5000`). Use `--search-sync-interval-ms 0` to disable it. Without
`tantivy-backend`, the flag is accepted and no background index task is started.

---

## 14. Search Commands

### 14.1 `kanban search`

```bash
kanban search <query> [--status ready] [--status review] [--assignee worker-a] [--label backend] [--include-archived] [--limit 20] [--offset 0] [--json]
```

默认 CLI build 启用 `tantivy-backend`。当 `index/v1/tasks/` 存在可读 Tantivy 索引时，`kanban search` 使用 Tantivy；缺失、损坏、过期或二进制显式以 `--no-default-features` 构建时回落 SQLite，并在顶层 `meta` 中标记 stale。搜索匹配 task title、description、comments、run summary/error、event kind/payload。

`--label <name-or-id>` 可重复；多个 label 使用 AND 语义，并在 search
分页前过滤 task。带 label 过滤的 Tantivy search 会回落到 SQLite fallback，
以保持当前 label 关联关系和分页语义正确。

Task ref 形状的 query 始终使用 SQLite 精确匹配语义，即使当前存在可用 Tantivy index：
纯数字 `12`、`#12` 匹配请求 board 内的 seq；`board#12` / `board/#12`
只在显式 board 与请求 board 相同时匹配；`t_...` 只匹配请求 board 内的 task id。
这些 query 不会因为 title、description 或聚合搜索文本包含相同数字/ref 片段而返回额外 task。

Human output compactly includes the public task ref, status, score, title, and snippet when available. It does not include the internal `t_...` task id by default; task id remains available in JSON output and diagnostic/detail-oriented surfaces.

```text
agent-work#12 [ready] score=60.0 实现状态机 - ready spec needle
```

JSON output:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "hits": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "score": 60.0,
        "snippet": "ready spec needle",
        "task": {
          "id": "t_01HX...",
          "seq": 12,
          "status": "ready",
          "title": "实现状态机"
        }
      }
    ]
  },
  "meta": {
    "backend": "sqlite",
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0
  }
}
```

### 14.2 `kanban index`

```bash
kanban index status
kanban index doctor
kanban index rebuild
kanban index sync
```

默认 CLI build 启用 `tantivy-backend`，Tantivy index 是可重建 derived cache；显式以 `--no-default-features` 构建时保留 SQLite fallback：

- `status` returns backend/meta.
- `doctor` returns the same fallback health meta for scripts.
- `rebuild` builds/replaces `index/v1/tasks/` beside the SQLite DB and stores a clean high-watermark state in `app_settings`.
- `sync` consumes `task_events.id` after the stored high-watermark, delete+reindexes affected task aggregates, then advances the high-watermark only after a successful commit.
- Task mutations do not update Tantivy inside their transactions; run `kanban index sync` after changes, rely on `kanban serve` background sync for local server/desktop sessions, or use `kanban index rebuild` to replace the derived index.

The persisted setting key is board-scoped as `search.tasks.state.<board_id>`. Its JSON contains `schema_version`, `index_version`, `backend`, `index_name`, `board_id`, `last_event_id`, `dirty`, `updated_at`, and optional `message`; it is included in JSONL export/import through existing `app_settings` handling.

JSON data shape:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "backend": "sqlite",
    "derived_index": false,
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0,
    "message": "SQLite fallback search is active; no derived index exists yet"
  }
}
```

With Tantivy enabled after rebuild, `backend` is `tantivy`, `derived_index` is `true`, and `index_version` is `tasks-v1`.
When the current `MAX(task_events.id)` is greater than the stored `last_event_id`, `stale=true` and `index_lag_events` reports the event lag. Search falls back to SQLite while stale to preserve current-result correctness.
Background sync errors do not make search fail open to stale Tantivy results; the next search still reports stale/fallback metadata and returns current SQLite results when the derived index is behind or unusable.

---

### Signal Ledger

```bash
kanban signal record --board <slug> --input <path|-> --json
kanban signal list --board <slug> [--status open|confirmed|rejected|superseded|resolved]... [--kind <kind>]... [--task <task-ref>] [--include-all] [--limit 100] --json
kanban signal show --board <slug> <signal-id> --json
kanban signal review --board <slug> [--status open|confirmed|rejected|superseded|resolved]... [--kind <kind>]... [--task <task-ref>] [--include-all] [--limit 100] --json
kanban signal confirm --board <slug> <signal-id>... [--reason <reason>|--reason-file <PATH|->] --json
kanban signal reject --board <slug> <signal-id>... [--reason <reason>|--reason-file <PATH|->] --json
kanban signal resolve --board <slug> <signal-id>... [--reason <reason>|--reason-file <PATH|->] --json
kanban signal supersede --board <slug> <signal-id>... --by <replacement-signal-id> [--reason <reason>|--reason-file <PATH|->] --json
```

`signal list` 和 `signal review` 共享 `status`、`kind`、`task`、`include-all`、
`limit` 查询过滤参数。没有显式 `--status` 时，两者默认只返回 `open` 和
`confirmed`；此时传 `--include-all` 会取消默认状态过滤并返回完整历史。显式
`--status` 始终优先，即使同时传 `--include-all`，结果仍只包含指定状态。
`--status` 和 `--kind` 都可以重复传入。

`record` input JSON supports `kind`, `title`, `summary`, `severity`, optional `task_ref` / `task_id` / `run_id` / `comment_id`, `actor`, `agent_type`, `dedupe_key`, `source`, `evidence`, and optional `comment.body`. `source` is a string identifier for where the observation came from; structured command details such as `command`, `cwd`, `exit_code`, `stderr`, or related logs belong in the natural `evidence` object. Signal responses use the same natural object rather than an escaped `evidence_json` string. When task context is present, the service writes the signal ledger rows and a `comment.kind = "signal"` backlink in one SQLite transaction. Signal backlink `metadata` includes `type:"signal_link"`, `signal_id`, `observation_id`, `signal_kind`, and `signal_status`; generic signal comment metadata remains open and lossless. V1 does not create follow-up tasks automatically.

Lifecycle transitions are `open -> confirmed|rejected|superseded|resolved` and `confirmed -> resolved`. `supersede` requires a same-board replacement signal and rejects cycles. Lifecycle reason 可用 `--reason-file <PATH|->` 从文件或 stdin 读取，并与 inline `--reason` 互斥。

## 15. Maintenance Commands

```bash
kanban doctor
kanban stats
kanban backup --out backup.sqlite
kanban export --format jsonl --out board.jsonl
kanban export --format jsonl --out -
kanban import --input board.jsonl --dry-run
kanban import --input board.jsonl --replace
kanban vacuum
kanban checkpoint

kanban entity list [--kind task] [--limit 50]
kanban entity show kb://task/t_...
kanban outbox list [--status pending] [--limit 50]
kanban derived status
kanban graph status
kanban graph neighbors kb://task/t_... [--predicate depends_on] [--limit 50]
kanban graph query [<SPARQL>|--sparql-file <PATH|->] [--limit 50]
kanban vector configure [--provider ollama] [--endpoint http://127.0.0.1:11434] [--model qwen3-embedding:0.6b] [--dimensions 1024] [--skip-check] [--vector-config <toml>]
kanban vector status [--vector-config <toml>]
kanban vector rebuild [--vector-config <toml>]
kanban vector sync [--vector-config <toml>]
kanban context build t_... [--lexical-limit 5] [--vector-config <toml>]
```

`kanban stats --json` 返回 status counts、过期 running claim 列表、blocked reason 聚合、unplanned active task 数量，以及 required steps 未完成的 active parent 数量，用于本地 operator recovery。
`kanban graph query` 的 SPARQL 可用 `--sparql-file <PATH|->` 从文件或 stdin 读取，并与 positional `<SPARQL>` 互斥。

`kanban backup` 使用 SQLite `VACUUM INTO` 创建一致备份；目标文件已存在时失败，避免覆盖。`backup --out -` 会被明确拒绝，因为 SQLite backup 需要 filesystem path，不能安全写入 stdout。
`kanban export --format jsonl` 导出数据库记录；目标文件已存在时失败，避免覆盖旧 snapshot。`export --out -` 会把 JSONL snapshot 写入 stdout，不输出 human status 文案，也不会写 stderr；该模式不能与 `--json` 组合，因为 JSONL stream 和 JSON envelope 不能共享 stdout。21 个稳定 discriminator 的 input/output 分别拥有 42 个 exact schema roots；每行 data 闭合，required-nullable 键不能省略但可显式为 `null`，export/import descriptor 与 schema authority 同源。JSONL 不复制 `task_runs.log_path` 指向的外部日志文件，导出的 run 记录会清空 `log_path`；导出中的 live `running` task 会清除 claim并恢复为 `ready`，对应 running run 会落为 `canceled`，并追加 `task.export_sanitized` 事件解释这次 portable snapshot 改写。需要完整可恢复副本时使用 `kanban backup`。JSONL export 包含 generic signal ledger record types：`signal_observation`、`signal`，以及 label ontology ledger record types：`label_ontology_observation`、`label_ontology_signal`、`label_ontology_action`、`label_ontology_action_atom_effect` 和 `label_ontology_action_signal`；因此 portable JSONL 与 SQLite backup 都会保留 signal、ontology observation/signal/action/effect provenance。JSONL `event.data.payload` 仍按 opaque JSON 保存；39-kind typed union 只属于 events API/SSE。
`kanban import --dry-run` 会在临时 SQLite 数据库中解析导入文件并运行同一 final doctor gate，不替换或创建所选目标 DB；脚本和 CI 可先用它验证 snapshot。`kanban import --replace` 是替换式恢复入口，必须显式传 `--replace`；导入文件必须至少包含一个 board，且每个 board 必须包含 columns。`kanban import --replace` 是 offline-only 操作；运行前必须停止 `kanban serve` 和常驻 `kanban dispatch`，如果检测到 active runtime lock 会直接拒绝。Import 在同一 SQLite transaction 内执行插入与 final doctor gate：基础关系表会校验 `task_labels`、`task_dependencies`、`task_runs`、`task_comments`、`task_events`、`task_attachments` 的 row board 与 referenced task / label / run board 一致；失败时整个 replace transaction 回滚，不提交部分数据。Ontology import 会延迟回填 `label_ontology_signals.superseded_by_signal_id` 与 `label_ontology_actions.parent_action_id`，因此不依赖 JSONL 中同表自引用 rows 的偶然顺序；导入后会拒绝跨 board / orphan generic signal context、generic signal supersede cycles、跨 board ontology links、orphan action-signal links、ontology supersede cycles 和 action parent cycles。
`kanban entity`、`kanban outbox`、`kanban derived` 是 Knowledge Substrate 的只读维护入口。SQLite 仍是事实源；这些命令只报告统一 entity registry、派生索引 outbox 和 derived store 状态，不改变 task 状态或 claim。
`kanban entity list --json` 返回 `{"data": [...]}`，`kanban entity show --json` 返回
`{"data": {...}}`；两者共享闭合的公开 entity item，并保留
`uri`、`kind`、`source_table`、`source_id`、`created_at`、`updated_at`，以及
required-nullable `board_id`、`task_id`、`title`、`summary`、`content_hash`、
`archived_at`。调用方不能把这些字段缺失解释为 `null`。`list` 的 `--kind` 与
`--limit` 由同一 SQLite service query 执行；`show` 继续按 exact URI 查询并保留
`not_found` error envelope。human-readable 输出不变。
`kanban graph` 和 `kanban vector` 是 helper subprocess 派生层入口。默认 CLI 不链接
Oxigraph/LanceDB heavy deps；它解析 `KANBAN_GRAPH_HELPER` / `KANBAN_VECTOR_HELPER`、
`/usr/lib/kanban/<helper>`、CLI sibling binary、`KANBAN_CARGO_TARGET_ROOT` 或
`CARGO_TARGET_DIR` 的 `release/<helper>`，最后回退到 `PATH` 中的 helper。helper 缺失或
返回非法 envelope 时，`status` 返回 disabled/degraded status；helper error envelope、
坏 board/db/config 或 payload/domain 错误会作为命令错误返回。启用后仍只作为可重建
relation/vector store，不参与 task 状态事务。
`kanban vector status --json` 保留 `message` 兼容字段，同时返回结构化
`diagnostics`、`dirty`、`board_dirty` 字段；dirty/error 判断应使用这些字段，不解析
`message` 文案。
`kanban vector configure` 默认写入全局 config：`$XDG_CONFIG_HOME/kanban/config.toml`（平台默认通常为 `~/.config/kanban/config.toml`），并默认配置本机 Ollama embedding provider。传 `--vector-config <toml>`（别名 `--config`）时写入指定 TOML。configure 默认调用 `/api/embed` 做短文本维度校验；校验失败时不写配置；`--skip-check` 只跳过这次连通性/维度检查。配置格式：

```toml
board = "kanban-tool"

[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "qwen3-embedding:0.6b"
dimensions = 1024
```

项目级 `.kb/config.toml` 可以覆盖全局 `[vector]`；命令行 `--vector-config <toml>` 优先级最高。解析顺序是：显式 `--vector-config`、最近的项目 `.kb/config.toml`、全局 config。`kanban board use <board>` 更新项目配置文件的 `board` 字段时必须保留该文件内已有 `[vector]` 配置。配置有效且 helper 可用时 `kanban vector status/rebuild/sync` 使用该 provider；未配置或 helper 不可用时保持 disabled/degraded fallback。`kanban context build` 当前仍使用 SQLite/lexical fallback，并通过 degraded markers 报告 graph/vector 不可用。
`kanban context build` 通过 SQLite hydrate canonical task，并合并 lexical、graph、vector hits。graph/vector 不可用或失败时返回 degraded markers；失败原因通过有界 diagnostics 暴露，context pack 本身仍可用。

`kanban outbox list --json` 返回 `{"data": [...]}`，每项保留完整 outbox job 字段，
包括 required-nullable `source_event_id` 与 `last_error`；`--status` 与 `--limit` 由同一
SQLite service 查询执行。`kanban derived status --json` 同样返回 `{"data": [...]}`，
每个 store 的 `last_rebuild_at`、`last_sync_at` 与 `last_error` 都是 required-nullable，
调用方不能把字段缺失解释为 `null`。

`kanban derived status` 中的 `last_event_id` 是 store 级成功处理水位，不是当前 board 的局部水位。`dirty=true` 表示该 store 仍有任意 board 的 pending/running/failed outbox，或最近一次派生更新失败；board-scoped `kanban index sync`、`kanban graph sync`、`kanban vector sync` 只清理当前 board 的 job，不能因为本 board clean 就强制清掉全局 dirty。

语义 label atom 使用独立 derived store `lancedb_label_atoms`，对应 LanceDB 表
`kb_label_atoms`。它不属于普通 task event outbox fanout：`kanban vector sync/rebuild`
只维护 `lancedb_chunks` / `kb_chunks`，不会把 label atom store 标记为完成。label
semantics service 写入 `label_semantics` / `label_atoms` 后单独标脏
`lancedb_label_atoms`；provider 或 feature 不可用时该 store 可报告 degraded，但不影响
普通 `kanban label` CRUD 和 `task_labels` 绑定。

### 15.1 `kanban doctor`

检查：

- DB 文件存在。
- migrations 完整；当前 schema user_version 为 23。
- `PRAGMA integrity_check`。
- orphan active run。
- running task 是否缺 claim。
- expired claim 数量。
- dependency cycle。
- archived dependency edge（archived parent -> active child is allowed history; archived child from active parent is reported）。
- 缺失 run log 文件。
- 可疑 run log 路径。
- `ready/running` task 带有未完成 parent dependency。
- `ready/running` task 缺少可执行 spec。
- `ready/running` task 带有未来 `scheduled_at`。
- 基础关系表 board consistency：`task_labels`、`task_dependencies`、`task_runs`、
  `task_comments`、`task_events`、`task_attachments` 的 row board 必须和 referenced
  task / label / run board 一致。当前 schema 用 board-scoped composite FK 保护
  `task_labels`、`task_dependencies`、`task_runs`、`task_comments` 和
  `task_attachments`；v22+ 还检查 `task_execution_plans` task board scope，v23+ 还检查 `task_steps` parent/linked task board scope。`task_events` 保留 nullable task/run refs 与 `ON DELETE SET NULL`
  语义，通过 INSERT/UPDATE triggers 校验非空 refs 的 board scope。
- SQLite `PRAGMA foreign_key_check`：doctor 将每条 violation 转换为 hard-error issue；
  JSONL import final gate 也会在 commit 前运行同一检查，失败时回滚整个 replace
  transaction。
- `index_outbox` backlog：`outbox_pending`、`outbox_running`、`outbox_failed`。
- derived store health：`derived_dirty_stores`、`derived_error_stores`、`derived_stores[]`，每个 store 包含 `dirty`、`last_error` 和按 store target 聚合的 pending/running/failed outbox 计数。
- foundation relationship consistency：人类输出包含 `consistency_errors` /
  `consistency_warnings` 计数；`--json` 额外返回 `consistency_issues[]`，每条 issue
  包含 `severity`、`code`、`message`、`record_ids`。Message 包含 `table`、`row`、
  `row_board`、`referenced` 和 `referenced_board`。非零 `consistency_errors` 会让
  `ok=false`。
- label ontology ledger health：v12+ 数据库必须存在 `label_ontology_observations`、`label_ontology_signals`、`label_ontology_actions`、`label_ontology_action_atom_effects`、`label_ontology_action_signals`；doctor 会报告 observation/signal/action/action-effect/action-signal 的跨 board link、orphan link、parent action 异常、supersede cycle 和可检查 soft reference 不一致。人类输出包含 `ontology_ledger_errors` / `ontology_ledger_warnings` 计数；`--json` 额外返回 `ontology_ledger_issues[]`，每条 issue 包含 `severity`、`code`、`message`、`record_ids`。非零 `ontology_ledger_errors` 会让 `ok=false`；warning 用于 rebuildable 或可解释的软引用异常，不单独让 doctor unhealthy。

`dirty` / pending outbox 表示派生层需要 sync/rebuild，不会改变 SQLite task truth；failed outbox 或 `last_error` 用于 operator 判断是否需要 `kanban index sync`、`kanban graph sync/rebuild` 或 `kanban vector sync/rebuild`。`derived_stores[].last_event_id` 表示对应 store 已成功提交的全局 event watermark；当 `dirty=true` 时，它仍然只是“已成功处理到哪里”的摘要，不代表所有 board 都已经干净。

---

## 16. JSON contract reference

JSON 输出、运行期 JSON error、clap parse-time error、stderr/stdout 数据平面和 JSONL / NDJSON streaming boundary 的权威契约统一见 [1.3 JSON output contract](#13-json-output-contract)。

本节仅保留跳转，避免同一份 CLI_SPEC 出现两个 JSON 契约来源。新增或修改 JSON / JSONL / error-code 行为时，只更新 1.3 及对应命令章节，并补充测试证据。


---

# File: docs/API_SPEC.md

# Local Web API SPEC

本 API 只面向 localhost Web UI 和本地脚本。它不是远程协作 API。

默认监听：

```text
127.0.0.1:8721
```

Base path：

```text
/api/v1
```

---

## 1. 通用约定

### 1.1 Content Type

Request：

```http
Content-Type: application/json
```

Response：

```http
Content-Type: application/json
```

SSE：

```http
Content-Type: text/event-stream
```

### 1.2 Actor

因为没有多用户系统，actor 是审计字段。

来源优先级：

1. Request body `actor`。
2. Header `X-KB-Actor`。
3. Server 默认 actor。
4. OS username。

### 1.2.1 Request header contracts

除 SSE stream 外，当前 83 个 HTTP endpoint 都拥有 operation-specific、exact、
`deny_unknown_fields` header contract；每个 contract 都包含可选 `Accept-Language`，并按真实
handler 输入选择 locale、locale + actor、locale + JSON content type，以及它们的
optional-body 变体。`X-KB-Actor` 只出现在会解析 actor 的 mutation handler。

有必需 JSON body 的 endpoint 要求且只允许一个 `Content-Type`；允许空 body 的 archive、
promote、reclaim、unblock、label proposal propose/accept/reject endpoint 将其建模为可选；没有
body 的 endpoint 不声明 `Content-Type`。这些 cardinality 是 transport contract，不改变 Axum
对具体媒体类型和 malformed JSON 的既有 400 行为。

SSE 的 `Last-Event-ID` 仍明确 `Excluded`：当前 runtime 忽略该 header，没有 resume contract；
不得因为其它 endpoint 已关闭 headers 就把它推断为 adopted input。

### 1.3 Success Response

成功响应按 endpoint 的元数据契约使用以下 wire envelope：

`DataEnvelope` 仅包含 `data`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {}
}
```

`MetadataEnvelope` 的 `meta` 是必需字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {},
  "meta": {}
}
```

`OptionalMetadataEnvelope` 只在 endpoint 产生对应元数据时包含 `meta`；没有元数据时
直接省略该字段，不返回 `"meta": null`。具体 endpoint 使用哪一种 envelope 及其
`meta` 字段由该 endpoint 的响应示例和说明定义。

### 1.4 Error Response

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot claim task from status todo"
  }
}
```

`error.code` 是稳定机器契约。`error.message` 是 human-readable 文案，会根据
`Accept-Language` 在 `zh-CN` 和 `en` 之间选择；未传 header 时保持既有默认 `en`。
客户端逻辑必须读取 `error.code`，不要解析 `error.message`。

### 1.5 HTTP Status Mapping

| Error code | HTTP status |
|---|---:|
| `invalid_input` | 400 |
| `not_found` | 404 |
| `conflict` | 409 |
| `dependency_cycle` | 409 |
| `invalid_transition` | 409 |
| `dependency_blocked` | 409 |
| `execution_plan_required` | 409 |
| `steps_incomplete` | 409 |
| `claim_conflict` | 409 |
| `claim_token_mismatch` | 403 |
| `internal` | 500 |

---

## 2. Health

### `GET /health`

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "ok": true,
    "db": "ok",
    "version": "2.1.2",
    "db_path": "/home/alice/.local/share/kb/kb.db",
    "db_fingerprint": "sqlite:131072:1717520000000"
  }
}
```

`db_path` and `db_fingerprint` let local Desktop/Web development surfaces verify
which local SQLite runtime answered the request. If the configured database file
has been deleted, `/health` returns `400 invalid_input` instead of recreating an
empty SQLite file. Other API routes apply the same missing-file guard before
running handlers, so stale/deleted runtimes fail explicitly instead of opening a
new empty database at the configured path. `/health` also validates that the
database has the expected migrated schema and returns `400 invalid_input` for an
empty or uninitialized SQLite file.

---

## 3. Boards

### 3.1 List boards

```http
GET /api/v1/boards?include_archived=false
```

Archived boards are hidden by default. Pass `include_archived=true` to include them.

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "b_01HX...",
      "slug": "default",
      "name": "Default",
      "description": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000,
      "archived_at": null
    }
  ]
}
```

### 3.2 Create board

```http
POST /api/v1/boards
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "slug": "agent-work",
  "name": "Agent Work",
  "description": "Local agent board",
  "actor": "alice"
}
```

Response status is `201 Created`. Board slugs must be unique, non-empty, no longer than 64 bytes, start with a lowercase ASCII letter or digit, contain only lowercase ASCII letters, digits, `.`, `_`, `-`, and must not start with reserved ID prefixes such as `b_`, `t_`, `r_`, `c_`, `a_`, `l_`, `col_`, or `e_`. Duplicate or invalid slugs return the normal `400 invalid_input` error envelope, not `500`.

### 3.3 Get board

```http
GET /api/v1/boards/{board_slug_or_id}
```

### 3.4 Archive board

```http
POST /api/v1/boards/{board}/archive
```

Archive marks `archived_at` and writes a `board.archived` event; it does not mutate tasks. The operation is rejected with `409 invalid_transition` if the board has active `running` tasks or `running` task runs. After archive, ordinary task mutations on that board are rejected, while audit history endpoints remain readable when called with explicit task/board identity.

### 3.5 B2-C6 boards exact contracts

四个 boards endpoint 使用 endpoint-specific contract roots：list query、create request、
get/archive path 与各自 success response。四个 success response 只共享闭合 `ApiBoard` component；
server 显式把 SQLite application record 映射为 wire DTO，不直接序列化 `BoardRecord`。
archive request body 继续复用既有 `ArchiveBoardRequest` contract。

`include_archived` 缺省为 `false`，传入 `true` 时真实转发到 service 并返回 archived boards。
Desktop `listBoards` caller exact 校验 `data` envelope 与 `ApiBoard` 的全部字段，missing、mistyped
或 extra field 返回 `invalid_response`。running-work archive guard、archived audit history、
not-found status/error code 与 locale-dependent message 不属于 schema authority，继续由
service/adapter 保证。四个 endpoint 的 headers obligation 仍为 `Todo`，migration state 保持
`Generated`。

---

## 4. Tasks

### 4.1 List tasks

```http
GET /api/v1/boards/{board}/tasks
```

Query params：

| Param | 说明 |
|---|---|
| `status` | 可重复：`?status=ready&status=running`。 |
| `priority` | 可重复：`?priority=0&priority=2`，值为 P0-P3 的 `0..3`。P0 表示 incident/blocker/must-handle-immediately；P3 是普通 backlog/低优先级/默认。 |
| `assignee` | 按 assignee。 |
| `label` | 按 label 名称或 id 过滤，可重复；多个 label 使用 AND 语义。 |
| `plan_filter` | 可重复：`plan_needed` / `has_steps` / `incomplete_required_steps`。 |
| `q` | title/description 搜索；task ref 形状按精确匹配处理。 |
| `include_archived` | bool。 |
| `limit` | 默认 100。 |
| `offset` | 分页 offset。 |
| `sort` | `seq` / `title` / `status` / `position` / `priority` / `assignee` / `scheduled_at` / `due_at` / `created_at` / `updated_at`，前缀 `-` 表示降序。`priority` sorts P0 -> P3; `-priority` sorts P3 -> P0. |

这两个 task-read endpoint 使用同一套严格 raw-query 语法，但各自拥有独立的 exact path/query
contract，并由两个 server-local typed Axum extractor 分别绑定真实 `{board}` path 与唯一 raw URI
query 消费点；handler 只接收已解析 request，不持有 `RawQuery` 或第二套 `Query<T>` extractor。
只有 `status`、`priority`、`label`、`plan_filter` 可以重复；不同语义值按 URI 首次出现顺序
保留，任何重复语义值返回 `400 invalid_input`。`assignee`、`q`、`include_archived`、
`limit`、`offset`、`sort` 任一重复也返回 `400`。任何未知 key 失败关闭；旧 `search`
alias 已删除，只接受 `q`。

raw query 最多 8192 bytes。参数对上限不是独立字面量，而是由 9 个 `status`、4 个
`priority`、3 个 `plan_filter`、32 个 `label` 和 6 个 scalar 参数推导出的 54。
`q` 最多 1024 个 Unicode 字符，`assignee` 与单个 `label` 最多 128 个。未提供 query 时
默认 `include_archived=false`、`limit=100`、`offset=0`、`sort=position`。`limit` 的 wire
authority 是 `kanban-contract` 的 1000；SQLite service defensive maximum 直接引用唯一
application authority，server 对该实际 service path 建立编译期相等门禁。`offset` 最大为
`i64::MAX`。空的 `q`、`assignee` 归一化为未提供；label 会规范化 Unicode 边缘空白，但必须
包含至少一个非空白字符，且 raw 字符长度不得超过 128；该预算在 trim 前计算，随后会被移除
的 Unicode 边缘空白也计入 128 字符。空或纯 Unicode 空白 `label`、enum、bool、数字或 sort
值无效。
query 使用严格 form 解码：`+` 表示空格，`%HH` 必须完整且解码结果必须是 UTF-8；合法
UTF-8 与 `&`、`/`、`=`、`+`、空格必须由标准 form encoder 转义，非法 percent encoding
或 UTF-8 返回 `400`。

Priority 只表达相对重要性和排序，不表达可 claim 状态。`ready` 才表示任务已显式进入可执行队列；普通 `ready` task 可以是 P1/P2/P3，不应为了表示“可做”全部标成 P0。P0 只用于 incident、当前目标 blocker 或必须立即处理的任务；P0 task 若仍缺规格、排期未到或依赖未完成，仍不能被 claim。

`q` 对 task ref 形状使用精确匹配而不是文本 contains 匹配：纯数字 `12`、
`#12` 匹配 `{board}` 内的 seq；`board#12` / `board/#12` 只在显式 board
与 `{board}` 相同时匹配；`t_...` 只匹配 `{board}` 内的 task id。其他文本仍执行
title/description 模糊搜索。

响应（以下为字段节选；完整、可消费的成功响应以 `schemas/fixtures/api/list-tasks-response.v1.valid.json` 为准）：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "t_01HX...",
      "seq": 12,
      "board_id": "b_01HX...",
      "board_slug": "agent-work",
      "ref": "agent-work#12",
      "title": "实现状态机",
      "description": "...",
      "status": "ready",
      "priority": 1,
      "position": 1024,
      "assignee": null,
      "scheduled_at": null,
      "due_at": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000,
      "labels": [
        {
          "id": "l_01HX...",
          "board_id": "b_01HX...",
          "name": "core",
          "color": null
        }
      ],
      "dependency_blocked": false,
      "unfinished_parent_count": 0
    }
  ],
  "meta": {
    "limit": 100,
    "offset": 0,
    "total": 1
  }
}
```

### 4.2 Create task

```http
POST /api/v1/boards/{board}/tasks
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "实现状态机",
  "description": "Markdown spec",
  "status": "ready",
  "assignee": "local-worker",
  "priority": 1,
  "scheduled_at": null,
  "due_at": null,
  "max_retries": 2,
  "depends_on": ["t_01HX..."],
  "labels": ["core"],
  "metadata": {},
  "actor": "alice"
}
```

Notes：

- `status` 只能是 `triage|todo|scheduled|ready`。
- 若不传 `status`，服务端计算初始状态。
- 若存在未完成 dependencies（parent 不是 `done` 或 `archived`），不能创建为 `ready`。
- 若 execution plan 仍为 `unplanned`，不能创建为 `ready`；先添加 step，或显式标记 `not_required` 并填写 reason。
- Task 响应会暴露派生 dependency 和 execution-plan 字段：`dependency_blocked`、`unfinished_parent_count`、`execution_plan_state`、required/optional step counts。它们是查询元数据，不是可写 task 字段。
- `priority` 是整数等级 `0..3`：`0` = P0 incident/blocker/must-handle-immediately，`1` = P1 近期重点，`2` = P2 重要后续，`3` = P3 普通 backlog/低优先级/默认。创建时会拒绝非法值。
- `labels` 可选。名称会先 trim；空白名称会被拒绝；所有 label 必须已存在于当前 board。任一 label 缺失时，整个 create 返回 `400 invalid_input`，且不会写入 `tasks`、`labels`、`task_labels` 或 `task_events`。Task create 不提供 create-missing 模式。
- `priority` 缺省为 `3`，`labels` / `depends_on` 缺省为空数组；其它 nullable 字段可显式传 `null`。`metadata` 只接受 JSON object 或 `null`，其 object 内容是 opaque extension，不在 transport layer 解释。
- path、request 与 `201` success response 分别由 `CreateTaskPath`、`CreateTaskRequest` 与 `CreateTaskResponse { data: ApiTask }` 拥有。request status 使用 create-only 的闭合 `triage|todo|scheduled|ready` vocabulary；公开 response 不包含 `claim_token`。
- handler 只做 contract 到 application input 的显式映射，并继续单次调用 `create_task_with_labels_and_dependencies`。label、dependency、retry policy、metadata validity 与 initial readiness 仍在同一 SQLite transaction/service guard 中处理；任一失败都整体回滚。

### 4.3 List task windows by status

```http
GET /api/v1/boards/{board}/tasks/by-status?status=triage&status=ready&include_archived=false&limit=50&offset=0&sort=-updated_at
```

这个只读 endpoint 将 board column 查询批量化为一次请求，并接受 4.1 节定义的同一套严格
query 语法。每个重复 `status` 生成一个独立 task window；`limit` 与 `offset` 分别应用到
每个 window。response 中的 status 顺序与 URI 中的重复参数顺序一致；省略 `status` 时返回
空的 `statuses` array。

响应（以下为字段节选；完整、可消费的成功响应以 `schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json` 为准）：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "statuses": [
      {
        "status": "ready",
        "tasks": [
          {
            "id": "t_01HX...",
            "ref": "default#12",
            "status": "ready",
            "title": "实现状态机"
          }
        ],
        "page": {
          "limit": 50,
          "offset": 0,
          "total": 3
        }
      }
    ]
  },
  "meta": {
    "limit": 50,
    "offset": 0
  }
}
```

### 4.4 Get task

```http
GET /api/v1/tasks/{task_id}
```

`task_id` is the global `t_...` id and is not scoped by board. Responses include `board_id`, `board_slug`, and `ref` so clients can render copyable `board#seq` task refs.

Query params：

| Param | 说明 |
|---|---|
| `include` | 可选。当前识别 `ontology`；可用逗号分隔，其他 include 值暂时保持兼容性忽略。 |

默认响应只包含 `data: ApiTask`，不返回 `meta`。传 `include=ontology` 时，`data`
保持同一 `ApiTask`，并在 `meta.details.ontology_summary` 返回该 task 的 label
ontology signal 摘要；没有 ontology signals 时为 `null`。Summary 是只读 task-level
工作流提示，包含 signal/status/degraded/stale/action counts、oldest open/confirmed
signal time/age、latest signal/action time、当前 `suggest_input_hash` 和最多 5 条
sample signals（id/kind/status/proposed_action/score/stale/degraded/action count）。完整
queue/review 仍使用 `/label-ontology/signals`、`/label-ontology/review` 和
`/label-ontology/signals/{signal_id}`。

Task detail aggregate endpoints such as
`GET /api/v1/tasks/{task_id}/detail?include=dependencies,steps,runs,events,comments,neighborhood`,
panel-specific timelines, or execution-context bundles are not part of the
current API. They remain a follow-up optimization candidate for reducing
TaskDetail panel fan-out after the existing per-panel routes and cache
invalidation behavior are stable.

### 4.5 Update task fields

```http
PATCH /api/v1/tasks/{task_id}
```

允许字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "新的标题",
  "description": "新的描述",
  "assignee": "worker-a",
  "priority": 1,
  "scheduled_at": 1717520000000,
  "due_at": 1717600000000,
  "max_retries": 2,
  "metadata": {},
  "actor": "alice",
  "expected_lock_version": 7
}
```

`priority` updates reject values outside `0..3`.

`max_retries: null` 清空 retry policy。Task DTOs include `execution_plan_state`, `required_step_count`, `completed_required_step_count`, and `optional_step_count` so clients can show plan readiness without separately listing steps.

禁止字段：

- `status`
- `claim_token`
- `current_run_id`
- `completed_at`

`PATCH` 不能直接设置 canonical `status`；状态必须通过 transition endpoint 修改。
不过允许字段仍会走 shared service path。更新 `description`、`scheduled_at`
等影响 spec 或 schedule 的字段后，服务端可以根据 spec、schedule 和
当前 dependencies 重新计算 active task 的目标状态，并写入对应事件。
Dependency edge 必须通过 dependency endpoints 修改；`max_retries` 只更新
retry policy，不是 status recompute 触发器。

---

## 5. Transitions

Transition request 使用各 endpoint 独立的封闭 DTO，未知顶层字段返回 `400`，不共享通用
transition/token body。Promote、reclaim、unblock 和 task archive 可完全省略 body；出现
body 时仍按对应 DTO 校验。`actor` 的解析优先级保持为 body、`X-KB-Actor`、server default。
Claim 与 heartbeat 省略 `ttl_ms` 时使用 `300000`；reclaim、complete、submit-review、block
与 archive 省略 `force` 时均为 `false`，不能绕过 lease、token 或状态机 guard。Claim-token mismatch
response 不回显客户端提交的错误 token，也不暴露服务端保存的真实 token。

### 5.1 Specify

```http
POST /api/v1/tasks/{task_id}/transitions/specify
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "description": "补全后的规格",
  "scheduled_at": null,
  "actor": "alice"
}
```

### 5.2 Promote

```http
POST /api/v1/tasks/{task_id}/transitions/promote
```

Promote is rejected with `409 execution_plan_required` while the task execution plan is `unplanned`.

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "dispatcher"
}
```

### 5.3 Claim / Start

```http
POST /api/v1/tasks/{task_id}/transitions/claim
```

Claim/start is rejected with `409 execution_plan_required` while the task execution plan is `unplanned`.

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice",
  "ttl_ms": 300000,
  "worker_profile": null,
  "metadata": {}
}
```

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "status": "running"
    },
    "run": {
      "id": "r_01HX...",
      "status": "running"
    },
    "claim_token": "ct_01HX...",
    "claim_expires_at": 1717520300000
  }
}
```

### 5.4 Heartbeat

```http
POST /api/v1/tasks/{task_id}/transitions/heartbeat
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "ct_01HX...",
  "ttl_ms": 300000,
  "note": "still running",
  "actor": "worker-default"
}
```

Explicit heartbeat remains supported. For a `running` task, a later valid task-scoped activity event also refreshes the task lease and active run heartbeat as an implicit liveness signal; that implicit renewal does not emit an extra `task.heartbeat` event. Board-level events and events without `task_id` do not renew a task.

### 5.5 Complete

```http
POST /api/v1/tasks/{task_id}/transitions/complete
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "ct_01HX...",
  "summary": "实现完成，测试通过",
  "result": {},
  "force": false,
  "actor": "worker-default"
}
```

`result` 是可选 opaque JSON value；schema 只约束字段存在形式，不收紧其内部结构。

### 5.6 Submit Review

```http
POST /api/v1/tasks/{task_id}/transitions/submit-review
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "ct_01HX...",
  "summary": "等待人工检查",
  "force": false,
  "actor": "worker-default"
}
```

Submit-review 不接受 `result`；该字段与其它未知顶层字段一样返回 `400`。

### 5.7 Block

```http
POST /api/v1/tasks/{task_id}/transitions/block
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "等待 API schema 确认",
  "claim_token": null,
  "force": false,
  "actor": "alice"
}
```

### 5.8 Unblock

```http
POST /api/v1/tasks/{task_id}/transitions/unblock
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice"
}
```

Response target 由服务端计算，不由客户端指定。

### 5.9 Reopen

```http
POST /api/v1/tasks/{task_id}/transitions/reopen
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "重新执行修正验证失败",
  "actor": "alice"
}
```

只允许 reopen `done` task，`reason` 必填且不能为空。Response target 由服务端按 spec、schedule、dependency 和 execution plan readiness 重新计算；`completed_at` 会清空，`result_summary` / natural JSON `result` 保留（持久层仍存于 `result_json`）。事件 `task.reopened` 的 payload 包含 `from`、`to`、`reason` 和 `original_completed_at`。

直接依赖该 task 的 child 中，仅 `triage|todo|scheduled|ready` 会重新计算；`running|blocked|review|done|archived` 不隐式改写。

### 5.10 Reclaim

```http
POST /api/v1/tasks/{task_id}/transitions/reclaim
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "force": false,
  "to_status": "ready",
  "reason": "claim expired",
  "actor": "dispatcher"
}
```

`to_status` 是封闭枚举，只接受 `ready` 或 `blocked`；其它 task status 返回 `400`。

### 5.11 Archive

```http
POST /api/v1/tasks/{task_id}/transitions/archive
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "force": false,
  "actor": "alice"
}
```

---

## 6. Dependencies

### 6.1 Add dependency

```http
POST /api/v1/tasks/{child_task_id}/dependencies
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "parent_task_id": "t_01HX...",
  "actor": "alice"
}
```

Response status is `201 Created` when a new edge is inserted. Re-adding the
same parent/child edge is idempotent and returns `200 OK` with the same
dependency envelope; it does not write another `dependency.added` event or
recompute the child status again. Dependency changes may demote an invalid
`ready` child to `todo`, but they do not auto-promote `todo` children to
`ready`. Reopening a `done` parent recomputes direct children only when the child is `triage|todo|scheduled|ready`; it leaves `running|blocked|review|done|archived` children unchanged.

### 6.2 Remove dependency

```http
DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}
```

### 6.3 List dependencies

```http
GET /api/v1/tasks/{task_id}/dependencies
```

Add/remove/list dependency endpoints return the same dependency envelope. For
the existing wire shape, `parents` and `children` are full `ApiTask` arrays.
The additional `task` and `edges` fields provide a compact hydrated relationship
view with stable named parent/child objects.

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_child",
      "board_id": "b_default",
      "board_slug": "default",
      "ref": "default#2",
      "title": "child",
      "status": "todo"
    },
    "parents": [],
    "children": [],
    "edges": [
      {
        "parent": {
          "id": "t_parent",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#1",
          "title": "parent",
          "status": "done"
        },
        "child": {
          "id": "t_child",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#2",
          "title": "child",
          "status": "todo"
        }
      }
    ]
  }
}
```

### 6.4 Steps and execution plan

Steps are ordered execution-plan items owned by a task. A step can be plain
text, or it can link to an existing normal task for context. A linked task is
not a dependency edge: linking does not affect dependency readiness, and the
linked task status does not automatically complete the step. Step completion is
tracked independently with `todo | done | skipped`.

```http
GET /api/v1/tasks/{task_id}/steps
POST /api/v1/tasks/{task_id}/steps
PATCH /api/v1/tasks/{task_id}/steps/{step_id}
DELETE /api/v1/tasks/{task_id}/steps/{step_id}
POST /api/v1/tasks/{task_id}/steps/{step_id}/done
POST /api/v1/tasks/{task_id}/steps/{step_id}/skip
POST /api/v1/tasks/{task_id}/steps/{step_id}/reopen
POST /api/v1/tasks/{task_id}/execution-plan/not-required
```

Create request:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "Write acceptance checks",
  "body": "Cover dependency and plan guards",
  "linked_task_ref": "default#13",
  "position": 2048,
  "required": true,
  "actor": "alice"
}
```

`linked_task_ref` is optional. Omit it for a plain text step. When present, it
must resolve to a non-archived task on the same board and must not be the parent
task itself.

Update request:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "Write acceptance checks",
  "body": null,
  "linked_task_ref": "default#14",
  "unlink_task": false,
  "position": 4096,
  "required": false,
  "actor": "alice"
}
```

Step status requests:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "note": "Implemented and verified",
  "actor": "alice"
}
```

`skip` and `reopen` use the same envelope but name the text field `reason`.

Mark not required request:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "Small text-only cleanup",
  "actor": "alice"
}
```

Step list and mutation responses return the parent task step snapshot:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_parent",
    "steps": [
      {
        "id": "st_01HX...",
        "parent_task_id": "t_parent",
        "title": "Write acceptance checks",
        "body": "Cover dependency and plan guards",
        "linked_task": { "id": "t_child", "ref": "default#13" },
        "position": 2048,
        "required": true,
        "status": "todo",
        "resolution_note": null,
        "resolved_by": null,
        "resolved_at": null,
        "created_by": "alice",
        "created_at": 1717520000000,
        "updated_by": "alice",
        "updated_at": 1717520000000
      }
    ],
    "execution_plan": {
      "board_id": "b_01HX...",
      "task_id": "t_parent",
      "state": "planned",
      "reason": null,
      "updated_by": "system",
      "updated_at": 0
    }
  }
}
```

`POST /execution-plan/not-required` returns the execution plan record directly.
Missing linked task targets return `404 not_found`; self-links, cross-board
links, archived linked tasks, and empty titles return `400 invalid_input` in the
standard error envelope. Completing or archiving a parent with incomplete
required steps returns `409 steps_incomplete`. Required steps are complete for
that guard only when their step status is `done` or `skipped`.

### 6.5 Task neighborhood

```http
GET /api/v1/tasks/{task_id}/neighborhood?depth=1&limit_nodes=250&include_archived_context=false
```

This read-only endpoint returns the selected task, direct dependency parents,
direct dependency children, direct linked-step parents/children, and every dependency or step edge whose source and target are
both visible. V1 only accepts `depth=1`; deeper graph expansion is intentionally
reserved for later.

Response:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "center_task_id": "t_01HX...",
    "nodes": [
      {
        "task": { "id": "t_01HX...", "ref": "default#12", "status": "ready" },
        "role": "center",
        "context_only": false
      }
    ],
    "edges": [
      {
        "id": "dependency:t_parent->t_child",
        "source_task_id": "t_parent",
        "target_task_id": "t_child",
        "kind": "dependency",
        "required": true,
        "blocking": true
      }
    ],
    "meta": {
      "depth": 1,
      "context_depth": 0,
      "node_count": 1,
      "edge_count": 0,
      "truncated": false,
      "limit_nodes": 250,
      "include_archived_context": false
    }
  }
}
```

`task` uses the same public task DTO as task list/detail responses and does not
expose `claim_token`.

### 6.6 Board task map

```http
GET /api/v1/boards/{board}/task-map?active_only=true&context_depth=1&limit_nodes=250&include_done_context=true&include_archived_context=false&hide_isolated=false
```

This read-only endpoint returns an operational graph for the board. By default it
includes all active, non-archived tasks (`triage`, `todo`, `scheduled`, `ready`,
`running`, `blocked`, `review`) plus at most one dependency-hop of non-archived
context. Done context is included by default and marked `context_only`; archived
context is excluded unless explicitly requested. V1 only accepts
`context_depth=0` or `context_depth=1`.

Node roles are `active` for active board tasks and `context` for one-hop context.
Dependency and step edges are returned only when both endpoints are visible. Dependency edges use `kind=dependency`, `required=true`, and `blocking=true`; step edges use `kind=step`, preserve the step `required` flag, and set `blocking=false`. Pure text steps have no task node and therefore do not appear as graph edges. The `meta` object reports active statuses, node/edge counts, truncation, limit, and the query context flags.


---

## 7. Comments

### 7.1 List comments

```http
GET /api/v1/tasks/{task_id}/comments
```

Comments are task-id scoped. Listing comments remains available for archived boards because it is read-only audit history; creating comments on archived boards is rejected.

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "c_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "author": "alice",
      "author_type": "user",
      "agent_type": null,
      "body": "这里需要确认边界条件。",
      "kind": "note",
      "metadata": {},
      "created_at": 1717520000000
    }
  ]
}
```

### 7.2 Add comment

```http
POST /api/v1/tasks/{task_id}/comments
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "body": "这里需要确认边界条件。",
  "kind": "note",
  "author_type": "user",
  "agent_type": null,
  "author": "alice",
  "metadata": {}
}
```

Notes：

- `kind` 默认为 `note`，当前允许 `note|decision|signal`。
- `decision` records meaningful multi-option choices; body remains the readable fallback, and structured decision metadata is carried by `metadata`.
- `author_type` marks who produced the comment and allows `user|agent`. If omitted, the service defaults to `user`.
- `agent_type` is optional open text for `author_type=agent` comments, such as `executor` or `reviewer`. Non-empty `agent_type` with `author_type=user` is rejected as `400 invalid_input`.
- `metadata` 默认为 `{}`，必须是 JSON object；response 同样使用自然 JSON `metadata` object。普通 note/signal metadata 保持开放且无损，不能因为键名与专用协议碰撞而在 transaction 提交后收紧。`kind=decision` 时必须包含非空 `options`，每个 option 必须有非空 `slug` / `title` / `detail`，slug 必须是唯一小写 ASCII slug，`selected` 必须匹配 option slug，`reason` 必须非空，`risk` / `verification` 如果出现也必须非空。无效 decision metadata 返回 `400 invalid_input`。
- `author` 走通用 actor 语义；也可以用 `X-KB-Actor` 或 server 默认 actor。
- 创建评论会写入 `task.comment.created` event。


### 7.3 B2-C3 comments wire contracts

`GET` 与 `POST /api/v1/tasks/{task_id}/comments` 各自拥有独立、闭合的 path 与 success root；
`POST` 另有独立、闭合的 request root。两者只共享 contract-owned `ApiComment` component 与
既有 shared error component。GET 没有 query/body，POST 没有 query；B7 已为两个 endpoint
登记并采用精确 header contract。endpoint surface migration 仍诚实标记为 `Generated`，直到
后续 API adoption cohort 补齐真实 router producer/contract consumer 的 exact surface evidence。

`ApiComment.author_type` 仅允许 `user|agent`，`kind` 仅允许 `note|decision|signal`，
`agent_type` 是必须出现但可为 `null` 的字段。`metadata` 是开放、无损的 response object。
create request 的 `metadata` 保持开放 JSON object；decision 的精确 typed shape 由独立的
`metadata.decision.input` / `NoTransport` contract 与真实 CLI producer/consumer witnesses 拥有。
运行时原始 JSON object 继续进入 SQLite service 的 decision cross-field guard，schema 不替代
selected/option uniqueness、slug、非空值、board archive 与 transaction/event 约束。

---

## 8. Runs

### 8.1 List task runs

```http
GET /api/v1/tasks/{task_id}/runs
```

Run listing is task-id scoped and remains available for archived boards as read-only audit history.

### 8.2 Get run

```http
GET /api/v1/runs/{run_id}
```

### 8.3 Get run log

```http
GET /api/v1/runs/{run_id}/log
```

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "run_id": "r_01HX...",
    "content": "worker output\n",
    "truncated": false
  }
}
```

Notes：

- Response 不包含 `claim_token`。
- 当前最多返回末尾 256 KiB；更大的 log 会设置 `truncated: true`。
- 若 run 没有 `log_path` 或文件不存在，返回 `not_found`。
- 若 `log_path` 不在受信任日志目录或文件名不匹配 `<run_id>.log`，返回 `invalid_input`。

---

## 9. Stats

### 9.1 Queue stats

```http
GET /api/v1/stats?board=default
```

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "board_id": "b_01HX...",
    "generated_at": 1717520000000,
    "status_counts": [
      {"status": "ready", "count": 3},
      {"status": "running", "count": 1}
    ],
    "stale_claims": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "title": "stale worker",
        "claim_owner": "dispatcher",
        "claim_expires_at": 1717520000000,
        "last_heartbeat_at": 1717519900000,
        "current_run_id": "r_01HX...",
        "retry_count": 1,
        "max_retries": 3
      }
    ],
    "blocked_reasons": [
      {"reason": "waiting on operator", "count": 2}
    ],
    "unplanned_active_tasks": 4,
    "active_parents_with_incomplete_required_steps": 1
  }
}
```

Notes：

- `stale_claims` 只包含 `running` 且 `claim_expires_at <= now` 的任务。
- `blocked_reasons` 按数量降序、reason 升序排序。

---

## 10. Events

### 10.1 List events

```http
GET /api/v1/events?board=default&after=0&limit=100
```

`board` accepts board slug or id. Events for archived boards remain readable so clients can inspect the audit trail after archive.

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": 123,
      "event_id": "e_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "run_id": "r_01HX...",
      "kind": "task.claimed",
      "actor": "alice",
      "payload": {"claim_owner":"alice","metadata":{}},
      "created_at": 1717520000000
    }
  ],
  "meta": {
    "next_after": 123
  }
}
```

### 10.2 SSE stream

```http
GET /api/v1/stream/events?board=default&after=123
```

SSE event：

```text
event: task.claimed
id: 124
data: {"id":124,"event_id":"e_...","board_id":"b_...","task_id":"t_...","run_id":"r_...","kind":"task.claimed","actor":"alice","payload":{"claim_owner":"alice","metadata":{}},"created_at":1717520000000}
```

`board`、`task_id`、`after`、`limit` 是该 endpoint 唯一接受的 query key，均只能出现一次；
未知或重复 key 返回标准 `400 invalid_input` envelope。默认值分别为 `default`、未提供、`0`、
`100`，runtime 将 `limit` 防御性限制到 `1000`。每个事件严格按 `event`、`id`、`data`
frame 顺序输出；`data` 是完整的 `StreamEventData` JSON，不允许额外字段。
`task_id`、`run_id`、`actor` 都是 required-nullable：键必须出现，值可以显式为 `null`。
39 个 known kind 的 payload 与 kind 使用同一 tagged union；缺字段、额外字段或 sibling
status/state 错配会 fail closed。未来 unknown kind 的合法 JSON payload 保持 lossless。

Reconnect：

- V1 implementation emits a finite snapshot of existing matching events and closes; clients should reconnect or poll `GET /api/v1/events` for updates.
- Browser clients may send Last-Event-ID, but V1 only honors the `after` query parameter.
- V1 finite snapshot 不发送 SSE comment/heartbeat frame；因此 heartbeat 不是 JSON payload
  contract，`Last-Event-ID` 也不是已采用的 header input contract。这两项只有未来 runtime
  真正实现后才能迁移为 typed contract。
- 若 event 已被压缩/清理，客户端重新 fetch board snapshot。

---

## 11. Columns / UI Settings

### 11.1 List columns

```http
GET /api/v1/boards/{board}/columns
```

### 11.2 Update columns

```http
PATCH /api/v1/boards/{board}/columns
```

Request：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "columns": [
    {"id": "col_triage", "title": "Triage", "position": 10, "hidden": false},
    {"id": "col_done", "title": "Done", "position": 80, "hidden": false}
  ]
}
```

MVP 不允许 column 改变 canonical status。

---

## 12. 标签 API

```http
GET /api/v1/boards/{board}/labels
POST /api/v1/boards/{board}/labels
GET /api/v1/boards/{board}/labels/semantics
GET /api/v1/boards/{board}/labels/{label_id}/semantics
PUT /api/v1/boards/{board}/labels/{label_id}/semantics
DELETE /api/v1/boards/{board}/labels/{label_id}/semantics?expected_semantics_hash=<hash>&reason=<text>
GET /api/v1/boards/{board}/labels/atoms
GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain
GET /api/v1/boards/{board}/labels/atom-index/status
POST /api/v1/boards/{board}/labels/atom-index/rebuild
GET /api/v1/boards/{board}/labels/atom-index/query?q=<text>&polarity=positive&limit=24
GET /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels/bootstrap
DELETE /api/v1/tasks/{task_id}/labels/{label_id}
GET /api/v1/boards/{board}/signals
GET /api/v1/boards/{board}/signals/review
GET /api/v1/signals/{signal_id}
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals
GET /api/v1/boards/{board}/label-ontology/review
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/revert
POST /api/v1/boards/{board}/label-ontology/validate
```

Board 级标签创建请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "core",
  "color": "blue"
}
```

Label 响应结构，用于 board 级标签创建和 label 列表：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "id": "l_01HX...",
  "board_id": "b_01HX...",
  "name": "core",
  "color": "blue",
  "created_at": 1717520000000,
  "updated_at": 1717520000000
}
```

`POST /api/v1/boards/{board}/labels` 按 board 作用域创建 label，并按 label
名称保持幂等。如果该 board 上已存在同名 label，响应返回已有 label。空白 name
会被拒绝。Base label identity CRUD 属于 vocabulary registry，不属于 ontology
ledger；创建 label identity 不写 `label_ontology_actions`，也不会创建
`label_semantics` 或 `label_atoms`。

Task 标签添加请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "core"
}
```

或批量添加：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "names": ["core", "api"]
}
```

如果需要在绑定时显式创建缺失 label identity：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "names": ["scratch-label"],
  "create_missing": true
}
```

`POST /api/v1/tasks/{task_id}/labels` 会把指定 name 或 names 的 label 绑定到 task。
`name` 与 `names` 互斥；二者都缺失、二者同时出现或 `names` 为空数组都会返回
invalid input。批量添加在同一 transaction 内执行，并先验证所有 label 名称；如果
任一 label 为空白或非法，不会创建 canonical label，也不会留下部分 task-label 绑定。
默认情况下，如果该 task 所属 board 上还不存在指定 name 的 label，请求会返回
invalid input，且不会增加 `labels` 或 `task_labels` 记录。传入
`"create_missing": true` 时，API 会只创建缺失的 canonical label identity，并绑定到
task；不会生成 `label_semantics` 或 `label_atoms`。重复绑定已有 task-label 关系不会
重复写入。成功响应返回更新后的 task，包含当前 `labels` 列表；显式创建模式下如果
本次创建了 label，响应 `meta.created_labels` 会列出新建 labels。

Task label bootstrap 请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "database",
  "description": "Database persistence work",
  "applies_when": ["touches SQLite migrations"],
  "excludes_when": ["UI-only polish"],
  "positive_examples": ["new table migration"],
  "negative_examples": ["CSS-only tweak"],
  "actor": "alice"
}
```

`POST /api/v1/tasks/{task_id}/labels/bootstrap` 是一次性 new-label adoption API：
在同一 transaction 内创建 task 所属 board 上缺失的 canonical label，或复用没有既有
semantics 的同名 label，写入该 label 的 `label_semantics`，同步重建 SQLite
`label_atoms`，标脏派生的 label atom vector index，并把该 label 绑定到 task。
`name` 按 label 名称解析；空白名称会被拒绝。语义输入会 trim 并丢弃空白值，且必须至少
提供 `description` 或一个非空语义数组值。

Bootstrap API 默认不会覆盖已有 `label_semantics`。如果同名 label 已经有
semantics，请求会失败，并要求调用方改用专用 semantics mutation 或
proposal/adoption 路径；重复调用同一 task/label 只在目标 label 仍无 semantics 时保持
task-label 绑定幂等。成功响应状态为 `201 Created`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "ref": "default#12",
      "labels": [
        {"id": "l_01HX...", "board_id": "b_01HX...", "name": "database", "color": null}
      ]
    },
    "semantics": {
      "label_id": "l_01HX...",
      "board_id": "b_01HX...",
      "label_name": "database",
      "description": "Database persistence work",
      "applies_when": ["touches SQLite migrations"],
      "excludes_when": ["UI-only polish"],
      "positive_examples": ["new table migration"],
      "negative_examples": ["CSS-only tweak"],
      "atoms": []
    }
  }
}
```

HTTP bootstrap 不包含 CLI `--verify` 的 orchestration：请求体没有 vector config、
minimum score 或 verify flag，响应也没有 `verification` 字段。该 endpoint 不会替调用方
重建 label atom vector index、运行 `label suggest` 或检查分数门槛；需要 pre-commit
staged verification 的零写入失败语义时使用 CLI `label bootstrap --verify`。API 调用后如需
诊断，可显式执行 index rebuild / suggest / review 流程，但这不具备 CLI staged verifier 的
同一事务 adoption contract。

`DELETE /api/v1/tasks/{task_id}/labels/{label_id}` 会移除 task 上的指定 label，
`{label_id}` 接受 label id 或 label 名称。成功响应同样返回更新后的 task，包含
当前 `labels` 列表。只有关联行发生变化时，label attach/remove 才写入 task
label event；该操作不改变 task status。

### 12.1 Label semantics, atoms, and atom index

`GET /api/v1/boards/{board}/labels/semantics` 返回当前 board 已定义 semantics 的
列表。`GET /api/v1/boards/{board}/labels/{label_id}/semantics` 返回单个 label
semantics；`{label_id}` 只接受 canonical `l_...` label id。Label name 允许包含
`/` 等 path 不安全字符，因此 semantics API path 不支持按 label name 寻址；需要按
名称查找时，先调用 `GET /api/v1/boards/{board}/labels` 获取对应 id。

`PUT /api/v1/boards/{board}/labels/{label_id}/semantics` 写入已有 label 的语义字典，
同步重建该 label 的 SQLite `label_atoms`，并标脏派生的 label atom vector index。
请求 body：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice",
  "expected_semantics_hash": "optional-current-hash",
  "replace": false,
  "reason": "Add a repeated boundary observed during label review",
  "source_signal_ids": ["los_..."],
  "description": "Backend service work",
  "applies_when": ["touches Rust service code"],
  "excludes_when": ["CSS-only"],
  "positive_examples": ["add API handler"],
  "negative_examples": ["adjust spacing"],
  "remove_applies_when": [],
  "remove_excludes_when": [],
  "remove_positive_examples": [],
  "remove_negative_examples": []
}
```

默认 `replace=false`，请求按 patch 语义处理：`description` 只在提供非空值时覆盖当前
description，数组字段会追加到对应集合，`remove_*` 数组删除匹配文本；缺省字段不会清空
已有 semantics。传 `replace=true` 时才完整替换五个语义字段，此时缺省数组视为空数组，
并且不能同时传任何 `remove_*` 字段。`expected_semantics_hash` 是 CAS guard；如果与
当前 `semantics_hash` 不一致，请求返回 conflict 且不写入。服务会 trim 并丢弃空白值。
每次实际改变 canonical semantics/atoms 的 constructive semantics write 都会在同一
SQLite transaction 写入一条 `update_semantics` root ontology action，记录 actor、reason、
source signal links（如有）、before/after hash 和单份 change snapshot；实际 added/removed
atoms 通过 `label_ontology_action_atom_effects` 写 `added` / `removed` rows。Description-only
patch 会写一条 root action 和零 atom effects；no-op patch 不写 action/effects，也不标脏
label atom index。生成 atoms 时，有 description
的 label 会生成一个 canonical `description` atom：
`label: {name}\ndescription: {description}`；没有 description 时才使用 `name`
fallback atom。atom text 会进一步规范化 whitespace：每个非空行内部 collapse，
canonical 行分隔保留。同一 label 下相同 `polarity + kind + normalized_text` 的 atom
会去重并保留首次 ordinal，`id` / `content_hash` 不包含 ordinal，因此只调整数组顺序
不会改变同一文本 atom identity。响应使用 `DataEnvelope`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "label_id": "l_01HX...",
    "board_id": "b_01HX...",
    "label_name": "backend",
    "description": "Backend service work",
    "applies_when": ["touches Rust service code"],
    "excludes_when": ["CSS-only"],
    "positive_examples": ["add API handler"],
    "negative_examples": ["adjust spacing"],
    "created_at": 1717520000000,
    "updated_at": 1717520000000,
    "atoms": [
      {
        "id": "la_...",
        "label_id": "l_01HX...",
        "board_id": "b_01HX...",
        "label_name": "backend",
        "polarity": "positive",
        "kind": "applies_when",
        "text": "touches Rust service code",
        "ordinal": 2,
        "content_hash": "...",
        "created_at": 1717520000000,
        "updated_at": 1717520000000
      }
    ]
  }
}
```

`DELETE /api/v1/boards/{board}/labels/{label_id}/semantics?expected_semantics_hash=<hash>&reason=<text>`
是 CAS-protected semantics clear：`expected_semantics_hash` 与非空 `reason` 都必填。
它删除该 label 的 semantics 与 SQLite atoms，但不删除 canonical label identity 或
task-label binding；同一 transaction 写一条 `update_semantics` root ontology action，
after snapshot 为空，并为实际 removed atoms 写 `removed` atom effects，随后标脏 label
atom index。Hash mismatch 时 canonical、action、effects 和 dirty state 全不变。成功返回：

```http
DELETE /api/v1/boards/default/labels/l_01HX/semantics?expected_semantics_hash=sem_abc123&reason=Retire%20obsolete%20semantics
X-Kanban-Actor: alice
```

<!-- schema-doc: contract=api.label-semantics-delete.response fixture=schemas/fixtures/api/delete-response.v1.valid.json -->
```json
{ "data": { "deleted": true } }
```

`GET /api/v1/boards/{board}/labels/atoms` 返回 SQLite `label_atoms` materialized
projection。它由 `label_semantics` 和 label name 展开、随 semantics mutation 同事务重建，
是 `lancedb_label_atoms` 派生索引的输入；不要把它描述成独立于 semantics 的第二份
semantic truth。

`GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain` 按当前 atom id 或稳定
`content_hash` 解析 atom，并返回 `LabelAtomExplainRecord`：`query`、`atom`、
`current_semantics`、`provenance_actions`、`supporting_signals`、
`validation_history`、`legacy_untracked` 和 `legacy_reason`。当前 atom 存在但没有
ontology provenance action 引用其 id 或 content hash 时返回 `200` 且
`legacy_untracked=true`；未知 id/hash 返回 not found。

`GET /api/v1/boards/{board}/labels/atom-index/status` 返回 label atom vector index
状态。server no-heavy route 通过 vector helper adapter 报告当前 helper 能力。无 vector provider、
adapter 不可用或 helper 缺失时仍返回 `200` disabled 状态。JSON 保留兼容字段 `message`，并额外返回结构化
`diagnostics: string[]`、`dirty: boolean | null`、`board_dirty: boolean | null`；调用方应使用结构化字段判断 dirty/error，
而不要解析 `message` 文案。同一 `VectorStoreStatus` shape 也用于 `/api/v1/vector/status`。

`POST /api/v1/boards/{board}/labels/atom-index/rebuild` 通过 vector helper adapter 调用
label atom 专用 `rebuild-label-atoms` helper command，重建 `lancedb_label_atoms` 派生索引并更新
`label_atom_index_boards` / `lancedb_label_atoms` status。helper/provider 缺失返回显式 API error，
不得写 canonical label truth，也不得把 chunk store status 当作 label atom rebuild success。
`GET /api/v1/boards/{board}/labels/atom-index/query` 通过 vector helper adapter 查询派生的
`lancedb_label_atoms` 索引。请求必须提供 `q=<text>` 或 `vector_json=<json-array>` 之一，二者互斥；
`embedding_model` 可选，`include_vector=true` 可要求 raw vector hit 返回向量，`polarity` 可选且只接受
`positive` / `negative`，`limit` 默认 24。hit 中的 `distance` 是 LanceDB `_distance`，不是 solver
similarity score。未配置 provider、adapter/helper 不可用或 vector store 不可用时，query 返回显式 API error，
不修改 SQLite truth。

### 12.2 Task label suggestions

```http
GET /api/v1/tasks/{task_id}/labels/suggestions?limit=5&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15
```

返回 task-level label suggestions。带可用 label atom vector store 的部署使用 task title + description embedding
查询 `lancedb_label_atoms`：正向 atoms 按 residual 多轮检索，负向 atoms 固定用原始
query 检索并做 penalty / suppression。solver 在 label group 层执行 Group OMP 选择，
再把选中 label 的 top positive atom vectors 作为 basis 做 non-negative refit；
`coverage` / `residual_norm` 来自 atom-level fitted vector，其中
`coverage = clamp(1 - residual_norm, 0.0, 1.0)`，因此二者不是两份独立证据；
`coverage_cosine` 是原始 query 与 fitted vector 的 cosine similarity，可作为
独立补充指标。候选 label 只有在
tentative refit 后带来足够 residual norm 降幅才会进入结果；coverage 或
residual norm 达到停止阈值后，solver 会提前停止而不是凑满
`max_selected_labels`。candidate group 与已选 label 语义向量过度相似时会被跳过，
以减少重复语义 label 同时出现在 `selected_labels`；这不会合并或删除 canonical
labels。`needs_new_label` 是兼容字段，只表示存在需要人工 review 的 label
coverage 诊断；具体原因必须读取 `reason_codes`，并结合 evidence atoms、
diagnostics 与人工语义判断，不应仅凭该布尔值创建 vocabulary。接口不会创建新
label，也不会写入 `label_semantics` / `label_atoms`。

`limit` 只控制 response 中 `selected_labels` / `candidates` 的最大条数，不会收窄
solver 内部搜索能力。内部能力由 `candidate_limit`、`atom_limit` 和
`max_selected_labels` 分别控制：候选 label group 数、每轮 atom vector 检索上限、
以及最多进入 non-negative refit 的 label 数。所有 limit 参数都必须是
`1..=1000`；`min_score` 必须在 `0..=1`。

未配置 provider、label vector adapter/feature 不可用、LanceDB 表缺失、索引为空或索引
dirty 时，接口仍返回 `200` 和结构化 degraded JSON；普通 label CRUD、task
list/search/filter 与状态转移不受影响。Dirty 判断来自结构化 status/SQLite dirty
字段，不依赖 `message` 文案。无 provider 时 `needs_new_label=false`，
避免把 #105 的新 label 创建流程误触发。

Response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_01HX...",
    "board_id": "b_01HX...",
    "selected_labels": [
      {
        "label_id": "l_01HX...",
        "label_name": "backend",
        "score": 0.82,
        "weight": 0.82,
        "already_applied": false,
        "evidence_atoms": [
          {
            "atom_id": "la_...",
            "label_id": "l_01HX...",
            "label_name": "backend",
            "polarity": "positive",
            "kind": "applies_when",
            "text": "touches server code",
            "score": 0.91
          }
        ],
        "negative_evidence_atoms": []
      }
    ],
    "candidates": [],
    "coverage": 0.82,
    "coverage_cosine": 0.91,
    "residual_norm": 0.18,
    "needs_new_label": false,
    "reason_codes": ["degraded_result", "label_atom_index_dirty"],
    "degraded": true,
    "diagnostics": ["label_atom_index_dirty"]
  }
}
```

稳定 diagnostics 包括：

- `vector_store_disabled`
- `label_atom_index_dirty`
- `label_atom_index_empty`
- `label_atom_index_error`
- `vector_query_error`

非 degraded coverage review 的稳定 `reason_codes` 包括：

- `no_selected_labels`
- `coverage_below_threshold`
- `residual_above_threshold`
- `unexplained_residual`

### 12.3 Label semantic proposals

```http
POST /api/v1/tasks/{task_id}/label-proposals?limit=5&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15
GET /api/v1/tasks/{task_id}/label-proposals
GET /api/v1/label-proposals/{proposal_id}
POST /api/v1/label-proposals/{proposal_id}/accept
POST /api/v1/label-proposals/{proposal_id}/reject
```

`POST /api/v1/tasks/{task_id}/label-proposals` 创建一次新 label proposal attempt。
请求 body 可为空或仅包含 `actor`；此时默认 provider 不可用，接口返回 `200`
degraded attempt，不创建 canonical label、`label_semantics`、`label_atoms` 或
`task_labels`。

Provider boundary：API 当前只支持空/default provider 或请求 body 中显式传入的
本地/offline candidate。真实 LLM provider 不在 `kanban-sqlite` 中实现；如果未来
server 支持本机 AI/runtime，它必须在 server/local/独立 AI crate 层实现
`LabelProposalProvider` adapter，并把 candidate 交给 SQLite service 做 deterministic
validation 和 persistence。

带本地/offline provider 输出时：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "proposal": {
    "name": "database",
    "description": "Database persistence work",
    "applies_when": ["touches SQLite migrations"],
    "excludes_when": ["UI-only polish"],
    "positive_examples": ["new table migration"],
    "negative_examples": ["CSS-only tweak"]
  },
  "actor": "alice",
  "source_signal_ids": ["los_..."],
  "ontology_actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "allow_retarget": false,
  "retarget_reason": null
}
```

数组字段缺省时按空数组处理。服务先读取当前 label suggestion 的启发式
`coverage` / `coverage_cosine` / `residual_norm` / top1 existing label。coverage 充足时不写 proposal；
coverage 不足且候选语义有效，并且残差 top1+margin 校验明确通过时，返回 `201` 并持久化
`proposed` proposal。与现有 label 发生 normalized-name 冲突的候选持久化为 `rejected`，diagnostics 包含
`near_duplicate_label_conflict`。Normalized-name conflict 忽略大小写、空白和标点，
是 deterministic near-duplicate heuristic。
`source_signal_ids` 可选；传入时，proposal 创建成功后会在同一 transaction 写入
`create_label_proposal` ontology action，并通过 action-signal links 记录该 proposal
由哪些 confirmed vocabulary-gap signals 支持。Proposal row 与 provenance action
要么同时写入，要么一起回滚。Source signals 默认必须属于同一 board、状态为
`confirmed`、kind 为 `vocabulary_gap`、`proposed_action` 为 `bootstrap_label`，且
normalized `proposed_label_name` 等于 proposal name。`ontology_actor` 只控制
`create_label_proposal` action provenance；省略时使用 `actor` 字符串作为
`type=user` actor。确需 retarget confirmed same-board source signal 时，必须传
`allow_retarget=true` 和非空 `retarget_reason`；reason 和 source signal 原始
target/proposed label 会写入 `change_json.retarget_override`。Override 不放宽
board/status 要求。

POST proposal route 接受与 label suggestion 相同的 query 参数。`limit` 只截断
suggestion 输出；`candidate_limit`、`atom_limit`、`max_selected_labels` 和 `min_score`
调节用于 heuristic coverage / residual validation 的底层 solver。

当 server 配置了可用 vector provider 时，proposal attempt 与 label suggestion
使用同一套 LanceDB label atom store。coverage 不足的候选会在持久化前执行残差
top1+margin 校验：候选语义的 residual score 和现有 label top1 都按返回 atom
vector 在本地计算 cosine similarity，不从 LanceDB distance 推导；候选必须超过现有
label top1，且超过幅度达到固定 margin。校验失败时候选仍会以 `rejected` proposal 持久化，diagnostics
包含 `label_proposal_residual_top1_failed` 或
`label_proposal_residual_margin_insufficient`。未配置 provider、feature 不可用或
vector 检索失败时返回 degraded attempt，不创建 canonical label、`label_semantics`、
`label_atoms` 或 `task_labels`。如果 residual validation 不可用或 degraded，且没有
明确通过 top1+margin 校验，attempt 返回 `proposal=null`，不新增 proposal row，
diagnostics 包含 `label_proposal_residual_validation_unavailable` 和具体原因。

Attempt response：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_...",
    "board_id": "b_...",
    "proposal": null,
    "degraded": true,
    "diagnostics": ["label_proposal_provider_unavailable", "vector_store_disabled"],
    "heuristic_coverage": 0.0,
    "heuristic_coverage_cosine": 0.0,
    "heuristic_residual_norm": 1.0,
    "top1_existing_label_id": null,
    "top1_existing_label_name": null
  }
}
```

Accept/reject body：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "coverage 不足，接受新 label",
  "actor": "alice",
  "source_signal_ids": ["los_..."],
  "ontology_actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "allow_retarget": false,
  "retarget_reason": null
}
```

Accept 只允许 `proposed` proposal。成功后会通过与 task-label bootstrap 相同的 adoption
primitive 创建 canonical `labels` 行与对应 `label_semantics` / `label_atoms`，
标脏 label atom index，并在同一 transaction 写入一条 `bootstrap_label` root ontology
action 和对应 added atom effects；
proposal status、canonical writes 和 provenance action 要么一起成功，要么一起回滚。
它不会自动写 `task_labels`。`source_signal_ids` 可选；省略时仍记录 bootstrap action，
但没有 action-signal links。传入时，accept 会通过 action-signal links 记录 new-label
bootstrap provenance。Source signals 必须属于同一 board 且处于 `confirmed`。
`actor` 字符串仍用于 proposal decision event；`ontology_actor` 只控制 accept 产生的
`bootstrap_label` ontology action provenance。省略 `ontology_actor` 时，bootstrap
action 使用 `actor` 字符串作为 `type=user` actor。`type=agent` 必须提供非空
`agent_type`；`type=user` 不能提供 `agent_type`。Source signals 默认还必须是
`vocabulary_gap` + `bootstrap_label`，且 normalized `proposed_label_name` 必须等于
proposal name。确实需要 retarget confirmed same-board source signal 时，必须传
`allow_retarget=true` 和非空 `retarget_reason`；bootstrap action
`change_json.retarget_override` 会记录 reason、source signal 原始 target/proposed
label 和最终 proposal/result label。如果 proposal 已有 `create_label_proposal`
action，accept 产生的 `bootstrap_label` action 会把 `parent_action_id` 指向该
creation action。Override 不放宽 board/status 要求。Reject 标记为
`rejected`，不接受 `source_signal_ids`、`ontology_actor` 或 retarget options。
accepted/rejected proposal 再次决策返回普通 `400 invalid_input` error envelope。

### 12.4 Generic signal ledger

Generic signal ledger API 提供 board-scoped 只读 inbox，用于展示 agent/product
在 kanban 工作流中记录的通用 signal，例如 CLI 参数摩擦、提示误导、参数设计问题或
operator 发现。它独立于 label ontology ledger；这些 endpoint 不创建、确认、拒绝、
resolve 或 supersede signal，也不会把通用 signal 混入 ontology review groups。

```http
GET /api/v1/boards/{board}/signals?status=open&kind=agent_cli_friction&task_ref=default%23123&include_all=false&limit=100
GET /api/v1/boards/{board}/signals/review?status=confirmed&kind=agent_cli_friction&task_ref=default%23123&include_all=false&limit=100
GET /api/v1/signals/{signal_id}
```

`GET /api/v1/boards/{board}/signals` 和 `/signals/review` 返回同一只读 DTO；
`review` endpoint 是 Desktop / operator console 的语义化入口。默认只返回 `open`
和 `confirmed`。可重复传 `status` 和 `kind`，并按 `task_ref` 过滤。
`include_all=true` 且没有显式 `status` 时返回完整历史；`limit` 使用普通列表上限。
这些 list/review routes 是 board-scoped surface；只返回该 board 的 signal rows。
`GET /api/v1/signals/{signal_id}` 是 operator-wide detail lookup，用于从 backlink、
inbox row 或审计记录直接打开已知 signal。该 detail route 不改变 signal 的
`board_id` truth，也不让 board-scoped list/review 泄漏其它 board 的 signal。

`signal_observations.task_id`、`run_id` 和 `comment_id` 是 provenance/history soft refs。
当前 service 写入路径、doctor 和 import final gate 维护这些 refs 与 observation board
的一致性；未来若需要把所有来源关系硬化，可迁移为 board-composite FK。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "sig_...",
      "board_id": "b_...",
      "observation_id": "obs_...",
      "kind": "agent_cli_friction",
      "title": "--require 参数命名不符合 agent 惯用预期",
      "summary": "agent 尝试使用 --required/--requires，实际 CLI 只接受 --require。",
      "severity": "medium",
      "status": "open",
      "dedupe_key": "kanban-task-create-require",
      "superseded_by_signal_id": null,
      "reviewed_by": null,
      "reviewed_at": null,
      "review_reason": null,
      "created_at": 1782930000000,
      "updated_at": 1782930000000,
      "observation": {
        "id": "obs_...",
        "board_id": "b_...",
        "task_id": "t_...",
        "task_ref_snapshot": "default#123",
        "run_id": "r_...",
        "comment_id": null,
        "actor": "codex",
        "agent_type": "codex",
        "source": "codex-hook",
        "evidence": {"command":"kanban task create --required ..."},
        "created_at": 1782930000000
      }
    }
  ]
}
```

`GET /api/v1/signals/{signal_id}` 返回单条 signal：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "id": "sig_...",
    "observation": {}
  }
}
```

### 12.5 Label ontology ledger

Label ontology ledger API 记录 task 标注过程、review queue、ontology mutation
provenance 和 validation history。Ledger 不会自动修改 task labels；canonical
binding 仍通过 task label API 或 CLI 完成。

所有 ontology actor object 使用 `{ "name": string, "type": "user"|"agent",
"agent_type": string|null }`。`type=agent` 必须提供非空 `agent_type`；
`type=user` 必须省略或传 `null`。

```http
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals?status=open&kind=false_negative&task_ref=default%2312&target_label_ref=cli&proposed_label_name=database&include_all=false&limit=100
GET /api/v1/boards/{board}/label-ontology/review?group_by=label&include_all=false&limit=100
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/revert
POST /api/v1/boards/{board}/label-ontology/validate
```

`POST /api/v1/tasks/{task_id}/label-ontology/observations` 在一个 transaction 中写入
observation 和 child signals。HTTP endpoint 不自行运行 `label suggest`；调用方必须传入
由工具采集且未改写的 `suggestion_snapshot`，或在没有 suggest 证据时显式传空 snapshot。
服务端会从 snapshot 派生 observation metrics，agent/reviewer 只提交候选、最终判断、
signals、candidate atom 和 rationale。请求 body：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
  "agent_candidates": [],
  "suggestion_snapshot": {
    "selected_labels": [],
    "coverage": 0.61,
    "coverage_cosine": 0.74,
    "residual_norm": 0.39,
    "needs_new_label": false,
    "degraded": false,
    "diagnostics": []
  },
  "final_decision": {},
  "capture_fingerprint": "optional-stable-key",
  "signals": [
    {
      "kind": "false_negative",
      "target_label_ref": "cli",
      "related_labels": [],
      "proposed_action": "add_positive_atom",
      "candidate_atom": {
        "polarity": "positive",
        "kind": "applies_when",
        "text": "extends CLI subcommands, command arguments, help output, or machine-readable JSON behavior"
      },
      "proposal": {},
      "agent_selected": true,
      "suggest_state": "candidate",
      "suggest_score": 0.08,
      "suggest_rank": 6,
      "final_selected": true,
      "rationale": "The task expands the CLI surface.",
      "confidence": 0.9
    }
  ]
}
```

HTTP ontology DTOs use natural JSON fields for new clients:
`agent_candidates`, `suggestion_snapshot`, `final_decision`, `diagnostics`,
signal `related_labels` / `proposal`, action `change` / `validation`, and
validate `validation`. Legacy escaped-string request siblings (`related_labels_json`,
`proposal_json`, `change_json`, `validation_json`, and the observation `*_json`
aliases) are no longer accepted by the public HTTP API. Unknown legacy keys fail
closed with `400 invalid_input`; clients must send the natural JSON fields. When `suggestion_snapshot`
contains `coverage`, `coverage_cosine`, `residual_norm`, `needs_new_label`,
`degraded`, or `diagnostics`, the server derives the stored observation metrics
from that snapshot. If the request also supplies the matching top-level
`suggest_*` field or `diagnostics` and the values conflict, the request returns
`400 invalid_input`. New clients should not repeat snapshot facts as top-level
scalars.

Service 会读取当前 task snapshot、解析 `target_label_ref`、计算 normalized proposed
label name、signal key 和 candidate atom content hash。`capture_fingerprint` 为空时
按 task、snapshots 和 signals 派生；同一 board 重复 fingerprint 会被唯一约束拒绝。
Observation response 返回 created observation，并展开 child `signals`。Observation
包含完整审计用 `task_snapshot_json.content_hash`，以及只基于 label suggest 输入
（normalized title + description）的 `suggest_input_hash`；后者用于后续 validation
comparability。

Signal 输入会在写入前做 ontology contract 校验。`candidate_atom` 的
`applies_when` / `positive_example` 只能使用 `positive` polarity，
`excludes_when` / `negative_example` 只能使用 `negative` polarity。
`add_positive_atom` 必须提供 target label 和 positive candidate atom；
`add_negative_atom` 必须提供 target label 和 negative candidate atom；
`update_semantics` 必须提供 target label；`bootstrap_label` 必须提供
`proposed_label_name`；`rename_label` 必须提供 target label 和
`proposed_label_name`；`split_label` / `merge_labels` 必须提供 target label 和非空
`related_labels`。Observation metric `suggest_coverage`、
`suggest_coverage_cosine`、`suggest_residual_norm` 以及 signal metric
`suggest_score` / `confidence` 必须是 finite `0.0..=1.0`；`suggest_rank` 必须为
`null` 或 `>= 1`。违反这些契约的 request 返回 `400 invalid_input`，不会写入
observation 或 signals。`rename_label` / `split_label` / `merge_labels` 当前只作为
review signal proposed_action 保存，不能通过 public HTTP route 写入 canonical structure
mutation action；旧 structure-plan rows 只读展示为 unsupported validation requirement。

`GET /api/v1/boards/{board}/label-ontology/signals` 默认只返回 `open` 和
`confirmed`。可重复传 `status` 和 `kind`，并按 `task_ref`、`target_label_ref`、
`proposed_label_name`、`include_all`、`limit` 过滤。

`GET /api/v1/boards/{board}/label-ontology/review` 返回只读聚合 review queue。
`group_by` 支持 `label`、`candidate_atom`、`proposed_label`、以及 opt-in `cluster`，
默认 `label`；`include_all=false` 默认只聚合 `open` 和
`confirmed` signals，`true` 时包含完整历史；`limit` 限制 group 数量。响应
`meta` 回显 `group_by`、`include_all` 和 `limit`。每个 group 包含：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "group_by": "label",
  "key": "lab_...",
  "label_id": "lab_...",
  "label_name": "cli",
  "candidate_atom_polarity": "positive",
  "candidate_atom_kind": "applies_when",
  "candidate_text": "extends CLI subcommands",
  "candidate_content_hash": "14ada47e4b0566c5",
  "proposed_label_name": null,
  "proposed_label_name_normalized": null,
  "cluster_key": null,
  "cluster_reason": null,
  "task_count": 2,
  "signal_count": 3,
  "open_count": 2,
  "confirmed_count": 1,
  "resolved_count": 0,
  "rejected_count": 0,
  "superseded_count": 0,
  "degraded_count": 1,
  "average_score": 0.31,
  "median_score": 0.28,
  "oldest_signal_at": 1781780000000,
  "latest_signal_at": 1781780100000,
  "sample_task_refs": ["default#12"],
  "signal_ids": ["los_..."],
  "action_count": 1,
  "action_ids": ["loa_..."],
  "proposal_ids": [],
  "labels": [{"id": "lab_...", "name": "cli"}],
  "candidate_atom_variants": [
    {
      "content_hash": "14ada47e4b0566c5",
      "polarity": "positive",
      "kind": "applies_when",
      "text": "extends CLI subcommands",
      "signal_count": 2
    }
  ]
}
```

Groups sort by distinct `task_count` desc, then `confirmed_count` desc,
`latest_signal_at` desc, and `key` asc。`group_by=cluster` 是可禁用的只读辅助视图：
默认不会启用，不写 canonical atoms，不确认、应用、validate 或关闭 signal，也不会创建
新的 SQLite truth 表。cluster key 每次请求时从已有 signal 文本重建，优先使用
lexical-normalized candidate text，其次 proposed label，再其次 rationale，最后退回到
kind/action/target/proposed-label scope 组合；所有 cluster key 都带有 signal kind、
proposed action、target label 和 proposed-label scope，避免跨 label/action/boundary 误合并；
`cluster_reason` 说明 key 来源。`GET /api/v1/label-ontology/signals/{signal_id}`
返回：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "signal": {},
    "observation": {},
    "actions": []
  }
}
```

`POST /api/v1/boards/{board}/label-ontology/actions` 写 review/lifecycle action：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "alice", "type": "user", "agent_type": null},
  "action_type": "confirm",
  "signal_ids": ["los_..."],
  "reason": "Observed across independent CLI tasks",
  "superseded_by_signal_id": null,
  "parent_action_id": null,
  "target_label_ref": null,
  "result_label_ref": null,
  "result_atom_id": null,
  "result_atom_content_hash": null,
  "result_proposal_id": null,
  "canonical_before_hash": null,
  "canonical_after_hash": null,
  "validation_requirement": null,
  "validation_status": null,
  "validation_effective_outcome": null
}
```

该公共 action endpoint 只接受 lifecycle action types：`confirm`、`reject`、
`supersede` 和 `resolve_no_change`，并会同步更新 source signal status。请求中的
`parent_action_id`、`target_label_ref`、result 字段、canonical hash、`change`、
`validation_requirement`、`validation_status`、
`validation_effective_outcome` 和 `validation` 必须为
`null`/缺省；否则返回
`invalid_input`。`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
`update_semantics`、`create_label_proposal`、`bootstrap_label`、`revert_ontology_mutation`、`validate` 等 mutation/validation action
types 不允许通过该 generic endpoint 写入；canonical mutation provenance 必须由
semantics PUT、apply atom、proposal create/accept、task-label bootstrap 或 validate 等
专用 route 在同一 transaction 内写入。`supersede` 写入时会沿 replacement
`superseded_by_signal_id` 链检查，若链路回到任一 source signal 或 replacement chain
自身已有环，则返回 `invalid_input`，不会写入新的 supersede action。

`POST /api/v1/boards/{board}/label-ontology/apply/atom` 对已有 label 执行
read-modify-upsert semantics，并写入 atom provenance action：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "signal_ids": ["los_1", "los_2"],
  "label_ref": "cli",
  "kind": "applies_when",
  "text": "extends CLI subcommands, command arguments, help output, or machine-readable JSON behavior",
  "reason": "Repeated false-negative signal across CLI surface tasks",
  "allow_retarget": false,
  "retarget_reason": null
}
```

Source signals 必须属于同一 board 且已 `confirmed`。`kind` 只接受
`applies_when`、`positive_example`、`excludes_when`、`negative_example`。如果 canonical
内容实际新增 atom，成功后返回 `add_positive_atom` 或 `add_negative_atom` action，
记录 result atom soft reference、content hash、before/after canonical hash、单份
change snapshot 和一个 `added` atom effect，并把 `validation_requirement` 置为
`required`。如果同内容 atom 已经存在，成功后返回
`adopt_existing_atom` provenance-only action，记录 existing atom soft reference、相同的
before/after canonical hash 和 source signal links；该 action 不修改 semantics/atoms、
不标脏 atom index，`validation_requirement=none` 且 effective outcome 为
`not_required`。默认要求所有带 `target_label_id` 的 source signals 都指向 `label_ref`；
不匹配时返回 `400 invalid_input` 并列出 offending signal ids。Atom text 可由 reviewer
泛化，不要求等于 source signal 的 candidate text。确实需要 retarget confirmed
same-board signals 时，必须传 `allow_retarget=true` 和非空 `retarget_reason`；
action `change_json.retarget_override` 会记录 reason、source signal 原始 target/proposed
label 和最终 target label。Override 不放宽 board/status 要求。该 route 只有在
canonical atom 实际新增时才标脏 label atom index；vector rebuild 和 suggest validation
在 transaction 外执行。

`POST /api/v1/boards/{board}/label-ontology/revert` 追加可追溯 rollback action，并把
目标 label semantics 恢复为被撤销 mutation action 的 before snapshot：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "reviewer", "type": "user", "agent_type": null},
  "target_action_id": "loa_...",
  "expected_current_hash": "optional-current-semantics-hash",
  "reason": "Rollback test-only atom mutation"
}
```

当前只支持 `add_positive_atom`、`add_negative_atom` 和 `update_semantics`。Route 要求
当前 canonical semantics hash 仍等于 `target_action_id` 的 `canonical_after_hash`；
`expected_current_hash` 非空时还必须等于当前 hash。成功后返回
`revert_ontology_mutation` action：`parent_action_id` 指向被撤销 action，source signal
links 从目标 action 复制，`change` 记录被撤销 action、before/after revert snapshot 和
`index_dirty=true`，并为本次 revert 实际 added/removed atoms 写 atom effects，随后标脏
label atom index。该 action 的 `validation_requirement` 为 `unsupported`，可记录
external failed/partial 诊断，但不会被当作可 trusted-passed 的 pending validation。该 route
不删除或修改原 action，也不处理 bootstrap label identity / task binding rollback；CLI
staged bootstrap verify 的失败路径在提交前零写入，不再依赖提交后的恢复流程。

`POST /api/v1/boards/{board}/label-ontology/validate` 追加 external attestation
validation action。HTTP route 接收调用方提交的自然 JSON `validation`，
但当前不运行 vector rebuild、index query 或 `label suggest`，因此它不能产生
trusted automated `passed`。需要 trusted automated validation 时使用 CLI
`label ontology validate --trusted`，由工具采集 index/suggest evidence 后写入。

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "parent_action_id": "loa_...",
  "signal_ids": ["los_1", "los_2"],
  "reason": "Source task still does not select the target label after atom rebuild",
  "validation_status": "failed",
  "validation": {
    "evidence_type": "external_attestation",
    "reviewer": "codex",
    "cases": [
      {
        "signal_id": "los_1",
        "case_type": "positive_atom",
        "passed": false,
        "before": {
          "target": {"label_id": "l_cli", "selected": false, "score": 0.12},
          "coverage": 0.61
        },
        "after": {
          "degraded": false,
          "target": {"label_id": "l_cli", "selected": false, "score": 0.14},
          "coverage": 0.60,
          "notes": "Manual review of stored suggest output did not meet pass criteria"
        }
      }
    ]
  }
}
```

Service 会把 supplied `validation` 包进 validation envelope，附上
source signal cases、observation task snapshot / suggest input hash 与当前 task hash
对比、parent action result 引用和 summary。公共 supplied/collected payload 只保存在
top-level `manual`；generated `cases[]` 通过 `after.manual_case_ref` 指向
`manual.cases[]` 中对应 signal 的原始 evidence，避免多 signal validation 把同一 payload
重复存入每个 case。`parent_action_id` 必须指向同一 board 上
`validation_requirement=required` 的 canonical mutation action，且 parent action 必须带有
canonical result evidence（例如 atom/result label/proposal 引用、canonical hash 和
非空 change snapshot）。HTTP supplied JSON 是 external attestation；它可保存
`failed` / `partial` 诊断，但 `validation_status="passed"` 会返回 `invalid_input`，
因为 passed validation 需要工具采集的 `trusted_automated` evidence。`unsupported`
parent 可以记录 external failed/partial 诊断，但不能 passed。结构化字段或字符串
`"automated"` 本身不构成可信来源。

Trusted automated validation 的 persisted payload 由 CLI collector 生成，而不是由 HTTP
caller 手写：top-level `evidence_type="trusted_automated"`、`collector.source`、
非空 `embedding_model`、object `solver_options`、clean `index.status`、
`index.generation` 和覆盖每个 linked source signal 的 `cases[]`。CLI collector 在长
SQLite transaction 外 rebuild atom index 并运行 suggest；写 action 时 service 在短
transaction 中重新核验 parent action、source signals、canonical after hash、index
dirty/error 状态和 generation。Trusted 表示工具采集、current hash/index generation
一致，并在指定 cases/controls 上机械通过；它不是全局语义正确性证明。

Typed policy 按 parent action 检查：

- `add_positive_atom`：`case_type="positive_atom"`，`after.degraded=false`，
  `after.evidence_atoms[]` 必须包含 parent `result_atom_id` 或
  `result_atom_content_hash`；target label 必须 selected 或 score >= 0.50；
  score/coverage 不能比 before 恶化。
- `add_negative_atom`：`case_type="negative_atom"`，`after.evidence_atoms[]`
  不用于 result negative atom 校验；parent result atom 必须出现在
  `after.negative_evidence_atoms[]`。false-positive task 上必须证明
  `after.target.selected=false`，或 before/after score 都存在且 after score 低于
  before score。必须提供至少一个 `after.positive_controls[]` 且每个 control
  passed 且未 regressed；若没有 positive control，必须提供带非空 reason 的
  `after.positive_control_waiver`。
- `bootstrap_label`：`case_type="bootstrap_label"`，所有 linked source signals
  都必须有 passed case；new/result label 必须 selected 或 score >= 0.50；
  evidence atoms 必须来自 result label。

Validation comparability 默认使用 observation 的 `suggest_input_hash`；status、
`updated_at`、`lock_version` 或 task label binding 只改变完整 snapshot 时写入
`task_metadata_drift` / `label_binding_drift` warning，不会让 passed validation stale。
title/description 变化会写入 `suggest_input_drift` 并使 case incomparable；旧
observation 缺少 `suggest_input_hash` 时写入 `legacy_suggest_input_hash_missing`，
不能静默 passed。`passed` 会把 linked source signals 转为 `resolved`；`failed` 与
`partial` 保留 signals 供后续修正或人工处理。

---

## 13. Search

### 13.1 Search tasks

```http
GET /api/v1/search/tasks?board=default&q=needle&status=ready&label=backend&assignee=worker-a&include_archived=false&limit=20&offset=0
```

默认 CLI/server build 启用 `tantivy-backend`。SQLite DB 旁存在 `index/v1/tasks/` 时，search 使用 Tantivy task index。Tantivy index 缺失、损坏、过期或二进制显式以 `--no-default-features` 构建时会回落到 SQLite，并带上 stale metadata。搜索匹配 task title、description、comments、run summary/error 和 event kind/payload。

`label` 按 label 名称或 id 过滤，可重复，并在评分和分页前使用 AND 语义。
带 label 过滤的 search 即使存在可用 Tantivy index，也会使用 SQLite fallback，
以确保结果反映当前 task-label 关联行。

Task ref 形状的 `q` 始终使用 SQLite 精确匹配语义，即使当前存在可用 Tantivy index：
纯数字 `12` 和 `#12` 匹配请求 `board` 内的 seq；`board#12` 和 `board/#12`
只在显式 board 等于请求 board 时匹配；`t_...` 只匹配请求 board 内的 task id。
Ref 形状 query 不会从 title、description、comments、runs 或 events 中返回模糊匹配。

Response:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "hits": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "score": 60.0,
        "snippet": "ready spec needle",
        "task": {
          "id": "t_01HX...",
          "seq": 12,
          "status": "ready",
          "title": "实现状态机"
        }
      }
    ],
    "meta": {
      "backend": "sqlite",
      "stale": false,
      "index_version": null,
      "last_event_id": 42,
      "index_lag_events": 0
    }
  },
  "meta": {
    "limit": 20,
    "offset": 0
  }
}
```

Task mutations do not write Tantivy inside their SQLite transactions. When served by `kanban serve` with `tantivy-backend`, a background loop makes one prompt startup `sync_search_index` attempt and then syncs every `--search-sync-interval-ms` milliseconds by default (`5000`; `0` disables). Manual `kanban index sync` remains available after normal task changes, and `kanban index rebuild` replaces the derived index. The Tantivy state is stored in board-scoped `app_settings` under `search.tasks.state.<board_id>` and round-trips through existing export/import.

### 13.2 Search task windows by status

```http
GET /api/v1/search/tasks/by-status?board=default&q=needle&status=ready&status=review&include_archived=false&limit=50&offset=0
```

This read-only endpoint batches board search columns into one request. It
accepts the same query, board, label, assignee, archive, and pagination params as
`GET /api/v1/search/tasks`, but returns one independent search window per
repeated `status`. `limit` and `offset` apply per status window. Status order in
the response follows query parameter order. Omitting `status` returns an empty
`statuses` array.

Response:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "statuses": [
      {
        "status": "ready",
        "tasks": [
          {
            "id": "t_01HX...",
            "ref": "default#12",
            "status": "ready",
            "title": "实现状态机"
          }
        ],
        "search_meta": {
          "backend": "sqlite",
          "stale": false,
          "index_version": null,
          "last_event_id": 42,
          "index_lag_events": 0
        },
        "page": {
          "limit": 50,
          "offset": 0,
          "total": null
        }
      }
    ]
  },
  "meta": {
    "limit": 50,
    "offset": 0
  }
}
```

### 13.3 Search status

```http
GET /api/v1/search/status?board=default
```

Response:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "backend": "sqlite",
    "derived_index": false,
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0,
    "message": "SQLite fallback search is active; no derived index exists yet"
  }
}
```

When the current `MAX(task_events.id)` is greater than the stored Tantivy `last_event_id`, `stale=true` and `index_lag_events` reports that high-watermark lag.
If background sync is disabled, delayed, or fails, search keeps returning current SQLite fallback results with stale metadata instead of trusting an out-of-date derived index.

---

## 14. Maintenance

### 14.1 Doctor

```http
POST /api/v1/maintenance/doctor
```

Response includes SQLite integrity, migration/user version, expired running tasks, orphan run checks, dependency cycle count, archived dependency edge count, missing and suspicious run log counts, executable status invariant counts for dependency/spec/schedule violations, foundation relationship consistency diagnostics, label ontology ledger diagnostics, and Knowledge Substrate diagnostics. Archived parent -> active child edges are allowed historical dependency edges; archived child edges from active parents are counted.

Foundation relationship diagnostics are read-only:

- `consistency_errors` / `consistency_warnings` summarize board consistency findings for base relationship rows.
- `consistency_issues[]` reports structured findings with `severity`, `code`, `message`, and `record_ids`.
- Covered tables: `task_labels`, `task_dependencies`, `task_steps`, `task_execution_plans`, `task_runs`, `task_comments`, `signal_observations`, `signals`, `task_events`, and `task_attachments`.
- v24+ databases require `signal_observations` and `signals` for the generic signal ledger.
- Hard errors mean a row's `board_id` differs from a referenced task / label / run / comment / observation board. The message includes `table`, `row`, `row_board`, `referenced`, and `referenced_board`.
- v25+ databases add board-scoped composite FKs for `signals.observation_id` and `signals.superseded_by_signal_id`.
- These checks complement service-layer board-scoped writes. `task_labels`, `task_dependencies`, `task_steps`, `task_execution_plans`, `task_runs`, `task_comments`, `signals`, and `task_attachments` are protected by board-scoped composite FKs in current schema. `signal_observations` and `task_events` retain nullable source references; doctor/import still check those board relationships as a hard-error diagnostic layer for corrupted JSONL/raw-SQL inputs.
- `PRAGMA foreign_key_check` results are surfaced as hard-error `consistency_issues[]` with table, rowid, parent table, and FK index. Import runs the same gate before commit and rolls back on violation.
- Nonzero `consistency_errors` make `ok=false`.

Ontology ledger diagnostics are read-only:

- `ontology_ledger_errors` / `ontology_ledger_warnings` summarize hard errors and warnings.
- `ontology_ledger_issues[]` reports structured findings with `severity`, `code`, `message`, and `record_ids`.
- v12+ databases require `label_ontology_observations`, `label_ontology_signals`, `label_ontology_actions`, `label_ontology_action_atom_effects`, and `label_ontology_action_signals`.
- Hard errors include cross-board ontology links, orphan action-signal/action-effect links, missing parent/supersede references, label/proposal/task board mismatches, signal supersede cycles, and action parent cycles. Nonzero errors make `ok=false`.
- Warnings are reserved for rebuildable or historically explainable soft references, such as an action `result_atom_id` whose current `label_atoms` row was rebuilt away.

Derived-layer diagnostics are read-only:

- `outbox_pending` / `outbox_running` / `outbox_failed` summarize `index_outbox`.
- `derived_dirty_stores` counts stores with `dirty=true`.
- `derived_error_stores` counts stores with `last_error` or failed outbox.
- `derived_stores[]` reports each store's `store_name`, `schema_version`, `last_event_id`, `dirty`, `last_error`, and pending/running/failed outbox counts for that store target.

`derived_stores[].last_event_id` is the store-level successful event watermark, not a board-local watermark. `dirty=true` means the store still has unfinished outbox on any board or a recent update failure; a board-scoped sync/rebuild can advance the watermark while leaving the store dirty if another board still has pending or failed work.

These fields do not make Tantivy/Oxigraph/LanceDB authoritative. SQLite remains the source of truth, and dirty derived stores remain rebuildable caches.

### 14.2 Checkpoint

```http
POST /api/v1/maintenance/checkpoint
```

Runs `PRAGMA wal_checkpoint(TRUNCATE)` and returns `busy`, `log_frames`, and `checkpointed_frames`.

### 14.3 Backup

MVP 建议只提供 CLI backup，不开放 HTTP backup。

---

## 15. Web UI Interaction Rules

1. 拖拽列时调用 transition endpoint。
2. 普通字段编辑调用 `PATCH /tasks/{id}`。
3. Web UI 不显示 claim_token，除非 debug 模式。
4. running task 的 complete/block 操作，若无 token，则 UI 走 `force=true` 并要求确认。
5. blocked task unblock 后目标列由服务端返回，前端不要预设。
6. SSE 收到 event 后，优先 refetch affected task，避免客户端状态机漂移。

### Signal Comments

API comment DTOs use `kind: "signal"` for signal ledger backlink comments. The natural `metadata` object contains `type:"signal_link"`, `signal_id`, `observation_id`, `signal_kind`, and `signal_status` for service-generated backlinks. Generic signal comment metadata remains open and lossless; clients should render the body as a readable fallback and may link to a signal detail only when the complete backlink shape is present.


## Transport catalog implementation note

本规范所列的每个 API/SSE method/path 都由 `kanban-contract` 的 endpoint descriptor catalog 作为唯一实现 source。注册 handler 时使用稳定 `operation_id` 与 `adapter_id`；这两个 identity 分别表示公开 endpoint 和 server runtime binding，不是 Rust type name、函数地址或 `stringify!` 推导值。


## B1-C2b task-read 成功响应契约

`GET /api/v1/boards/:board/tasks` 与 `GET /api/v1/boards/:board/tasks/by-status` 各自拥有独立、精确且闭合的成功响应契约，仅共享 `ApiTask`、`ApiLabel` 与既有 `OffsetPaginationMeta`/`TotalPaginationMeta` primitives。列表响应为 `data[]` 与既有 `TotalPaginationMeta { limit, offset, total }`；按状态响应包含有序窗口，每个窗口使用同一 `TotalPaginationMeta`，外层使用既有 `OffsetPaginationMeta { limit, offset }`。这只是 Rust 类型复用，JSON wire 形状不变。

Desktop 仅对这两个读取端点使用 endpoint-specific recursive exact parser：成功响应的 envelope、`meta`、窗口、共享 `ApiTask`、`ApiLabel` 与既有 `OffsetPaginationMeta`/`TotalPaginationMeta` primitives 都必须闭合且完整，pagination 数值必须是非负 safe integer；错误响应也必须是闭合的 `error { code, message, details? }` envelope。任何 malformed、mixed、missing 或 extra shape 统一返回 `invalid_response`，合法错误继续保留 `code`、`message` 与可选 `details`。其它 generic optional envelope 不受影响。两个 endpoint 的 headers 仍为 `Todo`，本次不采纳任何其它 endpoint。


## B2-C3 comments exact contract

List/create comments 使用各自的 exact path/response root，create 另有 exact request root；五个 roots 均有真实 producer/consumer witness。`author_type` 仅为 `user|agent`，`kind` 仅为 `note|decision|signal`；agent 的 `agent_type` 可缺失、null 或空白（空白归一化为 null），user 携带非空 `agent_type` 仍由 service 拒绝。unknown enum 在 typed API 边界返回 400。写入 comment 与 `task.comment.created` event 保持同一 SQLite transaction。B7 已采用 endpoint-specific header contracts；surface migration 仍为 Generated，等待后续 API adoption cohort 的 exact surface evidence。

### 8.4 B2-C4 exact run-read contracts

List/get 分别拥有闭合 path 与 success root，只共享 contract-owned `ApiRun`。Run status 是闭合
enum：`running|succeeded|failed|canceled|expired`。`worker_profile`、`worker_pid`、
`finished_at`、`exit_code`、`summary` 与 `error` 都是必须出现但可为 `null` 的字段。
`claim_token` 只存在于显式 claim transition response，不进入 list/get run；SQLite `log_path`
只供独立 get-run-log handler 解析受信任文件，不进入 list/get run。读取 archived task 的历史仍被
允许；list 先由全局 task id 解析真实 board，再通过同一 SQLite service path board-scope 查询。
Headers 保持 `Todo`，两个 endpoint 因而保持 `Generated`。schema 只约束 JSON shape，不替代
claim、complete、reopen、archive 或 board isolation service guard。
### 12.6 B4-C1 exact task-label association contracts

`GET /api/v1/tasks/{task_id}/labels`、`POST /api/v1/tasks/{task_id}/labels` 与 `DELETE /api/v1/tasks/{task_id}/labels/{label_id}` 分别拥有闭合、endpoint-specific path/success root；POST 另有闭合 request root。成功响应递归复用同一个 contract-owned `ApiLabel`/`ApiTask` wire shape；POST 的可选 `meta.created_labels` 仅在实际创建 label identity 时出现。Desktop add/remove caller 对 envelope、task、nested labels 与 created-label metadata 做递归 exact 校验，malformed/extra/null meta 统一返回 `invalid_response`。

本批不收敛 board label identity、bootstrap、semantics、atoms/index、suggestion/proposal 或 ontology ledger endpoint。HTTP headers 保持 `Todo`，三个 endpoint 因而保持 `Generated`；name/names 互斥、batch transaction、board/archive scope、幂等与事件语义继续由既有 SQLite service guard 拥有。

## B5-C2 execution/dependency exact contracts

以下 endpoint 已采用各自闭合的 path 与 success response root；有 JSON request 的 endpoint
同时采用闭合 request root：

- `GET|POST /api/v1/tasks/{task_id}/dependencies`
- `DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}`
- `POST /api/v1/tasks/{task_id}/execution-plan/not-required`
- `GET /api/v1/runs/{run_id}/log`
- `GET /api/v1/boards/{board}/columns`

Dependency 三个 success root 只复用 contract-owned `ApiDependencies`、`ApiDependencyTask`、
`ApiDependencyEdge` 和既有 `ApiTask` components；它们仍是 endpoint-specific roots，不是一个
family shortcut。add 的 `201 Created` / idempotent `200 OK`、cycle rejection、edge/event/status
recompute transaction 与 board scope 继续由 SQLite service guard 决定。Mark-plan response 只返回
execution-plan record，不能绕过 required-step 或 task-status guard。

Get-run-log 的 runtime 本来就是 JSON envelope，因此采用
`{ data: { run_id, content, truncated } }` exact root；`log_path` 和 claim token 保持私有，256 KiB
tail window 与 lossy UTF-8 读取语义不变。List-columns 返回完整 column fields，包括 required-nullable
`wip_limit`，column status 仍是 canonical task status enum，不能改变 `tasks.status`。

这些 endpoint 的无 query/body 维度标记 `NotApplicable`。通用 actor、locale 与 content-type
headers 本批保持 `Todo`，由后续统一 transport-closure cohort 处理，因此 endpoint migration
保持 `Generated`。


---

# File: docs/DISPATCHER_SPEC.md

# Dispatcher SPEC

Dispatcher 是本地可选调度器。它只处理本机 SQLite DB，不处理远程 worker，不处理多用户协作。

---

## 1. 目标

Dispatcher 负责：

1. reclaim：回收超时、崩溃或失联的 `running` 任务。
2. claim：从 `ready` 队列选择任务并进入 `running`。
3. run：执行本地 worker profile。
4. heartbeat：维持 claim。
5. finish：根据 worker 结果写回 `done/review/blocked/ready`。

Dispatcher 不负责：

- 远程执行。
- 多机协调。
- 权限控制。
- 长期日志存储。

---

## 2. 运行方式

### 2.1 单次运行

```bash
kanban dispatch --once
```

执行一轮：

1. reclaim expired。
2. claim up to capacity。
3. 对已 claim task 启动 worker。

### 2.2 常驻运行

```bash
kanban dispatch
kanban dispatch --max-iterations 10
```

前台循环执行。`--max-iterations` 用于测试、脚本或受控 smoke；不传时持续运行直到进程收到外部停止信号。

### 2.3 与 server 同进程

后续扩展。当前实现先提供独立 `kanban dispatch` 前台 loop；`kanban serve` 不启动 dispatcher。

### 2.4 Worker profile config

```bash
kanban dispatch --worker-profile backend --profile-config ./workers.toml
```

最小配置格式：

```toml
[workers.backend]
command = "cargo nextest run -p kanban-sqlite --no-fail-fast"
claim_ttl_ms = 300000
heartbeat_interval_ms = 30000
on_success = "done"
on_failure = "blocked"
log_dir = ".kb/logs/runs"
```

当前 CLI 只读取被 `--worker-profile` 选中的 section。支持字段：

- `command`
- `claim_ttl_ms`
- `heartbeat_interval_ms`
- `on_success`: `done|review|blocked|ready`
- `on_failure`: `done|review|blocked|ready`
- `log_dir`

`log_dir` 必须位于受信任 run log 根目录内：平台默认 run log 目录、
`<db_dir>/logs`，或 `<db_dir>/.kb/logs`。Dispatcher 在 claim task 之前拒绝
其他路径，避免写出后续 `kanban run logs` 和 `doctor` 会判定为可疑的 run log。

---

## 3. Dispatcher Loop

伪代码：

```rust
loop {
    let now = clock.now_ms();

    reclaim_expired(now)?;
    while running_count() < max_concurrency {
        match claim_next_ready_task(now)? {
            Some(claimed) => spawn_worker(claimed)?,
            None => break,
        }
    }

    sleep(poll_interval);
}
```

---

## 4. Promotion

Dispatcher 不执行 `todo/scheduled -> ready` promotion。`ready` 表示显式人工 promote 意图；依赖完成或计划到期只会改变查询返回的 derived state，不会把 task 放入 ready 队列。

---

## 5. Ready Queue Selection

默认排序：

```sql
ORDER BY priority ASC, created_at ASC
```

`priority` is the implemented P0-P3 integer level where `0` (P0) is highest and
`3` (P3) is lowest/default, so dispatcher claim order selects P0 first among tasks that are already `ready`.

Priority does not place work into the ready queue. P0 means incident, current
blocker, or must-handle-immediately work; P1 is near-term focus; P2 is important
follow-up; P3 is ordinary backlog/low/default. Ordinary ready tasks should remain
P1/P2/P3 unless they are truly immediate blockers. A P0 task in `todo`,
`scheduled`, or `triage` is still not claimable until the normal state-machine
guards allow explicit promotion to `ready`. A task whose execution plan is still
`unplanned` is not claimable even if its status is `ready`; add steps or mark the plan `not_required` before dispatcher claim.

可选后续扩展：

- assignee/profile matching。
- due_at 优先。
- label filter。
- WIP limit。

MVP selection 输入：

```text
board_id
worker_profile optional
limit
```

如果 task.assignee 不为空：

- 当 worker profile 与 assignee 匹配时可 claim。
- 人工 CLI start 可忽略 worker profile，但 actor 写入 claim_owner。

---

## 6. Claim Algorithm

Claim 必须原子执行。

伪 SQL：

```sql
BEGIN IMMEDIATE;

SELECT id
FROM tasks
JOIN boards ON boards.id = tasks.board_id
WHERE tasks.board_id = ?
  AND boards.archived_at IS NULL
  AND status = 'ready'
  AND claim_token IS NULL
  AND (assignee IS NULL OR assignee = ?)
  AND (
    EXISTS (
      SELECT 1 FROM task_steps s
      WHERE s.board_id = tasks.board_id
        AND s.parent_task_id = tasks.id
        AND s.required = 1
    )
    OR EXISTS (
      SELECT 1 FROM task_execution_plans ep
      WHERE ep.board_id = tasks.board_id
        AND ep.task_id = tasks.id
        AND ep.state = 'not_required'
    )
  )
  AND NOT EXISTS (
    SELECT 1
    FROM task_dependencies d
    JOIN tasks p ON p.id = d.parent_task_id
    WHERE d.child_task_id = tasks.id
      AND p.status NOT IN ('done','archived')
  )
ORDER BY priority ASC, created_at ASC
LIMIT 1;

UPDATE tasks
SET status = 'running',
    claim_token = ?,
    claim_owner = ?,
    claim_expires_at = ?,
    last_heartbeat_at = ?,
    started_at = COALESCE(started_at, ?),
    updated_at = ?,
    lock_version = lock_version + 1
WHERE id = ?
  AND status = 'ready'
  AND claim_token IS NULL;

INSERT INTO task_runs (...);
UPDATE tasks SET current_run_id = ? WHERE id = ?;
INSERT INTO task_events (...);

COMMIT;
```

如果 update affected rows = 0，说明被其他进程抢先 claim，重新选择下一个。

---

## 7. Worker Profile

配置示例：

```toml
[workers.default]
command = "./scripts/run-task.sh"
concurrency = 1
claim_ttl_ms = 300000
heartbeat_interval_ms = 30000
max_runtime_ms = 3600000
on_success = "done"   # done | review
on_failure = "blocked" # blocked | ready

[workers.codegen]
command = "kb-agent --task $KB_TASK_ID"
concurrency = 2
on_success = "review"
on_failure = "blocked"
```

### 7.1 环境变量

Worker process 获得：

| Env | 说明 |
|---|---|
| `KB_DB_PATH` | SQLite DB path。 |
| `KB_BOARD_ID` | board id。 |
| `KB_BOARD_SLUG` | board slug。 |
| `KB_TASK_ID` | task id。 |
| `KB_TASK_SEQ` | task seq。 |
| `KB_TASK_TITLE` | title。 |
| `KB_CLAIM_TOKEN` | claim token。 |
| `KB_RUN_ID` | run id。 |
| `KB_ACTOR` | dispatcher/worker actor。 |

Worker 可通过 CLI 回写：

```bash
kanban --db "$KB_DB_PATH" task heartbeat "$KB_TASK_ID" --claim-token "$KB_CLAIM_TOKEN"
kanban --db "$KB_DB_PATH" task done "$KB_TASK_ID" --claim-token "$KB_CLAIM_TOKEN" --summary "..."
```

也可以让 dispatcher wrapper 根据进程退出码自动 complete/block。

---

## 8. Heartbeat

默认：dispatcher wrapper 负责 heartbeat，不要求 worker 自己做。

规则：

- 每 `heartbeat_interval_ms` 更新一次。
- heartbeat TTL 延长至 `now + claim_ttl_ms`。
- 若 heartbeat 失败，dispatcher 应终止 worker 或等待 reclaim。

---

## 9. Finish Policy

### 9.1 Success

Worker exit code = 0。

根据 profile：

| `on_success` | Transition |
|---|---|
| `done` | `running -> done` |
| `review` | `running -> review` |

### 9.2 Failure

Worker exit code != 0。

根据 profile：

| `on_failure` | Transition |
|---|---|
| `blocked` | `running -> blocked` with reason。 |
| `ready` | reclaim to ready and increment retry。 |

如果 `retry_count >= max_retries`，强制进入 `blocked`。

### 9.3 Timeout

如果 run 超过 `max_runtime_ms`：

- 尝试 terminate worker。
- close run as `expired`。
- 根据 retry policy 进入 `ready` 或 `blocked`。

---

## 10. Reclaim

Reclaim 条件：

1. `claim_expires_at <= now`。
2. worker_pid 不存在。
3. run 超时。
4. manual force。

Reclaim side effects：

- task status: `ready` 或 `blocked`。
- clear claim fields。
- close active run as `expired/canceled`。
- insert `task_events(kind='task.reclaimed')`。

---

## 11. PID Checking

因为只支持单机，可以检查 PID。

限制：

- PID 可能复用。
- 只能作为辅助信号，claim TTL 仍是主机制。
- 跨平台实现需要抽象。

建议：

- Linux/macOS：检查 pid 是否存在。
- Windows：后续实现，MVP 可只依赖 TTL。

---

## 12. Logs

Worker stdout/stderr 不全量写 DB。

默认路径：

```text
~/.local/state/kb/logs/r_<run_id>.log
```

DB 记录：

- `task_runs.log_path`
- `task_runs.summary`
- `task_runs.error`

CLI：

```bash
kanban run logs r_01HX...
```

---

## 13. Failure Cases

| Case | 行为 |
|---|---|
| Dispatcher 崩溃 | running task claim 过期后被下次 dispatcher reclaim。 |
| Worker 崩溃 | heartbeat 停止，claim 过期，reclaim。 |
| SQLite busy | 等待 busy_timeout；仍失败则记录错误并下轮重试。 |
| Task 被人工 block | Dispatcher 不再处理。 |
| Board 被归档 | Dispatcher 不再 claim/reclaim 该 board；若仍有 running task/run，board archive 本身会被拒绝。 |
| Task 被人工 force complete | Worker 后续 complete 失败，因 token/run 已关闭。 |
| DB integrity failed | Dispatcher 停止，提示运行 `kanban doctor`。 |

---

## 14. MVP Scope

MVP dispatcher 必须实现：

- claim one ready task。
- spawn command。
- heartbeat。
- complete/block based on exit code。
- reclaim expired claims。

MVP 可暂不实现：

- profile concurrency > 1。
- complex worker matching。
- Windows PID checking。
- per-label routing。
- cron-like recurring tasks。


---

# File: docs/IMPLEMENTATION_PLAN.md

# Implementation Plan

本文档给出分阶段实现计划、验收标准和测试策略。

---

## Phase 0：Repository Skeleton

目标：建立 Rust workspace 和基本工程纪律。

交付：

- `Cargo.toml` workspace。
- crates：`kanban-core`、`kanban-sqlite`、`kanban-cli`。
- migrations 目录。
- lint/format/test workflow。
- error type。
- ID/time utility。

验收：

- `cargo test` 通过。
- `cargo fmt --check` 通过。
- `cargo clippy` 无关键 warning。

---

## Phase 1：SQLite Schema + Core Domain

目标：数据结构和 migration 可用。

交付：

- 执行 `001_initial.sql`。
- `kanban init`。
- 默认 board。
- 默认 columns。
- 领域类型：Board、Task、Status、Run、Event。
- status enum 与 parse/serialize。

验收：

- 新建 DB 后 `PRAGMA integrity_check` 返回 ok。
- 重复运行 `kanban init` 不破坏已有 DB。
- schema version 可查询。

测试：

- migration test。
- schema smoke test。
- enum roundtrip test。

---

## Phase 2：Task CRUD + Events

目标：任务可创建、查询、更新，事件可记录。

交付：

- `kanban task create/list/show/update`。
- `task_events` 写入。
- `--json` 输出。
- `expected_lock_version` 乐观锁。

验收：

- 创建 task 后有 `task.created` event。
- update 不允许修改 status。
- board task list 默认隐藏 archived。

测试：

- create/list/show integration test。
- event transaction test。
- invalid input test。

---

## Phase 3：State Machine Transitions

目标：核心状态机可用。

交付：

- specify。
- promote。
- claim/start。
- heartbeat。
- complete/done。
- block。
- unblock。
- reclaim。
- archive。

验收：

- 非法 transition 被拒绝。
- 每个 transition 写 event。
- running task 必须有 claim token。
- complete 后清理 claim fields。

测试：

- transition matrix unit tests。
- block/unblock target recomputation tests。
- token mismatch tests。

---

## Phase 4：Dependencies

目标：支持 parent/child 依赖和显式 manual promotion。

交付：

- `kanban dep add/remove/list`。
- cycle detection。
- dependency-aware create/promote/claim。
- parent complete 后不自动 promote children；child 保持 `todo`，由 derived dependency fields 表达是否仍被阻塞。

验收：

- child 依赖未完成 parent 时不能 ready/running。
- parent 完成后 child 可被手动 promotion。
- cycle 添加失败。

测试：

- direct cycle。
- indirect cycle。
- child demotion when dependency added。
- manual promotion after completion。

---

## Phase 5：Runs + Dispatcher MVP

目标：可执行任务并恢复崩溃任务。

交付：

- `task_runs` 写入。
- `kanban dispatch --once`。
- `kanban dispatch` loop。
- worker profile command。
- heartbeat wrapper。
- expired reclaim。
- run logs。

验收：

- ready task 被 dispatcher claim 并执行。
- command exit 0 后 task done/review。
- command exit non-zero 后 task blocked/ready。
- worker timeout 后 reclaim/block。
- dispatcher crash 后 task 可被 reclaim。

测试：

- claim race test，多线程同时 claim 同 task 只有一个成功。
- worker success integration。
- worker failure integration。
- expired claim reclaim test。

---

## Phase 6：Local Web API

目标：Web 端可通过 HTTP 操作 board/task。

交付：

- `kanban serve`。
- REST endpoints。
- unified error response。
- SSE event stream。
- health endpoint。

验收：

- Web API 能完成 CLI 同等生命周期。
- SSE 收到 task events。
- API 不能 PATCH status。
- 默认只监听 127.0.0.1。

测试：

- route integration tests。
- API transition tests。
- SSE reconnect test。

---

## Phase 6.5：Board Lifecycle MVP

目标：让单 SQLite DB 内的多个 board 可通过 CLI/API 创建、选择、归档，并让 task ref 兼容未来聚合视图。

交付：

- `kanban board list/create/show/use/current/archive`。
- `POST /api/v1/boards` 与 `POST /api/v1/boards/{board}/archive`。
- 项目级 `.kb/config.toml` active board，解析顺序为 `--board`、`KB_BOARD`、最近项目 config、`default`。
- task ref 支持全局 `t_...`、当前 board 的裸 seq / `#seq`、显式 `board#seq` / `board/#seq`。
- CLI/API task 输出包含 `board_slug` 和可复制 `ref`。

验收：

- `board create` 创建默认 columns 并写 `board.created` event。
- `board use` 写入项目级 `.kb/config.toml`。
- `t_...` 可跨 active board resolve，`board#seq` 可显式跨 board resolve。
- Archived board 默认隐藏且拒绝普通写入；events/runs/comments 历史仍可读。
- Board archive 在存在 `running` task/run 时被拒绝。
- Duplicate board slug 返回 user-facing invalid input / HTTP 400。

---

## Phase 7：Web UI MVP

目标：基本看板 UI 可用。

交付：

- Board columns。
- Task cards。
- Task detail drawer。
- Create/update task。
- Drag/drop 调用 transition。
- Comments。
- Event timeline。
- Run history。
- SSE live refresh。

验收：

- UI 不直接修改 status。
- blocked unblock 后根据服务端返回移动列。
- running complete force 时有确认。

---

## Phase 8：Maintenance & Hardening

目标：本地数据可靠性。

交付：

- `kanban doctor`。
- `kanban backup`。
- `kanban checkpoint`。
- `kanban vacuum`。
- JSONL export。
- orphan run 检查。

验收：

- backup 可恢复。
- integrity check 可报告问题。
- expired running task 可列出并 reclaim。

---

## Testing Strategy

### Unit Tests

- status parse/format。
- transition guard。
- initial status computation。
- unblock target computation。
- dependency cycle detection。

### SQLite Integration Tests

- migration fresh DB。
- migration idempotency。
- FK constraints。
- JSON validation constraints。
- CAS claim affected rows。

### Concurrency Tests

- 10/50/100 concurrent claim attempts on one task。
- CLI + server simultaneous writes。
- busy timeout behavior。

### Dispatcher Tests

- successful worker。
- failed worker。
- timeout worker。
- heartbeat extension。
- reclaim expired。

### CLI Golden Tests

- human output stable enough。
- JSON output exact contract。
- exit code mapping。

### API Tests

- task lifecycle。
- invalid transition HTTP 409。
- error body format。
- SSE stream ordering。

---

## Definition of Done for MVP

MVP 完成定义：

1. `kanban init` 创建可用 DB。
2. `kanban board list/create/show/use/current/archive` 可用。
3. `kanban task create/list/show/update` 可用，输出包含 `board#seq` ref。
4. `kanban task start/heartbeat/done/block/unblock/archive` 可用。
5. dependencies 可用。
6. events 可用。
7. runs 可用。
8. dispatcher 可执行本地命令。
9. web API 覆盖核心 lifecycle。
10. web UI 可视化 board。
11. 并发 claim 测试稳定通过。
12. `kanban doctor` 能发现基本数据异常。

---

## Recommended First Milestone

最小可用 milestone：

```text
Phase 0 + Phase 1 + Phase 2 + Phase 3 + 部分 Phase 4
```

即：

- SQLite schema。
- CLI task lifecycle。
- 状态机。
- events。
- dependencies 基础。

先不要做 Web UI 和 dispatcher，直到状态机与 schema 稳定。


### Phase 2 transport identity 收敛

已完成 API/SSE method/path authority：84 个 descriptor（83 API + 1 SSE）和真实 router binding 的双向 parity 已锁定。后续 DTO adoption 必须复用 descriptor 的 `operation_id` 与 obligation，逐项把 `Todo` 收敛为 `Contract`、`NotApplicable` 或带理由 `Excluded`；不得重新引入 server 或 surface catalog 的手写 API/SSE path 表。

B1-A 已完成 wire 收口：API error response 已使用闭合 `ApiErrorCode`，label semantics delete handler 已使用 `DeleteResponse`/`DeleteResult` 并有真实 producer、typed consumer、schema 与 AST ownership evidence。delete endpoint 的 path/query/header/body obligation 尚未逐项建模，故 endpoint 和 response migration 继续标为 `generated`；下一步应先建模并验证这些请求义务，再考虑 adoption。

B1-C0 已完成 transport proof foundation：全部现有 contract 都显式区分 HTTP transport 与 `NoTransport`；HTTP contract 记录 location 与参数 cardinality，`Success`/`Error` 分别表示 2xx exact success 与 shared non-2xx response。任意 `Adopted` contract 及 endpoint exact reference 都要求 `granularity=Exact`；endpoint exact 唯一性由唯一 method/path、精确 `operation_key` 和 location 结构性保证。shared component 的多 endpoint linkage、linkage OR witness orphan policy 和 exact-miscount 均由 mutation tests fail closed。该阶段没有迁移 handler DTO，也没有关闭任何 endpoint `Todo`；冻结值保持 SSE `Todo`、endpoint `Todo=389`、总未闭合 `636`。

B1-C1 已完成两个 board task-read endpoint 的 path/query transport adoption：4 个 endpoint-specific
exact contract 拥有独立 DTO、schema、正负 fixture、DTO producer、非默认 board sentinel 的真实
router consumer 与 AST ownership/mutation evidence。两个 server-local typed extractor 各自绑定
path，并各自只调用一次共享 ordered raw-query parser；handler 不持有 raw URI。真实 URI matrix
锁定 8192 bytes raw 总预算、由 9/4/3/32 repeated 与 6 scalar 推导出的 54-pair cap、
RepeatedOrdered distinct 语义、1024/128/128 Unicode 字符预算、label 的 raw 128 字符预算
包含随后会被 trim 的 Unicode 边缘空白、纯 Unicode 空白 label 拒绝、
唯一 application/service/contract limit authority 链、全部 filter 转发、标准 form encoding 与
max/max+1。Desktop caller 继续使用 `URLSearchParams` 标准 form encoder，并以保留字符/UTF-8
断言锁定只发送 `q` 而不恢复 `search`。本阶段仅将 SQLite service 的 `MAX_TASK_LIST_LIMIT`
改为直接复用 application authority；service 查询行为与 `kanban-core` 状态机未改变。GET body
是 `NotApplicable`，headers/success 保持 `Todo`，因此两个 endpoint
仍为 `Generated`。冻结值保持 endpoint `Contract=19`、`Todo=383`、`NotApplicable=102`，
总未闭合 `630`。


## B1-C2b task-read 成功响应验收

验收要求：两个 endpoint 各有 response root/schema/正负 fixture/producer-consumer evidence；共享 `ApiTask`/`ApiLabel` 与既有 pagination primitives；Desktop 对这两个 endpoint exact recursive fail-closed，hostile payload 返回 `invalid_response`；`just schema-contract`、`just desktop-check` 与 affected gate 通过。权威行为见 [API_SPEC](API_SPEC.md#b1-c2b-task-read-成功响应契约) 与 [SCHEMA_CONTRACTS](SCHEMA_CONTRACTS.md#b1-c2b-task-read-成功响应契约)。


### B2-C3 comments pair acceptance

- list/create comment 各自拥有 exact path 与 success root，create 拥有 exact request root。
- 真实 non-default-board route 生成 committed fixtures；contract 与 Desktop 分别独立消费。
- private `CommentBody`/`CommentDto` 归零，handler 经共享 SQLite service path 显式适配 `ApiComment`。
- board isolation、archived-board write guard、decision metadata、event transaction 与 locale/status
  继续由既有 service/API tests 锁定。

B2-C3 review closure：五个 comments roots 均提升为有 structured witnesses 的 Adopted；endpoint 仍 Generated、headers Todo。验收额外锁定 AST canonical tail/adapter、防 bypass mutations、Desktop create hostile/error transport、comment/event rollback 与完整 identity/kind matrix；权威统计见 SCHEMA_CONTRACTS。

### B2-C4 run reads acceptance

- 仅迁移 list-runs/get-run；get-run-log、transition、create-task 与 steps 不进入本批。
- contract-owned `ApiRun`/`ApiClaim` 取代 private `RunDto`/`ClaimDto`；共享 adapter 隐去
  `RunRecord.claim_token` 与 `log_path`。
- 四个 roots 以 non-default-board 真实 claim/complete/reopen/claim lifecycle 产生 active 与
  finished run fixtures，并分别由 contract root 独立消费。
- Server AST mutation、board isolation、archived history、not-found/privacy，以及 Desktop exact
  parser/transport/hostile/error tests 构成运行时 adoption 证据。
## B2-C5 create-task contract checkpoint

- `POST /api/v1/boards/:board/tasks` 的 path/request/success 三项 contract 已采用，headers 保持
  `Todo`，endpoint 诚实保持 `Generated`。
- contract owner 提供 create-only status、opaque object metadata 与闭合 `ApiTask` response；
  server 删除 private `CreateTaskBody`，Desktop 使用 exact response parser。
- 真实 router、transaction rollback/retry/readiness guard、schema witness 与 Desktop consumer 是
  本切片验收面；不改变 SQLite/core authority，也不提前关闭其它 CRUD endpoint。
### B2-C6 boards endpoints acceptance

- list query、create request、get/archive path 与四个 success response 各有 exact root、schema、
  正负 fixture 和独立 producer/consumer witness；archive body 继续复用既有
  `ArchiveBoardRequest` owner。
- handler 删除 private `CreateBoardBody` 和 `BoardRecord` wire 泄漏，经
  `kanban_sqlite::api` 显式映射到 contract-owned `ApiBoard`；真实 fixtures 使用 non-default board。
- list 的 `include_archived` 必须真实转发到 service options；archive running guard、archived
  history/read、404/status/i18n 不得改变。
- Desktop `listBoards` production caller 使用 endpoint exact parser；AST ownership 与 mutation
  gate 阻止 private DTO、错误 response root、默认 board/path 绕过或 private service/adapter。

B2-C6 完成时 8 个新 roots 为 Adopted；四个 endpoint 因 headers 仍为 Todo 而保持 Generated。
权威统计见 SCHEMA_CONTRACTS。

### B7 API transport header closure

- 83 个 non-SSE endpoint 各自拥有 exact header contract，统一复用五种 locale/actor/body
  cardinality profile；endpoint obligation `Todo` 已归零。
- 真实 router gate 锁定 locale、body actor 优先级、header actor fallback、required/optional/no-body
  `Content-Type` 行为；SSE `Last-Event-ID` 继续保持明确 exclusion。
- schema roots=301，semantic adopted/generated/planned=222/79/26，surface
  adopted/generated/planned/excluded=2/82/135/5，unfinished=322。
- 该阶段只关闭 API endpoint catalog，不等于全局 `schema-audit-closed`；后续继续处理 semantic
  generated/planned 与 CLI/JSONL/helper surface obligations。
### B2-C7 steps family acceptance

- list/create/update/remove/done/skip/reopen 共 19 个 endpoint-specific path/request/success roots 全部 `Adopted`，headers 保持 `Todo`，endpoint 保持 `Generated`。
- server 删除 private step DTO/request owners，以 contract DTO 作为唯一 wire owner，同时保留原 SQLite service path、required/optional/resolution/status/plan 与 transition guards。
- committed fixtures 由非默认 board 的真实 router producer 证明；request/path 使用程序化 DTO producer 与独立真实 router consumer，Desktop 七个 production callers 使用递归 exact consumer。
- syn AST ownership mutation、schema shape、既有 required-step/plan transition service tests、`schema-contract`、server、web 与 affected gates 共同构成验收证据。

### B2 integrated train acceptance

Create-task、boards 与 steps 的全部 exact roots、真实 router witnesses、Desktop consumers 和 service guards 已按合集重新生成并验证；集成冻结为 62 roots / 60 adopted contracts / 120 witnesses / 567 unfinished。后续 B2 切片必须从该 train 快照继续，不得回退到任一独立 lane 的局部统计。


---

# File: docs/ADR.md

# Architecture Decision Records

本文件记录当前 SPEC 的关键架构决策。

---

## ADR-0001：SQLite-only

### Status

Accepted

### Context

项目明确不考虑多用户、多租户、团队协作和远程 worker。核心运行环境是本地单机，同时需要 CLI 和 Web。

### Decision

只支持 SQLite。

默认 DB：

```text
~/.local/share/kb/kb.db
```

可通过 `--db <path>` 指定项目本地 DB。

### Consequences

优点：

- 单 binary 易分发。
- CLI 使用成本低。
- 备份简单。
- 本地事务足够强。
- WAL 支持 reader/writer 并发。

代价：

- 不支持跨机器共享写入。
- 不做 server cluster。
- 一次只有一个 writer。
- 需要控制 transaction 长度。

---

## ADR-0002：Status Enum 是真相，Column 是视图

### Status

Accepted

### Context

传统 Trello-like 工具常把 list/column 视为状态。但本项目需要 dispatcher、claim、heartbeat、reclaim、run history。`running` 不是普通视觉列，而是 claim 成功后的执行状态。

### Decision

`tasks.status` 是 canonical truth。`board_columns` 只是 UI 展示映射。

### Consequences

优点：

- Web、CLI、dispatcher 遵循同一状态机。
- 可保护 `ready -> running`。
- 能支持 review/scheduled/blocked 等非纯视觉状态。

代价：

- 拖拽列不能简单 PATCH status。
- Web UI 需要根据目标列调用 transition endpoint。

---

## ADR-0003：Snapshot + Append-only Events，不做纯 Event Sourcing

### Status

Accepted

### Context

看板 UI 高频查询当前任务列表。纯 event sourcing 会让当前状态查询复杂化，需要重放事件或额外投影。

### Decision

采用：

```text
tasks snapshot + task_events append-only
```

状态变化时，snapshot update 与 event insert 必须在同一 transaction 内完成。

### Consequences

优点：

- 当前 board 查询简单。
- 事件仍可用于审计、SSE、debug。
- 实现复杂度可控。

代价：

- 需要保证 snapshot/event 一致。
- 事件不是唯一事实源。

---

## ADR-0004：CLI 可以直接访问 SQLite，但必须走统一 service path

### Status

Accepted

### Context

如果 CLI 必须依赖常驻 server，会降低本地工具可用性。直接访问 SQLite 更适合脚本和开发流。

### Decision

CLI 可以直接打开 SQLite DB，但只能调用统一 Rust service path；当前实现主要是
`kanban-sqlite::service` use-case 函数，并复用 `kanban-core` 的纯状态机 helper。
CLI 不允许绕过状态机执行裸 SQL 修改状态。

### Consequences

优点：

- 不需要 server 即可使用。
- 脚本友好。
- 和 Web 行为一致。

代价：

- 需要处理 CLI/server/dispatcher 同机并发。
- 所有状态逻辑必须集中在共享 service/state-machine path，避免 CLI、server 或
  dispatcher 各自实现一套状态转换。

---

## ADR-0005：Actor 是审计字符串，不是用户模型

### Status

Accepted

### Context

项目不做多用户和权限，但仍需要知道某个操作来自谁或哪个 worker。

### Decision

保留 `actor`、`created_by`、`claim_owner` 字段。它们是字符串，不关联 users 表。

### Consequences

优点：

- 保留审计能力。
- 支持 CLI、Web、dispatcher、worker profile 区分来源。
- 不引入 RBAC 复杂度。

代价：

- 不提供权限隔离。
- actor 可被本地调用者伪造，这是预期边界。

---

## ADR-0006：Worker stdout/stderr 存文件，DB 只存摘要与路径

### Status

Accepted

### Context

运行日志可能很大。把日志 blob 放进 SQLite 会影响性能和备份体积。

### Decision

日志写入：

```text
~/.local/state/kb/logs/r_<run_id>.log
```

DB 只存：

- `log_path`
- `summary`
- `error`
- `exit_code`

### Consequences

优点：

- SQLite 保持轻量。
- 日志可直接 tail。
- 备份策略可分开处理 DB 和 logs。

代价：

- 移动 DB 时需要同时移动 logs/attachments。
- log path 需要 doctor 检查。

---

## ADR-0007：默认只监听 localhost

### Status

Accepted

### Context

不做远程服务和多用户登录。暴露到局域网会制造安全边界问题。

### Decision

`kanban serve` 默认并且建议只监听：

```text
127.0.0.1:8721
```

MVP 不提供 `0.0.0.0` 远程模式。

### Consequences

优点：

- 无需登录系统。
- 降低误暴露风险。

代价：

- 不能多人访问。
- 不能远程手机/浏览器访问。

---

## ADR-0008：状态变化必须有专用 Transition Command

### Status

Accepted

### Context

直接 PATCH `status` 容易绕过 claim/run/event/dependency guard。

### Decision

禁止普通 update 修改 status。所有状态变化都使用 command：

- specify
- promote
- claim
- heartbeat
- complete
- submit_review
- block
- unblock
- reclaim
- archive

### Consequences

优点：

- 状态机可验证。
- run/claim/event 一致。
- Web/CLI/dispatcher 行为一致。

代价：

- API 数量更多。
- UI 拖拽逻辑更复杂。

---

## ADR-0009：Knowledge Substrate 派生层

### Status

Accepted

### Context

后续搜索、关系扩展、agent context、artifact provenance 和向量召回需要跨 task/run/comment/artifact/skill 的统一身份与派生索引，但不能削弱 SQLite 状态机、claim 和 dependency guard。

### Decision

SQLite 继续作为 operational source of truth。新增：

- `entities`：跨库统一 `kb://...` identity registry。
- `relation_predicates` / `entity_relations`：受控 predicate 与可重建关系镜像。
- `index_outbox`：派生 store 的 at-least-once job surface。
- `derived_store_state`：Tantivy/Oxigraph/LanceDB 等派生层健康和水位。

Tantivy、Oxigraph、LanceDB 都是可重建 derived stores，不参与状态机事务。

`derived_store_state` 的语义是 store 全局状态，不是 board 局部状态：

- `last_event_id` 表示该 store 已成功处理并提交的全局 task event 高水位。成功 sync/rebuild 只能把它单调推进，不能倒退。
- `dirty=true` 表示该 store 仍有未完成 outbox、失败 outbox 或最近一次派生更新失败；即使某个 board 已 sync/rebuild 完成，其他 board 仍有 pending/failed job 时也必须保持 dirty。
- board-scoped sync/rebuild 只清理当前 board 的 outbox job；是否把 `dirty` 置回 false 取决于同一 store target 是否还存在任何 board 的 unfinished outbox。
- `last_error` 记录最近一次 store 级失败证据。成功处理会清除 `last_error`，失败会保持 `dirty=true` 并保留/标记相关 outbox 失败状态。
- `index_outbox` 是恢复和重放入口；`derived_store_state` 是 operator health/watermark 摘要。两者都不能使派生层成为事实源。

### Consequences

优点：

- 后续 graph/vector/context broker 可以接同一 entity/relation contract。
- SQLite 状态机边界保持清楚。
- 派生 store 损坏时可 fallback/rebuild。
- `kanban doctor` / maintenance API 汇总 outbox backlog、dirty stores、last_error 和 failed outbox，用于本地 operator 判断 sync/rebuild，而不是让派生层参与 SQLite 事务。

代价：

- 需要维护 entity backfill/outbox/derived state。
- `derived_store_state` 是派生 store 的主健康/水位记录；Tantivy 的旧 `app_settings` search state 仅保留为兼容 metadata。

---

## ADR-0010：单 DB 多 board 与 CLI task ref

### Status

Accepted

### Context

本地项目需要不同 board/project，但未来也需要聚合视图和跨 board 审计。如果每个项目拆一个 SQLite DB，聚合、搜索、事件和 dispatcher 恢复都会变复杂。另一方面，裸 `#12` 在 shell 中容易被当作注释，且 board-local seq 不能跨 board 唯一。

### Decision

继续使用单 SQLite DB 内多个 board：

- `tasks.id` 是全局唯一 `t_...`。
- `tasks.seq` 只在 `board_id` 内唯一。
- CLI/API 展示 copyable task ref：`board_slug#seq`。
- CLI task ref 支持全局 `t_...`、当前 active board 的 `12` / `#12`、显式 `board#12` / `board/#12` / `b_...#12`。
- Active board 解析顺序是 `--board`、`KB_BOARD`、最近 `.kb/config.toml`、`default`。
- `.kb/config.toml` 只记录当前项目选择的 board，不表示项目拥有独立 DB。
- Board slug 禁用保留 ID 前缀和会破坏 ref 语法的字符。

Archived board 默认不可写；归档只标记 board，不改 task 状态，并拒绝仍有 `running` task/run 的 board。Read-only events/runs/comments 历史保留可查，作为审计入口。

### Consequences

优点：

- 保留未来聚合 board / dashboard 的数据基础。
- `t_...` 可作为脚本稳定全局引用。
- `board#seq` 对人和 shell 都更可复制。
- 项目级 active board 不破坏单 DB 备份、搜索和 dispatcher 语义。

代价：

- CLI 必须维护 task ref parser/resolver。
- Archived board 需要区分 read-only history 与 mutation guard。
- 裸 `#12` 只能作为兼容输入，文档和输出不能依赖它。

---

## ADR-0011：Schema Train 边界：status、type、labels、dependency type 与 decision comments

### Status

Proposed

### Context

`kanban-tool` 接下来会进入一组 schema/model 扩展：

- `task_type`：表达任务是什么类型。
- `dependency_type`：表达任务之间是什么关系。
- labels：表达可搜索、可筛选、可推荐的多维标签。
- comments：承载人和 agent 的协作记录。
- decision comments：记录人或 LLM/agent 在多个方案之间做出的选择。

当前 comment 模型里的 `kind` 混用了两类概念：

- 谁写的：system / worker / agent / user。
- 写的是什么：普通记录 / 决策记录。

这会让后续结构化 decision comment 变脏。需要先把模型边界切开：

- author/source 轴：谁留下了这条 comment。
- content kind 轴：这条 comment 表达什么语义。

本项目是 dogfood local tool，不需要为早期 comment schema 保留沉重兼容层。可以直接修改模型，只要迁移清晰，并让 CLI/API/Desktop 一次性跟上。

### Decision

保留现有核心原则：

- `tasks.status` 继续是唯一 canonical workflow state。
- hard dependency 继续是状态机和 dispatcher guard 的事实来源。
- `task_events` 继续是 append-only audit trail。
- comments 继续承载协作记录，但 comment schema 要拆清楚作者和内容语义。
- 新字段默认不改变状态机、dispatcher claim 或 ready eligibility，除非本 ADR 明确允许。

### Field Responsibilities

| Field / Model | 责任 | 是否影响状态机 | 是否影响 dispatcher | 是否影响 dependency/search/context 展示 | 是否用于 search/context/UI |
|---|---|---:|---:|---:|---:|
| `status` | canonical workflow state | 是 | 是 | 是 | 是 |
| `priority` | ready/dispatcher 的排序权重 | 否 | 是，排序 | 是，列表和推荐排序 | 是 |
| `scheduled_at` | 计划时间，参与 scheduled/ready guard | 是 | 是 | 是，列表和上下文排序 | 是 |
| `due_at` | 截止时间，只展示、筛选、排序 | 否 | 可排序 | 可排序 | 是 |
| `task_type` | 任务类别，例如 bug/feature/research/ops/follow_up | 否 | 否 | 可用于展示/排序，不改变 eligibility | 是 |
| labels | 多标签分类、搜索、推荐和 UI grouping | 否 | 否 | 否，除非未来显式配置排序策略 | 是 |
| `dependency_type` | 依赖边语义，区分 hard block 和 soft relation | 仅 hard block | 仅 hard block | 是，但必须区分 hard/soft | 是 |
| `comment.author_type` | 评论作者角色：`user` 或 `agent` | 否 | 否 | 否 | 是 |
| `comment.author` | 展示名，例如 `kanban-user`、`codex` | 否 | 否 | 否 | 是 |
| `comment.agent_type` | 可选 agent 细分，例如 `codex`、`executor`、`dispatcher` | 否 | 否 | 否 | 是 |
| `comment.kind` | 内容语义：`note` 或 `decision` | 否 | 否 | 否 | 是 |
| `comment.metadata_json` | `comment.kind` 对应的结构化 payload | 否 | 否 | 否 | 是 |
| `event.kind` | append-only audit event 类型 | 否，event 是结果不是输入 | 否 | 否 | 是 |

### Workflow State

`status` 仍然是任务是否可执行、是否被 claim、是否 blocked/review/done 的唯一事实来源。

任何新字段都不能隐式表达状态：

- `task_type=bug` 不表示高优先级。
- label `blocked` 不表示 task blocked。
- decision selected option 不表示 task done。
- comment 中写 “blocked” 不改变 task status。

状态变化只能通过 transition command。

### Task Type

`task_type` 表达“这个 task 是什么工作类别”，不表达“它现在处于什么执行状态”。

建议第一批 task types：

```text
bug | feature | research | ops | docs | refactor | test | follow_up
```

`task_type` 可以用于：

- Desktop/List/Board 筛选。
- Search/context 过滤。
- 依赖、search 和 context 解释。
- 未来排序加权。

`task_type` 不用于：

- dispatcher claim eligibility。
- 状态机 transition guard。
- hard dependency 判断。
- 替代 labels。

枚举策略：

- 第一版使用受控枚举。
- 后续如需要开放扩展，再单独做 ADR。
- 未知 type 应被拒绝，而不是静默写入。

### Labels

labels 表达多维、可叠加的分类。一个 task 可以有多个 label。

labels 适合表达：

- area：`desktop`、`cli`、`sqlite`
- domain：`search`、`dispatcher`、`comments`
- semantic group：`llm-facing`、`release-risk`
- 用户临时整理方式

labels 不适合表达：

- workflow state
- hard dependency
- execution ownership
- decision result

未来 semantic label recommender 可以推荐 label，但推荐结果必须显式保存后才成为 task label。

### Dependency Type

现有 dependency 的核心语义是 hard prerequisite：

```text
parent done or archived => child may become ready
parent neither done nor archived => child cannot be ready/running
```

引入 `dependency_type` 后，必须保留 hard dependency 的清晰语义。

建议第一批 dependency types：

| Type | 语义 | 是否阻塞 child |
|---|---|---:|
| `blocks` | parent 是 child 的硬前置条件 | 是 |
| `relates_to` | 相关任务，仅用于导航/search/context | 否 |
| `informs` | parent 提供背景、设计输入或决策依据 | 否 |
| `spawned_from` | child 由 parent 执行过程中发现 | 否 |
| `duplicates` | 重复或替代关系 | 否 |

只有 `blocks` 参与：

- dependency blocked 判断
- promote guard
- claim guard
- dispatcher eligibility
- hard dependency blocking

soft dependency 可以进入 Desktop 展示、search 和 context，但不能让任务变成 blocked，也不能阻止 claim。

### Comment Author Model

comment 的作者模型只表达“谁写的”。

本项目是本地 dogfood 工具，不建用户系统。作者角色只保留两类：

```text
user | agent
```

规则：

- `user`：本地操作者，也就是“我”。
- `agent`：不是我写的，都算 agent。
- `author`：展示名，例如 `kanban-user`、`codex`。
- `agent_type`：仅当 `author_type=agent` 时可用，例如 `codex`、`executor`、`reviewer`、`dispatcher`。
- 不引入 users table、identity table、RBAC 或权限模型。

这意味着不再使用 comment kind 表示 `system`、`worker` 或 `agent`。这些都属于 author/source 轴。

### Comment Kind Model

`comment.kind` 只表达“这条 comment 的内容语义”。

第一版只保留两类：

```text
note | decision
```

#### `note`

普通协作记录。包括：

- 进展说明
- 交接记录
- 执行总结
- 问题描述
- reviewer 反馈
- 验证记录
- 人或 agent 的普通回复

“遇到的问题”默认也是 `note`。如果问题真的阻塞任务，应该同时通过 transition command 把 task 变成 `blocked`，并写入 `status_reason`。

#### `decision`

结构化选择记录。用于表达：

- 有多个 option。
- 最终选择了其中一个。
- 有选择理由、风险和验证方式。

decision 不是 task status，不是 event，不是 ADR 替代品。

### Comment Metadata

`comment.metadata_json` 是 `comment.kind` 的结构化 payload。

规则：

- `kind=note` 时，metadata 默认 `{}`。
- `kind=decision` 时，metadata 必须符合 decision schema。
- metadata 非法 JSON 或 schema 不匹配时拒绝写入。
- metadata 不参与状态机。
- metadata 不替代 event。
- metadata 不应该变成随意塞字段的长期垃圾桶。

### Decision Comment Schema

建议第一版 shape：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "options": [
    {
      "slug": "comment-metadata",
      "title": "Use comment metadata",
      "detail": "Store structured decision data in task_comments.metadata_json."
    },
    {
      "slug": "decision-table",
      "title": "Create decision table",
      "detail": "Create a separate task_decisions table with option rows."
    }
  ],
  "selected": "comment-metadata",
  "reason": "Keeps decisions close to task discussion and avoids a parallel timeline.",
  "risk": "metadata schema needs validation discipline.",
  "verification": "CLI/API/Desktop tests cover creation, reading, rendering, and invalid metadata rejection."
}
```

Validation rules:

- `options` 必须非空。
- 每个 option 必须是 object，且有非空 string `slug`、`title`、`detail`。
- 每个 option 必须有唯一 `slug`。
- `selected` 必须匹配某个 option slug。
- `reason` 必填且非空。
- `risk` 可选但推荐；如果出现，必须是非空 string。
- `verification` 可选但推荐；如果出现，必须是非空 string。
- `slug` 使用稳定小写 ASCII slug，必须以小写字母或数字开头，只包含小写字母、数字和 `-`，便于 CLI、JSON 和前端引用。
- `detail` 可以是 Markdown 文本，但 Desktop 渲染必须走安全 markdown 规则。

### Desktop Rendering Rules

Desktop TaskDetail comment list：

- `note`：按普通 markdown comment 渲染。
- `decision`：
  - 展示 comment body 作为自然语言摘要，例如“已决定继续使用 comment metadata 承载结构化决策信息，正文保留为简短结论，方便没有结构化渲染的环境阅读。”
  - 展示所有 option slug。
  - selected option 使用明确绿色/selected 状态。
  - 点击 option 展开 `title` 和 `detail`。
  - 展示 reason、risk、verification。
  - 如果 decision metadata 无效，不应该静默当作 selected；应显示错误状态或 degraded note。

### CLI / API Rules

CLI：

```bash
kanban comment add <task-ref> "<body>"
kanban comment add <task-ref> "<body>" --kind note
kanban comment add <task-ref> "<body>" --kind decision --metadata-json '<json>'
```

`kind=decision` 的 body 是自然语言 fallback 摘要，不重复完整选项表；`options`、`selected`、`reason`、`risk` 和 `verification` 只放在 `metadata_json` 中，由 Desktop 在正文下方结构化渲染。

可后续增加更友好的命令：

```bash
kanban decision add <task-ref> ...
```

但第一版不要求。

API：

- comment create request 显式包含：
  - `body`
  - `author_type`
  - `author`
  - `agent_type`
  - `kind`
  - `metadata`
- comment response 返回同样字段。
- 不再把 `system/worker` 作为 kind 返回。

### Event Kind

`event.kind` 只记录系统事实：

- `comment.added`
- `task.created`
- `task.updated`
- `task.claimed`
- `task.completed`
- `dependency.added`

event 不承载 decision 本体。添加 decision comment 时，event 只记录 `comment.added`，decision 内容在 comment snapshot 中。

### Dispatcher And Frontier Rules

Dispatcher claim eligibility 只能看：

- `status`
- hard dependency (`dependency_type=blocks`)
- `scheduled_at`
- claim token / lease
- board archived state
- assignee / worker profile

Dispatcher 排序可以看：

- `priority`
- `created_at`
- future explicit dispatcher policy

Dispatcher 不看：

- `task_type`
- labels
- `comment.kind`
- `comment.metadata_json`
- decision selected option
- soft dependency

Frontier 可以展示和解释更多字段，但不得把 soft 字段解释成 hard blocker。

### Migration Strategy

本项目是 dogfood 版本，不做沉重兼容层。采用直接 schema train：

1. 本 ADR 固定边界。
2. 修改 `task_comments`：
   - 增加 `author_type`
   - 保留/明确 `author`
   - 增加 `agent_type`
   - 收窄 `kind` 为 `note | decision`
   - 增加 `metadata_json`
3. 更新 Rust domain/API/CLI/Desktop type。
4. 迁移现有 comment：
   - 不是用户本人写的，一律 `author_type=agent`
   - 用户本人写的，`author_type=user`
   - 普通历史 comment 一律 `kind=note`
   - 已有 decision comment 若能识别则 `kind=decision`，否则 `note`
5. 实现 decision metadata validation。
6. 实现 Desktop decision rendering。
7. 后续再做 `task_type`、`dependency_type`、labels 扩展。

### Consequences

优点：

- comment 模型语义清楚：作者归作者，内容类型归内容类型。
- decision comment 可以成为真正结构化对象。
- Desktop 渲染会简单很多。
- LLM/agent 做选择时可以留下可索引、可展开、可复盘的记录。
- 不再让 `system/worker` 这类来源概念污染 content kind。

代价：

- 需要 schema migration。
- 需要一次性更新 CLI/API/Desktop。
- 旧 comment JSON shape 会改变。
- 需要认真做 decision metadata validation，避免 `metadata_json` 变成任意垃圾桶。
- 全局 `kanban-tool` skill 需要同步，因为 CLI/API/comment JSON 行为会变化。

### Non-Goals

- 不引入多用户系统。
- 不引入 RBAC、团队、组织、邀请或云同步。
- 不用 decision comment 替代 ADR。
- 不让 comment metadata 影响 dispatcher claim。
- 不让 labels/type/metadata 变成隐式 status。
- 不把 `task_dependencies` 改成完整知识图谱。
- 不在本 ADR 中实现具体 migration。

---

## ADR-0012：Label Proposal Provider 边界

### Status

Accepted

### Context

Semantic label suggestion 的日常路径应保持 deterministic：SQLite 存 canonical
`labels` / `task_labels` / `label_semantics` / `label_atoms`，LanceDB 只是
`kb_label_atoms` derived index，solver 只做本地向量计算。Label proposal 是“coverage
不足时建议新 label semantics”的可选流程，它可以由人工、离线工具或未来本地 LLM provider
产生 candidate。

真实 LLM provider 如果直接放进 `kanban-sqlite`，会把外部 SDK、HTTP client、prompt、
credentials 和 runtime 配置拖入 SQLite service。这样会破坏本项目的 local-first / SQLite-only
边界，也会让 proposal validation 与模型调用耦合过深。

### Decision

`kanban-sqlite` 只定义并消费 `LabelProposalProvider` trait：

- `DisabledLabelProposalProvider`：默认 provider 不可用，返回 degraded attempt，不写 canonical label。
- `ManualLabelProposalProvider`：接收 CLI/API 显式传入的本地/offline candidate。
- `propose_task_label_with_store`：从 SQLite 读取 task 和 suggestion context，调用 provider，
  然后执行 deterministic validation、residual top1+margin gate、proposal persistence、
  accept/reject lifecycle。

真实 LLM provider 不属于 `kanban-sqlite`。可选实现位置是：

- `kanban-server`：当 localhost server 显式配置本地 provider/runtime 时注入 trait object。
- `kanban-cli` 或本地 runtime：当命令显式读取本地/offline candidate 或未来本机模型输出时注入。
- 独立 `kanban-ai` / `kanban-llm` crate：承载 SDK、HTTP client、prompt 和 credential 读取，
  再向上层暴露实现 `LabelProposalProvider` 的 adapter。

### Consequences

优点：

- SQLite service 不依赖 LLM SDK、HTTP AI client、runtime credentials 或外部模型配置。
- proposal lifecycle 仍由 deterministic SQLite service 守住，不会因为 provider 类型不同而绕过
  residual validation 或 accept/reject gate。
- 日常 `label suggest` 不依赖 proposal provider；provider 不可用只是 degraded proposal attempt。
- 未来 provider 可以替换或禁用，不需要改 canonical label truth 或 task label binding 语义。

代价：

- 真实 provider 需要在上层做 adapter 和配置装配。
- server/CLI 需要明确区分“candidate 生成失败”和“SQLite validation 拒绝 candidate”。
- 需要持续避免把 prompt、credential、HTTP retry 等 concerns 下沉进 `kanban-sqlite`。

### Non-Goals

- 本 ADR 不实现真实 LLM provider。
- 不上传本地 task 数据到远程服务。
- 不让 provider 自动绑定 task label。
- 不改变 proposal accept 后才创建 label semantics / atoms 的生命周期。

---

## ADR-0013：暂不引入 label ontology graph projection

### Status

Accepted

### Context

当前 label ontology 已有 SQLite truth 与查询面：

- `labels` / `task_labels` 表达当前 task label binding truth。
- `label_semantics` 表达 canonical ontology semantics；`label_atoms` 是从 semantics 与
  label name 展开的 SQLite materialized projection。
- `label_semantic_proposals` 表达新 label proposal lifecycle。
- `label_ontology_observations` / `signals` / `actions` / `action_signals` 表达
  provenance、review、mutation 和 validation history。
- `label ontology review`、`label atom explain`、JSONL export/import 和 doctor 已经从
  SQLite records 直接回答第一批 review/provenance 问题。

项目也已有通用 Knowledge Substrate graph：`entity_relations` 作为 SQLite mirror，
Oxigraph 作为可重建 derived store，`index_outbox` / `derived_store_state` 管理 dirty、
sync 和 rebuild。这个 graph 当前覆盖 task-board、task dependency 等通用 entity
关系，不覆盖 label ontology ledger。

第一版 ledger 还没有明确的关系查询需求需要 ontology-specific graph。过早投影 signals、
actions、atoms 和 proposals 会增加 schema、outbox、query API 和 rebuild 复杂度，并提高把
graph 误当第二 truth 的风险。

### Decision

暂不实现 label ontology graph projection。

在 rename/split/merge、cross-action provenance、atom lineage 或 review workbench 出现明确
关系查询需求前，ontology 查询继续走 SQLite service/API：

- `label ontology review`
- `label ontology show`
- `label atom explain`
- `label proposal list/show`
- JSONL export/import 与 doctor

未来若新增 ontology graph projection，它必须满足：

- SQLite `labels`、`task_labels`、`label_semantics`、`label_atoms`、proposal 和
  `label_ontology_*` 仍是事实来源；`label_atoms` 是 projection，不是独立 semantic truth。
- projection 只能从 SQLite 快照/outbox 派生，可删除重建。
- projection 状态通过 `index_outbox` 和 `derived_store_state` 或等价派生层控制面表达。
- graph API 只能查询 relation/provenance，不提供 confirm/apply/validate/revert/bootstrap
  或其它 canonical mutation 写入口。
- graph dirty、error、删除或重建失败不改变 task status、task labels、semantics、atoms、
  proposal 或 ledger rows。

### Consequences

优点：

- 第一版 ontology workflow 保持简单，避免过早增加第二个 provenance 表达。
- SQLite ledger/review/explain 继续作为可审计事实来源。
- 未来如果确有查询需求，可以复用已存在的 Knowledge Substrate derived-store contract。
- graph 故障不会影响 ontology mutation、validation 或 review 的 canonical state。

代价：

- 复杂 lineage / relationship traversal 暂时需要通过 SQLite query、review grouping、
  `atom explain` 或导出后离线分析完成。
- 未来若要支持 ontology graph，需要单独设计 projection schema、outbox fanout 和 rebuild
  测试。

### Non-Goals

- 本 ADR 不新增 ontology RDF schema。
- 不把 `label_ontology_*` rows 写入 `entity_relations`。
- 不扩展 `kanban graph` 为 ontology mutation API。
- 不用 graph 替代 label ontology review、show、atom explain 或 validation history。

---

## ADR-0014：Label ontology closure contract

### Status

Accepted

### Context

Label identity CRUD、task label binding、semantics mutation、proposal accept、bootstrap、
validation 和 review lifecycle 曾经混用 provenance 语义。最危险的问题是 routine task
capture 可以隐式创建 vocabulary，label identity delete 可以隐式删除 semantics/atoms，
semantics mutation 会 fan out 多条 per-atom action，trusted validation raw JSON 可能绕过
collector，bootstrap verify 曾依赖 post-commit compensation。

### Decision

采用收窄后的 closure contract：

- `labels` identity CRUD 是基础 vocabulary registry，不写 ontology mutation action；task
  label binding 只绑定已存在 label，写普通 task event。
- `label delete` 永不隐式删除 `label_semantics` / `label_atoms`；force 只允许移除 task
  bindings 后删除空 identity。
- `label_semantics` / `label_atoms` canonical mutation 一次 transaction 只写一条 root
  mutation action；实际 atom delta 写入 `label_ontology_action_atom_effects` 的
  `added` / `removed` rows。No-op 不写 action/effects，也不标脏 index。
- Semantics clear 继续使用 `update_semantics` action type，必须有 actor、非空 reason 和
  `expected_semantics_hash`。
- Atom explain 优先读取 effect rows；legacy per-atom actions 只做兼容读取，不回写压缩历史。
- Trusted automated validation 只能由 CLI collector 生成，表示 current hash/index
  generation 和指定 cases/controls 机械通过，不表示全局语义正确。
- CLI bootstrap verify 是 pre-commit staged verification；失败、provider unavailable 或
  verify/commit 间 state 变化时零 canonical 写入。
- `validation_requirement` 与 validation attempt outcome 分离；effective outcome 是查询
  reducer 结果。Unsupported parent 可记录 external failed/partial 诊断，但不能 passed。
- Public structure plan write 入口关闭；rename/split/merge 暂仅可作为 review signal 或
  legacy action 读取。

### Consequences

优点：

- Routine task capture 不能再绕过 vocabulary adoption。
- Ledger 行数随真实 mutation 数线性增长，atom explain 粒度来自 effect rows。
- Destructive semantics clear 有 CAS、reason 和 revertable root action。
- Trusted/external validation 边界由 Rust visibility、collector entry 和 tests 锁住。

代价：

- 旧 per-atom action 保留历史噪声，需要 explain/revert 的 legacy compatibility。
- Base label identity delete 需要用户先显式 clear semantics。
- Structure mutation 需要未来单独 typed apply、binding migration 和 validation policy。

### Non-Goals

- 不新增 action type、signal type、validation status 或 graph/dashboard projection。
- 不回写或压缩历史 per-atom actions。
- 不实现 rename/split/merge canonical mutation。

## ADR-0013: Generic Signal Ledger

### Status

Accepted

### Context

Agent/Product failures and observations need a durable review lifecycle that is not label-specific and is not just free-form comment metadata.

### Decision

Add board-scoped `signal_observations` and `signals` tables. `kanban signal record` writes observation and signal rows, and when task context exists it writes a short `task_comments.kind = signal` backlink in the same SQLite transaction. Lifecycle review supports `open -> confirmed|rejected|superseded|resolved` and `confirmed -> resolved`; supersede requires same-board replacement and cycle prevention. V1 does not automatically create follow-up tasks.

### Consequences

Signal ledger becomes the canonical place for generic agent/product signals. Label ontology ledger remains label-specific and is not reused for generic product signals.


## ADR: API/SSE transport descriptor 作为单一 method/path authority

- 决策：在 `kanban-contract` default feature 保存 84 个 API/SSE descriptor；server router 以 `operation_id` + 显式 `adapter_id` 绑定真实 handler，并读取 descriptor method/path。
- 原因：此前 `SurfaceOperation` 与 router 各自手写 method/path，虽然有 parity test，仍保留双写漂移面。
- 后果：`SurfaceOperation` 的 API/SSE 记录改为投影；CLI/JSONL 保持其独立 inventory。schema root 使用 `contract_id`，不与 endpoint `operation_id` 混淆。DTO/schema adoption 不在本决策中提前完成。


## ADR: B1-A Error 与 delete response 的 wire 收口边界

- 决策：`ErrorBody.code` 使用闭合的 `ApiErrorCode`，server adapter 显式将 `KanbanError` 映射为 enum；label semantics delete handler 使用 `DeleteResponse`/`DeleteResult`，不再公开 `DataEnvelope<serde_json::Value>`。
- 原因：稳定 error code 与固定 delete acknowledgement 已具备可验证 wire 形状；把任意 `String`/`Value` 留在公开边界会削弱 schema、typed consumer 与 drift gate。
- 后果：该决策只拥有 wire/schema evidence。HTTP status、locale message、service guard、状态机、CAS、transaction 与 SQLite 继续由 adapter/service/core 负责。delete endpoint 的 path/query/header/body obligations 尚未建模，因此 endpoint 与 response migration 均保持 `generated`，不以局部 response typing 提前关闭 adoption。


## ADR: B1-C0 Transport location、cardinality 与 exact/shared binding

- 决策：API/SSE semantic contract 显式声明 `Http { operation_key, location, parameters }`，非 HTTP contract 显式声明 `NoTransport`；parameter cardinality 只允许 `RequiredOne|OptionalOne|RepeatedOrdered`。`Success` 只表示 2xx success；非 2xx `Error` 是仅允许 `SharedComponent` 的第七个 transport location，但 endpoint 仍只有 path/query/headers/body/success/SSE 六类 obligation。任意 `Adopted` contract 和 endpoint exact reference 都必须是 `granularity=Exact`。
- 原因：仅有 contract ID 与 input/output 方向无法区分 path/query/header/body/2xx success/shared error/SSE，也无法证明 query 重复值顺序、path placeholder 映射或 shared error envelope 的真实复用关系；把 error 继续标成 `Success`、允许 Family 冒充 exact，都会使 coverage 失真。
- 唯一性：endpoint exact binding 不维护全局 second-binding map。method/path 唯一、contract 的精确 `operation_key` 和单一 location 已共同推出合法 binding 唯一：同 route 的第二个 endpoint 先被 method/path 拒绝，不同 route 被 operation key 拒绝，同 endpoint 的第二个 obligation 被 location 拒绝。surface catalog 的重复 exact reference 仍是可达输入，继续单独 fail closed。
- Shared orphan policy：`SharedComponent` 可以跨多个 endpoint 复用且永不计入 exact/adoption coverage。generated/adopted shared 满足“至少一个显式 linkage”或“同 surface 的真实 adoption witness”之一即可；只有两者均缺失才是 orphan。`api.error.response` 使用 `location=Error`，当前由 list-tasks 显式链接。
- 后果：validator 对 unknown/`Planned`/`Excluded` 引用、错误 binding/granularity/location/direction/operation/surface、path 名称/缺失/额外/顺序/大小写漂移、header 大小写冲突、非法 parameter location 和 shared miscount fail closed。13 个 B1-B lifecycle request 保持 body transport 与既有 runtime 语义；本决策不迁移 handler DTO、不改变 HTTP status/service/state-machine 行为，也不关闭 endpoint `Todo`。冻结值为 `stream-events.sse=Todo`、endpoint `Todo=389`、总未闭合 `636`。


## ADR: B1-C1 Task-read exact path/query contract 与单一 ordered parser

- 决策：`GET /api/v1/boards/:board/tasks` 与 `/tasks/by-status` 分别拥有独立 path/query DTO，
  形成 4 个 `Adopted` exact contract。两个 server-local typed Axum extractor 分别绑定对应
  `Path<...>`，并各自从 `parts.uri.query()` 读取一次 raw URI 后进入共享 ordered parser；handler
  只接收已绑定的 request，不持有 `RawQuery`、`Query<T>` 或第二个 raw source。
- Query grammar：只有 `status`、`priority`、`label`、`plan_filter` 是
  `RepeatedOrdered`；其余 scalar 重复、未知 key 与旧 `search` alias 均返回
  `400 invalid_input`。54-pair 上限由 9/4/3/32 个 repeated budgets 加 6 个 scalar 参数推导；
  raw query 上限为 8192 bytes。`q` 是唯一文本搜索 key。label 会 trim Unicode 边缘空白，
  但纯 Unicode 空白失败关闭；percent/UTF-8、enum、priority、limit、offset 和 sort 边界由
  真实 router URI matrix 固定。
- 证据边界：每个 contract 都有独立 DTO-to-fixture producer 和 fixture-to-real-router consumer；
  非默认 board sentinel 证明真实 path consumption。AST tests 锁定 DTO ownership、typed
  extractor、两个 raw URI 消费点及 handler `&path.board` 到 `list_tasks_page` 的实参，并以显式
  mutation 覆盖 alias、private DTO、wrong extractor、dual source、second raw parser 和两个
  handler 各自的 `path.board -> default`。producer/consumer region guard 只证明当前源码区域直接
  分离，不把任意未来共同 helper indirection 夸大为 mutation-complete 证明。
- 后果：Desktop/Web/CLI 的 HTTP caller 必须使用上述 grammar；现有 Desktop caller 已使用 `q`
  并保留 repeated 参数顺序。SQLite service 的 defensive limit 直接引用唯一 application authority，
  server equality gate 覆盖该实际 service path；service 查询行为与 core 状态机不变。GET body 为
  `NotApplicable`，headers 与 success response 保持 `Todo`，所以两个 endpoint 只推进到
  `Generated`，不提前声称完整 adoption。冻结值变为 `Contract=19`、`Todo=383`、
  `NotApplicable=102`、总未闭合 `630`。


## B1-C2b task-read 成功响应决策

决定让两个 task-read endpoint 分别拥有闭合响应 contract，只复用 `ApiTask`、`ApiLabel` 与既有 pagination primitives，避免共享 envelope 掩盖 endpoint 差异。行为细节以 [API_SPEC](API_SPEC.md#b1-c2b-task-read-成功响应契约) 和 [SCHEMA_CONTRACTS](SCHEMA_CONTRACTS.md#b1-c2b-task-read-成功响应契约) 为准。
## ADR-0015：Oxigraph quick-xml 安全临时 vendor patch

### Status

Accepted（Phase 2 temporary exception）

### Context

`oxrdfxml 0.2.3` 与 `sparesults 0.3.3` 的 crates.io 版本仍解析到受 RUSTSEC-2026-0194/RUSTSEC-2026-0195 影响的 `quick-xml < 0.41`；security commit `52870a3` vendor 了上游修复源码并统一到 `quick-xml 0.41.0`。

### Decision

允许 root `Cargo.toml` 唯一的 `[patch.crates-io]` 例外，且仅接受 `oxrdfxml`/`sparesults` 两个精确仓内 vendor 路径、package name/version 与普通文件目标。`schema_dependency_policy` 对额外 key、非精确 source/path、path traversal、symlink、全部 `[replace]` 保持 fail-closed；schema-tool registry closure 不变，产品图继续禁止 schema tooling 泄漏。

由 security owner 维护，待 crates.io 上游版本发布并确认 `quick-xml >= 0.41` 后移除 vendor、`[patch]`、lockfile 变更及本 ADR；advisory、provenance 或 vendor digest 变化必须重新 review。复核期限：2026-10-12。


---

# File: migrations/001_initial.sql

```sql
-- Kanban Tool initial SQLite schema
-- Time convention: INTEGER unix epoch milliseconds UTC.
-- JSON convention: TEXT with CHECK(json_valid(...)).

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 120000;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;

BEGIN;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL DEFAULT '',
  applied_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS boards (
  id TEXT PRIMARY KEY CHECK(id LIKE 'b_%'),
  slug TEXT NOT NULL UNIQUE CHECK(length(trim(slug)) > 0),
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  description TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER
);

CREATE TABLE IF NOT EXISTS board_columns (
  id TEXT PRIMARY KEY CHECK(id LIKE 'col_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  position INTEGER NOT NULL,
  hidden INTEGER NOT NULL DEFAULT 0 CHECK(hidden IN (0, 1)),
  wip_limit INTEGER CHECK(wip_limit IS NULL OR wip_limit >= 0),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, status),
  UNIQUE(board_id, position)
);

CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY CHECK(id LIKE 't_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL,

  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  description TEXT,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  status_reason TEXT,

  assignee TEXT,
  priority INTEGER NOT NULL DEFAULT 0,
  position INTEGER NOT NULL DEFAULT 0,

  scheduled_at INTEGER,
  due_at INTEGER,

  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  started_at INTEGER,
  completed_at INTEGER,
  archived_at INTEGER,

  claim_token TEXT,
  claim_owner TEXT,
  claim_expires_at INTEGER,
  last_heartbeat_at INTEGER,
  current_run_id TEXT,

  retry_count INTEGER NOT NULL DEFAULT 0 CHECK(retry_count >= 0),
  max_retries INTEGER CHECK(max_retries IS NULL OR max_retries >= 0),

  result_summary TEXT,
  result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),

  lock_version INTEGER NOT NULL DEFAULT 0 CHECK(lock_version >= 0),

  UNIQUE(board_id, seq),
  UNIQUE(id, board_id),
  CHECK(
    (status != 'running') OR
    (claim_token IS NOT NULL AND claim_owner IS NOT NULL AND claim_expires_at IS NOT NULL)
  )
);

CREATE TABLE IF NOT EXISTS task_dependencies (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  child_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(child_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_runs (
  id TEXT PRIMARY KEY CHECK(id LIKE 'r_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,

  status TEXT NOT NULL CHECK(status IN ('running', 'succeeded', 'failed', 'canceled', 'expired')),
  worker_profile TEXT,
  worker_pid INTEGER,

  claim_token TEXT NOT NULL,
  claim_owner TEXT NOT NULL,
  claim_expires_at INTEGER NOT NULL,

  started_at INTEGER NOT NULL,
  last_heartbeat_at INTEGER,
  finished_at INTEGER,

  exit_code INTEGER,
  summary TEXT,
  error TEXT,
  log_path TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),

  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_comments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author TEXT NOT NULL,
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'text' CHECK(kind IN ('text', 'system', 'worker', 'decision')),
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK(event_id LIKE 'e_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  actor TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_attachments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'a_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  filename TEXT NOT NULL CHECK(length(trim(filename)) > 0),
  rel_path TEXT NOT NULL CHECK(length(trim(rel_path)) > 0),
  content_type TEXT,
  size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
  sha256 TEXT,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS labels (
  id TEXT PRIMARY KEY CHECK(id LIKE 'l_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  color TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, name),
  UNIQUE(id, board_id)
);

CREATE TABLE IF NOT EXISTS task_labels (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(task_id, label_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL CHECK(json_valid(value_json)),
  updated_at INTEGER NOT NULL
);

-- Indexes: tasks
CREATE INDEX IF NOT EXISTS idx_tasks_board_status_position
  ON tasks(board_id, status, position);

CREATE INDEX IF NOT EXISTS idx_tasks_board_priority_created
  ON tasks(board_id, priority DESC, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_tasks_assignee_status
  ON tasks(board_id, assignee, status);

CREATE INDEX IF NOT EXISTS idx_tasks_scheduled
  ON tasks(board_id, status, scheduled_at);

CREATE INDEX IF NOT EXISTS idx_tasks_claim_expiry
  ON tasks(board_id, status, claim_expires_at);

CREATE INDEX IF NOT EXISTS idx_tasks_updated
  ON tasks(board_id, updated_at DESC);

-- Indexes: dependencies
CREATE INDEX IF NOT EXISTS idx_deps_child
  ON task_dependencies(child_task_id);

CREATE INDEX IF NOT EXISTS idx_deps_parent
  ON task_dependencies(parent_task_id);

-- Indexes: runs
CREATE INDEX IF NOT EXISTS idx_runs_task_started
  ON task_runs(task_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_runs_status
  ON task_runs(board_id, status, started_at DESC);

-- Indexes: comments
CREATE INDEX IF NOT EXISTS idx_comments_task_created
  ON task_comments(task_id, created_at ASC);

-- Indexes: events
CREATE INDEX IF NOT EXISTS idx_events_board_id
  ON task_events(board_id, id ASC);

CREATE INDEX IF NOT EXISTS idx_events_task_created
  ON task_events(task_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_events_kind_created
  ON task_events(kind, created_at DESC);

-- Indexes: labels
CREATE INDEX IF NOT EXISTS idx_task_labels_label
  ON task_labels(label_id, task_id);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (1, '001_initial', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
```


---

# File: migrations/003_comment_author_identity.sql

```sql
-- Add explicit comment author identity while preserving existing kind values.

BEGIN;

ALTER TABLE task_comments
  ADD COLUMN author_type TEXT NOT NULL DEFAULT 'human'
  CHECK(author_type IN ('human', 'agent', 'system'));

UPDATE task_comments
SET author_type = CASE kind
  WHEN 'worker' THEN 'agent'
  WHEN 'system' THEN 'system'
  ELSE 'human'
END
WHERE author_type = 'human';

ALTER TABLE task_comments
  ADD COLUMN agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (3, '003_comment_author_identity', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
```

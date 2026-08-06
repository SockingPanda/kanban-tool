# Kanban Tool

Kanban Tool 是本地优先、单机、单用户的看板与 durable work queue。任务、依赖、评论、执行记录、事件、labels、ontology、signals、entities、relations、检索和派生状态都归属于同一个 canonical Turso 数据库。

产品只有一条运行路径：

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP / SSE
    kanban serve（唯一 host）
        ↓
ApplicationService + 状态机 + 事务
        ↓
kanban-service（唯一 Turso owner）
        ↓
canonical Turso database
```

只有 `kanban serve` 可以打开、初始化、迁移、备份、替换和关闭数据库。其他入口不会直开数据库，也没有 host 不可用时的 embedded/SQLite fallback。

## 快速开始

```bash
cargo install --path crates/kanban-cli --bin kanban
kanban serve
```

默认 host 为 `http://127.0.0.1:8721`，默认数据库为 `$XDG_DATA_HOME/kb/kanban.db`（未设置时通常为 `~/.local/share/kb/kanban.db`）。数据库路径只在 `serve` 或配置解析中使用，可由 `--db`、`KANBAN_DB` 或兼容变量 `KB_DB` 覆盖。

项目 shell 命令不触碰数据库：

```bash
kanban init
kanban config show
kanban board use agent-work
kanban board current
```

启动 host 后可以使用完整的任务和知识路径：

```bash
kanban board list
kanban board columns default
kanban task create "整理项目首页" --description "让第一次访问的人看懂项目"
kanban task step not-required default#1 --reason "单步任务"
kanban task specify default#1 --description "补充可执行规格"
kanban task promote default#1
kanban task claim default#1
kanban search "项目首页"
kanban index rebuild
kanban index sync
kanban entity upsert --uri 'kb://task/t_example' --kind task --source-table tasks --source-id t_example
kanban context build default#1
kanban graph neighborhood default#1
kanban --board default graph map
kanban vector status
```

所有 mutation 都由 host 的 `ApplicationService` 和同一事务边界校验；adapter 只负责 typed 请求、错误映射和展示。

## 入口和功能面

- **CLI**：普通命令通过 `kanban-client` 访问 localhost；`serve`、`init`、配置/board 选择、completion 和 Codex hook 是本地 shell 或 host 装配命令。
- **MCP**：`kanban-mcp` 使用 stdio 和 `rmcp`，当前工具清单由 `crates/kanban-mcp/src/main.rs` 的稳定 inventory 测试锁定；所有 tool 都调用 typed client，不启动 host、不直接写数据库。
- **Desktop**：Tauri/React shell 通过 typed HTTP 使用 `board`、`list`、`map`、`events`、`runs`、`signals`、`ontology`、`maintenance`、`health`、`settings` 十个导航视图；task detail、attachments、steps、comments、dependencies、context 和 maintenance 继续复用同一 host。

CLI 的 canonical leaf 和 HTTP 的 method/path 由 `kanban-protocol` 的
`surface_operation_catalog()`/`endpoint_catalog()` 固定；可见 alias 只改善交互，不增加第二条
contract operation。当前知识面包含 board columns、entity upsert、task specify、graph
neighborhood/map，以及 search index 的 status/doctor/rebuild/sync；完整参数和 wire 形状见
[`docs/CLI_SPEC.md`](docs/CLI_SPEC.md) 与 [`docs/API_SPEC.md`](docs/API_SPEC.md)。

跨入口的领域面包括：

| 领域 | 当前 owner / 派生边界 |
| --- | --- |
| 看板、任务、计划、步骤、依赖、评论、runs、events | `kanban-service` canonical facts、状态机和 event transaction |
| labels、ontology、signals | service canonical ledger；ontology atom index 和 provider 结果可重建 |
| search | Turso FTS `task_search_fts`；未 ready/stale 时由 service 回退 canonical SQL |
| graph | `entities`、`relation_predicates`、`entity_relations` canonical；service 执行有深度上限、环检测和 board isolation 的 BFS |
| vector/context | Turso `vector32`、host 内 Ollama provider 和 bounded context merge；provider 不可用时返回 degraded diagnostics |
| maintenance/migration | host-owned doctor、checkpoint、backup、portable import/export、vacuum、projection rebuild/cleanup，以及可选 `legacy-sqlite-import` v30 importer |

FTS、vector、graph、context 和 projection 均可从 canonical facts 删除后重建，不能反向改变业务事实。

## 状态和 dispatcher

权威状态为 `triage|todo|scheduled|ready|running|blocked|review|done|archived`；`board_columns` 只是展示映射。`ready → running` 只能通过原子 claim，并与 active run、lease 和 `task.claimed` event 同事务提交。heartbeat、release、review、done、block、specify、unblock、reopen、reclaim 和 archive 都是显式 service commands，不提供任意 `transition(target_status)`。

`kanban serve` 默认不消费队列。只有 `--dispatcher-profile <path>` 才在同一 host 进程启动单 worker dispatcher；它只 claim `ready`，复用共享 claim/heartbeat/finish path，不会 claim `review`、`todo` 或 `scheduled`。

## 数据迁移

当前支持两条互补路径：

1. **Turso v1 → v2 原地升级**：host 先验证 schema family、shape、foreign keys 和 board isolation，创建已验证 sibling backup，再在事务内升级；失败回滚并保持旧数据库可启动。
2. **portable/legacy 导入**：portable JSONL 只导入 canonical facts，提交后入队 FTS/vector/graph rebuild；`import-v30` 读取 legacy SQLite v30，只在显式启用 `legacy-sqlite-import` feature 的 host 上执行，默认构建 fail-closed。

两条路径都经 host-owned `import_journal`、fingerprint、staging 和 recovery 语义；CLI、HTTP 和 Desktop 只请求 host，MCP 不暴露数据库替换、backup、vacuum 或 migration 管理命令。

## 工作区和文档

产品单元是七个 Rust crate（`kanban-core`、`kanban-service`、`kanban-protocol`、`kanban-client`、`kanban-server`、`kanban-cli`、`kanban-mcp`）、Desktop Tauri package `kanban-desktop` 和私有 `xtask`。旧 backend/helper sidecar 已从 active workspace 删除；相关 release/projection runbook 仅作为历史归档，不是当前 runtime 或 release gate。

核心事实源：

- [`docs/SPEC.md`](docs/SPEC.md)：产品范围和当前行为；
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)：进程、crate、ownership 和派生边界；
- [`docs/STATE_MACHINE.md`](docs/STATE_MACHINE.md)：状态、transition、claim、lease 和 dispatcher；
- [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md)：canonical schema、约束和导入；
- [`docs/API_SPEC.md`](docs/API_SPEC.md)、[`docs/CLI_SPEC.md`](docs/CLI_SPEC.md)：HTTP/CLI contract；
- [`docs/SCHEMA_CONTRACTS.md`](docs/SCHEMA_CONTRACTS.md)：protocol/schema/adoption evidence；
- [`docs/migration/turso-full-feature-parity.md`](docs/migration/turso-full-feature-parity.md)：`6ea277` baseline 到当前 owner/surface/test/gate 的 parity ledger。

文档同步使用 `just diff-check`、`just spec-bundle-check` 和受影响的 schema marker 检查。adoption、full、release、push、PR、merge 和发布均不因文档更新自动视为通过或执行。

## 明确边界

本项目不提供多用户/RBAC/云同步/远程访问、第二 canonical backend、第二 mutation path、自定义 IPC、自动 server supervision 或发布/PR 工作流。旧 Tantivy/LanceDB/Oxigraph/helper sidecar 只在历史迁移证据中保留，当前功能使用 Turso FTS、`vector32` 和 service BFS。

## 许可证

Kanban Tool 使用 [Apache License 2.0](LICENSE) 开源。

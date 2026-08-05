# Kanban Tool

Kanban Tool 是一个本地优先、单用户的看板与 durable work queue。任务、依赖、评论、执行记录和事件保存在本机的 canonical Turso 数据库中；CLI、MCP 和 Desktop 看到的是同一份事实。

当前产品刻意只有一条执行路径：

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP
    kanban serve
        ↓
ApplicationService + state machine
        ↓
  kanban-store-turso
        ↓
~/.local/share/kb/kanban.db
```

只有 `kanban serve` 可以打开、初始化和关闭数据库。其他入口永远通过本机 typed client 调用 host；server 不可用时返回 `server_unavailable`，不会退回直开数据库，也不会创建数据库文件。

## 快速开始

从源码安装 CLI：

```bash
cargo install --path crates/kanban-cli --bin kanban
```

先启动唯一的 application host：

```bash
kanban serve
```

默认数据库是 `$XDG_DATA_HOME/kb/kanban.db`（未设置时通常为 `~/.local/share/kb/kanban.db`），可以只在 host 上通过 `--db <path>`、`KANBAN_DB` 或兼容变量 `KB_DB` 覆盖。默认 HTTP 地址为 `http://127.0.0.1:8721`；客户端命令可以用 `--server-url` 或 `KANBAN_SERVER_URL` 指定地址。

项目 shell 配置不需要启动 host：

```bash
kanban init                    # 幂等创建/复用 .kb/config.toml，不创建数据库
kanban config show             # 查看 db、board、locale 及来源
kanban board use agent-work    # 只更新项目 active board 选择
kanban board current
kanban completions bash > ~/.local/share/bash-completion/completions/kanban
```

在另一个终端完成第一条 walking-skeleton 链路：

```bash
kanban board list
kanban task create "整理项目首页" --description "让第一次访问的人看懂项目"
kanban task list
kanban task step not-required default#1 --reason "单步任务"
kanban task promote default#1
kanban task show default#1
```

所有 mutation 都由 host 的 `ApplicationService` 校验并写入同一事务。任务状态由状态机决定，不能从 CLI、MCP 或 Desktop 直接修改数据库列。

## 三个入口

- **CLI**：domain 命令通过 `kanban-client` 使用 localhost HTTP；`config`、`init`、board selection、completion 和 Codex hook 是不触碰 canonical DB 的本地 shell。
- **MCP**：`kanban-mcp` 是最小 stdio server，只提供 tools；它不会拉起 `kanban serve`，所有 `tools/call` 都调用同一个 typed client。
- **Desktop**：Tauri 应用是外部 host 的薄前端。`RuntimeConfig` 只包含 `apiBaseUrl`、`actor` 和 `board`，不包含 `dbPath`，也不内嵌 SQLite 或 Axum server。可通过 `KANBAN_SERVER_URL` 指向 host。

## 当前能力

能力按纵向 operation 切片推进；每条切片都闭合 store、application、HTTP、client 和入口适配器。

| 阶段 | operation |
| --- | --- |
| Walking skeleton | `board.list`、`task.create`、`task.list`、`task.show`、`task.plan.not_required`、`task.promote` |
| Durable queue | `task.claim`、`task.heartbeat`、`task.release`、`task.review`、`task.done`、`task.block`；可选单 worker dispatcher |
| 协作信息 | `comment.create/list`、`step.create/list/update`、`dependency.create/list/remove`、`run.list/show/log`、`event.list` |

健康检查、board columns、stats 和 task selector 是 Desktop 支持所需的只读 query，也通过同一个 application path 提供。Run 不是独立 mutation surface：claim 创建 run，heartbeat/release/review/done/block 在同一事务中更新 run 和事件；入口只读 run。

## 状态与一致性

任务的 canonical status 为：

```text
triage → todo / scheduled → ready → running → review → done
                         ↘ blocked ↗
```

`board_columns` 只是展示映射，不能形成第二套状态机。关键边界包括：

- `ready → running` 通过原子 claim 完成；同一任务最多一个 active run。
- claim、heartbeat、release、review、done、block 复用同一状态机和 application transaction。
- 外键、board isolation、唯一约束、依赖环检查和 `task_events` 由 canonical store 保证。
- `task.create`、`comment.create`、`step.create` 支持实体范围内的 `idempotency_key`；dependency create 由复合唯一约束自然幂等。
- server 重启后 task、plan、run 和 event 仍从同一数据库恢复。

## Dispatcher

`kanban serve` 默认不消费队列。只有显式传入 `--dispatcher-profile <path>` 才会在同一 host 进程中运行最小单 worker loop。profile 固定声明 board、worker command、poll interval、claim TTL、heartbeat interval、success/failure policy 和 log directory。

dispatcher 只扫描 `ready`，并通过 `ApplicationService` 完成 claim、heartbeat 和 finish；不会自动 claim `review`、`todo` 或 `scheduled`。没有 daemon 注册、named pipe、Job Object 或自动 server supervision。

## 数据与配置

canonical 数据库：

```text
~/.local/share/kb/kanban.db
```

Host 启动时幂等执行 embedded schema migration，并 seed `default` board 与固定 status columns。当前 baseline 包含 boards、board_columns、tasks、task_execution_plans、task_steps、task_dependencies、task_runs、task_comments 和 task_events。

actor 是审计字符串，不是用户或权限模型。HTTP 只绑定 loopback，不提供公网、多用户、RBAC 或远程 worker。

## 明确非目标

本轮不提供，也不会以 fallback 形式重新引入：

- SQLite/Turso 双 backend、CLI/MCP/Desktop 直开数据库；
- `kanban-runtime*`、framed IPC、named pipe、capability negotiation 或通用 mutation receipt；
- 自动 server supervision、跨进程/跨机器数据库写入和 `multiprocess_wal`；
- SQLite importer、旧 API 完整兼容和 v2/v3 runtime 恢复协议；
- labels、signals、semantic search、graph、vector、Tantivy/LanceDB/Oxigraph projection 以及 derived control plane；
- 这些能力未来可以单独设计，但不属于当前 canonical path。

## 文档与开发

- 产品范围和阶段能力：[`docs/SPEC.md`](docs/SPEC.md)
- 进程、crate 和数据流：[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- 状态机：[`docs/STATE_MACHINE.md`](docs/STATE_MACHINE.md)
- HTTP contract：[`docs/API_SPEC.md`](docs/API_SPEC.md)
- CLI contract：[`docs/CLI_SPEC.md`](docs/CLI_SPEC.md)
- 数据模型：[`docs/DATA_MODEL.md`](docs/DATA_MODEL.md)
- 架构决策：[`docs/ADR.md`](docs/ADR.md)

修改前请阅读 [`AGENTS.md`](AGENTS.md)，并按受影响的 package 运行最小 `just` 检查。项目只做本地 commit，不自动 push 或创建 PR。

## 许可证

Kanban Tool 使用 [Apache License 2.0](LICENSE) 开源。

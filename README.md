# Kanban Tool

kanban-tool 是本地优先、单机单用户的看板与 durable work queue。它把任务状态、执行 claim、事件和
可重建的检索/图投影放在同一个本地 host 边界内，适合个人开发流程和本地自动化。

## 快速开始

启动唯一 host：

```bash
kanban serve
```

在另一个终端创建并查看任务：

```bash
kanban board list
kanban task create "准备发布说明"
kanban task list
```

脚本需要稳定结构时使用 `--json`；完整 flags 和 alias 以 `kanban --help` 及子命令 help 为准。

## 工作原理

```text
CLI / MCP / Desktop
        │ typed localhost HTTP/SSE
        ▼
kanban serve（唯一 host）
        │ 共享 application service
        ▼
kanban-core + kanban-service
        │ canonical Turso 数据库
        ▼
可重建的 FTS / vector / graph / context projection
```

所有 mutation 经过共享 service path；`tasks.status` 是状态事实，`ready -> running` 只能由原子 claim
完成。跨 crate 拓扑、依赖方向和数据 ownership 见 [`docs/architecture.md`](docs/architecture.md)。

## 主要能力

- 任务流程：显式创建、规格、claim、heartbeat、review、done、block、reopen 和 archive；见
  [`kanban-core` 状态机](crates/kanban-core/docs/state_machine.md)。
- 持久执行：host 维护 run、lease、event 和 dispatcher；见 [`kanban-server`](crates/kanban-server/README.md)。
- 持久化与恢复：Turso canonical facts、upgrade、import、backup 和 rebuild；见
  [`kanban-service`](crates/kanban-service/README.md)。
- 类型化集成：HTTP/SSE、CLI、MCP 和 Desktop 都从 protocol/client contract 工作；见各 owner README。

## 深入阅读

- [架构](docs/architecture.md)
- [CLI](crates/kanban-cli/README.md)
- [类型化客户端](crates/kanban-client/README.md)
- [HTTP 主机](crates/kanban-server/README.md)
- [MCP](crates/kanban-mcp/README.md)
- [Desktop](apps/desktop/README.md)
- [Schema 契约](crates/kanban-protocol/docs/schema.md)
- [架构决策](docs/adr/README.md)

## 范围

产品面向本机 loopback host 和单一用户，不提供 SaaS、多租户、远程访问、RBAC、云同步或第二个
canonical mutation path。CLI、MCP、Desktop 和 dispatcher 都不能绕过 `kanban serve` 直接写数据库。

## 许可证

 Apache-2.0，见 [`LICENSE`](LICENSE)。

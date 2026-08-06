# Kanban Tool

kanban-tool 是本地优先、单机单用户的看板与 durable work queue。它把任务状态、执行 claim、事件和
可重建的检索/图投影放在同一个本地 host 边界内，适合个人开发流程和本地自动化。

## Quick start

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

## How it works

```text
CLI / MCP / Desktop
        │ typed localhost HTTP/SSE
        ▼
kanban serve（唯一 host）
        │ shared application service
        ▼
kanban-core + kanban-service
        │ canonical Turso
        ▼
可重建的 FTS / vector / graph / context projection
```

所有 mutation 经过共享 service path；`tasks.status` 是状态事实，`ready -> running` 只能由原子 claim
完成。跨 crate 拓扑、依赖方向和数据 ownership 见 [`docs/architecture.md`](docs/architecture.md)。

## Main capabilities

- Task workflow：显式创建、规格、claim、heartbeat、review、done、block、reopen 和 archive；见
  [`kanban-core state machine`](crates/kanban-core/docs/state_machine.md)。
- Durable execution：host 维护 run、lease、event 和 dispatcher；见 [`kanban-server`](crates/kanban-server/README.md)。
- Persistence and recovery：Turso canonical facts、upgrade、import、backup 和 rebuild；见
  [`kanban-service`](crates/kanban-service/README.md)。
- Typed integrations：HTTP/SSE、CLI、MCP 和 Desktop 都从 protocol/client contract 工作；见各 owner README。

## Learn

- [Architecture](docs/architecture.md)
- [CLI](crates/kanban-cli/README.md)
- [Typed client](crates/kanban-client/README.md)
- [HTTP host](crates/kanban-server/README.md)
- [MCP](crates/kanban-mcp/README.md)
- [Desktop](apps/desktop/README.md)
- [Schema contracts](crates/kanban-protocol/docs/schema.md)
- [Architecture decisions](docs/adr/README.md)

## Scope

产品面向本机 loopback host 和单一用户，不提供 SaaS、多租户、远程访问、RBAC、云同步或第二个
canonical mutation path。CLI、MCP、Desktop 和 dispatcher 都不能绕过 `kanban serve` 直接写数据库。

## License

 Apache-2.0，见 [`LICENSE`](LICENSE)。

# Kanban Tool

Kanban Tool 是本地优先、单机、单用户的看板与 durable work queue。任务、执行记录、评论、附件、
labels、ontology、signals、entities、relations、检索、上下文和可重建投影都在同一个 canonical
Turso host 边界内协作。

产品只有一条运行路径：

```text
CLI / MCP / Desktop
        │ typed localhost HTTP/SSE
        ▼
kanban serve（唯一 host）
        │ KanbanService application path
        ▼
kanban-service（唯一 Turso owner）
        │
        ▼
canonical Turso database + 可重建 projection
```

只有 `kanban serve` 可以打开、初始化、迁移、备份、替换和关闭 Turso。CLI、MCP、Desktop 和
dispatcher 通过 typed localhost contract 访问 host；host 不可用时返回稳定错误，不会直开数据库、
创建第二个 backend 或切换到 embedded/SQLite fallback。

## 快速开始

```bash
kanban serve
```

另一个终端可以创建任务并查看队列：

```bash
kanban board list
kanban task create "准备发布说明"
kanban task list
kanban task step not-required default#1 --reason "单步任务"
kanban task promote default#1
```

需要稳定脚本输出时使用 `--json`；精确 flags、alias 和 leaf command 以 Clap help 与各自 owner
文档为准。

## 当前功能

所有 mutation/query 都经过共享 `kanban-service::KanbanService`、`kanban-core` 状态机和 service-owned
事务。当前功能面包括：

- boards、tasks、execution plans、steps、dependencies、comments、attachments、runs 和 events；
- board/task labels、label semantics、atoms、atom-index、suggestions、proposals 和 ontology ledger；
- 通用 signals、entities/relations、bounded graph BFS、Turso FTS search、`vector32`/Ollama vector
  provider 以及 bounded context pack；
- host-owned doctor、checkpoint、backup、portable import/export、vacuum、projection rebuild/cleanup
  和可选 `legacy-sqlite-import` v30 importer。

label proposal 同时支持 task scope 和 board scope；CLI `kanban label proposals list` 不带
`--task-ref` 时按当前 board 查询，详细行为见 [CLI 指南](crates/kanban-cli/README.md)。

FTS、vector、graph、context 和 projection state 都是可重建派生数据，不能反向写 canonical facts。

## 状态与一致性

`tasks.status` 是唯一状态事实，`board_columns` 只是展示映射。`ready -> running` 只能通过原子
claim，并与 active run、lease 和 event 同事务提交；heartbeat、release、review、done、block、
specify、unblock、reopen、reclaim 和 archive 都是显式 service command，不提供任意
`transition(target_status)`。dispatcher 只 claim `ready`，不会自动 claim `review`、`todo` 或
`scheduled`。

## 入口边界

- **CLI**：除 `serve`、配置/init、completion 和 hook 外，通过 `kanban-client` 请求 host；不直接开库。
- **MCP**：stdio tools 只调用 typed client，不启动 host，也不暴露 host-admin 数据库管理操作。
- **Desktop**：Tauri/React shell 只调用 loopback API，不直连 Turso、不复制状态机。

## 深入阅读

- [跨 crate 架构与 ownership](docs/architecture.md)
- [`kanban-core` 状态机](crates/kanban-core/docs/state_machine.md)
- [`kanban-service` 持久化](crates/kanban-service/docs/persistence.md)
- [`kanban-service` 迁移与导入](crates/kanban-service/docs/migration.md)
- [`kanban-service` 维护](crates/kanban-service/docs/maintenance.md)
- [`kanban-protocol` schema/wire 契约](crates/kanban-protocol/docs/schema.md)
- [CLI](crates/kanban-cli/README.md)、[HTTP host](crates/kanban-server/README.md)、[MCP](crates/kanban-mcp/README.md)、[Desktop](apps/desktop/README.md)

## 范围

产品面向本机 loopback 和单一用户，不提供 SaaS、多租户、RBAC、公网访问、云同步、第二 canonical
backend 或第二 mutation path。

## 许可证

Apache-2.0，见 [`LICENSE`](LICENSE)。

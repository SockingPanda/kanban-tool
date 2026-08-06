# 架构

kanban-tool 只有一条 canonical host 路径：

```text
CLI / MCP / Desktop
        │ typed localhost HTTP/SSE
        ▼
kanban-server（kanban serve）
        │ host lifecycle + HTTP/SSE
        ▼
kanban-service（KanbanService） ── kanban-core
        │
        ▼
Turso canonical 数据库 + 可重建 projection
```

## 所有权

- `kanban-core` 拥有领域 ID、状态机、readiness 和纯错误；不依赖内部 crate、HTTP 或数据库。
- `kanban-service` 拥有 `KanbanService` application path、事务、Turso schema/migration、repository、
  projection provider 和只读 importer；它是唯一直接拥有 Turso canonical persistence 的 crate。
- `kanban-server` 拥有 host 生命周期、Axum router、数据库装配和 dispatcher。
- `kanban-protocol` 拥有 DTO、error envelope、schema 和 endpoint/surface catalog；不拥有 row 或 handler。
- `kanban-client` 拥有 typed localhost transport；CLI、MCP、Desktop 依赖它而不直连数据库。
- `xtask` 只执行离线 artifact、依赖和文档检查，不是运行时依赖。

第三方依赖的精确 owner 和 feature 由 Cargo manifest 与 `$style` 维护。内部依赖方向必须保持单向：
domain 不向 adapter 反向依赖，adapter 不复制 service 状态机。

## 规范事实与派生数据

业务事实包括 board/task/lifecycle、execution plan、依赖、评论、附件 metadata、labels/ontology/
signals、entities/relations、runs 和 events。`tasks.status` 是唯一状态事实，event 是追加审计事实。
label proposal 也是 service-owned ontology fact：task-scoped proposal 通过
`/api/v1/tasks/:task_id/label-proposals` 创建/列出，board-wide proposal 通过
`GET /api/v1/boards/:board/label-proposals` 列出，可按 `status` 过滤；accept/reject 仍走共享
proposal decision path。

FTS、vector、graph/context、projection jobs、缓存和 capability probe 是可重建的派生或运行时状态；
它们可以删除后重建，不能反向写 canonical facts。详细事务和迁移边界归
[`kanban-service`](../crates/kanban-service/README.md)。

## Host 边界

只有 `kanban serve` 打开、初始化、迁移、备份、替换和关闭 Turso 并执行共享 application path。
client、CLI、MCP、Desktop 和 dispatcher 通过 typed localhost contract 工作；host 停止或输入无效时
返回稳定错误，不 fallback 到另一个数据库。

## 指南

- [`kanban-core` 状态机](../crates/kanban-core/docs/state_machine.md)
- [`kanban-service` 持久化](../crates/kanban-service/docs/persistence.md)
- [`kanban-protocol` schema 契约](../crates/kanban-protocol/docs/schema.md)
- [ADR 索引](adr/README.md)

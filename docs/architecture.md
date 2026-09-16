# 架构

kanban-tool 只有一条 canonical host 路径：

```text
CLI / MCP                 Web / Desktop WebView
        │ 原生 gRPC                │ binary gRPC-Web
        └─────────────┬────────────┘
        ▼
kanban-server（kanban serve） ── kanban-web-artifact
        │ typed application service
        ▼
kanban-service（KanbanService） ── kanban-core
        │
        ▼
Turso canonical 数据库 + 可重建 projection
```

## 所有权

- `kanban-core` 拥有领域 ID、状态机、readiness 和纯错误；不依赖内部 crate、HTTP 或数据库。
- `kanban-service` 拥有 `KanbanService` application path、Turso 的打开/初始化/迁移、连接、事务、
  repository、projection provider 和只读 importer；它是唯一直接拥有 Turso canonical persistence 的 crate。
- `kanban-server` 拥有 host 进程生命周期、数据库及附件/run-log 路径准备、同端口 HTTP/2 与
  gRPC-Web 装配、业务 RPC adapter、Axum router 和 dispatcher。
- `kanban-web-artifact` 拥有 Web dist 的 no-follow filesystem 校验与 immutable snapshot；它只依赖
  `kanban-protocol` 的 manifest value contract，server/xtask 通过它消费统一的 artifact 事实，HTTP
  content type、ETag 与 package copy 仍归各自 adapter。
- `kanban-protocol` 拥有 DTO、正式 Protobuf、typed error、schema 和 surface catalog；不拥有 row 或 handler。
- `kanban-client` 拥有共享原生 gRPC channel；CLI、MCP 的异步调用复用它，Desktop WebView 使用
  Web 的同源 gRPC-Web 数据源，所有入口都不直连数据库。
- `kanban-live-core` 拥有纯内存 Hub、快照、delta 与有界历史；刷新提示通过共享 service gate
  取得，不是事务成功、审计 event ID 或投影 revision。
- `xtask` 只执行离线 artifact、依赖和文档检查，不是运行时依赖。

第三方依赖的精确 owner 和 feature 由 Cargo manifest 与 `$style` 维护。内部依赖方向必须保持单向：
domain 不向 adapter 反向依赖，adapter 不复制 service 状态机。
Web artifact filesystem adapter 只能沿 `kanban-web-artifact -> kanban-protocol` 方向依赖，
`kanban-protocol` 不反向依赖 filesystem。

## 规范事实与派生数据

业务事实包括 board/task/lifecycle、execution plan、依赖、评论、对象目录与关系、文件 metadata、
labels/ontology/signals、entities/relations、runs 和 events。`tasks.status` 是唯一任务状态事实，
event 是追加审计事实。

模块、迭代和文件通过同一对象关系模型工作，`object_relation_edges` 持有关系事实。任务对象
复用原任务 ID，以 object/source 双版本保护通用修改，执行状态仍由任务 service 管理。文件对象
引用附件目录中的不可变 blob，所属对象通过关系连接；旧任务附件入口适配同一事实模型。
对象与文件的 mutation 共享 service 写锁、事务和通知边界，全部读取复用现有查询注册表。

FTS、vector、graph/context、projection jobs、缓存和 capability probe 是可重建的派生或运行时状态；
它们可以删除后重建，不能反向写 canonical facts。详细事务和迁移边界归
[`kanban-service`](../crates/kanban-service/README.md)。

## Host 边界

`kanban-server` 负责 host 进程生命周期、数据库及附件/run-log 路径准备、router、dispatcher 和
shutdown；启动时由 `kanban-service::KanbanService::open_with_roots` 打开并初始化 Turso，并在 service
内执行 migration、连接、事务、repository、projection 与维护操作。因而只有 `kanban serve` 进程会
触达这份 canonical 数据库；client、CLI、MCP、Desktop 和 dispatcher 通过 本机 gRPC contract
工作，host 停止或输入无效时返回稳定错误，不 fallback 到另一个数据库。

## 指南

- [`kanban-core` 状态机](../crates/kanban-core/docs/state_machine.md)
- [`kanban-service` 持久化](../crates/kanban-service/docs/persistence.md)
- [`kanban-protocol` schema 契约](../crates/kanban-protocol/docs/schema.md)
- [ADR 索引](adr/README.md)

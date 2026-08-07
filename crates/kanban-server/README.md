# kanban-server

`kanban-server` 是唯一 host。它准备数据库及附件/run-log 路径，装配 `kanban-service`、Axum routes、
host 进程生命周期和可选 dispatcher，并在 loopback HTTP/SSE 边界暴露 `kanban-protocol` contract。
Turso 的打开、初始化、迁移、连接和事务由 `kanban-service` 的 `KanbanService` 持有。

host 负责验证 transport 输入、配置 actor/idempotency、映射 service 错误和管理 shutdown。dispatcher
只能通过共享 service claim `ready`，不得直接写数据库，也不得 claim `review`。

CLI、MCP、Desktop 和 typed client 通过 localhost 使用 host；它们不应依赖 server 的 store internals。
精确 route 以 server router 与 protocol endpoint catalog 为准。Router 只调用 `KanbanService`，不形成
第二 mutation path。

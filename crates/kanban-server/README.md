# kanban-server

`kanban-server` 是唯一 host。它装配 `kanban-service`、Axum routes、数据库生命周期和可选 dispatcher，
并在 loopback HTTP/SSE 边界暴露 `kanban-protocol` contract。

host 负责验证 transport 输入、配置 actor/idempotency、映射 service 错误和管理 shutdown。dispatcher
只能通过共享 service claim `ready`，不得直接写数据库，也不得 claim `review`。

CLI、MCP、Desktop 和 typed client 通过 localhost 使用 host；它们不应依赖 server 的 store internals。
精确 route 以 server router 与 protocol endpoint catalog 为准。Ontology router 同时提供 task-scoped
`/api/v1/tasks/:task_id/label-proposals` 和 board-wide `GET /api/v1/boards/:board/label-proposals`
（可选 `status` query）；两者都调用 `KanbanService`，不形成第二 mutation path。

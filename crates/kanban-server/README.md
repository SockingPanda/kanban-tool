# kanban-server

`kanban-server` 是唯一 host。它准备数据库及附件/run-log 路径，装配 `kanban-service`、Axum routes、
host 进程生命周期和可选 dispatcher。一个 loopback listener 同时接受原生 HTTP/2 gRPC、
binary gRPC-Web 和现有 HTTP/SSE；业务 adapter 复用共享 application。
Turso 的打开、初始化、迁移、连接和事务由 `kanban-service` 的 `KanbanService` 持有。

host 负责验证 transport 输入、配置 actor/idempotency、映射 service 错误和管理 shutdown。dispatcher
只能通过共享 service claim `ready`，不得直接写数据库，也不得 claim `review`。

CLI、MCP、Desktop 和 typed client 通过 localhost 使用 host；它们不应依赖 server 的 store internals。
精确 route 以 server router 与 protocol endpoint catalog 为准。Router 只调用 `KanbanService`，不形成
第二 mutation path。

RPC 的 `WatchChanges` 使用与其他入口相同的 service mutation gate，通知表示需要重新读取。
它不代表事务结果、审计 event ID 或查询投影 revision。原生 RPC 从 HTTP/2 `:authority` 校验
目标；请求同时带 `Host` 时两者必须一致，Origin 仅接受当前 listener 和明确的 Desktop 开发来源。
`grpc-timeout` 约束请求和响应流；取消或退出会释放订阅名额。

Host 正常退出先通知所有持续响应并等待收尾。dispatcher 退出、HTTP 提前返回、强制退出和
Host future 被取消也会关闭 RPC；不通过 detached task 管理第二套生命周期。长期协议边界见
[本地 gRPC 决策](../../docs/adr/0007_local_grpc.md)。

Host 使用 `hyper-util` 驱动同一 listener 上的 HTTP/1 与 HTTP/2，并通过 `JoinSet` 持有连接任务。
正常退出最多等待 5 秒收尾，超时后中断并回收剩余连接；强制退出或取消 Host future 时直接中断。
即使客户端停止读取大响应，连接和响应资源也会随 Host 回收。

# 架构

目标调用方向：Atlas 页面 → application 查询/命令/实时接口 → gRPC-Web adapter → 同一个 kanban serve → KanbanService → Turso。CLI 与 MCP 内部走原生 gRPC，MCP 对外继续标准 stdio。静态 /app/ 资源仍使用 HTTP。

## 模块

kanban-live-core 不依赖数据库和 Protobuf。它保留 BoardSource、Hub、有限投影历史、TaskCommands，并新增 RefreshSource。RefreshSource 只提供 subscribe_refreshes 和 check_board，返回内部类型。

kanban-rpc-proto 从 proto/kanban/framework/v1/board.proto 生成契约。命名空间仍是实验框架 v1，不能冒充已冻结的完整产品契约。

kanban-rpc-host 同时提供 BoardService、TaskService、WorkspaceService。workspace_service 返回 Tower service，可供当前 Axum 0.7 的 host 装配，不要求把 tonic 内部 Router 与旧 Axum Router 直接 merge。此兼容点尚需 Rust 联编确认。

integration/kanban-adapter 中的 KanbanAdapter 接收已有 KanbanService。修改标题仍调用原 UpdateTaskCommand，并保留 expected_lock_version。check_realtime_board 只在静默 read fence 下验证 canonical board，无整板遍历。

Atlas 新增 application/realtime/source.ts 与 bind.ts。source 中没有 wire 类型；bind 将刷新通知转换为明确的 rpc-refresh-required 控制消息，沿用现有查询失效编排。它不构造 event_id、不推进审计 cursor、不伪造任务数据。

## 宿主所有权

新增 kanban-server 的 grpc-framework feature 与 grpc.rs 可选装配点。workspace_rpc_mount 返回 Router 和 RpcApp lifecycle handle。调用方负责合并到现有路由、把实际 listener 的同源 Origin 传入，并在原 graceful/force shutdown 中关闭 runtime。

该函数不打开数据库、不监听第二个端口。默认 serve 不会自动调用它，避免部分 API 装配被误当成整仓迁移。

## 两种订阅

WatchBoard 是完整的小型卡片集合，支持分块快照、增量和有界历史恢复。它适用于能接受该集合完整性的消费者。

WatchChanges 是 Atlas 迁移期的查询失效通道。它不发送卡片数据；每次连接先发送 attached，后续写入提示发送 write_hint。原分页、筛选、total 与详情保持自己的权威读取。G07 将这些读取升级为有 queryKey、datasetRevision 和明确分页边界的订阅。

## 安全与资源

loopback 和 single-user 边界保持。浏览器 RPC endpoint 必须与 Web artifact 同源；不同 localhost 别名或端口均视为不同源。Origin 在执行前校验，CORS 不承担授权。

同一个 RpcApp 的投影流和刷新流共享 16 个订阅名额。刷新消息限制为 4 KiB，初始请求限制 4 KiB，10 ms 合并提示，15 s 心跳，前端 35 s liveness watchdog。数值是实现预算，未作性能承诺。

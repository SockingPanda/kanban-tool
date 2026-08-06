# Single-host Turso ownership

## Status

Accepted

## Context

CLI、MCP、Desktop、dispatcher 和 server 都需要访问同一份任务事实。多个入口直接持有数据库或各自实现
mutation 会产生第二套状态机、并发和迁移语义。

## Decision

`kanban serve` 是唯一打开 Turso 的 host，也是 canonical persistence owner。其他入口通过 typed
localhost HTTP/SSE 调用共享 application service；service 负责事务、migration、repository 和 projection。

## Consequences

依赖方向和错误语义集中在 server/service path，入口 adapter 保持薄。离线工具可以读取 artifact，但不
获得 runtime 数据库写权限。


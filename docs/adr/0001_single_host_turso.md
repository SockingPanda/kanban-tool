# 单 Host 的 Turso 所有权

## 状态

Accepted

## 背景

CLI、MCP、Desktop、dispatcher 和 server 都需要访问同一份任务事实。多个入口直接持有数据库或各自实现
mutation 会产生第二套状态机、并发和迁移语义。

## 决策

`kanban serve` 是唯一允许打开 canonical Turso 的进程；`kanban-service` 是唯一直接依赖并持有
Turso persistence 的 crate。`kanban-server` 负责 host 生命周期与装配，其他入口通过 typed localhost
HTTP/SSE 调用共享 application service；service 负责事务、migration、repository 和 projection。

## 影响

依赖方向和错误语义集中在 server/service path，入口 adapter 保持薄。离线工具可以读取 artifact，但不
获得 runtime 数据库写权限。

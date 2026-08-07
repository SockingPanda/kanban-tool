# 维护

维护命令由 `kanban-service` 提供，并由 `kanban serve` 的 host-admin 边界调用。它们不能建立第二个
数据库 owner，也不能把派生状态提升为业务事实。

## 安全操作

- `doctor` 检查 schema、foreign key、派生状态和 capability，并报告可修复问题。
- `checkpoint`、`backup` 和 `export` 生成 host-owned 的可验证副本或导出物。
- `rebuild` 重建 FTS、vector、graph/context 等 derived projection。
- `vacuum` 和 cleanup 只在 service 允许的路径与事务边界内执行。

维护结果必须区分 completed、degraded、restart-required 和失败；失败不得伪装成已完成。精确参数、
HTTP operation 和 CLI flags 分别由 protocol catalog 与 Clap help 持有。

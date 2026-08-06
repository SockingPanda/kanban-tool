# kanban-service

`kanban-service` 是唯一直接拥有 Turso canonical persistence 的 crate。它把 `kanban-core` 的领域
规则、application DTO、事务、repository 和 projection provider 组合成共享 `KanbanService`；HTTP
host、CLI、MCP 和 Desktop 不能绕过这条路径写数据库。

## 所有权

- `kanban serve` 装配 service 并持有数据库连接。
- Turso schema、migration、repository、projection、search/vector provider 和只读 legacy importer
  在本 crate 内维护。
- `KanbanService` 是唯一 application mutation/query 入口；Turso row model 不跨出 persistence 边界。
- labels/ontology/proposals 属于同一 canonical service path。任务 proposal 使用 task scope，
  board-wide proposal 查询由 `GET /api/v1/boards/:board/label-proposals` 提供，可按 `status` 过滤；
  proposal accept/reject 与 ontology action ledger 由 service transaction 保护。

行为指南：

- [规范持久化](docs/persistence.md)
- [迁移与导入](docs/migration.md)
- [维护](docs/maintenance.md)

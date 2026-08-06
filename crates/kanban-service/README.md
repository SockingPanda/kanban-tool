# kanban-service

`kanban-service` 是唯一直接拥有 Turso canonical persistence 的 crate。它把 `kanban-core` 的领域
规则、DTO/port、事务和 repository 组合成共享 application service；HTTP host、CLI、MCP 和 Desktop
不能绕过这条路径写数据库。

## Ownership

- `kanban serve` 装配 service 并持有数据库连接。
- Turso schema、migration、repository、projection、search/vector provider 和只读 legacy importer
  在本 crate 内维护。
- adapter 只转换 transport 输入输出；row model 不跨出 persistence 边界。

行为指南：

- [canonical persistence](docs/persistence.md)
- [migration and import](docs/migration.md)
- [maintenance](docs/maintenance.md)


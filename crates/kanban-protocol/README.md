# kanban-protocol

`kanban-protocol` 是跨 adapter 的机器可读 wire contract owner。它提供 DTO、error envelope、event
payload、HTTP/SSE endpoint descriptor、CLI/MCP surface catalog 和 schema registry。

它不拥有数据库 row、service input、HTTP handler 或 CLI command implementation。server、client、CLI、
MCP 和 Desktop 必须以这里的 typed contract 互操作；业务状态、事务和权限仍由 `kanban-core` 与
`kanban-service` 负责。

精确 schema artifact、fixture 和生成约束见 [`docs/schema.md`](docs/schema.md)。精确 method/path、
Clap flags 和 MCP tool 名称由代码 catalog、`kanban --help` 与对应 adapter 持有，不在本 README 维护
穷举清单。


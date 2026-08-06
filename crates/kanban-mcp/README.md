# kanban-mcp

`kanban-mcp` 通过 stdio 提供 MCP adapter。它使用 `kanban-client` 访问唯一 localhost host，并以
`kanban-protocol` 的 MCP catalog 注册 typed tools；不打开数据库，不复制 service 状态机。

启动前确认 `kanban serve` 已运行。host-admin 操作仍由 host/CLI 边界管理，MCP 不因 catalog 扩展而取得
第二条 mutation path。精确 tool 名称和输入 schema 以 MCP catalog 与 protocol schema 为准。


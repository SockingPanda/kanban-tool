# kanban-client

`kanban-client` 是面向 loopback `kanban serve` 的 typed HTTP/SSE client。它负责 URL、actor、selector、
请求 envelope、响应 DTO 和稳定错误映射；不打开 Turso、不执行本地 fallback，也不复制状态机。

默认 host 是 `http://127.0.0.1:8721`。调用者应先启动 `kanban serve`，再通过 client 的 typed operation
访问 board、task、lifecycle、search、graph、maintenance 等 surface。精确 path 和 DTO 由
`kanban-protocol` catalog 持有。


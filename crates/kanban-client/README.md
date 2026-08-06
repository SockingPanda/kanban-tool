# kanban-client

`kanban-client` 是面向 loopback `kanban serve` 的 typed HTTP/SSE client。它负责 URL、actor、selector、
请求 envelope、响应 DTO 和稳定错误映射；不打开 Turso、不执行本地 fallback，也不复制状态机。

默认 host 是 `http://127.0.0.1:8721`。调用者应先启动 `kanban serve`，再通过 client 的 typed operation
访问 board、task、lifecycle、search、graph、maintenance 等 surface。精确 path 和 DTO 由
`kanban-protocol` catalog 持有。

label proposal 查询区分 task scope 与 board scope：`list_task_label_proposals` 请求
`/api/v1/tasks/:task_id/label-proposals`，`list_board_label_proposals` 请求
`GET /api/v1/boards/:board/label-proposals`，后者可传 `status` 过滤。client 只负责 typed transport
和错误映射，不复制 proposal 或 board isolation 规则。

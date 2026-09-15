# kanban-live-core

可重建的实时投影与有界历史。`BoardSource` 提供 application 一致读取，`Hub` 保存快照、增量及恢复 cursor；命令通过 `TaskCommands` 返回 application service。

本 crate 不依赖 Protobuf、HTTP 或数据库。每个 Hub 由一个 source pump 更新；通知计数、审计事件和投影 cursor 各有独立语义。当前卡片投影属于完整看板集合，分页与详情必须使用各自的完整查询契约。

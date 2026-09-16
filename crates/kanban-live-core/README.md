# kanban-live-core

可重建的实时投影与有界历史。本 crate 不依赖 Protobuf、HTTP 或数据库；Host 负责业务查询
与一致读取，通知计数、审计事件和投影 cursor 各有独立语义。

`query::QueryHub` 保存完整 typed 查询的不可变编码字节；业务 request/response 的解释由 Host 持有。
相邻样本使用公共前缀、删除长度和插入字节形成精确 splice，窗口总量、排序和深层字段都保留在原始结果中。
epoch、scope 与 base cursor 必须精确匹配；历史缺失时恢复完整快照。

历史同时受条数和字节预算限制。`ByteBudget` 由整个 Host 共享，并追踪发送中的 `Arc<QueryBytes>`，
因此历史淘汰后，慢消费者继续持有的字节仍计入总预算。取消发送或释放最后一个引用时归还预算。

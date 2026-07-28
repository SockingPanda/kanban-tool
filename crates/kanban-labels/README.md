# kanban-labels

`kanban-labels` 是纯内存的语义标签推荐与解析组件。它接收外部已经生成的标签原子向量（label atom embedding），对任务或查询向量做候选聚合、多标签选择和可解释证据输出。

本 crate 不连接 SQLite，不执行迁移，不提供 CLI / API 行为，也不替代数据库中作为事实来源的 `labels` / `task_labels` 存储。上层仍负责标签定义持久化、向量生成、权限边界和最终写入。

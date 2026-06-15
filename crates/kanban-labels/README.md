# kanban-labels

`kanban-labels` 是纯内存的语义 label 推荐与解析组件。它接收外部已经生成的 label atom embedding，对任务或查询 embedding 做候选聚合、多 label 选择和可解释证据输出。

本 crate 不连接 SQLite，不执行 migration，不提供 CLI/API 行为，也不替代数据库中的 canonical `labels` / `task_labels` 存储。上层仍负责 label 定义的持久化、embedding 生成、权限边界和最终写入。

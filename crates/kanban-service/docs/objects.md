# 对象与关系

`KanbanService::object_catalog` 读取类型和属性目录，`object_list` / `object_get` 读取对象，
`object_execute` 执行带 `board_id`、`actor` 和 `request_id` 的命令。模块、周期和普通对象复用
同一关系引擎。任务对象使用原任务 ID；任务状态、claim、lease、run 和事件仍由任务服务维护。

关系的唯一事实是 `object_relation_edges`。正向和反向属性读取同一条边；类型、board、基数、
唯一约束和端点版本在同一事务内校验。关系换属必须同时声明删除的旧边、增加的新边和全部
端点的预期版本。目录变更也必须提供预期 catalog version。

对象版本由 object version 和可选 task source version 组成。任务执行状态变化会使旧的双版本
前提失效，通用编辑不能绕过任务状态机。相同 `request_id` 和完整输入可重放已提交回执；更改
actor 或命令内容会返回冲突。提交结果未知时保留原请求以核对回执，不生成新请求标识重试。

对象 mutation 与其他 service mutation 共用写锁和通知机制。当前 Turso 不支持递归 CTE，
无环关系由持有写锁的事务内遍历检查；遍历超过对象或边数预算时拒绝操作。导入、doctor 和
启动校验复用对象不变量检查。周期关闭会冻结成员快照并解除实时成员关系，不改变任务状态。

长期取舍见 [对象关系事实](../../../docs/adr/0008_object_relations.md)。

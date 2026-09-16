# 0008 对象关系作为唯一关系事实

状态：已采纳。

模块、周期和自定义对象需要共用双向引用、基数、版本与历史语义。维护多份正反关系数组会使
换属和导入产生不一致，因此所有通用引用写入 `object_relation_edges`，正反属性只读取该事实。

任务保留现有 canonical task row 和执行状态机，并以原任务 ID 获得通用对象身份。通用命令
使用 object / source 双版本和 catalog version，不能直接改变 claim、lease 或 run。全部写入
经过同一 service mutation gate、事务和事件路径，adapter 不持有第二套数据库写入能力。

外键、唯一约束和不可变边保护局部约束；传递环和生命周期规则由事务内的 service 算法检查，
超出遍历预算时拒绝写入。旧格式导入统一转换后必须重新检查这些规则。

详细调用边界见 [service 对象指南](../../crates/kanban-service/docs/objects.md)。

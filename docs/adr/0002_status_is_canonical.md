# 状态是规范事实

## 状态

Accepted

## 背景

看板列是用户界面映射，而任务 lifecycle 还要受排期、依赖、execution plan、claim 和 lease 约束。让列
或 adapter 保存另一份状态会使读取与 mutation 分叉。

## 决策

`tasks.status` 是唯一状态事实。`board_columns` 只描述展示顺序和列属性；任何状态变化都经由
`kanban-core` guard 与 `kanban-service` 的显式 lifecycle command。

## 影响

readiness 可以从 canonical facts 重算，projection 只服务查询。CLI、MCP、Desktop 和 dispatcher 不能
直接设置任意目标状态。

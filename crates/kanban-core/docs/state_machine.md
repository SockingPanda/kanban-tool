# 任务状态机

本指南描述 `kanban-core` 拥有的状态语义和纯状态规则。HTTP、CLI、MCP、Desktop 和 dispatcher
只调用 `kanban-service::KanbanService` 的显式 lifecycle command；精确入口由各自 owner 提供。

## 状态

`triage` 表示规格不完整，`todo` 表示尚未满足执行条件，`scheduled` 等待排期，`ready` 表示可以
被 claim，`running` 持有 active claim，`blocked` 记录阻塞原因，`review` 等待结果审查，`done` 表示
完成，`archived` 表示只读历史。`tasks.status` 是唯一事实；看板列只负责展示映射。

## Readiness（可执行性）

readiness 根据标题和描述、排期、父依赖以及 execution plan 重新计算。规格或依赖发生变化时，service
不能凭调用者指定的目标状态强行写入 `ready`；必须重新计算并保持 board isolation。

## 生命周期转换

- 创建只接受 `triage`、`todo`、`scheduled` 或 `ready` 的请求，必要条件不满足时回落到重算结果。
- `todo|scheduled -> ready` 由显式 promote 守卫完成。
- `ready -> running` 只能通过原子 claim。
- `running -> ready`、`running -> review` 和 `running|review -> done` 需要相应的 claim/审查条件。
- `triage|todo|scheduled|ready|running|review -> blocked` 需要非空原因。
- `blocked`、`done` 和 `review` 的恢复路径都保留历史并重新计算可执行状态。

具体 transition 的持久化、event、projection enqueue 和错误映射属于 `kanban-service`；不要在
adapter 中复制状态机。labels、ontology、signals、attachments 和 import 也不能绕过这条 service
path 直接改写 `tasks.status` 或其他 canonical facts。

## Claim 与 lease

claim 使用 `ready` 条件、空 claim 和版本检查完成 CAS，并与 active run、lease 和 event 在同一事务中
提交。heartbeat、release、review、done、block 和 reclaim 都校验 owner/token、expiry 与版本；失败
不得留下孤立 run、event 或 projection job。

## Dispatcher（调度器）

dispatcher 只 claim `ready`，复用同一 service lifecycle path，并在 worker 运行期间维护 heartbeat。它
不得 claim `review`，也不得直接写 canonical task、run 或 event。

## 不变量

- 所有 mutation 经共享 application/service path。
- 同一任务最多一个 active run；claim、run 和 event 保持一致。
- board isolation、依赖环、required step 和 idempotency 由领域与 service 边界共同保护。
- projection、缓存和派生索引只能从 canonical facts 重建，不能反向成为状态事实。

## 相关指南

- [`kanban-service` 持久化](../../kanban-service/docs/persistence.md)
- [`kanban-service` 迁移](../../kanban-service/docs/migration.md)
- [`kanban-protocol` schema 契约](../../kanban-protocol/docs/schema.md)

# 规范持久化

本页是 `kanban-service` 的持久化边界指南。精确表、列、索引、trigger 和 schema fingerprint 由
`src/schema.rs`、`src/migration.rs` 及生成 artifact 持有；本页不复制完整 inventory。

## 规范所有权

`kanban serve` 是唯一接触 canonical Turso 的进程。`kanban-server` 负责数据库及附件/run-log 路径准备
与 host 生命周期；`KanbanService::open_with_roots` 在本 crate 内打开、初始化和迁移 Turso，连接、事务、
repository 与 projection 也由本 crate 持有。所有其他入口通过 localhost typed HTTP 工作；不存在第二条
canonical mutation path，也不存在并行 SQLite runtime backend。

业务事实包括 board/task/lifecycle、执行计划和步骤、依赖、评论、附件 metadata、labels/ontology/
signals、entities/relations、runs 与 append-only events。`tasks.status` 是状态事实，event 是审计事实。
`label_semantic_proposals` 也是 service-owned ontology fact；它同时支持 task scope 与 board scope，
board-wide 列表由 host 的 typed contract 暴露，精确 endpoint 由 `kanban-protocol` catalog 持有。

## 派生数据

FTS、vector、graph/context、projection jobs、缓存和 capability probe 都是可重建的派生或运行时状态。
它们可以在不改变业务事实的前提下删除并重建；projection 失败只能让对应查询降级，不能回写 task、
label、signal 或 entity 的 canonical 状态。

## 事务边界

mutation 使用 service-owned immediate transaction。状态快照、run、event、idempotency、依赖环、board
foreign key 和 projection enqueue 必须整体提交或整体回滚。claim/lease 使用 token、owner、expiry 和
版本检查保护并发调用。

ID、时间和 JSON 的具体编码由 Rust DTO、schema 与数据库约束共同校验，文档只解释语义，不维护字段数量。

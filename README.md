# Kanban Tool 文档包

本文档包面向一个 **Rust 核心实现、SQLite-only、本地单机运行、同时提供 Web 与 CLI 能力** 的 Kanban 工具。

本项目不是 Trello 的简单复制品，而是一个本地优先的可执行工作队列：

- Kanban UI 负责可视化与人工操作。
- CLI 负责脚本化、本地开发流与 agent/automation 入口。
- SQLite 负责持久化任务、状态、依赖、评论、事件、运行记录。
- Rust core 负责状态机与一致性约束。
- Dispatcher 是可选本地调度器，用于自动提升、claim、heartbeat、reclaim 和执行 worker profile。

## 范围约束

明确包含：

- 单机本地运行。
- SQLite 作为唯一数据库。
- Web 端与 CLI。
- 多 board/project，但不是多租户。
- 单用户语义；actor 只是审计字段，不是权限主体。
- 本地 dispatcher/worker 能力。
- append-only events + tasks snapshot。

明确不包含：

- 多用户协作。
- 多租户。
- 远程 worker。
- PostgreSQL/MySQL/MongoDB 后端。
- RBAC、组织、团队、邀请、审计权限模型。
- 云同步或网络文件系统共享 SQLite。

## 文档索引

| 文件 | 内容 |
|---|---|
| [`docs/SPEC.md`](docs/SPEC.md) | 产品与技术总 SPEC |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Rust crate、进程、数据流与配置架构 |
| [`docs/STATE_MACHINE.md`](docs/STATE_MACHINE.md) | 状态定义、转换表、不变量 |
| [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md) | 领域对象、ID、时间、事件、附件、查询模型 |
| [`docs/CLI_SPEC.md`](docs/CLI_SPEC.md) | CLI 命令、参数、输出、退出码 |
| [`docs/API_SPEC.md`](docs/API_SPEC.md) | 本地 Web API 与 SSE 事件流 |
| [`docs/DISPATCHER_SPEC.md`](docs/DISPATCHER_SPEC.md) | 本地 dispatcher / worker 调度规格 |
| [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) | 分阶段实现计划、测试策略、验收标准 |
| [`docs/ADR.md`](docs/ADR.md) | 关键架构决策记录 |
| [`docs/V0.5.md`](docs/V0.5.md) | V0.5 已实现范围、验证记录与暂未包含项 |
| [`migrations/001_initial.sql`](migrations/001_initial.sql) | SQLite 初始 schema |

## 推荐仓库结构

```text
kanban-tool/
  Cargo.toml
  crates/
    kanban-core/
    kanban-sqlite/
    kanban-cli/
    kanban-server/
    kanban-dispatcher/
  web/
  docs/
  migrations/
```

## 默认二进制名

本文档中使用 `kb` 作为 CLI binary 名称。项目正式命名后可统一替换。

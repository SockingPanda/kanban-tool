# 项目文档规范

本文规定 kanban-tool 的文档事实源、owner placement 和同步边界。它描述文档如何协作，不重复产品行为、wire schema 或运行时实现。

## 文档分层

| 文档或事实源 | 持有内容 | 更新触发 | 不放置的内容 |
| --- | --- | --- | --- |
| [`AGENTS.md`](../AGENTS.md) | 跨任务稳定契约、仓库地图、文档路由和验证边界 | 稳定不变量、owner 或入口发生长期变化 | 高频实现库存、测试进度和临时 workaround |
| [`CONTEXT.md`](../CONTEXT.md) | 跨文档共享的领域术语和概念边界 | 术语在 domain modeling 中确认或变更 | 命令、路径、实现细节、导航索引和历史 |
| `README.md` | 产品首页、最小使用路径和指南索引 | 用户可观察的入口或最小路径变化 | 精确 machine inventory 和维护过程 |
| [`docs/architecture.md`](architecture.md) | 跨 crate 拓扑、ownership、canonical/derived 原则 | 架构边界或数据权威发生变化 | 精确 endpoint、字段、命令和迁移 ledger |
| crate/app `README.md` 与 `docs/` | 拥有行为的领域指南和使用细节 | 对应 crate/app 的行为、边界或使用方式变化 | 根契约和其他 owner 的事实副本 |
| [`docs/adr/`](adr/README.md) | 难以逆转、出乎预期且有真实取舍的长期决定 | 形成或取代一项架构决定 | 进度、测试名称、baseline 和一次性 workaround |
| 代码、Clap help、router/catalog、schema 与生成 artifact | 精确 machine contract 和可执行库存 | 机器接口或生成源变化 | 用 Markdown 手工复制同一库存 |
| 任务、CI、Git history/tag 与 release asset | 进度、验证证据和历史追溯 | 一次任务、gate 或发布证据变化 | 当前 active 指南的事实源 |

## 更新清单

1. 先找到拥有该行为的代码、schema、生成 artifact 或 crate/app 文档；它是当前事实源。
2. 行为变化先更新 owner source，再更新根入口和必要的导航链接；不要在两个 active 文档复制同一事实。
3. 术语变化先经过 `$domain-modeling` 的边界澄清；术语确认后立即更新根 `CONTEXT.md`。
4. 需要写实现说明时，把内容放在对应 owner 文档；不要把 `CONTEXT.md` 变成 spec 或 scratch pad。
5. 只有难以逆转、出乎预期且存在真实取舍的决定才新增 ADR，并同步 [`docs/adr/README.md`](adr/README.md)。
6. 完成的 ledger、旧 recovery runbook、聚合快照和一次性 workaround 移出 active 文档树；历史沿 Git/tag、release asset 或 task record 追溯。
7. 文档改动完成后运行 `just docs-check` 和 `just diff-check`；若同时修改 Rust owner，再运行受影响的 Rust gate。

## Agent 阅读路径

Agent 先读 `AGENTS.md` 获得入口和稳定契约；需要统一产品用词时读 `CONTEXT.md`；需要行为、状态、persistence 或 wire 细节时回到对应 owner 文档。Context 是词汇表，不是所有事实的汇总页。

## Context 与 ADR 边界

根 `CONTEXT.md` 是当前唯一的项目级领域语言上下文。只有未来出现彼此独立且语言确实分裂的 bounded context，才引入 `CONTEXT-MAP.md` 和上下文级 `CONTEXT.md`。

ADR 一项决定一篇，只记录长期取舍和原因；实现进度、验证结果和迁移状态留在任务、CI 或 Git history。文档事实源分层的长期取舍见 [`0005`](adr/0005_documentation_sources_of_truth.md)。

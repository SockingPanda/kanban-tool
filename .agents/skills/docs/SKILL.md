---
name: docs
description: 维护 kanban-tool 的文档事实源、owner placement 和同步边界；当产品行为、crate/app ownership、状态机、persistence、wire/schema、CLI、Desktop layout 或安装环境发生变化，或需要判断应读和应改哪份文档时使用。不负责 Rust 实现、长文措辞、底层检查命令或提交。
---

# 文档 owner contract

## 行为契约

能力：根据真实代码、schema、测试和当前入口，把长期说明放到拥有该行为的 crate/app，并保持根入口
可导航、事实不重复、机器库存不漂移。

触发：产品行为、公开入口、状态/事务、persistence/migration、wire/schema、CLI、Desktop、crate
边界或长期架构取舍变化；用户询问文档落点或同步面。

不触发：只改中文句子用 `$prose`；只改 Rust/Cargo 用 `$style`；只选择验证用 `$check`；只创建提交用 `$commit`。

## Placement

- 根 `README.md` 是产品首页、最小使用路径和指南索引。
- 根 `AGENTS.md` 是仓库地图、稳定不变量和文档路由。
- 跨 crate 拓扑只写 `docs/architecture.md`；领域指南放到拥有行为的 crate/app `README.md` 或 `docs/`。
- Rust crate/module guide 使用准确的 `#[doc = include_str!(...)]` 纳入 rustdoc；移动源码时同步检查目标路径。
- 长期跨模块取舍一项一 ADR，放在 `docs/adr/`；迁移进度、gate、测试名称和 baseline 留在任务、CI 或 Git history。

## Ownership

`kanban-core` → state machine；`kanban-service` → persistence/migration/maintenance；`kanban-protocol` →
wire/schema/catalog；`kanban-client` → typed transport；`kanban-server` → host/dispatcher；`kanban-cli`、
`kanban-mcp`、`apps/desktop` → 各自 adapter/shell；`.config/codex` → 环境说明。

精确 CLI 由 Clap help、精确 HTTP/MCP 由 router/catalog、精确 schema 由 migration/generated artifact
持有。不要手工复制 operation、表、字段、命令、测试或 gate inventory。

## History and evidence

当前指南只描述当前行为；已完成 ledger、旧 recovery runbook 和聚合快照离开 active tree。修改前先核对
owner source，冲突时报告 current implementation 与既有承诺，不静默创造第二份规范。

## 验证案例

- canonical：新增 wire 字段时更新 protocol owner 与生成 artifact，README 只保留使用语义。
- near-miss：私有函数重命名且公开事实不变时不新增长期文档。
- failure：不得只改派生文件；应回到 owner source 并验证本地链接和 `include_str!`。

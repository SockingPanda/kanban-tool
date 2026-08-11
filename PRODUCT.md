# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- 主要用户是 AI agents：它们通过 CLI、MCP 和 typed localhost surface 驱动 durable work queue，在本地持久化、领取、执行、更新和观察工作。
- 次要用户是人类：它们通过 Web/Desktop UI 更直观地查看全局状态、数据与信息，并在必要时进行操作或介入。
- 使用边界是 local-first、单机、单用户；产品不以远程协作团队为默认用户模型。

## Product Purpose

`kanban-tool` 为 agent-driven 工作提供一个本地、可持久化、可观察的 durable work queue。它把任务状态与执行事件保存在 canonical 数据源中，让 agents 能够可靠地推进工作，让人类能够在需要时理解全局并介入。

产品成功意味着：

- agents 可以通过 CLI/MCP 和 typed localhost surface 持久、可恢复地驱动队列；
- claim、run、状态更新及其事件保持一致，避免并发执行产生含糊事实；
- 人类可以通过 Web/Desktop 观察全局状态、数据和信息，并沿共享语义执行必要操作；
- canonical 数据留在本机，projection 和 cache 可以从事实重建。

## Positioning

产品的核心机制是单机 canonical service：`kanban serve` 是唯一允许触达 canonical Turso 的进程，CLI、MCP、Desktop 和 Web 通过共享的 typed service 语义使用同一事实与状态机。这使 agent-first 的 durable queue、可审计的状态推进和人类观察/介入共用一条 canonical mutation path，而不是分别维护多套状态或依赖 SaaS workspace。

## Operating Context

- agents 主要从 CLI 或 MCP 发起工作；入口通过 typed localhost surface 与本地服务交互。
- `kanban serve` 负责 canonical Turso 的服务边界；其他入口不得绕过共享 application/service path 直接写 canonical 状态。
- Web/Desktop 是人类的观察与操作界面，复用 CLI/MCP 使用的状态、事务和错误语义。
- UI 中的 `project` 只映射 canonical board；产品需要支持在多个 board/project 之间切换，但不引入 workspace/team 作为新的领域实体。
- 评估和维护发生在本地单机环境；canonical 数据是业务事实，projection、cache 和派生索引只是可重建的派生物。

## Capabilities and Constraints

- `tasks.status` 是事实；看板 column 只是展示映射，不得形成第二套状态机。
- `ready -> running` 只能通过原子 claim；claim、run 与对应 event 必须保持一致。
- dispatcher 只 claim `ready`，不得自动 claim `review`。
- 必须保留 board isolation、外键、唯一约束、idempotency、依赖环检查和事务原子性。
- 所有 mutation 必须经过共享 application/service path；不得建立第二条 canonical mutation path。
- projection、cache 和派生索引必须可以从 canonical 事实重建，不能反向写事实。
- 产品是 local-first、单机、单用户；不引入 SaaS、多租户、RBAC、云同步、远程 workspace 或团队协作语义。
- `project` 是 board 的 UI 语义映射；不得借此发明 workspace/team 或改变 canonical 领域模型。
- Web surface 受 strict CSP 与现有 Astryx runtime 约束；未来交互不能以放宽 CSP 或绕过 typed service 为代价。

## Brand Commitments

- 产品名固定为 `kanban-tool`；本文件不引入其他品牌名或别名。
- Plane 前端仅作为交互与信息架构参考，不是 `kanban-tool` 的功能事实、产品定位或品牌承诺。

## Evidence on Hand

- 产品与边界说明：`README.md`、`CONTEXT.md`、`docs/architecture.md`。
- 状态机与 canonical 约束：`crates/kanban-core/docs/state_machine.md`，以及 `crates/kanban-core`、`crates/kanban-service`、`crates/kanban-server`、`crates/kanban-protocol`、`crates/kanban-client` 的实现与测试。
- Web 入口与现有界面：`apps/web/README.md`、`apps/web/src/App.tsx`、`apps/web/src/ProductShell.tsx` 及 `apps/web/src/features/`。
- 可复核的现有视觉基线：`apps/web/tests/a11y-visual.spec.ts-snapshots/` 下的 PNG snapshots；它们记录 incumbent UI，不代表未来方向已经实现。
- 已批准的 Plane-inspired 视觉参考：`.impeccable/mocks/plane-adaptation/01-board-overview-v2.png`、`02-task-side-peek.png`、`03-list-command-palette.png`、`04-project-switcher-open.png`。这些 mock 只表达交互/信息架构方向，不证明对应 UI、project switcher 或 command palette 已在产品中实现。
- 当前没有可用于未来叙事的客户、用户、营收、性能或第三方证明材料；后续工作不得捏造这些内容。

## Product Principles

1. Canonical facts first：所有 mutation 共享同一 application/service path，状态事实只有一份。
2. Agent-first、human-observable：优先保证 agents 能可靠推进 durable queue，同时让人类能理解并在必要时介入。
3. Local-first、single-user：本地可恢复性与单机边界优先于 SaaS、多租户或远程协作扩展。
4. Rebuildable projections：projection、cache 和派生索引服务于观察与查询，但永远不能成为业务事实来源。
5. Board semantics stay honest：UI 的 `project` 只映射 canonical board，不借视觉隐喻扩展出 workspace/team 领域。

## Accessibility & Inclusion

Web/Desktop 界面应保持键盘可达、可见焦点、语义化交互和 reduced-motion 兼容；现有 Web 测试已覆盖 keyboard/a11y、focus return、strict-CSP shell 与 reduced-motion 证据。具体 WCAG 等级尚未作为独立产品承诺确认，后续实现不得降低现有可访问性行为。

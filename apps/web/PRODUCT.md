# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- 主要执行者是 AI agents。它们通过 CLI、MCP 和 typed localhost service 领取、推进、审阅与完成 durable work。
- 人类是观察者与必要时的操作者。人类打开 Web/Desktop UI，主要为了快速理解多个项目当前的任务状态、执行进度、运行记录、事件、signals 与 ontology，并在需要时介入。
- 产品是单机、单用户工具，不以团队协作、工作区治理或 SaaS 管理员为目标。

## Product Purpose

kanban-tool 是 local-first 的看板与 durable work queue。它让 AI agents 的工作承诺、状态转换、执行证据和恢复边界成为 canonical facts，同时给人类一个高密度、可扫读、可追踪的控制面。

成功意味着：

- agent 能沿唯一 typed application/service path 安全推进任务；
- 人类能在多个项目之间快速切换，并在很短时间内判断“正在发生什么、哪里需要注意、下一步是什么”；
- Web、Desktop、CLI 与 MCP 对同一状态机和业务事实保持一致。

## Positioning

kanban-tool 不是通用协作 SaaS。它的差异化机制是：以本机 canonical durable queue 为事实源，让 agent 执行路径和人类观察界面共享同一套 task state、claim、run、event 与 evidence，而不是在 UI 中维护第二套项目管理事实。

## Operating Context

- 一台本机上存在多个 project/board；人类需要在它们之间切换。
- agent 通过任务依赖、required steps、claim、heartbeat、run、review 与 completion 形成可恢复的工作链。
- 人类主要使用 Projects、Project Overview 与 Tasks；Tasks 需要在 board、list、table、map/timeline 等不同观察方式之间切换。
- 深层运行与诊断信息仍通过 Runs、Events、Signals、Ontology、Health 和 Maintenance 等 owner surface 提供。
- Storybook 是组件开发、状态演示和人机评审工具，不是正式产品 UI 或 canonical mutation 入口。

## Capabilities and Constraints

- `tasks.status` 是事实；看板列和其他视图只是展示映射。
- 所有 mutation 必须经过共享 typed application/service path；Web、Desktop 或 Storybook 不得直接写 canonical persistence。
- `kanban serve` 是唯一允许触达 canonical Turso 的进程；前端只通过同源 localhost HTTP/SSE 工作。
- project 在现有协议中对应 canonical board。当前 board list 只保证 `id`、`slug`、`name`、`description` 与 archive 信息；没有正式 contract 的 project lifecycle、progress、risk、owner、cover、summary 或 task counts 不得伪造，也不得通过 N+1 查询拼装。
- 保留现有 canonical deep links 与 task state semantics。重构可以替换信息架构、视觉和组件边界，但不能建立第二状态机或第二 mutation path。
- 产品保持 strict CSP、可离线/可恢复边界、项目隔离、幂等性和 transaction invariants。
- 不引入 workspace/team、RBAC、云同步、多租户、远程 SaaS 或 Plane 的协作业务模型。

## Brand Commitments

- 产品名称固定为 `kanban-tool`。
- Plane 是本次 Web/Desktop 控制面的唯一绑定产品参考：学习其双层导航、信息密度、视图体系、控件复用和完成度。
- 不复制 Plane 的名称、logo、内容、插图、业务术语或 AGPL 源码；所有界面必须表达 kanban-tool 自己的 canonical facts。
- 产品文案直接、克制、面向操作；技术 ID、ref、状态和错误必须保持精确，不用营销话术掩盖事实。

## Evidence on Hand

- canonical 产品边界与 crate ownership：仓库根 `AGENTS.md`、`docs/architecture.md`。
- task 状态机：`crates/kanban-core/docs/state_machine.md`。
- Web 当前 capability 与非目标：`apps/web/docs/capability-ledger.md`。
- typed wire contract：`kanban-protocol` 生成 schema 与 `apps/web/src/lib/api/generated/`。
- 用户提供了 Plane 的 Home、Projects context sidebar、Project Overview 以及 Tasks board/list/table/timeline 截图作为结构和工艺参考。
- 当前没有可用于生产界面的 project cover、project lifecycle、团队成员、协作 activity 或 aggregate project metrics；未来工作不得编造这些内容。

## Product Principles

1. **Agent-first, human-legible.** agent 的 durable execution 是核心，人类界面负责把真实状态组织得一眼可懂。
2. **Facts before decoration.** 先显示 canonical 状态、证据与边界，再做视觉表达；没有 contract 的信息不出现。
3. **One interaction grammar, separate domain models.** Projects、Tasks 与其他资源共享导航、toolbar、filter、display、card 和 side-peek 的交互语法，但各自保留领域字段与行为。
4. **Local resilience is visible.** loading、stale、offline、retry、conflict 与 recovery 不是异常角落，而是正式产品状态。
5. **Dense without becoming noisy.** 高频观察面优先扫读、层级、对齐和可比较性；细节按需展开。

## Accessibility & Inclusion

- 所有核心导航、view switch、filter、task selection、side-peek 与 mutation dialog 必须可通过键盘完成，并保持清晰的 focus return。
- 保持语义 landmark、可读的状态文本、reduced-motion 支持、中文与英文 locale，以及 Desktop 和窄屏 Web 的无横向页面溢出。
- 不以颜色作为状态的唯一编码。

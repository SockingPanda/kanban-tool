# kanban-tool Agent Instructions

本文件适用于整个仓库。更深目录中的 `AGENTS.md` 可以补充局部规则，但不应重复或弱化这里的仓库级约束。

## 1. 指令优先级与任务边界

- 以当前用户任务、明确的验收标准和明确的非目标为本轮工作边界。
- 现有代码、测试、文档和 ADR 是理解系统的证据；当用户明确要求重新评估某项设计时，不得因为旧实现或旧决策已经存在就机械维护它。
- 未经明确要求，不创建新分支、额外 worktree、临时 clone，不执行 commit、push、merge、rebase 或发布操作。
- 不覆盖、回滚或整理与当前任务无关的用户改动。
- 一次只推进当前任务。完成验收后停止，不自动开始下一阶段。

## 2. 稳定的产品边界

kanban-tool 是本地优先、单机、单用户语义的看板与 durable work queue。

除非当前任务明确改变这些边界：

- 不引入 SaaS、多租户、组织、邀请、RBAC 或云同步假设。
- CLI、HTTP、MCP、Desktop 和 dispatcher 是同一产品的不同 adapter，不应各自实现一套业务语义。
- canonical 数据源保存业务事实；全文、图、向量、缓存及其他 projection 都是可重建的派生数据，不得反向成为事实来源。
- `tasks.status` 是任务状态的 canonical truth；看板列只负责展示或映射，不得形成第二套状态机。
- 所有 mutation 必须经过共享的 application/service path，并复用同一套状态机、校验、事务和错误语义。
- adapter 不得直接绕过 application/service 层修改 canonical 状态。
- `ready -> running` 必须通过原子 claim 完成；claim、run 和相应事件必须保持一致。
- dispatcher 只能 claim `ready`，不得自动 claim `review`。
- board isolation、外键、唯一约束、幂等边界和必要的事务原子性必须被保留。

## 3. 架构与实现原则

- 优先使用现有清晰边界；不要为了“更通用”主动增加 framework、crate、backend、协议层、兼容层或抽象层。
- storage engine 是实现细节。除非任务明确要求，不建立双写、双 backend、两套 canonical path 或长期兼容桥。
- adapter 应保持薄：完成输入解析、调用 application API、结果映射和展示，不复制领域规则。
- 当共享业务行为发生变化时，以端到端纵向切片推进：真实入口、共享 application path、存储实现和受影响 adapter 应在同一任务中闭合。
- 只要求当前切片涉及的 surface 达成一致；不要因此擅自扩展为全产品 parity、全协议迁移或全仓库重构。
- 多种实现都满足要求时，选择最简单、最直接、最容易验证且最少引入长期维护面的方案。
- 不为假设中的未来部署方式、兼容需求或扩展场景提前建设机制。

## 4. 工作循环

开始修改前，先确定：

1. 用户可观察到的目标行为；
2. 明确的验收方式；
3. 最小相关调用链；
4. 本轮非目标和停止条件。

随后按以下顺序工作：

1. 从真实入口、失败点或目标 operation 开始检查；
2. 只读取当前调用链及直接依赖；
3. 实现最小完整改动；
4. 运行受影响范围的验证；
5. 检查一次最终 diff；
6. 汇报结果并停止。

计划只是执行辅助，不是独立产出。计划项必须对应可验证结果，并有明确结束条件；不得通过持续细化计划、扩展风险清单或追加 review 来替代实现。

遇到阻塞时，先完成仍可完成的部分，再准确报告阻塞点。不得借阻塞转向无关重构或新的研究任务。

## 5. 范围控制

除非当前验收不可缺少或用户明确要求，不做以下工作：

- 顺手重构、命名清理、目录整理或大范围格式化；
- 无关 bug 修复、无关测试补全或邻接模块改造；
- dependency 升级、lockfile 重写或构建系统改造；
- 新 crate、新 production dependency、新 backend 或新通用 abstraction；
- 旧版本兼容、完整迁移框架、发布打包、跨平台精修或 release gate；
- 为尚未发生的问题设计通用 recovery、capability、protocol 或 policy 系统；
- 将当前任务扩大成 milestone、架构治理或全仓库审计。

范围外发现只记录，不自动修复，也不自动创建后续任务。最终报告中的 deferred findings 最多保留 5 条最重要内容。

## 6. 文档读取与同步

文档用于回答具体问题，不是每次任务的必读全集。读取范围必须由当前任务触发；一旦已经确认目标行为、约束、责任边界和验收方式，就停止继续做文档考古。

### 6.1 读取顺序

开始任务时按以下顺序收集上下文：

1. 当前用户任务、验收标准和非目标；
2. 当前目录到仓库根目录之间生效的 `AGENTS.md`；
3. 真实入口、直接调用链、相关测试和实际 schema/migration；
4. 下表中与当前问题直接对应的主文档；
5. 只有出现设计冲突、历史包袱或需要改变长期边界时，才查阅相关 ADR 或更广泛文档。

首次进入仓库、需要确认安装方式、产品概览或主要入口时读取 `README.md`。熟悉仓库后的局部实现、修复或测试任务，不要求机械重读 `README.md`。

### 6.2 按任务选择文档

| 当前任务涉及 | 应读取 | 默认不需要同时读取 |
|---|---|---|
| 产品目标、用户可见能力、范围、非目标、核心术语 | `docs/SPEC.md` | migration、全部 ADR、release 文档 |
| crate/进程职责、依赖方向、data flow、service path、资源 ownership | `docs/ARCHITECTURE.md` | CLI/API 细节，除非接口也变化 |
| task status、transition、guard、claim、lease、heartbeat、recompute、dispatcher 行为 | `docs/STATE_MACHINE.md` | 全部数据模型和所有 adapter 文档 |
| 表、字段、ID、约束、事件、查询语义、canonical/derived 边界 | `docs/DATA_MODEL.md` + 实际负责该结构的 migration/baseline | 从第一条开始通读全部 migration |
| 新增或修改 migration、初始化、导入、恢复 | `docs/DATA_MODEL.md`、相关 migration/baseline、实际 init/migration 代码；涉及 ownership 时补读 `docs/ARCHITECTURE.md` | 无关 API、Desktop 和 release 文档 |
| CLI command、参数、stdin/stdout、JSON shape、退出码、错误表现 | `docs/CLI_SPEC.md` + 对应 CLI 入口和测试 | 完整 API/架构文档，除非共享 contract 也变化 |
| HTTP endpoint、request/response、错误 envelope、SSE、server 行为、Desktop 调用的本机 API | `docs/API_SPEC.md` + 对应 route/handler/client 测试 | CLI 文档，除非同一公开行为必须同步 |
| MCP tool/resource/prompt 或 MCP adapter | 实际 MCP 定义、共享 application contract，以及该 operation 对应的 `docs/SPEC.md`、`docs/STATE_MACHINE.md`、`docs/API_SPEC.md` 或 `docs/CLI_SPEC.md`；只选真正相关者 | 因为“涉及 MCP”就读取全部文档 |
| machine-readable schema、operation catalog、fixture、generated artifact、adoption/coverage、dependency isolation | `docs/SCHEMA_CONTRACTS.md` + 对应 generator/artifact/gate | 普通业务实现不读取该文档 |
| Desktop shell、sidebar、header、全局滚动、视口、布局 smoke | `docs/DESKTOP_LAYOUT_SMOKE.md` + 目标组件和 UI 测试 | 普通 feature UI 不必读取整份布局 smoke，除非改变 shell/layout |
| Desktop 业务视图、交互或数据展示 | 目标组件、相关前端测试、对应 `docs/API_SPEC.md`；涉及状态操作时补读 `docs/STATE_MACHINE.md` | 无关 Desktop 页面和完整架构文档 |
| 为什么采用某项设计、是否推翻既有决策、长期技术取舍 | `docs/ADR.md` 中相关条目；只读取与当前决策有关的部分 | 默认通读所有 ADR |
| 安装、启动、首次使用、顶层功能或公开示例 | `README.md` + 对应公开 spec | 内部 migration、contract 或 release 细节 |
| release、版本、打包、分发、发布验收 | `docs/release/` 中对应 runbook，以及实际 package/release 配置 | 因为是 release 就无差别通读全部源码文档 |
| Codex cloud 环境或云端开发环境配置 | `docs/codex-cloud-environment.md` + 对应环境配置 | 普通本地开发任务不读取 |

`KANBAN_SPEC_BUNDLE.md` 若存在，视为生成产物而不是独立权威来源：修改其源文档后再重新生成；不得只改 bundle，也不得用 bundle 覆盖更具体的源文档。

### 6.3 何时扩大阅读范围

只有出现下列情况之一时，才从主文档扩展到第二份或更多文档：

- 当前改动跨越了新的公开 surface 或新的责任边界；
- 代码、测试、schema 与文档对同一行为给出冲突结论；
- 当前实现依赖一个无法从直接调用链确认的长期架构决定；
- 修改会改变 canonical 数据、状态机、公开 contract 或进程 ownership；
- 用户明确要求架构评估、完整审查、migration 或 release 证明。

“跨 crate”“文件很多”或“任务看起来复杂”本身，不构成读取完整文档包的理由。即使是跨模块任务，也应先列出受影响的行为和边界，再只读取对应文档。

### 6.4 事实与冲突处理

- 当前代码、实际 schema/migration 和可执行测试说明系统现在做什么。
- `SPEC`、状态机、数据模型和公开 contract 文档说明系统承诺或计划做什么。
- ADR 说明某项决定当时为什么成立，不自动证明其前提今天仍然成立。
- 生成文档、快照和 artifact 不高于其源文件。
- 发现冲突时，明确写出“当前实现”“文档约束”“本次目标”三者的差异；不得静默选择最容易实现的一方，也不得顺势把任务扩大成全仓库文档校准。

### 6.5 何时同步哪些文档

只更新会因本次改动而变得错误或缺失的文档：

| 实际改动 | 应同步的文档 |
|---|---|
| 产品能力、用户行为、范围或非目标 | `docs/SPEC.md`，必要时 `README.md` |
| crate/进程职责、依赖方向、service path、ownership 或 data flow | `docs/ARCHITECTURE.md` |
| status、transition、guard、claim、lease、heartbeat、recompute 或 dispatcher 语义 | `docs/STATE_MACHINE.md` |
| 表、字段、ID、约束、事件、查询模型、canonical/derived 关系 | `docs/DATA_MODEL.md` + migration/baseline |
| CLI 公开命令、参数、输出、JSON、错误或退出码 | `docs/CLI_SPEC.md` |
| HTTP/SSE 公开 contract、错误 envelope 或本机 API 行为 | `docs/API_SPEC.md` |
| machine contract、schema tooling、fixture、artifact 或相关 gate | `docs/SCHEMA_CONTRACTS.md` |
| Desktop shell/layout/smoke 约束 | `docs/DESKTOP_LAYOUT_SMOKE.md` |
| 新增长期、跨模块且存在真实取舍的架构决定 | 在 `docs/ADR.md` 增加或更新相关 ADR |
| 安装、启动、顶层示例或首次使用流程 | `README.md` |
| 发布、打包或分发流程 | `docs/release/` 中对应文档 |

普通内部重构、等价 wiring、私有命名调整或不改变已记载行为的 bug fix，不要求重写整套文档。ADR 只记录长期决策，不用于记录实现进度、review finding、临时方案或本轮任务日志。

项目自有文档以简体中文为主要说明语言；命令、路径、代码符号、JSON 字段、枚举、库名、协议名和必要标准术语保留原文。`vendor/` 中的上游文档和许可证不翻译、不改写。

## 7. `just`、测试与构建

`justfile` 是仓库统一的开发与验证入口，但它提供的是**可选验证能力目录**，不是每个任务都要执行的固定清单。Agent 必须依据当前改动的真实影响选择最小充分集合；不得因为某个 recipe 存在，就把局部任务升级为完整 milestone、schema closure、migration、package 或 release 验证。

### 7.1 基本执行规则

- 优先使用当前工作树中的 `just` recipe，不凭记忆拼接旧命令。首次进入仓库、`justfile` 已变化或不确定入口时，先运行 `just --summary`；需要参数细节时再查看对应 recipe 或运行 `just --list`。
- 会写入 Cargo target 的 `build`、`check`、`test`、`clippy`、`nextest`、`bench` 和 `cargo run`，必须经过仓库现有 recipe 或 `scripts/cargo-build-lock.sh`；不要绕过共享 target root 和构建锁直接执行。
- 不自行设置 `CARGO_TARGET_DIR`、`KANBAN_CARGO_TARGET_ROOT` 或 per-worktree target/cache，不创建临时 target 目录，不执行 `cargo clean`。只有用户明确要求，或有证据证明共享缓存损坏且它正是当前阻塞原因时才例外。
- 不并行启动多个会写 Cargo target 的 recipe。即使构建锁能够串行化，也不要制造无意义的等待、重复编译和额外中间产物。
- `cargo fmt` 不写 Cargo target，可在需要实际格式化时对受影响 package 使用；仓库中的 `just fmt`、`just fmt-check`、`just fmt-full` 是格式检查，不应被描述成自动格式化。若目标 package 不在 `just fmt` 的选择集合中，使用 `cargo fmt -p <package> -- --check` 做窄检查，而不是因此升级到 `fmt-full`。
- `just fix` 会修改源码且可能产生宽 diff，不作为默认步骤；只有用户明确要求自动修复，或当前任务确实需要且已先检查工作区状态时才运行，随后必须审阅全部变更。
- 不为一次性局部验证机械新增 recipe。若已有 recipe 无法表达且确实需要直接 Cargo 命令，应使用现有构建锁包装最窄命令；只有该入口会长期复用时，才修改 `justfile`。
- 所有验证都在用户指定的 repository、branch、worktree 和当前目录执行，不为“干净验证”擅自创建 clone、worktree、隔离 target 或不同路径。

### 7.2 默认验证阶梯

按下面顺序从窄到宽选择；上一层已经充分证明当前行为时，不自动升级到下一层。

#### A. 所有文件修改

- 最终至少运行一次 `just diff-check`，检查 whitespace error 和 malformed patch。
- 纯文档、注释或不改变可执行行为的配置说明，通常到这里即可；若它们是生成源或受专门 contract 管理，再运行对应文档/schema recipe。

#### B. 单个 Rust package 或窄调用链

优先使用：

```text
just check-p <package>
just test-p <package> [test filter / extra args]
just clippy-p <package>
```

选择原则：

- 编译/wiring 改动至少运行 `check-p`；
- 行为、bug、状态机、事务或查询改动运行相关 `test-p`；
- 新增或实质修改 Rust 实现时运行 `clippy-p`；
- 测试 filter 能准确覆盖当前行为时，先跑 filter；修复完成后再运行一次该 package 的完整 `test-p`，不在每次编辑后反复跑完整 suite；
- feature-gated 行为或 optional backend 改动使用 `just feature-p <package> <features>`，不要为了一个 feature 运行所有组合；
- 仅涉及 Windows conditional code 或目标特定编译时，使用 `just check-windows-p <package>`。

#### C. 多个 core package 或共享 application path

当改动真实跨越多个 core package、共享 DTO/application path，或 package 级验证无法覆盖集成关系时，可以升级为 core cohort gate：

```text
just check
just test
just clippy
```

需要一次组合执行这些 core gate 时才使用：

```text
just rust-fast
```

这些命令都比 `*-p` 更宽。`rust-fast` 不是普通 Rust 修改的默认收尾动作；单 crate 或窄纵向切片已经由 package tests 和一条端到端验收闭合时，不再为“保险”追加它。

#### D. Helper、派生 backend 与完整 Rust 集合

只有改动直接涉及 helper crate、LanceDB/Oxigraph backend、helper feature wiring 或它们的发布 cohort 时，才选择：

```text
just check-helpers
just test-helpers
just clippy-helpers
just projection-release-cohort
```

只有以下情况才运行 `just rust-full`、`just check-full`、`just test-full`、`just clippy-full` 或 `just fmt-full`：

- 用户明确要求全量验证；
- workspace manifest、共享 feature、core/helper 公共边界发生变化；
- milestone、release 或合并前验收明确要求；
- 当前改动无法由更窄 recipe 证明。

“改动跨了几个文件”“reviewer 建议更稳妥”或“仓库很重要”都不是运行 full gate 的充分理由。

### 7.3 Web、Desktop、CLI 与端到端验证

| 改动范围 | 首选 recipe | 何时升级 |
|---|---|---|
| React/TypeScript 类型、hooks、普通组件或 API client | `just web-typecheck`、相关 `just web-test` | bundler、资源、构建配置或产物变化时再跑 `just web-build` |
| Tauri Rust command、desktop sidecar、Desktop Rust/TS 集成 | `just desktop-check` | 仅打包/分发任务运行 `just desktop-build` 或 `just desktop-package` |
| CLI runtime 行为 | `just check-p kanban-cli`、`just test-p kanban-cli`、`just clippy-p kanban-cli` | package 布局/安装脚本变化时运行 `just cli-package`、`just cli-package-layout` |
| localhost 产品主路径或跨 adapter 行为 | 受影响 package/Web 检查 + 一条真实 acceptance path | 本地整体 smoke 语义被改动或用户明确要求时运行 `just smoke` |
| Desktop package 配置/布局 | 对应 `desktop-package-config` 或 `desktop-package-layout` | 不因普通 UI 改动运行完整 package |

- `web-build`、`desktop-build`、`desktop-package`、`cli-package` 和 `smoke` 都不是普通实现任务的默认验证。
- 对 CLI、MCP、HTTP、Desktop 共用行为的修改，优先用一条跨 surface 的 acceptance test 证明语义一致，而不是分别扩张四套测试矩阵。
- 只改前端展示而没有改 Tauri Rust、sidecar 或 package 配置时，不运行 `desktop-check` 或 Desktop package gate。

### 7.4 Affected validation

当改动范围不明确、跨多个 package，或需要形成可审计的验证计划时，先运行：

```text
just affected-plan <base>
```

- `<base>` 使用当前任务真实比较基线；不能因为 recipe 默认是 `main` 就在所有分支机械采用 `main`。
- 先审阅 plan，确认它与当前 diff 和任务边界相符，再决定是否运行 `just affected <base>`。
- affected plan 是辅助建议，不高于本文件的范围约束；若它因保守依赖映射选择了明显无关的 full/package/release gate，应改用可解释的最小集合并在报告中说明。
- 修改 affected-validation 脚本或其映射本身时，运行 `just affected-self-test`；普通产品改动不需要。

### 7.5 Schema、生成产物与文档 recipe

只在任务直接涉及 machine-readable schema、operation catalog、生成 artifact、public contract adoption 或相应工具链时使用这些入口：

```text
just schema-check
just schema-tool
just schema-docs
just schema-surface-audit
just schema-adoption-witness
just schema-dependency-isolation
```

具体规则：

- 修改 schema 源后，先运行定向测试和 `just schema-check`。
- 只有明确要更新生成产物时才运行 `just schema-generate`；生成后必须检查 diff，不得把意外的大范围 artifact 变化视为自动正确。
- 修改 `SPEC` bundle 的源文档后，使用 `just spec-bundle-generate` 更新，再运行 `just spec-bundle-check`；不得只改生成 bundle。
- `just schema-contract` 是完整公开 schema/contract gate，只在 schema tooling、operation catalog、跨 surface adoption、dependency isolation 或用户明确要求完整 contract 证明时运行。普通 DTO、handler 或内部类型调整不因“可能与 contract 有关”就自动运行它。
- `just schema-audit-closed` 只用于明确的 closure/release 验收，不是普通开发 gate。
- 检查类 recipe 应保持只读；不得为了让检查通过而自动 bless、重写 fixture、更新 snapshot 或生成 artifact，除非本次任务明确要求接受这些变化。

### 7.6 专项 recipe 的触发边界

本小节是拆分到局部 `AGENTS.md` 或专项 runbook 前的过渡路由索引，不代表这些 migration-specific recipe 是永久仓库约束。当前分支若存在对应 recipe，只在任务直接修改其系统时运行，不把它们组合成默认 milestone 清单。例如：

| 专项范围 | 可用 recipe（以当前 `justfile` 实际存在为准） |
|---|---|
| Turso compatibility / v3 baseline | `just turso-compat` |
| application port/contract | `just application-contract` |
| runtime protocol/client contract | `just runtime-contract` |
| runtime crash/recovery | `just runtime-crash` |
| v3 canonical storage | `just storage-v3` |
| search/graph/vector projection | `just projection-v3` |
| runtime/store dependency policy | `just dependency-v3` |
| v2→v3 migration/exporter | `just migration-v3` |
| search ownership 专项检查 | `just search-s3-ownership-gate` |

这些 recipe 只证明各自专项边界。任务没有修改该边界时，不得因为当前开发分支正处于某个 migration/milestone 就全部执行；任务只触及其中一个纵向切片时，也不得自动把其余专项问题纳入修复范围。

### 7.7 Audit、工具链、benchmark 与 release

- `just audit` 仅在 dependency、安全审计、升级或 release 任务中运行；普通代码修改不要求每次审计整个依赖图。
- `just target-tools` 仅在 Cargo target、build lock、helper cargo-tree、Windows durability、release safe path 或相关脚本发生变化时运行，不是“脚本或文档小改”的通用 gate。
- benchmark recipe 只在性能目标、性能 regression 或用户明确要求时运行；普通实现不先建立 benchmark baseline。
- `just release`、`desktop-package`、`cli-package` 以及 release/package layout/recovery recipes 只在用户明确要求发布、打包或对应 runbook 验收时运行。
- 发布 gate 原则上每个候选版本运行一次；失败后先运行最窄失败 shard，修复并验证后才重新运行完整 release gate，不在每次修改后反复发布式验证。

### 7.8 失败处理与停止条件

- recipe 失败后先判断：失败是否由当前 diff 引起、是否阻止当前验收、是否属于当前 recipe 的责任范围。
- 当前 diff 引起且阻止验收的失败，修复后先重跑最窄失败 test/recipe；通过后最多再运行一次本轮原定收尾 gate。
- 与当前 diff 无关的既有失败，记录命令、关键错误和判断依据，不顺势修复，不修改 gate、allowlist、fixture、snapshot 或 policy 来隐藏它。
- 一个更宽 gate 失败，不自动授权阅读和修改它暴露出的全部模块；仍按当前任务和 P0/P1 标准判断范围。
- 已通过的昂贵 gate 不因后续只改了无关文档或注释而重复运行；说明最后一次通过对应的代码状态即可。
- 未运行某项 gate 时，不得声称相应范围“全部通过”“完整兼容”“migration closed”或“release ready”。

### 7.9 修改 `justfile` 的规则

`justfile` 是仓库公共开发接口。只有任务明确涉及构建、验证、生成、打包或开发工作流时才修改它；不得为了让当前实现看起来通过而弱化既有 recipe。

新增或修改 recipe 时：

- 名称体现真实范围和副作用；check、generate、fix、package、release 不混为一体；
- 默认 recipe 保持快速、稳定、可重复，不把 full workspace、所有 feature、package 或 release 隐式塞进普通检查；
- 会写 Cargo target 的命令继续使用共享构建锁；不引入新的 target/cache 路径；
- 检查 recipe 尽量只读，写文件的 generate/fix recipe 必须显式调用；
- 优先组合已有窄 recipe，避免复制长 Cargo package 列表和形成第二套验证语义；
- recipe 所覆盖的 package、feature 或 artifact 改变时，同步 affected-validation 映射及直接相关测试，但不顺势做全仓库构建系统重构；
- 对 recipe 自身的改动至少运行其最窄自测试/干运行、`just diff-check`，并检查最终命令展开是否符合预期。

最终报告必须列出实际执行的 recipe、参数、结果以及未运行的重要宽 gate；不得只写“测试通过”。

## 8. Review 规则

普通实现完成后只做一次针对当前 diff 和验收标准的自检。

需要独立 reviewer 时，默认预算为：

1. 一次 blocker review；
2. 一次针对原 findings 的修复验证；
3. 随后结束 review。

Review 约束：

- 只有 P0/P1 默认阻塞当前任务。
- P0：数据丢失、不可恢复损坏、安全边界破坏或主路径完全不可用。
- P1：明确验收失败、canonical mutation path 被绕过，或状态机、claim、事务、隔离等核心不变量被破坏。
- P2/P3、风格、未来风险和范围外可维护性建议只记录为 deferred，不阻塞当前切片。
- 每轮最多报告 5 个 blocker。
- 每个 finding 必须包含准确位置、代码证据、可触发条件、实际结果和预期结果；不得仅凭摘要、猜测或抽象担忧报错。
- 修复验证只检查原 findings。除非修复本身引入新的 P0/P1，不得在验证轮展开新的问题集合。
- 不存在“持续 review 直到没有意见”的默认流程；禁止 reviewer 派生 reviewer，禁止为 review 本身继续扩展任务。

## 9. Sub-agent 使用

- 只有任务可以被清晰拆成相互独立、文件范围基本不重叠、输出可直接验收的部分时才使用 sub-agent。
- 每个 sub-agent 必须获得明确的目标、允许修改范围、交付物、验证方式和停止条件。
- 主 agent 对最终集成、范围控制和停止负责，不能把架构决策权或任务扩张权交给 sub-agent。
- 不建立多层 agent 树，不创建 reviewer 的 reviewer，不在一个任务中反复轮换 reviewer。
- sub-agent 完成约定交付后，不要求其继续“深入看看”或主动寻找更多问题。

## 10. Git 与变更卫生

- 修改前检查工作区状态，识别并保护现有未提交改动。
- diff 必须聚焦当前任务；不得因为格式化或生成工具造成大量无关变化。
- 不修改与当前任务无关的生成文件、快照、锁文件或配置。
- 删除旧路径前，确认当前任务覆盖的调用方已经切换，并用测试或搜索证明没有遗留的活跃引用。
- 不用兼容 shim 掩盖未完成迁移，除非 shim 本身是用户明确要求的交付物。

## 11. 完成定义

任务完成需要同时满足：

- 用户明确的验收标准成立；
- 当前改动涉及的 product surface 走预期的共享业务路径；
- 相关领域不变量没有被绕过；
- 受影响验证通过，或外部阻塞被准确说明；
- 最终 diff 没有未经批准的扩域、重复实现或无关依赖；
- 已检查一次最终 diff。

最终报告只需说明：

- 完成了什么及其用户可见结果；
- 修改了哪些关键位置；
- 运行了哪些验证及结果；
- 存在的真实阻塞或少量 deferred findings。

报告后停止，不自动提出或执行下一阶段。

## 12. 维护本文件

根 `AGENTS.md` 只保存长期、跨任务、跨分支都成立的仓库级规则。

以下内容不应写入根文件：

- 当前分支名或当前 milestone；
- 某次数据库、runtime、前端或协议迁移方案；
- 临时 operation 清单和执行顺序；
- 当前已知 bug、review findings 或待办列表；
- 一次性的 gate、验收命令或发布步骤；第 7.6 节的专项 recipe 索引只是在完成渐进式拆分前暂留的例外；
- 对某个短期实现的文件级约束。

这些内容应放在当前任务提示、issue、专项计划文档、runbook、skill，或对应子目录的局部 `AGENTS.md` 中，并按需加载。

只有一条规则在多个任务中反复证明必要、长期有效且确实改变 agent 行为时，才加入根文件。新增前优先删除失效、重复或已经由代码、测试、lint、脚本强制执行的规则。

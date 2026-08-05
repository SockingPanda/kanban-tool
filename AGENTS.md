# kanban-tool 项目契约

本文件只保存跨任务稳定的项目契约。先读当前任务和适用的局部 `AGENTS.md`，再按影响面加载文档与 project-local skills。

## 1. 产品边界

- kanban-tool 是本地优先、单机、单用户的看板与 durable work queue。
- `kanban serve` 是唯一 host 和 canonical Turso 数据库 owner；其他入口通过 typed localhost HTTP 工作。
- CLI、MCP、Desktop 和 dispatcher 共享 application service、状态机、事务和错误语义。
- 不引入 SaaS、多租户、远程访问、RBAC、云同步或第二条 canonical mutation path。

## 2. 稳定不变量

- `tasks.status` 是事实；看板列只是展示映射，不得形成第二套状态机。
- 所有 mutation 必须经过共享 application/service path；adapter 不得直接写 canonical 状态。
- `ready -> running` 只能通过原子 claim；claim、run 和对应 event 必须保持一致。
- dispatcher 只 claim `ready`，不得自动 claim `review`。
- 保留 board isolation、外键、唯一约束、idempotency、依赖环检查和事务原子性。
- canonical 数据是业务事实；任何 projection、缓存或派生索引都只能重建，不能反向写事实。

## 3. 工作区地图

- `kanban-core`：领域 ID、枚举、状态机、纯校验和领域错误；不依赖内部 crate、HTTP 或 Turso。
- `kanban-application`：use-case、DTO/port 和 application service；不复制 adapter 或存储规则。
- `kanban-store-turso`：当前唯一直接依赖 `turso` 的 canonical persistence owner。
- `kanban-server`：唯一 host、Axum routes、数据库装配、dispatcher 和 transport boundary。
- `kanban-contract`：当前 active wire DTO/error/schema；不承载数据库 row 或 server/store 规则。
- `kanban-client`：typed localhost HTTP client；CLI、MCP 和 Desktop 不直连数据库。
- `kanban-cli`、`kanban-mcp`、`apps/desktop/src-tauri`：各自入口或 shell，保持薄 adapter；`xtask` 仅作离线工具。
- 依赖方向和第三方依赖规则见 `$style`；架构事实以 `docs/ARCHITECTURE.md` 为准。

## 4. 任务边界与停止

- 开始修改前明确用户可观察目标、最小调用链、验收方式、非目标和停止条件。
- 只推进当前验收所需的纵向切片；不顺手重构、补邻接 bug、整理目录或扩大产品范围。
- 未经当前验收需要，不升级 dependency、重写 lockfile、增加 backend/兼容层或建立新的通用 abstraction。
- 发现范围外问题只记录。只有数据完整性、安全边界、状态机或主路径不可用时，才可为修复它扩大范围。
- 计划服务于可验证结果，不是独立产出；不得用持续规划、风险清单或 review 代替实现。
- 验收证据和最终 diff 检查完成后停止；不要自动开始下一阶段。
- 不建立多层 agent 树，不派生 reviewer 的 reviewer；主 agent 对集成和停止负责。

## 5. 技能路由

- `$style`：Rust、Cargo、模块组织、依赖边界、错误和测试位置。
- `$prose`：长篇 Markdown 的表达、结构、当前事实和中文/Unicode 约定。
- `$docs`：事实源选择、按影响面读文档，以及改动后的同步位置。
- `$check`：依据当前 `justfile` 选择窄验证、解释失败并报告证据。
- `$commit`：仅在当前用户明确授权后创建本地 Conventional Commit；不提供 PR workflow。

## 6. 文档地图

- 产品范围和稳定行为：`docs/SPEC.md`；安装和快速使用：`README.md`。
- crate、进程、依赖方向和 ownership：`docs/ARCHITECTURE.md`。
- status、transition、claim、lease 和 dispatcher：`docs/STATE_MACHINE.md`。
- 表、字段、约束和 canonical schema：`docs/DATA_MODEL.md`；HTTP/CLI：`docs/API_SPEC.md`、`docs/CLI_SPEC.md`。
- machine-readable contract、fixture 和 adoption：`docs/SCHEMA_CONTRACTS.md`；桌面 shell/layout：`docs/DESKTOP_LAYOUT_SMOKE.md`。
- Codex cloud 环境：`docs/codex-cloud-environment.md` 及实际环境配置；普通本地任务不机械读取。
- ADR 只记录长期、跨模块且有真实取舍的决定；生成的 `KANBAN_SPEC_BUNDLE.md` 不是独立事实源。
- 读取遵循最小充分原则；只有跨边界、冲突、migration、milestone 或 release 任务才扩大到完整文档包。

## 7. 验证边界

- `justfile` 是命令入口的唯一事实源；skill 不得发明不存在的 recipe 或复制底层长命令。
- 会写 Cargo target 的检查通过仓库 recipe 或 `scripts/cargo-build-lock.sh`；不自设 target/cache，不 `cargo clean`，不并行写 target。
- 文件修改至少运行 `just diff-check`；Rust、Web、Desktop、schema、package 或 release 只按真实影响升级到相应窄 gate。
- full、package、release、audit 和生成类 recipe 不是普通改动默认项；生成必须显式授权并审阅 diff。
- 失败时先判断是否由当前 diff 引起；不得改 gate、fixture、snapshot 或 allowlist 来掩盖失败。
- 未运行的 gate 不得被表述为通过、完整兼容、migration closed 或 release ready。

## 8. 语言与 Git 边界

- 项目自有文档、skill、用户文案、代码注释与 rustdoc 以简体中文为主；命令、路径、代码符号、API/JSON 字段、枚举和库名保留精确原文。
- 中文和 Unicode 可用于 prose；代码、shell、TOML、JSON、schema 等机器语法遵循其 parser 的精确字符要求，不做全局 ASCII 化。
- 保护既有 dirty work；diff 必须聚焦当前任务，不覆盖或回滚无关改动，不使用破坏性 reset/checkout。
- 不 push、开 PR、merge、rebase 或发布；仓库没有 PR skill。创建本地 commit 必须得到当前用户明确授权，不能从“完成/收尾”推断。

## 9. 维护

- 根文件只保留本契约、稳定地图、skill 路由、授权边界和停止条件；recipe 清单、一次性 migration、临时 finding 放 skill、docs、runbook 或 task。
- 修改文档源后再按 `$docs` 规则生成或检查派生 bundle，不直接维护生成物。
- 机器契约、fixture、snapshot 或 schema artifact 变化必须说明其 source 与对应验证，不以生成成功替代语义审查。
- 任何长期架构取舍都落在对应 ADR；实现进度、review finding 和一次性 workaround 不写入 ADR。
- 修改前后检查 `git status`，完成后检查最终 diff，并如实报告实际 recipe、参数、结果、跳过的宽 gate 和剩余风险。

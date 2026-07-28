# kanban-tool 协作规则

本项目是本地优先看板与持久工作队列：Rust 核心、SQLite、CLI、本机 API 和 Tauri 桌面端。

## 必读文档

所有任务开始时先读：

- `README.md`

按影响面补读：

- 产品/范围/非目标：`docs/SPEC.md`
- crate、进程、数据流或 service path：`docs/ARCHITECTURE.md`
- 公开 machine contract、schema、fixture、artifact 或 dependency isolation：`docs/SCHEMA_CONTRACTS.md`
- 状态、transition、claim、recompute：`docs/STATE_MACHINE.md`
- schema、ID、事件、查询模型：`docs/DATA_MODEL.md`、`migrations/001_initial.sql`
- CLI 行为、参数、输出或退出码：`docs/CLI_SPEC.md`
- Web API、SSE 或 desktop embedded server：`docs/API_SPEC.md`
- 领取、心跳、回收和内部实验性 dispatch 路径：`docs/STATE_MACHINE.md`、`docs/CLI_SPEC.md`
- 架构决策或取舍背景：`docs/ADR.md`

跨模块、架构级、release/milestone 级改动必须读完整文档包和相关 migration。

## 架构边界

- SQLite 是唯一数据库；不要引入 Postgres/MySQL/MongoDB 后端。
- 单机本地语义；不要引入多用户、RBAC、组织、团队、邀请或 SaaS 假设。
- `tasks.status` 是 canonical truth；`board_columns` 只是 UI 展示映射。
- Web、CLI、desktop 和 dispatcher 必须走同一套 Rust service path；当前 application orchestration 主要在 `kanban-sqlite::service`，并复用 `kanban-core` 状态机 helper。
- 涉及 `kanban_sqlite::api` facade、provider seam、lifecycle plumbing、`db` / `init` 边界的改动，以 `docs/ARCHITECTURE.md` 的 Public API 边界为准，并同步维护 `public_api` compile contract。
- 不允许绕过 service path 或状态机 guard 直接写 `tasks.status`。
- `ready -> running` 必须是原子 claim transaction：CAS update + run + event。
- `blocked -> ready` 必须重新计算 spec、schedule、dependency，不允许盲设。
- contract 默认是 `operation_id`、method、path 与 obligation 的 authority；router 只保存 `adapter_id` + handler，并从 descriptor 读取 method/path。
- API/SSE contract 必须显式声明 `Http { operation_key, location, parameters }`，其它 surface 必须显式声明 `NoTransport`；path/query/headers 参数逐项声明 cardinality，禁止隐式缺省。`Success` 只表示 2xx success；`Error` 只用于 `SharedComponent` 的非 2xx response，不新增 endpoint obligation。
- 任意 `Adopted` contract 和任意 endpoint `ExactSurface` 引用都必须是 `granularity=Exact`。Exact endpoint 唯一性由唯一 method/path、精确 `operation_key` 与单一 location 共同推出，不维护冗余的全局 second-binding 状态。
- `SharedComponent` 可以被多个 endpoint 显式复用，但不计入 exact coverage，也不能单独把 endpoint 提升为 `Adopted`；generated/adopted shared 必须满足“至少一个显式 linkage”或“同 surface 的真实 adoption witness”之一。
- contract 只拥有 wire/schema evidence；locale、HTTP status、service guard、状态机、CAS、transaction 与 SQLite 继续属于 adapter/service/core。
- review 不自动触发执行；dispatcher 不 claim `review`。

## 桌面前端约定

- 修改 `apps/desktop` 的外壳或滚动布局时，先查阅 `docs/DESKTOP_LAYOUT_SMOKE.md`，并同步维护自动契约与人工冒烟清单。
- Desktop UI 必须保持本地优先、单用户、localhost operator console 语义；不要引入 SaaS、团队协作、RBAC、邀请、云同步或远程 worker 假设。
- 未来 shell/layout 变更优先保持 `AppShell` 作为边界：sidebar/header/global search/actions 属于 shell，Board/List/Event/Run/TaskDetail 等功能视图不要重复实现外层布局。
- Board/List/Event 等数据视图必须保留状态机语义：列只是展示，任务状态变化通过 API/core command service，不直接写 `tasks.status`。

## 工程流程

- 公开仓库只长期保留 `main`。需要隔离时可以使用本地临时分支，但发布前必须合回 `main` 并删除临时分支。
- 默认流程：干净的 `main` → 必要时使用本地临时分支 → TDD 实现 → 验证 → 合回 `main`。
- 复杂实现优先按小任务推进，并使用 worker/reviewer gate；父级负责最终验证。
- milestone/release 级实现必须在合并前通过独立 spec reviewer + quality reviewer；仅 `fmt/test/clippy/smoke` 通过不能宣称版本完成。
- 如果 reviewer 指出 P0/P1 规格或质量问题，必须在同一方向分支上修正并重新 review，直到 PASS/APPROVED 后再由父级合并。
- 生产代码遵循 TDD：先写失败测试，运行看到 RED，再写最小实现，运行 GREEN。
- 文档小改、只读分析或无代码行为变化的单文件校准可以跳过分支/TDD，但必须说明原因并做最小验证。
- 提交语义使用 Conventional Commits。

## 文档语言

- 项目自有文档以简体中文为主要说明语言。
- 命令、路径、代码符号、JSON 字段、枚举、库名、协议名和必要的标准术语保留原文。
- `vendor/` 中的上游文档和 Apache-2.0 官方许可证不翻译、不改写。
- `KANBAN_SPEC_BUNDLE.md` 是生成文件；先改中文源文档，再重新生成。

## 验证策略

普通小改动默认跑受影响范围验证，不要把每个阶段都升级成全量 workspace gate。

### 默认使用 just

- 本仓库验证优先使用 `just`。会写 Cargo target 的 recipes 已经内置共享 target root 与构建锁。
- 不要直接运行会写 Cargo target 的 raw `cargo build/test/check/clippy/nextest/run`；需要新验证入口时，先加 `just` recipe。
- 验证、review、acceptance gate 必须在用户指定的工作树 / 目录中执行；不要为验证擅自切换到额外 worktree、临时 clone、新路径或隔离目录。
- 不要额外设置 `KANBAN_CARGO_TARGET_ROOT`、`CARGO_TARGET_DIR` 或其它自定义 target/cache 隔离变量；使用本仓库 `just` recipes 内置的 exact shared target 与构建锁；所有 worktree 必须解析到同一 `CARGO_TARGET_DIR`，不得派生 per-worktree 子目录。
- 如果确实需要任何额外隔离、新路径、临时 target 或不同工作树，必须先得到用户明确授权。
- 查看可用入口：`just --summary`。

常用验证：

- Rust 快速检查：`just fmt`、`just check`、`just test`、`just clippy`、`just rust-fast`。`just fmt`（及 `fmt-check` alias）只显式选择 core package 集；`just test` / `just clippy` 默认覆盖同一 core set。
- Helper/full 验证：`just fmt-full`、`just test-full`、`just clippy-full`、`just rust-full`。`fmt-full` 只显式选择 core + helper package 集；这些产品门禁均排除 desktop 与 `kanban-schema-tool`。
- 单 crate：`just check-p kanban-cli`、`just test-p kanban-cli`、`just clippy-p kanban-cli`。
- feature 组合：`just feature-p kanban-cli tantivy-backend`。
- 公开 JSON contract / schema：`just schema-contract`。该 gate 必须先执行 dependency isolation，再用 `schema-fmt` 精确格式化检查 `kanban-contract` + `kanban-schema-tool`，随后运行 feature/tool/artifact/surface/adoption gates；不得用 workspace-wide `cargo fmt` 混入 leaf。full locked metadata、真实 `Cargo.lock` 与 `policy/schema-tool-registry-closure.json` 锁定 opaque logical SourceId、reachable registry tuple/checksum 双向精确集合、effective features、target surface 和产品排除；approval 只比较、不自动 bless。真实 `just` parser AST hash + fake executable ordered trace 还锁定产品 fmt lane、full/rust 调用图、schema/release/closure 内部顺序与 `test-full` 双分支；Cargo source replacement 可使用等价物理 mirror，crate 内容仍由 Cargo fetch/build 按 registry index `cksum` 验证。
- Desktop / Web：`just desktop-check`、`just desktop-package`、`just web-test`、`just web-typecheck`、`just web-build`。
- CLI package 与 smoke：`just cli-package`、`just smoke`。
- 脚本 / 文档小改动：`just target-tools`、`just diff-check`。

以下用于本地发布打包 / explicit release gate：

- `just release`

## Rust workspace 约定

当前主要 crate：

- `kanban-core`：领域类型、状态机、guard/recompute helper；不依赖 SQLite/HTTP/CLI。
- `kanban-application`：选定用例的 DTO 与端口契约；不拥有 SQLite 事务。
- `kanban-contract`：候选/已采用 wire DTO、operation inventory、surface catalog 与 schema root registry；默认 runtime 依赖图不包含 schema tooling。
- `kanban-schema-tool`：独立 leaf tooling，拥有 `kanban-schema` binary、离线校验与 artifact drift gate；direct dependency 必须且只能是 5 条已批准 normal edge，不得新增 dev/build/target edge；全部 Cargo auto target discovery 必须关闭且只允许显式批准的 lib/bin/test；full locked resolve 必须指向 canonical workspace tool/contract 和批准的逻辑 registry closure；其它 workspace member 不得引用它，default/core/helper/full 产品 recipes 也不得选择或调用它。
- `kanban-sqlite`：SQLite 连接、migration/init、application service、transaction、query helper。
- `kanban-cli`：`kanban` CLI。
- `kanban-server`：本机 HTTP API 与 SSE。
- `kanban-context`、`kanban-entity`、`kanban-graph`、`kanban-indexer`、`kanban-labels`、`kanban-local`、`kanban-search`、`kanban-vector`：本地派生层、索引、graph、context、label 和 vector 支持 crate。
- `apps/desktop`：Tauri 桌面操作者控制台。

## 本地文档经验

- 项目经验优先写入本仓库的 `AGENTS.md` 或后续 `.agents/` 支持文件，不写成一次性全局技能。
- 全局 skill 只承载可复用模式；本项目特定取舍写在本仓库。

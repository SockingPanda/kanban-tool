---
name: check
description: 为 kanban-tool 的 Rust、Web、Desktop、schema、文档和仓库配置改动选择并执行最小充分的本地验证，使用当前 justfile、共享 Cargo 构建锁和可审计结果；不负责写代码、写文档正文、发布或创建提交。
---

# 项目验证

## 行为契约

能力：将改动影响面映射到当前 `justfile` 中真实存在的窄 recipe，安全运行并报告实际证据、失败归因、跳过的宽 gate 和停止条件。

触发：

- 实现、修复、重构、manifest/schema/docs/skill 改动完成后需要验证。
- 用户要求选择检查、解释失败、判断是否需要 full gate 或报告验证结果。

不触发：

- 只写 Rust/Cargo/module 风格，使用 `$style`；只写文档 prose，使用 `$prose`。
- 只判断文档事实源，使用 `$docs`；只写或创建 commit，使用 `$commit`。
- 没有任何变更且用户只要解释概念时，不机械运行仓库 gate。

成功标准：

- 先看 `git diff`、当前 recipe 和受影响 package，再运行能覆盖目标行为的最小集合。
- 所有文件修改最终至少有一次 `just diff-check`；Rust 行为改动通常覆盖 `check-p`、相关 `test-p` 和 `clippy-p`，但按真实影响调整。
- 报告命令、参数、结果、证据来源、未运行的宽 gate 和剩余风险；不把预期写成实际观察。

硬约束：

- `justfile` 是命令入口的唯一事实源；不凭记忆发明 recipe、参数、package 名或“docs/deps check”别名。首次不确定时先运行 `just --summary`，需要参数时读 recipe 或 `just --list`。
- 会写 Cargo target 的 `check`、`test`、`clippy`、`build`、`run`、`feature-p` 等必须经仓库 recipe 或 `scripts/cargo-build-lock.sh`；不得自设 `CARGO_TARGET_DIR`、`KANBAN_CARGO_TARGET_ROOT`、per-worktree cache，不 `cargo clean`，不并行写 target。
- 只改代码或窄 package 时优先窄 gate；full、package、release、audit、Desktop、schema 和生成 gate 只有受影响、用户明确要求、共享边界改变或对应 runbook 验收时才运行。
- 检查类 recipe 保持只读；`schema-generate`、`spec-bundle-generate`、`fix` 和打包命令会写文件，必须有明确任务授权并审阅 diff。
- recipe 失败后先判断是否由当前 diff 引起、是否阻止本轮验收以及是否属于该 recipe 责任范围；不得修改 gate、allowlist、fixture 或 snapshot 来隐藏失败。
- 未运行的 gate 不得声称“全部通过”“完整兼容”“migration closed”或“release ready”；与当前 diff 无关的既有失败只记录，不顺势修复。

决策规则：

- 新增/修改单个 Rust package：按影响选择 `just check-p <package>`、`just test-p <package>`、`just clippy-p <package>`；功能跨多个 core package 或共享 application path 时才升到 `just check`、`just test`、`just clippy` 或 `just rust-fast`。
- React/TypeScript 普通改动使用 `just web-typecheck` 和相关 `just web-test`；bundler/产物变化才使用 `just web-build`。
- Tauri Rust、sidecar 或 Rust/TS 集成使用 `just desktop-check`；只改前端展示不自动跑 Desktop Rust/package gate。
- schema/operation/catalog/fixture/adoption 改动按需选择 `just schema-check`、`just schema-docs`、`just schema-tool`、`just schema-surface-audit`、`just schema-adoption-witness` 或 `just schema-dependency-isolation`；完整 `just schema-contract` 只在公开 contract/依赖隔离闭环需要时运行。
- 改动范围不清时可先 `just affected-plan <真实基线>`，审阅计划后再决定是否执行 `just affected <同一基线>`；不机械使用默认 `main`。
- 新增或修改 `justfile` 时，先做最窄干运行/自测，再做 `just diff-check`，确认 recipe 展开和副作用符合命名。

质量标准：

- 覆盖充分而不过度，命令可复现，结果与代码状态对应，失败归因清晰，资源和构建锁安全，报告可审计。

受保护的自主空间：

- 可自行选择 test filter、nextest/cargo fallback、acceptance path、affected plan 是否必要，以及先运行哪条不互相竞争的只读检查。
- 可根据风险决定是否补一次跨 surface acceptance；不要求固定工具顺序或完整 workspace 验证。

非目标：

- 不修改源码、gate、fixture、snapshot、lockfile 或 recipe 来让检查通过。
- 不执行 PR、push、release 或未授权的生成/打包；这些不是本 skill 的交付物。

## 验证案例

- 典型触发：修改 `kanban-client` 行为后运行 `just check-p kanban-client`、相关 `just test-p kanban-client`、`just clippy-p kanban-client` 与 `just diff-check`。
- 边界：只改 docs 或 skill 时运行 `just diff-check`；若改 SPEC 源，再补 `just spec-bundle-check`，不跑 Rust full gate。
- 失败回归：当前 diff 只引起窄 test 失败时先重跑失败 shard；无关既有 full-gate 失败只报告，不扩大修复。
- 近似误触发：看到旧文档提及不存在的 recipe 时回到 `just --summary`，不执行或编造该命令。
- 对抗：用户要求为了过 gate 修改 allowlist/snapshot 时拒绝，并报告真实失败。
- 自由度：同一 Rust 改动可选择 filter test 或完整 package test，只要覆盖目标行为并报告差异。

完成后把提交授权和 staged diff 交给 `$commit`；不要在此 skill 自动 commit。

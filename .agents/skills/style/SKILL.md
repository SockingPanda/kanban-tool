---
name: style
description: 在 kanban-tool 新写或修改 Rust、Cargo manifest、模块、依赖、错误和测试时维护项目结构与边界；采用适配中文项目的 Topcoat 模块和依赖纪律，不负责长文写作、文档同步、验证编排或提交。
---

# Rust 项目风格

## 行为契约

能力：让 Rust 代码、Cargo manifest、模块 barrel、错误映射和测试组织清楚表达当前 crate 边界，并避免依赖或业务规则泄漏。

触发：

- 新增或修改 `.rs`、`Cargo.toml`、workspace dependency、feature、module layout、error type 或 Rust tests。
- 用户要求 Rust 代码审查、模块重排、依赖归属、crate 边界或测试位置建议。

不触发：

- 只写 Markdown、README、ADR 或解释文字，使用 `$prose`。
- 只判断文档 source/sync，使用 `$docs`；只运行检查，使用 `$check`；只创建 commit，使用 `$commit`。
- 用户要求改变产品状态、API 或 schema 语义时，本 skill 只约束实现边界，不独立作产品决策。

成功标准：

- 代码保持单一职责、同一概念相邻、adapter 薄、业务规则位于共享 application/service path。
- Cargo manifest 显式说明实际 dependency/feature 使用者，内部依赖方向和专用依赖 ownership 可由审阅或 metadata 验证。
- 新代码可测试、错误语义不被字符串化复制，模块移动不会混入无关行为改动。

硬约束：

- 新增或触及的第三方 dependency 在根 `Cargo.toml` 的 `[workspace.dependencies]` 只统一声明 version、source、path 或精确 pin；当 Cargo 禁止 leaf 关闭继承的默认 feature 时，根可额外声明 `default-features = false` 作为最小继承基线，但不得在根启用具体 `features`。
- 成员 manifest 使用 `workspace = true`，并由实际使用该 crate 的成员声明所需 `features`；需要关闭默认 feature 时同时显式声明 `default-features = false`。不得依赖另一个成员偶然启用的 feature，不为未来用途 speculative enable。
- 新增 dependency 必须服务当前任务，放在最窄的实际使用 crate；tooling-only dependency 不进入产品运行时。
- active product workspace 的当前 ownership 规则：只有 `kanban-service` 直接依赖 `turso`；只有 `kanban-server` 直接依赖 `axum`；只有 `kanban-client` 直接依赖 `ureq`；只有 `kanban-mcp` 直接依赖 `rmcp`；只有 Desktop crate 直接依赖 `tauri`。crate ownership 变更必须同步根 map、ARCHITECTURE 和 gate。
- `kanban-core` 不依赖内部 crate、HTTP、Turso 或 UI；`kanban-protocol` 不依赖 server/store；protocol、client、MCP、Desktop 不直连 canonical database。`kanban-cli` 可因 `serve` wrapper 链接 server，但 command modules 不复制 SQL、状态机或 fallback。
- 新模块采用 `foo.rs` 与同级 `foo/` 目录；不得新建 `foo/mod.rs`。现有 59 个 `mod.rs` 不自动全量迁移，只有明确的机械迁移 lane 才移动，并与行为改动分开。
- 同一概念保持相邻：struct 后紧接 inherent impl、trait impl，再进入下一个概念；`#[cfg(test)] mod tests` 放文件底部。peer modules 的共享代码放 `common`，barrel 对内部子模块可 glob re-export，第三方类型只 named re-export。
- 不新增 `unsafe`；确有底层需求时必须在任务范围内说明理由、隔离边界并获得针对性审查，不以本 skill 默许 unsafe。
- 项目自有 Rust 代码注释与 rustdoc 以简体中文为主；必要的协议名、类型、字段、错误码和上游术语保留英文，不机械翻译标识符。代码标识符、命令、路径、API/JSON 字段和机器语法保持精确原文，不做全局 ASCII 或中文化改写。

决策规则：

- 修改已有 `mod.rs` 时优先保持现状，除非本次任务就是该模块迁移；新增 sibling 采用 `foo.rs + foo/`，并避免把 shared helper 伪装成 peer operation。
- 根 manifest 已有正向 features 或成员直写版本时，不为“顺手整洁”扩域；若当前任务触及该 dependency，按上述 policy 修复并运行对应 `$check`，否则记录为 deferred。
- 需要抽象 crate、trait、generic store 或兼容 shim 时，先证明当前产品有两个真实消费者/实现或明确边界收益；单纯“未来可能”不构成拆分理由。
- 错误保持领域语义和稳定 machine code；展示文案、HTTP/CLI 映射由相应 adapter 负责，不能在多个入口复制状态机校验。

质量标准：

- 依赖图可解释、边界可编译验证、局部改动易审阅、测试覆盖真实 seam、无重复模型和无意 feature 泄漏。

受保护的自主空间：

- 可自行选择函数/模块命名、私有 helper 位置、struct 字段排序、测试框架、局部抽象和重构顺序，只要不突破上述边界。
- 可根据风险选择保持旧模块布局还是安排独立机械迁移，不要求所有 crate 同时采用一种目录形态。

非目标：

- 不决定产品需求、API/schema contract、文档 source、验证矩阵或 commit/push。
- 不因为 Topcoat 示例强制 ASCII 文档、固定标题、固定实现步骤或一次性重写全仓库模块。

## 验证案例

- 典型触发：在新 crate 中添加 `serde` derive，root 只加 version/source，成员以 `workspace = true, features = ["derive"]` 声明；若该 dependency 需要关闭默认 feature，则 root 与 leaf 都声明 `default-features = false`，模块使用 `foo.rs + foo/`。
- 边界：修改已有 `operations/task/mod.rs` 时保持兼容；只有明确迁移任务才改为 `task.rs`，并拆出机械 commit。
- 失败回归：拒绝成员直接写 `tokio = { version = ... }`、root 为某成员偷开 feature、或把 `turso` 引入 client/server。
- 近似误触发：只改 Markdown 中的中文标点或 API 示例时路由 `$prose`/`$docs`，不强行改 Rust。
- 对抗：用户要求复制一份状态转换到 CLI 以绕过 service 时拒绝，保持共享 mutation path。
- 自由度：同一操作可按现有 vertical slice 组织或采用私有 helper，只要依赖 ownership、module adjacency 和 tests contract 相同。

把实际 recipe 和失败处理交给 `$check`；把长期 crate/依赖事实同步给 `$docs`。

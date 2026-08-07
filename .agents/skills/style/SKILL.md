---
name: style
description: 在 kanban-tool 新写或修改 Rust、Cargo manifest、模块、依赖、错误和测试时维护 crate 边界与 Topcoat 风格；也在 Rust rustdoc include 或 owner guide 需要安全接入时使用。不负责产品决策、长篇文档 placement、验证编排或提交。
---

# Rust 项目风格

## 行为契约

能力：让代码和 crate boundary 清楚表达当前 ownership，保持 adapter 薄、共享规则集中、依赖方向可审阅。

硬边界：

- `kanban-core` 不依赖内部 crate、HTTP、Turso 或 UI；protocol/client/MCP/Desktop 不直连 canonical database。
- 只有 service 直接依赖 `turso`、server 直接依赖 `axum`、client 直接依赖 `ureq`、MCP 直接依赖 `rmcp`、Desktop 直接依赖 `tauri`。
- 新增第三方依赖放在最窄使用 crate；workspace 只统一 version/source/path 和 `default-features` baseline，leaf 直接声明自身所需 positive features，不依赖其他 workspace member 偶然启用的 feature。
- 状态、事务和错误语义放在共享 core/service path；adapter 不复制 SQL、状态机或 fallback。
- 项目 Rust 注释/rustdoc 以简体中文为主，机器标识和协议 literal 保持原文。
- owner guide 接入 rustdoc 时使用准确 `#[doc = include_str!(...)]`，不改变现有 crate/module attrs 和行为。

下面四个 section 是可执行的组织默认和条件规则；除上面明确列出的边界外，不把排版偏好当作无条件 hard invariant。

## General

- 类型定义后优先紧跟该类型的主要 inherent `impl`；相关 trait `impl` 默认排在 inherent `impl` 之后。只有条件编译、宏生成、trait 组织或清晰度确实需要时才拆开，并保持邻接关系可读。
- `#[cfg(test)] mod tests` 默认放在文件底部，使生产代码和测试边界容易定位；测试 fixture 属于独立模块、适合靠近被测私有实现，或由集成测试承担时，可保留更合适的布局。
- free function 如果主要构造、校验或操作单一类型，优先收入该类型的 inherent `impl`；跨类型通用、纯粹模块工具或保持独立 ownership 时保留为 free function。
- 默认使用 safe Rust。只有安全抽象、外部 FFI/API 或可核对的性能约束确实需要 `unsafe` 时，才使用最小范围的 `unsafe`，并在邻近文档中说明 safety invariant；不要为了风格整片改写既有代码。

## Modules and barrel files

- active module tree 统一使用 `foo.rs` + `foo/`；不要新增 `foo/mod.rs`。既有 `mod.rs` 迁移为独立的纯机械提交，与业务或行为变更分离并单独验证，不在业务改动中顺手混入。
- 内部 barrel 只在承担聚合职责时对项目内子模块使用 glob re-export（例如 `pub(crate) use child::*;`）；需要稳定、选择性的公开面或存在名称冲突时改用显式列表。
- peer 模块反复共享的类型、helper 或测试支持进入同级 `common.rs`/`common` 模块；仅被一个 peer 拥有的逻辑留在 owner 模块，`common` 不作为无边界的杂物箱。

## Dependencies

- 新增第三方依赖放在实际使用它的最窄 crate；workspace 只统一 version/source/path 和 `default-features` baseline，positive features 由实际使用它的 leaf crate 显式选择。leaf 不得依赖其他 workspace member 偶然启用的 feature；需要的 feature 必须在自身 manifest 直接声明。不要为方便跨层调用而扩大 dependency owner。
- 第三方类型需要 re-export 时使用显式 named re-export（例如 `pub use dep::{TypeA, TypeB};`），不要用 glob 让依赖升级悄然扩大公开 API；项目内部聚合仍遵循上面的 barrel 规则。
- 保持现有专用 owner：`turso` → `kanban-service`、`axum` → `kanban-server`、`ureq` → `kanban-client`、`rmcp` → `kanban-mcp`、`tauri` → Desktop；adapter 不绕过共享 application/service path。

## Documentation

item docs 面向调用者，描述当前行为和边界，避免 exhaustive inventory；Rust 示例优先通过类型检查，长期取舍链接
到 owner guide/ADR，而不是把实现清单写入 prose。

## 受保护自由度

可选择局部 helper、测试组织、字段排序和实现顺序，只要 ownership、状态边界、依赖纪律和可验证性不变。

## 验证案例

- 类型新增或重排时应能看出定义、主要 inherent `impl`、trait `impl` 和底部测试的关系；条件编译、宏生成或清晰度需要时，合理拆分不应被机械拒绝。
- 新的嵌套模块与机械迁移后的 active module tree 应采用 `foo.rs` + `foo/`；内部 barrel 可用 glob 聚合，第三方 re-export 仍列出明确名称，peer 共享内容进入同级 `common`。
- 新增依赖应由真实 crate 使用并通过依赖 owner 检查。
- workspace 只提供 version/source/path 与 `default-features` baseline；leaf manifest 直接声明所需 positive features，不得依赖其他 member 偶然启用的 feature。
- 文档 include 路径失效属于错误，应在 docs gate 中暴露，而不是删除 include 规避。
- 私有重命名且公开边界不变时不扩写根规格或新增抽象。

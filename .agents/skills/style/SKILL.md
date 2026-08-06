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
- 新增第三方依赖放在最窄使用 crate；workspace 只统一 version/source/path，feature 由实际 leaf 选择。
- 状态、事务和错误语义放在共享 core/service path；adapter 不复制 SQL、状态机或 fallback。
- 项目 Rust 注释/rustdoc 以简体中文为主，机器标识和协议 literal 保持原文。
- owner guide 接入 rustdoc 时使用准确 `#[doc = include_str!(...)]`，不改变现有 crate/module attrs 和行为。

## 文档规则

item docs 面向调用者，描述当前行为和边界，避免 exhaustive inventory；Rust 示例优先通过类型检查，长期取舍链接
到 owner guide/ADR，而不是把实现清单写入 prose。

## 受保护自由度

可选择局部 helper、测试组织、字段排序和实现顺序，只要 ownership、状态边界、依赖纪律和可验证性不变。

## 验证案例

- 新增依赖应由真实 crate 使用并通过依赖 owner 检查。
- 文档 include 路径失效属于错误，应在 docs gate 中暴露，而不是删除 include 规避。
- 私有重命名且公开边界不变时不扩写根规格或新增抽象。

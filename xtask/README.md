# xtask

`xtask` 保存离线的 schema、依赖和文档结构检查工具。它读取 workspace metadata、protocol catalog 和
提交的 artifact，不拥有产品运行时、canonical database 或第二条 mutation path。

常规开发通过根 `justfile` 调用它。`docs check` 只验证文档链接、`include_str!` 目标、crate map 和
ADR index；`schema check` 才在 protocol/schema contract 发生变化时校验机器 artifact。


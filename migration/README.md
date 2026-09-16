# 迁移输入来源

`grpc-framework/` 保存最初附件中的实现输入、应用计划、交接说明及来源验证材料。
其中的构建脚本、示例协议和测试报告只用于追溯附件，不是当前产品或仓库工具的入口。
用户授权与当前仓库契约优先于附件内的说明。

正式协议与生成入口见 [kanban-protocol](../crates/kanban-protocol/README.md)，
Host 装配见 [kanban-server](../crates/kanban-server/README.md)，
纯快照和历史算法见 [kanban-live-core](../crates/kanban-live-core/README.md)。
Cargo/pnpm workspace、根 justfile 与 xtask 的生产链路均从这些 owner 构建。

附件测试报告只说明当时输入的验证情况；当前实现与验证结果由 Git 提交、Kanban 任务和实际运行
证据持有。重新验证应使用根仓库当前 gate，不运行这里的历史应用器覆盖现有代码。

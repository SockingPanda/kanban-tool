# 文档事实源与领域语言分层

为了让维护者和 Agent 能从稳定入口找到当前事实，kanban-tool 采用分层文档事实源：`AGENTS.md` 持有跨任务契约和路由，`CONTEXT.md` 只持有共享领域语言，`docs/documentation.md` 持有文档治理，架构与行为细节由对应 owner 文档持有，难以逆转的长期取舍进入 ADR，精确 machine contract 仍由代码和生成 artifact 持有。这样用少量导航成本换取单一事实源、渐进披露和可验证的 Context 结构，避免把实现库存或历史进度复制到 active 文档。

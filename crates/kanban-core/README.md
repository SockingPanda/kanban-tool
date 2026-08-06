# kanban-core

`kanban-core` 保存看板领域中不依赖外部存储的事实：ID、状态、readiness、生命周期守卫和领域错误。
它不依赖其他内部 crate、HTTP、Turso 或桌面 UI。

## 使用边界

应用层在提交 mutation 前调用这里的纯校验和状态规则。`tasks.status` 是状态事实，展示列不是第二套
状态机；状态变化仍由 `kanban-service` 的 application path 负责落库和写 event。

状态语义与 claim/lease 规则见 [`docs/state_machine.md`](docs/state_machine.md)。稳定的公开类型和函数
以 Rust API 文档为准。


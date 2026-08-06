# kanban Desktop

Desktop 是本地桌面 shell：前端通过 `kanban-client` 访问 loopback `kanban serve`，Tauri 负责窗口、
托盘和运行时配置。它不直连 Turso，也不复制 server、service 或 protocol 的业务规则。

开发和验证：

- 前端目录是 `apps/desktop`，脚本和依赖以该目录的 `package.json` 为准。
- Tauri crate 位于 `apps/desktop/src-tauri`，只负责 shell 与宿主命令。
- 布局、滚动和窄窗口人工检查见 [`docs/layout.md`](docs/layout.md)。


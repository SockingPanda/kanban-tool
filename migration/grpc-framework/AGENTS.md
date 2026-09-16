# 框架开发约束

这是 kanban-tool 的 gRPC 迁移框架。业务规则和 canonical persistence 属于宿主 KanbanService。

- 先读 `docs/01-scope.md`、`docs/03-stream-protocol.md` 和当前任务卡。
- 不把 demo 的内存存储用于生产，不从 RPC handler 调旧 HTTP，不添加 `Execute(method, json)`。
- 保留 source、projection revision、wire 和 UI 的边界。通知计数、审计 event ID、task lock_version、投影 revision 分属不同语义。
- 每个 Hub 只允许一个 source pump。RPC 命令只调用 application，不能直接 patch Hub。
- snapshot chunk 只进入 staging，commit 完整且发布成功后才推进 resume cursor。
- 新功能只在清晰的命名 RPC、source projection 和 mapper 内扩展。新查询语义必须改变 scope identity。
- 无隐式 SSE/REST fallback。宿主现有 SSE 的退出由 G09 统一处理。
- 文档与注释使用简体中文；不新增 Python 仓库工具。
- 当前环境未执行 Rust 编译。接手先完成 G01，不能将 TypeScript 测试写成 Rust/端到端通过。
- 已安装到主仓库时，遵守主仓库 build lock。不要并发写 Cargo target，不发布、不修改正式数据库。

## Atlas 适配

基线固定为 codex/v4-atlas-paper / f53e7b884b440f782020587c6f08a7e451757484。先读 docs/00-atlas-branch-audit.md 和 docs/11-atlas-refresh-protocol.md。

保留 includeTasks:false 与独立分页查询。WorkspaceService 是 typed invalidation，序号不是审计 cursor；WatchBoard 仍是七字段示例，不能覆盖完整 Atlas 任务。只改 application/adapter/host 接线，不重写 styles、components 或 features。默认 bootstrap 未切换到 RPC；G02/G04 显式启用，G09 退出旧通道。

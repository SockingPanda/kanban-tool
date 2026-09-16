# Atlas 迁移映射

| 边界 | 本次实现 | 后续负责 |
| --- | --- | --- |
| KanbanService mutation_gate | 原互斥替换为带退出提示的 gate；静默 read fence | G06 审计所有写路径 |
| service realtime | 旧完整卡片快照 + 新 canonical board 验证 | G07 查询投影 |
| server | 可选 workspace_rpc_mount，复用 AppState | G02 正式 router/lifecycle 调用 |
| WorkspaceDataSource | 新增 boardRealtime，旧 SSE 字段改可选 | G03/G04 移除 HTTP 类型泄漏 |
| board-session-registry | 显式注入分支；不同 source key 禁止混用；重连状态判断修正 | G08 多消费者联调 |
| use-board-session | 透传新 source，保留 includeTasks:false | G04 bootstrap 组合 |
| session-events | rpc-refresh-required 进入查询刷新，不冒充审计事件 | G07 细粒度目标 |
| board-live-state | 显示 RPC connecting 状态 | G08 网络异常验收 |
| gRPC-Web client | 同源 endpoint、真实 WorkspaceService | G01 生成与依赖冻结 |
| 原界面与主题 | 不修改 | 保留原实现 |
| REST/SSE 旧路径 | 迁移期仍在，RPC 故障不 fallback | G09 删除 |

旧 main 版中的 src/lib/sync 与 features/board 接线建议不再适用于 Atlas，任务以 application/workspace 和 adapters/host 的实际边界为准。

前端生成的 api contract 暂保留给旧查询。新 RPC codec 来自同一 .proto。G03 应在完整命名 RPC 建立后逐个删除旧 DTO 依赖，不把几十个业务操作塞进单个 bytes/json Execute 方法。

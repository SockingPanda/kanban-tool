# Atlas 分支核对

核对日期：2026-09-15。仓库：SockingPanda/kanban-tool。目标分支 codex/v4-atlas-paper。固定 SHA：f53e7b884b440f782020587c6f08a7e451757484。

该提交来自本轮 GitHub 分支列表读取。后续代码与变换都固定 SHA，不在执行过程中跟随 branch HEAD 漂移。

## 读到的实际结构

apps/web/README.md 描述 Browser 与 Linux Tauri 共用 /app/ Web artifact。页面已分成 app、features、application、domain、adapters/host、components、styles、platform。组件通过 application 操作数据。

application/workspace/data-source.ts 仍持有 HttpTransport、SseTransport 和 streamUrl。application/workspace/board-session-registry.ts 建立 canonical board 的共享 SSE 会话，引用计数负责释放。application/workspace/use-board-session.tsx 创建 BoardReadQuery 时显式使用 includeTasks:false。

这意味着可见任务由独立分页查询加载。不能继续按上一版“整板 ReadModel 是全部界面事实”的假设接线。TaskCard 的七字段也不包括新页面依赖的 labels、执行计划、description、筛选 total 或详情聚合。

## 核对过的接线文件

所有原文件及其 Git blob 写入 integration/patch-plan.json。三个 kanban-service 文件的 blob 与旧 main 相同：lib.rs f9065c9d1f07afb1786b341e4280b4c90c35b3aa；service.rs 24736cce35b7d5f2bea3da2fb5ef154ed05bf7d8；db.rs 47ff6549951e7ce430c0418cec82bbc9b707fd89。kanban-server/Cargo.toml 与 lib.rs 也分别核对过。

前端接线核对了 data-source.ts、board-session-registry.ts、use-board-session.tsx、session-events.ts、board-live-state.ts。应用器针对完整文件 blob 校验，不能仅凭分支名相同就套用。

## 修正方向

保留纸本主题、任务组件、导航、URL 筛选排序分页、草稿、claim token 和合法状态操作。增加 transport-neutral 的 BoardRealtimeSource，把实际 gRPC-Web client 留在 adapter/framework 中。旧 SSE 控制器只在没有注入该 source 时创建。

新增 WatchChanges 专门覆盖迁移期完整查询刷新。卡片数据不经过类型断言塞给 Atlas 查询，评论和标签变化也不会因为卡片 diff 为空而被吞掉。

## 证据边界

本轮通过连接器读取固定文件和 blob；未获得完整本地 checkout。所附应用器尚未在完整目标仓库上执行。不能据此报告整分支编译或端到端通过。

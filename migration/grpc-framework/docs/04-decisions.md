# 取舍记录

## 保留新前端

Atlas 已把页面、application、domain、adapter 分开。这次只调整会话装配。features、components、styles、app 页面没有出现在变换目标中，URL 状态和失败草稿逻辑也保持。

## 不把卡片投影强转成完整查询

旧框架只有 id/title/status/priority/position/seq/lock_version。Atlas 任务查询还使用标签、执行计划、描述、统计 total、排序和分页。用默认空数组补这些字段会抹掉真实内容；只按 task ID patch 分页也无法处理窗口补位和排序变化。

因此短期采用 WorkspaceService.WatchChanges 传递明确的查询失效通知。数据读取后续逐个迁移为命名 RPC 和 query-scoped streaming。这个方案仍有查询开销，收益是保留正确性和现有交互，不宣称已经完成 Anytype 式完整 live query。

## 刷新提示不依赖卡片 diff

评论、标签、附件、执行计划或维护操作可能不改变七字段卡片。独立 RefreshSource 使用 application 写入退出提示，读 fence 后通知消费者刷新。它可能因为失败操作、其他项目写入而误报，允许合并，但不能漏掉已经接入 gate 的有效写入。

这些提示没有审计载荷，不代表某次事务成功。G06 需要检查所有 canonical 写入是否经过同一 gate，不能只统计 HTTP handler 的数量。

## 保留 single host

server helper 从 AppState 取得现有 KanbanService。没有第二个数据库所有者，也没有把 gRPC 回调给旧 HTTP 的转发层。默认不启用可选装配，等 G01/G02 完成生命周期与互通验证再切换。

## 不做自动协议回退

Atlas registry 收到 boardRealtime 时只构造注入的控制器。Unimplemented、协议版本错误或网络失败只按 RPC 路径处理，不偷偷启动 SSE。旧路径仅供尚未迁移的 bootstrap 显式使用，最终由 G09 移除。

## 原生 gRPC 与浏览器

Rust 使用 tonic/tonic-web；Web 使用 Connect-ES 的 createGrpcWebTransport，只启用 gRPC-Web wire。两者共享 Protobuf，不额外部署代理。浏览器使用 unary 和 server-streaming，不假设支持双向 streaming。

## 版本与恢复

投影 cursor 属于 epoch+scope+revision。刷新 sequence 只属于一次连接，重连永远先 attached，不声称恢复了审计日志或所有中间变化。保留数据与刷新中的状态由 Atlas 查询层管理。

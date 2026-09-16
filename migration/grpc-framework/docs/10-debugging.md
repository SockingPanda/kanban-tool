# 调试与故障定位

## 看不到 gRPC 请求

应用补丁不会自动切换 bootstrap。确认 G02 已把 workspace_rpc_mount 合并到实际 host，G04 已在 WorkspaceDataSource 注入 boardRealtime。该 source 的 key 必须稳定包含 endpoint 和协议身份。

## 收到 Unimplemented

确认路由是 /kanban.framework.v1.WorkspaceService/WatchChanges，protocol_version 为 1，RpcApp 调用了 with_refresh_source。缺失装配会明确返回 Unimplemented；不要添加 SSE fallback 来掩盖。

## gRPC 流在动，任务页面没变化

检查 invalidated body 是否到达 bindBoardRealtime 的 rpc-refresh-required，再检查 Atlas session-events 是否产生原查询刷新。不要把 sequence 填进 task_events 游标，也不要把七字段卡片当完整任务页。

## 评论变化没有卡片 delta

这在旧卡片投影中可能成立。Atlas 迁移期应接 WatchChanges。它独立订阅 application 写入退出提示，覆盖卡片字段之外的变更。若仍无通知，按 G06 检查该操作是否绕过 gate。

## 当前 RPC 很忙

刷新提示是 coarse invalidation，其他看板写入和失败操作可能触发一次冗余读取。服务端 10 ms 合并、watch channel 只保留最新提示；客户端保留原查询取消与合并。应测量后用 scope-aware 提示或 query-scoped 投影降低开销，不在 handler 中写第二份规则。

## 同源/CSP 错误

127.0.0.1 与 localhost、不同端口都是不同 origin。通过原 serve 或 Vite 同源代理路由 RPC，不放宽成任意本地端口。Origin 必须进入 RpcApp 的显式允许清单。

## 断线与关闭

心跳默认 15 s，watchdog 35 s。刷新流重连先 attached 并重新读取，没有历史重放。连续失败 8 次进入 failed/circuit-open；手动重试重建连接。宿主退出必须调用 runtime.stop，关闭网络不能让一个后台 task 继续持有数据库或无限等待。

## 应用器拒绝

HEAD 或任一目标 blob 改变时停止。不要修改 manifest SHA 绕过校验。先针对新提交重新核对实际差异；不覆盖正在开发的文件。原框架目录也不要直接叠加覆盖，使用独立 worktree。

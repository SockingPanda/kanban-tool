# 旧卡片投影流的保留范围

本节描述 GetBoard/WatchBoard 的完整七字段卡片集合，仍由本包提供。Atlas 分页前端的新增接线使用 [查询刷新流](11-atlas-refresh-protocol.md)，不能把本节卡片快照强转为完整 BoardReadModel。

# 实时流协议与恢复

## cursor 的含义

`Cursor = { epoch, scope, revision }`。

epoch 在创建 Hub 时生成。scope 当前固定为 `board:<canonical board id>:cards:v1`。revision 是该 Hub 发布过的投影版本，从第一次完整发布后的 1 开始。

它不是 `task_events.id`、task lock_version 或通知计数。不同 scope、epoch 的数字不能比较。数据库替换必须销毁并重建 Hub，不能让同一 epoch 继续指向另一份数据库。

## 初次连接

```text
SnapshotBegin(revision=R, count=N)
SnapshotChunk(index=0, tasks=...)
SnapshotChunk(index=1, tasks=...)
SnapshotCommit(revision=R, count=N, chunks=K)
```

服务端读取一个不可变的 Arc 快照后分块发送，每块最多 64 张卡片，单个实际 Protobuf frame 上限 512 KiB。快照最多 50,000 张卡片，数据估算预算 16 MiB。

前端只将 chunk 写入 staging。必须检查 board、scope、epoch、revision、chunk 连续序号、重复 ID、声明数量和预算。只有完整 commit 通过，并且同步发布回调成功返回，resume cursor 才变为 R。

快照中断时丢弃 staging，仍保留上次已提交画面。空快照使用 count=0、chunks=0，同样需要 commit。

## 增量

```text
BoardDelta(base_revision=R, revision=R+1,
           upserts=[完整的新卡片], removed_ids=[移出查询的 ID])
```

一批 delta 原子应用。输入基线必须恰好是当前 cursor，不跳版本。相同的最近一批重复交付被忽略；同一版本不同内容、跨 epoch、未知必需消息或 task lock_version 回退会触发恢复。

当前 view 固定显示整个活动看板，顺序由 position、seq、id 确定。position 随卡片 upsert 更新，不需要再增加一套前端排序事实。过滤、窗口、查询排序尚未实现，见 G07。

## 断线续读

客户端每次新建 WatchBoard RPC，传入最后成功应用的 cursor，协议版本固定为 1。

| 请求情况 | 响应 |
| --- | --- |
| 没有 cursor | 发送完整快照 |
| 身份相符且历史中存在 base_revision | 按序补发 delta |
| 已是当前版本 | 等待新变化或发心跳 |
| epoch/scope 改变 | reset 后完整快照 |
| 历史过期、单批过大被丢弃 | reset 后完整快照 |
| cursor 超前 | reset 后完整快照 |

ResetRequired 后，服务端在同一条流开始快照。客户端可以保留旧画面，但必须停止沿用旧 cursor。没有拿到 commit 前不能确认新 epoch。

## 历史保留

每个 Hub 保留最多 256 个增量批次，估算总量最多 8 MiB，单批最多 256 KiB。超过单批预算时直接清空旧历史并保存新的完整投影，旧订阅者随后通过 reset + snapshot 收敛。

增量缓存只在内存里。它不保证重启后逐条重放，也不保证每一个中间编辑状态都到达 UI。source 合并期间连续出现 A→B→C，最终可能只推送 A→C。审计、运行日志等需要逐条记录的能力继续由其持久日志负责，再单独迁移 streaming RPC。

## 客户端生命周期

WebSession 使用同一个生成 gRPC-Web 客户端进行重连，没有 SSE 或 REST fallback。35000 毫秒无任何消息会取消该次 RPC。失败退避从 250 毫秒起，带 20% jitter，最高基准值 5 秒；连续无进展 8 次后停止并报告错误。

35 秒是策略参数，不是精度保证，后台标签页定时器可能被系统延后。原生 gRPC 的连接重试不替代这里的业务恢复。[R5]

新启动会提高 generation 并取消旧连接。旧连接的数据、异常和 finally 清理都不得清除新连接的 staging。心跳只证明活性，其 server_revision 不能当成 last_applied_revision。

## 消费预算与安全

宿主最多接纳 16 个活跃订阅，防止许多慢消费者持有大量旧 Arc 快照。流直接按网络需求读取共享历史，没有给每个消费者创建无界 producer 队列。

数据预算是内容估算值，不是进程 RSS 上界；BTreeMap/HashMap、字符串、transport buffer 和多个在途快照会增加实际内存。必须通过 G08 的慢消费者压力测试测量。gRPC 流控本身不表示前端已经应用消息。[R6]

未知 enum 状态和未知 oneof 分支不能静默确认 cursor。Protobuf 的未知字段保留机制不等于应用能理解新增的必要投影操作。[R7]

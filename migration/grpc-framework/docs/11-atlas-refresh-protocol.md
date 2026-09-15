# Atlas 查询刷新流协议

## 请求

WorkspaceService.WatchChanges 接收 canonical board_id 和 protocol_version=1。board_id 先经过长度/格式校验，再由原 application 在 read fence 内验证存在且未归档。没有 resume 字段。

## 帧

WorkspaceChangeFrame 包含 board_id、epoch、sequence 和一个 oneof body。invalidated(reason=ATTACHED) 必须是第一帧，sequence=1。后续 invalidated(reason=WRITE_HINT) 递增 sequence。heartbeat 保持当前 sequence，不产生刷新。

epoch 每条连接重新生成。sequence 仅用于发现当前连接内的重复、跳号和异常，不是数据库版本、审计 ID、投影恢复 cursor 或 UI 确认位置。协议没有 exactly-once 承诺。

## 服务端顺序

先 subscribe_refreshes，再在 header 前 check_board。这样读取期间的写入提示仍会留在接收器中。每次提示唤醒后用 10 ms 窗口合并，先 borrow_and_update，再做静默 check_board，读取期间出现的新提示留到下一轮。

刷新源来自 mutation gate 退出。它不提供每次写入的业务内容，失败、取消和其他看板写入也可能触发提示。读 fence 不发通知，所以不会形成订阅自我唤醒循环。

不因为心跳查询数据库，不创建独立数据库轮询器，不持久保存刷新提示。check_board 当前只验证 board，而不扫描任务集合。若 source 验证失败，gRPC stream 返回状态错误，客户端保留页面数据并显式报连接问题。

## 客户端

连接首帧必须 attached。board 或 epoch 不一致、非法 uint64、未知 body/reason、心跳推进序号、重复或跳号都会中止该次尝试。下次连接重新 attached 并刷新，不能跳过缺口后继续报告成功。

控制器用 generation 隔离 stop/retry 后的迟到回调。liveness watchdog 可以终止不响应取消的 iterator 等待，正式传输仍依靠 AbortSignal 取消 fetch。不会为每帧制造无限 Promise 队列。

attached 或 write_hint 调用 onRefresh；心跳只更新连接状态。application 绑定器发送 rpc-refresh-required，让原分页/详情查询按既有取消与缓存规则重读。它不把响应状态解释为查询已经成功应用。

## 消费上限与关闭

服务端两个流族共享 16 个订阅名额。请求和刷新帧限制为 4 KiB。watch channel 合并旧提示，不为慢消费者积累无界业务队列。前端连续失败默认最多 8 次，之后需要人工重试。手动重试和项目切换释放旧连接。

RpcApp.stop 发送 sticky shutdown signal。WorkspaceRpcMount 的调用方必须把它接到现有 host 生命周期；本包没有新增 detached 常驻 host。

## 与未来查询流的区别

该流是 typed invalidation，不是完整 live query。G07 的 task-list/map/inspector 流应包含规范化 queryKey、字段投影、结果成员、排序、total、稳定分页或窗口、snapshot fence、dataset revision。只有这些字段完整后，才能用推送数据替代权威查询，而不丢失 Atlas 的分页和筛选语义。

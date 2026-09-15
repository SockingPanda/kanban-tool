# 查询订阅

页面进入项目后，Host 数据源把目录、任务分页、详情、依赖图、运行记录和诊断等读取放进同一个
`QueryService.WatchQueries`。用户写入经 `KanbanService` 的具名方法提交；可见结果由完整查询
确认。连接失败会显示当前状态与重试入口，已有草稿和最后一次可用结果继续保留。

正式消息由 [query.proto](../../../crates/kanban-protocol/proto/kanban/v1/query.proto) 持有。
[Host 数据源](../src/adapters/host/data-source.ts) 负责 RPC 接线，
[QueryRegistry](../src/adapters/host/query-registry.ts) 持有网络连接、已提交结果及 cursor，
[observeRead](../src/application/query/observe-read.ts) 将组件读取映射到实际依赖的查询。
页面展示状态与服务查询缓存各有明确用途，页面不另建网络缓存或事件失效控制器。

## 完整结果与确认

快照和 byte splice delta 都先进入暂存区。结束帧到达后，客户端校验结果长度、SHA-256 和具名
响应类型，再一次提交数据与 cursor。delta 必须精确匹配已提交基线；基线失配或途中中断时，
暂存内容不能成为页面事实。读取失败保留依赖订阅，让恢复结果能够再次完成同一个展示映射。

Query cursor 由 `epoch`、`scope` 和 `revision` 组成，只用于查询恢复。项目动态中的数字 `id`
与稳定 `event_id` 是审计记录身份；客户端仍校验看板、任务、次序及重复身份，审计记录不驱动
其他页面重新读取。RecentEvents 直接提供最近的有界窗口。

外部 CLI、MCP 或其他页面写入后，Host 推送相关查询的完整结果变化。没有业务变化的查询不会
触发页面内容提交。显式重试与写后同步设置 `refresh`，等待服务端完成本次请求之后的读取并
返回 `Ready`；旧连接迟到的确认不能完成新刷新。表单等待这些确认后释放 pending 状态。

## 归属与生命周期

相同数据源中的相同查询共享缓存和订阅。挂载组件及详情折叠区决定查询集合；集合变化时，客户端
取消旧连接，携每个已提交 cursor 建立新的 multiplex 连接。查询结果、暂存数据和查询数量均受
客户端预算约束，无消费者后释放缓存。

看板会话只负责 canonical board 身份、操作能力和连接状态，不再把事件映射成全局刷新。
URL 持有筛选、排序、分页和 Inspector 选择，组件展示映射保持这些状态以及焦点、滚动和草稿。
切换项目或卸载时，身份、请求代次与 AbortSignal 一起阻止旧结果覆盖新页面。

`pagehide` 主动暂停连接并丢弃未完成结果；`pageshow` 恢复当前仍被消费的查询。这个生命周期
也覆盖浏览器保留旧 document 的情况，不依赖 React 卸载一定发生。在线恢复及用户重试继续
使用 QueryService；健康页面的稳定诊断同样使用查询，启动阶段的 `/health` 探测保持独立。

## 验证边界

单元测试验证原子提交、精确 delta 基线、迟到确认、预算、共享查询、故障恢复及最后卸载释放。
浏览器产品 fixture 使用正式 binary gRPC-Web 帧和完整 QueryResult，检查布局、URL、筛选分页、
草稿、claim、焦点和滚动。真实 Host 验收另行检查外部写入、途中中断、断流重试、生命周期恢复
以及稳定订阅期间没有 unary 跟读。两类证据分别记录，配置声明不代表真实浏览器已经通过。

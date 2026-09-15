# 持久 SSE 与查询同步

Host adapter 使用生成的事件 validator 解析 `/api/v1/stream/events`，application 按项目与事件
范围更新查询。HTTP、SSE envelope 和 payload 的事实源是 `kanban-protocol` 的
[事件定义](../../../crates/kanban-protocol/src/event_payload.rs) 与
[schema](../../../crates/kanban-protocol/docs/schema.md)，前端不维护第二份事件 catalog。

## 连接与恢复

- `id` 是有序游标，`event_id` 是事件的稳定身份；客户端检查顺序、去重和相同身份的内容冲突。
- 初次连接使用 `after`，重连携带已确认游标。服务先补齐 catch-up，再接入 live 流。
- 连接失败进入恢复，期间通过有界 polling 补读。确认边界完成后再回到实时状态。
- 未知事件、序列缺口、无效 envelope 和游标异常触发保守刷新；缓存不能反向写业务事实。
- 补读页依赖上一页的游标，事件应用依赖前一条确认结果，必须顺序执行。

## 查询隔离

[`application/sync`](../src/application/sync/contracts.ts) 持有同步控制器接口和查询目标。
[`invalidation.ts`](../src/application/sync/invalidation.ts) 把已验证事件映射到具体查询，
[`board-sync-sink.ts`](../src/application/sync/board-sync-sink.ts) 负责有界读取与刷新编排。

查询身份包含 runtime、项目、视图及查询条件。项目切换释放旧订阅，异步结果同时核对身份、请求
代次与 AbortSignal，迟到响应不能覆盖当前项目。任务详情另外校验任务 ID、字段版本和变更结果；
失效后的表单仍保留草稿，成功状态以 canonical reload 为准。

- 任务、状态、依赖、步骤和标签变更只更新对应项目和任务的列表、详情、依赖图及派生状态。
- Run、日志和动态按其真实任务、运行与游标范围刷新。
- 普通标签仍有自己的任务投影；已移除页面不再注册专属缓存或发起读取。
- 项目动态可展示服务仍产生的事件，展示事件不意味着重新启用某个功能页面。
- 维护操作只有服务确认后才刷新相关查询，刷新失败单独呈现并允许重试。

## 界面行为与验证

离线时保留当前查询最后一次可用数据并显示连接状态。没有可用数据时显示连接错误与重试入口。
恢复后按已确认游标补齐；关闭详情折叠区会中止该区正在进行的读取，重新展开时按失效情况读取。

同步控制器测试覆盖顺序、去重、缺口、重连、polling、取消和项目隔离。浏览器验收还需要证明页面
确实更新、失败草稿保留、快速切换项目不会串数据，以及刷新与前进、后退能恢复 URL 中的查询状态。
测试结果保存在任务和构建证据中，不将配置声明当作实际通过结果。

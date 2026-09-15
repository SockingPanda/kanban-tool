# 唯一 Host 的本地 gRPC

## 决定

Web 与 Desktop 的业务通信采用 Connect-ES `createGrpcWebTransport()` 和 binary Protobuf，
由 `tonic-web` 接入现有 Host。CLI 与 MCP 内部通过共享异步 channel 使用原生 gRPC；MCP 对外
保持 stdio/JSON-RPC。业务协议收敛为命名 RPC，HTTP 保留静态 Web、runtime、manifest 与健康探测。

所有协议共用一个 loopback listener。原生 HTTP/2 的 `:authority` 与 HTTP `Host` 一致校验；
同源静态 Web 保持 strict CSP。开发代理也只连接明确的 loopback Host。

## 所有权

正式 Protobuf 契约归 `kanban-protocol`，Host 装配和业务转换归 `kanban-server`，原生 channel 归
`kanban-client`，纯快照、delta 与有界历史算法归 `kanban-live-core`。只有 `kanban-service`
持有 Turso；RPC 不建立第二条 canonical mutation path。

## 一致性和资源

请求保留 actor、CAS、幂等键、claim token，以及未提供、清空和具体值的区别。动态 JSON 仅用于
业务允许的动态字段，不能替代完整请求或响应。64 位数值在 wire 上保持精度。

写入提示只唤醒一致读取，不承诺事务成功。查询快照完整提交后才能推进 cursor；delta 必须核对
查询身份和基线。连接取消、查询切换、订阅回收和 Host 退出共同约束资源生命周期；有限历史与
编码字节预算约束慢消费者和大快照。

Host 直接持有 HTTP/1 与 HTTP/2 连接任务。正常退出先通知持续响应并等待最多 5 秒；超过等待
预算时中断剩余连接并回收响应资源。强制退出或 Host future 取消直接中断所持连接，回收不能
依赖客户端继续读取或主动断开。连接库内部的后台任务不能成为独立于 Host 的资源 owner。

## 取舍

完整查询在单个 gRPC-Web 连接中复用，避免浏览器 HTTP/1 同源连接额度限制。服务端共享相同规范化
查询的读取和有界历史。delta 采用完整 typed Protobuf 投影的字节 splice：排序、过滤、分页补位和
跨任务依赖继续由 application query 决定，adapter 不复制第二套成员更新规则。接收端必须在完整
编码与摘要验证后提交，不能将单个 chunk 映射为 UI 业务状态。

浏览器无法直接使用原生 HTTP/2 gRPC，gRPC-Web 提供相同 Protobuf 契约及服务端流。浏览器上传
使用其支持的请求形式，保持 service 的附件大小上限。生成和互通检查增加构建步骤，由根
`justfile` 与 `xtask` 统一管理。该决定不引入远程访问、多租户或额外数据库。

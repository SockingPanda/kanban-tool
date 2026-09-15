# kanban-rpc-host

将实时投影和查询失效 source 暴露为原生 gRPC 与 gRPC-Web service。标准 Tower service 可由 `kanban-server` 装配，生产监听地址、Origin 策略和关闭流程由唯一 host 持有。

`serve` 供隔离示例及互通测试使用。业务调用通过注入的 application adapter；流使用有界订阅名额和恢复历史，查询失效消息只提示重新读取，不表示事务已成功。

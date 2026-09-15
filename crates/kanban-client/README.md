# kanban-client

`kanban-client` 是面向 loopback `kanban serve` 的原生异步 gRPC client。所有业务操作经过共享
application service；client 负责连接、actor、selector、typed 请求与响应以及稳定错误映射。

先启动 `kanban serve`，在调用者自己的 Tokio runtime 中使用 client：

```rust,no_run
use kanban_client::{KanbanClient, DEFAULT_SERVER_URL};

# async fn example() -> Result<(), kanban_client::ClientError> {
let client = KanbanClient::new(DEFAULT_SERVER_URL, "开发者")?;
let board = client.get_board("default").await?;
println!("{}", board.name);
# Ok(())
# }
```

`new` 同步校验配置，不需要 runtime，也不建立连接。第一个异步操作建立原生 HTTP/2 Channel；
后续操作和 `clone` 共享该 Channel，由 tonic 处理连接复用和重连。client 只接受 `http` loopback
地址，默认是 `http://127.0.0.1:8721`。连接预算为 2 秒，单次 unary 调用总预算为 30 秒；
丢弃调用 Future 会取消仍在等待的请求。

每个业务入口直接调用 `kanban-protocol` 生成的具名方法，并通过正式 codec 转换 path、query、
input 和返回 DTO。actor 使用 `x-kb-actor-bin` metadata，支持中文与其他 Unicode 文本。服务端的
标准 RPC error detail 保留 `ClientError::code()`、业务错误和兼容的 `Api.status` 数值。请求和响应的
消息预算使用 protocol 的统一上限，附件内容上限由 application service 校验。

label proposal 查询区分 task scope 与 board scope：调用 `list_task_label_proposals` 或
`list_board_label_proposals`，后者可传 `status` 过滤。ontology 的既有 `Value` 外壳在 client 边界转换为
具名 typed DTO；`ontology_data` 是同步的本地 DTO 转换。

## 事件订阅

`open_event_stream(...).await` 先建立正式 `WatchChanges` 订阅，再通过 `ListEvents` 读取有界事件页。
`next_item().await` 返回领域事件或不推进业务 cursor 的 heartbeat；两个输入 cursor 取较新者，
board 和可选 task 过滤保持一致。断线重连由调用者保存最后交付的领域事件 ID 后重新打开流。

订阅直接由 `EventStream` 持有，丢弃即可释放服务端订阅。client 不创建后台读取任务；取消一次
`next_item` 等待后，可以继续读取同一个流。连接关闭返回 `ClientError::StreamClosed`。

正式 wire contract 由 [`kanban-protocol`](../kanban-protocol/README.md) 持有，Host 生命周期见
[`kanban-server`](../kanban-server/README.md)，本机传输取舍见
[本机 gRPC ADR](../../docs/adr/0007_local_grpc.md)。

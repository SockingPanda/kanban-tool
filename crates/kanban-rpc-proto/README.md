# kanban-rpc-proto

从根 `proto/` 下的框架 Protobuf 定义生成 Rust 消息、client 和 service 接口。`build.rs` 使用锁定的 tonic/prost 工具链，浏览器生成入口是根 `just grpc-contracts-generate`。

消息定义只负责跨 adapter 的值，不持有数据库或执行状态规则。完整业务契约由 `kanban-protocol` 归口维护。

# kanban-protocol

`kanban-protocol` 是跨 adapter 的机器可读 wire contract owner。它提供 DTO、error envelope、event
payload、正式 RPC catalog、CLI/MCP surface catalog 和 schema registry。

正式本机 RPC 契约位于 [`proto/kanban/v1/`](proto/kanban/v1/)。`rpc::v1` 导出生成的 tonic
client/server 和 Protobuf 消息；`decode_parts`、`from_parts` 与响应的 `TryFrom` 在原 DTO 和
Protobuf 之间逐字段转换。`rpc::MAX_MESSAGE_BYTES` 为附件内容及 envelope 预留空间，附件实际
内容上限仍由 service 验证。

`just grpc-contracts-generate` 同时生成正式 `.proto`、Rust codec、操作映射和浏览器客户端；
`just grpc-contracts-check` 检查这些产物是否与 source 一致。字段编号由 owner 的 ledger 保留，
删除字段后不能将原编号用于另一字段。

它不拥有数据库 row、service input、HTTP handler 或 CLI command implementation。server、client、CLI、
MCP 和 Desktop 必须以这里的 typed contract 互操作；业务状态、事务和权限仍由 `kanban-core` 与
`kanban-service` 负责。

`rpc::catalog::methods()` 是当前可调用的业务与查询 RPC 清单，并与正式 Protobuf descriptor 核对。
DTO 的 path/query/body 名称用于保持输入、JSON 输出和 schema 的字段语义；它们不注册 HTTP 业务路由。

精确 schema artifact、fixture 和生成约束见 [`docs/schema.md`](docs/schema.md)。精确 RPC service/method、
Clap flags 和 MCP tool 名称由代码 catalog、`kanban --help` 与对应 adapter 持有，不在本 README 维护
穷举清单。

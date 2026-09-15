# Schema 与 wire 契约

本页解释 `kanban-protocol` 的机器契约边界。DTO、schema registry、endpoint/surface catalog 和生成
artifact 是精确事实源；本页只保留语义和维护规则。

## 源码与 artifact

- Rust DTO、event payload、error envelope 和 catalog 在 `crates/kanban-protocol/src/`。
- `schemas/` 中的 JSON Schema 与 fixture 由 `xtask` 生成或校验，不能手工维护第二份 inventory。
- schema 使用 JSON Schema Draft 2020-12，root 必须自包含，局部引用只能指向 `#/$defs/...`。
- `/app/runtime.json` 的 `WebRuntimeConfig` 由 `runtime_catalog` 以 `Config` surface 声明；它是
  host metadata，不是 `/api/v1` endpoint，因此不会出现在 HTTP endpoint catalog 中。

## 契约边界

正式 RPC 使用 binary Protobuf。请求中的 presence 区分省略、显式清空和具体值；可选集合使用
wrapper 保留空集合，PATCH nullable 字段使用 oneof 保留清空语义。`int64`、`uint64` 及动态
JSON 的 signed/unsigned integer 分支保持整数精度。业务错误通过标准 `google.rpc.Status`
中的 `kanban.v1.ErrorDetail` 传递，原生 tonic 和浏览器 Connect 客户端共享稳定业务错误码。

精确 RPC 清单、原 DTO parts 和字段映射分别位于 `proto/rpc-operations.json`、
`proto/rpc-methods.json`，字段编号保存在 `proto/rpc-field-numbers.json`。生成实现由 `xtask`
持有，protocol 的 build script 只编译 owner 下的 Protobuf source。

schema 描述序列化形状、字段可选性和 transport envelope；状态 transition、claim token、board
isolation、依赖环、idempotency 和事务原子性由 service/server/client 测试与领域规则证明，不能从
JSON Schema 推断。

HTTP 精确 method/path 来自 protocol/server catalog，CLI 精确 flag 来自 Clap，MCP 精确 tool 来自
MCP catalog。文档示例只能使用这些当前 source 可核对的字段，不复制完整清单。

label proposal 有 task-scoped 与 board-wide 两种独立的 typed contract；board-wide contract 使用
`ListBoardLabelProposalsQuery` 的可选 `status` query，响应为 `ListBoardLabelProposalsResponse`。两者都经
`kanban-service`，不能在 adapter 中拼接第二套查询或直接读取 Turso row。

## 完整查询流

`proto/kanban/v1/query.proto` 的 `QueryDefinition` 与 `QueryResult` 用对应的具名 oneof 复用完整
业务请求和响应。查询身份包含 Host runtime、canonical board、规范化过滤/排序/分页与投影版本；
订阅 ID、恢复 cursor 和显式 `refresh` 不进入身份。一个 `WatchQueries` 连接复用多个活跃查询。

`QueryBegin`、`QueryChunk`、`QueryEnd` 传送完整 snapshot 或基于旧编码的精确 splice delta。
delta 描述偏移、删除字节数和插入字节；最终仍还原成完整 typed `QueryResult`，包含 total、窗口
成员顺序和所有业务字段。客户端检查基线、分块顺序、编码大小、SHA-256、结果类型及作用域后，
一次发布投影与 cursor。中断快照不推进 cursor；历史淘汰或无法匹配基线时恢复完整快照。
Rust 的版本、单帧、单投影和查询数量预算由 `rpc::query` 持有。

显式重试或写后确认可设置 `refresh`。Host 在收到该请求后重新执行受 service read fence 保护的
权威查询，先传送有变化的完整投影，再发送 `QueryReady`；结果相同时只确认，不推进 revision。
Ready 证明这次读取完成，不代替命令结果、审计事件或事务提交证明。

统计与图查询的 `generated_at` 保留最近一次发布样本的真实生成时间。仅采样时钟前进、业务内容
未变化时不发布新投影；发生业务变化后，发布那次读取的完整结果及时间字段。需要随时间变化的
统计、运行日志只在有活跃订阅时重新检查，不对所有查询增加轮询。

## 变更流程

新增或修改 contract 时，在 protocol 中更新 DTO/root/catalog，维护 valid/invalid fixture，并贯通真实
producer/consumer。仅 protocol/schema artifact 变化才运行 `just schema-check`；普通 prose 使用
`just docs-check` 与 `just diff-check`。

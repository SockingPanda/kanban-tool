# Schema 与 wire 契约

本页解释 `kanban-protocol` 的机器契约边界。DTO、schema registry、endpoint/surface catalog 和生成
artifact 是精确事实源；本页只保留语义和维护规则。

## 源码与 artifact

- Rust DTO、event payload、error envelope 和 catalog 在 `crates/kanban-protocol/src/`。
- `schemas/` 中的 JSON Schema 与 fixture 由 `xtask` 生成或校验，不能手工维护第二份 inventory。
- schema 使用 JSON Schema Draft 2020-12，root 必须自包含，局部引用只能指向 `#/$defs/...`。

## 契约边界

schema 描述序列化形状、字段可选性和 transport envelope；状态 transition、claim token、board
isolation、依赖环、idempotency 和事务原子性由 service/server/client 测试与领域规则证明，不能从
JSON Schema 推断。

HTTP 精确 method/path 来自 protocol/server catalog，CLI 精确 flag 来自 Clap，MCP 精确 tool 来自
MCP catalog。文档示例只能使用这些当前 source 可核对的字段，不复制完整清单。

label proposal 有 task-scoped 与 board-wide 两种独立的 typed contract；board-wide contract 使用
`ListBoardLabelProposalsQuery` 的可选 `status` query，响应为 `ListBoardLabelProposalsResponse`。两者都经
`kanban-service`，不能在 adapter 中拼接第二套查询或直接读取 Turso row。

## 变更流程

新增或修改 contract 时，在 protocol 中更新 DTO/root/catalog，维护 valid/invalid fixture，并贯通真实
producer/consumer。仅 protocol/schema artifact 变化才运行 `just schema-check`；普通 prose 使用
`just docs-check` 与 `just diff-check`。

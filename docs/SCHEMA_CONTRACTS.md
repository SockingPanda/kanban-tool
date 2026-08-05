# JSON Schema 与机器契约

## 1. 适用范围与权威来源

本文件只描述 wire contract、schema artifact、surface catalog 以及它们的校验方式。
它不定义业务状态机，也不把 schema 校验当作事务或权限校验。

当前单 Host 产品路径是：

```text
CLI / MCP / Desktop
        ↓ typed localhost client
kanban serve (HTTP)
        ↓
ApplicationService + State Machine
        ↓
kanban-store-turso → canonical Turso database
```

`kanban-protocol` 是公开 DTO、事件 payload、错误 envelope、operation inventory 和
transport descriptor 的 Rust 权威来源；只有根目录私有 `xtask` 生成和校验 JSON Schema
artifact。`kanban-server`、`kanban-client`、CLI、MCP 和 Desktop 是运行时 producer/consumer，
不能各自复制一套 DTO 或业务错误解释。

语义分工如下：

- DTO/schema：字段、类型、必填/可选、未知字段策略和基础值域。
- `endpoint_catalog()` 与 `surface_operation_catalog()`：机器契约的 source inventory。
  当前仍含尚未迁入单 Host 路径的历史条目；这些条目是完整功能 parity 义务。HTTP、CLI、
  MCP 身份必须同时由真实 router/adapter 证明，catalog 本身不创建 route，也不能单独证明
  adoption。
- ApplicationService、`kanban-core` 状态机和 `kanban-store-turso`：事务、CAS、board
  isolation、依赖和 run/event 一致性。
- `docs/API_SPEC.md`、`docs/CLI_SPEC.md`：用户可见的 operation、HTTP/退出码和输出行为。

`schemas/` 中的文件是生成/提交产物，不是新的事实来源。若 source inventory、真实
server route、adapter 或测试与 committed artifact 冲突，以当前 source 和运行时为准，
先修正 contract source，再运行 `just schema-generate`；不得手工改 generated JSON 来掩盖漂移。

## 2. 契约状态与当前迁移边界

`operation_inventory()` 的每一项必须使用以下状态：

| 状态 | 含义 | 证据要求 |
| --- | --- | --- |
| `planned` | 已确定精确边界，尚未生成 root | 不得填写伪 schema、fixture 或采用证据 |
| `generated` | DTO、root 和 fixture 已生成 | 不得声称运行时已经采用 |
| `adopted` | 真实 producer/consumer 使用同一 DTO/contract | fixture、producer witness、consumer witness 和精确测试 |
| `excluded` | 明确不是稳定 JSON contract | 非空排除理由；不得同时声称有 schema/fixture |

`adoption` witness 必须同时记录 `operation`、`contract_id`、`surface`、`direction`、
`package`、`test_target` 和 `exact_test`。request/input 的 producer 必须由真实 DTO
序列化；consumer 必须从 committed fixture 进入真实 router/handler。response/output 的
producer 必须来自真实 adapter 响应路径。producer 和 consumer 不得共用一个高层 exercise
helper。

当前 committed `schemas/json-schema/draft-2020-12/operations.json` 与
`surface-operations.json` 仍可见历史的 SQLite、projection、label、signal、search、
graph/vector 和维护命令条目。它们记录必须恢复的完整产品能力，但不能仅凭 catalog 解释为
当前单 Host runtime 已经采用。尚未接入真实 adapter 的条目必须保持 `planned` 或
`generated`，并在对应纵向切片完成 DTO、fixture、route/tool/command 与 exact witness 后
才能改为 `adopted`。除非某条记录只是重复 wire 表达且业务能力已有明确的新 owner，不得把
它改成 `excluded` 来缩小功能范围。在所有 parity 项有真实证据前，不能声称
`just schema-audit-closed` 或整个 schema catalog 已完全收口。

SQLite backend、`kanban-sqlite`/`kanban-local`、Tantivy/LanceDB/Oxigraph projection 以及
helper subprocess 不属于目标 runtime 架构；旧源码只作为迁移证据，不得重新接入 active
workspace。它们承载的 labels、signals、ontology、search、graph、vector、context 与维护
语义必须迁入 `kanban-core`、`kanban-service`、`kanban-protocol` 及各 adapter 后，旧目录
才允许删除。

## 3. 当前 active operation 的精确 contract

下面列出本轮已接入真实 HTTP、typed client 与 adapter 的 run/event contract。每个路径
使用独立的 path/query/success root；共享 headers 仍由各 endpoint 的 descriptor 明确引用。

| Operation | HTTP path | contract id |
| --- | --- | --- |
| stats | `GET /api/v1/stats` | `api.get-stats.query`, `api.get-stats.response` |
| run list | `GET /api/v1/tasks/:task_id/runs` | `api.list-runs.path`, `api.list-runs.response` |
| run show | `GET /api/v1/runs/:run_id` | `api.get-run.path`, `api.get-run.response` |
| run log | `GET /api/v1/runs/:run_id/log` | `api.get-run-log.path`, `api.get-run-log.response` |
| event list | `GET /api/v1/events` | `api.list-events.query`, `api.list-events.response` |

对应的 schema root 和正例 fixture 位于：

```text
schemas/json-schema/draft-2020-12/
schemas/fixtures/api/list-runs-*.v1.valid.json
schemas/fixtures/api/get-run-*.v1.valid.json
schemas/fixtures/api/get-run-log-*.v1.valid.json
schemas/fixtures/api/list-events-*.v1.valid.json
```

CLI 的当前读取面是 `events`、`runs`、`run show`、`run logs`，其 output contract 分别为
`cli.events.output`、`cli.runs.output`、`cli.run-show.output` 和 `cli.run-logs.output`。
`run logs` 不再接受 `--tail-bytes`；固定的 `ApiRunLog` 返回 `run_id`、完整或截断的
`content` 和 `truncated`，CLI JSON 保留现有 nullable `tail_bytes` 字段但新路径返回
`null`。MCP 使用 `event_list`、`run_list`、`run_show`、`run_log`，只调用
`kanban-client`；Desktop 复用同一 typed HTTP client，不打开数据库。

`ListEventsResponse` 的 `meta.next_after` 是游标；已知 event kind 的 payload 必须通过
对应 typed payload 校验，未知 kind 保留任意 JSON value（包括数组、标量和嵌套对象），
不能静默丢字段。run/event 是只读 adapter surface；run 的创建和更新仍由 task claim、
heartbeat、release、review、done、block 的共享 ApplicationService mutation path 完成。

`cli.board-use.output` 与 `cli.board-current.output` 是本地配置 shell contract：它们返回
board selector、项目 `.kb/config.toml` 路径、配置来源及 `created`/`updated` 标记，不返回
`ApiBoard` domain record。两个命令不访问 host；若需要校验或创建 canonical board，必须
通过 localhost HTTP operation 完成。

## 4. Wire 规则

- 方言固定为 JSON Schema Draft 2020-12；request/input 使用
  `SchemaSettings::for_deserialize()`，response/output 使用 `for_serialize()`。
- root ID 使用 `urn:kanban-tool:schema:<surface>:<semantic-name>:v1`；root 版本与 crate
  或 API 版本独立，破坏性 wire change 必须提升 root 版本并删除被替代 artifact，不保留
  双轨输出。
- schema 必须自包含，只允许本地 `#/$defs/...` `$ref`；产物不得包含时间戳、绝对路径或
  网络 resolver。
- `#[serde(deny_unknown_fields)]` 与 `required-nullable` 必须反映真实 DTO 行为。显式
  `null` 与省略字段不是同一语义；不能在 schema 中把 required-nullable 改成 optional。
- 基础 JSON 校验不覆盖跨字段业务规则。状态转换、claim token、依赖 cycle、board
  isolation、幂等 key、事务原子性和错误 code 必须由 ApplicationService/store 测试证明。
- 不为 schema tooling 引入 HTTP、文件 resolver、TLS、OpenAPI 或生产 runtime validator。

## 5. 依赖边界与单 Host gate

active workspace 只保留 `kanban-core`、`kanban-application`、`kanban-protocol`、
根目录私有 `xtask`、`kanban-store-turso`、`kanban-client`、`kanban-cli`、`kanban-mcp`、
`kanban-server` 和 Desktop Tauri host。数据库依赖方向固定为：

```text
kanban-server → kanban-store-turso → turso
kanban-cli / kanban-mcp / Desktop → kanban-client → localhost HTTP
```

CLI、MCP、Desktop、test fixture 和 test-support 不得依赖 `kanban-store-turso`、
`kanban-sqlite`、`kanban-local`、`rusqlite` 或任何数据库-owning path；只有
`kanban-server` 可以初始化、打开和关闭 Turso。该限制覆盖 normal、dev、build、target-
specific dependency 和测试 fixture，不只检查源码 import。

`scripts/check-single-host-dependencies.py` 是单 Host manifest gate；它拒绝 legacy package
进入 workspace、projection helper 进入 active workspace，以及任意 adapter 的 forbidden
dependency alias。schema tooling 另有独立边界：根目录私有 `xtask` 只能作为离线生成/
校验工具，不能进入产品 runtime graph。`kanban-mcp` 会启用 `kanban-protocol/schema`
来生成 RMCP tool input schema；这不授权它依赖 `xtask`、`jsonschema` runtime 或数据库
crate。

## 6. Artifact 目录与生成规则

```text
schemas/
  fixtures/
    api/ cli/ jsonl/ metadata/ sse/
  json-schema/draft-2020-12/
    operations.json
    surface-operations.json
    manifest.json
    api/ cli/ jsonl/ metadata/ sse/
```

`operations.json` 记录 semantic contract；`surface-operations.json` 记录精确传输操作；
`manifest.json` 记录 root、fixture 和 hash。连续生成必须 byte-identical。公开 Markdown
中的完整 JSON 示例如需进入 schema docs gate，必须由 `schema-doc` marker 绑定到
manifest-owned fixture；片段、伪值或解释性 payload 使用 `schema-doc-ignore` 并给出理由。

transport descriptor 由 `endpoint_catalog()` 生成，至少区分：

- `Path`、`Query`、`Headers`、`Body`、`Success`、`Error` 和 `Sse` location；
- `RequiredOne`、`OptionalOne`、`RepeatedOrdered` cardinality；
- endpoint-specific exact contract 与可复用的 `SharedComponent`。

route、CLI leaf command 或 MCP tool 新增/删除/重命名时，必须同步 source inventory、
descriptor、fixture、adoption witness 和 generated artifact；不能用 family/wildcard 记录
来绕过审计。

## 7. Schema recipes 与验证顺序

根目录 `xtask/` 是 `publish = false` 的私有 workspace leaf。其命令面如下：

```text
xtask schema generate
xtask schema check
xtask schema audit
xtask schema witnesses
xtask deps check
xtask agents check
```

`schema generate` 生成并随后校验 committed schema tree；`schema check` 只读检查重新生成
结果、fixture、manifest 和 hash 漂移；`schema audit` 校验 operation/surface catalog，可
附加 `--require-closed` 要求没有未闭合项；`schema witnesses` 在 audit 后输出 `adopted`
operation inventory 的 JSON；`deps check` 运行 schema dependency policy、cargo tree 隔离
和 single-host 依赖检查；`agents check` 检查根 `AGENTS.md`、技能包结构以及 active
recipe/package 映射。

开发者入口仍由 `justfile` 提供：`just schema-generate`、`just schema-check` 和
`just schema-audit-closed` 使用 `xtask`，通用的 `just schema ...`、`just deps ...` 和
`just agents ...` 透传对应命令。`xtask` 不反向调用 `just`；它只直接调用必要的脚本和
自身的 schema/catalog 逻辑。

当前 `justfile` 仍提供以下 schema 入口：

```text
just schema-generate
just schema-check
just schema-docs
just schema-fmt
just schema-tool
just schema-dependency-isolation-self-test
just schema-dependency-isolation
just schema-adoption-witness-self-test
just schema-adoption-witness
just schema-surface-audit
just schema-contract
just schema-audit-closed
```

- `schema-generate` 生成 source inventory 对应的 committed tree；`schema-check` 只读检查
  fresh generation、fixture、manifest 和 hash 漂移。
- `schema-tool` 保留现有 recipe 名称，但实际对私有 `xtask` 执行 check、test 和 clippy，
  不再调用已迁移的 `kanban-schema-tool`。
- `schema-docs` 检查 spec bundle、marker、JSON fence 与 fixture 映射；它不把 prose 示例
  变成新的 contract。
- `schema-surface-audit` 的目标是对照真实 server route 与 CLI Clap leaf command；当前
  recipe 的历史 filter 仍待迁移，不能把“0 tests passed”当作 single-host surface 证明。
  MCP inventory 以实际 tool router 测试为准。
- `schema-adoption-witness` 先按 `(package, test_target)` 分组列出并执行 exact witness，
  再报告 producer/consumer；缺失、重复、ignored 或未执行均失败。
- `just schema-contract` 仍是现有 schema-contract composite gate，继续组合
  `just schema-dependency-isolation`、`just schema-fmt`、`just feature-p kanban-protocol schema`、
  `just schema-tool`、`just schema-check`、`just schema-docs`、`just schema-surface-audit` 和
  `just schema-adoption-witness`；它没有被 `xtask` 替代，也不会被 `xtask` 反向调用。
- `schema-dependency-isolation`、`schema-surface-audit`、`schema-adoption-witness` 和
  `schema-contract` 仍包含旧 catalog/registry closure 的收口责任；在 legacy source 与
  artifact 被重新分类并由真实单 Host surface 接管前，它们不是完整功能完成证明，也不得
  为使其通过而伪造 witness 或重新接入 legacy/projection crate。
- `schema-audit-closed` 仅用于 source inventory 已清理且没有 `planned`/`generated`/未闭合
  endpoint obligation 的阶段性收口。本分支当前仍有 legacy artifact，不能据此宣称 closed。

所有会写 Cargo target 的命令必须经仓库 build lock 和上述 recipe；不要为 schema 文档任务
单独设置 target/cache，也不要运行与当前 contract 无关的 full/release/projection gate。

## 8. 新 operation 的最小闭环

新增 operation 时按以下顺序完成一个纵向 slice：

1. 在 `kanban-protocol` 定义精确 DTO、schema root、inventory 和 endpoint/surface descriptor。
2. 添加 valid/invalid fixture，并为真实 producer 与 consumer 各提供独立 exact witness。
3. 在 `kanban-store-turso`、ApplicationService、server、`kanban-client` 和所需 adapter
   中接通同一 operation；adapter 不得直连 store。
4. 运行受影响 package tests、contract tests、`just schema-check`、
   `just schema-surface-audit`、`just schema-adoption-witness` 和 `just diff-check`。
5. 若 operation 实际被取消，状态改为 `excluded` 并写明理由；不得留下看似 adopted 的
   fixture 或 route。

这套闭环适用于完整单 Host 产品路径。labels、signals、ontology、search、graph、vector、
context、projection、运维、旧 SQLite importer 与 Desktop 历史视图按 parity ledger
分片恢复；每一片都必须同步 contract catalog 与真实 surface。旧 wire/CLI 兼容不是目标，
但不得借此删除业务能力或数据迁移语义。

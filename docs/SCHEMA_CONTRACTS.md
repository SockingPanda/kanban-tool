# JSON Schema 与机器契约

本文件描述 `kanban-protocol` 的 wire DTO、schema artifact、endpoint/surface catalog、fixture 和 adoption 证据边界。它不定义业务状态机、事务或权限；这些规则属于 `kanban-core`/`kanban-service`。

当前单 Host 路径为：

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP / SSE
kanban serve（唯一 host）
        ↓
kanban-service + kanban-core
        ↓
canonical Turso
```

## 1. 权威来源和状态

- `kanban-protocol`：DTO、event payload、error envelope、`endpoint_catalog()`、`surface_operation_catalog()` 和 schema registry 的唯一 Rust source。
- `kanban-server`：真实 HTTP/SSE producer/consumer；`kanban-client`：typed transport consumer；CLI/MCP/Desktop：入口 adapter。
- `schemas/`：由 `xtask`/脚本生成的提交产物，不手工编辑。
- `docs/API_SPEC.md`、`docs/CLI_SPEC.md`：用户可见 wire/output 行为；`docs/migration/turso-full-feature-parity.md`：跨 surface 事实和 gate ledger。

| `MigrationState` | 含义 | 最小证据 |
| --- | --- | --- |
| `planned` | 已定义边界但尚未生成 root | 精确 owner/operation |
| `generated` | root/fixture 已生成 | committed schema/fixture |
| `adopted` | 真实 producer/consumer 使用同一 contract | fixture、exact witness、route/adapter test |
| `excluded` | 不是 JSON document contract | 非空排除理由；不能同时声称有 schema |

`adopted` 是协议/表面 contract 状态，不等于整个 runtime、Desktop、schema-audit 或 full package gate 已运行。真实 route、CLI leaf、MCP tool 和 Desktop API 必须与 catalog 一一核对；catalog 本身不创建 route。

## 2. Active catalog 范围

当前 catalog 覆盖：

- API：health、boards（含 columns）、tasks/lifecycle（含 specify）、steps、comments、attachments、dependencies、entities（含 upsert）、graph（含 neighborhood/map）、search/context（含 index rebuild/sync）、labels/ontology、signals、runs/events、maintenance 和 vector；
- CLI：board/config/task/label/comment/context/attachment/dep/entity/graph/search/index/signal/vector、run/event、host-admin、hook/init/completion；原始 bytes 下载、completion、hook stdin/stdout、serve daemon 等非 JSON 输出保持 `excluded`。canonical leaf 以 Clap `get_name()` 为准，visible alias 不重复登记；
- MCP：`kanban-protocol::MCP_OPERATION_CATALOG` 的 machine-readable catalog，共 103 个
  tool，覆盖全部 102 个非 host-admin HTTP operation；`MCP_HOST_ADMIN_OPERATION_IDS` 明确
  禁止 12 个 host-admin operation。search/graph/vector 与 label atom-index 的 domain
  `rebuild`/`sync` 仍属于允许的 MCP surface；
- JSONL/metadata/config：portable import/export、decision/signal/ontology metadata、project config。

旧 backend/helper/projection 名称若仍出现在 historical catalog、migration fixture 或 archive 文档中，只表示 baseline/data lineage；active owner 已是 `kanban-service` 的 Turso FTS/vector32/BFS/worker，不能据此重新引入第二 backend。

## 3. Wire rules

1. 方言固定 JSON Schema Draft 2020-12；request/input 使用 deserialize settings，response/output 使用 serialize settings。
2. root ID 使用 `urn:kanban-tool:schema:<surface>:<semantic-name>:v1`；破坏性 wire change 提升 root version，不保留双轨输出。
3. schema 必须自包含，只允许本地 `#/$defs/...` `$ref`，不得有时间戳、绝对路径或网络 resolver。
4. `deny_unknown_fields`、required-nullable、显式 `null` 与省略字段必须反映真实 DTO。schema validation 不替代跨字段业务 guard。
5. path/query/headers/body/success/sse、cardinality（`RequiredOne`、`OptionalOne`、`RepeatedOrdered`）由 endpoint descriptor 精确记录；不使用 wildcard/family 逃避审计。
6. status transition、claim token、dependency cycle、board isolation、idempotency、transaction atomicity 和 provider degraded 由 service/server/client tests 证明，而不是由 JSON Schema 推断。

## 4. Artifacts 和 adoption witness

```text
schemas/
  fixtures/{api,cli,jsonl,metadata,sse}/
  json-schema/draft-2020-12/
    operations.json
    surface-operations.json
    manifest.json
    api/ cli/ jsonl/ metadata/ sse/
```

`operations.json` 是 semantic contract inventory；`surface-operations.json` 是精确 transport/adapter inventory；`manifest.json` 绑定 root、fixture 和 digest。连续生成必须 byte-identical。

每条 adoption witness 记录 `operation`、`contract_id`、`surface`、`direction`、`package`、`test_target`、`exact_test`。producer 必须来自真实 DTO 序列化；consumer 必须从 committed fixture 进入真实 route/handler/adapter；不能用一个高层 exercise helper 同时代替两侧证据。

当前代码已有 protocol schema tests、server fixture adoption tests、CLI contract tests、MCP inventory test、Desktop contract tests 和 service capability/lifecycle tests。尚未执行的 `schema-surface-audit`、`schema-adoption-witness`、`schema-audit-closed` 或 full package gate 必须按实际输出更新 ledger，不能从 artifact 生成成功推断为 green。

## 5. 依赖与单 Host gate

active runtime 依赖方向：

```text
kanban-server → kanban-service → turso
kanban-cli / kanban-mcp / Desktop → kanban-client → localhost HTTP
xtask → kanban-protocol[schema] + cargo metadata
```

CLI、MCP、Desktop、fixture 和 test-support 不得依赖 `kanban-service` 的数据库-owning path、SQLite importer feature、第二 store 或 `xtask` runtime。`legacy-sqlite-import` 只在 host/service feature 中编译，执行的是只读 legacy source import，不是 active SQLite backend。

`scripts/check-single-host-dependencies.py` 与 schema dependency policy 负责 package/feature/target-specific 依赖隔离；失败时修复 manifest/source，不通过删除契约或伪造 witness 掩盖。

## 6. Recipe 和验证顺序

```text
just schema-generate
just schema-check
just schema-docs
just schema-surface-audit
just schema-adoption-witness
just schema-contract
```

`xtask schema generate/check/audit/witnesses` 是离线工具；`just schema-docs` 还检查 `KANBAN_SPEC_BUNDLE.md`、JSON fence marker 和 fixture 映射。只改文档时至少运行 `just diff-check`、`just spec-bundle-check`；schema marker/fixture 变化再运行 `just schema-docs`。写 Cargo target 的 recipe 统一经 `scripts/cargo-build-lock.sh`。

`schema-audit-closed` 只有在 source inventory 无 `planned/generated`/未闭合 obligation 且所有 exact witness 实际通过时才可称 closed；本次文档任务不预先宣称该状态。release/package/full gate、push、PR 和发布不属于 schema 文档同步。

## 7. 新 operation 闭环

1. 在 `kanban-protocol` 定义 DTO、root、endpoint/surface descriptor 和 migration state；
2. 添加 valid/invalid fixture 与 producer/consumer exact witness；
3. 贯通 service → server → client → 需要的 CLI/MCP/Desktop adapter；
4. 运行受影响 package tests、`just schema-check`、`just schema-surface-audit`、`just schema-adoption-witness` 和 `just diff-check`；
5. 若取消 operation，改为 `excluded` 并保留理由，不伪造 route 或 fixture。

完整 parity 还要求 FTS/vector/graph/context 能从 canonical facts rebuild、migration/rollback/recovery 可验证、旧 sidecar 不在 active workspace；具体 owner/test/gate 见 [`migration/turso-full-feature-parity.md`](migration/turso-full-feature-parity.md)。

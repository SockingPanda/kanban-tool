# JSON Schema Contracts

## 1. 当前 Authority

`kanban-contract` 承载公开机器契约的迁移账册、候选/已采用 wire DTO 和 JSON Schema
root registry。`kanban-schema-tool` leaf crate 独占 binary、离线校验、artifact 与 hash/drift
tooling。状态必须区分：

- Rust 类型已经可以生成 schema。
- runtime adapter 已经实际使用该类型生产或消费 JSON。

早期 foundation 的 API error/health response、label semantics delete response 与 decision
metadata input 现在均为 `adopted`。API error 的 `code` 已收敛为闭合的 `ApiErrorCode`
snake_case enum；status 与 locale-dependent `message` 仍由 server adapter/core error rendering
决定。label semantics delete response 已由真实 CLI adapter 生产；decision metadata input 也有
独立的 typed producer 与真实 CLI consumer witness。任何仍为 `generated` / `planned` 的条目继续
以真实 adapter 为 runtime 行为 authority；不能因为 `kanban-contract` 中存在同形 DTO 就宣称
owner 已迁移。

提交的 schema 由 Rust 类型确定性生成。`schemas/fixtures/**` 是手工提交且同时经过
Serde/JSON Schema 测试的 canonical examples。Adoption evidence 必须按 direction 分工：
`Deserialize` request 的 producer 由真实 contract DTO 程序化构造并序列化，结果与 committed
valid fixture 精确相等；consumer 从该 fixture 反序列化，并通过真实 runtime router/handler。
`Serialize` response 的 producer 才来自真实 adapter response path。producer/consumer 不得
共用同一个 exercise helper 或仅靠测试名伪装独立证据。每个 witness 必须包含 `operation`、
`contract_id`、`surface`、`direction`、`package`、`test_target` 和 `exact_test`。

语义 authority 保持分层：

- wire DTO 与 schema：字段、类型、required/optional、unknown-field policy 和基础值域。
- `docs/API_SPEC.md`、`docs/CLI_SPEC.md`：operation、HTTP/exit code、stdout/stderr
  和用户可见行为。
- `kanban-sqlite::service` 与 `kanban-core`：transaction、状态机、CAS、dependency、
  recompute 和 structured metadata 的跨字段业务 guard。

schema 校验通过不代表业务命令可以执行；业务测试不能被 schema fixture 替代。

## 2. Migration State

`operation_inventory()` 中每个 semantic contract 都必须使用以下状态之一：

| 状态 | 含义 | 必备证据 |
|---|---|---|
| `planned` | 已识别精确边界，尚未生成 root | 不允许伪填 schema、fixture、adoption 或 exclusion |
| `generated` | Rust 类型、root 和手工 schema fixtures 已存在 | schema ID、正例 fixture；不得声明 runtime adoption |
| `adopted` | 真实 producer/consumer 已切到同一 contract | schema fixture、真实 producer fixture、双方结构化且可执行的精确 test witness |
| `excluded` | 明确不是稳定 JSON contract | 具体 exclusion 理由，不能同时声明 schema 或 fixture |

`schema-audit-closed` 只允许 `adopted` 和 `excluded`。它同时拒绝 family、
wildcard、bidirectional shortcut；双向协议必须拆成精确 input/output contract。
因此“生成了 schema”永远不能代替“runtime 已采用”。

当前已生成的 schema roots：

- foundation：API error response、`GET /health` response、label semantics delete response、
  decision comment metadata input。
- B1-B lifecycle request：`SpecifyTaskRequest`、`PromoteTaskRequest`、`ClaimTaskRequest`、
  `ReclaimTaskRequest`、`HeartbeatTaskRequest`、`CompleteTaskRequest`、
  `SubmitReviewTaskRequest`、`BlockTaskRequest`、`UnblockTaskRequest`、
  `ReopenTaskRequest`、`ArchiveTaskRequest`、`ArchiveBoardRequest`、
  `AddDependencyRequest`。
- B1-C1 task-read request：`GET /api/v1/boards/:board/tasks` 与
  `GET /api/v1/boards/:board/tasks/by-status` 各自独立的 path/query DTO，共 4 个 exact roots；
  query schema 对 repeated fields 同时声明 `uniqueItems` 与 9/4/3/32 `maxItems`，并冻结
  `q=1024`、`assignee=128`、单个 `label=128` 的 `maxLength`、label 的 Unicode
  `White_Space` 反集 pattern 及 `limit=1000`。raw 8192-byte cap 与由各字段预算推导出的
  54-pair cap 由 server runtime 门禁负责；标准 form-encoded UTF-8/reserved-character fixture、
  Unicode 纯空白负例与非默认 board sentinel 证明真实 producer/consumer。
- B2-C6 boards：list query、create request、get/archive path 与四个 endpoint-specific success
  response，共 8 个 exact roots；四个 success roots 只共享闭合 `ApiBoard` component。

当前 train authority snapshot 有 483 个 schema roots：semantic contract migration 为
483 个 `adopted`、0 个 `generated`、0 个 `planned`，没有 `excluded`；483 个 adopted
contract 登记 966 个 structured witnesses。Projection v2 新增
`cli.maintenance-status.output`、`cli.maintenance-run.output` 与
`cli.maintenance-rebuild.output`：status 显式携带 database identity/protocol、owner lease
summary、generation/provider/cursor/pending age/error/fallback reason，且不暴露 opaque lease
token。Search/index exact roots 同步要求 nullable database identity/protocol/generation、
必填 resolved board id 与 nullable structured fallback reason。#440 已将既有 114 个有限 JSON CLI leaf 全部绑定到
exact output roots：除既有 bootstrap/config、diagnostics、maintenance、board/task/step/run、
comment/dependency/event/entity cohort 外，本批继续闭合 33 个 label/semantics/proposal/atom/
ontology leaf、15 个 graph/vector/context/search/index helper-facing leaf，以及 14 个
signal/hook/dispatch/export/import operator leaf。每个 root 都由真实 binary producer、typed
consumer、committed fixture 与 fail-closed runtime ownership gate验证；export stdout JSONL
stream 不属于有限 envelope。#441 已将 21 个 portable JSONL discriminator 的 input/output
拆成 42 个 exact roots，并把 descriptor、export/import runtime adapter、逐行 fixture validation
与双向 adoption witnesses 收敛为同一 authority；record data 是闭合 natural JSON，required-nullable
键禁止省略但接受显式 `null`。CLI task/step/run adapter 会显式丢弃
persistence-only `claim_token` 与内部 `log_path`，包括递归 linked task；dependency、events 与
helper subprocess protocol 继续保持各自 owner，public CLI contract 只拥有最终 stdout shape。
#447 已把 2 个 decoded TOML config input、7 个 graph helper response 与 12 个 vector helper
response 闭合为 21 个 exact roots；worker profile input 只拥有被 CLI 选中的
`[workers.<profile>]` section，未选 section 保持 opaque/forward-compatible，选中 section 继续
严格拒绝未知或非法字段；真实 config decoder、subprocess adapter 与 protocol decoder 分别提供
producer/consumer witness，schema tooling 依赖仍隔离在 leaf crate。
#446 已把 labels、signals、ontology 与 proposals API 的 75 个 generated roots（48 个 request
deserialize、27 个 response serialize）绑定到真实 router/DTO producer 与 exact contract
consumer。#443 删除了 3 个无真实 router obligation 的 generated orphan，authority 已闭合。

`surface_operation_catalog()` migration 是独立维度：246 个 `adopted`、0 个 `generated`、
0 个 `planned`、5 个 `excluded`。其中 CLI 为 114 个 `adopted`、5 个非 JSON
`excluded`，21 个 JSONL record surfaces 与 6 个 structured metadata surfaces 全部为
`adopted`，Config/Helper 为 2/19 个 `adopted`；API 为 83 个 `adopted`，SSE 为 1 个
`adopted`。Endpoint
obligation histogram 同样独立：296 个
`Contract`、0 个 `Todo`、207 个 `NotApplicable`、1 个有运行时证据的 `Excluded`。
`schema-check` 的未闭合项为 0：semantic generated/planned 0 + surface
generated/planned 0 + endpoint Todo 0。

B7 transport closure 为 83 个 non-SSE endpoint 各注册一个 operation-specific exact header
root，并按真实 router 行为复用五种闭合 wire profile。所有 profile 都接受可选
`Accept-Language`；actor mutation 增加可选 `X-KB-Actor`；required/optional JSON body 分别使用
`RequiredOne`/`OptionalOne` 的 `Content-Type`，无 body endpoint 不声明该参数。83 个 roots 均以
profile fixture producer 和真实 router consumer 作为 structured witnesses。SSE
`Last-Event-ID` 保持有运行时证据的 `Excluded`。由此 API endpoint catalog 已无 `Todo`；全局
`schema-audit-closed` 已无 semantic、surface 或 endpoint authority 缺口。

下方各 cohort 的 headers `Todo` 与冻结计数是当时的历史验收快照；B7 authority snapshot
取代这些局部数值，不应据此回退当前 catalog。

本轮在 events API topology 快照之上继续采用 claim response、ontology signals response 与
label-semantics delete acknowledgement。`api.list-events.response` 仍是 API 专属 exact root；
其 data 只在 Rust/schema component 层复用 `StreamEventData` 字段，不把 SSE exact root 当成
API linkage。下方 B2/B3/B5/B6 段落均为历史 cohort freeze，不覆盖本快照。

#441 同时采用六个 exact metadata contract：decision comment input、signal record input、
service-generated signal backlink output、label proposal candidate input、ontology record input
与 ontology external validation evidence input。Decision 已知字段和 option extensions 使用
typed-open contract，跨字段选择/理由规则仍归 service；普通 comment response 的 `metadata`
始终是开放、无损的自然 JSON object，不能因为用户键名碰撞而在 transaction 提交后收紧。
Signal backlink 的闭合 DTO 只由真实 service producer 与独立 fixture consumer 证明。

`StreamEventData` 对 39 个 known event kind 使用 `kind + payload` sibling 约束：known payload
拒绝缺失的 required-nullable key、额外字段、错误 enum/value range 与 kind/status 不匹配；未来
unknown kind 保留任意合法 JSON payload。外层 `task_id`、`run_id`、`actor` 均为“键必需、值可
显式 `null`”。这项 typed union 由 list-events API 与 SSE 共同复用；portable JSONL 的
`event.data.payload` 仍是 opaque `Value`，不得据此误称 JSONL event payload 已关闭。

B5-C2 采用 dependency list/add/remove、execution-plan not-required、get-run-log 与
list-board-columns 的 endpoint-specific path/request/success roots。无 query/body 的维度明确为
`NotApplicable`；通用 actor/locale/content-type headers 仍保持 `Todo`，留给后续统一 transport
closure cohort。get-run-log 的现有 JSON response 是真实 runtime contract，并继续只暴露
`run_id/content/truncated`；SQLite `log_path` 与 claim token 均不得进入 wire。dependency cycle、
同 transaction edge/recompute/event、execution-plan guard、status recompute 与 board scope 仍由
既有 service path 拥有，schema 不替代这些业务不变量。

## 3. Exact Surface Catalog

`surface_operation_catalog()` 记录可以自动发现的公开 transport operation：

- API：83 个 JSON method/path，加 1 个 SSE method/path。
- CLI：119 个 Clap leaf command；非 JSON text/daemon/hook protocol 逐项 `excluded`。
- JSONL：21 个精确 `type=<discriminator>`。
- Metadata：6 个无 transport 的 exact structured metadata operation。

防漏 seam 与生产注册同源：

- API 由 `AuditedRouter` / `endpoint_route!` 注册 Axum route；每个 binding 以 descriptor 的实际 method/path 建立并审计。
- CLI 测试从 `clap::CommandFactory` 递归枚举真实 leaf command。
- JSONL exporter/importer 共用 `PORTABLE_RECORDS` discriminator/table/scope descriptor。

JSONL exact roots 只描述当前 natural JSON wire contract。SQLite importer 在进入 exact
record decoder 前允许一次 one-way compatibility normalization：仅接受上一版真实 exporter
写出的 coherent storage-native snapshot（JSON text columns 与 integer booleans），并拒绝与
natural records 混用；同一 record 同时出现 natural/storage-native renamed keys 时，必须在
normalization 前拒绝，不能由 legacy 值覆盖 natural 值。Normalization 后仍由相同的 21 个
input roots 和现有 service/doctor guards 校验；export producer 不写 legacy keys，因此该
migration 不新增 schema root、surface operation 或双轨 output contract。

以上集合与 committed `surface-operations.json` byte-stable catalog 对照。新增、删除或
重命名 route、command、export type 时，`schema-surface-audit` 必须先 RED，直到精确
catalog 和迁移状态被有意更新。`/api/v1/**`、`kanban ** --json` 或一个
bidirectional family 不能用于关闭这些 operation。

## 4. Dependency Boundary

| Build mode | Enabled dependencies | Intended use |
|---|---|---|
| `kanban-contract` default | `serde`、`serde_json` | contract 数据类型与迁移账册 |
| `kanban-contract/schema` | default + `schemars 1.2.1` | 从 Rust DTO 生成 schema document |
| `kanban-schema-tool` | `kanban-contract/schema` + `jsonschema 0.47.0` + `sha2` | 离线 metaschema、fixture、manifest 和 drift gate |

Phase 1 将 leaf tool 的 direct dependency 拓扑精确锁定为 5 条 normal edge：
`jsonschema`、`kanban-contract`、`serde`、`serde_json` 与 `sha2`。它们必须来自 root
workspace canonical 声明，且 source/path、version requirement、default feature、feature set、
alias、optional 与 target signature 全部一致；tool 不得声明 dev、build 或 target-specific
dependency。除 tool 自身外，任何 workspace member 都不得通过 normal/dev/build、alias、
optional 或 target-specific direct edge 引用它。structured manifest policy 锁定 canonical 声明；
metadata policy 必须从 `crates/kanban-schema-tool/Cargo.toml` 运行 full locked graph，不得使用
`--no-deps`，并 fail-closed 校验 `resolve.root`、package/node 唯一性、tool/contract canonical
package ID 与 manifest path、五条 resolved direct edge 及 tool-root reachable closure。除当前
workspace tool/contract 外，closure 的每个 package 都必须来自 crates.io；path/git direct 或
transitive override 都失败。

`policy/schema-tool-registry-closure.json` 是唯一的 registry closure approval。它用
`format_version = 1`、`lockfile_version = 4`、`root_package = "kanban-schema-tool"`
和 canonical `packages[]` 表达当前 reachable registry set；每项字段必须精确为
`name`、`version`、`source`、`checksum`，按 `(name, version, source)` 排序，未知字段、
重复、缺失、额外项、非 canonical 顺序和 checksum 漂移全部失败。policy 解析真实
`Cargo.lock` 并双向比较，但普通 gate 永不自动写入或 bless approval。该边界检测
committed lockfile 相对 approved snapshot 的 identity/checksum 漂移；Cargo fetch/build
另行按 registry index `cksum` 验证 crate 内容。

Cargo metadata 的 `SourceId` 仅作为 opaque identity：本项目锁定 pinned toolchain 下批准的
logical SourceId 字符串，不把其 URL 字符串当成 Cargo 的通用 canonical network URL；
物理下载允许 Cargo source replacement mirror。六个产品 graph 的真实 `cargo tree` 另行负责
all-features/all-target normal runtime 传递性泄漏扫描，不能替代 dev/build direct-edge
检查。Phase 2 若需改变拓扑，必须先形成新决策并显式更新 gate，不能通过 manifest、
resolve、lockfile、approval 或 recipe 漂移暗中扩边。

`kanban-contract` 的 manifest feature 必须精确为 `default = []` 与
`schema = ["dep:schemars"]`；dependencies 必须精确为 `serde`、`serde_json` 和 optional
workspace `schemars`，且不允许 dev/build/target dependency。root canonical `schemars` 固定
`1.2.1`、`default-features = false`、`features = ["std", "derive"]`；full resolve 必须启用
contract `schema` 并形成唯一同名 crates.io `schemars 1.2.1` edge。

`schemars 1.x` 与 `jsonschema` 都关闭默认 feature。正常 CLI/server/desktop/dispatcher 及
`kanban-vector-lancedb`、`kanban-graph-oxigraph` 产品 helper 依赖图不得启用 `kanban-contract/schema`、依赖 `kanban-schema-tool`，也不得包含本项目采用的
`schemars 1.x` / `jsonschema`。Tauri 自身当前存在独立的 `schemars 0.8` transitive
依赖；隔离 gate 明确区分该既有图与 leaf tooling graph。

任何拥有 adopted producer/consumer witness 的 package 都必须通过 normal dependency
引用当前 workspace 的 `crates/kanban-contract`；只有 dev-dependency，或指向 registry、
git、其它本地 path 的同名 package，都不能证明运行时采用。witness gate 从完整
`cargo metadata` 同时锁定 canonical manifest path、workspace package ID、unconditional
non-optional normal dependency 声明和 default resolve edge：两者都要求 `kind is None` 与
`target is None`，声明还要求 `optional is false`。平台或 feature-specific witness 当前不受
支持。默认 metadata/exact test 是正向采用证明；随后以 adopter package ID 运行
`cargo tree --all-features --target all --edges normal,features --locked`，作为负向泄漏扫描，
覆盖 host、target-specific 与产品 feature runtime graph，拒绝 `kanban-contract/schema`、
`kanban-schema-tool`、`schemars 1.x` 或 `jsonschema` 泄漏，并要求 tree 实际出现当前
workspace contract path。离线 tooling 只能通过 leaf crate 执行，tooling owner 本身不能充当
runtime adoption witness。

schema tooling 不启用 HTTP/file resolver、TLS、OpenAPI 或生产 runtime validation。

## 5. Root Contract

- Dialect 固定为 JSON Schema Draft 2020-12，不依赖库默认值。
- request/input 使用 `SchemaSettings::for_deserialize()`。
- response/output 使用 `SchemaSettings::for_serialize()`。
- root ID 固定为 `urn:kanban-tool:schema:<surface>:<semantic-name>:v1`。
- root major 与 crate version、API route version 解耦；breaking wire contract 提升 root
  major，并在同一 train 删除被替代 artifact，不保留兼容双轨。
- schema 必须 self-contained；只允许 `#/$defs/...` 形式的本地 `$ref`。
- 产物不得包含 timestamp、hostname、绝对路径或联网 reference。
- `DecisionMetadata.risk` / `verification` 允许 missing，但 explicit `null` 同时被真实
  Serde DTO 和 JSON Schema 拒绝。

Decision metadata 的现有 service 语义允许未知 top-level/option 字段，所以该 root 是
typed-open contract：已知字段被验证，扩展字段被保留。selected 必须匹配 option、
slug 唯一、纯空白字符串拒绝等跨字段/业务约束仍由现有 service guard 负责。

## 6. Committed Layout

```text
schemas/
  fixtures/
    api/
    metadata/
  json-schema/
    draft-2020-12/
      operations.json
      surface-operations.json
      manifest.json
      api/
      metadata/
```

`operations.json` 记录 semantic contract 状态；`surface-operations.json` 记录精确
transport operation。`manifest.json` 分别记录两者的 hash，以及每个 root 的 ID、path、
operation、direction、strictness、schema fixtures 和 SHA-256。生成顺序和 JSON key
顺序稳定；连续生成必须 byte-identical。

仓库顶层 Markdown 与 `docs/**/*.md` 中每个 CommonMark `json` fence 都必须紧邻以下两类
marker 之一。opening fence 可使用至少三个 backtick 或 tilde、最多三个前导空格，并可在
严格的首个 `json` info token 后携带 attributes；closing fence 必须使用相同字符且长度不短于
opening fence：

- `schema-doc` 同时声明 exact `contract` 与其 manifest-owned positive `fixture`；inline JSON
  必须可解析且与该 fixture 的 JSON value 完全一致。
- `schema-doc-ignore` 必须填写非空理由，只用于片段、伪值或其它有意不作为完整 wire
  example 的说明性 payload。

`schema-docs` 会拒绝未标记 fence、malformed/orphan marker、未知 contract、fixture mapping
漂移、无效 JSON 与 inline/fixture mismatch。新增公开示例不能依赖“看起来相似”的手工
payload；要么复用 committed canonical fixture，要么明确说明为何只是 illustrative fragment。

## 7. Commands

```bash
just schema-generate
just schema-check
just schema-docs
just schema-tool
just schema-surface-audit
just schema-dependency-isolation
just schema-adoption-witness-self-test
just schema-adoption-witness
just schema-contract
just schema-audit-closed
```

- `schema-generate` 从 registry 重新生成 committed tree，然后立即验证。
- `schema-check` 不写文件，比较 fresh generation 与 committed tree，并拒绝 missing、
  stale、orphan 或 byte drift。
- `schema-docs` 先运行 marker 负测，再审计顶层与 `docs/` 下全部项目文档中的 public JSON fence。
- `schema-surface-audit` 对照真实 API/CLI/JSONL surface 与 exact catalog。
- `schema-dependency-isolation-self-test` 用 fake `cargo` 锁定 default contract、六个产品
  crate/helper 的 all-features/all-target 负向扫描与 leaf tool 正向控制 argv；
  manifest/metadata/lockfile/approval mutations 覆盖 full resolve 的
  missing/duplicate node/record、wrong package/source/path、同名多版本 checksum、
  direct/transitive path/git override、contract/schema/schemars 漂移和 registry closure
  双向集合漂移。真实 `just` parser AST hash 与 fake nested
  `just`/build-lock/cargo/python/script JSONL trace 另外锁定产品 `fmt`（core）、
  `fmt-full`（core + helper）、`schema-fmt`（contract + leaf）的互斥 package selection，
  full/rust/test 调用图、schema 子 gate、`schema-audit-closed` 内部调用、`release`
  13 步顺序和 `test-full` 的 nextest/fallback 双分支。mutation tests 必须拒绝
  workspace-wide fmt、package 漂移、gate 删除、命令旁路与顺序调换。
- `schema-dependency-isolation` 先运行该自测，再用结构化 manifest/full locked metadata policy
  检查全部 workspace declaration、resolved identity、真实 `Cargo.lock` 与 committed registry
  approval，再用真实 cargo tree 检查六个产品的传递性 runtime graph 与 leaf tooling graph；
  tool 不能进入产品 default/core/helper/full/rust 门禁，也不能作为 runtime adopter。
- `schema-adoption-witness-self-test` 持久验证 dev-only dependency、registry/git/其它
  path 同名包冒充、resolve package ID 漂移、all-target normal graph 泄漏、缺失 test
  target、0 exact tests 和“列出但未执行”等防伪分支。
- `schema-adoption-witness` 先运行上述负测，再从当前 Rust inventory 读取 adopted witness；
  Cargo plan/metadata/tree/test 调用全部使用 `--locked`。gate 先按
  `(package, test_target, exact_test)` 去重 locator，再按 `(package, test_target)` 分组；同一组
  只启动一次未过滤的 list 与一次完整 test-target process。list 输出必须唯一列出组内每个
  `exact_test`，完整执行输出必须逐项显示这些 test 真实通过；同一 locator 可以承载多条
  contract/role mapping，报告仍逐条保留。gate 不再为每个 witness 单独运行
  `--exact --list` 或要求单测试进程的 `1 passed` summary。当前每个 adopted contract 的
  producer/consumer mapping 都必须被分组执行覆盖；当前计数以本文件前部唯一的 train
  authority snapshot 为准。
- `schema-contract` 先运行 dependency isolation，再运行只选择 `kanban-contract` 与
  `kanban-schema-tool` 的 `schema-fmt`，随后汇总 feature tests/clippy、metaschema、正负
  fixtures、determinism、docs marker、surface audit、adoption witness 和 committed drift gate。
- `schema-audit-closed` 用于整个 migration train 的最终关闭检查；真实 trace 锁定它先执行
  adoption witness，再通过 build lock 运行 `kanban-schema audit --require-closed`。当前
  migration train 的 contract、surface 与 endpoint obligation 已全部闭合；该 gate 应成功，
  G006 已由 WATCH 转为 closed evidence。
- `release` 精确依次调用 `affected-self-test`、`schema-contract`、`audit`、`rust-full`、
  `bench-check`、`target-tools`、`cli-package`、`cli-package-layout`、
  `desktop-package-config`、`desktop-package`、`desktop-package-layout`、`smoke` 与
  `diff-check`；AST + ordered trace 对删除或重排 fail closed。

所有会写 Cargo target 的命令必须通过这些 `just` recipes 和仓库 build lock 运行。

## 8. Adoption Checklist

将条目改为 `adopted` 前必须同时满足：

- operation 是精确 input 或 output，不是 family/wildcard/bidirectional shortcut。
- runtime producer/consumer 实际引用 `kanban-contract` 的同一 DTO。
- `adoption.producer_fixture` 能通过对应 schema；request/input 由 contract DTO 程序化
  serialize 后与它精确比较，response/output 则来自真实 adapter producer path。
- consumer 对 request/input 必须从 committed fixture 开始并通过真实 production handler；
  producer 与 consumer 不能调用同一个高层 exercise helper。
- `adoption.producer` 和 `adoption.consumer` 都声明完整的结构化 witness；其
  `operation`、`contract_id`、`surface`、`direction` 与 adopted surface/contract
  完全一致。
- 每个 witness 的 `package` 以 normal path dependency 引用当前 workspace
  `crates/kanban-contract`；声明 path/source 和 resolve package ID 都一致。
- 以 adopter package ID 生成的 all-target normal graph 不启用 `kanban-contract/schema`、
  不依赖 `kanban-schema-tool`，且包含当前 workspace contract path。
- `test_target` 使用 `lib` 或具名 integration test target；分组 list 必须唯一列出每个
  `exact_test`，完整 test-target process 必须整体成功并逐项显示这些 test 通过；缺失、重复、
  ignored、未执行或 target 内任一测试失败都失败。
- schema direction 与真实 Serde 使用方向一致。
- strictness 与现有行为一致，没有把 open metadata 错误收紧。
- service/state-machine/exit-code/HTTP-status/transaction tests 继续保留。
- `schema-check`、surface audit、受影响测试和默认 dependency isolation 均有证据。


## 9. Transport descriptor authority (Phase 2)

`kanban-contract` default feature 现在还拥有 dependency-free 的 transport descriptor catalog。它精确列出 84 个真实 endpoint（83 JSON API + 1 SSE）：稳定 `operation_id`、`surface`、自有 `HttpMethod`、`path`、migration/exclusion 和六项明确 obligation（path/query/headers/body/success/SSE）。每一项 obligation 必须显式为 `Contract(contract_id)`、`NotApplicable`、`Excluded { reason }` 或 `Todo`；没有 `Option` 或隐式默认。

`OperationContract.transport` 对 API/SSE 必须显式为
`Http { operation_key, location, parameters }`，对 CLI/JSONL/metadata/config/helper 必须显式为
`NoTransport`。location 是 `Path|Query|Headers|Body|Success|Error|Sse`：前四项只允许
`Deserialize`，后三项只允许 `Serialize`。`Success` 只表示 2xx success；`Error` 只允许
`SharedComponent`，表示非 2xx response，且不会增加第七种 endpoint obligation。只有
path/query/headers 可以声明 wire parameter；每个参数必须选择
`RequiredOne|OptionalOne|RepeatedOrdered`。名称为空或有首尾空白、header 大小写冲突、缺
cardinality、非 `RequiredOne` 的 path 参数、path placeholder 的名称/缺失/额外/顺序/大小写
不精确匹配，以及 Body/Success/Error/SSE 携带 parameters 都会失败。

`SurfaceOperation` 仍保留 CLI/JSONL inventory，但 API/SSE 条目由 descriptor 投影生成，不能再维护独立的手写 method/path 表。Schema root 的关联字段为 `contract_id`；`operation_id` 只属于 transport endpoint，二者不能混用。

server 在唯一注册点为每个 descriptor 建立稳定、显式、唯一的 `adapter_id` 与 handler binding，并从 descriptor 取得 method/path。validator 同时拒绝 duplicate `operation_id`、duplicate method/path、wrong surface 及缺失/重复/orphan runtime binding；这一步只收敛 transport 身份，DTO adoption 仍按 migration train 单独完成。

Endpoint migration state 与单项 obligation adoption 分开收敛：endpoint 可以保持
`Generated`，同时把已经真实迁移的 body 标为 `Contract(contract_id)`，且该 contract 为
`Adopted`。任意 `Adopted` contract 都必须是 `granularity=Exact`；obligation 只能引用
`Generated|Adopted` 且 `binding=ExactSurface, granularity=Exact` 的 contract，并要求
operation、surface、direction 与 location 全部精确匹配。唯一 method/path、精确
`operation_key` 与单一 location 已结构性保证 endpoint exact binding 唯一，因此不保留一个
不可达的全局 second-binding guard；surface catalog 自身仍显式拒绝重复 exact reference。
unknown、`Planned`、`Excluded` 或错位引用均 fail closed。只要其它 obligation 仍为 `Todo`，
endpoint 就不能提升为 `Adopted`。

`SharedComponent` 使用无 exact `operation_key` 的 HTTP transport，可以被多个 endpoint
显式链接。orphan policy 是严格的 OR：至少一个显式 linkage，或同 surface 的真实 adoption
witness；两者均缺失才失败，已有显式 linkage 时不再要求 witness operation 出现在 catalog。
shared reference 会进入投影 artifact 供审计，但不进入 exact adoption set、不满足 endpoint
obligation，也不能单独把整个 endpoint 伪装为 `Generated` 或 `Adopted`。当前
`api.error.response` 使用 `location=Error` 并显式链接到
`GET /api/v1/boards/:board/tasks`，不计入 success exact coverage，也不改变该 endpoint 的
`Planned` 状态。

B1-B lifecycle request 采用 13 个独立 DTO，不提供通用 transition/token body。所有 DTO
拒绝未知顶层字段；`ClaimTaskRequest.metadata` 与 `CompleteTaskRequest.result` 仅保持
`serde_json::Value` opaque extension，`SubmitReviewTaskRequest` 则完全不接受 `result`。
`ReclaimTaskRequest.to_status` 是封闭的 `ready|blocked` 枚举。Promote、reclaim、unblock、
task archive 和 board archive 保留 optional body 与既有默认值，actor 仍按 body、
`X-KB-Actor`、server default 的优先级解析。


## B1-C2b task-read 成功响应契约

`GET /api/v1/boards/:board/tasks` 与 `GET /api/v1/boards/:board/tasks/by-status` 各自拥有独立、精确且闭合的成功响应契约，仅共享 `ApiTask`、`ApiLabel` 与既有 `OffsetPaginationMeta`/`TotalPaginationMeta` primitives。列表响应为 `data[]` 与既有 `TotalPaginationMeta { limit, offset, total }`；按状态响应包含有序窗口，每个窗口使用同一 `TotalPaginationMeta`，外层使用既有 `OffsetPaginationMeta { limit, offset }`。这只是 Rust 类型复用，JSON wire 形状不变。

Desktop 仅对这两个读取端点使用 endpoint-specific recursive exact parser：成功响应的 envelope、`meta`、窗口、共享 `ApiTask`、`ApiLabel` 与既有 `OffsetPaginationMeta`/`TotalPaginationMeta` primitives 都必须闭合且完整，pagination 数值必须是非负 safe integer；错误响应也必须是闭合的 `error { code, message, details? }` envelope。任何 malformed、mixed、missing 或 extra shape 统一返回 `invalid_response`，合法错误继续保留 `code`、`message` 与可选 `details`。其它 generic optional envelope 不受影响。两个 endpoint 的 headers 仍为 `Todo`，本次不采纳任何其它 endpoint。

当前冻结值：schema roots=23，adopted contracts=21，witnesses=42，Contract=21，Todo=381，NotApplicable=102，unfinished=628。


## B2-C3 comments pair

Comments pair 注册 5 个 endpoint-specific roots：list path/response 与 create path/request/response。
成功 envelope 均为闭合 `data`-only shape，且 list/create response root 不互相复用；只共享闭合
`ApiComment` component。`CommentAuthorType` 与 `CommentKind` 是闭合 enum，`agent_type` 为
required-nullable。create request 的 metadata 保持为开放 JSON object；独立的
`metadata.decision.input` exact contract 描述 decision typed shape，跨字段业务约束仍由既有
SQLite service guard 执行。

五个 roots 均为 `Adopted`，每个 root 都登记 structured producer/consumer witness；两个 endpoint 仍为 `Generated`，因为 headers 继续为 `Todo`。本批生成后的权威快照为 schema roots=28、adopted contracts=26、witnesses=52、endpoint Contract=26、Todo=373、NotApplicable=105、unfinished=620（inventory generated/planned/todo 分别为 29/218/373）。

## B2-C4 run reads

`GET /api/v1/tasks/:task_id/runs` 与 `GET /api/v1/runs/:run_id` 注册四个 endpoint-specific
path/success roots，并共享 contract-owned `ApiRun`。四个 roots 均为 `Adopted`，各自登记真实
structured producer/consumer witness；两个 endpoint 因 headers 仍为 `Todo` 而保持
`Generated`。`ApiRun.status` 仅允许 `running|succeeded|failed|canceled|expired`，nullable
lifecycle 字段必须出现；公开 response 不包含 `claim_token` 或内部 `log_path`。本批权威快照：
schema roots=32、adopted contracts=30、witnesses=60、endpoint Contract=30、Todo=365、
NotApplicable=109、unfinished=612（inventory generated/planned/todo 为 29/218/365）。
## B2-C5 create task

`POST /api/v1/boards/:board/tasks` 注册 endpoint-specific path、request 与 success response 三个
`Adopted + Exact + ExactSurface` roots。request 使用 create-only 闭合 status vocabulary
`triage|todo|scheduled|ready`，精确保留 priority=3、空 labels/depends_on 与 nullable 字段的既有
Serde 默认；metadata 只允许 opaque JSON object。response 是闭合
`CreateTaskResponse { data: ApiTask }`，不允许 private envelope 或 claim token。

真实非默认 board router fixture 同时覆盖 labels、dependencies、actor、priority、metadata、
`201` 与 ready 降级；独立 DTO producer、真实 router consumer、contract response consumer、
Desktop exact consumer 和 syn ownership mutation 形成结构化 evidence。handler 继续唯一调用
`create_task_with_labels_and_dependencies`，SQLite transaction/retry/readiness/privacy authority不变。
endpoint 因 headers 仍为 `Todo` 保持 `Generated`。本批冻结值为 schema roots=31、adopted
contracts=29、witnesses=58、endpoint Contract=29、Todo=369、NotApplicable=106、unfinished=616。
## B2-C6 boards endpoints

`GET/POST /api/v1/boards`、`GET /api/v1/boards/:board` 与
`POST /api/v1/boards/:board/archive` 由 8 个 endpoint-specific roots 覆盖：list query、create
request、get/archive path 与四个 success response。四个 success root 只复用 contract-owned
`ApiBoard` component；server 不再把 `kanban_sqlite::api::BoardRecord` 直接暴露为 wire type，
也不再拥有 private `CreateBoardBody`。archive body 继续唯一复用先前 adopted 的
`ArchiveBoardRequest`，没有建立重复 owner。

所有新 roots 均为 `Adopted`，并登记独立、可执行的 structured producer/consumer witness。
真实 router fixtures 使用 non-default board，list query 的 `include_archived` 进入
`kanban_sqlite::api::list_boards` 的 application options；create/get/archive 继续走同一 facade。
running-work archive guard、archived history/read、not-found HTTP status 与 locale-dependent message
仍由既有 service/adapter 测试保护。Desktop 的生产 `listBoards` caller 对 envelope 与每个
`ApiBoard` 字段执行 exact parser，拒绝 missing、mistyped 与 extra fields。

四个 endpoint 的 path/query/body/success 已闭合或标为 `NotApplicable`，headers 仍为
`Todo`，所以 endpoint migration 诚实保持 `Generated`。本批权威快照为 schema roots=36、
adopted contracts=34、witnesses=68、endpoint Contract=34、Todo=360、NotApplicable=110、
unfinished=607（inventory generated/planned/todo 分别为 29/218/360）。
## B2-C7 steps family

Steps family 的 list/create/update/remove/done/skip/reopen 七个 endpoint 各自拥有 exact path 与
success root；create/update/done/skip/reopen 另有 endpoint-specific request root。成功响应只共享
闭合的 `ApiTaskStep`、`ApiExecutionPlan` 与 `ApiTaskSteps` components，不复用 endpoint root。
`ApiStepStatus` 闭合为 `todo|done|skipped`；nullable resolution、linked task 与 execution-plan
字段在成功响应中保持 required-nullable。request 的 optional/default/alias 行为与既有 handler
一致，title/reason、required-step completion、plan readiness、linked-task 与 transition guard
继续由 SQLite service/state-machine 拥有，schema 只证明 wire shape。

19 个 roots 均为 `Adopted` 并登记 structured producer/consumer witnesses。真实 router producer
使用非默认 `project` board，Desktop 的七个 production callers 使用 endpoint-specific recursive
exact consumer；server AST mutation gate 锁定 contract-owned path/request/response、canonical service
call 与 private DTO/request owner 删除。七个 endpoint 保持 `Generated`，因为 headers 仍为 `Todo`。
本批生成后的权威快照为 schema roots=47、adopted contracts=45、witnesses=90、endpoint
Contract=45、Todo=345、NotApplicable=114、unfinished=592（inventory generated/planned/todo 分别为
36/211/345）。

## B2-C5-C7 integrated freeze

Create-task、boards 与 steps 三条已审查 lane 在 feature train 上语义合并；生成器权威快照为
schema roots=62、adopted contracts=60、witnesses=120。semantic contract migration 为
adopted/generated/planned=60/2/27；surface migration 为
adopted/generated/planned/excluded=1/31/187/5；endpoint obligation histogram 为
Contract/Todo/NotApplicable/Excluded=60/320/124/0。unfinished=567 由 semantic
generated+planned 29、surface generated+planned 218 与 endpoint Todo 320 相加得到。各 endpoint
仍因 headers Todo 保持 Generated；合并未关闭 get-run-log、transition、SSE 或其它后续义务。
### B3-C1 lifecycle transport 第一批冻结

第一批选择 specify、promote、reopen、unblock、archive 五个不返回 claim token 的 task lifecycle endpoint，分别采用 operation-specific path 与 success exact roots；既有 request roots 保持 adopted。真实 Axum handler 使用 contract path DTO 和 operation-specific response alias，Desktop 对 DataEnvelope<ApiTask> 做 outer/nested exact parsing，并拒绝额外 envelope、错误 data 类型、缺失 required-nullable 字段和 claim_token 泄漏。

五个 endpoint 仍保持 Generated，因为 headers 与 query obligations 均继续为 Todo；本批 10 个新 roots 均为 Adopted，登记 20 个方向独立的 producer/consumer witnesses。权威快照为 schema roots=42、adopted contracts=40、witnesses=80、endpoint Contract=40、Todo=355、NotApplicable=109、unfinished=602（inventory generated/planned/todo 为 29/218/355）。剩余 claim、reclaim、heartbeat、complete、submit-review、block 独立进入后续 cohort，以单独审计 claim-token、CAS、force、actor 与状态机 guard。

### B5-C1 maintenance auxiliary

- B5-C1 auxiliary 首批选择 `POST /api/v1/maintenance/doctor` 与
  `POST /api/v1/maintenance/checkpoint`：两者共享单一 SQLite maintenance service 边界、没有
  path/query/body 输入，也不引入 search/context/helper provider 的重型 feature 组合。两项
  success response 已迁入 contract-owned exact DTO，并由真实 router producer、独立 fixture
  consumer 与 Desktop fail-closed parser 共同约束；service 仍拥有 SQLite integrity、derived
  store 诊断与 WAL checkpoint 语义。endpoint 因 headers obligation 尚未收敛保持
  `generated`。后续 auxiliary 拆分应把 search、context、graph/vector status 与 events/SSE
  分成独立 cohort，避免把 provider feature、helper lifecycle 和 streaming transport 混入本批。
## B6-C1 SSE finite stream

`GET /api/v1/stream/events` 采用 endpoint-specific `StreamEventsQuery` 与
`StreamEventData` 两个 exact roots。真实 router producer/consumer 锁定 query、严格 payload、
`event -> id -> data` frame 顺序以及有限 snapshot 关闭语义；未知/重复 query 继续返回可本地化
的 `400 invalid_input`。V1 明确忽略 `Last-Event-ID` 且不产生 heartbeat/comment frame，故
headers obligation 以运行时测试和 API SPEC 为证据标记 `Excluded`，不虚构 typed header 或
heartbeat JSON contract。该 endpoint 六项 obligation 已闭合并提升为 `Adopted`。本批权威
快照：schema roots=34、adopted contracts=32、witnesses=64、endpoint Contract=32、Todo=360、
NotApplicable=111、Excluded=1、unfinished=605（inventory generated/planned/todo 为
28/217/360）。

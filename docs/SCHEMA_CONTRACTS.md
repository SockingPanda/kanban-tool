# JSON Schema 契约

## 1. 当前权威来源

`kanban-contract` 承载公开机器契约目录、wire DTO 和 JSON Schema
根注册表。`kanban-schema-tool` 叶子 crate 独占二进制程序、离线校验、artifact 与 hash/drift
工具。状态必须区分：

- Rust 类型已经可以生成 schema。
- 运行时适配器已经实际使用该类型生产或消费 JSON。

API error/health response、label semantics delete response 与 decision metadata input
均为 `adopted`。API error 的 `code` 使用闭合的 `ApiErrorCode` snake_case 枚举；
status 与依赖 locale 的 `message` 仍由 server adapter/core 错误渲染决定。运行时行为以真实
adapter 为权威；不能因为 `kanban-contract` 中存在同形 DTO 就宣称已采用该契约。

提交的 schema 由 Rust 类型确定性生成。`schemas/fixtures/**` 是手工提交且同时经过
Serde/JSON Schema 测试的权威示例。采用证据必须按方向分工：
`Deserialize` 请求的 producer 由真实 contract DTO 程序化构造并序列化，结果与已提交的
有效 fixture 精确相等；consumer 从该 fixture 反序列化，并通过真实运行时 router/handler。
`Serialize` 响应的 producer 才来自真实 adapter 响应路径。producer/consumer 不得
共用同一个 exercise helper 或仅靠测试名伪装独立证据。每个 witness 必须包含 `operation`、
`contract_id`、`surface`、`direction`、`package`、`test_target` 和 `exact_test`。

语义权威保持分层：

- wire DTO 与 schema：字段、类型、必填/可选、未知字段策略和基础值域。
- `docs/API_SPEC.md`、`docs/CLI_SPEC.md`：operation、HTTP/退出码、stdout/stderr
  和用户可见行为。
- `kanban-sqlite::service` 与 `kanban-core`：事务、状态机、CAS、依赖、
  重新计算和结构化元数据的跨字段业务保护。

schema 校验通过不代表业务命令可以执行；业务测试不能被 schema fixture 替代。

## 2. 契约状态

`operation_inventory()` 中每个 semantic contract 都必须使用以下状态之一：

| 状态 | 含义 | 必备证据 |
|---|---|---|
| `planned` | 已识别精确边界，尚未生成 root | 不允许伪填 schema、fixture、采用或排除 |
| `generated` | Rust 类型、root 和手工 schema fixture 已存在 | schema ID、正例 fixture；不得声明运行时采用 |
| `adopted` | 真实 producer/consumer 已切到同一 contract | schema fixture、真实 producer fixture、双方结构化且可执行的精确测试 witness |
| `excluded` | 明确不是稳定 JSON contract | 具体排除理由，不能同时声明 schema 或 fixture |

`schema-audit-closed` 只允许 `adopted` 和 `excluded`。它同时拒绝接口族、
通配符和双向捷径；双向协议必须拆成精确的 input/output contract。
因此“生成了 schema”永远不能代替“运行时已采用”。

以下是结构根的代表性类别，不是当前 485 个根的完整清单：

- 基础契约：API 错误响应、`GET /health` 响应、标签语义删除响应和决策评论元数据输入。
- 生命周期请求：`SpecifyTaskRequest`、`PromoteTaskRequest`、`ClaimTaskRequest`、
  `ReclaimTaskRequest`、`HeartbeatTaskRequest`、`CompleteTaskRequest`、
  `SubmitReviewTaskRequest`、`BlockTaskRequest`、`UnblockTaskRequest`、
  `ReopenTaskRequest`、`ArchiveTaskRequest`、`ArchiveBoardRequest`、
  `AddDependencyRequest`。
- 任务读取请求：`GET /api/v1/boards/:board/tasks` 与
  `GET /api/v1/boards/:board/tasks/by-status` 各自独立的 path/query DTO，共 4 个精确 root；
  query schema 对可重复字段同时声明 `uniqueItems` 与 9/4/3/32 `maxItems`，并冻结
  `q=1024`、`assignee=128`、单个 `label=128` 的 `maxLength`、label 的 Unicode
  `White_Space` 反集 pattern 及 `limit=1000`。raw 8192-byte cap 与由各字段预算推导出的
  54 对上限由 server 运行时门禁负责；标准表单编码的 UTF-8/保留字符 fixture、
  Unicode 纯空白负例与非默认 board 哨兵证明真实 producer/consumer。
- 看板端点：list query、create request、get/archive path 与四个端点专属成功
  response，共 8 个精确 root；四个 success root 只共享闭合的 `ApiBoard` 组件。

当前权威快照有 485 个 schema root：485 个 `adopted`、0 个 `generated`、
0 个 `planned`、0 个 `excluded`，并登记 970 个结构化 witness。117 个有限 JSON CLI
叶子命令均绑定到精确输出 root；export stdout JSONL 流不属于有限 envelope。21 个
JSONL discriminator 的 input/output 分别拥有精确 root，记录数据使用闭合的自然 JSON；
required-nullable 键禁止省略，但接受显式 `null`。CLI task/step/run adapter 会丢弃仅供持久层
使用的 `claim_token` 与内部 `log_path`，包括递归 linked task；dependency、events 与 helper
subprocess protocol 仍由各自组件负责，公开 CLI 契约只拥有最终 stdout shape。

配置与辅助进程拥有 2 个 TOML 配置输入、7 个 graph helper 响应、12 个既有 vector helper
响应契约，以及 adopted 的 Projection v2 request/response helper 协议。worker profile 输入只约束 CLI 选中的 `[workers.<profile>]` 配置节；未选配置节
保持不透明并允许向前兼容，选中配置节严格拒绝未知或非法字段。真实配置解码器、子进程
适配器和协议解码器分别提供 producer/consumer witness，schema 工具依赖仍隔离在叶子 crate。

`surface_operation_catalog()` 是独立维度：250 个 `adopted`、0 个 `generated`、
0 个 `planned`、5 个 `excluded`。其中 CLI 为 117 个 `adopted`、5 个非 JSON
`excluded`，21 个 JSONL record surfaces 与 6 个 structured metadata surfaces 全部为
`adopted`，Config/Helper 为 2/20 个 `adopted`；API 为 83 个 `adopted`，SSE 为 1 个
`adopted`。端点义务直方图同样独立：296 个
`Contract`、0 个 `Todo`、207 个 `NotApplicable`、1 个有运行时证据的 `Excluded`。
`schema-check` 的未闭合项为 0：semantic generated/planned 0 + surface
generated/planned 0 + 端点 Todo 0。

83 个非 SSE 端点各有一个 operation-specific 精确 header root，并按真实 router 行为复用
五种闭合 wire 配置。所有配置都接受可选
`Accept-Language`；actor mutation 增加可选 `X-KB-Actor`；required/optional JSON body 分别使用
`RequiredOne`/`OptionalOne` 的 `Content-Type`，无 body 的端点不声明该参数。83 个 root 均以
配置 fixture producer 和真实 router consumer 作为结构化 witness。SSE
`Last-Event-ID` 保持有运行时证据的 `Excluded`。由此 API endpoint catalog 已无 `Todo`；全局
`schema-audit-closed` 已无 semantic、surface 或端点权威缺口。

## 3. 精确公开面目录

`surface_operation_catalog()` 记录可以自动发现的公开传输操作：

- API：83 个 JSON method/path，加 1 个 SSE method/path。
- CLI：122 个 Clap 叶子命令；非 JSON 文本/守护进程/hook 协议逐项 `excluded`。
- JSONL：21 个精确 `type=<discriminator>`。
- Metadata：6 个无传输的精确结构化元数据操作。

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
catalog 和契约状态被有意更新。`/api/v1/**`、`kanban ** --json` 或一个
bidirectional family 不能用于关闭这些 operation。

## 4. 依赖边界

| 构建模式 | 启用的依赖 | 用途 |
|---|---|---|
| `kanban-contract` 默认模式 | `serde`、`serde_json` | contract 数据类型与状态目录 |
| `kanban-contract/schema` | 默认模式 + `schemars 1.2.1` | 从 Rust DTO 生成 schema 文档 |
| `kanban-schema-tool` | `kanban-contract/schema` + `jsonschema 0.47.0` + `sha2` | 离线 metaschema、fixture、manifest 和漂移门禁 |

叶子工具的直接依赖拓扑精确锁定为 5 条普通边：
`jsonschema`、`kanban-contract`、`serde`、`serde_json` 与 `sha2`。它们必须来自 root
workspace canonical 声明，且 source/path、version requirement、default feature、feature set、
alias、optional 与 target signature 全部一致；tool 不得声明 dev、build 或 target-specific
dependency。除 tool 自身外，任何 workspace member 都不得通过 normal/dev/build、alias、
optional 或 target-specific 直接边引用它。结构化 manifest 策略锁定权威声明；
metadata policy 必须从 `crates/kanban-schema-tool/Cargo.toml` 运行 full locked graph，不得使用
`--no-deps`，并 fail-closed 校验 `resolve.root`、package/node 唯一性、tool/contract canonical
package ID 与 manifest path、五条 resolved direct edge 及 tool-root reachable closure。除当前
workspace tool/contract 外，closure 的每个 package 都必须来自 crates.io；path/git direct 或
transitive override 都失败。

`policy/schema-tool-registry-closure.json` 是唯一的 registry 闭包批准记录。它用
`format_version = 1`、`lockfile_version = 4`、`root_package = "kanban-schema-tool"`
和 canonical `packages[]` 表达当前 reachable registry set；每项字段必须精确为
`name`、`version`、`source`、`checksum`，按 `(name, version, source)` 排序，未知字段、
重复、缺失、额外项、非 canonical 顺序和 checksum 漂移全部失败。policy 解析真实
`Cargo.lock` 并双向比较，但普通 gate 永不自动写入或 bless approval。该边界检测
已提交 lockfile 相对批准快照的 identity/checksum 漂移；Cargo fetch/build
另行按 registry index `cksum` 验证 crate 内容。

Cargo metadata 的 `SourceId` 仅作为不透明标识：本项目锁定指定 toolchain 下批准的
逻辑 SourceId 字符串，不把其 URL 字符串当成 Cargo 的通用权威网络 URL；
物理下载允许 Cargo source replacement mirror。六个产品 graph 的真实 `cargo tree` 另行负责
all-features/all-target normal runtime 传递性泄漏扫描，不能替代 dev/build direct-edge
检查。若需改变拓扑，必须先形成新决策并显式更新 gate，不能通过 manifest、
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

## 5. 根契约

- 方言固定为 JSON Schema Draft 2020-12，不依赖库默认值。
- request/input 使用 `SchemaSettings::for_deserialize()`。
- response/output 使用 `SchemaSettings::for_serialize()`。
- root ID 固定为 `urn:kanban-tool:schema:<surface>:<semantic-name>:v1`。
- root 主版本与 crate 版本、API route 版本解耦；破坏性 wire contract 提升 root
  主版本，并同时删除被替代 artifact，不保留兼容双轨。
- schema 必须自包含；只允许 `#/$defs/...` 形式的本地 `$ref`。
- 产物不得包含时间戳、主机名、绝对路径或联网引用。
- `DecisionMetadata.risk` / `verification` 允许缺失，但显式 `null` 同时被真实
  Serde DTO 和 JSON Schema 拒绝。

Decision metadata 的现有 service 语义允许未知顶层/option 字段，所以该 root 是
类型化开放契约：已知字段被验证，扩展字段被保留。selected 必须匹配 option、
slug 唯一、纯空白字符串拒绝等跨字段/业务约束仍由现有 service guard 负责。

## 6. 已提交目录结构

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
起始 fence：

- `schema-doc` 同时声明 exact `contract` 与其 manifest-owned positive `fixture`；inline JSON
  必须可解析且与该 fixture 的 JSON value 完全一致。
- `schema-doc-ignore` 必须填写非空理由，只用于片段、伪值或其它有意不作为完整 wire
  example 的说明性 payload。

`schema-docs` 会拒绝未标记 fence、malformed/orphan marker、未知 contract、fixture mapping
漂移、无效 JSON 与 inline/fixture mismatch。新增公开示例不能依赖“看起来相似”的手工
payload；要么复用 committed canonical fixture，要么明确说明为何只是 illustrative fragment。

## 7. 命令

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
  14 步顺序、Projection release cohort 和 `test-full` 的 nextest/fallback 双分支。mutation tests 必须拒绝
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
- `projection-release-cohort` 对 `kanban-cli` 与 `kanban-server` 显式启用
  `tantivy-backend,oxigraph-backend`，分别执行完整测试和 clippy；默认产品依赖图仍由
  helper isolation gate 证明不携带 Oxigraph/LanceDB 重型 helper，不能用
  `--all-features` 混淆默认隔离与发布能力。
- `release` 精确依次调用 `affected-self-test`、`schema-contract`、`audit`、`rust-full`、
  `projection-release-cohort`、`bench-check`、`target-tools`、`cli-package`、`cli-package-layout`、
  `desktop-package-config`、`desktop-package`、`desktop-package-layout`、`smoke` 与
  `diff-check`；AST + ordered trace 对删除或重排 fail closed。`cli-package` 使用
  `--no-default-features --features tantivy-backend,oxigraph-backend` 构建主 CLI，
  并继续把独立 LanceDB/Oxigraph helper binaries 一并装入发布包。

所有会写 Cargo target 的命令必须通过这些 `just` recipes 和仓库 build lock 运行。

## 8. 采用检查清单

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


## 9. 传输描述符权威

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
obligation，也不能单独决定整个 endpoint 的 migration state。当前 `api.error.response`
使用 `location=Error` 并显式链接到 8 个 endpoint；它仍不计入 success exact coverage。
这些 endpoint 依靠各自完整的 exact obligations 达到 `Adopted`。

生命周期请求采用 13 个独立 DTO，不提供通用 transition/token body。所有 DTO
拒绝未知顶层字段；`ClaimTaskRequest.metadata` 与 `CompleteTaskRequest.result` 仅保持
`serde_json::Value` opaque extension，`SubmitReviewTaskRequest` 则完全不接受 `result`。
`ReclaimTaskRequest.to_status` 是封闭的 `ready|blocked` 枚举。Promote、reclaim、unblock、
task archive 和 board archive 保留 optional body 与既有默认值，actor 仍按 body、
`X-KB-Actor`、server default 的优先级解析。

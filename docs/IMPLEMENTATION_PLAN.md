# Implementation Plan

本文档给出分阶段实现计划、验收标准和测试策略。

---

## Phase 0：Repository Skeleton

目标：建立 Rust workspace 和基本工程纪律。

交付：

- `Cargo.toml` workspace。
- crates：`kanban-core`、`kanban-sqlite`、`kanban-cli`。
- migrations 目录。
- lint/format/test workflow。
- error type。
- ID/time utility。

验收：

- `cargo test` 通过。
- `cargo fmt --check` 通过。
- `cargo clippy` 无关键 warning。

---

## Phase 1：SQLite Schema + Core Domain

目标：数据结构和 migration 可用。

交付：

- 执行 `001_initial.sql`。
- `kanban init`。
- 默认 board。
- 默认 columns。
- 领域类型：Board、Task、Status、Run、Event。
- status enum 与 parse/serialize。

验收：

- 新建 DB 后 `PRAGMA integrity_check` 返回 ok。
- 重复运行 `kanban init` 不破坏已有 DB。
- schema version 可查询。

测试：

- migration test。
- schema smoke test。
- enum roundtrip test。

---

## Phase 2：Task CRUD + Events

目标：任务可创建、查询、更新，事件可记录。

交付：

- `kanban task create/list/show/update`。
- `task_events` 写入。
- `--json` 输出。
- `expected_lock_version` 乐观锁。

验收：

- 创建 task 后有 `task.created` event。
- update 不允许修改 status。
- board task list 默认隐藏 archived。

测试：

- create/list/show integration test。
- event transaction test。
- invalid input test。

---

## Phase 3：State Machine Transitions

目标：核心状态机可用。

交付：

- specify。
- promote。
- claim/start。
- heartbeat。
- complete/done。
- block。
- unblock。
- reclaim。
- archive。

验收：

- 非法 transition 被拒绝。
- 每个 transition 写 event。
- running task 必须有 claim token。
- complete 后清理 claim fields。

测试：

- transition matrix unit tests。
- block/unblock target recomputation tests。
- token mismatch tests。

---

## Phase 4：Dependencies

目标：支持 parent/child 依赖和显式 manual promotion。

交付：

- `kanban dep add/remove/list`。
- cycle detection。
- dependency-aware create/promote/claim。
- parent complete 后不自动 promote children；child 保持 `todo`，由 derived dependency fields 表达是否仍被阻塞。

验收：

- child 依赖未完成 parent 时不能 ready/running。
- parent 完成后 child 可被手动 promotion。
- cycle 添加失败。

测试：

- direct cycle。
- indirect cycle。
- child demotion when dependency added。
- manual promotion after completion。

---

## Phase 5：Runs + Dispatcher MVP

目标：可执行任务并恢复崩溃任务。

交付：

- `task_runs` 写入。
- `kanban dispatch --once`。
- `kanban dispatch` loop。
- worker profile command。
- heartbeat wrapper。
- expired reclaim。
- run logs。

验收：

- ready task 被 dispatcher claim 并执行。
- command exit 0 后 task done/review。
- command exit non-zero 后 task blocked/ready。
- worker timeout 后 reclaim/block。
- dispatcher crash 后 task 可被 reclaim。

测试：

- claim race test，多线程同时 claim 同 task 只有一个成功。
- worker success integration。
- worker failure integration。
- expired claim reclaim test。

---

## Phase 6：Local Web API

目标：Web 端可通过 HTTP 操作 board/task。

交付：

- `kanban serve`。
- REST endpoints。
- unified error response。
- SSE event stream。
- health endpoint。

验收：

- Web API 能完成 CLI 同等生命周期。
- SSE 收到 task events。
- API 不能 PATCH status。
- 默认只监听 127.0.0.1。

测试：

- route integration tests。
- API transition tests。
- SSE reconnect test。

---

## Phase 6.5：Board Lifecycle MVP

目标：让单 SQLite DB 内的多个 board 可通过 CLI/API 创建、选择、归档，并让 task ref 兼容未来聚合视图。

交付：

- `kanban board list/create/show/use/current/archive`。
- `POST /api/v1/boards` 与 `POST /api/v1/boards/{board}/archive`。
- 项目级 `.kb/config.toml` active board，解析顺序为 `--board`、`KB_BOARD`、最近项目 config、`default`。
- task ref 支持全局 `t_...`、当前 board 的裸 seq / `#seq`、显式 `board#seq` / `board/#seq`。
- CLI/API task 输出包含 `board_slug` 和可复制 `ref`。

验收：

- `board create` 创建默认 columns 并写 `board.created` event。
- `board use` 写入项目级 `.kb/config.toml`。
- `t_...` 可跨 active board resolve，`board#seq` 可显式跨 board resolve。
- Archived board 默认隐藏且拒绝普通写入；events/runs/comments 历史仍可读。
- Board archive 在存在 `running` task/run 时被拒绝。
- Duplicate board slug 返回 user-facing invalid input / HTTP 400。

---

## Phase 7：Web UI MVP

目标：基本看板 UI 可用。

交付：

- Board columns。
- Task cards。
- Task detail drawer。
- Create/update task。
- Drag/drop 调用 transition。
- Comments。
- Event timeline。
- Run history。
- SSE live refresh。

验收：

- UI 不直接修改 status。
- blocked unblock 后根据服务端返回移动列。
- running complete force 时有确认。

---

## Phase 8：Maintenance & Hardening

目标：本地数据可靠性。

交付：

- `kanban doctor`。
- `kanban backup`。
- `kanban checkpoint`。
- `kanban vacuum`。
- JSONL export。
- orphan run 检查。

验收：

- backup 可恢复。
- integrity check 可报告问题。
- expired running task 可列出并 reclaim。

---

## Testing Strategy

### Unit Tests

- status parse/format。
- transition guard。
- initial status computation。
- unblock target computation。
- dependency cycle detection。

### SQLite Integration Tests

- migration fresh DB。
- migration idempotency。
- FK constraints。
- JSON validation constraints。
- CAS claim affected rows。

### Concurrency Tests

- 10/50/100 concurrent claim attempts on one task。
- CLI + server simultaneous writes。
- busy timeout behavior。

### Dispatcher Tests

- successful worker。
- failed worker。
- timeout worker。
- heartbeat extension。
- reclaim expired。

### CLI Golden Tests

- human output stable enough。
- JSON output exact contract。
- exit code mapping。

### API Tests

- task lifecycle。
- invalid transition HTTP 409。
- error body format。
- SSE stream ordering。

---

## Definition of Done for MVP

MVP 完成定义：

1. `kanban init` 创建可用 DB。
2. `kanban board list/create/show/use/current/archive` 可用。
3. `kanban task create/list/show/update` 可用，输出包含 `board#seq` ref。
4. `kanban task start/heartbeat/done/block/unblock/archive` 可用。
5. dependencies 可用。
6. events 可用。
7. runs 可用。
8. dispatcher 可执行本地命令。
9. web API 覆盖核心 lifecycle。
10. web UI 可视化 board。
11. 并发 claim 测试稳定通过。
12. `kanban doctor` 能发现基本数据异常。

---

## Recommended First Milestone

最小可用 milestone：

```text
Phase 0 + Phase 1 + Phase 2 + Phase 3 + 部分 Phase 4
```

即：

- SQLite schema。
- CLI task lifecycle。
- 状态机。
- events。
- dependencies 基础。

先不要做 Web UI 和 dispatcher，直到状态机与 schema 稳定。


### Phase 2 transport identity 收敛

已完成 API/SSE method/path authority：84 个 descriptor（83 API + 1 SSE）和真实 router binding 的双向 parity 已锁定。后续 DTO adoption 必须复用 descriptor 的 `operation_id` 与 obligation，逐项把 `Todo` 收敛为 `Contract`、`NotApplicable` 或带理由 `Excluded`；不得重新引入 server 或 surface catalog 的手写 API/SSE path 表。

B1-A 已完成 wire 收口：API error response 已使用闭合 `ApiErrorCode`，label semantics delete handler 已使用 `DeleteResponse`/`DeleteResult` 并有真实 producer、typed consumer、schema 与 AST ownership evidence。delete endpoint 的 path/query/header/body obligation 尚未逐项建模，故 endpoint 和 response migration 继续标为 `generated`；下一步应先建模并验证这些请求义务，再考虑 adoption。

B1-C0 已完成 transport proof foundation：全部现有 contract 都显式区分 HTTP transport 与 `NoTransport`；HTTP contract 记录 location 与参数 cardinality，`Success`/`Error` 分别表示 2xx exact success 与 shared non-2xx response。任意 `Adopted` contract 及 endpoint exact reference 都要求 `granularity=Exact`；endpoint exact 唯一性由唯一 method/path、精确 `operation_key` 和 location 结构性保证。shared component 的多 endpoint linkage、linkage OR witness orphan policy 和 exact-miscount 均由 mutation tests fail closed。该阶段没有迁移 handler DTO，也没有关闭任何 endpoint `Todo`；冻结值保持 SSE `Todo`、endpoint `Todo=389`、总未闭合 `636`。

B1-C1 已完成两个 board task-read endpoint 的 path/query transport adoption：4 个 endpoint-specific
exact contract 拥有独立 DTO、schema、正负 fixture、DTO producer、非默认 board sentinel 的真实
router consumer 与 AST ownership/mutation evidence。两个 server-local typed extractor 各自绑定
path，并各自只调用一次共享 ordered raw-query parser；handler 不持有 raw URI。真实 URI matrix
锁定 8192 bytes raw 总预算、由 9/4/3/32 repeated 与 6 scalar 推导出的 54-pair cap、
RepeatedOrdered distinct 语义、1024/128/128 Unicode 字符预算、label 的 raw 128 字符预算
包含随后会被 trim 的 Unicode 边缘空白、纯 Unicode 空白 label 拒绝、
唯一 application/service/contract limit authority 链、全部 filter 转发、标准 form encoding 与
max/max+1。Desktop caller 继续使用 `URLSearchParams` 标准 form encoder，并以保留字符/UTF-8
断言锁定只发送 `q` 而不恢复 `search`。本阶段仅将 SQLite service 的 `MAX_TASK_LIST_LIMIT`
改为直接复用 application authority；service 查询行为与 `kanban-core` 状态机未改变。GET body
是 `NotApplicable`，headers/success 保持 `Todo`，因此两个 endpoint
仍为 `Generated`。冻结值保持 endpoint `Contract=19`、`Todo=383`、`NotApplicable=102`，
总未闭合 `630`。


## B1-C2b task-read 成功响应验收

验收要求：两个 endpoint 各有 response root/schema/正负 fixture/producer-consumer evidence；共享 `ApiTask`/`ApiLabel` 与既有 pagination primitives；Desktop 对这两个 endpoint exact recursive fail-closed，hostile payload 返回 `invalid_response`；`just schema-contract`、`just desktop-check` 与 affected gate 通过。权威行为见 [API_SPEC](API_SPEC.md#b1-c2b-task-read-成功响应契约) 与 [SCHEMA_CONTRACTS](SCHEMA_CONTRACTS.md#b1-c2b-task-read-成功响应契约)。


### B2-C3 comments pair acceptance

- list/create comment 各自拥有 exact path 与 success root，create 拥有 exact request root。
- 真实 non-default-board route 生成 committed fixtures；contract 与 Desktop 分别独立消费。
- private `CommentBody`/`CommentDto` 归零，handler 经共享 SQLite service path 显式适配 `ApiComment`。
- board isolation、archived-board write guard、decision metadata、event transaction 与 locale/status
  继续由既有 service/API tests 锁定。

B2-C3 review closure：五个 comments roots 均提升为有 structured witnesses 的 Adopted；endpoint 仍 Generated、headers Todo。验收额外锁定 AST canonical tail/adapter、防 bypass mutations、Desktop create hostile/error transport、comment/event rollback 与完整 identity/kind matrix；权威统计见 SCHEMA_CONTRACTS。

### B2-C4 run reads acceptance

- 仅迁移 list-runs/get-run；get-run-log、transition、create-task 与 steps 不进入本批。
- contract-owned `ApiRun`/`ApiClaim` 取代 private `RunDto`/`ClaimDto`；共享 adapter 隐去
  `RunRecord.claim_token` 与 `log_path`。
- 四个 roots 以 non-default-board 真实 claim/complete/reopen/claim lifecycle 产生 active 与
  finished run fixtures，并分别由 contract root 独立消费。
- Server AST mutation、board isolation、archived history、not-found/privacy，以及 Desktop exact
  parser/transport/hostile/error tests 构成运行时 adoption 证据。
## B2-C5 create-task contract checkpoint

- `POST /api/v1/boards/:board/tasks` 的 path/request/success 三项 contract 已采用，headers 保持
  `Todo`，endpoint 诚实保持 `Generated`。
- contract owner 提供 create-only status、opaque object metadata 与闭合 `ApiTask` response；
  server 删除 private `CreateTaskBody`，Desktop 使用 exact response parser。
- 真实 router、transaction rollback/retry/readiness guard、schema witness 与 Desktop consumer 是
  本切片验收面；不改变 SQLite/core authority，也不提前关闭其它 CRUD endpoint。
### B2-C6 boards endpoints acceptance

- list query、create request、get/archive path 与四个 success response 各有 exact root、schema、
  正负 fixture 和独立 producer/consumer witness；archive body 继续复用既有
  `ArchiveBoardRequest` owner。
- handler 删除 private `CreateBoardBody` 和 `BoardRecord` wire 泄漏，经
  `kanban_sqlite::api` 显式映射到 contract-owned `ApiBoard`；真实 fixtures 使用 non-default board。
- list 的 `include_archived` 必须真实转发到 service options；archive running guard、archived
  history/read、404/status/i18n 不得改变。
- Desktop `listBoards` production caller 使用 endpoint exact parser；AST ownership 与 mutation
  gate 阻止 private DTO、错误 response root、默认 board/path 绕过或 private service/adapter。

B2-C6 完成时 8 个新 roots 为 Adopted；四个 endpoint 因 headers 仍为 Todo 而保持 Generated。
权威统计见 SCHEMA_CONTRACTS。

### B7 API transport header closure

- 83 个 non-SSE endpoint 各自拥有 exact header contract，统一复用五种 locale/actor/body
  cardinality profile；endpoint obligation `Todo` 已归零。
- 真实 router gate 锁定 locale、body actor 优先级、header actor fallback、required/optional/no-body
  `Content-Type` 行为；SSE `Last-Event-ID` 继续保持明确 exclusion。
- schema roots=301，semantic adopted/generated/planned=222/79/26，surface
  adopted/generated/planned/excluded=2/82/135/5，unfinished=322。
- 该阶段只关闭 API endpoint catalog，不等于全局 `schema-audit-closed`；后续继续处理 semantic
  generated/planned 与 CLI/JSONL/helper surface obligations。
### B2-C7 steps family acceptance

- list/create/update/remove/done/skip/reopen 共 19 个 endpoint-specific path/request/success roots 全部 `Adopted`，headers 保持 `Todo`，endpoint 保持 `Generated`。
- server 删除 private step DTO/request owners，以 contract DTO 作为唯一 wire owner，同时保留原 SQLite service path、required/optional/resolution/status/plan 与 transition guards。
- committed fixtures 由非默认 board 的真实 router producer 证明；request/path 使用程序化 DTO producer 与独立真实 router consumer，Desktop 七个 production callers 使用递归 exact consumer。
- syn AST ownership mutation、schema shape、既有 required-step/plan transition service tests、`schema-contract`、server、web 与 affected gates 共同构成验收证据。

### B2 integrated train acceptance

Create-task、boards 与 steps 的全部 exact roots、真实 router witnesses、Desktop consumers 和 service guards 已按合集重新生成并验证；集成冻结为 62 roots / 60 adopted contracts / 120 witnesses / 567 unfinished。后续 B2 切片必须从该 train 快照继续，不得回退到任一独立 lane 的局部统计。

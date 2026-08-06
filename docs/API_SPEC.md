# 本地 HTTP API 规范

`kanban serve` 提供本机 application API。CLI、MCP、Desktop 只能通过 typed localhost HTTP/SSE client 调用它；它们不打开数据库，也不各自实现状态转换。默认地址为 `http://127.0.0.1:8721`，产品路由前缀为 `/api/v1`，健康路由为 `/health`。

## 1. 通用契约

- 请求/响应使用 JSON；成功 envelope 为 `{ "data": ... }`，列表可带 `meta`。
- server 只绑定 loopback；client 拒绝非 loopback URL。host 停止时返回 `server_unavailable`，不会 fallback。
- mutation actor 依次来自 body `actor`、`X-KB-Actor`、host 默认 actor；comment `author` 是命名上的 body 优先级例外。
- `error.code` 是稳定机器字段，`message` 只供人阅读。常见 code：`invalid_input`（400）、`not_found`（404）、`conflict`/`idempotency_conflict`/`dependency_cycle`/`claim_conflict`/`invalid_transition`（409）、`claim_token_mismatch`（403）、`feature_not_available`（501）、`internal`（500）。
- path 中 task 使用全局 `t_...`，run 使用 `r_...`，step 使用 `step_...`；board-local selector 由 typed client 先解析，不在 handler 里复制第二套语义。

<!-- schema-doc-ignore: envelope 说明性示例，不绑定具体 endpoint root。 -->
```json
{"data": {}}
```

## 2. Host、health、boards 和 stats

| Method | Path | 语义 |
| --- | --- | --- |
| `GET` | `/health` | host、Turso path/fingerprint 和版本健康 |
| `GET` | `/api/v1/boards` | board 列表，可 `include_archived` |
| `POST` | `/api/v1/boards` | 创建 board，返回 `CreateBoardResponse` |
| `GET` | `/api/v1/boards/:board` | board 详情 |
| `POST` | `/api/v1/boards/:board/archive` | 显式归档 board，拒绝 active running work |
| `GET` | `/api/v1/boards/:board/columns` | 固定 status columns |
| `GET` | `/api/v1/stats` | board-scoped queue/claim/step 统计 |

`board_columns` 只做展示，不写第二套状态机。

## 3. Host-admin maintenance

以下操作都在唯一 host 的 Turso handle 内执行；backup/export 先 staging、校验、原子 rename，不覆盖既有目标或 symlink。

| Method | Path | 结果 |
| --- | --- | --- |
| `GET` | `/api/v1/maintenance/doctor` | schema/FK/task/run/projection diagnostics |
| `POST` | `/api/v1/maintenance/checkpoint` | WAL/checkpoint report |
| `POST` | `/api/v1/maintenance/backup` | verified backup、SHA-256、bytes |
| `POST` | `/api/v1/maintenance/export` | portable canonical JSONL |
| `POST` | `/api/v1/maintenance/import` | portable import；`replace=true` 走 verified backup + atomic canonical replace |
| `POST` | `/api/v1/maintenance/import-v30` | legacy SQLite v30 importer；需 `legacy-sqlite-import` feature，否则 `feature_not_available` |
| `POST` | `/api/v1/maintenance/vacuum` | host-owned compaction |
| `GET` | `/api/v1/maintenance/status` | owner lease、generation、dirty/error、job count |
| `POST` | `/api/v1/maintenance/run` | action `run|compact|rebuild|cleanup` |
| `POST` | `/api/v1/maintenance/rebuild` | projection rebuild |
| `POST` | `/api/v1/maintenance/cleanup` | 仅清理可重建派生内容 |

`projection_maintenance_owner` 保护并发；portable import 只写 canonical facts，提交后 enqueue FTS/vector/graph rebuild。MCP 不提供这些 host-admin mutations。

## 4. Boards、tasks、plans、steps、dependencies

### Board/task

| Method | Path | 语义 |
| --- | --- | --- |
| `GET` | `/api/v1/boards/:board/tasks` | status/priority/plan/assignee/query/pagination/sort 过滤 |
| `POST` | `/api/v1/boards/:board/tasks` | task create；支持 task/idempotency key、metadata、labels、depends_on |
| `GET` | `/api/v1/tasks/:task_id` | task aggregate；`include=details` 可返回 ontology/labels/dependencies/steps/runs/event meta |
| `PATCH` | `/api/v1/tasks/:task_id` | 只更新 service 允许的内容/排期/metadata 字段，不直接改 status |
| `POST` | `/api/v1/tasks/:task_id/execution-plan/not-required` | 显式 plan gate |

task list/search 默认排除 `archived`；所有 selector、board isolation、idempotency 和 dependency guard 由 service 处理。

### Lifecycle

| Method | Path | 语义 |
| --- | --- | --- |
| `POST` | `/api/v1/tasks/:task_id/transitions/promote` | `todo|scheduled → ready` |
| `POST` | `/api/v1/tasks/:task_id/transitions/specify` | 补充 triage 规格并重算状态 |
| `POST` | `/api/v1/tasks/:task_id/transitions/claim` | 原子 `ready → running`，创建 run + event |
| `POST` | `/api/v1/tasks/:task_id/transitions/heartbeat` | matching token 延长 lease |
| `POST` | `/api/v1/tasks/:task_id/transitions/release` | matching token 取消 run 并回到 ready |
| `POST` | `/api/v1/tasks/:task_id/transitions/submit-review` | running → review，结束 active run |
| `POST` | `/api/v1/tasks/:task_id/transitions/complete` | running/review → done；校验 required steps |
| `POST` | `/api/v1/tasks/:task_id/transitions/block` | 非空 reason，必要时结束 run |
| `POST` | `/api/v1/tasks/:task_id/transitions/unblock` | blocked 后按 canonical facts 重算目标 |
| `POST` | `/api/v1/tasks/:task_id/transitions/reopen` | 保留完成历史并重算活动状态 |
| `POST` | `/api/v1/tasks/:task_id/transitions/reclaim` | expired/force claim 的显式回收 |
| `POST` | `/api/v1/tasks/:task_id/transitions/archive` | 显式归档 guard |

所有 lifecycle mutation 共享 `ApplicationService`；不提供任意目标状态 endpoint。

### Steps/dependencies

| Method | Path | 语义 |
| --- | --- | --- |
| `GET/POST` | `/api/v1/tasks/:task_id/steps` | list/create execution steps |
| `PATCH/DELETE` | `/api/v1/tasks/:task_id/steps/:step_id` | update/remove step |
| `POST` | `/api/v1/tasks/:task_id/steps/:step_id/{done,skip,reopen}` | step lifecycle |
| `GET/POST` | `/api/v1/tasks/:task_id/dependencies` | list/add same-board parent edge |
| `DELETE` | `/api/v1/tasks/:child_task_id/dependencies/:parent_task_id` | remove edge；cycle/FK 在 service 拒绝 |

## 5. Comments、attachments、runs、events

| Method | Path | 语义 |
| --- | --- | --- |
| `GET/POST` | `/api/v1/tasks/:task_id/comments` | note/decision/signal comment；task-local idempotency |
| `GET/POST` | `/api/v1/tasks/:task_id/attachments` | metadata + staged file publish；checksum/path guard |
| `GET` | `/api/v1/tasks/:task_id/attachments/:attachment_id` | 重新校验 size/SHA 后下载 raw bytes |
| `DELETE` | `/api/v1/tasks/:task_id/attachments/:attachment_id` | `.trash/` reversible delete + event |
| `GET` | `/api/v1/tasks/:task_id/runs` | task runs |
| `GET` | `/api/v1/runs/:run_id` | run detail |
| `GET` | `/api/v1/runs/:run_id/log` | 固定 256 KiB bounded log snapshot |
| `GET` | `/api/v1/events` | append-only event list，`after` + `limit` cursor |
| `GET` | `/api/v1/stream/events` | SSE finite event snapshot/stream |

run 没有独立 create/update API；claim 创建 run，后续 lifecycle 同事务更新 run/event。未知 event kind 的 JSON payload 原样保留。

## 6. Labels、ontology、signals

### Labels

| Method | Path | 语义 |
| --- | --- | --- |
| `GET/POST` | `/api/v1/boards/:board/labels` | board label list/create |
| `GET/POST` | `/api/v1/tasks/:task_id/labels` | task label list/add |
| `DELETE` | `/api/v1/tasks/:task_id/labels/:label_id` | task label remove |

### Ontology/atom/proposal

语义、atoms、atom-index、suggestions、proposals 和 ontology ledger 的当前路径包括：

```text
GET/PUT/DELETE /api/v1/boards/:board/labels/{semantics,atoms,atom-index/*}
GET           /api/v1/boards/:board/labels/atoms/:atom_ref/explain
GET           /api/v1/tasks/:task_id/labels/suggestions
GET/POST       /api/v1/tasks/:task_id/label-proposals
GET/POST       /api/v1/tasks/:task_id/label-ontology/observations
GET/POST       /api/v1/boards/:board/label-ontology/{signals,review,actions,apply/atom,revert,validate}
GET/POST       /api/v1/{label-ontology/signals/:signal_id,label-proposals/:proposal_id*}
```

每个 action 由 service 做 CAS、board guard、atom effects、review/validate/revert 和 event；index 是可重建派生状态。

### Generic signals

```text
GET/POST /api/v1/boards/:board/signals
GET      /api/v1/boards/:board/signals/review
POST     /api/v1/boards/:board/signals/{confirm,reject,resolve,supersede}
GET      /api/v1/signals/:signal_id
```

record、backlink comment、review transition 和 `signal.reviewed` event 在同一事务提交。

## 7. Search、FTS、graph、vector、context

### Search

```text
GET  /api/v1/search/tasks
GET  /api/v1/search/tasks/by-status
GET  /api/v1/search/status
POST /api/v1/search/index/rebuild
POST /api/v1/search/index/sync
```

普通文本在 Turso FTS `task_search_fts` ready 时使用 match/score/highlight；exact `t_...`/`board#seq`/`#seq` 走 canonical selector。FTS stale/unavailable 时 service 回退 canonical SQL，并在 `search_meta` 标记 `backend`、`generation`、`fallback_reason`。

### Graph/entity

```text
GET/PUT /api/v1/entities
GET     /api/v1/entities/:uri
GET     /api/v1/graph/status
GET     /api/v1/graph/neighbors
GET     /api/v1/graph/query
POST    /api/v1/graph/rebuild
POST    /api/v1/graph/sync
GET     /api/v1/tasks/:task_id/neighborhood
GET     /api/v1/boards/:board/task-map
```

graph query 使用 canonical `entities`/relations 的 bounded BFS，包含 depth、dedup、cycle 和 board isolation。

### Vector/context

```text
GET  /api/v1/vector/status
POST /api/v1/vector/configure
POST /api/v1/vector/rebuild
POST /api/v1/vector/sync
GET  /api/v1/vector/query-chunks
GET  /api/v1/vector/query-label-atoms
GET  /api/v1/tasks/:task_id/context
```

vector32 查询使用 host Ollama embedding provider；provider outage 返回 typed degraded diagnostics。context pack 按 subject、lexical、graph、vector、budget 和 provenance 稳定合并；不可用 provider 不阻断可用 lexical/canonical 结果。

## 8. Contract 与验证边界

`kanban-protocol::endpoint_catalog()` 是 method/path/DTO/schema descriptor 的权威来源；真实 router、typed client、CLI/MCP/Desktop 和 fixture/adoption witness 必须逐项绑定。catalog 中的 `adopted` 只表示 protocol surface contract 已闭合，不能单独证明运行时 full/adoption gate 已运行。

已有 server/service evidence 包括 task lifecycle、label round-trip、ontology action/revert、signal ledger、FTS capability、graph BFS/rebuild、vector fixture/degraded、context merge、maintenance import/replace 和 Desktop contract tests。完整 schema adoption、surface audit、full package、release 和 PR gate 不由本文档更新自动执行，结果见 parity ledger 的待验收清单。

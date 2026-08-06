# Kanban Tool 产品规范

文档类型：当前实现规范。代码、`kanban-protocol` catalog、真实 router/adapter 和测试优先于历史快照；尚未运行的 adoption/full gate 不在本文中标记为通过。

Kanban Tool 是本地优先、单机、单用户的看板与 durable work queue。canonical 事实只存于 host-owned Turso 数据库；CLI、MCP、Desktop 和 dispatcher 共享同一个 `ApplicationService`、状态机、事务和错误语义。

## 1. 固定执行路径

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP / SSE
      kanban serve（唯一 host）
        ↓
ApplicationService + kanban-core 状态机
        ↓
kanban-service（Turso、schema、migration、projection provider）
        ↓
canonical Turso database
```

硬性规则：

1. 只有 `kanban serve` 可以打开、初始化、迁移、备份、替换和关闭 Turso。
2. CLI、MCP、Desktop 只依赖 `kanban-client` 或 Desktop 的 TS HTTP client；它们不依赖数据库-owning crate，也不在 host 停止时 fallback。
3. 所有 mutation/query 都进入共享 service path。adapter 只解析输入、发送 typed request、映射 error 或渲染输出。
4. `tasks.status` 是事实，board column 只是展示；没有任意 `transition(target_status)`。
5. canonical facts、event 和 projection job enqueue 必须在规定的同一事务边界内提交或回滚。

默认 host 为 `http://127.0.0.1:8721`，默认数据库为 `$XDG_DATA_HOME/kb/kanban.db`；`--db`、`KANBAN_DB`、`KB_DB` 只由 `serve`/配置解析使用。`config show`、`init`、`board use/current`、completion 和 Codex hook 是不触库的本地 shell。

## 2. 产品范围

### 2.1 当前功能域

| 域 | canonical owner | 当前入口 |
| --- | --- | --- |
| boards/tasks/plans/steps/dependencies/comments/runs/events | `kanban-service` | HTTP、typed client、CLI、MCP、Desktop detail/board |
| labels、ontology、signals | `kanban-service` ledger | HTTP、CLI、MCP、Desktop workbench/detail |
| search/index | Turso FTS + host projection worker | HTTP、CLI、MCP、Desktop list/context |
| entities/relations/graph | `kanban-service` canonical relations + bounded BFS | HTTP、CLI、MCP、Desktop map/context |
| vector/context | Turso `vector32` + host Ollama provider + service merge | HTTP、CLI、MCP、Desktop typed API |
| host maintenance/migration | host-owned service/worker | HTTP、CLI、Desktop Maintenance/Health；MCP 不暴露管理写操作 |

以上功能共享同一条事实路径。FTS/vector/graph/context/projection 是可重建派生结果，不是第二事实源。

### 2.2 非目标

- 多用户、团队、邀请、RBAC、多租户、SaaS、云同步、公网监听或远程 worker；
- 第二 canonical backend、CLI/MCP/Desktop 直开数据库、embedded fallback 或自定义 IPC；
- 外部 Tantivy/LanceDB/Oxigraph/helper sidecar 作为 active runtime；旧 sidecar 仅在历史归档与迁移证据中保留；
- 自动 server supervision、`multiprocess_wal`、通用 mutation receipt、发布、push、PR、merge 或 release cohort。

`legacy-sqlite-import` 是 host 的显式可选 feature，不等于第二 runtime backend；它只读 legacy SQLite v30，并把事实导入 canonical Turso。

## 3. 状态与队列

canonical 状态集合：

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

`ready` 表示规格、排期、依赖和 execution plan 均允许执行；只有 `task.claim` 能把它原子地变为 `running`。claim 同事务创建 active run 和 event，最多一个 active run。`heartbeat`、`release`、`review`、`done`、`block`、`specify`、`unblock`、`reopen`、`reclaim`、`archive` 都是显式 typed command；dispatcher 只扫描 `ready`。

状态机完整 guard 和 event 见 [`STATE_MACHINE.md`](STATE_MACHINE.md)。

## 4. Host、adapter 与 worker

### 4.1 HTTP/SSE

`kanban-server` 合并 boards、tasks、steps、comments、attachments、dependencies、entities、graph、search、context、labels、ontology、signals、runs、events、stats、maintenance 和 vector routers；`/health` 与 `/api/v1/stream/events` 也由同一 host 提供。`kanban-client` 只做 typed localhost HTTP/SSE，不复制 SQL 或 fallback。

### 4.2 CLI

CLI 顶层包括 `serve`、board/config/task/label/comment/context/attachment/dep/entity/graph/events/runs/run/search/index/signal/vector、doctor/stats/backup/export/import/import-v30/checkpoint/vacuum/maintenance、init/completions/__complete/hook。所有 domain/host-admin 命令（`serve` 除外）通过 client 请求 host；`import-v30` 未启用 feature 时返回 `feature_not_available`。

当前 canonical leaf 以 `kanban-protocol::surface_operation_catalog()` 与 Clap 的
`get_name()` 对齐。看板列使用 `board columns`，实体写入使用 `entity upsert`，triage
规格补全使用 `task specify`；graph 提供 `neighborhood`/`map`，search index 提供
`index rebuild`/`index sync`。visible alias 不单独产生 contract operation；已清理的旧
projection/admin、独立 lifecycle leaf 和 task-read 旧路径不属于 active surface。

### 4.3 MCP

`kanban-mcp` 是 Rust stdio server。公开工具由
`kanban-protocol::MCP_OPERATION_CATALOG`（`mcp_operation_catalog()`）机器可读目录固定，共
103 个 tool，覆盖全部 102 个非 host-admin HTTP operation；
`MCP_HOST_ADMIN_OPERATION_IDS` 明确禁止 12 个 host-admin operation。MCP 不启动 host、不打开
数据库、不提供 migration/backup/vacuum/replace 管理命令；search/graph/vector 与 label
atom-index 的 domain `rebuild`/`sync` 不属于这 12 个禁止项。

### 4.4 Desktop

Desktop 通过 `KanbanApi` 使用 HTTP，只保留 `apiBaseUrl`、actor、board 等运行时配置。十个导航视图为 `board`、`list`、`map`、`runs`、`events`、`signals`、`ontology`、`maintenance`、`health`、`settings`；task detail 继续承载 attachments、comments、dependencies、steps、runs/events、labels 和 context 的 typed API。危险 maintenance 操作需要二次确认，并展示 `server_unavailable`/`restart_required` 等稳定结果。

### 4.5 Worker

- dispatcher 是 `kanban serve --dispatcher-profile <path>` 启动的同进程单 worker；只 claim `ready`，复用 lifecycle commands。
- projection worker 在同一个 host 生命周期内处理 FTS、vector32/Ollama、relations 和 context 的 `projection_jobs`，支持 generation、fingerprint、lease、重试、degraded 和 rebuild。

两个 worker 都不得打开第二个数据库、维护第二套状态机或直接写 adapter DTO。

## 5. 数据与迁移

schema family 为 `kanban.turso`，当前 lineage 为 v1/v2：queue/history、labels/ontology/signals、entities/relations、retrieval、projection、import journal 和 attachment staging 均由 `kanban-service` 持有。canonical facts 包括 boards/tasks/plans/steps/dependencies/runs/comments/events、labels/ontology/signals、entities/relations、attachment metadata 和导入事实；`retrieval_documents`、`retrieval_vectors`、FTS、vector、BFS 结果及 projection control rows 可删除后重建。

支持两条迁移路径：

1. **Turso v1 → v2 原地升级**：host 校验 family、exact shape、constraints、foreign keys 和 board isolation；创建 verified sibling backup 后运行事务 migration，失败 rollback，重复启动幂等。
2. **portable/legacy 导入**：portable JSONL 只写 canonical facts，按 `import_journal` 记录 fingerprint/staging/phase，提交后入队 derived rebuild；`import-v30` 只读 legacy SQLite v30，attachment 先 staging、checksum/board isolation preflight，显式 feature 未启用则 fail-closed。

backup、export/import、checkpoint、vacuum、`/api/v1/maintenance/rebuild|cleanup` 和数据库
替换都由 host 管理；这里的 maintenance operation 不应与 MCP 可调用的 search/graph/vector
或 label atom-index domain `rebuild`/`sync` 混同。MCP 不承载前述 host-admin 命令。

## 6. 契约和错误

`kanban-protocol` 是 DTO、event payload、错误 envelope、endpoint/surface catalog 和 JSON Schema 的权威来源。错误以稳定 `error.code` 表示，message 不属于机器契约；常见 code 包括 `invalid_input`、`not_found`、`conflict`、`idempotency_conflict`、`dependency_cycle`、`claim_conflict`、`claim_token_mismatch`、`invalid_transition`、`feature_not_available`、`server_unavailable` 和 `internal`。

HTTP/API、CLI output、MCP tool schema 和 Desktop parser 必须引用同一 protocol DTO。本次 by-status
切片已运行 schema adoption witness 与 surface audit；这不替代完整 runtime/package gate，不能
因为 catalog 已有 `adopted` 条目就宣称所有 runtime gate 完成。

## 7. 验收边界

每个功能域的“已接入”至少需要：

1. canonical schema/constraint 与 service operation；
2. HTTP route + typed client；
3. 需要的 CLI/MCP/Desktop entry；
4. producer/consumer fixture、真实 route/adapter 测试和 board/事务负向测试；
5. FTS/vector/graph/context 可从 canonical facts rebuild，且旧 sidecar 不在 active workspace。

本文件只记录当前源码和已有测试事实；最终 adoption/full/schema gate、release、push 和 PR 由单独任务执行并独立报告。

详细 wire 行为见 [`API_SPEC.md`](API_SPEC.md) 与 [`CLI_SPEC.md`](CLI_SPEC.md)；逐域 baseline、owner、入口、迁移规则、实际测试和未完成 gates 见 [`migration/turso-full-feature-parity.md`](migration/turso-full-feature-parity.md)。

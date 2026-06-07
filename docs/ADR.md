# Architecture Decision Records

本文件记录当前 SPEC 的关键架构决策。

---

## ADR-0001：SQLite-only

### Status

Accepted

### Context

项目明确不考虑多用户、多租户、团队协作和远程 worker。核心运行环境是本地单机，同时需要 CLI 和 Web。

### Decision

只支持 SQLite。

默认 DB：

```text
~/.local/share/kb/kb.db
```

可通过 `--db <path>` 指定项目本地 DB。

### Consequences

优点：

- 单 binary 易分发。
- CLI 使用成本低。
- 备份简单。
- 本地事务足够强。
- WAL 支持 reader/writer 并发。

代价：

- 不支持跨机器共享写入。
- 不做 server cluster。
- 一次只有一个 writer。
- 需要控制 transaction 长度。

---

## ADR-0002：Status Enum 是真相，Column 是视图

### Status

Accepted

### Context

传统 Trello-like 工具常把 list/column 视为状态。但本项目需要 dispatcher、claim、heartbeat、reclaim、run history。`running` 不是普通视觉列，而是 claim 成功后的执行状态。

### Decision

`tasks.status` 是 canonical truth。`board_columns` 只是 UI 展示映射。

### Consequences

优点：

- Web、CLI、dispatcher 遵循同一状态机。
- 可保护 `ready -> running`。
- 能支持 review/scheduled/blocked 等非纯视觉状态。

代价：

- 拖拽列不能简单 PATCH status。
- Web UI 需要根据目标列调用 transition endpoint。

---

## ADR-0003：Snapshot + Append-only Events，不做纯 Event Sourcing

### Status

Accepted

### Context

看板 UI 高频查询当前任务列表。纯 event sourcing 会让当前状态查询复杂化，需要重放事件或额外投影。

### Decision

采用：

```text
tasks snapshot + task_events append-only
```

状态变化时，snapshot update 与 event insert 必须在同一 transaction 内完成。

### Consequences

优点：

- 当前 board 查询简单。
- 事件仍可用于审计、SSE、debug。
- 实现复杂度可控。

代价：

- 需要保证 snapshot/event 一致。
- 事件不是唯一事实源。

---

## ADR-0004：CLI 可以直接访问 SQLite，但必须走 Core Service

### Status

Accepted

### Context

如果 CLI 必须依赖常驻 server，会降低本地工具可用性。直接访问 SQLite 更适合脚本和开发流。

### Decision

CLI 可以直接打开 SQLite DB，但只能调用 `kanban-core` service / `kanban-sqlite` repository，不允许绕过状态机执行裸 SQL 修改状态。

### Consequences

优点：

- 不需要 server 即可使用。
- 脚本友好。
- 和 Web 行为一致。

代价：

- 需要处理 CLI/server/dispatcher 同机并发。
- 所有状态逻辑必须集中在 core。

---

## ADR-0005：Actor 是审计字符串，不是用户模型

### Status

Accepted

### Context

项目不做多用户和权限，但仍需要知道某个操作来自谁或哪个 worker。

### Decision

保留 `actor`、`created_by`、`claim_owner` 字段。它们是字符串，不关联 users 表。

### Consequences

优点：

- 保留审计能力。
- 支持 CLI、Web、dispatcher、worker profile 区分来源。
- 不引入 RBAC 复杂度。

代价：

- 不提供权限隔离。
- actor 可被本地调用者伪造，这是预期边界。

---

## ADR-0006：Worker stdout/stderr 存文件，DB 只存摘要与路径

### Status

Accepted

### Context

运行日志可能很大。把日志 blob 放进 SQLite 会影响性能和备份体积。

### Decision

日志写入：

```text
~/.local/state/kb/logs/r_<run_id>.log
```

DB 只存：

- `log_path`
- `summary`
- `error`
- `exit_code`

### Consequences

优点：

- SQLite 保持轻量。
- 日志可直接 tail。
- 备份策略可分开处理 DB 和 logs。

代价：

- 移动 DB 时需要同时移动 logs/attachments。
- log path 需要 doctor 检查。

---

## ADR-0007：默认只监听 localhost

### Status

Accepted

### Context

不做远程服务和多用户登录。暴露到局域网会制造安全边界问题。

### Decision

`kb serve` 默认并且建议只监听：

```text
127.0.0.1:8721
```

MVP 不提供 `0.0.0.0` 远程模式。

### Consequences

优点：

- 无需登录系统。
- 降低误暴露风险。

代价：

- 不能多人访问。
- 不能远程手机/浏览器访问。

---

## ADR-0008：状态变化必须有专用 Transition Command

### Status

Accepted

### Context

直接 PATCH `status` 容易绕过 claim/run/event/dependency guard。

### Decision

禁止普通 update 修改 status。所有状态变化都使用 command：

- specify
- promote
- claim
- heartbeat
- complete
- submit_review
- block
- unblock
- reclaim
- archive

### Consequences

优点：

- 状态机可验证。
- run/claim/event 一致。
- Web/CLI/dispatcher 行为一致。

代价：

- API 数量更多。
- UI 拖拽逻辑更复杂。

---

## ADR-0009：Knowledge Substrate 派生层

### Status

Accepted

### Context

后续搜索、关系扩展、agent context、artifact provenance 和向量召回需要跨 task/run/comment/artifact/skill 的统一身份与派生索引，但不能削弱 SQLite 状态机、claim 和 dependency guard。

### Decision

SQLite 继续作为 operational source of truth。新增：

- `entities`：跨库统一 `kb://...` identity registry。
- `relation_predicates` / `entity_relations`：受控 predicate 与可重建关系镜像。
- `index_outbox`：派生 store 的 at-least-once job surface。
- `derived_store_state`：Tantivy/Oxigraph/LanceDB 等派生层健康和水位。

Tantivy、Oxigraph、LanceDB 都是可重建 derived stores，不参与状态机事务。

### Consequences

优点：

- 后续 graph/vector/context broker 可以接同一 entity/relation contract。
- SQLite 状态机边界保持清楚。
- 派生 store 损坏时可 fallback/rebuild。

代价：

- 需要维护 entity backfill/outbox/derived state。
- 短期内现有 Tantivy state 与新 `derived_store_state` 并存，直到搜索 sync 迁移完成。

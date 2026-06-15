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

`kanban serve` 默认并且建议只监听：

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

`derived_store_state` 的语义是 store 全局状态，不是 board 局部状态：

- `last_event_id` 表示该 store 已成功处理并提交的全局 task event 高水位。成功 sync/rebuild 只能把它单调推进，不能倒退。
- `dirty=true` 表示该 store 仍有未完成 outbox、失败 outbox 或最近一次派生更新失败；即使某个 board 已 sync/rebuild 完成，其他 board 仍有 pending/failed job 时也必须保持 dirty。
- board-scoped sync/rebuild 只清理当前 board 的 outbox job；是否把 `dirty` 置回 false 取决于同一 store target 是否还存在任何 board 的 unfinished outbox。
- `last_error` 记录最近一次 store 级失败证据。成功处理会清除 `last_error`，失败会保持 `dirty=true` 并保留/标记相关 outbox 失败状态。
- `index_outbox` 是恢复和重放入口；`derived_store_state` 是 operator health/watermark 摘要。两者都不能使派生层成为事实源。

### Consequences

优点：

- 后续 graph/vector/context broker 可以接同一 entity/relation contract。
- SQLite 状态机边界保持清楚。
- 派生 store 损坏时可 fallback/rebuild。
- `kanban doctor` / maintenance API 汇总 outbox backlog、dirty stores、last_error 和 failed outbox，用于本地 operator 判断 sync/rebuild，而不是让派生层参与 SQLite 事务。

代价：

- 需要维护 entity backfill/outbox/derived state。
- `derived_store_state` 是派生 store 的主健康/水位记录；Tantivy 的旧 `app_settings` search state 仅保留为兼容 metadata。

---

## ADR-0010：单 DB 多 board 与 CLI task ref

### Status

Accepted

### Context

本地项目需要不同 board/project，但未来也需要聚合视图和跨 board 审计。如果每个项目拆一个 SQLite DB，聚合、搜索、事件和 dispatcher 恢复都会变复杂。另一方面，裸 `#12` 在 shell 中容易被当作注释，且 board-local seq 不能跨 board 唯一。

### Decision

继续使用单 SQLite DB 内多个 board：

- `tasks.id` 是全局唯一 `t_...`。
- `tasks.seq` 只在 `board_id` 内唯一。
- CLI/API 展示 copyable task ref：`board_slug#seq`。
- CLI task ref 支持全局 `t_...`、当前 active board 的 `12` / `#12`、显式 `board#12` / `board/#12` / `b_...#12`。
- Active board 解析顺序是 `--board`、`KB_BOARD`、最近 `.kb/config.toml`、`default`。
- `.kb/config.toml` 只记录当前项目选择的 board，不表示项目拥有独立 DB。
- Board slug 禁用保留 ID 前缀和会破坏 ref 语法的字符。

Archived board 默认不可写；归档只标记 board，不改 task 状态，并拒绝仍有 `running` task/run 的 board。Read-only events/runs/comments 历史保留可查，作为审计入口。

### Consequences

优点：

- 保留未来聚合 board / dashboard 的数据基础。
- `t_...` 可作为脚本稳定全局引用。
- `board#seq` 对人和 shell 都更可复制。
- 项目级 active board 不破坏单 DB 备份、搜索和 dispatcher 语义。

代价：

- CLI 必须维护 task ref parser/resolver。
- Archived board 需要区分 read-only history 与 mutation guard。
- 裸 `#12` 只能作为兼容输入，文档和输出不能依赖它。

---

## ADR-0011：Schema Train 边界：status、type、tags、dependency type 与 decision comments

### Status

Proposed

### Context

`kanban-tool` 接下来会进入一组 schema/model 扩展：

- `task_type`：表达任务是什么类型。
- `dependency_type`：表达任务之间是什么关系。
- tags/labels：表达可搜索、可筛选、可推荐的多维标签。
- comments：承载人和 agent 的协作记录。
- decision comments：记录人或 LLM/agent 在多个方案之间做出的选择。

当前 comment 模型里的 `kind` 混用了两类概念：

- 谁写的：system / worker / agent / user。
- 写的是什么：普通记录 / 决策记录。

这会让后续结构化 decision comment 变脏。需要先把模型边界切开：

- author/source 轴：谁留下了这条 comment。
- content kind 轴：这条 comment 表达什么语义。

本项目是 dogfood local tool，不需要为早期 comment schema 保留沉重兼容层。可以直接修改模型，只要迁移清晰，并让 CLI/API/Desktop 一次性跟上。

### Decision

保留现有核心原则：

- `tasks.status` 继续是唯一 canonical workflow state。
- hard dependency 继续是状态机和 dispatcher guard 的事实来源。
- `task_events` 继续是 append-only audit trail。
- comments 继续承载协作记录，但 comment schema 要拆清楚作者和内容语义。
- 新字段默认不改变状态机、dispatcher claim 或 ready eligibility，除非本 ADR 明确允许。

### Field Responsibilities

| Field / Model | 责任 | 是否影响状态机 | 是否影响 dispatcher | 是否影响 DAG/frontier | 是否用于 search/context/UI |
|---|---|---:|---:|---:|---:|
| `status` | canonical workflow state | 是 | 是 | 是 | 是 |
| `priority` | ready/frontier/dispatcher 的排序权重 | 否 | 是，排序 | 是，排序 | 是 |
| `scheduled_at` | 计划时间，参与 scheduled/ready guard | 是 | 是 | 是 | 是 |
| `due_at` | 截止时间，只展示、筛选、排序 | 否 | 可排序 | 可排序 | 是 |
| `task_type` | 任务类别，例如 bug/feature/research/ops/follow_up | 否 | 否 | 可用于解释/排序，不改变 eligibility | 是 |
| tags/labels | 多标签分类、搜索、推荐和 UI grouping | 否 | 否 | 否，除非未来显式配置排序策略 | 是 |
| `dependency_type` | 依赖边语义，区分 hard block 和 soft relation | 仅 hard block | 仅 hard block | 是，但必须区分 hard/soft | 是 |
| `comment.author_type` | 评论作者角色：`user` 或 `agent` | 否 | 否 | 否 | 是 |
| `comment.author` | 展示名，例如 `kanban-user`、`codex` | 否 | 否 | 否 | 是 |
| `comment.agent_type` | 可选 agent 细分，例如 `codex`、`executor`、`dispatcher` | 否 | 否 | 否 | 是 |
| `comment.kind` | 内容语义：`note` 或 `decision` | 否 | 否 | 否 | 是 |
| `comment.metadata_json` | `comment.kind` 对应的结构化 payload | 否 | 否 | 否 | 是 |
| `event.kind` | append-only audit event 类型 | 否，event 是结果不是输入 | 否 | 否 | 是 |

### Workflow State

`status` 仍然是任务是否可执行、是否被 claim、是否 blocked/review/done 的唯一事实来源。

任何新字段都不能隐式表达状态：

- `task_type=bug` 不表示高优先级。
- tag `blocked` 不表示 task blocked。
- decision selected option 不表示 task done。
- comment 中写 “blocked” 不改变 task status。

状态变化只能通过 transition command。

### Task Type

`task_type` 表达“这个 task 是什么工作类别”，不表达“它现在处于什么执行状态”。

建议第一批 task types：

```text
bug | feature | research | ops | docs | refactor | test | follow_up
```

`task_type` 可以用于：

- Desktop/List/Board 筛选。
- Search/context 过滤。
- DAG/frontier 解释。
- 未来排序加权。

`task_type` 不用于：

- dispatcher claim eligibility。
- 状态机 transition guard。
- hard dependency 判断。
- 替代 tags/labels。

枚举策略：

- 第一版使用受控枚举。
- 后续如需要开放扩展，再单独做 ADR。
- 未知 type 应被拒绝，而不是静默写入。

### Tags / Labels

tags/labels 表达多维、可叠加的分类。一个 task 可以有多个 tag。

tags 适合表达：

- area：`desktop`、`cli`、`sqlite`
- domain：`search`、`dispatcher`、`comments`
- semantic group：`llm-facing`、`release-risk`
- 用户临时整理方式

tags 不适合表达：

- workflow state
- hard dependency
- execution ownership
- decision result

未来 semantic tag recommender 可以推荐 tag，但推荐结果必须显式保存后才成为 task 标签。

### Dependency Type

现有 dependency 的核心语义是 hard prerequisite：

```text
parent done => child may become ready
parent not done => child cannot be ready/running
```

引入 `dependency_type` 后，必须保留 hard dependency 的清晰语义。

建议第一批 dependency types：

| Type | 语义 | 是否阻塞 child |
|---|---|---:|
| `blocks` | parent 是 child 的硬前置条件 | 是 |
| `relates_to` | 相关任务，仅用于导航/search/context | 否 |
| `informs` | parent 提供背景、设计输入或决策依据 | 否 |
| `spawned_from` | child 由 parent 执行过程中发现 | 否 |
| `duplicates` | 重复或替代关系 | 否 |

只有 `blocks` 参与：

- dependency blocked 判断
- promote guard
- claim guard
- dispatcher eligibility
- hard DAG blocking

soft dependency 可以进入 DAG 可视化、Desktop 展示和 context，但不能让任务变成 blocked，也不能阻止 claim。

### Comment Author Model

comment 的作者模型只表达“谁写的”。

本项目是本地 dogfood 工具，不建用户系统。作者角色只保留两类：

```text
user | agent
```

规则：

- `user`：本地操作者，也就是“我”。
- `agent`：不是我写的，都算 agent。
- `author`：展示名，例如 `kanban-user`、`codex`。
- `agent_type`：仅当 `author_type=agent` 时可用，例如 `codex`、`executor`、`reviewer`、`dispatcher`。
- 不引入 users table、identity table、RBAC 或权限模型。

这意味着不再使用 comment kind 表示 `system`、`worker` 或 `agent`。这些都属于 author/source 轴。

### Comment Kind Model

`comment.kind` 只表达“这条 comment 的内容语义”。

第一版只保留两类：

```text
note | decision
```

#### `note`

普通协作记录。包括：

- 进展说明
- 交接记录
- 执行总结
- 问题描述
- reviewer 反馈
- 验证记录
- 人或 agent 的普通回复

“遇到的问题”默认也是 `note`。如果问题真的阻塞任务，应该同时通过 transition command 把 task 变成 `blocked`，并写入 `status_reason`。

#### `decision`

结构化选择记录。用于表达：

- 有多个 option。
- 最终选择了其中一个。
- 有选择理由、风险和验证方式。

decision 不是 task status，不是 event，不是 ADR 替代品。

### Comment Metadata

`comment.metadata_json` 是 `comment.kind` 的结构化 payload。

规则：

- `kind=note` 时，metadata 默认 `{}`。
- `kind=decision` 时，metadata 必须符合 decision schema。
- metadata 非法 JSON 或 schema 不匹配时拒绝写入。
- metadata 不参与状态机。
- metadata 不替代 event。
- metadata 不应该变成随意塞字段的长期垃圾桶。

### Decision Comment Schema

建议第一版 shape：

```json
{
  "options": [
    {
      "slug": "comment-metadata",
      "title": "Use comment metadata",
      "detail": "Store structured decision data in task_comments.metadata_json."
    },
    {
      "slug": "decision-table",
      "title": "Create decision table",
      "detail": "Create a separate task_decisions table with option rows."
    }
  ],
  "selected": "comment-metadata",
  "reason": "Keeps decisions close to task discussion and avoids a parallel timeline.",
  "risk": "metadata schema needs validation discipline.",
  "verification": "CLI/API/Desktop tests cover creation, reading, rendering, and invalid metadata rejection."
}
```

Validation rules:

- `options` 必须非空。
- 每个 option 必须有唯一 `slug`。
- `selected` 必须匹配某个 option slug。
- `reason` 必填。
- `risk` 可选但推荐。
- `verification` 可选但推荐。
- `slug` 使用稳定 ASCII slug，便于 CLI、JSON 和前端引用。
- `detail` 可以是 Markdown 文本，但 Desktop 渲染必须走安全 markdown 规则。

### Desktop Rendering Rules

Desktop TaskDetail comment list：

- `note`：按普通 markdown comment 渲染。
- `decision`：
  - 展示 comment body 作为摘要。
  - 展示所有 option slug。
  - selected option 使用明确绿色/selected 状态。
  - 点击 option 展开 `title` 和 `detail`。
  - 展示 reason、risk、verification。
  - 如果 decision metadata 无效，不应该静默当作 selected；应显示错误状态或 degraded note。

### CLI / API Rules

CLI：

```bash
kanban comment add <task-ref> "<body>"
kanban comment add <task-ref> "<body>" --kind note
kanban comment add <task-ref> "<body>" --kind decision --metadata-json '<json>'
```

可后续增加更友好的命令：

```bash
kanban decision add <task-ref> ...
```

但第一版不要求。

API：

- comment create request 显式包含：
  - `body`
  - `author_type`
  - `author`
  - `agent_type`
  - `kind`
  - `metadata`
- comment response 返回同样字段。
- 不再把 `system/worker` 作为 kind 返回。

### Event Kind

`event.kind` 只记录系统事实：

- `comment.added`
- `task.created`
- `task.updated`
- `task.claimed`
- `task.completed`
- `dependency.added`

event 不承载 decision 本体。添加 decision comment 时，event 只记录 `comment.added`，decision 内容在 comment snapshot 中。

### Dispatcher And Frontier Rules

Dispatcher claim eligibility 只能看：

- `status`
- hard dependency (`dependency_type=blocks`)
- `scheduled_at`
- claim token / lease
- board archived state
- assignee / worker profile

Dispatcher 排序可以看：

- `priority`
- `created_at`
- future explicit dispatcher policy

Dispatcher 不看：

- `task_type`
- tags
- `comment.kind`
- `comment.metadata_json`
- decision selected option
- soft dependency

Frontier 可以展示和解释更多字段，但不得把 soft 字段解释成 hard blocker。

### Migration Strategy

本项目是 dogfood 版本，不做沉重兼容层。采用直接 schema train：

1. 本 ADR 固定边界。
2. 修改 `task_comments`：
   - 增加 `author_type`
   - 保留/明确 `author`
   - 增加 `agent_type`
   - 收窄 `kind` 为 `note | decision`
   - 增加 `metadata_json`
3. 更新 Rust domain/API/CLI/Desktop type。
4. 迁移现有 comment：
   - 不是用户本人写的，一律 `author_type=agent`
   - 用户本人写的，`author_type=user`
   - 普通历史 comment 一律 `kind=note`
   - 已有 decision comment 若能识别则 `kind=decision`，否则 `note`
5. 实现 decision metadata validation。
6. 实现 Desktop decision rendering。
7. 后续再做 `task_type`、`dependency_type`、tags/labels 扩展。

### Consequences

优点：

- comment 模型语义清楚：作者归作者，内容类型归内容类型。
- decision comment 可以成为真正结构化对象。
- Desktop 渲染会简单很多。
- LLM/agent 做选择时可以留下可索引、可展开、可复盘的记录。
- 不再让 `system/worker` 这类来源概念污染 content kind。

代价：

- 需要 schema migration。
- 需要一次性更新 CLI/API/Desktop。
- 旧 comment JSON shape 会改变。
- 需要认真做 decision metadata validation，避免 `metadata_json` 变成任意垃圾桶。
- 全局 `kanban-tool` skill 需要同步，因为 CLI/API/comment JSON 行为会变化。

### Non-Goals

- 不引入多用户系统。
- 不引入 RBAC、团队、组织、邀请或云同步。
- 不用 decision comment 替代 ADR。
- 不让 comment metadata 影响 dispatcher claim。
- 不让 tags/type/metadata 变成隐式 status。
- 不把 `task_dependencies` 改成完整知识图谱。
- 不在本 ADR 中实现具体 migration。

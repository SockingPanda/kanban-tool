# 架构决策记录

本文件按时间记录 SPEC 的关键架构决策。每条 ADR 的背景、统计值和迁移状态都是
决策时快照，不会随着实现自动改写；当前行为和实时契约覆盖以对应的
`docs/*_SPEC.md`、`docs/SCHEMA_CONTRACTS.md` 与 `docs/migration/turso-full-feature-parity.md`
为准。ADR-0023 记录当前最终 Turso 架构；历史 ADR 中的旧 SQLite/backend/helper 名称不
表示 active workspace 仍保留这些路径。

ADR-0001 和 ADR-0004 保留为历史记录，但已被当前的 single-host 决策
（ADR-0022）取代。它们中关于 CLI 直开数据库或旧文件名的内容不再描述当前产品。

---

## ADR-0001：仅使用 SQLite

### 状态

已被 ADR-0022 取代（历史决策）

### 后续决策

见 [ADR-0022：Turso single-host canonical application host](#adr-0022turso-single-host-canonical-application-host)。

### 背景

项目明确不考虑多用户、多租户、团队协作和远程 worker。核心运行环境是本地单机，同时需要 CLI 和 Web。

### 决策

只支持 SQLite。

默认数据库：

```text
~/.local/share/kb/kb.db
```

可通过 `--db <path>` 指定项目本地数据库。

### 影响

优点：

- 单一二进制文件易分发。
- CLI 使用成本低。
- 备份简单。
- 本地事务足够强。
- WAL 支持读写并发。

代价：

- 不支持跨机器共享写入。
- 不做 server 集群。
- 同一时刻只有一个写入者。
- 需要控制事务长度。

---

## ADR-0002：Status 枚举是事实，Column 是视图

### 状态

已接受

### 背景

传统的类 Trello 工具常把 list/column 视为状态。但本项目需要 dispatcher、claim、heartbeat、reclaim 和 run 历史。`running` 不是普通视觉列，而是 claim 成功后的执行状态。

### 决策

`tasks.status` 是权威事实。`board_columns` 只是界面展示映射。

### 影响

优点：

- Web、CLI、dispatcher 遵循同一状态机。
- 可保护 `ready -> running`。
- 能支持 review/scheduled/blocked 等非纯视觉状态。

代价：

- 拖拽列不能简单 PATCH status。
- Web 界面需要根据目标列调用状态转换端点。

---

## ADR-0003：快照 + 只追加事件，不做纯事件溯源

### 状态

已接受

### 背景

看板界面会高频查询当前任务列表。纯事件溯源会让当前状态查询复杂化，需要重放事件或额外投影。

### 决策

采用：

```text
tasks snapshot + task_events append-only
```

状态变化时，快照更新与事件插入必须在同一事务内完成。

### 影响

优点：

- 当前 board 查询简单。
- 事件仍可用于审计、SSE、调试。
- 实现复杂度可控。

代价：

- 需要保证快照/事件一致。
- 事件不是唯一事实源。

---

## ADR-0004：CLI 可以直接访问 SQLite，但必须走统一服务路径

### 状态

已被 ADR-0022 取代（历史决策）

### 后续决策

见 [ADR-0022：Turso single-host canonical application host](#adr-0022turso-single-host-canonical-application-host)。

### 背景

如果 CLI 必须依赖常驻 server，会降低本地工具可用性。直接访问 SQLite 更适合脚本和开发流程。

### 决策

CLI 可以直接打开 SQLite 数据库，但只能调用统一 Rust 服务路径；当前实现主要是
`kanban-sqlite::service` 用例函数，并复用 `kanban-core` 的纯状态机辅助函数。
CLI 不允许绕过状态机执行裸 SQL 修改状态。

### 影响

优点：

- 不需要 server 即可使用。
- 脚本友好。
- 和 Web 行为一致。

代价：

- 需要处理 CLI/server/dispatcher 同机并发。
- 所有状态逻辑必须集中在共享的 service/state-machine 路径，避免 CLI、server 或
  dispatcher 各自实现一套状态转换。

---

## ADR-0005：Actor 是审计字符串，不是用户模型

### 状态

已接受

### 背景

项目不做多用户和权限，但仍需要知道某个操作来自谁或哪个 worker。

### 决策

保留 `actor`、`created_by`、`claim_owner` 字段。它们是字符串，不关联用户表。

### 影响

优点：

- 保留审计能力。
- 支持 CLI、Web、dispatcher、worker profile 区分来源。
- 不引入 RBAC 复杂度。

代价：

- 不提供权限隔离。
- actor 可被本地调用者伪造，这是预期边界。

---

## ADR-0006：Worker stdout/stderr 存文件，数据库只存摘要与路径

### 状态

已接受

### 背景

运行日志可能很大。把日志数据放进 SQLite 会影响性能和备份体积。

### 决策

日志写入：

```text
~/.local/state/kb/logs/runs/<run_id>.log
```

数据库只存：

- `log_path`
- `summary`
- `error`
- `exit_code`

### 影响

优点：

- SQLite 保持轻量。
- 日志可直接用 `tail` 查看。
- 备份策略可分开处理数据库和日志。

代价：

- 移动数据库时需要同时移动日志/附件。
- 日志路径需要由 doctor 检查。

---

## ADR-0007：默认只监听 localhost

### 状态

已接受

### 背景

不做远程服务和多用户登录。暴露到局域网会制造安全边界问题。

### 决策

`kanban serve` 默认并且建议只监听：

```text
127.0.0.1:8721
```

MVP 不提供 `0.0.0.0` 远程模式。

### 影响

优点：

- 无需登录系统。
- 降低误暴露风险。

代价：

- 不能多人访问。
- 不能远程手机/浏览器访问。

---

## ADR-0008：状态变化必须使用专用转换命令

### 状态

已接受

### 背景

直接 PATCH `status` 容易绕过 claim/run/event/dependency 保护。

### 决策

禁止普通 update 修改 status。所有状态变化都使用专用命令：

- specify
- promote
- claim
- heartbeat
- done
- submit_review
- block
- unblock
- reclaim
- archive

### 影响

优点：

- 状态机可验证。
- run/claim/event 一致。
- Web/CLI/dispatcher 行为一致。

代价：

- API 数量更多。
- 界面拖拽逻辑更复杂。

---

## ADR-0009：Knowledge Substrate 派生层

### 状态

历史决策（外部 projection lane 已退出 active workspace）

当前 `entities`、`relation_predicates`、`entity_relations` 和检索/向量派生语义由
`kanban-service` 在 Turso host 内提供；旧 Tantivy/Oxigraph/LanceDB/helper 仅保留为
迁移证据。当前 owner 与 rebuild 规则见 ADR-0023、`DATA_MODEL.md`。

### 背景

后续搜索、关系扩展、agent 上下文、artifact 来源和向量召回需要跨 task/run/comment/artifact/skill 的统一身份与派生索引，但不能削弱 SQLite 状态机、claim 和依赖保护。

### 决策

SQLite 继续作为运行事实源。新增：

- `entities`：跨库统一的 `kb://...` 身份注册表。
- `relation_predicates` / `entity_relations`：受控 predicate 与可重建关系镜像。
- `index_outbox`：派生存储的至少一次任务入口。
- `derived_store_state`：Tantivy/Oxigraph/LanceDB 等派生层健康和水位。

Tantivy、Oxigraph、LanceDB 都是可重建的派生存储，不参与状态机事务。

`derived_store_state` 的语义是存储全局状态，不是 board 局部状态：

- `last_event_id` 表示该存储已成功处理并提交的全局 task event 高水位。成功同步/重建只能把它单调推进，不能倒退。
- `dirty=true` 表示该存储仍有未完成 outbox、失败 outbox 或最近一次派生更新失败；即使某个 board 已完成同步/重建，其他 board 仍有待处理/失败任务时也必须保持 dirty。
- board 范围的同步/重建只清理当前 board 的 outbox 任务；是否把 `dirty` 置回 false，取决于同一存储目标是否还存在任何 board 的未完成 outbox。
- `last_error` 记录最近一次存储级失败证据。成功处理会清除 `last_error`，失败会保持 `dirty=true` 并保留/标记相关 outbox 失败状态。
- `index_outbox` 是恢复和重放入口；`derived_store_state` 是操作者使用的健康/水位摘要。两者都不能使派生层成为事实源。

### 影响

优点：

- 后续图/向量/context broker 可以接同一实体/关系契约。
- SQLite 状态机边界保持清楚。
- 派生存储损坏时可回退/重建。
- `kanban doctor` / maintenance API 汇总 outbox 积压、脏存储、last_error 和失败 outbox，供本地操作者判断是否同步/重建，而不是让派生层参与 SQLite 事务。

代价：

- 需要维护实体回填/outbox/派生状态。
- `derived_store_state` 是派生存储的主健康/水位记录；Tantivy 的旧 `app_settings` 搜索状态仅保留为兼容元数据。

---

## ADR-0010：单数据库多 board 与 CLI task 引用

### 状态

已接受

### 背景

本地项目需要不同 board/project，但未来也需要聚合视图和跨 board 审计。如果每个项目拆一个 SQLite 数据库，聚合、搜索、事件和 dispatcher 恢复都会变复杂。另一方面，裸 `#12` 在 shell 中容易被当作注释，且 board 内的 seq 不能跨 board 唯一。

### 决策

继续使用单个 SQLite 数据库内的多个 board：

- `tasks.id` 是全局唯一 `t_...`。
- `tasks.seq` 只在 `board_id` 内唯一。
- CLI/API 展示可复制的 task 引用：`board_slug#seq`。
- CLI task 引用支持全局 `t_...`、当前 board 的 `12` / `#12`、显式 `board#12` / `board/#12` / `b_...#12`。
- 当前 board 的解析顺序是 `--board`、`KB_BOARD`、最近的 `.kb/config.toml`、`default`。
- `.kb/config.toml` 只记录当前项目选择的 board，不表示项目拥有独立数据库。
- Board slug 禁用保留 ID 前缀和会破坏引用语法的字符。

已归档 board 默认不可写；归档只标记 board，不改 task 状态，并拒绝仍有 `running` task/run 的 board。只读 events/runs/comments 历史保留可查，作为审计入口。

### 影响

优点：

- 保留未来聚合 board / 仪表盘的数据基础。
- `t_...` 可作为脚本稳定全局引用。
- `board#seq` 对人和 shell 都更可复制。
- 项目级当前 board 不破坏单数据库备份、搜索和 dispatcher 语义。

代价：

- CLI 必须维护 task 引用的解析/解析目标逻辑。
- 已归档 board 需要区分只读历史与变更保护。
- 裸 `#12` 只能作为兼容输入，文档和输出不能依赖它。

---

## ADR-0011：Schema 批次边界：status、type、labels、dependency type 与 decision comments

### 状态

提议中

### 背景

`kanban-tool` 接下来会进入一组 schema/model 扩展：

- `task_type`：表达任务是什么类型。
- `dependency_type`：表达任务之间是什么关系。
- labels：表达可搜索、可筛选、可推荐的多维标签。
- comments：承载人和 agent 的协作记录。
- decision comment：记录人或 LLM/agent 在多个方案之间做出的选择。

当前 comment 模型里的 `kind` 混用了两类概念：

- 谁写的：system / worker / agent / user。
- 写的是什么：普通记录 / 决策记录。

这会让后续结构化 decision comment 变得混乱。需要先把模型边界切开：

- 作者/来源轴：谁留下了这条 comment。
- 内容类型轴：这条 comment 表达什么语义。

项目早期只面向本地单用户场景，不需要为早期评论结构保留沉重兼容层。可以直接修改模型，
只要迁移清晰，并让 CLI、API 与 Desktop 同步跟上。

### 决策

保留现有核心原则：

- `tasks.status` 继续是唯一的权威工作流状态。
- hard dependency 继续是状态机和 dispatcher 保护的事实来源。
- `task_events` 继续是只追加审计轨迹。
- comments 继续承载协作记录，但 comment schema 要拆清楚作者和内容语义。
- 新字段默认不改变状态机、dispatcher claim 或 ready 资格，除非本 ADR 明确允许。

### 字段职责

| 字段 / 模型 | 责任 | 是否影响状态机 | 是否影响 dispatcher | 是否影响依赖/搜索/上下文展示 | 是否用于搜索/上下文/界面 |
|---|---|---:|---:|---:|---:|
| `status` | 权威工作流状态 | 是 | 是 | 是 | 是 |
| `priority` | ready/dispatcher 的排序权重 | 否 | 是，排序 | 是，列表和推荐排序 | 是 |
| `scheduled_at` | 计划时间，参与 scheduled/ready guard | 是 | 是 | 是，列表和上下文排序 | 是 |
| `due_at` | 截止时间，只展示、筛选、排序 | 否 | 可排序 | 可排序 | 是 |
| `task_type` | 任务类别，例如 bug/feature/research/ops/follow_up | 否 | 否 | 可用于展示/排序，不改变执行资格 | 是 |
| labels | 多标签分类、搜索、推荐和界面分组 | 否 | 否 | 否，除非未来显式配置排序策略 | 是 |
| `dependency_type` | 依赖边语义，区分硬阻塞和软关系 | 仅硬阻塞 | 仅硬阻塞 | 是，但必须区分硬/软关系 | 是 |
| `comment.author_type` | 评论作者角色：`user` 或 `agent` | 否 | 否 | 否 | 是 |
| `comment.author` | 展示名，例如 `alice`、`codex` | 否 | 否 | 否 | 是 |
| `comment.agent_type` | 可选 agent 细分，例如 `codex`、`executor`、`dispatcher` | 否 | 否 | 否 | 是 |
| `comment.kind` | 内容语义：`note` 或 `decision` | 否 | 否 | 否 | 是 |
| `comment.metadata_json` | `comment.kind` 对应的结构化 payload | 否 | 否 | 否 | 是 |
| `event.kind` | 只追加审计事件类型 | 否，event 是结果不是输入 | 否 | 否 | 是 |

### 工作流状态

`status` 仍然是任务是否可执行、是否被 claim、是否 blocked/review/done 的唯一事实来源。

任何新字段都不能隐式表达状态：

- `task_type=bug` 不表示高优先级。
- label `blocked` 不表示 task 处于 blocked。
- decision 的选中项不表示 task 处于 done。
- comment 中写 “blocked” 不改变 task status。

状态变化只能通过状态转换命令完成。

### 任务类型

`task_type` 表达“这个 task 是什么工作类别”，不表达“它现在处于什么执行状态”。

建议第一批 task 类型：

```text
bug | feature | research | ops | docs | refactor | test | follow_up
```

`task_type` 可以用于：

- Desktop/List/Board 筛选。
- 搜索/上下文过滤。
- 依赖、搜索和上下文解释。
- 未来排序加权。

`task_type` 不用于：

- dispatcher 领取资格。
- 状态机转换保护。
- 硬依赖判断。
- 替代 labels。

枚举策略：

- 第一版使用受控枚举。
- 后续如需要开放扩展，再单独做 ADR。
- 未知类型应被拒绝，而不是静默写入。

### 标签

labels 表达多维、可叠加的分类。一个 task 可以有多个 label。

labels 适合表达：

- 区域：`desktop`、`cli`、`sqlite`
- 领域：`search`、`dispatcher`、`comments`
- 语义组：`llm-facing`、`release-risk`
- 用户临时整理方式

labels 不适合表达：

- 工作流状态
- 硬依赖
- 执行所有权
- 决策结果

未来的语义 label 推荐器可以推荐 label，但推荐结果必须显式保存后才成为 task label。

### 依赖类型

现有 dependency 的核心语义是硬前置条件：

```text
parent done or archived => child may become ready
parent neither done nor archived => child cannot be ready/running
```

引入 `dependency_type` 后，必须保留 hard dependency 的清晰语义。

建议第一批 dependency 类型：

| 类型 | 语义 | 是否阻塞子任务 |
|---|---|---:|
| `blocks` | 父任务是子任务的硬前置条件 | 是 |
| `relates_to` | 相关任务，仅用于导航/search/context | 否 |
| `informs` | 父任务提供背景、设计输入或决策依据 | 否 |
| `spawned_from` | 子任务在父任务执行过程中被发现 | 否 |
| `duplicates` | 重复或替代关系 | 否 |

只有 `blocks` 参与：

- 依赖阻塞判断
- promote 保护
- claim 保护
- dispatcher 执行资格
- 硬依赖阻塞

软依赖可以进入 Desktop 展示、搜索和上下文，但不能让任务变成 blocked，也不能阻止 claim。

### 评论作者模型

comment 的作者模型只表达“谁写的”。

本项目面向本地单用户场景，不建立用户系统。作者角色只保留两类：

```text
user | agent
```

规则：

- `user`：本地操作者写入的内容。
- `agent`：自动化主体写入的内容。
- `author`：展示名，例如 `alice`、`codex`。
- `agent_type`：仅当 `author_type=agent` 时可用，例如 `codex`、`executor`、`reviewer`、`dispatcher`。
- 不引入 users table、identity table、RBAC 或权限模型。

这意味着不再使用 comment kind 表示 `system`、`worker` 或 `agent`。这些都属于作者/来源轴。

### 评论类型模型

`comment.kind` 只表达“这条 comment 的内容语义”。

第一版只保留两类：

```text
note | decision
```

后续 Generic Signal Ledger 决策把 `signal` 加入该集合，用于指向通用 signal ledger；
当前完整约束以 `docs/DATA_MODEL.md` 和 `docs/API_SPEC.md` 为准。

#### `note`

普通协作记录。包括：

- 进展说明
- 交接记录
- 执行总结
- 问题描述
- 审查者反馈
- 验证记录
- 人或 agent 的普通回复

“遇到的问题”默认也是 `note`。如果问题真的阻塞任务，应该同时通过状态转换命令把 task 变成 `blocked`，并写入 `status_reason`。

#### `decision`

结构化选择记录。用于表达：

- 有多个选项。
- 最终选择了其中一个。
- 有选择理由、风险和验证方式。

decision 不是 task status，不是 event，也不是 ADR 的替代品。

### 评论元数据

`comment.metadata_json` 是 `comment.kind` 的结构化 payload。

规则：

- `kind=note` 时，metadata 默认 `{}`。
- `kind=decision` 时，metadata 必须符合 decision schema。
- metadata 非法 JSON 或 schema 不匹配时拒绝写入。
- metadata 不参与状态机。
- metadata 不替代 event。
- metadata 不应该变成随意塞字段的长期垃圾桶。

### 决策评论 Schema

建议第一版结构：

<!-- schema-doc-ignore: 说明性或不完整 payload；已提交的 schema fixture 仍是可执行权威 -->
```json
{
  "options": [
    {
      "slug": "comment-metadata",
      "title": "使用评论元数据",
      "detail": "把结构化决策数据保存在 task_comments.metadata_json 中。"
    },
    {
      "slug": "decision-table",
      "title": "创建决策表",
      "detail": "创建独立的 task_decisions 表并保存各个选项。"
    }
  ],
  "selected": "comment-metadata",
  "reason": "让决策紧邻任务讨论，避免产生平行时间线。",
  "risk": "metadata schema 需要严格验证。",
  "verification": "CLI/API/Desktop 测试覆盖创建、读取、渲染和非法元数据拒绝。"
}
```

验证规则：

- `options` 必须非空。
- 每个 option 必须是 object，且有非空字符串 `slug`、`title`、`detail`。
- 每个 option 必须有唯一 `slug`。
- `selected` 必须匹配某个 option slug。
- `reason` 必填且非空。
- `risk` 可选但推荐；如果出现，必须是非空字符串。
- `verification` 可选但推荐；如果出现，必须是非空字符串。
- `slug` 使用稳定小写 ASCII slug，必须以小写字母或数字开头，只包含小写字母、数字和 `-`，便于 CLI、JSON 和前端引用。
- `detail` 可以是 Markdown 文本，但 Desktop 渲染必须遵守安全 Markdown 规则。

### Desktop 渲染规则

Desktop TaskDetail 评论列表：

- `note`：按普通 Markdown 评论渲染。
- `decision`：
  - 展示 comment body 作为自然语言摘要，例如“已决定继续使用 comment metadata 承载结构化决策信息，正文保留为简短结论，方便没有结构化渲染的环境阅读。”
  - 展示所有选项 slug。
  - 选中项使用明确的绿色/selected 状态。
  - 点击选项展开 `title` 和 `detail`。
  - 展示 reason、risk、verification。
  - 如果 decision metadata 无效，不应该静默当作 selected；应显示错误状态或降级 note。

### CLI / API 规则

CLI：

```bash
kanban comment add <task-ref> "<body>"
kanban comment add <task-ref> "<body>" --kind note
kanban comment add <task-ref> "<body>" --kind decision --metadata-json '<json>'
```

`kind=decision` 的 body 是自然语言回退摘要，不重复完整选项表；`options`、`selected`、`reason`、`risk` 和 `verification` 只放在 `metadata_json` 中，由 Desktop 在正文下方结构化渲染。

可后续增加更友好的命令：

```bash
kanban decision add <task-ref> ...
```

但第一版不要求。

API：

- comment 创建请求显式包含：
  - `body`
  - `author_type`
  - `author`
  - `agent_type`
  - `kind`
  - `metadata`
- comment 响应返回同样字段。
- 不再把 `system/worker` 作为 kind 返回。

### 事件类型

`event.kind` 只记录系统事实：

- `comment.added`
- `task.created`
- `task.updated`
- `task.claimed`
- `task.completed`
- `dependency.added`

event 不承载 decision 本体。添加 decision comment 时，event 只记录 `comment.added`，decision 内容在 comment 快照中。

### Dispatcher 与候选集规则

Dispatcher 领取资格只能看：

- `status`
- 硬依赖（`dependency_type=blocks`）
- `scheduled_at`
- claim token / 租约
- board 归档状态
- 负责人 / worker profile

Dispatcher 排序可以看：

- `priority`
- `created_at`
- 未来显式的 dispatcher 策略

Dispatcher 不看：

- `task_type`
- labels
- `comment.kind`
- `comment.metadata_json`
- decision 选中项
- 软依赖

候选集可以展示和解释更多字段，但不得把软字段解释成硬阻塞条件。

### 迁移策略

项目当时仍处于本地单用户的早期版本，不做沉重兼容层。采用直接结构迁移批次：

1. 本 ADR 固定边界。
2. 修改 `task_comments`：
   - 增加 `author_type`
   - 保留/明确 `author`
   - 增加 `agent_type`
   - 收窄 `kind` 为 `note | decision`
   - 增加 `metadata_json`
3. 更新 Rust domain/API/CLI/Desktop 类型。
4. 迁移现有 comment：
   - 不是用户本人写的，一律 `author_type=agent`
   - 用户本人写的，`author_type=user`
   - 普通历史 comment 一律 `kind=note`
   - 已有 decision comment 若能识别则 `kind=decision`，否则 `note`
5. 实现 decision metadata 验证。
6. 实现 Desktop decision 渲染。
7. 后续再做 `task_type`、`dependency_type`、labels 扩展。

### 影响

优点：

- comment 模型语义清楚：作者归作者，内容类型归内容类型。
- decision comment 可以成为真正结构化对象。
- Desktop 渲染会简单很多。
- LLM/agent 做选择时可以留下可索引、可展开、可复盘的记录。
- 不再让 `system/worker` 这类来源概念污染内容类型。

代价：

- 需要 schema migration。
- 需要一次性更新 CLI/API/Desktop。
- 旧 comment JSON shape 会改变。
- 需要认真做 decision metadata 验证，避免 `metadata_json` 变成任意垃圾桶。
- 全局 `kanban-tool` skill 需要同步，因为 CLI/API/comment JSON 行为会变化。

### 非目标

- 不引入多用户系统。
- 不引入 RBAC、团队、组织、邀请或云同步。
- 不用 decision comment 替代 ADR。
- 不让 comment metadata 影响 dispatcher claim。
- 不让 labels/type/metadata 变成隐式 status。
- 不把 `task_dependencies` 改成完整知识图谱。
- 不在本 ADR 中实现具体 migration。

---

## ADR-0012：Label Proposal Provider 边界

### 状态

历史 provider 边界；当前 ontology/label proposal owner 已收敛到 `kanban-service`，
Turso `vector32`/host Ollama 只提供可降级、可重建的派生能力。文中
`kanban-sqlite`、LanceDB 和旧 runtime 名称只描述决策快照。

### 背景

语义 label 建议的日常路径应保持确定性：SQLite 保存权威的
`labels` / `task_labels` / `label_semantics` / `label_atoms`，LanceDB 只是
`kb_label_atoms` 派生索引，求解器只做本地向量计算。Label proposal 是“覆盖不足时建议新 label
semantics”的可选流程，它可以由人工、离线工具或未来本地 LLM provider 产生候选项。

真实 LLM provider 如果直接放进 `kanban-sqlite`，会把外部 SDK、HTTP client、prompt、
credential 和 runtime 配置拖入 SQLite service。这样会破坏本项目的本地优先 / 仅 SQLite
边界，也会让 proposal 验证与模型调用耦合过深。

### 决策

`kanban-sqlite` 只定义并消费 `LabelProposalProvider` trait：

- `DisabledLabelProposalProvider`：默认 provider 不可用，返回降级尝试，不写入权威 label。
- `ManualLabelProposalProvider`：接收 CLI/API 显式传入的本地/离线候选项。
- `propose_task_label_with_store`：从 SQLite 读取 task 和建议上下文，调用 provider，
  然后执行确定性验证、残差 top1+margin 门禁、proposal 持久化和
  accept/reject 生命周期。

真实 LLM provider 不属于 `kanban-sqlite`。可选实现位置是：

- `kanban-server`：当 localhost server 显式配置本地 provider/runtime 时注入 trait object。
- `kanban-cli` 或本地 runtime：当命令显式读取本地/离线候选项或未来本机模型输出时注入。
- 独立 `kanban-ai` / `kanban-llm` crate：承载 SDK、HTTP client、prompt 和 credential 读取，
  再向上层暴露实现 `LabelProposalProvider` 的适配器。

### 影响

优点：

- SQLite service 不依赖 LLM SDK、HTTP AI client、runtime credential 或外部模型配置。
- proposal 生命周期仍由确定性的 SQLite service 守住，不会因为 provider 类型不同而绕过
  残差验证或 accept/reject 门禁。
- 日常 `label suggest` 不依赖 proposal provider；provider 不可用只会产生降级的 proposal 尝试。
- 未来 provider 可以替换或禁用，不需要修改权威 label 事实或 task label 绑定语义。

代价：

- 真实 provider 需要在上层做适配器和配置装配。
- server/CLI 需要明确区分“候选项生成失败”和“SQLite 验证拒绝候选项”。
- 需要持续避免把 prompt、credential、HTTP 重试等关注点下沉进 `kanban-sqlite`。

### 非目标

- 本 ADR 不实现真实 LLM provider。
- 不上传本地 task 数据到远程服务。
- 不让 provider 自动绑定 task label。
- 不改变 proposal accept 后才创建 label semantics / atoms 的生命周期。

---

## ADR-0013：暂不引入 label ontology 图投影

### 状态

历史决策（不建立独立 ontology graph）；当前仍不建立 ontology 专属 graph mutation path，
但通用 entities/relations 的 canonical BFS 已由 `kanban-service` 提供。Ontology ledger
继续由 Turso facts/service actions 负责，graph 故障不会改变 canonical ontology。

### 背景

当前 label ontology 已有 SQLite 权威事实与查询面：

- `labels` / `task_labels` 表达当前 task label 绑定事实。
- `label_semantics` 表达权威 ontology semantics；`label_atoms` 是从 semantics 与
  label name 展开的 SQLite 物化投影。
- `label_semantic_proposals` 表达新 label proposal lifecycle。
- `label_ontology_observations` / `signals` / `actions` / `action_signals` 表达
  来源、审查、变更和验证历史。
- `label ontology review`、`label atom explain`、JSONL export/import 和 doctor 已经从
  SQLite records 直接回答第一批 review/provenance 问题。

项目也已有通用 Knowledge Substrate 图：`entity_relations` 作为 SQLite 镜像，
Oxigraph 作为可重建派生存储，`index_outbox` / `derived_store_state` 管理 dirty、
同步和重建。这个图当前覆盖 task-board、task dependency 等通用实体
关系，不覆盖 label ontology 账本。

第一版账本还没有明确的关系查询需求需要 ontology 专属图。过早投影 signal、
action、atom 和 proposal 会增加 schema、outbox、query API 和重建复杂度，并提高把
图误当第二事实源的风险。

### 决策

暂不实现 label ontology 图投影。

在 rename/split/merge、跨 action 来源、atom 谱系或审查工作台出现明确
关系查询需求前，ontology 查询继续走 SQLite service/API：

- `label ontology review`
- `label ontology show`
- `label atom explain`
- `label proposal list/show`
- JSONL export/import 与 doctor

未来若新增 ontology graph projection，它必须满足：

- SQLite `labels`、`task_labels`、`label_semantics`、`label_atoms`、proposal 和
  `label_ontology_*` 仍是事实来源；`label_atoms` 是 projection，不是独立 semantic truth。
- 投影只能从 SQLite 快照/outbox 派生，可删除重建。
- 投影状态通过 `index_outbox` 和 `derived_store_state` 或等价派生层控制面表达。
- graph API 只能查询关系/来源，不提供 confirm/apply/validate/revert/bootstrap
  或其它权威变更写入口。
- 图 dirty、error、删除或重建失败不改变 task status、task labels、semantics、atoms、
  proposal 或账本记录。

### 影响

优点：

- 第一版 ontology workflow 保持简单，避免过早增加第二个 provenance 表达。
- SQLite ledger/review/explain 继续作为可审计事实来源。
- 未来如果确有查询需求，可以复用已存在的 Knowledge Substrate derived-store contract。
- graph 故障不会影响 ontology mutation、validation 或 review 的 canonical state。

代价：

- 复杂 lineage / relationship traversal 暂时需要通过 SQLite query、review grouping、
  `atom explain` 或导出后离线分析完成。
- 未来若要支持 ontology graph，需要单独设计 projection schema、outbox fanout 和 rebuild
  测试。

### 非目标

- 本 ADR 不新增 ontology RDF schema。
- 不把 `label_ontology_*` rows 写入 `entity_relations`。
- 不扩展 `kanban graph` 为 ontology mutation API。
- 不用 graph 替代 label ontology review、show、atom explain 或 validation history。

---

## ADR-0014：标签本体收口契约

### 状态

已接受

### 背景

标签身份增删改查、任务标签绑定、语义变更、提案接受、引导初始化、验证与审查生命周期
曾经混用来源语义。最危险的问题是普通任务采集可以隐式创建词汇，删除标签身份可以隐式
删除语义与 atom，语义变更会拆成多条逐 atom 操作，受信验证的原始 JSON 可能绕过采集器，
引导验证也曾依赖提交后的补偿操作。

### 决策

采用收窄后的 closure contract：

- `labels` identity CRUD 是基础 vocabulary registry，不写 ontology mutation action；task
  label binding 只绑定已存在 label，写普通 task event。
- 删除 label identity 时永不隐式删除 `label_semantics` / `label_atoms`；force 只允许移除 task
  bindings 后删除空 identity。
- `label_semantics` / `label_atoms` canonical mutation 一次 transaction 只写一条 root
  mutation action；实际 atom delta 写入 `label_ontology_action_atom_effects` 的
  `added` / `removed` rows。No-op 不写 action/effects，也不标脏 index。
- Semantics clear 继续使用 `update_semantics` action type，必须有 actor、非空 reason 和
  `expected_semantics_hash`。
- Atom explain 优先读取 effect rows；legacy per-atom actions 只做兼容读取，不回写压缩历史。
- Trusted automated validation 只能由 CLI collector 生成，表示 current hash/index
  generation 和指定 cases/controls 机械通过，不表示全局语义正确。
- CLI bootstrap verify 是 pre-commit staged verification；失败、provider unavailable 或
  verify/commit 间 state 变化时零 canonical 写入。
- `validation_requirement` 与 validation attempt outcome 分离；effective outcome 是查询
  reducer 结果。Unsupported parent 可记录 external failed/partial 诊断，但不能 passed。
- Public structure plan write 入口关闭；rename/split/merge 暂仅可作为 review signal 或
  legacy action 读取。

### 影响

优点：

- Routine task capture 不能再绕过 vocabulary adoption。
- Ledger 行数随真实 mutation 数线性增长，atom explain 粒度来自 effect rows。
- Destructive semantics clear 有 CAS、reason 和 revertable root action。
- Trusted/external validation 边界由 Rust visibility、collector entry 和 tests 锁住。

代价：

- 旧 per-atom action 保留历史噪声，需要 explain/revert 的 legacy compatibility。
- Base label identity delete 需要用户先显式 clear semantics。
- Structure mutation 需要未来单独 typed apply、binding migration 和 validation policy。

### 非目标

- 不新增 action type、signal type、validation status 或 graph/dashboard projection。
- 不回写或压缩历史 per-atom actions。
- 不实现 rename/split/merge canonical mutation。

## ADR-0015：通用信号账本

### 状态

已被 ADR-0022 取代（signal surface 非 active）

### 背景

Agent/Product 的故障和观察需要一个持久的审查生命周期；它不能局限于 label，也不能只是自由格式的评论元数据。

### 决策

新增 board 范围的 `signal_observations` 和 `signals` 表。`kanban signal record` 写入 observation 与 signal 记录；存在 task 上下文时，还会在同一 SQLite 事务中写入简短的 `task_comments.kind = signal` 回链。生命周期审查支持 `open -> confirmed|rejected|superseded|resolved` 和 `confirmed -> resolved`；supersede 要求替代 signal 与原 signal 属于同一 board，并防止成环。V1 不会自动创建后续任务。

### 影响

Signal 账本成为通用 agent/product 信号的权威存储。Label ontology 账本仍然只服务于 label，不复用于通用产品信号。


## ADR-0016：API/SSE 传输描述符作为唯一 method/path 权威

### 状态

已被 ADR-0022 取代

当前 Axum router 直接注册 active single-host paths；descriptor 是待清理的机器契约来源，
不是运行时 route factory。

### 背景

此前 `SurfaceOperation` 与 router 各自手写 method/path；即使已有一致性测试，仍存在双写漂移面。

### 决策

在 `kanban-protocol` 默认 feature 中保存 API/SSE 描述符；server router 以
`operation_id` + 显式 `adapter_id` 绑定真实 handler，并读取描述符的 method/path。

### 影响

`SurfaceOperation` 的 API/SSE 记录改为投影；CLI/JSONL 保持独立清单。schema root 使用
`contract_id`，不与端点 `operation_id` 混淆。DTO/schema 采用不在本决策中提前完成。

## ADR-0017：B1-A 错误与删除响应的 wire 收口边界

### 状态

已接受

### 背景

稳定错误码与固定删除确认响应已具备可验证的 wire 形状；把任意
`String`/`Value` 留在公开边界会削弱 schema、类型化 consumer 与漂移门禁。

### 决策

`ErrorBody.code` 使用闭合的 `ApiErrorCode`，server 适配器显式将 `KanbanError` 映射为
枚举；label semantics 删除 handler 使用 `DeleteResponse`/`DeleteResult`，不再公开
`DataEnvelope<serde_json::Value>`。

### 影响

该决策只拥有 wire/schema 证据。HTTP status、locale 消息、service 保护、状态机、
CAS、事务与 SQLite 继续由 adapter/service/core 负责。决策时删除端点的
其它义务尚未建模；其后续当前状态以 `docs/SCHEMA_CONTRACTS.md` 为准。

## ADR-0018：B1-C0 传输位置、基数与精确/共享绑定

### 状态

已接受

### 背景

仅有 contract ID 与 input/output 方向无法区分 path/query/header/body/2xx success/shared
error/SSE，也无法证明 query 重复值顺序、path placeholder 映射或共享错误 envelope 的
真实复用关系。

### 决策

API/SSE 语义 contract 显式声明 `Http { operation_key, location, parameters }`，非 HTTP
contract 显式声明 `NoTransport`；参数基数只允许
`RequiredOne|OptionalOne|RepeatedOrdered`。`Success` 只表示 2xx success；非 2xx `Error`
只允许用于 `SharedComponent`。任意 `Adopted` contract 和端点精确引用都必须是
`granularity=Exact`。

端点精确绑定不维护全局第二绑定映射。method/path 唯一、精确
`operation_key` 和单一 location 共同推出合法绑定唯一；公开面目录中的重复精确
引用仍单独失败关闭。

`SharedComponent` 可以跨多个端点复用且不计入精确/采用覆盖。
generated/adopted shared 必须至少有显式链接，或同一公开面的真实采用 witness。

### 影响

验证器对未知/`Planned`/`Excluded` 引用，以及错误的
binding/granularity/location/direction/operation/surface 失败关闭。本 ADR 保存的是
迁移当时的边界和冻结值，不代表当前覆盖；实时状态见 `docs/SCHEMA_CONTRACTS.md`。

## ADR-0019：B1-C1 Task-read 精确 path/query 契约与单一有序解析器

### 状态

历史快照；当前 task-read surface 以 ADR-0023、`docs/API_SPEC.md` 和
`kanban-protocol::endpoint_catalog()` 为准

### 背景

两个 task-read 端点需要证明各自精确消费 path/query，同时避免 handler 或多个 parser
重复拥有 raw query。

### 决策

`GET /api/v1/boards/:board/tasks` 使用唯一的 task-list path/query DTO；状态筛选通过同一
query contract 表达，搜索按状态的只读结果由 `/api/v1/search/tasks/by-status` 负责。server
使用类型化 Axum extractor 从一次 raw URI 进入共享有序解析器；handler 只接收已绑定的
request，不持有第二个 raw source。
- Query 语法：只有 `status`、`priority`、`label`、`plan_filter` 是
  `RepeatedOrdered`；其余标量重复、未知 key 与旧 `search` 别名均返回
  `400 invalid_input`。54 对上限由 9/4/3/32 个重复参数预算加 6 个标量参数推导；
  raw query 上限为 8192 字节。`q` 是唯一文本搜索 key。label 会 trim Unicode 边缘空白，
  但纯 Unicode 空白失败关闭；percent/UTF-8、枚举、priority、limit、offset 和 sort 边界由
  真实 router URI 矩阵固定。
contract 仍有 DTO-to-fixture producer 和 fixture-to-real-router consumer；非默认 board 哨兵
证明真实 path 消费。当前 route/catalog 对齐测试覆盖类型化 extractor、raw URI 单一来源、
`&path.board` 到 `list_tasks_page` 的实参和 query 边界。producer/consumer 区域保护只证明
当前源码区域直接分离，不把任意未来共同 helper 间接层夸大为变异完备证明。

### 影响

Desktop/Web/CLI 的 HTTP 调用方必须使用上述语法；现有 Desktop 调用方已使用 `q`
  并保留重复参数顺序。Turso service 的防御性上限直接引用唯一 application 权威，
  server 相等性门禁覆盖该实际 service 路径；service 查询行为与 core 状态机不变。本文保留
  决策时的迁移边界；当前采用状态以 `docs/SCHEMA_CONTRACTS.md` 为准。

## ADR-0020：B1-C2b task-read 成功响应决策

### 状态

已接受

### 背景

共享响应 envelope 会掩盖两个 task-read 端点的精确响应差异。

### 决策

让两个 task-read 端点分别拥有闭合响应 contract，只复用 `ApiTask`、`ApiLabel` 与既有
pagination primitives。

### 影响

行为细节以 [API_SPEC](API_SPEC.md#4-任务) 和
[SCHEMA_CONTRACTS](SCHEMA_CONTRACTS.md#2-契约状态) 为准。

## ADR-0021：Oxigraph quick-xml 安全临时 vendor patch

### 状态

已被 ADR-0022 取代

该例外只适用于已经退出 active workspace 的 Oxigraph projection lane。当前根
`Cargo.toml` 不再包含该 `[patch.crates-io]`；以下内容保留为历史决策记录，不描述当前
产品依赖图。

### 背景

`oxrdfxml 0.2.3` 与 `sparesults 0.3.3` 的 crates.io 版本仍解析到受
RUSTSEC-2026-0194/RUSTSEC-2026-0195 影响的 `quick-xml < 0.41`；仓库当前通过 root
`Cargo.toml` 与对应的 `vendor/` 目录使用上游修复源码，并统一到 `quick-xml 0.41.0`。

### 决策

允许根目录 `Cargo.toml` 中唯一的 `[patch.crates-io]` 例外，且仅接受 `oxrdfxml`/`sparesults` 两个精确仓内 vendor 路径、package name/version 与普通文件目标。`schema_dependency_policy` 对额外 key、非精确 source/path、path traversal、symlink、全部 `[replace]` 保持失败关闭；schema-tool 注册表闭包不变，产品依赖图继续禁止 schema tooling 泄漏。

由安全负责人维护，待 crates.io 上游版本发布并确认 `quick-xml >= 0.41` 后移除 vendor、`[patch]`、lockfile 变更及本 ADR；advisory、provenance 或 vendor digest 变化必须重新审查。复核期限：2026-10-12。

---

## ADR-0022：Turso single-host canonical application host

### 状态

已接受（当前唯一 host/owner 决策）

### 后续决策

本 ADR 继续约束唯一 host、typed localhost client、共享 mutation path、Turso ownership 和
dispatcher 边界。它作出时的“阶段性后置项”已经由完整功能重构接入当前 service path；具体
owner、入口、迁移规则、已有测试和未运行 gates 以
[`docs/migration/turso-full-feature-parity.md`](migration/turso-full-feature-parity.md)、
[`ARCHITECTURE.md`](ARCHITECTURE.md) 和 [`DATA_MODEL.md`](DATA_MODEL.md) 为准。

当前实现已将 application service 与 Turso persistence 合并为 `kanban-service`，并保持
`kanban-protocol` 作为独立 wire/schema crate。该收敛不改变 single-host ownership，也不能用
“暂不支持”替代 parity ledger 的闭合。

### 背景

CLI、MCP 和 Desktop 曾分别持有 storage/runtime 入口。即使它们读写同一份数据库文件，
也可能各自解释 task transition、claim、comment 和 event，造成语义漂移。Turso 默认还要求
同一本地数据库文件由一个 OS 进程 owner 打开；继续维护多进程直连、runtime framing 或兼容
fallback 会把数据库 ownership 问题扩展成另一套产品协议。

### 决策

1. `kanban serve` 是唯一 application host，也是唯一可以打开、初始化和关闭 Turso 数据库的
   进程。默认路径是 `~/.local/share/kb/kanban.db`，默认监听 `127.0.0.1:8721`。
2. CLI、MCP、Desktop 统一通过 typed localhost HTTP client 调用 host。server 不可用时返回
   `server_unavailable`；不允许“有 server 走 HTTP、没 server 直开数据库”的双路径。
3. 所有 mutation 进入同一个 `ApplicationService`，由同一状态机、事务、board isolation、
   CAS claim、owner/token 校验和 error contract 保护。adapter 只负责解析、调用和展示。
4. 使用 `turso = 0.7.2`、`default-features = false`；不启用 `multiprocess_wal`。同一 host 进程
   内按 operation 获取 connection，数据库文件不由其他入口或其他进程直接打开。
5. dispatcher 只作为 `kanban serve --dispatcher-profile <path>` 的同进程 opt-in 单 worker
   loop，复用 application commands；默认不自动消费队列。
6. 不建设或保留自定义 framed IPC、named pipe、runtime protocol、capability negotiation、
   generalized mutation receipt、第二 projection control plane 或旧 API 兼容层。labels、
   signals、search、graph、vector、context、projection 和 importer 若提供能力，必须复用
   当前 `kanban-service`/Turso host path。

> 历史范围说明：这里的“未迁移”描述 ADR-0022 作出时的阶段，不代表完整功能重构可以删去
> 这些能力；后续迁移仍必须复用本 ADR 定义的 single-host application path。

### 影响

优点：

- 三个入口共享同一条可验证的 command/query path，业务错误不会在 adapter 间分叉。
- 单一 DB owner 简化 Turso 生命周期、重启恢复和并发边界；HTTP client 保持 adapter 薄。
- 每个 operation 可以独立完成 store → application → HTTP → client → adapter 的纵向切片。

代价：

- 使用 CLI、MCP 或 Desktop 前必须先运行 `kanban serve`。
- host 是本机单用户服务，不提供离线直连、多进程数据库访问或公网 API。
- 未注册或显式 feature-disabled 的路径才返回 `feature_not_available`；当前已接入的 labels、
  signals、search、graph、vector、context、projection 和 maintenance 不得用该错误码
  代替实现。最终 adoption/full 状态仍以 parity ledger 的实际 gate 为准。

### 非目标

本决策不定义多用户/RBAC/云同步、公网 host、自动 server supervision、跨机器 worker 或
发布/PR 工作流。`legacy-sqlite-import` 只是 host 的显式只读导入 feature，不是第二
canonical backend。FTS/vector/graph/context/projection rebuild、backup/maintenance 和
Desktop 历史 surface 已有当前 owner，但完成状态仍由 parity ledger 的实际测试/gate 判断，
不能从本 ADR 的决策文字推断为 release ready。

---

## ADR-0023：最终 Turso 全功能 workspace 收敛

### 状态

已接受；schema adoption、surface audit、full package 和 Desktop check 仍须以实际命令结果
单独记录，不由本 ADR 标为通过。

### 背景

baseline `6ea277` 同时存在窄 queue、外部 projection/helper、独立 backend、多个 adapter
路径和十个 Desktop 视图。继续保留这些进程会让 canonical owner、状态机、依赖和导入语义
出现第二套事实。当前实现已经把完整业务域接回 Turso single-host，文档需要固定最终边界
并明确未运行的验收 gates。

### 决策

1. active 产品单元固定为七个 Rust crate：`kanban-core`、`kanban-service`、
   `kanban-protocol`、`kanban-client`、`kanban-server`、`kanban-cli`、`kanban-mcp`，加
   Desktop `kanban-desktop` 和私有 `xtask`；不新增 backend/helper sidecar。
2. `kanban serve` 是唯一 Turso host/owner。CLI、MCP、Desktop 只能经 typed localhost
   HTTP/SSE；所有 mutation/query 经过 `ApplicationService`、`kanban-core` guard、Turso
   transaction 和 protocol DTO。
3. 搜索使用 Turso FTS `task_search_fts`，向量使用 Turso `vector32` + host Ollama provider，
   图和 context 使用 `kanban-service` canonical relation + bounded BFS/merge。FTS/vector/
   graph/context/projection 可删可重建，provider/index 故障只产生 degraded diagnostics。
4. 数据迁移保留两条路径：Turso v1→v2 原地升级（verified sibling backup + transaction
   rollback）和 portable JSONL/legacy SQLite v30 导入（`import_journal`、staging、fingerprint、
   derived rebuild；v30 仅显式 `legacy-sqlite-import` feature）。导入不创建第二 runtime backend。
5. baseline 的旧 backend、external projection、helper protocol 和 sidecar 已删除；相关
   release/projection 文档只允许标为 historical archive，不得成为 active runbook 或 release
   gate。
6. release、push、PR、merge、发布和外部协调不是本次 architecture/parity 文档的验收条件。
7. API method/path 以 `kanban-protocol::endpoint_catalog()` 为准，CLI canonical leaf 以
   Clap `get_name()` 与 `surface_operation_catalog()` 对齐。当前新增的 board columns、entity
   upsert、task specify、graph neighborhood/map 和 search index rebuild/sync 都属于 active
   surface；visible alias 不形成第二条 contract operation。

### 影响

- 七个 crate、Desktop 和 `xtask` 的依赖方向可由 manifest 与 single-host dependency gate
  审计；不存在第二产品 runtime。
- HTTP、CLI、MCP、Desktop 的 domain surface 可以按 parity ledger 逐域核验，catalog adopted
  不再被误读为 full runtime green。
- canonical Turso 事实、event、run/claim 和导入 journal 的一致性优先于 projection
  availability；FTS/vector/graph/context 的重建和 degraded 状态可观察、可回放。

### 非目标

不引入多用户/RBAC/云同步、公网访问、自动 server supervision、自定义 IPC 或另一套
projection control plane。历史 sidecar 文档不承诺兼容行为，也不替代实际 gate 结果。

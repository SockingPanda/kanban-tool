# Kanban Tool 产品规范

文档类型：持续维护的当前规范
范围：Rust 核心 + 仅 SQLite + CLI + localhost Web；dispatcher 仅保留为实验性实现
约束：无多用户、无多租户、无远程同步、无 PostgreSQL 后端

---

## 1. 产品定位

本工具是一个本地优先的 Kanban 工作系统。它既能作为人类使用的看板，也能作为自动化任务、agent 工作流或本地脚本的持久工作队列。

核心目标：

1. **持久化**：任务、状态、依赖、评论、事件、运行历史必须落盘。
2. **可恢复**：本地进程崩溃后，任务可以通过领取期限（claim TTL）、心跳（heartbeat）与重新领取（reclaim）恢复。
3. **可审计**：每次关键变化写入 `task_events`。
4. **多入口一致**：Web、CLI 和任何保留的实验性执行入口必须走同一套 Rust 用例/服务路径
   （当前主要在 `kanban-sqlite::service`，并复用 `kanban-core` 状态机辅助函数），
   不允许绕过状态机直接写状态。
5. **仅 SQLite**：第一版只支持 SQLite，不设计 PostgreSQL/MongoDB 后端。
6. **单用户本地语义**：actor 是操作来源字符串，用于审计，不用于鉴权。

一句话定义：

> 一个 SQLite 驱动的本地 Kanban 状态机，通过 CLI、桌面界面和 localhost Web API 为人、脚本与 agent 提供同一份任务事实。

---

## 2. 非目标

以下能力不进入当前设计：

- 多用户实时协作。
- 用户表、团队表、权限表、邀请机制。
- 多租户隔离。
- SaaS 部署。
- 跨机器 dispatcher/worker。
- SQLite 文件放在 NFS、Dropbox、iCloud Drive 等同步盘上共享写入。
- 任意自定义工作流编辑器。
- 任意自定义字段数据库。
- 复杂自动化规则引擎。

---

## 3. 核心对象

| 对象 | 说明 |
|---|---|
| Board | 本地项目/看板。不是租户。一个 SQLite 数据库内可以有多个 board。 |
| Task | 看板卡片，也是可执行工作单元。 |
| Status | 权威状态。界面列只是状态的展示映射。 |
| Dependency | 父任务阻塞子任务。 |
| Comment | 人或自动化留下的协作文本；`kind=signal` 用作信号账本回链。 |
| Event | 只追加事件流，用于审计、SSE、调试。 |
| Run | 一次执行尝试。只有 claim/start 后才产生。 |
| Attachment | 附件元数据，blob 存文件系统。 |
| Label | 本地标签。 |
| Label Semantics / Atoms | Label 的权威本体事实，用于本地建议与审查。 |
| Label Proposal / Ontology Ledger | 新 label 候选生命周期与只追加来源记录；它们解释 ontology 演化，但不替代当前 label 事实。 |
| Signal Observation / Signal | 通用 Agent/Product 信号账本；记录产品或 agent 操作信号、审查生命周期和可选 task/run/comment 上下文。 |
| Column | 界面展示配置，映射到 status。 |

---

## 4. 状态模型

权威状态：

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

### 4.1 状态语义

| 状态 | 语义 |
|---|---|
| `triage` | 待澄清、待补全规格、尚不可执行。 |
| `todo` | 已定义，但依赖未完成，或尚未进入 ready 队列。 |
| `scheduled` | 已定义，但 `scheduled_at` 在未来。 |
| `ready` | 可被人工或实验性 dispatcher 领取。 |
| `running` | 已被某个操作者/worker 领取，正在执行。 |
| `blocked` | 因外部依赖、失败、人工输入等原因阻塞。 |
| `review` | 执行完成但需要人工检查。 |
| `done` | 完成。 |
| `archived` | 归档，不参与默认列表和调度。 |

### 4.2 关键原则

1. `running` 只能通过 `claim/start` 状态转换进入。
2. `ready -> running` 必须在单个 SQLite 事务中完成 CAS 更新、创建 run、写 event。
3. `blocked -> ready` 不能盲目设置，必须重新检查依赖与计划时间。
4. 界面拖拽到列时，本质上调用状态转换，不是直接更新 `tasks.status`。
5. CLI 也不能绕过状态转换服务。

完整转换表见 [`STATE_MACHINE.md`](STATE_MACHINE.md)。

---

## 5. 存储模型

### 5.1 SQLite 文件位置

默认路径遵循 XDG 目录约定：

```text
~/.local/share/kb/kb.db
~/.local/share/kb/attachments/
~/.local/state/kb/logs/
~/.config/kanban/config.toml
```

也支持项目本地模式：

```text
.project/
  .kb/
    kb.db
    attachments/
```

通过 CLI 指定：

```bash
kanban --db .kb/kb.db task list
```

### 5.2 SQLite 配置

每个连接初始化时必须执行：

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 120000;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;
```

### 5.3 存储策略

采用：

```text
tasks 当前快照 + task_events 只追加事件流
```

不采用纯事件溯源（event sourcing）。原因：

- 查询当前看板需要快照表，不能每次重放事件。
- 事件流用于审计、实时推送、调试、增量同步到 Web UI。
- 快照与事件必须在同一事务内更新。

初始 schema 见 [`../migrations/001_initial.sql`](../migrations/001_initial.sql)。

### 5.4 Label 本体事实与来源记录

Label 系统保持四类角色分离：

1. `labels` / `task_labels` 是任务当前绑定事实。
2. `label_semantics` / `label_atoms` 是 label 的权威本体事实。
3. `lancedb_label_atoms` 等向量索引是可删除、可重建的派生检索层。
4. `label_semantic_proposals` 与 `label_ontology_*` 账本记录候选、分歧、审查、
   变更来源和验证历史。

当前不引入 label ontology 专属图投影。现有 `kanban graph` / Oxigraph 只消费
Knowledge Substrate 的 `entity_relations` 镜像；ontology 审查、atom 解释、proposal
和验证历史直接从 SQLite 权威数据读取。未来若为 rename/split/merge 或来源
关系查询增加图投影，它必须是可删除、可重建的派生存储，不能拥有权威写入路径。

会构造新事实的 ontology 变更必须通过专用服务路径：semantics patch/replace、
atom apply、task-label bootstrap、proposal create/accept 和 validation 都要在同一 SQLite
事务中写入对应权威记录与来源 action，或一起回滚。通用 ontology action endpoint
只允许生命周期审查 action，不能伪造权威的 before/after hash、result atom/result label、
result proposal 或验证证据。

已提交的 label 范围 semantics/atom 变更可以通过专用撤销路径追加
`revert_ontology_mutation` action：它要求当前权威 hash 仍等于被撤销 action 的
after hash，将 semantics 恢复到 before snapshot，标脏 atom 索引，并保留原 action
历史。该路径不承担 bootstrap label identity 或 task binding 回滚。

Semantics upsert 默认是 patch，不是完整替换：缺省字段保留当前值，数组字段追加或按
`remove_*` 删除；只有显式 replace 才把缺省数组解释为空。`expected_semantics_hash`
用于防止更新丢失。Proposal accept 与单 task bootstrap 共用 new-label adoption
primitive；proposal accept 不自动写 `task_labels`，bootstrap 会绑定来源 task。旧数据或
cleanup 路径中缺少 action 来源记录的 atom 只能通过 `legacy_untracked=true` 标记，不应
被当作新的 ontology 增长方式。
当 apply atom 发现同内容 atom 已存在时，只写 `adopt_existing_atom` 这一仅记录来源的 action；
它把新的来源信号连接到既有 atom，不修改权威 semantics/atoms，也不标脏派生 atom 索引。

---

## 6. 桌面端能力

桌面端是使用 Web 技术栈构建的 Tauri 本机界面，不是由 `kanban serve` 托管的浏览器
看板，也不是远程协作服务。它通过本机 HTTP API 工作。

Tauri 内嵌 API 绑定到回环地址上的随机可用端口：

```text
127.0.0.1:<动态端口>
```

独立运行的 `kanban serve` 才默认使用 `127.0.0.1:8721`。

主要页面：

1. Board 看板页。
2. Task 详情抽屉。
3. 评论。
4. 事件时间线。
5. Run / 执行历史。
6. 筛选/搜索。
7. 设置。

桌面端前端只调用 HTTP API，不直接访问 SQLite。

API 见 [`API_SPEC.md`](API_SPEC.md)。

---

## 7. CLI 能力

CLI 是一等入口，必须覆盖核心生命周期：

```bash
kanban init
kanban board list
kanban board create agent-work --name "Agent Work"
kanban board use agent-work
kanban task create "实现 SQLite schema"
kanban task list --status ready
kanban task show agent-work#1
kanban task show t_xxx
kanban task start t_xxx
kanban task heartbeat t_xxx --claim-token <token>
kanban task block t_xxx "等待接口确认"
kanban task unblock t_xxx
kanban task done t_xxx --claim-token <token>
kanban task archive t_xxx
kanban events t_xxx
kanban runs t_xxx
kanban serve
```

CLI 必须支持：

- `--json`：机器可读输出。
- `--db <path>`：指定 SQLite DB。
- `--board <slug-or-id>`：显式指定当前 board。
- `--actor <name>`：覆盖操作者标识。
- 稳定退出码。

当前 board 的选择顺序是 `--board`、`KB_BOARD`、最近的 `.kb/config.toml`、`default`。`kanban board use <board>` 写入项目级 `.kb/config.toml`，但仍使用同一个全局 SQLite 数据库。Task 引用必须支持全局 `t_...`、当前 board 的裸 seq / `#seq`、以及显式 `board#seq` / `board/#seq`；CLI 和 API 输出应带可复制的 `board_slug#seq` 引用。

CLI 见 [`CLI_SPEC.md`](CLI_SPEC.md)。

---

## 8. 核心不变量

实现必须保证：

1. 一个 task 同时最多一个有效领取。
2. 一个有效领取必须有一个有效 run。
3. `running` task 必须有 `claim_token`、`claim_owner`、`claim_expires_at`。
4. task 不能依赖自己。
5. 依赖图不能形成环。
6. 有未完成父任务的子任务不得进入 `ready/running`。
7. `archived` task 不参与默认 list、promotion、claim。
8. `done` 和 `archived` 是类终态；默认不再被自动执行入口修改。
9. 已归档 board 不接受普通 task/comment/自动执行入口写入；只读 events/runs/comments 历史仍可审计。
10. Board archive 不会改变 task 状态；如果 board 上仍有 `running` task/run，必须拒绝 archive。
11. 每次状态变化必须写 `task_events`。
12. task snapshot 与对应 event 必须同 transaction 提交。
13. `tasks.status`、label 绑定事实、label semantics 事实、ontology 账本和派生检索层各自有明确写权限；派生存储不拥有权威写入路径。
14. 新的构造性 ontology 变更不通过通用生命周期 action endpoint；必须由专用 command/API/service 路径同时写入权威状态与来源 action；采用已存在 atom 时只写 `adopt_existing_atom` 来源 action，不伪装成新增 atom。
15. label ontology 图投影当前不存在；如未来新增，只能从 SQLite 权威数据派生并重建，不得成为 `labels`、`task_labels`、`label_semantics`、`label_atoms` 或 `label_ontology_*` 的写入口。
16. label ontology 纵向回归语料集是测试/评估基础设施：它可比较固定语料集的 selected labels、score 和 evidence atoms，但语料集运行本身不得修改权威 label/ontology/ledger 事实，也不得成为日常 task label 绑定的默认流程。
17. label ontology 质量分析是只读投影：分母来源必须可审计；原始分歧信号数不得被命名或解释为模型错误率、precision 或 recall。没有带 expected labels 的独立评估批次时，precision/recall 必须显示为 unavailable。

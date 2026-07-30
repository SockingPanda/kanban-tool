# Kanban Tool 规范合集

本文档由以下文件合并而成：

- README.md
- docs/SPEC.md
- docs/ARCHITECTURE.md
- docs/STATE_MACHINE.md
- docs/DATA_MODEL.md
- docs/CLI_SPEC.md
- docs/API_SPEC.md
- docs/SCHEMA_CONTRACTS.md
- docs/ADR.md
- migrations/001_initial.sql
- migrations/003_comment_author_identity.sql

`docs/SPEC.md`、`docs/CLI_SPEC.md`、`docs/API_SPEC.md`、`docs/STATE_MACHINE.md` 和 `docs/SCHEMA_CONTRACTS.md` 等分主题文档是当前行为的权威来源；本文件是这些源文档的同步快照，便于一次性阅读和离线传递。


---

# 文件：README.md

# Kanban Tool

**一个放在自己电脑上的看板，也是一条可以被人、脚本和 AI agent 共同使用的可靠工作队列。**

很多看板只负责展示“任务现在在哪一列”。一旦真正开始执行，问题就会变得更具体：

- 这项工作真的可以开始了吗，还是仍被依赖阻塞？
- 是谁接走了任务？执行进程中断后，任务会不会永远卡在“进行中”？
- 人在界面上操作、脚本修改状态、agent 更新任务时，三者看到的是不是同一份事实？
- 几天以后，还能不能说清楚任务为什么被阻塞、何时恢复、经历过哪些尝试？

Kanban Tool 想解决的就是这些问题。

它把每张卡片当成一个有生命周期的工作单元：任务、依赖、评论、执行记录和状态变化都保存在本地 SQLite 中。

桌面界面适合人查看和操作，CLI 适合脚本与 agent。所有受支持入口共享同一套状态机，不会各自维护一份互相打架的状态。

## 你可以拿它做什么

- **管理个人项目**：用看板整理任务，用明确的状态区分“还没想清楚”“暂时做不了”和“现在可以执行”。
- **给本地 agent 一份长期工作清单**：任务不会随着一次对话或一个进程结束而消失。
- **连接人工与自动化流程**：人可以创建、澄清和验收任务，脚本或 worker 可以领取、心跳、提交结果。
- **保留可追溯的过程**：关键变化写入事件记录，每次执行尝试、评论和阻塞原因都能回看。
- **在本机统一多个项目**：一个 SQLite 数据库可以容纳多个 board，同时保持清晰的项目边界。

如果你只想要一个多人在线协作的云看板，Trello、Linear 或 GitHub Projects 会更合适。Kanban Tool 面向的是另一种场景：**单用户、本地优先、需要可靠状态和自动化入口的工作流。**

## 它怎样工作

一项任务通常会经过这样的过程：

```text
主流程：triage → todo / scheduled → ready → running → review → done
异常：  活动状态 → blocked → 重新检查 → triage / todo / scheduled / ready
```

- `triage`：想法还不够清楚，暂时不能执行。
- `todo`：任务已经定义，但依赖尚未完成，或还没有进入执行队列。
- `scheduled`：任务有明确的未来开始时间。
- `ready`：条件已经满足，可以被操作者或显式调用的客户端领取。
- `running`：任务已被领取，拥有一次真实的执行记录。
- `blocked`：执行遇到外部依赖、失败或需要人工输入。
- `review`：工作已经提交，等待人工确认。
- `done`：任务完成。

这里的状态不只是看板上的列。比如 `ready → running` 会以原子事务完成领取、创建 run 并写入事件；超时的领取可以被回收。多个活动状态都可能进入 `blocked`；解除阻塞时，系统会重新检查任务规格、排期和依赖，再决定它应该回到 `triage`、`todo`、`scheduled` 还是 `ready`。

这也是 Kanban Tool 与普通任务列表最核心的区别：**它不只记录你想做什么，也认真记录一项工作是否真的能够执行，以及执行过程中发生了什么。**

## 快速开始

目前项目仍处在“从源码构建、先在真实工作中使用和打磨”的阶段，GitHub 暂无可直接下载的预编译 Release。安装 CLI 需要 [Rust](https://www.rust-lang.org/tools/install)：

```bash
git clone https://github.com/SockingPanda/kanban-tool.git
cd kanban-tool
cargo install --path crates/kanban-cli --bin kanban
```

初始化数据库，创建并选中一个 board：

```bash
kanban init
kanban board create personal --name "Personal"
kanban board use personal
```

创建第一项工作：

```bash
kanban task create "整理项目首页" \
  --description "让第一次访问的人看懂项目" \
  --priority 1
kanban task list
```

你会看到类似下面的摘要：

```text
personal#1 [todo] P1 整理项目首页 · plan: unplanned · steps: 0/0
```

Kanban Tool 要求可执行任务明确说明 execution plan（执行计划）。这个任务只有一步，可以直接标记为不需要拆分；系统会重新计算条件并把它推进到 `ready`：

```bash
kanban task step not-required personal#1 --reason "单步任务"
kanban task list
# personal#1 [ready] P1 整理项目首页 · plan: not_required · steps: 0/0
```

开始任务时，Kanban Tool 会返回一个 claim token（领取凭证）：

```bash
kanban task start personal#1
# Claimed ... token=claim_...
```

完成任务需要带回这个 token，从而避免另一个过期或并发执行者误改状态：

```bash
kanban task done personal#1 --claim-token claim_...
```

常用的下一步：

```bash
kanban task show personal#1 --details
kanban task list --status ready --status running
kanban search "项目首页"
kanban doctor
kanban maintenance status
kanban maintenance run --once
```

## 三种使用方式

### 桌面看板

`apps/desktop` 提供 Tauri 桌面操作界面，适合浏览 board、查看任务详情和进行人工操作。它不是独立的数据层；界面上的状态变化仍然经过与 CLI 相同的 Rust service 和状态机。

当前公开的快速开始只覆盖 CLI；Desktop 尚无预编译下载，需要按照仓库开发流程从源码构建。

### CLI 与本地脚本

`kanban` CLI 覆盖 board、task、dependency、step、comment、event、run、search、backup 和 maintenance 等入口。大多数命令支持 `--json`，因此既适合人直接使用，也适合作为自动化程序的稳定接口。

### 本地 HTTP API

`kanban serve` 默认在 `127.0.0.1:8721` 启动本地 HTTP API 和 SSE 事件流，供 Desktop 或本地脚本使用。它不会提供浏览器版看板，也不会监听公网地址。

## 数据放在哪里

Kanban Tool 不要求远程数据库。在 Linux 上，默认数据通常保存在 XDG 目录：

```text
~/.local/share/kb/kb.db
~/.local/share/kb/attachments/
~/.local/state/kb/logs/
~/.config/kanban/config.toml
```

其他平台或自定义环境可以通过 `kanban config show` 查看实际解析出的路径。

也可以把数据库放进某个项目的 `.kb/` 目录：

```bash
kanban init --db .kb/kb.db
```

SQLite 始终是最终事实来源；搜索、图和向量能力都可以从它重新构建。项目不支持把同一个 SQLite 文件放在 NFS、Dropbox、iCloud Drive 等同步目录中由多台机器共同写入。

## 当前边界

Kanban Tool 有意保持本地、单用户：

- 不提供多用户、团队、邀请或 RBAC。
- 不提供多租户 SaaS。
- 不提供云同步或远程 worker。
- 不支持 PostgreSQL、MySQL 或 MongoDB 后端。
- localhost API 只服务本机界面和本地脚本，不是公网协作 API。
- 仓库中保留的实验性 dispatch 命令不属于公开支持能力；自动化请使用 CLI 或本机 API 显式编排。

这些不是“以后再补的企业功能清单”，而是当前产品边界。它让项目可以把精力放在本地任务状态、恢复能力、审计记录和人机协作上。

## 想深入了解

不需要从头读完整个文档包。可以按问题选择入口：

| 你想了解什么 | 从这里开始 |
|---|---|
| 产品范围和核心概念 | [`docs/SPEC.md`](docs/SPEC.md) |
| CLI 命令和输出 | [`docs/CLI_SPEC.md`](docs/CLI_SPEC.md) |
| 状态为什么这样流转 | [`docs/STATE_MACHINE.md`](docs/STATE_MACHINE.md) |
| 本地 API 与 SSE | [`docs/API_SPEC.md`](docs/API_SPEC.md) |
| Rust crate、进程和数据流 | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| 数据对象、ID 与事件 | [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md) |
| JSON schema 与公开契约 | [`docs/SCHEMA_CONTRACTS.md`](docs/SCHEMA_CONTRACTS.md) |

[`KANBAN_SPEC_BUNDLE.md`](KANBAN_SPEC_BUNDLE.md) 是主要文档的单文件同步快照，适合离线阅读或一次性交给其他工具处理。

## 参与开发

项目是一个 Rust workspace，桌面端位于 `apps/desktop`。开始修改前请先阅读 [`AGENTS.md`](AGENTS.md)，其中记录了架构边界和验证约定。

查看可用的开发命令：

```bash
just --summary
```

针对当前改动选择最小验证集：

```bash
just affected-plan base="main"
just affected base="main"
```

README 和其他规范源文档发生变化后，需要同步检查单文件文档包：

```bash
just spec-bundle-generate
just spec-bundle-check
```

Linux CLI 可以构建为独立的 Debian 包：

```bash
./scripts/package-cli-linux.sh --format deb \
  --no-default-features \
  --features tantivy-backend,oxigraph-backend
```

桌面包与 CLI 包彼此独立；桌面包不会自动安装系统级 `kanban` 命令。

当前工程验证和安装打包主要围绕 Debian / Ubuntu；其他平台可以从源码尝试，但项目暂未提供同等完成度的安装包承诺。

## 许可证

Kanban Tool 使用 [Apache License 2.0](LICENSE) 开源。


---

# 文件：docs/SPEC.md

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

完整转换表见 [`STATE_MACHINE.md`](docs/STATE_MACHINE.md)。

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

初始 schema 见 [`../migrations/001_initial.sql`](migrations/001_initial.sql)。

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

API 见 [`API_SPEC.md`](docs/API_SPEC.md)。

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

CLI 见 [`CLI_SPEC.md`](docs/CLI_SPEC.md)。

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


---

# 文件：docs/ARCHITECTURE.md

# 架构

Kanban Tool 把本机 SQLite 作为唯一权威事实来源。CLI、Tauri 桌面端和本机 HTTP API
共享同一组 Rust 用例与状态机；搜索、图和向量存储都是可重建的派生层。

本架构面向本地单机运行：Rust 工作区、SQLite、CLI、本机 HTTP API、Tauri 桌面端，
以及暂不作为公开支持能力的实验性 dispatcher 入口。

---

## 1. 总体架构

```text
Tauri Desktop
  -> kanban-server handler/DTO
        \
kanban-cli \
dispatcher  -> kanban-application API / DTO 契约
                     | 由 kanban-sqlite::api / SqliteApplication 实现
                     | 使用 kanban-core 的纯状态机辅助函数
                     v
                权威 SQLite WAL
                     |
                     | task_events / index_outbox / 脏代际标记
                     v
                可重建派生存储
                (Tantivy / Oxigraph / LanceDB)
```

当前实现已经把一组已选择的面向适配器的 DTO/port 垂直切片抽到 `kanban-application`；它不是完整的 application service，也不拥有 SQLite 事务。CLI、HTTP
server、desktop 和 dispatcher 通过 `kanban_sqlite::api` 或 `SqliteApplication` 进入同一组
SQLite 支持的用例；`kanban-sqlite::service` 仍是事务、状态机保护、权威
写入、events、runs、outbox 和来源记录的实现 owner。`kanban-core` 承载
`TaskStatus`、ID/error/clock 和纯状态机辅助函数，不拥有持久化记录。

`kanban-sqlite` crate 根模块不再重新导出数据库/init/service 符号。生产适配器必须导入
`kanban_sqlite::api`、`kanban_sqlite::application::SqliteApplication`，或显式的
`kanban_sqlite::db` / `kanban_sqlite::init` 基础设施模块。测试原始检查入口集中到
`kanban-test-support`，crate 内部测试可使用显式 `db` / `init` 模块。

可把系统按八个运行平面理解：

| 平面 | 当前内容 | 写权限边界 |
|---|---|---|
| 交互/适配器 | `kanban-cli`、`kanban-server`、desktop、dispatcher 入口 | 转换输入/输出和 locale/message 渲染，不直接写 SQLite 事实 |
| Wire 契约 | `kanban-contract` 的候选 Serde DTO、精确公开面目录、操作清单与 schema 根注册表 | 只定义公开机器契约候选；只有 `Adopted` 条目表示运行时采用，不拥有 service 保护、SQLite 记录或运行时验证 |
| Schema 工具 | `kanban-schema-tool` 的 `kanban-schema` 二进制程序、metaschema/fixture 校验、manifest/hash 和漂移门禁 | 独立叶子工具，不进入产品运行时依赖图，也不能充当采用 witness |
| 应用契约 | `kanban-application` 已选择的用例 DTO/port API，SQLite 实现位于 `kanban-sqlite` | 适配器逐步依赖稳定 API/DTO；该 crate 不是完整 application service |
| 领域/状态机 | `kanban-core` 的 status、保护和重新计算辅助函数 | 纯逻辑，不访问 SQLite/HTTP/CLI |
| 权威 SQLite 事实 | tasks/status、dependencies、labels、semantics、proposals、ontology 账本 | 只能由 service 路径写入 |
| 传播/控制平面 | `task_events`、`index_outbox`、dirty/generation/status 标记 | 记录同步水位和恢复入口，不替代事实 |
| 可重建派生存储 | Tantivy、Oxigraph、LanceDB `kb_chunks` / `kb_label_atoms` | 可删除重建，无权威写入路径 |

---

## 2. Crate 结构

当前主要仓库结构（省略测试、脚本、生成文件和部分支持文件）：

```text
crates/
  kanban-core/
    src/
      domain/
      state_machine.rs
      error.rs
      clock.rs
      id.rs

  kanban-contract/
    src/
      wire.rs
      inventory.rs
      schema.rs

  kanban-application/
    src/
      lib.rs

  kanban-schema-tool/
    src/
      lib.rs
      bin/kanban-schema.rs

  kanban-sqlite/
    src/
      db.rs
      init.rs
      service.rs
      service/
        sql.rs
        transaction.rs
        boards.rs
        tasks.rs
        transitions.rs
        dispatch.rs
        search.rs
        ...

  kanban-cli/
    src/
      main.rs
      commands/
      output.rs

  kanban-server/
    src/
      dto.rs
      handlers/
      router.rs
      state.rs

  kanban-context/
  kanban-entity/
  kanban-graph/
  kanban-indexer/
  kanban-labels/
  kanban-local/
  kanban-search/
  kanban-vector/

apps/
  desktop/
```

Desktop 包由 Tauri 构建，内置 `kanban-vector-lancedb` 与
`kanban-graph-oxigraph` 辅助进程。Desktop 启动内嵌 server 时把已有的
内置辅助进程路径注入 `kanban-server::AppState`；CLI `.deb` 仍由
`scripts/package-cli-linux.sh` 独立安装 `/usr/bin/kanban` 与 `/usr/lib/kanban/` 下的辅助程序。

### 2.1 `kanban-core`

职责：

- 定义基础领域类型：`Board`、`BoardColumn`、`TaskStatus`。
- 提供类型化 ID、clock 和统一错误类型。
- 实现纯状态机、ready 重新计算与状态转换保护辅助函数。
- 提供轻量 locale 与消息渲染辅助函数；只渲染用户可见文案，不翻译权威 status、ID、JSON key 或数据库值。
- 不依赖 SQLite、HTTP、CLI、前端。
- 当前不定义完整命令输入/输出，也不定义 application service 接口。
  这些用例编排和持久化记录主要在 `kanban-sqlite::service`。

示例：

```rust
pub enum TaskStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
    Running,
    Blocked,
    Review,
    Done,
    Archived,
}

pub fn initial_status(...) -> TaskStatus;
pub fn recompute_ready_status(...) -> TaskStatus;
pub fn can_promote_from(status: TaskStatus) -> bool;
pub fn can_complete_from(status: TaskStatus) -> bool;
```

### 2.1a `kanban-contract`

职责：

- 为逐步迁入的公开 API、CLI、JSONL、SSE、结构化元数据、配置和辅助进程
  wire DTO 提供唯一候选归属；适配器迁移时负责 application/SQLite 记录到 wire DTO
  的显式映射。
- 默认 feature 只包含轻量 Serde 类型；唯一增量 `schema` feature 启用 `schemars`
  并公开 schema 根注册表。该 crate 不拥有二进制程序、`jsonschema`、SHA-256 或漂移工具。
- 用精确公开面目录枚举实际 Axum method/path、Clap 叶子命令和 JSONL
  discriminator；对应测试从真实声明生成 key，新增公开入口而未登记时自动失败。
- 对 API/SSE contract 显式记录 `operation_key`、`Path|Query|Headers|Body|Success|Error|Sse`
  location 和参数 cardinality；非 HTTP surface 显式记录 `NoTransport`。`Success` 只表达 2xx
  success，`Error` 只表达 `SharedComponent` 非 2xx response，且不新增第七 endpoint obligation。
  传输验证器负责 direction/location、operation/surface、granularity、path placeholder 和
  重复/缺失参数的失败关闭拓扑校验，不承担 HTTP status 或业务语义。
- 用操作清单明确每个公开面的方向、严格性、fixture、schema ID
  或 exclusion，并区分 `Planned`、`Generated`、`Adopted`、`Excluded`。`Generated`
  只表示离线 schema/fixture 就绪；`Adopted` 还必须绑定 direction-correct evidence：
  request/input producer 由 contract DTO 程序化序列化并精确匹配已提交 fixture，consumer
  从 fixture 经真实运行时 handler；response/output producer 来自真实适配器。双方包含
  operation/contract/surface/direction 和精确 Cargo test locator，且不共用同一个高层 exercise
  helper。端点整体迁移状态与六项义务分开收敛：
  `Generated` 端点可以先把已迁移的 body 声明为 adopted 精确 contract，但其它
  义务仍为 `Todo` 时不能提升为 `Adopted`；审计要求该 contract 与运行时 operation
  唯一、双向且精确绑定。witness gate 以 canonical manifest 和 Cargo package ID 锁定
  当前 workspace `kanban-contract`，要求无条件、非可选的普通依赖声明与默认
  resolve edge，并以 `--all-features --target all --edges normal,features --locked` 扫描采用者的运行时
  泄漏，随后真实执行双方测试；registry/git/其它 path 的同名 package 不构成采用。最终收口门禁只允许
  `Adopted` 或 `Excluded`。
- 生成显式 Draft 2020-12、离线
  `urn:kanban-tool:schema:<surface>:<semantic-name>:v1` root；schema 字节从候选 wire 类型
  确定生成。fixtures 是手写正负样例，用于验证 schema 与当前候选 wire shape；
  它们本身不构成运行时采用证据。

该 crate 不依赖 `kanban-sqlite`、`kanban-server`、`kanban-cli`、desktop、
dispatcher 或重型辅助后端。JSON Schema 只验证 wire 结构/值域，
不能替代状态机、CAS、依赖、重新计算、事务或评论语义保护。
详细生成与验证契约见 [`SCHEMA_CONTRACTS.md`](docs/SCHEMA_CONTRACTS.md)。

### 2.1b `kanban-schema-tool`

职责：

- 独占 `kanban-schema` binary、离线 inventory audit、metaschema/fixture 校验、
  committed artifact 写入/漂移检查和 SHA-256 manifest。
- direct dependency 必须且只能是 `jsonschema`、`kanban-contract/schema`、`serde`、
  `serde_json` 与 `sha2` 这 5 条 normal edge；不得声明 dev、build、optional、alias 或
  target-specific edge，也不得依赖任何产品或内部 workspace crate。
- `autolib`、`autobins`、`autoexamples`、`autotests`、`autobenches` 与 auto build script
  全部关闭；只允许显式声明的一个 lib、一个 bin 与一个 integration test。contract 同样
  只允许一个 lib 和两个显式 integration tests；metadata 与普通文件/symlink gate 锁定
  target name、kind、lexical `src_path` 和仓库内归属。
- dependency policy 从 tool manifest 运行 full locked `cargo metadata`，锁定
  `resolve.root`、canonical tool/contract package ID 与 manifest path、五条 resolved
  direct edge、启用 `kanban-contract/schema` 后批准的逻辑 registry
  `schemars 1.2.1` edge，以及 `jsonschema=[]`、
  `schemars=[derive,schemars_derive,std]` effective feature union，并拒绝
  tool-root reachable closure 中的其它 path/git override。
- `policy/schema-tool-registry-closure.json` 是独立治理数据边界，只包含当前
  tool-root closure 的 registry packages；两个 canonical workspace path packages 不进入
  snapshot。policy 解析真实 `Cargo.lock`，要求每个 reachable registry package 唯一映射到
  64 位小写十六进制 checksum，并与按 `(name, version, source)` canonical 排序的 committed
  `{name, version, source, checksum}` 集合双向完全一致。普通 gate 只比较，禁止自动写入或
  bless；该检查证明 committed lockfile 相对 approval 的漂移，crate 内容仍由 Cargo
  fetch/build 按 registry index `cksum` 验证。
- Cargo metadata 的 `SourceId` 是 opaque identity；这里锁定的是 pinned toolchain 下
  本项目批准的 logical SourceId 字符串，不宣称其中 URL 是 Cargo 通用 canonical network
  URL。物理 index/download 可由 Cargo source replacement mirror 提供，不要求直连
  crates.io 原始来源。
- 除该 tool 自身外，任何 workspace member 都不得以任何 dependency kind、alias、optional
  或 target-specific direct edge 引用它；六个产品 runtime graph 另由 all-features/all-target
  cargo tree gate 扫描传递性 tooling 泄漏。
- 作为 workspace leaf crate 排除在 default/core/helper/full 产品门禁之外。产品 `fmt`
  （及 `fmt-check` alias）精确选择 core packages，`fmt-full` 精确选择 core + helper，
  `schema-fmt` 则只选择 `kanban-contract` + `kanban-schema-tool`，并且必须在 schema
  dependency preflight 之后执行；不存在 workspace-wide fmt 旁路。
- 真实 `just --dump-format json --dump` parser AST hash 与 fake nested
  `just`/build-lock/cargo/python/script 有序 JSONL trace 形成双门禁，锁定上述 fmt lane、
  full/rust/test 分支、schema 子 gate、`schema-audit-closed` 的 adoption + locked audit，
  以及 `release` 从 affected self-test、显式 Tantivy/Oxigraph Projection cohort 到
  diff-check 的 14 步精确顺序。leaf 仅由独立
  schema gates 执行格式、check、tests、clippy、生成和校验；witness gate 显式拒绝该
  tooling owner 冒充 runtime adopter。

### 2.2 `kanban-sqlite`

职责：

- SQLite 连接初始化。
- migration。
- 事务封装。
- application/service 编排与 repository 实现。
- 复杂查询。
- CAS claim。
- 追加 event。
- task/comment/dependency/run/label/ontology 用例。
- label proposal 验证/持久化，以及 `LabelProposalProvider` trait 边界。

公开 API 边界：

- `kanban_sqlite::service` 是实现 owner，负责事务、状态机保护、
  权威写入、events、runs 和来源记录。
- `kanban_sqlite::api` 根模块是面向适配器/产品用例的精选 facade，用于 CLI、server、desktop
  和 dispatcher contract path 复用已允许的 use case、query、record 和 provenance 类型。它不拥有新的
  编排语义，不是 `service::*` 的宽泛重新导出，也不导出数据库连接辅助函数、init
  辅助函数、运行时生命周期保护、provider/vector-store seam，或未列入允许清单的 service-only
  实现辅助函数。
- `kanban_sqlite::api::provider` 承载 adapter/test 需要显式注入 provider 或 vector store 的 seam，
  包括 `LabelProposalProvider`、manual/disabled proposal provider、`*_with` label suggestion/proposal
  helpers、label atom/vector-store status/query/rebuild/sync helpers，以及 trusted-suggestion validation DTO。
  这些符号不从 `api` root 暴露。
- `kanban_sqlite::api::lifecycle` 承载进程运行时/替换生命周期管线：
  `DatabaseRuntimeGuard`、`DatabaseReplaceGuard`、`begin_database_runtime` 和
  `begin_database_replace`。这些保护是二进制程序/运行时 owner 的基础设施，不是普通产品用例。
- `kanban_sqlite::db` 和 `kanban_sqlite::init` 仍是显式基础设施模块；`connect_file`、
  `init_database` 不从 `api` root 暴露。
- crate 根模块不再提供 `kanban_sqlite::*` 旧版重新导出；旧根路径是破坏性变更，
  并由 `tests/ui/root_legacy_reexport_removed.rs` 负向编译契约锁定。`api` 根模块、
  `api::provider`、`api::lifecycle` 和显式 `db` / `init` 边界由 `public_api` trybuild contract 锁定。
- `kanban_sqlite::application::SqliteApplication` 实现 `kanban-application` 的 backend port，
  用于需要以 application API 组合 selected use-case slice 的 adapter/benchmark 路径。
- `kanban-application` DTO/trait 演进遵循 additive-first 策略：优先新增可选字段、option
  struct 或 extension trait；破坏性 DTO/trait 变更必须和 adapter 更新、public API compile
  contract 同步提交。

关键要求：

- 所有状态变化必须在事务内完成。
- claim 必须使用 `BEGIN IMMEDIATE` 或等价机制抢写锁。
- 不允许业务层执行裸 SQL 更新状态。
- `kanban-sqlite` 不直接依赖 LLM SDK、HTTP AI client、runtime credentials 或外部模型
  provider。真实 label proposal provider 只能在 `kanban-server`、`kanban-cli` 本地
  runtime、或单独 `kanban-ai` / `kanban-llm` crate 中实现，再通过
  `LabelProposalProvider` trait 注入 SQLite service。

### 2.3 `kanban-cli`

职责：

- 解析命令。
- 构造 command input。
- 调用 `kanban_sqlite::api` root 中的 shared use-case 函数；需要 provider/vector-store seam 时显式使用
  `kanban_sqlite::api::provider`，需要 runtime guard 时显式使用 `kanban_sqlite::api::lifecycle`；状态判断复用
  `kanban-core` 的纯状态机 helper。
- 输出人类可读表格或 JSON。
- 返回稳定退出码。
- `--locale` / `KANBAN_LOCALE` 只选择人类可读输出语言；脚本契约仍以 `--json` 为准。

CLI 可以直接打开 SQLite 数据库调用 service，不需要 server 常驻。

### 2.4 `kanban-server`

职责：

- localhost HTTP API。
- 只提供本机 API，不托管浏览器版看板。
- SSE 事件流。
- 请求 DTO 转换为命令输入。
- 错误格式统一。
- 根据 `Accept-Language` 渲染 `error.message`；`error.code` 和 JSON shape 保持稳定。
- 通过 `AppState` 接收可选 graph/vector helper binary path；缺失时 graph/vector
  status endpoint 返回 degraded diagnostics，而不是把 helper-heavy crates 编进 server。

独立运行 `kanban serve` 时默认只监听：

```text
127.0.0.1:8721
```

Tauri Desktop 的内嵌服务器绑定 `127.0.0.1:0`，由操作系统选择可用端口。

### 2.5 `kanban-vector`

职责：

- 定义可重建向量派生层的数据结构和错误模型。
- `EmbeddingProvider` 只表示外部 embedding provider 的文本向量化能力。
- `ChunkVectorStore` 表示 task chunk derived index 的 upsert/delete/query 能力。
- `LabelAtomVectorStore` 表示 label atom derived index 的 upsert/delete/query 能力，并提供 suggestion/proposal 所需的 query-text embedding。
- `VectorStore` 只是兼容组合 trait；`LanceDbStore` 可以同时实现 chunk 和 label atom 能力，但上层服务应按实际能力依赖更窄的 trait。

边界要求：

- chunk context/rebuild 路径只依赖 `ChunkVectorStore`。
- label suggestion/proposal/atom-index 路径只依赖 `LabelAtomVectorStore`，不依赖 chunk store 语义。
- CLI/server no-heavy 路径通过 subprocess helper adapter 连接 graph/vector 派生层；
  context chunk 查询走 chunk commands，label suggestion/proposal、bootstrap staged verification
  和 label atom status/rebuild/query 走 label atom 专用 helper commands。label atom helper 在
  helper 进程内使用真实 `LanceDbStore` 写 `lancedb_label_atoms`，并通过 `kanban-derived-io` 的窄
  SQLite IO 更新 `LANCEDB_LABEL_ATOMS_STORE` / `label_atom_index_boards` 状态；server/CLI 不把
  chunk store `status` 当作 label atom 状态。
- label atom 场景获取 model 名称时使用通用 `VectorStoreBackend::embedding_model()`；`chunk_embedding_model()` 仅作为 chunk 路径的兼容入口。
- LanceDB 表仍按 derived store 隔离：task chunks 写入 `kb_chunks`，label atoms 写入 `kb_label_atoms`。

### 2.6 标签提案 provider 边界

语义 label proposal 分成两层：

```text
上层 provider
  - 人工/离线候选项输入
  - 未来的本地 LLM / AI 运行时集成
  - credential、模型配置与 HTTP/client 关注点
        ↓ LabelProposalProvider
kanban-sqlite
  - task/建议上下文查询
  - 确定性验证
  - 残差 top1+margin 门禁
  - proposal 持久化与 accept/reject 生命周期
```

`kanban-sqlite` 只接受 `LabelProposalProvider` trait object，不拥有真实 LLM provider。
默认 `DisabledLabelProposalProvider` 只产生降级尝试；`ManualLabelProposalProvider`
用于 CLI/API 显式传入的本地/离线候选项。未来真实 provider 的候选位置是
`kanban-server`、本地运行时或独立 `kanban-ai` / `kanban-llm` crate，并且必须保持
SQLite service 不知道 credential、HTTP transport、prompt 模板或外部 SDK。

### 2.7 标签本体角色

Label 系统有六个角色，但不是六个严格独立的存储层：

1. `labels` / `task_labels`：权威 label identity 与 task 当前绑定事实；基础 identity
   CRUD 是词汇表注册表，不写 ontology 账本。
2. `label_semantics`：权威 ontology semantics；`label_atoms` 是从 semantics 与 label
   name 展开的 SQLite 物化投影。
3. `kb_label_atoms` / `label_atom_index_boards`：可重建 label atom 派生检索。
4. `label suggest`：基于当前 task、atom 和向量证据的计算/诊断，不是持久事实。
5. `label_semantic_proposals`：候选新 label 的生命周期记录，accept 前不改变当前 task-label 事实。
6. `label_ontology_*` 账本：observation、signal、action、validation 来源记录。

Proposal 与账本是 SQLite 权威记录，因为它们需要审计和可查询历史；但它们不替代
`task_labels` 的当前绑定事实，也不替代 `label_semantics` 的 ontology semantics。
账本覆盖 semantics/atom 变更来源；`labels` identity create/delete 位于
账本之外。
正式文档使用“权威事实”“派生检索”“proposal 工作流”和
“ontology 来源记录”这些边界词；不要把未定义的内部简称写成架构术语。

### 2.8 标签本体图边界

当前没有 label-ontology 专属 graph projection。`kanban graph` / Oxigraph 只镜像
`entity_relations` 中已有的 Knowledge Substrate 关系，例如 task-board 与 task dependency；
label ontology 的 query surface 仍是 SQLite ledger、proposal、semantics、`label ontology
review`、`label atom explain` 和 validation history。

在 rename/split/merge provenance 查询或跨 action 关系查询出现明确需求前，不新增
ontology graph store、ontology RDF schema 或后台 projection。若后续确实需要，它必须复用
Knowledge Substrate 的派生层边界：

- SQLite `labels` / `label_semantics` / `label_atoms` / `label_ontology_*` 仍是事实来源；
  其中 `label_atoms` 是 materialized projection，不是独立 semantic truth。
- Graph projection 只能从 SQLite 快照和 outbox 重建，可删除重建。
- Graph API 只能查询 relation/provenance，不提供 canonical ontology mutation path。
- Graph 故障、dirty 或删除不会改变 task labels、semantics、atoms、signals 或 actions。

---

## 3. 数据流

### 3.1 创建任务

```text
CLI/Web
  -> CreateTask 命令
  -> 验证输入
  -> 计算初始状态
  -> 插入 tasks
  -> 插入 task_events(kind='task.created')
  -> 返回 task 快照
```

初始状态计算：

```text
如果规格不完整                  -> triage
否则如果 scheduled_at > 当前时间 -> scheduled
否则如果存在依赖                -> todo
否则                            -> ready 候选
新任务 execution plan=unplanned  -> 将 ready 候选降为 todo
```

添加第一个 step，或明确标记 `not_required` 后，服务才会重新计算是否进入 `ready`。

### 3.2 领取任务

```text
CLI/Web/Dispatcher
  -> ClaimTask 命令
  -> BEGIN IMMEDIATE
  -> 验证 task.status == ready
  -> 验证不存在未完成的父任务依赖
  -> 通过 CAS 把 tasks 更新为 running
  -> 插入 task_runs(status='running')
  -> 更新 tasks.current_run_id
  -> 插入 task_events(kind='task.claimed')
  -> COMMIT
```

### 3.3 完成任务

```text
Worker/CLI/Web
  -> CompleteTask 命令
  -> BEGIN IMMEDIATE
  -> 验证处于 running/review
  -> 如果处于 running：除非 force=true，否则验证 claim token
  -> 更新 task_runs
  -> 把 tasks 更新为 done 或 review
  -> 清除领取字段
  -> 插入 task_events(kind='task.completed')
  -> 子任务保持 todo；派生依赖状态反映它们是否仍被阻塞
  -> COMMIT
```

### 3.4 重新打开任务

```text
CLI/Web
  -> ReopenTask 命令
  -> BEGIN IMMEDIATE
  -> 验证 task.status == done
  -> 验证 reason 非空
  -> 根据规格、计划时间、依赖和执行计划重新计算目标状态
  -> 清除 completed_at，同时保留 result_summary/result_json
  -> 插入 task_events(kind='task.reopened')
  -> 重新计算直接的活跃子任务；running/blocked/review/done/archived 子任务保持不变
  -> COMMIT
```

### 3.5 Web 实时更新

```text
状态变更命令
  -> 插入 ID 单调递增的 task_events
  -> server SSE 循环轮询或订阅事件
  -> 浏览器接收事件
  -> 浏览器获取已变更 task 或应用补丁
```

---

## 4. 进程模型

### 4.1 无 server 模式

```bash
kanban task create "..."
kanban task list
```

CLI 直接打开 SQLite DB。

适用：脚本、本地开发、快速使用。

### 4.2 server 模式

```bash
kanban serve
```

启动：

- localhost HTTP server。
- SSE 事件流。

适用：为 Tauri Desktop 或本机脚本提供 API；该命令本身不提供浏览器看板。

---

## 5. 配置

默认配置文件：

```text
~/.config/kanban/config.toml
```

可解析的项目或全局配置示例：

```toml
board = "default"
db = "/path/to/kb.db"

[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "qwen3-embedding:0.6b"
dimensions = 1024
```

当前配置只接受顶层 `board`、`db` 和可选的 `[vector]`；未知字段会被拒绝。默认操作者
来自 `USER`、`USERNAME` 或回退值 `local`，不是配置字段。

CLI 还支持项目级当前 board 配置：

```text
<project>/.kb/config.toml
```

当前版本只写入一个顶层字段：

```toml
board = "agent-work"
```

当前 board 的解析顺序是 `--board`、`KB_BOARD`、向上查找最近的 `.kb/config.toml`，最后回退到 `default`。项目配置只选择同一个全局 SQLite 数据库内的 board，不表示每个项目使用一个数据库。

---

## 6. 并发

### 6.1 SQLite 写入策略

- 使用 WAL。
- 使用短事务。
- 对 claim/reclaim/complete 使用 `BEGIN IMMEDIATE`。
- 使用乐观锁：`lock_version`。
- 并发 claim 同一 task 时，只有一个 `UPDATE ... WHERE status='ready' AND claim_token IS NULL` 成功。

### 6.2 不做的事情

- 不引入分布式锁。
- 不用网络文件系统共享数据库。
- 不允许多个机器同时写同一 SQLite 文件。

### 6.3 同机多进程

允许：

- 多个 CLI 命令。
- 一个 server。
- 一个 dispatcher。

SQLite WAL 和 busy timeout 负责排队。业务层仍需保证事务短小。

---

## 7. 错误模型

公开错误 wire 词汇由 `kanban-contract::ApiErrorCode` 作为唯一闭合集合 owner。
HTTP status 映射与 operation-level transport 说明仅在 `docs/API_SPEC.md` 的
“HTTP 状态映射”表中维护；架构文档不复制 code 表，避免与 server 适配器的实际
`KanbanError -> ApiErrorCode` 映射漂移。

`error.message` 仍是面向人的 locale 相关文案；状态机、service 保护、CAS、
事务与 SQLite 错误权威不转移给 wire contract。

---

## 8. 可观测性

本地工具仍需要基本可观测性：

- `task_events` 是第一审计来源。
- server 输出结构化日志。
- dispatcher 对每次 run 写入 `task_runs`。
- worker stdout/stderr 可写入本地日志文件，数据库只存路径和摘要。
- `kanban doctor` 检查数据库、WAL、schema、完整性、孤儿 run、基础关系表
  board 一致性、label ontology 账本一致性，并报告 Knowledge Substrate 的
  `index_outbox` 积压、派生存储 dirty/error 状态和各存储的 last_error。派生层
  异常不改变 SQLite task 事实；操作者通过同步/重建恢复 Tantivy/Oxigraph/LanceDB。

统一 Projection v2 maintenance runtime 在数据库级使用 singleton lease，但 lease
同时绑定当前进程实际编译的 store capability 集和运行制品 build identity。status
不会把“存在活动 owner”解释为“所有 store 都可维护”：当前构建缺少 backend 时报告
`unavailable`，活动 owner 未声明 store capability 时报告 `unverified`，两者都附带
稳定 fallback reason，并使 `doctor --strict-derived` fail closed。continuous runtime
必须声明全部 projection store capability 才能领取 singleton lease；feature-limited
制品在 claim 前拒绝，避免部分能力 owner 长期垄断数据库级维护入口。

一次 `run --once` 或 `rebuild --all` 中，store backend/provider/delivery 的局部失败
以闭合的结构化 store result 返回并记录到对应 projection state；runtime 仍按稳定顺序
尝试其余已编译 store。数据库访问、singleton owner、lease/fence 或 shutdown 失败属于
全局错误，会终止本次 pass。错误作用域由运行时的显式结果类型和调用边界决定，不解析
错误文案；任何派生失败都不回滚已经提交的 SQLite 权威 mutation。

### 8.1 Board scope 与 schema/service/doctor 分工

Board 是本地 project/board，不是租户。正常写路径的隔离边界在 service 层：
CLI、HTTP、desktop 和 dispatcher 通过 `kanban-sqlite::service` resolve board/task/label/run，
再在同一事务中写入权威 SQLite 事实。派生存储只消费 SQLite/outbox
投影，不拥有权威写入权限。

关键关系表已经使用包含 `board_id` 的 composite FK 或 trigger。`task_labels`、
`task_dependencies`、`task_runs`、`task_comments`、`task_attachments` 在 SQLite 层直接
保证 row board 与 referenced task/label/run board 一致；`task_events` 保留 nullable
task/run refs 与 `ON DELETE SET NULL` 历史语义，通过 INSERT/UPDATE triggers 校验非空
refs 的 board scope。Ontology action-signal 使用 board-scoped composite FK；nullable
ontology refs、parent/supersede links、proposal resolved label 等用 triggers 保护；historical
atom refs 保持 soft ref。

- service 保护是普通 CLI/API/Desktop/dispatcher 写入的主防线；
- `kanban doctor` 是现有数据库的只读巡检层，发现跨 board 关系记录或
  `PRAGMA foreign_key_check` 违规时让 `ok=false`；
- JSONL import 在替换事务提交前运行同类一致性/外键门禁，失败会回滚整个
  导入。

---

## 9. 安全边界

因为不做多用户/远程：

- 默认只绑定 `127.0.0.1`。
- 不提供登录系统。
- 不提供远程访问配置。
- 不在 API 中执行任意 shell，worker profile 只能来自本地配置。
- 附件路径必须限制在数据目录内，防止路径穿越。


### 传输描述符边界

`kanban-contract` 是 localhost 传输的 method/path 权威：其默认 feature 无运行时 HTTP 依赖，仍可被叶子 schema 工具离线使用。`kanban-server::router::registered_api_routes()` 仅提供显式 `adapter_id` 和真实 handler；path/method 从 contract 描述符读取。这样 CLI/JSONL 清单与 API/SSE 传输标识分层，server 不能自行复制传输字符串。

每个 API/SSE 语义 contract 还必须显式声明 HTTP location；其它公开面必须声明
`NoTransport`。任意 `Adopted` contract 与端点精确引用都必须保持
`granularity=Exact`。唯一 method/path、精确 `operation_key` 和单一 location 共同保证一个
`ExactSurface` contract 不可能合法绑定两个端点义务，因此不保留不可达的全局
第二绑定保护。`SharedComponent` 允许被多个端点显式链接，或由同一公开面的真实
采用 witness 证明不是孤儿；这两个条件取其一。共享 contract 永远不计入端点精确覆盖，
也不单独决定端点迁移状态。

两个 task-read 端点的 path、query、headers 与成功响应都由
端点专属精确 contract 覆盖，当前迁移状态为 `Adopted`。精确 wire 形状、
query 预算、producer/consumer 证据与实时覆盖状态以 `docs/API_SPEC.md`、
`docs/SCHEMA_CONTRACTS.md` 和生成的 schema artifact 为准；架构层只规定传输权威
和共享 service 路径，不复制阶段性冻结统计。


---

# 文件：docs/STATE_MACHINE.md

# 状态机规范

本文件定义权威任务状态、合法转换、守卫条件与副作用。

---

## 1. 状态枚举

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

建议 Rust 表示：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
    Running,
    Blocked,
    Review,
    Done,
    Archived,
}
```

---

## 2. 状态职责

| 状态 | 是否可编辑 | 是否可领取 | 是否默认展示 | 说明 |
|---|---:|---:|---:|---|
| `triage` | 是 | 否 | 是 | 待澄清。 |
| `todo` | 是 | 否 | 是 | 已定义，但依赖或执行计划尚未允许进入 `ready`。 |
| `scheduled` | 是 | 否 | 是 | 等时间到；时间到本身不会改变状态。 |
| `ready` | 是 | 是 | 是 | 已进入可执行队列。 |
| `running` | 部分 | 否 | 是 | 正在执行。 |
| `blocked` | 是 | 否 | 是 | 阻塞。 |
| `review` | 是 | 否 | 是 | 待检查。 |
| `done` | 部分 | 否 | 默认可隐藏 | 已完成。 |
| `archived` | 否 | 否 | 默认隐藏 | 归档。 |

---

## 3. 状态转换命令

### 3.1 创建（`create`）

```text
none -> triage | todo | scheduled | ready
```

候选初始状态按以下顺序计算：

1. 如果调用方显式提供了允许创建的 `input.status`，使用它作为候选状态。
2. 否则，如果必需规格不完整，候选状态为 `triage`。
3. 否则，如果 `scheduled_at > now`，候选状态为 `scheduled`。
4. 否则，如果存在尚未进入 `done` 或 `archived` 的父依赖，候选状态为 `todo`。
5. 否则，候选状态为 `ready`。

候选状态为 `ready` 时，创建服务仍会保存为 `todo`，因为新任务的执行计划从
`unplanned` 开始。

允许显式创建状态：

```text
triage | todo | scheduled | ready
```

这里的 `ready` 是允许请求并通过基础守卫的候选状态，不表示新任务会直接保存为
`ready`。新任务还没有执行计划，因此服务会把候选 `ready` 保存为
`todo`，计划状态为 `unplanned`。添加第一个 step 使计划成为 `planned`，或者通过
`kanban task step not-required` 填写原因后标记为 `not_required`；其它守卫也满足
时，重新计算才会进入 `ready`。

删除最后一个 step 会使派生计划回到 `unplanned`，并把可重新计算的任务退回
`todo`。当前删除操作只写入 `task.step.removed`，不会额外写入
`task.execution_plan.unplanned` 事件。

不允许直接创建：

```text
running | blocked | review | done | archived
```

副作用：

- 向 `tasks` 插入记录。
- 写入 `task_events(kind='task.created')`。

---

### 3.2 明确规格（`specify`）

```text
triage -> todo | scheduled | ready
```

守卫条件：

- `title` 非空。
- `description` / 规格满足本地校验。
- 如果 `scheduled_at > now`，目标必须是 `scheduled`。
- 如果父依赖未全部进入 `done` 或 `archived`，目标必须是 `todo`。
- 如果执行计划仍为 `unplanned`，目标必须是 `todo`。
- 否则可进入 `ready`。

副作用：

- 更新任务字段。
- 写入 `task_events(kind='task.specified')`。

---

### 3.3 提升为可执行（`promote`）

```text
todo -> ready
scheduled -> ready
```

守卫条件：

- 所有父依赖都是 `done` 或 `archived`。
- 执行计划不是 `unplanned`：必须有 step 形成 `planned`，或显式标记
  `not_required` 并填写原因。
- 任务未归档。
- 对 `scheduled`，必须 `scheduled_at <= now`。

副作用：

- 更新状态。
- 写入 `task_events(kind='task.promoted')`。

`promote` 表示显式进入 `ready` 的意图，通常由人工 CLI 或 Web 操作触发：

```bash
kanban task promote t_xxx
```

执行计划或规格变更也可能触发活动状态重算。例如添加第一个 step 或标记
`not_required` 后，满足其它条件的 `todo` 会通过 `task.recomputed` 进入 `ready`，
而不是写入 `task.promoted`。排期时间到达本身不会触发这种重算。

---

### 3.4 领取 / 开始（`claim` / `start`）

```text
ready -> running
```

守卫条件：

- `task.status == 'ready'`。
- `claim_token IS NULL`。
- 所有父依赖都是 `done` 或 `archived`。
- 执行计划不是 `unplanned`。
- 任务未归档。

同一事务内的原子副作用：

1. 以 CAS 更新 `tasks`：
   - `status = 'running'`
   - `claim_token = <new_token>`
   - `claim_owner = <actor>`
   - `claim_expires_at = now + ttl`
   - `last_heartbeat_at = now`
   - `started_at = COALESCE(started_at, now)`
   - `lock_version = lock_version + 1`
2. 插入 `task_runs(status='running')`。
3. 更新 `tasks.current_run_id`。
4. 写入 `task_events(kind='task.claimed')`。

失败：

- 如果受影响行数为 0，返回 `claim_conflict` 或 `dependency_blocked`。

---

### 3.5 心跳（`heartbeat`）

```text
running -> running
```

守卫条件：

- `task.status == 'running'`。
- 领取凭证匹配。
- 领取尚未被强制回收。

副作用：

- 延长 `claim_expires_at`。
- 更新任务的 `last_heartbeat_at`。
- 更新活动 run 的 `claim_expires_at` 与 `last_heartbeat_at`。
- 每次显式心跳都写入 `task_events(kind='task.heartbeat')`。

对于 `running` 任务，后续有效的任务级事件（例如评论、step 或 label 变更）也会作为
隐式存活信号：服务层刷新任务与活动 run 的领取期限和最后心跳时间，但不额外写入
`task.heartbeat` 事件。board 级事件或没有 `task_id` 的事件不会刷新领取期限。

---

### 3.6 完成（`complete`）

```text
running -> done
review -> done
```

守卫条件：

- `running -> done` 必须匹配领取凭证，除非 `force=true`。
- `review -> done` 不需要领取凭证。
- 如果存在必需 step，它们必须全部为 `done` 或 `skipped`；可选 step 不阻塞父任务完成。

副作用：

- 把任务状态更新为 `done`。
- 设置 `completed_at = now`。
- 清除领取字段。
- 把活动 run 状态更新为 `succeeded`。
- 写入 `task_events(kind='task.completed')`。
- 不自动改写或提升子任务；子任务保持原状态。此前为 `todo` 的仍是 `todo`，派生依赖
  状态会更新为不再受该父任务阻塞。

---

### 3.7 提交审核（`review`）

```text
running -> review
```

守卫条件：

- 领取凭证匹配，除非 `force=true`。

副作用：

- 把任务状态更新为 `review`。
- 清除领取字段。
- 把活动 run 状态更新为 `succeeded`。
- 写入 `task_events(kind='task.submitted_for_review')`。

---

### 3.8 阻塞（`block`）

```text
triage | todo | scheduled | ready | running | review -> blocked
```

守卫条件：

- `reason` 非空。
- 从 `running` 进入 `blocked` 时必须匹配领取凭证，除非 `force=true`。

副作用：

- 把状态更新为 `blocked`。
- 设置 `status_reason`。
- 如果原状态为 `running`，把活动 run 关闭为 `failed`，并记录退出码 `1`。
- 清除领取字段。
- 写入 `task_events(kind='task.blocked')`。

---

### 3.9 解除阻塞（`unblock`）

```text
blocked -> triage | todo | scheduled | ready
```

目标状态按以下顺序计算：

1. 规格不完整时进入 `triage`。
2. 否则，`scheduled_at > now` 时进入 `scheduled`。
3. 否则，父依赖未全部完成时进入 `todo`。
4. 否则，执行计划仍为 `unplanned` 时进入 `todo`。
5. 否则进入 `ready`。

副作用：

- 清除 `status_reason`。
- 更新为计算出的目标状态。
- 写入 `task_events(kind='task.unblocked')`。

---

### 3.10 回收领取（`reclaim`）

```text
running -> ready | todo | blocked
```

守卫条件：

- 批量自动回收只扫描 `running` 且 `claim_expires_at <= now` 的任务。
- 指定任务回收要求任务为 `running`，并且领取已过期或显式传入 `force=true`。
- 当前实现不检查工作进程 PID，也没有最长运行时间回收。

目标状态：

- 默认 `ready`。
- 如果回收后的 `retry_count` 达到 `max_retries`，则进入 `blocked`。
- 如果目标原本为 `ready`，但执行计划守卫不再满足，则降级为 `todo`。

副作用：

- 把活动 run 关闭为 `expired` 或 `canceled`。
- 清除领取字段。
- 增加 `retry_count`。
- 写入 `task_events(kind='task.reclaimed')`。

---

### 3.11 归档（`archive`）

```text
triage | todo | scheduled | ready | blocked | review | done -> archived
```

默认不允许直接归档 `running`，除非 `force=true`。非强制归档还要求所有必需 step
均已完成或跳过；`force=true` 才会绕过该守卫。

副作用：

- 设置 `archived_at = now`。
- 把状态更新为 `archived`。
- 强制归档时清除领取字段。
- 写入 `task_events(kind='task.archived')`。

---

#### 3.11.1 看板归档

看板归档属于 board 生命周期操作，不是任务状态转换。

规则：

- 设置 `boards.archived_at = now`。
- 写入 `task_events(kind='board.archived')`。
- 不改写该看板上的任务。
- 如果看板上存在 `running` 任务或 `running` run 记录，则拒绝归档。
- 归档后，拒绝针对该看板的普通任务、评论和内部实验性 dispatcher 写操作。
- 事件、run 记录与评论的只读历史查询仍然可用，以便审计。

---

### 3.12 重新打开（`reopen`）

```text
done -> triage | todo | scheduled | ready
```

守卫条件：

- 只允许重新打开 `done` 任务；`review`、`archived` 和其它状态必须拒绝。
- `reason` 必须非空。

目标状态由服务端重新计算，不由调用方指定：

1. 规格不完整时进入 `triage`。
2. 否则，`scheduled_at > now` 时进入 `scheduled`。
3. 否则，父依赖未全部进入 `done` 或 `archived` 时进入 `todo`。
4. 否则，执行计划尚不可执行时进入 `todo`。
5. 否则进入 `ready`。

副作用：

- 清除 `completed_at`。
- 保留 `result_summary` / `result_json`。
- 写入 `task_events(kind='task.reopened')`，payload 包含 `from`、`to`、`reason`、
  `original_completed_at`。
- 直接依赖该任务的子任务中，仅 `triage | todo | scheduled | ready` 会按可执行条件
  重新计算；`running | blocked | review | done | archived` 不会被隐式改写。

---

## 4. 当前已实现的显式转换矩阵

| 来源 \ 目标 | triage | todo | scheduled | ready | running | blocked | review | done | archived |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| none | create | create | create | 请求 `ready` 后保存为 `todo` | - | - | - | - | - |
| triage | - | specify | specify | specify | - | block | - | - | archive |
| todo | - | - | - | promote | - | block | - | - | archive |
| scheduled | - | - | - | promote | - | block | - | - | archive |
| ready | - | - | - | - | claim | block | - | - | archive |
| running | - | reclaim | - | reclaim | - | block / reclaim | submit_review | complete | force_archive |
| blocked | unblock | unblock | unblock | unblock | - | - | - | - | archive |
| review | - | - | - | - | - | block | - | complete | archive |
| done | reopen | reopen | reopen | reopen | - | - | - | - | archive |
| archived | - | - | - | - | - | - | - | - | - |

表中只列出当前已有的显式转换服务。由规格、依赖或执行计划变化触发的状态重算不作为
独立命令列入矩阵；它们通过 `task.recomputed` 进入计算出的活动状态。任务级 `reopen`
当前只实现从 `done` 进入服务端重新计算出的活动状态。

### 4.1 未实现候选

以下命令目前没有实现，不属于当前转换矩阵：

- `schedule`
- `unschedule`
- `demote`
- `restore`

---

## 5. 依赖规则

### 5.1 依赖语义

```text
parent_task_id -> child_task_id
```

表示子任务被父任务阻塞。只有父任务为 `done` 或 `archived` 时，子任务才能进入
`ready` 或 `running`。归档父任务会满足强依赖守卫，但不会删除依赖边，也不会自动
提升子任务。

### 5.2 规则

1. `parent_task_id != child_task_id`。
2. 新增依赖不能产生环。
3. 如果给一个 `ready` 子任务增加未完成父任务（不是 `done` 或 `archived`），子任务必须
   降级为 `todo`。
4. 父任务从 `done` 被重新打开时，仅直接子任务中的可重新计算活动状态
   （`triage | todo | scheduled | ready`）会按可执行条件重新计算；
   `blocked | review | running | done | archived` 不会被隐式改写。
5. 不允许给 `running` 子任务新增未完成依赖；当前接口没有强制例外。必须先通过阻塞或
   回收让子任务退出 `running`，再新增依赖。

---

## 6. UI 列映射

UI 列不是状态真相，只是展示配置。

默认列：

| 默认显示名 | 状态 |
|---|---|
| Triage | `triage` |
| Todo | `todo` |
| Scheduled | `scheduled` |
| Ready | `ready` |
| Running | `running` |
| Blocked | `blocked` |
| Review | `review` |
| Done | `done` |

`archived` 默认隐藏。

拖拽行为：

- 从 `ready` 拖到 `running`：调用 `claim` / `start`。
- 从 `running` 拖到 `done`：调用 `complete`，需要活动领取或显式强制。
- 从任意非终态拖到 `blocked`：弹窗要求填写原因，然后调用 `block`。
- 从 `blocked` 拖到其他列：调用 `unblock`，不直接设置目标状态。
- 拖到 `archived`：调用 `archive`。

---

## 7. 测试要求

必须覆盖：

1. 状态转换矩阵单元测试。
2. 依赖环检测。
3. `ready -> running` 并发领取只有一个成功。
4. 过期领取回收。
5. `block` / `unblock` 重新计算目标状态。
6. 完成父任务后不自动改写子任务状态，并更新派生的依赖阻塞状态。
7. 内部实验性 dispatcher 不处理已归档任务。
8. `unplanned` 任务不能 `promote` 或 `claim`，内部实验性 dispatcher 也不能领取。
9. 必需 step 未完成时，父任务不能 `complete`。
10. 非法直接转换返回 `invalid_transition`。


---

# 文件：docs/DATA_MODEL.md

# 数据模型

本文件定义领域模型、SQLite 表、ID、时间、JSON、附件、事件与常用查询。

---

## 1. ID 规范

除预置看板列这类固定 ID 外，公开实体 ID 通常使用带前缀的 ULID（类 UUID 字符串），
便于在日志和 CLI 中区分。

| 对象 | 前缀 | 示例 |
|---|---|---|
| 看板（Board） | `b_` | `b_01HY...` |
| 任务（Task） | `t_` | `t_01HY...` |
| 步骤（Step） | `step_` | `step_01HY...` |
| 执行记录（Run） | `r_` | `r_01HY...` |
| 评论（Comment） | `c_` | `c_01HY...` |
| 附件（Attachment） | `a_` | `a_01HY...` |
| 标签（Label） | `l_` | `l_01HY...` |
| 看板列（Column） | `col_` | `col_ready` |
| 事件（Event） | `e_` | `e_01HY...` |

`task_events.event_id` 保存带 `e_` 前缀的公开事件 ID；`task_events.id` 是单独的自增整数，
用于 SSE 偏移量和顺序分页。

领取凭证不是实体 ID。`tasks.claim_token` 与 `task_runs.claim_token` 使用
`claim_...` 格式，例如 `claim_01HY...`；调用方必须把它视为临时凭证，不应当作可公开枚举的
稳定身份。

---

## 2. 时间规范

所有时间字段使用：

```text
INTEGER，UTC Unix 时间戳（毫秒）
```

字段命名：

- `created_at`
- `updated_at`
- `scheduled_at`
- `started_at`
- `completed_at`
- `archived_at`
- `claim_expires_at`
- `last_heartbeat_at`

Rust 内部建议使用 `time::OffsetDateTime`，在数据库边界转换为以毫秒表示的 `i64`。

---

## 3. JSON 字段规范

SQLite 中 JSON 存 `TEXT`，必须满足：

```sql
CHECK(json_valid(field_name))
```

默认值：

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{}
```

用途：

| 字段 | 说明 |
|---|---|
| `tasks.metadata_json` | 轻量扩展信息。 |
| `task_runs.metadata_json` | worker 配置、环境、命令摘要等。 |
| `task_events.payload_json` | 事件载荷。 |

禁止把大对象、完整 stdout/stderr 日志或附件二进制内容放进 JSON。

---

## 4. 看板（Board）

看板（Board）表示本地项目或看板，不是租户。

主要字段：

| 字段 | 说明 |
|---|---|
| `id` | 带 `b_` 前缀的 ID。 |
| `slug` | CLI 和 Web 使用的人类可读短名。 |
| `name` | 展示名。 |
| `description` | 可选说明。 |
| `created_at` | 创建时间。 |
| `updated_at` | 更新时间。 |
| `archived_at` | 归档时间。 |

默认看板：

```text
default
```

Board slug 由服务层校验：必须唯一、非空、不超过 64 字节，以小写 ASCII 字母或数字开头，只能包含小写 ASCII 字母、数字、`.`、`_`、`-`，并且不能使用 `b_`、`t_`、`r_`、`c_`、`a_`、`l_`、`col_`、`e_` 等保留前缀。这样可以避免与公开 ID、`board#seq` 任务引用和路径式别名语法冲突。

已归档的看板默认不出现在看板列表中，也不接受普通任务、评论或实验性 dispatcher 写入。归档只设置看板的 `archived_at` 并写入 `board.archived` 事件，不改变任务状态；如果看板上仍有 `running` 任务或 `running` 执行记录，归档会被拒绝。事件、执行记录、评论等只读历史仍可通过明确的任务或看板身份查询，用于审计。

### 4.1 看板隔离的责任边界

SQLite 是权威事实来源，但看板隔离由数据库结构、服务和诊断门禁共同保证：

1. 数据库约束：所有看板作用域内的行都有 `board_id` 并引用 `boards(id)`；
   被引用的任务、标签和执行记录 ID 也各自有外键，确保引用对象存在。`task_labels`、
   `task_dependencies`、`task_execution_plans`、`task_runs`、`task_comments`、
   `task_attachments` 和较新的标签语义、atom 与本体链接表使用包含 `board_id`
   的复合外键，直接阻止这些关系表出现跨看板行。`task_steps` 的父任务也由复合外键
   约束；可选的 `linked_task_id` 只有普通任务外键，其同看板约束由服务守卫与诊断或
   导入门禁负责。`task_events` 保留可空的 task/run 引用和
   `ON DELETE SET NULL` 语义，由 INSERT/UPDATE 触发器校验非空引用的看板作用域。
2. 服务守卫：CLI、HTTP、桌面端和实验性 dispatcher 的正常写路径必须先在同一看板
   作用域内解析 task、label、run 等对象，再写入关系行；例如任务标签绑定、
   依赖、评论、事件、run 和附件都不应跨看板组合。
3. Doctor 和导入检查：`kanban doctor` 与 JSONL 导入的最终门禁会只读检查基础关系表
   中 `row.board_id` 与被引用任务、标签、执行记录所属看板是否一致，并运行
   `PRAGMA foreign_key_check`。任何违规都会成为严重错误；导入会在
   提交前回滚整个替换事务。

---

## 5. 任务（Task）

任务（Task）是核心对象，既是看板卡片，也是可执行工作单元。

### 5.1 字段分组

#### 身份

| 字段 | 说明 |
|---|---|
| `id` | 任务 ID。 |
| `board_id` | 所属看板。 |
| `seq` | 看板内递增数字，便于显示 `board#12`。 |

任务的公开身份分为两层：

- `id` 是全局唯一的 `t_...`，可跨看板直接定位任务。
- `seq` 只在同一看板内唯一，CLI/API 展示时应组合成 `board_slug#seq`，例如 `agent-work#12`。

#### 内容

| 字段 | 说明 |
|---|---|
| `title` | 必填。 |
| `description` | Markdown 文本。 |
| `status_reason` | 阻塞等状态原因。 |
| `result_summary` | 完成摘要。 |
| `result_json` | 完成结果的自然 JSON；存储值必须是合法 JSON，CLI/API 公开为解码后的 `result`。 |
| `metadata_json` | 扩展字段。 |

#### 工作流

| 字段 | 说明 |
|---|---|
| `status` | 权威状态。 |
| `priority` | 类枚举的整数优先级：`0` = 最高的 P0，`1` = P1，`2` = P2，`3` = 最低且默认的 P3。数据库默认值是 `3`，迁移后由 `CHECK(priority BETWEEN 0 AND 3)` 约束。创建和更新命令会拒绝 P0—P3 之外的值。 |
| `position` | UI 排序键。 |
| `scheduled_at` | 计划时间。 |
| `due_at` | 截止时间，仅展示/过滤，不驱动状态机。 |
| `retry_count` | 已重试次数。 |
| `max_retries` | 最大重试次数。 |

#### 操作者与执行

| 字段 | 说明 |
|---|---|
| `assignee` | 人或 worker 配置名称。 |
| `created_by` | 操作者字符串。 |
| `claim_token` | 当前领取凭证，格式为 `claim_...`。 |
| `claim_owner` | 当前领取者。 |
| `claim_expires_at` | 领取过期时间。 |
| `last_heartbeat_at` | 最近心跳时间。 |
| `current_run_id` | 当前或最近的 run ID。 |

#### 时间戳

| 字段 | 说明 |
|---|---|
| `created_at` | 创建。 |
| `updated_at` | 更新。 |
| `started_at` | 首次进入 running。 |
| `completed_at` | 完成。 |
| `archived_at` | 归档。 |

#### 并发

| 字段 | 说明 |
|---|---|
| `lock_version` | 乐观锁版本。 |

### 5.2 优先级语义

`priority` 表示任务的相对重要性和排序权重，不表示状态机可执行性。`ready`
表示任务已经由人工或服务明确放入可领取队列；P0—P3 只影响列表排序，以及内部实验性 dispatcher 在候选任务之间的排序。

优先级约定：

| 优先级 | 语义 | 示例 |
|---|---|---|
| `0` / P0 | 事故、阻断当前目标或必须立即处理的任务。应当少量使用，不作为普通 `ready` 任务的默认值。 | 修复导致本地队列无法领取任务的回归；解除发布前的 P1/P0 审查阻塞。 |
| `1` / P1 | 近期工作焦点，当前迭代或当前工作流应优先完成。 | 今天要完成的实现切片；当前 PR 必须补齐的测试。 |
| `2` / P2 | 重要的后续任务，但不阻塞当前主线。 | 整理文档示例；补充非关键冒烟测试。 |
| `3` / P3 | 普通待办、低优先级或默认值。 | 想法、低风险清理、未来可做的体验改进。 |

`ready` 与 P0 不能互相替代：

- 普通可执行任务应是 `ready` + P1/P2/P3，而不是为了进入队列全部标成 P0。
- P0 任务如果仍缺规格、排期未到或依赖未完成，仍不能被领取；它应保持
  `triage`、`scheduled` 或 `todo`，直到满足状态机守卫后再提升到 `ready`。
- 内部实验性 dispatcher 只领取 `ready` 任务；只有在多个 `ready` 任务之间，才按
  P0 到 P3 排序。这不是当前公开支持的使用路径。

---

## 6. 依赖（Dependency）

表：`task_dependencies`

数据库结构不变量：`parent_task_id` 和 `child_task_id` 必须都属于该行的
`board_id`。旧数据库升级到复合外键结构前会先检查已有的跨看板行；
发现不一致时迁移会失败，并要求先用 doctor/repair 清理。

字段：

| 字段 | 说明 |
|---|---|
| `parent_task_id` | 前置任务。 |
| `child_task_id` | 被阻塞任务。 |
| `board_id` | 两个任务共同所属的看板。 |
| `created_at` | 创建时间。 |

语义：

```text
前置任务为 done 或 archived => 后续任务可以变为 ready
前置任务既不是 done 也不是 archived => 后续任务不能进入 ready/running
```

添加依赖时必须做环检测。归档前置任务会满足强依赖守卫，但依赖边会作为历史保留，也不会自动提升后续任务。

前置任务从 `done` 重新打开后，直接后续任务中只有 `triage|todo|scheduled|ready` 会按就绪条件重新计算；`running|blocked|review|done|archived` 不会被隐式改写。


---

## 7. 步骤与执行计划（Step / Execution Plan）

步骤（Step）是父任务内部的有序执行步骤，不是阻塞依赖关系。Step 可以是普通文本，
也可以链接到另一个普通任务作为上下文。链接任务不会自动创建
`task_dependencies` 边，也不会根据所链接任务的状态自动完成 step；step 自己有独立的
`todo | done | skipped` 状态。

### 7.1 步骤

表：`task_steps`

数据库结构通过复合外键保证 `parent_task_id` 属于该行的 `board_id`。可选的
`linked_task_id` 只有指向 `tasks(id)` 的普通外键；服务与诊断或导入门禁必须另外保证
它属于同一看板，且不能等于 `parent_task_id`。服务还必须拒绝已归档的父任务、
已归档的链接任务、空白标题和跨看板链接。

字段：

| 字段 | 说明 |
|---|---|
| `id` | Step ID，格式为 `step_...`。 |
| `board_id` | 所属看板。 |
| `parent_task_id` | 被规划的父任务。 |
| `position` | 父任务内步骤排序键。 |
| `title` | 步骤标题。 |
| `body` | 可选说明文本。 |
| `linked_task_id` | 可选的上下文任务。 |
| `required` | 是否阻止父任务完成或归档。 |
| `status` | `todo`、`done` 或 `skipped`。 |
| `resolution_note` | 完成、跳过或重新打开的说明。 |
| `resolved_by` | 最近一次处理的操作者。 |
| `resolved_at` | 最近一次处理时间。 |
| `created_by` | 创建者。 |
| `created_at` | 创建时间。 |
| `updated_by` | 最近更新者。 |
| `updated_at` | 最近更新时间。 |

索引：

- `idx_steps_parent_position(parent_task_id, position)`
- `idx_steps_linked_task(linked_task_id)`
- `idx_steps_board_status(board_id, status)`

语义：

```text
父任务包含有序步骤
可选的 linked_task_id 只提供任务上下文
```

Step 不会直接驱动 `dependency_blocked` 或 `unfinished_parent_count`。必需步骤
只参与执行计划守卫：父任务不能完成或归档，直到所有必需步骤
都是 `done` 或 `skipped`。

### 7.2 执行计划

表：`task_execution_plans`

字段：

| 字段 | 说明 |
|---|---|
| `board_id` | 所属看板。 |
| `task_id` | 被规划的任务。 |
| `state` | `unplanned`、`planned` 或 `not_required`。 |
| `reason` | `not_required` 的说明。 |
| `updated_by` | 最近更新者。 |
| `updated_at` | 最近更新时间。 |

索引：

- `idx_execution_plans_board_state(board_id, state)`

派生口径：

```text
步骤数量 > 0 => planned
存在明确的 not_required 行且没有步骤 => not_required
其他情况 => unplanned
```

事件：

```text
task.step.created
task.step.updated
task.step.removed
task.step.done
task.step.skipped
task.step.reopened
task.execution_plan.planned
task.execution_plan.not_required
```

## 8. 执行记录（Run）

表：`task_runs`

数据库结构不变量：`task_id` 必须属于该行的 `board_id`。这保证一次 run 尝试
不能在 SQLite 层跨看板指向任务。

执行记录（Run）表示一次执行尝试。

### 8.1 Run 状态

```text
running | succeeded | failed | canceled | expired
```

### 8.2 字段

| 字段 | 说明 |
|---|---|
| `id` | 带 `r_` 前缀的 ID。 |
| `board_id` | 所属看板。 |
| `task_id` | 关联任务。 |
| `status` | run 状态。 |
| `worker_profile` | worker 配置名称。 |
| `worker_pid` | 可选、预留的本机 PID；当前内部实验性 dispatcher 不填充此字段。 |
| `claim_token` | 对应的领取凭证，格式为 `claim_...`。 |
| `claim_owner` | 本次领取的操作者。 |
| `claim_expires_at` | 本次领取的过期时间。 |
| `started_at` | run 开始。 |
| `last_heartbeat_at` | 最近心跳时间。 |
| `finished_at` | run 结束。 |
| `exit_code` | worker 退出码。 |
| `summary` | 简短摘要。 |
| `error` | 错误文本。 |
| `log_path` | stdout/stderr 日志路径。 |
| `metadata_json` | 执行元数据。 |

### 8.3 约束

- 当前为 `running` 的任务必须有当前 run。
- 一个任务可以有多个历史 run。
- 同一任务同时最多有一个 `running` run。

最后一条不由 SQLite 直接强制，需要服务层和事务共同保证。

---

## 9. 事件（Event）

表：`task_events`

事件（Event）是只追加的事实记录。

### 9.1 事件类型

API/SSE 当前已类型化的 39 个已知类型：

```text
board.created
board.archived
dependency.added
dependency.removed
label.created
label.deleted
signal.recorded
signal.reviewed
task.archived
task.blocked
task.claimed
task.comment.created
task.completed
task.created
task.execution_plan.not_required
task.execution_plan.planned
task.execution_plan.unplanned
task.heartbeat
task.label.added
task.label.removed
task.label_proposal.accepted
task.label_proposal.proposed
task.label_proposal.rejected
task.promoted
task.reclaimed
task.recomputed
task.reopened
task.retry_policy.updated
task.specified
task.step.created
task.step.done
task.step.removed
task.step.reopened
task.step.skipped
task.step.updated
task.submitted_for_review
task.unblocked
task.updated
task.export_sanitized
```

### 9.2 载荷示例

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{
  "claim_owner": "alice",
  "metadata": {}
}
```

`task_events.kind/payload_json` 的 SQLite 存储允许未来出现未知类型。事件 API 与 SSE
对上面 39 个已知类型使用精确的同级载荷契约，已知类型不匹配时按失败关闭处理；未知
类型的合法 JSON 载荷保持无损。外层 `task_id`、`run_id`、`actor` 都是
必需但可空的字段。可移植 JSONL 的事件载荷仍是不透明 JSON，不复用这组类型化联合。

### 9.3 使用场景

- 任务详情时间线。
- SSE 事件流。
- 调试领取与执行记录。
- CLI `kanban events`。
- 导出与导入。

---

## 10. 评论（Comment）

表：`task_comments`

字段：

| 字段 | 说明 |
|---|---|
| `id` | 评论 ID。 |
| `task_id` | 关联任务。 |
| `board_id` | 关联看板。 |
| `author` | 操作者字符串。 |
| `author_type` | `user` / `agent`，表示评论作者身份；本地操作者是 `user`，其他自动化来源是 `agent`。 |
| `agent_type` | 可选的开放文本，仅用于 `author_type=agent`，例如 `executor` / `reviewer`。 |
| `body` | Markdown 文本。 |
| `kind` | `note` / `decision` / `signal`，表示评论内容语义，不表示作者身份。`signal` 是信号账本的反向链接。 |
| `metadata_json` | `kind` 对应的结构化载荷；默认 `{}`，必须是合法 JSON 对象。`kind=decision` 时必须符合决策结构。`kind=signal` 的反向链接元数据包含 `type:"signal_link"`、`signal_id`、`observation_id`、`signal_kind`、`signal_status`。 |
| `created_at` | 创建时间。 |

`(task_id, board_id)` 通过复合外键关联 `tasks(id, board_id)`，因此评论不能跨看板挂接任务。

旧评论行或 JSONL 导入记录会迁移到新语义：旧 `human` 变为 `user`，旧 `agent/system` 或 `worker/system` 来源变为 `agent`，旧 `text/system/worker` 内容变为 `note`。没有结构化元数据的旧 `decision` 也按 `note` 保留正文作为回退。

创建评论时也会写入一条 `task_events(kind='task.comment.created')`。

`metadata_json` 是 SQLite 的权威存储列；CLI/API 响应会把它解码成自然、无损的
`metadata` 对象。普通 `note`/`signal` 元数据保持开放。只有服务生成的反向链接
完整结构由 `SignalLinkMetadataOutput` 独立证明，不能把用户自定义的同名键碰撞当成协议。

决策评论的元数据结构：

- `options`：非空数组。
- 每个选项都是对象，且包含非空字符串 `slug`、`title`、`detail`。
- `slug` 必须是稳定的小写 ASCII 短名：以小写字母或数字开头，只包含小写字母、数字和 `-`；在同一决策内唯一。
- `selected`：非空字符串，必须匹配某个选项的 `slug`。
- `reason`：非空字符串。
- `risk` / `verification`：可选；如果出现，必须是非空字符串。
- 未知顶层字段允许保留，但不参与状态机、内部实验性 dispatcher 或事件语义。

---

## 11. 附件（Attachment）

二进制内容不存入数据库。

附件默认保存在数据库目录下：

```text
<db_dir>/attachments/<board_id>/<task_id>/<attachment_id>/<filename>
```

例如，在使用常见 Linux 默认数据库目录时，路径通常为
`~/.local/share/kb/attachments/<board_id>/<task_id>/<attachment_id>/<filename>`。

数据库记录：

| 字段 | 说明 |
|---|---|
| `id` | 附件 ID。 |
| `task_id` | 关联任务。 |
| `board_id` | 关联看板。 |
| `filename` | 原始文件名。 |
| `rel_path` | 相对数据目录的路径。 |
| `content_type` | MIME 类型。 |
| `size_bytes` | 大小。 |
| `sha256` | 内容哈希。 |
| `created_by` | 操作者。 |
| `created_at` | 上传时间。 |

`(task_id, board_id)` 通过复合外键关联 `tasks(id, board_id)`，因此附件不能跨看板挂接任务。

安全要求：

- `filename` 必须经过安全清理。
- `rel_path` 必须位于数据目录内。
- 不允许通过 `../` 进行路径穿越。

---

## 12. 标签（Label）

标签（Label）用于轻量分类。

字段：

| 字段 | 说明 |
|---|---|
| `id` | 标签 ID。 |
| `board_id` | 所属看板。 |
| `name` | 标签名。 |
| `color` | UI 颜色标记。 |
| `created_at` | 创建时间。 |
| `updated_at` | 更新时间。 |

同一看板内标签名唯一。

任务与标签的关联通过 `task_labels(task_id, label_id, board_id, created_at)` 关联表表达。
两条复合外键分别约束任务和标签都属于 `board_id` 指定的看板，不能跨看板绑定。
标签只用于分类、过滤和展示；添加或移除标签不改变 `tasks.status`，
不触发依赖重新计算，也不会让内部实验性 dispatcher 领取 `review` 或其他非
`ready` 状态。

### 12.1 标签语义

`labels` 仍是权威的标签身份表：名称、颜色和看板作用域由 `labels`
定义。`task_labels` 仍是任务最终绑定标签的事实。语义推荐和向量检索使用
额外的事实表，不替代这两张表。
`labels` 的身份增删改查属于基础词表登记，不写入本体变更账本；
`label delete` 不会隐式删除语义或 atom，必须先通过受 CAS 保护的语义清理流程
清空语义。

表：`label_semantics`

| 字段 | 说明 |
|---|---|
| `label_id` | 关联 `labels(id)`，一个标签最多有一条语义记录。 |
| `board_id` | 冗余的看板作用域，用复合外键保证标签与看板一致。 |
| `description` | 标签的自然语言说明。 |
| `applies_when` | JSON 字符串数组，正向适用条件。 |
| `excludes_when` | JSON 字符串数组，反向排除条件。 |
| `positive_examples` | JSON 字符串数组，正向示例。 |
| `negative_examples` | JSON 字符串数组，反向示例。 |
| `created_at` / `updated_at` | 语义记录时间。 |

表：`label_atoms`

`label_atoms` 是从 `label_semantics` 与标签名展开的 SQLite 物化投影。
它保存 `positive` 与 `negative` 两种极性，供后续 Group OMP/NNLS 标签求解器和
LanceDB atom 检索使用；它随语义变更在同一事务内重建，不是独立于
`label_semantics` 的第二份语义事实。

| 字段 | 说明 |
|---|---|
| `id` | 稳定的 `la_...` atom ID。 |
| `label_id` / `board_id` | 关联权威标签与看板。 |
| `polarity` | 极性：`positive` / `negative`。 |
| `kind` | `name`、`description`、`applies_when`、`positive_example`、`excludes_when`、`negative_example`；有说明时，`description` atom 是 `label: {name}\ndescription: {description}` 形式的权威 atom，没有说明时才使用 `name` 回退 atom。 |
| `text` | 去除首尾空白并规范化空白后的 atom 文本；每个非空行内部的空白会折叠，权威行分隔保留，空文本不入库。 |
| `ordinal` | 同一标签展开后的顺序；同语义的重复 atom 去重时保留首次出现的 `ordinal`。 |
| `content_hash` | atom 语义内容哈希，用于派生层判断变化；输入为 `label_id + polarity + kind + normalized_text`，不包含 `ordinal`。 |
| `created_at` / `updated_at` | 投影行时间。 |

派生向量表：`kb_label_atoms`

`kb_label_atoms` 是 LanceDB 中可重建的标签 atom 向量表，独立于任务分块表
`kb_chunks`。它按 `board_id`、`embedding_model`、`polarity` 查询 atom 证据，
返回 `label_id`、atom ID、`polarity`、`kind`、`text` 和 LanceDB 原始
`_distance` 等字段。语义标签候选会使用返回的 atom 向量，在本地重新计算
查询向量与残差的余弦相似度，不把距离当作求解器分数。派生表损坏或缺少
提供方时，只会让标签 atom 索引降级，不影响普通标签增删改查、`task_labels` 绑定
或任务状态机。

### 12.2 通用信号账本

通用信号账本保存 agent 或产品在 kanban 工作流中发现的通用问题，
例如 CLI 参数使用不顺、提示误导、参数设计不符合 agent 惯用方式，或操作者发现的
产品反馈。它是看板作用域内的审计账本和只读收件箱数据源，不替代 `tasks.status`、
任务评论、run、事件或标签本体账本。

- `signal_observations` 保存一次观察的来源、操作者、task/run/comment 关联和原始证据。
- `signals` 保存一个可以独立审查的通用信号，并指向对应 observation。
- 通用信号与 `label_ontology_signals` 分离；本体信号仍只服务于标签
  语义、atom、提案审查和变更来源追踪。
- 当前公开 HTTP 接口只读取通用信号；生命周期写操作仍由 CLI/runtime
  的信号记录流程负责。
- 看板作用域内的列表和审查接口只通过 board 路由读取：
  `/api/v1/boards/{board}/signals*`。单条详情
  `GET /api/v1/signals/{signal_id}` 是面向操作者的全局详情查询，用于从
  反向链接或收件箱行直接打开已知信号；它不改变信号的 `board_id`
  事实，也不会把信号混入其他看板的列表。
- `signal_observations.task_id`、`run_id`、`comment_id` 是用于来源和历史的
  软引用。当前一致性由服务写入路径、doctor 和导入最终门禁维护；
  这些引用允许保留历史来源语义。未来如需把全部来源关系硬化，可迁移为
  带看板作用域的复合外键。

表：`signal_observations`

一行表示一次 agent 或操作者的观察。Observation 可关联 task、run 或 comment；
这些关联用于定位来源，不改变对应实体状态。

| 字段 | 说明 |
|---|---|
| `id` | `obs_...` 观察记录 ID。 |
| `board_id` | 来源看板作用域。 |
| `task_id` / `task_ref_snapshot` | 可空。来源任务与捕获时的人类可读引用快照；任务后续改动不影响快照。 |
| `run_id` | 可空。来源执行 run。 |
| `comment_id` | 可空。来源评论。 |
| `actor` / `agent_type` | 捕获者名称与可选的 agent 类型。 |
| `source` | 可空。信号来源，例如 `codex-hook`、`cli` 或 `operator`。 |
| `evidence_json` | JSON 对象字符串，保存命令、stderr、上下文片段、hook 提示等原始证据。 |
| `created_at` | 创建时间。 |

表：`signals`

一行表示一个可以独立进入操作者收件箱的通用信号。它只描述发现的问题和审查
生命周期，不直接触发修复，也不修改权威工作流。

| 字段 | 说明 |
|---|---|
| `id` | `sig_...` 信号 ID。 |
| `board_id` / `observation_id` | 看板作用域与来源 observation。 |
| `kind` | 通用信号类型，例如 `agent_cli_friction`。 |
| `title` / `summary` | 面向操作者的短标题与摘要。 |
| `severity` | 文本严重度，例如 `info`、`medium` 或 `high`。 |
| `status` | `open`、`confirmed`、`resolved`、`rejected`、`superseded`。 |
| `dedupe_key` | 可空。用于调用方聚合相似信号。 |
| `superseded_by_signal_id` | 可空。指向同一看板中的替代信号。 |
| `reviewed_by` / `reviewed_at` / `review_reason` | 生命周期审查记录。 |
| `created_at` / `updated_at` | 创建与更新时间。 |

默认审查队列只读取 `open` 与 `confirmed` 信号；完整历史需要明确设置
`include_all` 或指定状态。

### 12.3 标签本体账本

标签本体账本记录任务标注过程中的证据、分歧信号、审查与操作历史
以及验证结果。它是可查询的审计账本，不替代权威事实：

- `labels` / `task_labels` 仍决定任务当前实际绑定哪些标签。
- `label_semantics` 决定标签的权威语义；`label_atoms` 是它的 SQLite
  物化 atom 投影。
- `label_semantic_proposals` 仍负责新标签提案的生命周期。
- 本体账本覆盖语义和 atom 变更；基础 `labels` 身份的增删改查位于
  账本之外，只写普通事件。

这些表在标签系统中承担不同角色，不是六个严格独立的存储层。`label suggest` 是计算结果，
`kb_label_atoms` 是可重建的检索投影，提案和账本是需要持久审计的 SQLite 记录；
它们都不能直接替代 `task_labels` 的当前绑定事实。

表：`label_ontology_observations`

一行表示一次完整的任务标签判断过程。它保存当时的任务快照、agent 候选、
`label suggest` 快照、最终选择和由快照派生的求解器指标；即使任务、标签或
atom 后续变化，仍能还原当时为什么产生信号。Observation 是只读的来源记录：
写入记录不会修改 `task_labels`、`label_semantics`、`label_atoms`、标签 atom 索引或
提案。

| 字段 | 说明 |
|---|---|
| `id` | `lor_...` 观察记录 ID。 |
| `board_id` / `task_id` | 来源看板与任务。 |
| `task_ref_snapshot` | 捕获时的人类 ref，例如 `default#42`。 |
| `task_snapshot_json` | 捕获时的任务标题、说明、标签、版本和哈希等快照。 |
| `suggest_input_hash` | 可空。按标签建议输入（规范化标题 + 说明）计算的窄哈希，用于验证可比性；旧 observation 缺失时按旧版不可比较处理，不能静默标记为通过。 |
| `agent_candidates_json` | agent 原始候选标签、置信度和理由。 |
| `suggestion_snapshot_json` | 完整的建议输出、参数、模型和索引状态快照；新的捕获路径要保存未经改写的原始快照。 |
| `final_decision_json` | 对最终接受、拒绝和未采用标签的判断。 |
| `suggest_coverage` / `suggest_coverage_cosine` / `suggest_residual_norm` | 可查询的求解器指标。新的捕获路径从 `suggestion_snapshot_json` 派生这些值；调用方不应重复手写。`suggest_coverage = clamp(1 - suggest_residual_norm, 0.0, 1.0)`，二者不是独立证据；`suggest_coverage_cosine` 是查询向量与拟合向量的余弦相似度，可作为补充指标。 |
| `suggest_needs_new_label` / `suggest_degraded` | 捕获时的建议状态。新的捕获路径从 `suggestion_snapshot_json` 派生这些值。`suggest_needs_new_label` 是覆盖审查的兼容字段，不等于自动发现词表缺口；判断是否需要新标签还要结合原因代码、证据、诊断和人工语义判断。 |
| `diagnostics_json` | 建议诊断数组。新的捕获路径从快照的 `diagnostics` 派生；冲突的重复输入会被拒绝。 |
| `capture_fingerprint` | 同一看板内的幂等指纹。 |
| `created_by` / `created_by_type` / `agent_type` | 捕获者身份。 |
| `created_at` | 创建时间。 |

表：`label_ontology_signals`

一行只表达一个可独立审查的本体问题，例如某个已有标签漏选、
建议误选、存在词表缺口或标签边界、名称问题。

| 字段 | 说明 |
|---|---|
| `id` | `los_...` 信号 ID。 |
| `observation_id` / `board_id` | 来源 observation 与看板作用域。 |
| `kind` | `false_negative`、`false_positive`、`vocabulary_gap`、`name_issue`、`boundary_issue`、`structure_issue`。 |
| `status` | `open`、`confirmed`、`resolved`、`rejected`、`superseded`。 |
| `target_label_id` / `target_label_name_snapshot` | 已有标签目标；名称快照用于历史解释。 |
| `related_labels_json` | 拆分、合并等多标签关系快照。 |
| `proposed_action` | `observe`、`add_positive_atom`、`add_negative_atom`、`update_semantics`、`bootstrap_label`、`rename_label`、`split_label`、`merge_labels`。 |
| `candidate_atom_polarity` / `candidate_atom_kind` / `candidate_text` | 建议 atom 的极性、类型和泛化文本。 |
| `candidate_content_hash` | 按 `label_id + polarity + kind + normalized_text` 计算的聚合键。 |
| `proposed_label_name` / `proposed_label_name_normalized` | 词表缺口或重命名候选。 |
| `proposal_json` | 新标签或结构变更的候选语义快照。 |
| `agent_selected` / `suggest_state` / `suggest_score` / `suggest_rank` / `final_selected` | agent、建议与最终判断之间的分歧证据。 |
| `rationale` / `confidence` | 可审查理由和可选置信度。 |
| `signal_key` | observation 内的幂等键。 |
| `superseded_by_signal_id` / `status_reason` | 关闭或替代原因。 |
| `created_at` / `updated_at` / `reviewed_at` / `closed_at` | 生命周期时间。 |

`label ontology review`（标签本体审查）是基于信号的只读聚合投影，不是新的权威事实，也不是
新的可持久化派生存储。分组键来自调用方选择的维度：`label` 使用目标标签，
`proposed-label` 使用规范化后的候选标签名，`candidate-atom` 优先使用
`candidate_content_hash`。没有候选
atom 的信号不会进入一个全局空值分组；回退键会带上信号类型、目标
标签或候选标签，以及候选操作，例如
`no-candidate-atom|kind:vocabulary_gap|proposed:ontology ledger|action:bootstrap_label`。
因此一个分组的含义是“这些信号共享同一个审查键”，不是“这些信号已被证明
来自同一个根因”。

`cluster` 是需要明确启用的重复信号审查辅助功能；它默认关闭，不写入权威 atom，不会自动
确认、应用、验证或变更，也不成为 SQLite 事实。每次审查查询时，聚类键都会从
已有信号文本和审查范围重建：键始终包含信号类型、候选操作、目标
标签快照（或 ID 回退）以及候选标签范围，再依次附加词法规范化后的候选文本、
候选标签、理由，最后回退到纯范围组合。这个范围前缀避免把文本相同但标签边界或操作
不同的信号强制合并；输出中的 `cluster_key` 和 `cluster_reason` 只用于解释辅助分组来源。

审查队列默认使用不同来源任务数（`task_count`）作为主要热度指标，
再按已确认数量、最近信号时间和键排序。`signal_count` 只是分组内的原始
信号行数；同一任务可以贡献多条信号，所以它不能单独代表模型错误率、准确率、
召回率或标签建议质量。需要质量指标时必须另有分母，例如一致性队列
或固定评估集。

`label ontology quality` 是一个只读分析投影，不新增表，也不写权威事实。
它把 `label_ontology_observations` 作为分母来源，并在输出中记录来源、不同
任务数、observation 数、一致或降级的 observation 数、时间范围和任务引用样本；
同时把 `label_ontology_signals` 作为原始分歧分子来源，按类型和状态
给出原始信号数量。只有当分母中存在一致的 observations 时，才会给出
`disagreement_task_rate`；只有信号的数据集会明确返回比率不可用，避免把分歧
记录误称为错误率。准确率和召回率仍需要带预期标签的独立评估队列，当前
账本信号不能单独提供这些指标。

长期标签本体回归语料集属于测试和评估基础设施，不是新的 SQLite 事实。
当前固定语料集测试使用临时数据库和内存标签 atom 索引，跟踪重要标签的已知
正向和负向对照任务，并比较 `label suggest` 选中的标签、分数与证据 atom。
语料集运行本身应只读权威本体；只有测试中明确模拟的临时语义或 atom 变更，
才用于证明比较能够发现回归。真实数据库上的长期语料集需要等稳定任务集积累后再扩展，
不应替代账本信号、可信验证或人工审查。

当前没有标签本体专属的图投影。`label_ontology_*` 表本身就是 SQLite
来源事实；`kanban graph` / Oxigraph 只投影知识派生底座的
`entity_relations`，不保存也不拥有标签本体操作或信号事实。若未来出现明确的
重命名、拆分、合并或来源关系查询需求，新增投影必须从
`labels`、`label_semantics`、`label_atoms`、`label_semantic_proposals` 和
`label_ontology_*` 重建，并通过 `index_outbox` / `derived_store_state` 表达标脏、同步、
重建和错误状态；删除或损坏图存储不得改变权威的标签、本体或账本行。

表：`label_ontology_actions`

Action 是只追加的历史，表示审查者或 agent 实际确认、拒绝、修改本体或
记录验证的操作。直接修改标签语义或接受提案时，来源信息
也写成操作记录。

| 字段 | 说明 |
|---|---|
| `id` | `loa_...` 操作 ID。 |
| `board_id` | 看板作用域。 |
| `parent_action_id` | 验证等后续操作指向被验证的变更操作。 |
| `action_type` | `confirm`、`reject`、`supersede`、`resolve_no_change`、`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、`update_semantics`、`create_label_proposal`、`bootstrap_label`、`rename_label`、`split_label`、`merge_labels`、`validate`、`revert_ontology_mutation`。 |
| `reason` | 必填的人工或 agent 理由。 |
| `target_label_id` / `result_label_id` | 修改目标与结果标签。 |
| `result_atom_id` / `result_atom_content_hash` | 新增或采用 atom 的软引用和稳定哈希。 |
| `result_proposal_id` | 关联的 `label_semantic_proposals`。 |
| `canonical_before_hash` / `canonical_after_hash` | 修改前后权威语义的哈希。 |
| `change_json` | 修改前、修改后、差异或其他可解释的变更快照。 |
| `validation_requirement` | `none`、`required`、`unsupported`。表示父级变更是否需要类型化验证策略；不改写历史尝试结果。 |
| `validation_status` | `not_required`、`pending`、`passed`、`failed`、`partial`。对父级变更表示历史兼容或基础状态；对 `validate` 操作表示一次尝试结果。 |
| `validation_json` | 验证证据封装；服务会包装调用方提供或工具采集的载荷、来源信号用例、任务快照可比性、父级操作结果引用和摘要。公开的提供或采集载荷只保存在顶层 `manual`；生成的 `cases[]` 用 `after.manual_case_ref` 指向 `manual.cases[]` 中对应信号的证据，避免把同一载荷复制到每个用例。`failed` / `partial` 可保存外部或人工证明的诊断。`passed` 操作只能来自工具采集的 `trusted_automated` 证据（采集器来源、嵌入模型、求解器选项、干净的 atom 索引状态与代次、每个信号修改前后的用例），并按父级操作校验正向 atom、负向 atom、标签引导创建，以及负向正例对照或豁免策略；调用方手写 JSON 或自称 `automated` 不构成可信来源。 |
| `created_by` / `created_by_type` / `agent_type` | 操作者身份。 |
| `created_at` | 创建时间。 |

`validation_effective_outcome` 是读取 DTO 时归并计算的结果，不是独立存储列。它按
`validation_requirement` 和最近的验证子操作（`created_at,id`）计算：
`not_required`、`unsupported`、`pending`、`passed`、`failed` 或 `partial`。只有
`required + trusted passed` 会处理已链接的来源信号；`unsupported` 可以记录
外部的失败或部分成功诊断，但拒绝 `passed`。

`label_ontology_action_atom_effects` 连接一条根变更操作与本次实际新增或删除的
atom 快照。它保存 `board_id`、`action_id`、`label_id_snapshot`、`atom_id_snapshot`、
`atom_content_hash`、`polarity`、`kind`、`text`、`effect` 和 `created_at`；`effect` 只允许
`added` / `removed`，唯一约束为 `(action_id, atom_content_hash, effect)`。操作记录使用
带看板作用域的复合外键；atom 快照不使用实时外键，因为 `label_atoms` 会随投影
重建。

`result_atom_id` 有意不使用强外键。`label_atoms` 会随语义重建而删除再插入；
历史操作和影响记录依赖 `result_atom_content_hash`、影响行与 `change_json` 中的 atom
快照保持可解释。Atom 解释查询会优先使用
`label_ontology_action_atom_effects`，也允许用旧版 `result_atom_id` /
`result_atom_content_hash` 兼容旧数据。`adopt_existing_atom` 表示新的来源信号采用了当前已存在的 atom，
不代表权威内容新增。已有 atom 如果来自旧语义写入而没有任何本体操作引用，
查询结果只标记 `legacy_untracked=true`，不会伪造来源记录。

同一 `(board_id, result_proposal_id)` 只能有一条 `create_label_proposal` 操作；接受提案
生成的 `bootstrap_label` 操作通过 `parent_action_id` 指向这条创建
操作，从而让“创建提案 → 引导接受”的来源链路保持无歧义。

`revert_ontology_mutation` 是只追加的回滚历史：它不会修改或删除原变更
操作，而是用 `parent_action_id` 指向被撤销操作，并把权威语义恢复到该
操作的 `change_json.before` / `canonical_before_hash` 快照。当前实现只覆盖
标签作用域内的语义或 atom 变更（`add_positive_atom`、`add_negative_atom`、
`update_semantics`），成功后标脏标签 atom 索引并保持验证待定；引导创建产生的
标签身份或任务绑定回滚不由该操作类型表达。

当前建设性本体变更路径的责任边界如下：

- `label_semantics` 是权威本体事实；`label_atoms` 是它的 SQLite 物化
  投影；`label_ontology_actions` 是只追加的来源记录，不是第二份事实。
- `update_semantics`、`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
  `create_label_proposal` 和 `bootstrap_label` 操作只能由专用服务路径写入。
  `adopt_existing_atom` 是只记录来源的路径，修改前后哈希相同，只把新的
  来源信号连接到已有 atom，不修改权威语义或 atom，也不标脏 atom
  索引；其他建设性变更与对应的权威写入位于同一 SQLite 事务。
- 每个语义或 atom 变更事务只写一条根变更操作；`change_json`
  只保存一次修改前后的语义快照。实际新增或删除的 atom 写入
  `label_ontology_action_atom_effects`；仅修改说明的补丁写入零条影响记录，无实际变化的补丁不写
  操作、影响记录，也不标脏索引。
- 人工变更可以没有来源信号，但仍必须记录操作者、理由、修改前后
  哈希和变更快照。信号驱动的变更会额外写入
  `label_ontology_action_signals` 链接。
- `label semantics upsert` 默认使用补丁与 CAS 路径：`expected_semantics_hash` 防止
  更新丢失；缺省字段不清空旧语义；只有 `replace=true` 才执行完整替换，并将缺省
  数组解释为空集合。
- 直接从任务标签引导创建与接受提案共用同一采用原语。任务标签
  引导创建可以新建或复用没有语义的同名权威标签；接受提案当前会先拒绝
  任何已有的规范化名称冲突，因此成功路径会创建新的权威标签。二者都会写入
  语义和 atom、标脏标签 atom 索引，并写一个 `bootstrap_label` 根操作和新增的
  atom 影响记录；接受提案
  不写 `task_labels`，任务标签引导创建会绑定来源任务。失败时权威写入与
  来源操作一起回滚。
- `rename_label`、`split_label`、`merge_labels` 仍可作为信号的 `proposed_action` 或旧版
  操作读取；当前公开服务、CLI 和 HTTP 不再写新的结构规划变更操作。旧
  结构规划操作的验证要求解释为 `unsupported`。
- `legacy_untracked=true` 只表示当前 atom 没有可匹配的本体操作，例如旧数据或
  破坏性清理后的历史缺口；新的建设性变更不应依赖这种兼容路径来解释
  来源。

表：`label_ontology_action_signals`

多对多连接操作与信号。多个信号可以支持一次 atom 修改；同一个信号
也可以先被确认，随后关联变更操作和验证操作。

默认审查队列只读取 `open` 与 `confirmed` 信号；完整历史需要明确包含全部状态。
变更操作写入后通常保持来源信号为 `confirmed`。只有可信自动化
`passed` 验证会把已链接的来源信号转为 `resolved`；外部或人工
证明、`failed` 或 `partial` 验证只追加历史，不删除信号，也不把问题
伪装成已验证关闭。

### 12.4 标签语义提案

表：`label_semantic_proposals`

`label_semantic_proposals` 保存新增标签提案的生命周期，不是权威的
标签事实。它只记录“现有标签 atom 建议覆盖不足时，外部或人工提供方
给出的候选语义”。明确接受之前，不会创建 `labels`、`label_semantics`、
`label_atoms` 或 `task_labels`。

| 字段 | 说明 |
|---|---|
| `id` | `lp_...` 提案 ID。 |
| `board_id` / `task_id` | 提案来源任务。 |
| `status` | `proposed` / `accepted` / `rejected`。提供方不可用不会写成状态，而是返回降级尝试。 |
| `name` / `description` / `applies_when` / `excludes_when` / `positive_examples` / `negative_examples` | 候选标签语义。数组字段为 JSON 字符串数组。 |
| `heuristic_coverage` / `heuristic_coverage_cosine` / `heuristic_residual_norm` | 来自当前残差标签建议求解器的覆盖与残差元数据，用于记录提案创建时现有标签 atom 的覆盖程度；`heuristic_coverage = clamp(1 - heuristic_residual_norm, 0.0, 1.0)`，二者不是独立证据；`heuristic_coverage_cosine` 是查询向量与拟合向量的余弦相似度。 |
| `top1_existing_label_id` / `top1_existing_label_name` | 当前启发式排序第一的已有标签。 |
| `diagnostics_json` | JSON 字符串数组，包含降级、冲突或验证诊断。 |
| `decision_reason` / `resolved_label_id` / `decided_at` | 接受或拒绝的决策信息；接受后 `resolved_label_id` 指向新建的权威标签。 |

只有 `proposed` 状态的提案可以被接受。接受操作通过共享的采用原语创建同一看板内的
权威 `labels` 行，并写入对应的 `label_semantics` / `label_atoms`，同时标脏
`lancedb_label_atoms` 派生存储，写入 `bootstrap_label` 来源操作，并让
`resolved_label_id` 指向结果标签；提案状态、权威写入与来源操作
在同一事务内提交。它不写入 `task_labels`，不会把新标签自动绑定到来源
任务。

拒绝操作会把提案标记为 `rejected`。与现有标签发生规范化名称冲突的
候选会持久化为 `rejected`，诊断信息包含 `near_duplicate_label_conflict`。
规范化名称冲突是一种确定性的近似重复启发式判断，会忽略大小写、空白和标点。

---

## 13. 看板列（Column）

看板列（Column）属于 UI 展示层。

字段：

| 字段 | 说明 |
|---|---|
| `id` | 看板列 ID。 |
| `board_id` | 所属看板。 |
| `status` | 映射的权威状态。 |
| `title` | UI 名称。 |
| `position` | UI 排序。 |
| `hidden` | 是否隐藏。 |
| `wip_limit` | 可选的在制任务数量限制。 |

当前最小实现中，一个状态对应一个看板列。

---

## 14. 知识派生底座（Knowledge Substrate）

知识派生底座相关表只支持实体身份、关系镜像、派生发件箱和派生存储健康状态。SQLite 中的任务、执行记录、评论和事件仍是运行时权威事实来源。

### 14.1 实体登记

表：`entities`

字段：

| 字段 | 说明 |
|---|---|
| `uri` | 稳定的 `kb://...` 实体 URI。 |
| `kind` | 开放文本；当前自动投影使用 `board`、`column`、`task`、`run`、`event`、`comment`、`attachment`、`label`、`task_label`、`setting`。 |
| `source_table` | 来源 SQLite 表。 |
| `source_id` | 来源行 ID。 |
| `board_id` | 可选的看板作用域。 |
| `task_id` | 可选的任务作用域。 |
| `title` | 展示标题。 |
| `summary` | 简短摘要。 |
| `content_hash` | 内容哈希，用于派生层判断变化。 |
| `created_at` / `updated_at` / `archived_at` | 生命周期时间。 |

### 14.2 关系图镜像

表：`relation_predicates`、`entity_relations`

`relation_predicates` 定义受控谓词；`entity_relations` 保存可重建的关系镜像。关系层用于图与上下文查询，不改变任务状态机。状态机仍以 `tasks.status`、`task_dependencies` 和服务事务为准。

### 14.3 索引发件箱

表：`index_outbox`

字段：

| 字段 | 说明 |
|---|---|
| `id` | 自增作业 ID。 |
| `source_event_id` | 来源 `task_events.id`，允许事件被删除/导入时置空。 |
| `target` | `tantivy` / `oxigraph` / `lancedb` / `all`。 |
| `projection_store` | 可空的精确 store selector；当前只允许 `target=lancedb` 时使用 `lancedb_label_atoms`。`NULL` 保持旧路由语义。 |
| `entity_uri` | 目标实体。 |
| `action` | `upsert` / `delete` / `rebuild`。 |
| `payload_json` | 有界的作业载荷。 |
| `status` | `pending` / `running` / `done` / `failed`。 |
| `attempts` | 尝试次数。 |
| `last_error` | 最近失败原因。 |
| `created_at` / `updated_at` | 作业时间。 |

`index_outbox` 是至少执行一次语义的派生作业接口。任务变更事务只写 SQLite 权威事实、事件、实体和发件箱记录，不直接写 Tantivy、Oxigraph 或 LanceDB。

### 14.4 派生存储状态

表：`derived_store_state`

字段：

| 字段 | 说明 |
|---|---|
| `store_name` | 派生存储名称，例如 `tantivy_tasks`、`oxigraph_relations`、`lancedb_chunks`、`lancedb_label_atoms`。 |
| `schema_version` | 存储结构或契约版本。 |
| `last_event_id` | 存储已成功提交的全局 `task_events.id` 水位。 |
| `dirty` | 是否仍有未完成的发件箱记录、失败的发件箱记录，或最近一次存储更新失败。 |
| `last_rebuild_at` | 最近成功重建时间。 |
| `last_sync_at` | 最近成功同步时间。 |
| `last_error` | 最近失败证据。 |
| `updated_at` | 状态更新时间。 |

`last_event_id` 是存储全局成功处理水位，不是单个看板的局部水位。成功同步或重建只能单调推进这个值；当一个看板同步完成、但其他看板仍有 `pending`、`running` 或 `failed` 发件箱记录时，`dirty` 必须保持 `true`。`dirty=false` 只表示同一存储目标当前没有未完成的发件箱记录，而且最近一次存储更新没有失败。

`last_error` 在成功后清空，失败时保留错误证据并保持 `dirty=true`。操作者应通过 `kanban derived status`、`kanban doctor`、维护 API 和对应的同步或重建命令恢复派生层；派生存储损坏或落后不会改变 SQLite 中的任务事实。

### 13.5 Projection v2 consistency domain

表：`projection_database`、`projection_store_state`、`projection_deliveries`、
`projection_maintenance_owner`

`projection_database` 为一份 SQLite 文件保存稳定 `database_instance_id` 和 projection
protocol version。`projection_store_state` 为每个物理 store 保存 schema version、
legacy/v2 control-plane owner、连续 checkpoint、active/previous/building generation、
provider/model fingerprint、canonical 与 delivery 双 coverage digest、单调 fence epoch、
lease 和 lifecycle/error 状态。
`projection_deliveries` 把一个 `index_outbox` row 展开为各 store 的 board-scoped delivery；
唯一键是 `(outbox_id, store_name)`，每个 delivery 同时携带不可空 `board_id`、连续
store cursor、claim token、lease token、fence epoch 和 target generation。
`projection_maintenance_owner` 是 singleton database runtime lease，保存 owner、opaque
token、mode、expiry、heartbeat、已编译的 store capability 集和可追溯 build identity；
public status 不返回 token。Migration 028 会使无法证明 capability/build identity 的旧
lease 失效。lease 获取、续约与释放都比较 owner + token + canonical capability JSON +
build identity，过期、被篡改或来自另一构建的 owner 无法续约，也无法清除后继 owner。
`maintenance status` 会把当前二进制未编译的 backend 标为 `unavailable`；若活动 owner
没有声明某个 store capability，则该 store 标为 `unverified` 并提供 fallback reason，
不能因为 singleton owner 仍活跃就推断所有 store 都在被维护。

Migration 026 在 fanout/backfill 前验证每个相关 outbox row 能从 source event 或 entity
得到唯一 board；无法解析、orphan source event 或 event/entity board 冲突时 fail closed，
不会以 nullable/global delivery 绕过隔离。旧 `index_outbox.status=done` 只映射为
`legacy_done`，不伪造 v2 checkpoint 或 generation coverage。

Migration 029 以 additive `index_outbox.projection_store` selector 把
`lancedb_label_atoms` 接入同一 delivery domain，且不重建 `index_outbox` 或
`projection_deliveries`、不改变旧 ID/cursor。旧的 `target=lancedb|all` 且 selector 为
`NULL` 的 row 仍只路由到 `lancedb_chunks`；精确
`target=lancedb, projection_store=lancedb_label_atoms` 只路由到标签 atom store。
selector 与 target 在插入后不可改变；exact selector row 还被 SQLite 约束为
`source_event_id=NULL`、`kb://board/{board_id}` 实体、`action=rebuild` 和精确 payload
`{"scope":"board","version":1}`。所有 legacy LanceDB chunks 的 pending/complete/fail、
dirty 与 doctor count 查询都显式要求 `projection_store IS NULL`，不能凭共享
`target=lancedb` 吞入 label atom work。标签规范 mutation 在同一 canonical SQLite
事务里把 `label_atom_index_boards` 标脏，并由 trigger 原子写入 board-scoped
`rebuild` delivery。事务回滚时 dirty 标记、outbox 和 delivery 一并回滚；已有
pending/failed board rebuild 会合并，running rebuild 期间的新 mutation或 provider
failure 会留下新的 pending delivery，provider failure 即使没有旧 delivery 也必须生成
可恢复 work。迁移时已有的 dirty board 会逐板 backfill，不清空错误、旧 outbox 或
watermark。

Projection v2 的 snapshot 流程先固定 cursor，并按 store 从 canonical SQLite 读取完整、
稳定排序且强制携带 board scope 的 corpus：task search/chunk 投影包含 task 及其 comments、
runs、events；graph 投影包含 relation；label atom 投影包含 atom。每条 record 具有稳定
identity、payload 与 content hash；manifest 同时保存 canonical corpus 和 cursor 内
delivery 集合的 count + stable digest，并绑定 provider/model fingerprint。Provider 必须
实际消费 records，返回的 artifact evidence 必须匹配
database/protocol/schema/provider/generation/fence/cursor/两组 coverage；提交 snapshot
acknowledgement 的 transaction 会再次读取 canonical corpus 和 delivery coverage，任一
变化或存在 running claim 都拒绝批量完成。增量 batch 的 receipt 还必须精确匹配 lease、
fence、provider、generation、claim token 和 item count。

`lancedb_label_atoms` 的 canonical mutation 已通过 Migration 029 进入
`projection_deliveries`；`label_atom_index_boards` 在迁移期继续提供 per-board
dirty/error 兼容状态。generation 仍必须在 runtime/backend 的 provider fingerprint、
coverage、lease/fence 和物理 generation publish 门禁全部成立后才能发布，不能因为
delivery seam 已存在就绕过这些证据。

只有物理 store 完成 generation pointer CAS、active read-back 匹配，并证明上一物理
generation（若存在）仍可按 generation id 读取，SQLite 才原子发布 active/previous
metadata。若进程在物理 pointer swap 后退出，新 fence owner 可检查同一 generation 的
artifact evidence 并 reconcile SQLite publish。
若 logical active 的物理 artifact 已不可读，正常 publish CAS 仍 fail closed。只有
maintenance 的显式 recovery 路径可在新 snapshot/catch-up、当前 database/provider
binding 与 fenced lease 均成立时发布替代 generation；SQLite previous metadata 改为
实际可读且被物理 backend 保留的 generation，而不是伪造已丢失 artifact 的保留证据。

`derived_store_state` 和 `index_outbox` 在迁移期保留为 v1 compatibility projection。
generation begin 即把 store 切到 v2 control plane；legacy 与 v2 writer 在完整物理写周期
共享 per-database/per-store barrier，database replace 同时取得所有 store barrier，因此
旧 Tantivy/Oxigraph/LanceDB writer、v2 pointer swap 和 replace 不会交错。v2 reducer
只在 delivery 获得真实 generation coverage 后更新 legacy dirty/outbox 摘要，避免双控制面
永久 dirty 或虚假 clean。

表：`label_atom_index_boards`

`label_atom_index_boards` 只跟踪可重建的 `lancedb_label_atoms` 派生层在各看板
上的刷新状态，不是标签事实。`label_semantics` / `label_atoms` 更新会把对应
看板标脏；单个看板的标签 atom 重建成功，只会清理该看板的 `dirty` 标记。
只有该存储下所有看板都不再标脏时，`derived_store_state.dirty` 才能变为
`false`。

## 15. 常用查询

### 15.1 看板任务列表

```sql
SELECT *
FROM tasks
WHERE board_id = ?
  AND status != 'archived'
ORDER BY
  CASE status
    WHEN 'triage' THEN 10
    WHEN 'todo' THEN 20
    WHEN 'scheduled' THEN 30
    WHEN 'ready' THEN 40
    WHEN 'running' THEN 50
    WHEN 'blocked' THEN 60
    WHEN 'review' THEN 70
    WHEN 'done' THEN 80
    ELSE 90
  END,
  position ASC,
  priority ASC,
  created_at ASC;
```

### 15.2 就绪队列

```sql
SELECT *
FROM tasks t
WHERE t.board_id = ?
  AND t.status = 'ready'
  AND t.claim_token IS NULL
  AND NOT EXISTS (
    SELECT 1
    FROM task_dependencies d
    JOIN tasks p ON p.id = d.parent_task_id
    WHERE d.child_task_id = t.id
      AND p.status NOT IN ('done','archived')
  )
ORDER BY t.priority ASC, t.created_at ASC
LIMIT ?;
```

### 15.3 已过期的领取

```sql
SELECT *
FROM tasks
WHERE status = 'running'
  AND claim_expires_at IS NOT NULL
  AND claim_expires_at <= ?;
```

### 15.4 事件流

```sql
SELECT *
FROM task_events
WHERE board_id = ?
  AND id > ?
ORDER BY id ASC
LIMIT ?;
```

---

## 16. 导出与导入格式

JSONL 导出与导入使用可移植的看板快照格式：

```bash
kanban export --board default --format jsonl --out board.jsonl
kanban import --input board.jsonl --replace
```

每行：

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{"type":"task","data":{...}}
{"type":"event","data":{...}}
{"type":"comment","data":{...}}
{"type":"dependency","data":{...}}
```

通用信号账本使用稳定的记录类型：

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{"type":"signal_observation","data":{...}}
{"type":"signal","data":{...}}
```

标签本体账本使用稳定的记录类型：

<!-- schema-doc-ignore: 仅作示意或只展示部分载荷；已提交的结构 fixture 才是可执行的权威依据 -->
```json
{"type":"label_ontology_observation","data":{...}}
{"type":"label_ontology_signal","data":{...}}
{"type":"label_ontology_action","data":{...}}
{"type":"label_ontology_action_atom_effect","data":{...}}
{"type":"label_ontology_action_signal","data":{...}}
```

可移植描述符的权威定义共覆盖 21 个辨别字段值；输入和输出各有精确根结构，共
42 个 Draft 2020-12 结构定义。每行 `data` 都是闭合对象，必需但可空的键必须存在，
但可以明确为 `null`。真实的导出生产者与导入消费者使用同一描述符和 fixture 登记表。
SQLite 中的 `evidence_json`、`related_labels_json`、`proposal_json`、`change_json`、
`validation_json` 等仍是权威存储列；公开适配器只暴露去掉 `_json` 后的自然 JSON。

导入另有一条只向前兼容的迁移，用于读取采用自然 JSON 契约之前、
由上一版导出器生成的数据库原生 JSONL 快照。该格式通过 `column.hidden=0|1`
以及 `metadata_json` / `payload_json` 等真实 SQLite 列形状识别；同一快照必须保持
单一格式，不能混用数据库原生记录与自然 JSON 记录。同一记录只要同时出现自然 JSON
重命名键和数据库原生重命名键，就会在规范化前被拒绝，不能让旧版
值静默覆盖自然 JSON 值。导入器只会把结构一致的上一版本记录中的 JSON 文本列和整数
布尔值转换为当前自然 JSON 记录，再执行同一精确契约验证，以及下述事务和最终
一致性门禁。当前及后续导出始终只写自然 JSON，不再产生数据库原生键；
这不是长期双轨公开契约。

导入时会在同一事务中先插入各行，再运行最终一致性门禁。基础关系表
会检查 `task_labels`、`task_dependencies`、`task_runs`、`task_comments`、
`signal_observations`、`signals`、`task_events`、`task_attachments` 的行所属看板与
被引用任务、标签、执行记录、评论和观察记录所属看板是否一致；失败时整个
`--replace` 导入事务回滚，不提交部分数据。

本体相关行也在同一事务中插入，并延迟回填
`label_ontology_signals.superseded_by_signal_id` 与
`label_ontology_actions.parent_action_id`，避免依赖同表自引用行在文件中的顺序。导入完成前会校验本体账本的看板隔离：观察记录与信号所属看板、操作
父级所属看板、操作与信号链接所属看板、标签或提案软引用所属看板必须一致；
孤立的操作与信号链接、替代关系环和操作父级环会导致导入
失败。

通用信号的 `signals.superseded_by_signal_id` 同样会延迟回填，避免依赖同表自引用行在文件中的顺序。

`kanban doctor --json` 对上述基础关系表、SQLite `PRAGMA foreign_key_check`、本体
账本一致性和通用信号账本的看板一致性规则做只读巡检。
基础关系表问题返回 `consistency_errors`、`consistency_warnings`、
`consistency_issues[]`；本体账本问题返回 `ontology_ledger_errors`、
`ontology_ledger_warnings`、`ontology_ledger_issues[]`。问题项包含 `severity`、
`code`、`message`、`record_ids`，用于定位损坏行；基础关系表消息包含
`table`、`row`、`row_board` 和 `referenced_board`，外键问题会记录表、
rowid、父表和外键索引。严重错误包括行所属看板不匹配、
缺失 v12 本体表、跨看板链接、孤立的操作与信号或操作与影响链接、通用
信号孤立或跨看板上下文、通用信号替代关系环、父级或替代关系异常、标签、提案或任务所属看板不匹配、
替代关系环和操作父级环；错误数非零会让 `ok=false`。警告保留给仍可解释或可重建的软引用，例如历史操作的
`result_atom_id` 已被当前 `label_atoms` 重建删除。


---

# 文件：docs/CLI_SPEC.md

# CLI 规范

默认二进制名称：`kanban`

CLI 是一等入口；它与 Tauri Desktop 和本机 API 共用由 `kanban-sqlite::service` 支撑的服务路径
和 SQLite 模式。

---

## 1. 全局选项

```bash
kanban [GLOBAL_OPTIONS] <COMMAND>
```

| 选项 | 说明 |
|---|---|
| `--db <path>` | 指定 SQLite 数据库；优先级高于环境变量、配置文件和 XDG 默认路径。 |
| `--board <slug-or-id>` | 显式指定当前看板，优先级最高。 |
| `--actor <name>` | 操作者名称，默认为操作系统用户名。 |
| `--locale <auto|system|zh-CN|en>` | 选择区域设置；省略、`auto` 或 `system` 时使用系统区域设置。当前只覆盖部分错误提示和依赖命令文案。 |
| `--json` | JSON 输出。 |

SQLite 数据库路径的解析顺序：

1. `--db <path>`。
2. `KANBAN_DB` 环境变量。
3. `KB_DB` 环境变量（兼容短名）。
4. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `db = "<path>"`。
5. 用户全局配置 `$XDG_CONFIG_HOME/kanban/config.toml`，读取 `db = "<path>"`。
6. 回退到 XDG 数据默认路径，通常是 `~/.local/share/kb/kb.db`。

当前看板的解析顺序：

1. `--board <slug-or-id>`。
2. `KB_BOARD` 环境变量。
3. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `board = "<slug>"`。
4. 回退到 `default`。

`kanban board use <board>` 会把当前目录写成项目级 `.kb/config.toml`；后续子目录自动继承该当前看板。该配置只选择本地项目的看板，不创建新数据库。
如果同一配置文件也包含 `db = "<path>"` 或 `[vector]`，`board use` 必须保留这些字段。配置中的相对数据库路径按配置文件所在目录解析；环境变量和 `--db` 中的相对路径按当前工作目录解析。

区域设置不改变 JSON 键、状态枚举、任务引用、ID、退出码或机器可读诊断信息。当前本地化
只覆盖部分运行时错误提示，以及依赖新增和移除的少量人类可读输出；初始化、任务、步骤、
配置等多数人类输出仍为英文，因此不能把该选项理解为完整界面翻译。选择顺序：

1. `--locale <auto|system|zh-CN|en>`。
2. `KANBAN_LOCALE`。
3. 系统区域设置。

`auto` / `system` 会按 `LC_ALL`、`LC_MESSAGES`、`LANG` 解析系统区域设置；当前只支持中文和英文。脚本和自动化应优先使用 `--json`，不要依赖人类可读文案。

### 1.1 查看配置

```bash
kanban config show [--json]
```

`config show` 输出当前 CLI 会使用的 SQLite 数据库路径、当前看板和区域设置，以及每个值的来源。该命令用于智能体或操作人员排查优先级，不会打开、初始化或创建 SQLite 数据库。

`--json` 输出使用常规 `{ "data": ... }` 封装，`data` 结构如下：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "db": {
    "value": "/path/to/kb.db",
    "source": { "kind": "project_config", "path": "/repo/.kb/config.toml", "key": "db" }
  },
  "board": {
    "value": "kanban-tool",
    "source": { "kind": "env", "name": "KB_BOARD" }
  },
  "locale": {
    "value": "zh-CN",
    "input": "auto",
    "source": { "kind": "flag", "name": "--locale" }
  }
}
```

`source.kind` 是脚本可依赖的 ASCII 枚举：

| `source.kind` | 含义 |
|---|---|
| `flag` | 来自显式 CLI 标志，例如 `--db`、`--board`、`--locale`。 |
| `env` | 来自环境变量，例如 `KANBAN_DB`、`KB_DB`、`KB_BOARD`、`KANBAN_LOCALE`。 |
| `project_config` | 来自最近的项目级 `.kb/config.toml`。 |
| `global_config` | 来自 `$XDG_CONFIG_HOME/kanban/config.toml`。当前只适用于数据库路径。 |
| `default` | 来自 CLI 默认值或回退值。 |

`locale.value` 是实际解析后的区域设置；当输入为 `auto` / `system` 时，`input` 保留原始选择，`value` 保留系统区域设置的解析结果。`db.value` 对显式标志和环境变量保留调用方传入的路径形态；配置中的相对数据库路径按配置文件所在目录解析。

### 1.2 帮助输出契约

`kanban --help` 和公开命令组的 `--help` 输出必须为每个公开命令或子命令提供一句简短用途说明；隐藏内部命令（例如 `__complete`）除外。`kanban` 无参数或公开命令组缺少子命令时，必须显示同一类简洁帮助，而不是只输出解析错误；这仍属于 clap 参数解析阶段，退出码为 2，且不输出运行期 JSON 错误封装。全局选项的帮助必须说明它们影响的是 SQLite 数据库、当前看板、操作者、区域设置或 JSON 输出，不改变 JSON 键、状态枚举或退出码契约。

面向智能体的关键输入面必须在命令帮助中优先展示安全路径：多行或对 shell 敏感的文本使用 `--description-file -`、`--body-file -`、`--metadata-json-file <PATH|->`、`--metadata-file <PATH|->` 或 `--input -`，避免 shell 展开或引号处理污染。危险、破坏性或容易误解的标志必须在帮助中说明语义，例如 `task archive --force` 绕过普通归档保护，`import --replace` 是明确用于备份恢复流程的替换式恢复入口；兼容性空操作标志必须明确写出其不执行额外操作。

对 `PATH|-` 文本输入（如 `--reason-file`、`--input`、`--body-file`、`--metadata-json-file`）与其变体，`kanban` 实现上约束单次输入上限为 1MiB。超过上限时返回 `invalid_input`，并在 `--json` 下通过 `error.message` 指明输入长度限制，CLI 端可用更高层分片策略。该约束覆盖标准输入与文件输入，目的是避免错误输入导致 CLI 服务路径资源异常。

顶层帮助和面向智能体的关键命令可以包含 `Examples:`，但示例必须保持短小、稳定，并与实际命令语义一致；不要把 CLI 规范的完整说明复制进帮助。CLI 帮助契约由 `crates/kanban-cli/tests/help.rs` 覆盖，防止公开命令行退化为空描述。

顶层 `kanban --help` 必须包含简洁的 `Error codes:` 小节，覆盖当前公开退出码，帮助操作人员在终端直接发现参数解析阶段与运行阶段的错误码边界。该小节是人类可读的发现入口；脚本仍应依赖 `--json` 下的 `error.code` 和 `error.exit_code`，不要解析帮助文案。

### 1.3 JSON 输出契约

所有公开 `--json` 输出使用顶层封装：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {},
  "meta": {}
}
```

`meta` 只在需要分页、详细信息或诊断信息时出现。`data` 可以是一个对象，也可以是对象数组；公共输出不得依赖裸元组、未命名数组位置、只有内部 ID 的临时数组，或只回显输入参数。命令需要表达关系、删除或当前选择时，应返回命名 DTO，例如 `edge.parent`/`edge.child`、`step`、`board`。任务类 DTO 必须带可复制的 `ref`、`id`、`board_id` 或 `board_slug` 中的必要身份字段。

`board current --json` 和 `board use --json` 的 `data.board` 是完整看板对象；调用方应读取 `data.board.slug`，不要把 `data.board` 当字符串。

#### JSON 错误输出

当 `--json` 已被 clap 成功解析，且错误发生在运行期服务或 I/O 路径时，CLI 向标准输出写入稳定的错误封装，并使用对应退出码：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "error": {
    "code": "not_found",
    "message": "未找到：board missing",
    "exit_code": 3
  }
}
```

`error.code` 是脚本可依赖的 ASCII 枚举；`message` 是本地化的人类可读说明；`exit_code` 与进程退出码一致。运行期 `--json` 错误不写标准错误。

`error.code` 不应依赖业务校验消息文案推断；普通业务层 `KanbanError::InvalidInput` / `InvalidStatus` 都返回稳定的 `invalid_input`。已通过 clap 解析后的用户配置 TOML 解析失败也属于 `invalid_input`：例如 `kanban --json config show` 读取格式错误的 `.kb/config.toml` 或 `$XDG_CONFIG_HOME/kanban/config.toml` 时，向标准输出写入运行期 JSON 错误、退出 2、不写标准错误，并且不打开、初始化或创建 SQLite 数据库。仅对缺少结构化分类的外层错误（I/O、路径、异常第三方文本），以及穿过 `InvalidInput` 的 SQLite 或维护锁哨兵值，使用降级文本分类作为补充，例如 `sqlite_busy`。

参数解析错误发生在 clap 解析阶段，仍由 clap 写入标准错误并退出 2；这类错误不输出 JSON 封装。没有 `--json` 时，运行期错误继续写入人类可读的标准错误。

### 1.3.1 JSONL / NDJSON 流式输出边界

JSONL/NDJSON 只适用于流式或面向记录的接口，例如可移植导出/导入、监视/事件流，或未来逐条输出的长流命令。该类输出必须满足：标准输出中的每一行都是独立有效的 JSON 对象，编码为 UTF-8，记录之间仅用换行符分隔；人类可读的诊断、进度、警告和运行期错误不得混入同一个标准输出数据流。

有限命令仍使用 `--json` 的 `{data, meta?}` 成功封装或 `{error:{code,message,exit_code}}` 运行期错误封装。JSONL/NDJSON 不替代有限命令封装，也不能成为未设计的全局 `--jsonl` 快捷方式。若某个命令支持 `--out -` JSONL 流，则它不得与 `--json` 共享标准输出；需要结构化错误时，必须在命令级定义流错误策略，并用逐行 JSON、标准输出/标准错误纯净性和退出码测试覆盖。

当前公开错误代码：

| `error.code` | 退出码 | 含义 |
|---|---:|---|
| `generic_error` | 1 | 未分类通用错误。 |
| `invalid_input` | 2 | 参数已通过 clap 解析，但业务输入、值域或校验无效。 |
| `not_found` | 3 | 看板、任务、标签、步骤、运行记录等对象未找到。 |
| `invalid_transition` | 4 | 状态机拒绝该转换，或必需执行计划/步骤未满足。 |
| `claim_conflict` | 5 | 并发领取，以及能被分类器识别的领取或心跳冲突。 |
| `dependency_blocked` | 6 | 依赖未完成导致任务不能进入 ready/running。 |
| `sqlite_busy` | 7 | SQLite 忙碌/锁定，或维护锁/运行锁造成阻塞。 |
| `integrity_check_failed` | 8 | `doctor`、导入或维护过程发现完整性或一致性硬错误。 |
| `storage_error` | 1 | 其它存储错误；不保证可按 SQLite 锁或完整性错误自动恢复。 |

当前完成、提交审核或阻塞时的 `claim token mismatch` 会归类为
`invalid_transition`（退出码 4），不是 `claim_conflict`。自动化调用方应以这里记录的
当前分类为准，不要把所有领取凭证错误都假定为退出码 5。

### 1.4 Shell 补全

```bash
kanban completions <shell>
kanban __complete <kind> [prefix]
```

`kanban completions <shell>` 向标准输出写入补全脚本。支持的 shell：

```text
bash | zsh | fish | powershell | elvish
```

所有受支持的 shell 都会生成静态命令和选项补全。Bash 与 zsh 脚本还包含动态钩子，
会调用隐藏的内部辅助命令 `kanban __complete`，获取由数据库提供的候选值：

- 任务、评论、事件、运行记录和依赖命令所需的任务引用；
- `--board` 和看板身份参数所需的看板 slug；
- `--status` 所需的状态值；
- `comment add --kind` 所需的评论类型值（`note`、`decision`、`signal`）。

`kanban __complete` 是供 shell 脚本和测试使用的内部辅助命令，结果按换行符分隔。它接受：

```text
task-ref | dependency-task-ref | board | status | comment-kind
```

为满足补全场景，该辅助命令必须保持静默：数据库文件缺失、数据库未初始化、看板配置缺失，
或读取/查询失败时，都成功退出且不返回候选值、不写标准错误。生成静态补全脚本本身
不会打开或创建 SQLite 数据库。

### 1.5 Codex 钩子

```bash
kanban hook codex install [--handler-command <command-prefix>] [--timeout 30] [--record-signals] [--json]
kanban hook codex status [--json]
kanban hook codex uninstall [--json]
kanban hook codex handle failure [--record-signals]
kanban hook codex handle task-create
```

`kanban hook codex` 管理一组 Codex 生命周期钩子，为智能体提供与看板相关的反馈。
钩子安装到 Codex 用户配置路径：`$CODEX_HOME/hooks.json`；未设置 `CODEX_HOME` 时
使用 `~/.codex/hooks.json`。该功能不提供项目级安装模式，因为 kanban 旨在跨工作区
提供一致的 CLI 感知行为。

钩子提示词从用户的 kanban 配置路径读取：
`$XDG_CONFIG_HOME/kanban/codex-hooks.json`，通常是
`~/.config/kanban/codex-hooks.json`。若文件不存在，`install` 会用中文默认提示词创建，
但不会覆盖现有文件。若提示词文件缺失、格式错误、使用不支持的 `version`，或绑定指向
不存在的提示词别名，处理程序会回退到内置中文默认值，而不会让 Codex 钩子失败。

`install` 在匹配器 `^Bash$` 下添加两个受管的 `PostToolUse` 命令钩子：
一个处理失败的 `kanban ...` 命令记录，另一个为成功的
`kanban task create ...` 提供后续建议。受管命令前缀默认为
`kanban hook codex handle`；安装的命令如下：

```bash
kanban hook codex handle failure --installed-by kanban-hook-codex [--record-signals]
kanban hook codex handle task-create --installed-by kanban-hook-codex
```

`uninstall` 只删除带隐藏标记 `--installed-by kanban-hook-codex` 的钩子，并保留
无关的用户钩子。重复运行 `install` 是幂等的：写入新钩子前会替换先前的受管钩子。

`handle failure` 和 `handle task-create` 是内部钩子命令。它们从标准输入读取
Codex 钩子 JSON，不输出内容，或直接输出如下 Codex 钩子响应对象：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"systemMessage":"检测到 kanban CLI 命令失败。\n\n命令：kanban task list --bad-flag\n退出码：2\n\n继续调整。修正后继续当前任务，并在确有必要时记录后续工作。"}
```

`handle` 子命令有意不使用常规 `{ "data": ... }` JSON 封装，因为 Codex 会直接消费
钩子的标准输出。公开管理命令 `install`、`status` 和 `uninstall` 则使用常规
`--json` 封装。

提示词配置模式：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "version": 1,
  "codex_hooks": {
    "bindings": {
      "failure": "failure.zh-default",
      "task_create": "task_create.zh-default"
    },
    "prompts": {
      "failure.zh-default": "检测到 kanban CLI 命令失败。\n\n命令：{{command}}\n退出码：{{exit_code}}\n\n继续调整。修正后继续当前任务，并在确有必要时记录后续工作。",
      "task_create.zh-default": "检测到 kanban task 创建成功。\n\n命令：{{command}}\n任务：{{task_ref}}\n\n请考虑为该 task 执行 label/signal follow-up；需要标签时先运行 `kanban label suggest {{task_ref}} --json`。不要自动写 label ontology，除非已有完整 suggestion snapshot 和明确 decision payload。"
    }
  }
}
```

支持的占位符有意保持精简：

- `failure`：`{{command}}`、`{{exit_code}}`；
- `task_create`：`{{command}}`、`{{task_ref}}`。

`stderr` 和 `stdout` 不是提示词占位符。使用 `handle failure --record-signals` 时，
它们仍会作为有界的内部证据保存在所记录的通用信号中。

V1 行为：

- 非 `Bash` 工具和不调用 `kanban` 的 Bash 命令都是空操作；
- `handle failure` 只报告失败的 `kanban ...` 命令，提示词来自
  `codex-hooks.json` 或内置中文默认值；
- `handle failure --record-signals` 还会记录一个通用信号，其中包含
  `kind="agent_cli_failure"`、`source="kanban-hook-codex"` 和有界的命令证据；
- `handle task-create` 只报告成功的 `kanban task create ...` 命令，并使用
  `codex-hooks.json` 或内置中文默认值渲染标签/信号后续提示；
- 钩子绝不会静默启动 Codex 原生子智能体，也不会自动写入标签本体。它只注入建议；
  当前 Codex 会话必须自行决定是否启动原生智能体或记录本体观察。

---

## 2. 退出码

| 代码 | 含义 |
|---:|---|
| 0 | 成功。 |
| 1 | 通用错误或未分类存储错误。 |
| 2 | clap 参数错误，或运行期校验/无效输入。 |
| 3 | 未找到对象。 |
| 4 | 非法状态转换，或必需执行计划/步骤未满足。 |
| 5 | 并发领取，以及能被分类器识别的领取或心跳冲突。 |
| 6 | 依赖阻塞。 |
| 7 | SQLite 忙碌/锁定，或维护锁/运行锁阻塞。 |
| 8 | 完整性检查失败或一致性硬错误。 |

---

## 3. 初始化

### 3.1 `kanban init`

初始化本地数据库、默认看板和默认列。该命令是幂等的；重复执行只会应用缺失的迁移并确保默认数据存在，不会重置或覆盖已有任务数据。`--force` 是兼容旧脚本的空操作，不改变 `init` 行为。

```bash
kanban init
kanban init --db .kb/kb.db
kanban init --force
```

`--force` 是已弃用的兼容性空操作：保留用于兼容旧脚本，不改变 `init` 行为，不执行重置或覆盖，也不会绕过迁移或模式校验。

输出：

```text
Initialized Kanban database at ~/.local/share/kb/kb.db
Default board: default
```

JSON：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "db_path": "/home/user/.local/share/kb/kb.db",
    "board_id": "b_01HX...",
    "board_slug": "default"
  }
}
```

---

## 4. 看板命令

### 4.1 列出看板

```bash
kanban board list [--include-archived]
```

### 4.2 创建看板

```bash
kanban board create <slug> --name <name> [--description <text>]
```

示例：

```bash
kanban board create agent-work --name "智能体工作"
```

### 4.3 查看看板

```bash
kanban board show <slug>
```

### 4.4 选择看板

```bash
kanban board use <slug-or-id>
```

写入：

```toml
board = "agent-work"
```

写入当前目录的 `.kb/config.toml`。

### 4.5 查看当前看板

```bash
kanban board current
```

应用 `--board`、`KB_BOARD`、项目配置和回退优先级后，显示最终解析出的当前看板。
看板解析与数据库路径解析相互独立：`--db` / `KANBAN_DB` / `KB_DB` 决定打开哪个
SQLite 数据库，`--board` / `KB_BOARD` / `.kb/config.toml` 中的 `board` 则决定选择
该数据库里的哪个看板。

### 4.6 归档看板

```bash
kanban board archive <slug>
```

除非传入 `--include-archived`，否则 `kanban board list` 不显示已归档看板。系统会拒绝
向已归档看板进行普通任务写入。只要能显式解析出任务或看板，仍可通过任务、事件、运行记录
和评论历史命令读取审计历史。若看板中仍有 `running` 工作，归档请求会被拒绝；应先完成、
阻塞或回收这些工作。

---

## 5. 任务命令

### 5.1 创建任务

```bash
kanban task create <title> [OPTIONS]
```

选项：

| 选项 | 说明 |
|---|---|
| `--description <text>` | Markdown 描述。 |
| `--description-file <PATH|->` | 从文件或标准输入（`-`）读取 Markdown 描述；与 `--description` 互斥。推荐用于多行或包含 `$`、反引号、JSON 等对 shell 敏感的文本。 |
| `--status <status>` | 显式指定初始状态：`triage` / `todo` / `scheduled` / `ready`。 |
| `--assignee <name>` | 负责人或工作者配置名称。 |
| `--priority <int>` | 优先级 `0..3`：`0` = P0 事故、阻塞项或必须立即处理；`1` = P1 近期重点；`2` = P2 重要后续；`3` = P3 普通待办、低优先级或默认值。非法值会被拒绝。 |
| `--scheduled-at <epoch_ms>` | 计划时间，Unix 纪元毫秒数。 |
| `--due-at <epoch_ms>` | 截止时间，Unix 纪元毫秒数。 |
| `--max-retries <n>` | 工作者失败或回收后最多重试次数。 |
| `--label <name>` | 创建时附加已存在标签，可重复；若看板内缺少任一标签，整个创建操作都会被拒绝。 |
| `--metadata <json>` | 扩展 JSON。 |
| `--metadata-file <PATH|->` | 从文件或标准输入（`-`）读取扩展 JSON；与 `--metadata` 互斥。推荐用于避免 JSON 的 shell 引号处理问题。 |

优先级只表达相对重要性和排序，不表示任务可以被领取。只有 `ready` 才表示任务已被显式
放入可执行队列；普通 `ready` 任务通常仍应是 P1/P2/P3，不能为了表示“下一批可做”
而全部标成 P0。P0 只用于事故、当前目标的阻塞项或必须立即处理的任务；若 P0 任务仍缺
规格、排期未到或依赖未完成，它仍保持 `triage` / `scheduled` / `todo`，不能被领取。

`task create` 可以请求 `--status ready`，但新任务创建时尚无执行计划。服务会把任务实际
保存为 `todo`；查询和响应会把执行计划状态派生为 `unplanned`，并不会为此写入计划行。
添加第一个步骤或执行
`task step not-required` 后，服务才会结合规格、排期和依赖等其他保护条件重新计算状态。
显式请求 `scheduled` 时必须同时提供 `--scheduled-at`；显式请求 `ready` 时必须有非空
描述，且排期不能位于未来。

示例：

```bash
kanban task create "修复 claim 队列阻断回归" --priority 0
kanban task create "实现状态机" --description "补齐状态转换和测试" --priority 1 --status ready
kanban task create "补充文档示例" --priority 2
kanban task create "明早检查报告" --scheduled-at 1780640400000
kanban task create "修复 API 回归" --label backend --label p1
```

`--label` 只绑定当前看板中已存在的标签身份。名称会先去除首尾空白；空白名称会被拒绝。
任一标签缺失时，整个创建操作返回无效输入，且不会写入 `tasks`、`labels`、
`task_labels` 或 `task_events`。需要新的词汇身份时，先显式运行
`kanban label create`，或使用 `kanban label add --create-missing` 这类明确的身份
创建入口；任务创建本身没有自动创建缺失标签的模式。

人类可读输出：

```text
agent-work#12 [todo] P1 实现状态机 · plan: unplanned · steps: 0/0
```

JSON 输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "id": "t_01HX...",
    "board_id": "b_01HX...",
    "board_slug": "agent-work",
    "ref": "agent-work#12",
    "seq": 12,
    "status": "todo",
    "title": "实现状态机",
    "labels": []
  }
}
```

### 5.2 列出任务

```bash
kanban task list [OPTIONS]
```

选项：

| 选项 | 说明 |
|---|---|
| `--status <status>` | 按状态过滤，可重复。 |
| `--assignee <name>` | 按负责人过滤。 |
| `--label <name>` | 按标签名称或 ID 过滤，可重复；多个标签使用 AND 语义。 |
| `--search <query>` | 模糊搜索标题和描述；形似任务引用的查询按精确匹配处理。 |
| `--include-archived` | 包含已归档任务。 |
| `--limit <n>` | 限制数量。 |
| `--offset <n>` | 分页偏移。 |
| `--sort <field>` | `seq` / `title` / `status` / `position` / `priority` / `assignee` / `scheduled_at` / `due_at` / `created_at` / `updated_at`。降序可用 `<field>_desc`，也兼容 API 风格 `-<field>`。`priority` 按 P0 → P3 排序；`priority_desc` / `-priority` 按 P3 → P0 排序。 |
| `--plan-needed` | 只列出执行计划仍为 `unplanned` 的活动任务。 |
| `--has-steps` | 只列出至少有一个步骤的任务。 |
| `--incomplete-required-steps` | 只列出存在未完成必需步骤的任务。 |
| `--plan-filter <filter>` | 可重复：`plan-needed` / `has-steps` / `incomplete-required-steps`。 |

优先级排序不会把工作提升为 `ready`；它只对所选结果集中的任务排序。

`--search` 对任务引用形状使用精确匹配，而不是文本包含匹配：
纯数字 `12`、`#12` 匹配当前看板内的序号；`board#12` / `board/#12`
只在该看板与当前列表请求的看板相同时匹配；`t_...` 只匹配当前列表请求看板
内的任务 ID。其他文本仍执行标题和描述的模糊搜索。

示例：

```bash
kanban task list
kanban task list --status ready --status running
kanban task list --label backend --label p1
kanban task list --assignee agent-default --json
kanban task list --plan-needed
kanban task list --plan-filter incomplete-required-steps
```

### 5.3 查看任务

```bash
kanban task show <task_ref>
kanban task show <task_ref> --details
```

默认人类可读输出仍是紧凑的单行任务摘要；默认摘要便于快速扫描，保留可复制的引用、状态、
优先级、标题、标签，以及必要的计划/步骤信号，不默认展示内部 `t_...` ID：

```text
agent-work#12 [ready] P1 实现状态机 · plan: planned · steps: 0/0
```

`--details` 改变人类可读输出，按 `Task`、`Description`、`Plan`、`Schedule`、
`Timestamps`、`Execution`、`Result`、`Metadata` 分组显示易读字段列表。可用时包含
任务引用、ID、状态、标题、完整多行描述、负责人、优先级、标签、`scheduled_at`、
`due_at`、`created_at`、`updated_at`、执行计划状态、必需/可选步骤数量、领取信息、
运行记录、结果、元数据以及其他任务快照字段。
如果该任务有标签本体信号，详细输出还会追加紧凑的 `ontology_summary`，列出信号、
状态、降级、过期和操作数量、老化时间，以及少量信号 ID 示例。

`task show <task_ref> --json` 默认只返回 `{"data": TaskRecord}`。带 `--details`
时，`data` 仍是相同的 `TaskRecord`，但封装会包含
`meta.details.ontology_summary`；没有本体信号时该字段为 `null`。该摘要
只读，不改变任务、标签或本体信号状态。需要完整审核队列时继续使用
`label ontology list/show/review`。

`task_ref` 支持：

- `t_...`：全局任务 ID，忽略当前看板。
- `12`：当前看板内的序号。
- `#12`：当前看板内的序号；在 shell 中需要引号，例如 `'#12'`。
- `agent-work#12`：显式看板 slug + 序号。
- `agent-work/#12`：兼容别名/#序号形式。
- `b_01HX...#12`：显式看板 ID + 序号。

裸 `12` / `#12` 依赖当前看板；显式 `board#seq` 和 `t_...` 可跨当前看板使用。
当前版本会拒绝跨看板依赖。

### 5.4 更新任务字段

```bash
kanban task update <task_ref> [OPTIONS]
```

允许更新的选项：

| 选项 | 说明 |
|---|---|
| `--title <text>` | 更新标题。 |
| `--description <text>` | 更新描述。 |
| `--description-file <PATH|->` | 从文件或标准输入读取描述；与 `--description` 互斥。 |
| `--assignee <name>` | 更新负责人。 |
| `--clear-assignee` | 清空负责人；若同时提供 `--assignee`，以清空为准。 |
| `--priority <int>` | 更新优先级。 |
| `--scheduled-at <epoch_ms>` | 更新计划时间。 |
| `--clear-scheduled-at` | 清空计划时间；若同时提供 `--scheduled-at`，以清空为准。 |
| `--due-at <epoch_ms>` | 更新截止时间。 |
| `--clear-due-at` | 清空截止时间；若同时提供 `--due-at`，以清空为准。 |
| `--max-retries <n>` | 更新最大重试次数。 |
| `--clear-max-retries` | 清空最大重试次数；若同时提供 `--max-retries`，以清空为准。 |
| `--metadata <json>` | 更新扩展 JSON。 |
| `--metadata-file <PATH|->` | 从文件或标准输入读取扩展 JSON；与 `--metadata` 互斥。 |
| `--expected-lock-version <version>` | 要求任务当前 `lock_version` 与给定值一致，不一致时拒绝更新。 |

不允许通过更新命令修改状态；状态必须通过转换命令修改。允许更新的字段仍由共享服务路径
处理，因此修改描述、`scheduled_at` 等会影响规格或排期的字段后，服务会根据规格、
排期和当前依赖重新计算活动任务的目标状态并写入对应事件。依赖边通过 `kanban dep`
命令修改；`max_retries` 只更新重试策略，不触发状态重算。

示例：

```bash
kanban task update 12 --priority 1
kanban task update t_01HX --description "新的规格"
kanban task update t_01HX --description-file - <<'EOF'
新的多行规格，保留 $VAR、$(command)、反引号和 JSON 字面量。
EOF
kanban task update t_01HX --max-retries 2
kanban task update t_01HX --clear-max-retries
```

---

## 6. 状态转换命令

### 6.1 提升为可执行

```bash
kanban task promote <task_ref>
```

手动尝试 `todo/scheduled -> ready`。

### 6.2 启动/领取

```bash
kanban task start <task_ref> [OPTIONS]
kanban task claim <task_ref> [OPTIONS]
```

`start` 是 `claim` 更便于人类理解的别名。

选项：

| 选项 | 说明 |
|---|---|
| `--ttl-ms <ms>` | 领取有效期（TTL）。默认 300000。 |

输出：

```text
Claimed t_01HX... token=claim_01HX...
```

JSON 返回规范领取快照：`data.task` 是闭合的 `ApiTask`，`data.run`
是闭合的 `ApiRun`，令牌只允许出现在顶层 `data.claim_token`。下面仅节选身份
与状态字段；实际对象还包含各自模式声明的其余字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "status": "running",
      "current_run_id": "r_01HX..."
    },
    "run": {
      "id": "r_01HX...",
      "task_id": "t_01HX...",
      "status": "running"
    },
    "claim_token": "claim_01HX...",
    "claim_expires_at": 1717520000000
  }
}
```

### 6.3 心跳

```bash
kanban task heartbeat <task_ref> --claim-token <token>
```

选项：

| 选项 | 说明 |
|---|---|
| `--ttl-ms <ms>` | 延长 TTL。 |

显式心跳 API 保持兼容。除此之外，`running` 任务的有效任务级活动事件也会隐式刷新租约，
可作为存活信号；该隐式刷新不会再写 `task.heartbeat` 事件。看板级事件或没有 `task_id`
的事件不触发续租。

### 6.4 完成

```bash
kanban task done <task_ref> [--claim-token <token>] [--force]
kanban task complete <task_ref> [--claim-token <token>] [--force]
```

选项：

| 选项 | 说明 |
|---|---|
| `--claim-token <token>` | 从 `running` 完成时需要匹配当前领取；从 `review` 完成时可省略。 |
| `--force` | 只绕过 `running` 的领取匹配，不能绕过必需步骤守卫；仅供本地人工修复使用。 |

### 6.5 提交审核

```bash
kanban task review <task_ref> [--claim-token <token>] [--force]
```

使任务从 `running` 转为 `review`。正常路径需要匹配领取凭证；`--force` 只绕过领取守卫。

### 6.6 阻塞

```bash
kanban task block <task_ref> (<reason>|--reason-file <PATH|->)
```

选项：

| 选项 | 说明 |
|---|---|
| `--claim-token <token>` | 阻塞 `running` 任务时需要。 |
| `--force` | 强制阻塞。 |
| `--reason-file <PATH|->` | 从文件或标准输入（`-`）读取阻塞原因；与位置参数 `<reason>` 互斥。 |

### 6.7 解除阻塞

```bash
kanban task unblock <task_ref>
```

不会盲目进入 `ready`，而是根据规格、排期和依赖重新计算目标状态。

### 6.8 重新打开

```bash
kanban task reopen <task_ref> (--reason <text>|--reason-file <PATH|->)
```

只允许重新打开 `done` 任务，原因必填且不能为空，可用 `--reason-file <PATH|->`
从文件或标准输入读取；它与行内 `--reason` 互斥。重新打开会清空
`completed_at`，保留 `result_summary` / 自然 JSON `result`（持久层仍存于
`result_json`），并按规格、排期、依赖和执行计划就绪情况重新计算目标状态。

如果被重新打开的任务是其他任务的依赖父项，直接子任务中仅
`triage|todo|scheduled|ready` 会重新计算；`running|blocked|review|done|archived`
不会被隐式改写。

### 6.9 回收

```bash
kanban task reclaim --expired
kanban task reclaim
```

当前 CLI 回收当前看板内已过期的领取；裸 `kanban task reclaim` 与
`kanban task reclaim --expired` 等价。
JSON 输出固定为 `{"data":{"reclaimed":<u64>}}`，且拒绝未声明字段。

### 6.10 归档

```bash
kanban task archive <task_ref>
```

选项：

| 选项 | 说明 |
|---|---|
| `--force` | 允许归档 `running` 任务，并关闭当前运行记录。 |

---

### 6.11 步骤/执行计划

```bash
kanban task step list <task_ref>
kanban task step add <task_ref> <title> [--body <text>|--body-file <PATH|->] [--link-task <task_ref>] [--position <n>] [--required|--optional]
kanban task step update <task_ref> <step_ref> [--title <text>] [--body <text>|--body-file <PATH|->|--clear-body] [--link-task <task_ref>|--unlink-task] [--position <n>] [--required|--optional]
kanban task step done <task_ref> <step_ref> (--note <text>|--note-file <PATH|->)
kanban task step skip <task_ref> <step_ref> (--reason <text>|--reason-file <PATH|->)
kanban task step reopen <task_ref> <step_ref> (--reason <text>|--reason-file <PATH|->)
kanban task step remove <task_ref> <step_ref>
kanban task step not-required <task_ref> (--reason <text>|--reason-file <PATH|->)
```

步骤是执行计划的一等结构化项目。它可以是纯文本步骤，也可以通过
`--link-task` 引用同一看板内的普通任务作为上下文。链接任务不等于依赖，
链接任务的状态不会自动完成步骤。步骤自身的状态是 `todo`、`done` 或
`skipped`。

`step_ref` 支持步骤 ID，也支持父任务列表里的 `S<n>` 序号。`add` 默认创建
必需步骤；`--required` / `--optional` 互斥。供人类使用的规范形式是不带值的标志，
但针对该标志，CLI 也接受有界的智能体生成值：`--required true`、
`--required=false`，以及对应的 `--required=true` / `--required false` 形式。
只有字面量 `true` / `false` 会被当作布尔值消费；`--required` 之后的普通位置文本
仍是位置参数，任何其他额外值仍会触发解析错误。`--body-file <PATH|->` 从文件或
标准输入读取长正文，与 `--body` 互斥；`update --clear-body` 也与 `--body-file`
互斥。`update` 只有在显式传入 `--required` 或 `--optional` 时才改变是否必需。
`done`、`skip` 和 `reopen` 必须记录说明文本。`--note-file <PATH|->` 和
`--reason-file <PATH|->` 从文件或标准输入读取较长的备注/原因，分别与行内
`--note` / `--reason` 互斥。

人类可读列表输出示例：

```text
Execution plan: planned
Required steps: 1/2 done-or-skipped
Optional steps: 1

S1 step_01HX... [done] required pos=1024 编写测试
S2 step_01HY... [todo] required pos=2048 link=default#13 验证桌面界面
S3 step_01HZ... [todo] optional pos=3072 发布说明
```

`task step not-required` 只在没有步骤时可用；它记录原因并解除 `ready`/领取的
执行计划门禁。已有步骤的任务不能标记为 `not_required`。

---

## 7. 依赖命令

```bash
kanban dep add <parent_ref> <child_ref>
kanban dep remove <parent_ref> <child_ref>
kanban dep list <task_ref>
```

`--json` 输出使用已补全信息的依赖 DTO。`dep list --json` 返回以所查询任务为中心的快照：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_child",
      "board_id": "b_default",
      "board_slug": "default",
      "ref": "default#2",
      "title": "子任务",
      "status": "todo"
    },
    "parents": [
      {
        "id": "t_parent",
        "board_id": "b_default",
        "board_slug": "default",
        "ref": "default#1",
        "title": "父任务",
        "status": "done"
      }
    ],
    "children": [],
    "edges": [
      {
        "parent": {
          "id": "t_parent",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#1",
          "title": "父任务",
          "status": "done"
        },
        "child": {
          "id": "t_child",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#2",
          "title": "子任务",
          "status": "todo"
        }
      }
    ]
  }
}
```

`dep add --json` 和 `dep remove --json` 返回：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "edge": { "parent": {}, "child": {} },
    "dependencies": { "task": {}, "parents": [], "children": [], "edges": [] }
  }
}
```

常用 jq：

```bash
kanban dep list default#2 --json | jq -r '.data.edges[] | "\(.parent.ref) -> \(.child.ref)"'
```

添加和移除依赖的人类可读输出以中文为主：

```text
已添加依赖：default#1 -> default#2
已移除依赖：default#1 -> default#2
```

添加依赖后：

- 如果子任务当前是 `ready` 且父任务未完成（不是 `done` 或 `archived`），子任务降级为 `todo`。
- 父任务完成、归档或依赖移除后，子任务保持 `todo`；需要
  `kanban task promote <task_ref>` 才显式进入 `ready`。归档父任务不会删除依赖边。
- 父任务从 `done` 重新打开后，直接子任务中仅 `triage|todo|scheduled|ready`
  会按就绪条件重算；`running|blocked|review|done|archived` 不会被隐式改写。
- 重复添加同一父任务/子任务边是幂等空操作：不追加新的 `dependency.added` 事件，
  也不再次触发子任务状态重算。
- 如果产生环，返回 `invalid_input`，退出码为 2。
- 当前版本拒绝跨看板依赖，即使父任务/子任务通过全局 `t_...` 或显式
  `board#seq` 解析成功。

`task list/show --json` 返回派生依赖字段：`dependency_blocked`
和 `unfinished_parent_count`。未完成父任务指状态不是 `done` 或 `archived` 的父任务；
这些字段用于区分仍被未完成父任务阻塞的 `todo`，与已解除依赖但尚未人工提升的 `todo`。

---

## 8. 标签命令

```bash
kanban label list
kanban label create <name> [--color <color>]
kanban label delete <label> [--force] [--json]
kanban label bootstrap <task_ref> <label> [--description <text>] [--applies-when <text>]... [--excludes-when <text>]... [--positive-example <text>]... [--negative-example <text>]... [--verify] [--min-verify-score 0.50] [--vector-config <toml>] [--json]
kanban label add [--create-missing] <task_ref> <label>...
kanban label remove <task_ref> <label>
kanban label semantics list [--json]
kanban label semantics show <label> [--json]
kanban label semantics upsert <label> [--expected-semantics-hash <hash>] [--replace] [--reason <text>|--reason-file <PATH|->] [--source-signal <signal_id>]... [--description <text>] [--applies-when <text>]... [--excludes-when <text>]... [--positive-example <text>]... [--negative-example <text>]... [--remove-applies-when <text>]... [--remove-excludes-when <text>]... [--remove-positive-example <text>]... [--remove-negative-example <text>]... [--json]
kanban label semantics delete <label> --expected-semantics-hash <hash> (--reason <text>|--reason-file <PATH|->) [--json]
kanban label atoms list [--json]
kanban label atom explain <atom-id-or-content-hash> [--json]
kanban label atom-index status [--vector-config <toml>] [--json]
kanban label atom-index rebuild [--vector-config <toml>] [--json]
kanban label atom-index query <text> [--polarity positive|negative] [--limit 24] [--vector-config <toml>] [--json]
kanban label suggest <task_ref> [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label propose <task_ref> [--proposal-json <path>] [--source-signal <signal_id>]... [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label proposals list [--task <task_ref>] [--status proposed|accepted|rejected] [--json]
kanban label proposals show <proposal_id> [--json]
kanban label proposals accept <proposal_id> [--reason <text>|--reason-file <PATH|->] [--source-signal <signal_id>]... [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label proposals reject <proposal_id> [--reason <text>|--reason-file <PATH|->] [--json]
kanban label ontology record <task_ref> --input <path|-> [--suggestion-snapshot <path|-> | --capture-suggest] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label ontology list [--status open|confirmed|resolved|rejected|superseded]... [--kind false_negative|false_positive|vocabulary_gap|name_issue|boundary_issue|structure_issue]... [--task <task_ref>] [--label <label>] [--proposed-label <name>] [--include-all] [--limit 100] [--json]
kanban label ontology show <signal_id> [--json]
kanban label ontology review [--group-by label|candidate-atom|proposed-label|cluster] [--include-all] [--limit 100] [--json]
kanban label ontology quality [--sample-limit 20] [--json]
kanban label ontology confirm <signal_id>... (--reason <text>|--reason-file <PATH|->) [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology reject <signal_id>... (--reason <text>|--reason-file <PATH|->) [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology supersede <signal_id>... --by <signal_id> (--reason <text>|--reason-file <PATH|->) [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology resolve <signal_id>... --no-change (--reason <text>|--reason-file <PATH|->) [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology apply atom <signal_id>... --label <label> --kind applies-when|positive-example|excludes-when|negative-example (--text <text>|--text-file <PATH|->) (--reason <text>|--reason-file <PATH|->) [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology revert <action_id> (--reason <text>|--reason-file <PATH|->) [--expected-current-hash <hash>] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --status passed|failed|partial (--reason <text>|--reason-file <PATH|->) --input <PATH|-> [signal_id]... [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --trusted --status passed|failed|partial (--reason <text>|--reason-file <PATH|->) [signal_id]... [--positive-control <TASK_REF>]... [--positive-control-waiver <REASON>|--positive-control-waiver-file <PATH|->] [--vector-config <toml>] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--actor-type user|agent] [--agent-type <type>] [--json]
```

`label atom-index status`、`rebuild` 和 `query` 复用向量 TOML 解析规则：
显式 `--vector-config`/`--config` 优先，其次是最近项目的 `.kb/config.toml`，
最后是全局配置。只有显式传入 `--vector-config` 时，辅助进程参数才会附带该值；
省略时由辅助进程按默认配置解析。

标签语义、提案和本体命令中的 `--reason-file <PATH|->`、
`--retarget-reason-file <PATH|->`、`--text-file <PATH|->` 和
`--positive-control-waiver-file <PATH|->` 从文件或标准输入读取对应长文本，并与同名
行内参数互斥。`label atom-index query <text>` 的 `<text>` 是短查询标量，不提供
文件输入；需要持久本体证据时使用 `label ontology record --input <path|->`
或 `label ontology validate --input <PATH|->`。

`label create` 在当前看板作用域内创建标签；如果同一看板已存在同名标签，则返回
已有标签。`label add` 接受任务引用和一个或多个标签名称；默认只绑定任务所属看板上
已经存在的规范标签。缺失标签会返回无效输入，并提示先用 `label create`、
`label bootstrap`、提案/采用路径创建，或在明确接受只创建规范身份的情况下传入
`--create-missing`。`--create-missing` 只创建 `labels` 身份并绑定任务，不生成
`label_semantics` 或 `label_atoms`；JSON 输出改为
`{ "task": <TaskRecord>, "created_labels": [...] }`。
`label remove` 接受任务引用和标签名称或 ID。空白标签名称会被拒绝。

`label delete <label>` 删除当前看板上的规范标签身份，区别于
`label remove <task_ref> <label>` 的任务级解绑。标签身份的增删改查不属于
本体账本；创建/删除只写普通看板/任务事件，不写本体变更操作。默认情况下，如果标签
仍绑定任何任务，系统会拒绝删除并报告绑定数量；显式传入 `--force` 时，只移除任务绑定
后删除空标签身份。若标签仍有
`label_semantics` 或 `label_atoms`，即使传 `--force` 也会拒绝；必须先用
`label semantics delete --expected-semantics-hash <hash> --reason <text>` 清空语义。
JSON 返回 `{ "label": <LabelRecord>, "forced": bool, "removed_task_bindings": n, "removed_semantics": false, "removed_atoms": 0 }`。
删除规范标签不改变任务
状态；被删除标签会从 `label list`、`task show/list` 的标签和后续建议事实
中消失。

标签变更对任务—标签关联保持幂等。只有关联实际变化时，才追加
`task.label.added` / `task.label.removed` 事件；该操作不改变任务状态。
批量 `label add` 会先验证所有标签名称；如果任一标签为空白、非法或缺失且未传
`--create-missing`，不会创建规范标签，也不会留下部分任务—标签绑定。
显式创建模式与添加单个标签相同，只创建缺失的规范身份，并在输出中列出本次新建的标签。

`label bootstrap` 是一次性新标签采用辅助命令：在同一事务内创建当前任务所属看板上
缺失的规范标签，或复用尚无语义的同名标签；写入该标签的 `label_semantics`，同步重建
SQLite `label_atoms`，将派生的标签原子向量索引标记为脏，并把该标签绑定到任务。
`<label>` 按名称解析；空白名称会被拒绝。语义输入会去除首尾空白并丢弃空白值，
且必须至少提供 `description` 或一个非空
语义数组值。

引导操作默认不会覆盖已有 `label_semantics`。如果同名标签已经有语义，
命令会失败，并要求改用专用语义变更或提案/采用路径；重复执行同一任务/标签时，
只有目标标签仍无语义才会保持任务—标签绑定幂等。JSON
返回 `{ "task": <TaskRecord>, "semantics": <LabelSemanticsRecord>, "verification": null|<Verification> }`。

当前轻量 CLI 构建已把标签建议/提案、引导阶段校验和标签原子状态/重建/查询接到向量辅助
子进程适配器；`kanban vector ...` 仍保留原始分块/标签原子查询入口，辅助进程内部用
标签原子专用命令处理 `lancedb_label_atoms`，不复用分块存储状态来伪装标签原子状态。

传入 `--verify` 或 `--vector-config <toml>` 时，CLI 使用提交前的分阶段校验：
先在规范数据库事务外读取当前任务、目标标签状态和看板本体摘要，并在隔离的临时原子存储
中加载当前原子与候选原子。随后对来源任务运行非降级的 `label suggest`，要求新标签
出现在 `selected_labels` 或 `candidates` 中，且分数至少达到
`--min-verify-score`（默认 `0.50`）。重建、建议、阈值、提供程序或临时存储失败时，
不会写入规范标签、语义、原子、任务—标签绑定、本体操作、事件或脏标记。如果向量辅助
进程/提供程序不可用，会返回明确的校验错误；需要离线验收时，也可改走外部证明
`--input` 路径。

验证通过后 CLI 才开启短 `BEGIN IMMEDIATE` 事务，重算任务建议输入哈希、目标标签状态
和看板本体摘要；任一值变化都会返回冲突且零写入。成功路径在一个事务中写入规范
标签/语义/原子、任务绑定、普通任务—标签事件、一个 `bootstrap_label` 根本体操作，
以及对应的新增原子影响。校验摘要会写入根操作的变更快照和 CLI 输出；它不等同于
提交后受信任校验。无可用向量提供程序时，校验会在写入前失败；不需要本地向量校验时，
省略 `--verify` 和 `--vector-config`。

示例：

```bash
kanban label create backend --color blue
kanban label delete old-label --json
kanban label delete old-label --force --json
kanban label semantics delete old-label --expected-semantics-hash sem_abc123 --reason "删除标签身份前停用旧语义" --json
kanban label bootstrap default#12 database --description "数据库持久化工作" --applies-when "涉及 SQLite 迁移" --positive-example "新增数据表迁移" --json
kanban label bootstrap default#12 database --description "数据库持久化工作" --applies-when "涉及 SQLite 迁移" --positive-example "新增数据表迁移" --vector-config .kb/vector.toml --min-verify-score 0.50 --json
kanban label add default#12 backend
kanban label create api
kanban label add default#12 backend api
kanban label add --create-missing default#12 scratch-label --json
kanban label remove t_01HX... backend
kanban label list --json
```

人类可读输出使用紧凑的标签行：

```text
backend l_01HX... color=blue
```

如果任务有人类可读的标签，摘要末尾会追加方括号标签列表：

```text
default#12 [ready] P1 修复 API 回归 [backend,p1] · plan: planned · steps: 0/0
```

`label suggest` 返回任务级标签建议。带内置标签原子向量存储的构建，会把任务标题和
描述的嵌入向量作为查询，使用 `lancedb_label_atoms` 按残差多轮检索正向标签原子，
并用原始查询检索负向原子，以施加惩罚或抑制。求解器在标签组层执行 Group OMP 选择，
再用所选标签的最相关正向原子向量执行非负重拟合；`coverage` / `residual_norm`
来自该原子级拟合向量，
其中 `coverage = clamp(1 - residual_norm, 0.0, 1.0)`，因此二者不是两份独立
证据；`coverage_cosine` 是原始查询与拟合向量的余弦相似度，
可作为独立补充指标。
候选标签只有在试探性重拟合后带来足够的残差范数降幅，才会进入结果；覆盖率或残差范数
达到停止阈值后，求解器会提前停止，而不是凑满 `--max-selected-labels`。候选组与
已选标签语义向量过度相似时会被跳过，以减少重复语义标签同时出现在已选标签中；
这不会合并或删除规范标签。
`needs_new_label` 是兼容字段，只表示存在需要人工审核的标签覆盖率诊断；
具体原因必须读取 `reason_codes`，例如 `no_selected_labels`、
`coverage_below_threshold`、`residual_above_threshold`、`unexplained_residual`，
或与降级相关的原因。不要把 `coverage` 与 `residual_norm` 重复计票，也不要仅凭
`needs_new_label=true` 创建新词汇；必须结合 `reason_codes`、证据原子、诊断信息
和人工语义判断。
它不会自动创建新标签，也不会写入新标签提案。应用建议时仍使用现有
`label add <task_ref> <label>...` / API attach 流程。

默认轻量 CLI 通过向量辅助适配器运行标签向量查询；辅助进程/提供程序不可用时，
命令成功返回降级结果而不是失败，且 `needs_new_label=false`。`--vector-config`
使用与 `kanban vector configure/status` 相同的 TOML 解析规则，并把解析出的嵌入模型
传给辅助进程查询。`LabelAtomHit.distance` 保留 LanceDB `_distance` 的原始语义；
建议/提案分数只根据返回的原子向量与当前查询/残差，在本地计算余弦相似度，
不从距离值推导。

JSON 输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_01HX...",
    "board_id": "b_01HX...",
    "selected_labels": [],
    "candidates": [],
    "coverage": 0.0,
    "coverage_cosine": 0.0,
    "residual_norm": 1.0,
    "needs_new_label": false,
    "reason_codes": ["degraded_result", "vector_store_disabled"],
    "degraded": true,
    "diagnostics": ["vector_store_disabled"]
  }
}
```

人类可读输出简洁列出建议标签、分数、权重和是否已应用；降级时追加诊断信息行。

`--limit` 只控制最终输出中 `selected_labels` / `candidates` 的最大条数，不会收窄
求解器内部搜索能力。内部能力由 `--candidate-limit`、`--atom-limit` 和
`--max-selected-labels` 分别控制：候选标签组数、每轮原子向量检索上限，
以及最多进入非负重拟合的标签数。所有限制参数都必须是
`1..=1000`；`--min-score` 必须在 `0..=1`。

标签本体的长期回归语料库目前是本地测试基础设施，不是会写生产数据库的 CLI 变更流程。
修改标签求解器、语义/原子生成、受信任校验或重要标签本体时，可以运行：

```bash
just test-p kanban-sqlite label_ontology_longitudinal_regression
```

该测试在临时 SQLite 数据库中建立固定的重要标签、已知正样本任务和负对照任务，重建
内存标签原子索引，保存基线 `label suggest` 结果，再模拟一次范围过宽的原子变更，
并比较已选标签、分数和证据原子。它会断言正常语料库运行不会修改 `labels`、
`task_labels`、`label_semantics`、`label_atoms` 或本体账本记录；真实项目语料库
应在积累稳定任务后逐步扩展，但不应成为每个日常任务标签绑定的默认必跑步骤。

`label semantics` 管理当前看板上已有标签的语义字典。`<label>` 接受标签名称或
`l_...` ID。`upsert` 默认是补丁：`--description` 只在提供非空值时覆盖当前描述，
数组参数会追加到对应集合，`--remove-*` 只删除匹配的既有文本；未提供的字段不会被
解释为清空。传入 `--replace` 时才执行完整替换，此时未提供的数组会成为空数组，
并且不能同时传入 `--remove-*`。`--expected-semantics-hash <hash>` 是比较并交换保护：
哈希不等于当前语义哈希时返回冲突且不写入。`--reason` 和 `--source-signal` 会进入
`update_semantics` 本体操作；即使没有来源信号，建设性语义变更也会在同一事务写入
前后哈希、变更快照和操作者来源。`upsert` 会写入 `label_semantics` 并同步重建
该标签的 `label_atoms`，随后将派生的标签原子向量索引标记为脏。数组参数可重复；
空白值去除首尾空白后会被丢弃。生成原子时，有描述的标签会生成一个规范的
`description` 原子：`label: {name}\ndescription: {description}`；没有描述时
才使用 `name` 回退原子。原子文本会进一步规范化空白：折叠每个非空行内部的空白，
保留规范行分隔。同一标签下相同 `polarity + kind + normalized_text` 的原子会去重
并保留首次序号，`id` / `content_hash` 不包含序号，因此只调整数组顺序不会改变
同一文本原子的身份。
`delete` 是受 CAS 保护的语义清空操作：必须传入
`--expected-semantics-hash <hash>` 和非空 `--reason <text>`。它删除该标签的
语义与 SQLite 原子，但不删除规范标签身份或任务—标签绑定；同一事务会写一个
`update_semantics` 根本体操作，操作后快照为空，并为实际移除的原子写入 `removed`
原子影响，随后将标签原子索引标记为脏。哈希不匹配时，规范数据、操作、影响和脏状态
全部不变。成功返回 `{ "data": { "deleted": true } }`。需要在清空后删除标签身份时，
先清空语义，再执行 `label delete`。

`label atoms list` 读取 SQLite `label_atoms` 物化投影。这些原子来自
`label semantics upsert`、`label bootstrap`、`label ontology apply atom`，或接受标签
提案后生成的语义；它们是 `lancedb_label_atoms` 派生索引的输入，不是派生索引本身。

`label atom explain <atom-id-or-content-hash>` 是 `label atoms explain` 的单数别名，
按当前看板的原子 ID 或稳定 `content_hash` 解析现有原子，并返回当前原子、规范语义、
来源操作、支持信号/来源任务和校验历史。当前原子存在，但没有本体来源操作引用其 ID 或
内容哈希时，命令成功返回 `legacy_untracked=true` 和 `legacy_reason`；未知 ID/哈希
返回未找到。JSON 输出是 `LabelAtomExplainRecord`，包含 `query`、`atom`、
`current_semantics`、`provenance_actions`、`supporting_signals`、
`validation_history`、`legacy_untracked` 和 `legacy_reason`。由于内容哈希不含序号，
语义重建后同语义原子的 ID 改变时，仍可用内容哈希解释历史。

`label atom-index status` 返回标签原子向量索引的状态。未配置提供程序或辅助进程不可用时，
仍成功返回禁用/降级状态。JSON 保留兼容字段 `message`，并返回结构化的
`diagnostics: string[]`、`dirty: boolean | null`、`board_dirty: boolean | null`；
调用方应使用结构化字段判断脏状态/错误，而不要解析 `message` 文案。`status` 通过
辅助进程的 `label-atoms-status` 命令读取 `LANCEDB_LABEL_ATOMS_STORE` 与
`label_atom_index_boards` 语义；`query` 通过辅助适配器查询标签原子向量索引，
`--polarity` 只接受 `positive` 或 `negative`，人类可读输出和 JSON 命中记录都把
LanceDB `_distance` 暴露为 `distance`。`rebuild` 通过辅助进程的
`rebuild-label-atoms` 命令重建标签原子派生索引；辅助进程/提供程序不可用时返回
显式错误，不修改 SQLite 中的规范标签事实，也不把分块存储标记为成功。

`kanban vector query-label-atoms` 是公开的原始辅助查询入口，支持文本查询和原始向量查询。
输入必须且只能选择一种：位置参数 `<text>`、`--text-file <PATH|->`、
`--vector-json <JSON>` 或 `--vector-json-file <PATH|->`。`-` 表示从标准输入读取。示例：
`kanban vector query-label-atoms --text-file query.txt [--polarity positive|negative] [--limit N] [--embedding-model MODEL] [--vector-config <toml>]`，或
`kanban vector query-label-atoms --vector-json-file vector.json [--include-vector] [--embedding-model MODEL] [--polarity positive|negative] [--limit N]`。
`--include-vector` 只对辅助进程支持的原始向量/向量命中输出有意义。

`label propose` 是独立的新标签语义提案流程，不复用或改变 `label suggest`。
它先读取当前任务级标签建议的 `coverage` / `coverage_cosine` / `residual_norm` /
最相关现有标签。没有 `--proposal-json` 时，默认提供程序不可用；命令成功返回降级尝试，
不创建规范标签、`label_semantics`、`label_atoms` 或 `task_labels`。日常标签建议
不依赖该提案提供程序。
`--limit` 只截断提案尝试中复用的建议输出；`--candidate-limit`、`--atom-limit`、
`--max-selected-labels`、`--min-score` 会在提案持久化前调节底层标签建议求解器，
用于计算覆盖率、覆盖率余弦值、残差范数和最相关现有标签。
`--vector-config` 使用与 `label suggest` 相同的 TOML 解析规则。默认轻量 CLI
通过向量辅助适配器运行残差校验；未配置或辅助进程/提供程序不可用时保持降级回退，
不写入普通标签或任务—标签关联。

提供程序边界：CLI 当前只使用禁用的提供程序，或通过 `--proposal-json` 显式传入的
本地/离线候选。真实 LLM 提供程序不属于 `kanban-sqlite`；未来若接入本机 AI 运行时，
应在 CLI、本地运行时或独立 AI crate 中实现 `LabelProposalProvider` 适配器，
再把候选交给 SQLite 服务做确定性校验和持久化。

`--proposal-json` 提供本地/离线提供程序输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "database",
  "description": "数据库持久化工作",
  "applies_when": ["涉及 SQLite 迁移"],
  "excludes_when": ["只调整界面"],
  "positive_examples": ["新增数据表迁移"],
  "negative_examples": ["只修改 CSS"]
}
```

数组字段缺省时按空数组处理。`name` 不能为空，且描述或任一语义数组至少需要提供
一个非空值。只有当前启发式覆盖率不足时才持久化提案。与现有标签发生规范化名称冲突的
候选会写成 `rejected` 提案，并在诊断信息中返回 `near_duplicate_label_conflict`；
该规范化名称检查忽略大小写、空白和标点，是确定性的近重复启发式规则。
覆盖率不足的候选还会执行残差最相关结果加间隔校验：候选语义的残差分数和现有标签
最相关结果，都按返回的原子向量在本地计算余弦相似度，不从 LanceDB 距离推导；
候选必须超过现有标签最相关结果，且超过幅度达到固定间隔。校验失败时，本次尝试仍会
把候选持久化为 `rejected` 提案，诊断信息包含
`label_proposal_residual_top1_failed` 或
`label_proposal_residual_margin_insufficient`，用于审计为什么没有进入可接受状态。
如果残差校验不可用或已降级，且没有明确通过最相关结果加间隔校验，本次尝试返回
`degraded=true`、`proposal=null`，不新增提案记录，也不创建规范标签、
`label_semantics`、`label_atoms` 或 `task_labels`；诊断信息包含
`label_proposal_residual_validation_unavailable` 和具体原因。
传入 `--source-signal <los_...>` 时，提案创建成功后会在同一事务写入
`create_label_proposal` 本体操作，并通过操作—信号链接记录该提案由哪些已确认的
词汇缺口信号支持；提案记录与来源操作要么同时写入，要么一起回滚。来源信号默认必须是
同一看板上 `confirmed` 的 `vocabulary_gap` + `bootstrap_label` 信号，且规范化后的
`proposed_label_name` 必须等于提案名称。`--actor-type` / `--agent-type` 控制该
`create_label_proposal` 操作的操作者来源；操作者名称仍来自全局 `--actor`。
确实需要把已确认的同看板来源信号重定向到该提案时，必须同时传入 `--allow-retarget`
和非空 `--retarget-reason <text>`；原因和来源信号原始目标/候选标签会写入
`change_json.retarget_override`。重定向不会放宽看板/状态要求。

`label proposals accept` 只接受 `proposed` 提案。接受操作与单任务引导共用
同一个采用原语：创建规范标签、`label_semantics` 与 `label_atoms`，
将标签原子索引标记为脏，并写入 `bootstrap_label` 本体操作；提案记录、规范写入和
操作来源要么在同一事务中成功，要么一起回滚。它不会自动给来源任务写入
`task_labels`。未传 `--source-signal` 时仍会记录引导操作，只是没有操作—信号链接；
传入 `--source-signal <los_...>` 时会通过链接记录该新标签引导的信号来源，
且这些来源信号必须是同一看板上的 `confirmed` 信号。`--actor-type` /
`--agent-type` 控制该 `bootstrap_label` 操作的操作者来源；操作者名称仍来自
全局 `--actor`。
默认是 `user`。`--actor-type agent` 必须提供非空 `--agent-type`；`user` 不能提供
`--agent-type`。来源信号默认还必须是 `vocabulary_gap` + `bootstrap_label`，
且规范化后的 `proposed_label_name` 必须等于提案名称。如果提案已有
`create_label_proposal` 操作，接受时产生的 `bootstrap_label` 操作会把
`parent_action_id` 指向该创建操作，形成“提案创建 → 引导接受”链路。
确实需要把已确认的同看板来源信号重定向到该提案时，必须同时传入
`--allow-retarget` 和非空 `--retarget-reason <text>`；该原因、来源信号的原始
目标/候选标签和最终提案/结果标签会写入引导操作的
`change_json.retarget_override`。重定向不会放宽看板/状态要求。
`label proposals reject` 把提案标记为 `rejected`，不接受 `--source-signal`。
已接受或已拒绝的提案不能再次决策。

`label ontology record` 记录一次标签判断观察，并写入其中的子信号。
推荐输入边界是：工具采集或接收未经改写的 `label suggest` 快照，服务从快照派生
覆盖率、残差、降级、诊断等观察指标；智能体只提交候选、最终判断、信号、候选原子和
理由。CLI 可以用 `--capture-suggest` 在记录前用同一组建议选项运行一次真实的
`label suggest`，也可以用 `--suggestion-snapshot <path|->` 读取已保存的原始建议
JSON。快照可以是直接的建议响应，也可以是带 `data` 封装的 JSON 响应。

`--input` 只接受契约所有的自然 JSON 结构；旧 `_json` 兼容同级字段（例如
`diagnostics_json`、`related_labels_json`）会作为未知字段拒绝。新调用方不应重复手写
`suggest_coverage`、`suggest_residual_norm` 或 `diagnostics`。如果快照中已有这些字段，
而输入又提供冲突的标量或诊断信息，命令会失败。服务会读取当前任务快照、解析目标标签
引用、计算规范化候选标签名称、信号键和候选原子内容哈希；观察记录同时保存用于完整审计的
`task_snapshot_json.content_hash`，以及只基于标签建议输入（规范化标题和描述）的
`suggest_input_hash`。它只写账本，不修改 `task_labels`、`label_semantics`、
`label_atoms`、标签原子索引或提案。

信号输入会在写入前做本体契约校验。`candidate_atom` 的
`applies_when` / `positive_example` 只能使用 `positive` 极性，
`excludes_when` / `negative_example` 只能使用 `negative` 极性。
`add_positive_atom` 必须提供目标标签和正向候选原子；
`add_negative_atom` 必须提供目标标签和负向候选原子；
`update_semantics` 必须提供目标标签；`bootstrap_label` 必须提供
`proposed_label_name`；`rename_label` 必须提供目标标签和
`proposed_label_name`；`split_label` / `merge_labels` 必须提供目标标签和非空
`related_labels`。观察指标 `suggest_coverage`、`suggest_coverage_cosine`、
`suggest_residual_norm` 以及信号指标
`suggest_score` / `confidence` 必须是 `0.0..=1.0` 范围内的有限数；`suggest_rank` 必须为
`null` 或 `>= 1`。
`rename_label` / `split_label` / `merge_labels` 当前只作为审核信号的候选操作保存，
CLI 不提供写入规范结构变更操作或结构计划操作的命令；旧结构计划记录只读展示为
不支持的校验要求。

使用已保存标签建议快照时，推荐的输入结构如下：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
  "agent_candidates": [
    {"label": "cli", "reason": "该任务会改变 CLI 行为。"}
  ],
  "final_decision": {
    "selected": ["cli"],
    "rejected": []
  },
  "signals": [
    {
      "kind": "false_negative",
      "target_label_ref": "cli",
      "related_labels": [],
      "proposed_action": "add_positive_atom",
      "candidate_atom": {
        "polarity": "positive",
        "kind": "applies_when",
        "text": "扩展 CLI 子命令、命令参数、帮助输出或机器可读 JSON 行为"
      },
      "proposal": {},
      "agent_selected": true,
      "suggest_state": "candidate",
      "suggest_score": 0.08,
      "suggest_rank": 6,
      "final_selected": true,
      "rationale": "该任务扩展了 CLI 接口。"
    }
  ]
}
```

调用示例：

```bash
kanban label suggest default#42 --json > /tmp/default-42-suggest.json
kanban label ontology record default#42 \
  --input /tmp/default-42-record.json \
  --suggestion-snapshot /tmp/default-42-suggest.json \
  --json
```

或者让 CLI 在记录前采集快照：

```bash
kanban label ontology record default#42 \
  --input /tmp/default-42-record.json \
  --capture-suggest \
  --vector-config ./vector.toml \
  --json
```

`label ontology list` 默认只返回 `open` 和 `confirmed` 信号。`--include-all`
返回完整历史；`--status`、`--kind` 可重复过滤，`--task`、`--label` 和
`--proposed-label` 用于按来源任务、目标标签或候选新标签查询。
`label ontology show` 返回信号、观察和关联操作。`label ontology review`
是只读聚合审核队列视图，默认只聚合 `open` 和 `confirmed` 信号；传入
`--include-all` 时包含 `resolved` / `rejected` / `superseded` 历史。`--group-by`
支持按 `label`、`candidate-atom`、`proposed-label` 或需显式选择的 `cluster` 聚合，
`--limit` 限制返回组数。`--json` 中每个组返回聚合维度、键、相关标签/候选原子/
候选标签、聚类键/原因（仅聚类视图有值）、不同任务数、信号/状态/降级/操作数量、
分数摘要、任务引用示例、信号 ID、操作 ID 和提案 ID。排序优先使用不同任务数，
其次是已确认数量、最新信号时间和键。

审核组只表示一组信号共享同一个聚合键，不证明它们一定来自同一个根因。
`--group-by label` 使用 `target_label_id` 作为键，缺失目标标签时使用
`no-target-label`。`--group-by proposed-label` 使用规范化后的候选标签名称，
缺失候选新标签时使用 `no-proposed-label`。`--group-by candidate-atom` 优先使用
`candidate_content_hash`；如果信号没有候选原子，则键会包含信号类型、
目标标签或候选标签，以及候选操作，例如
`no-candidate-atom|kind:vocabulary_gap|proposed:ontology ledger|action:bootstrap_label`。
这个回退规则避免把不同类型、不同标签或不同候选操作的空候选信号合并到一个全局桶。
`--group-by cluster` 是只读审核辅助：它不写规范原子，也不会确认、应用、校验或关闭
信号。聚类键在每次查询时从已有信号文本重建，优先使用词法规范化后的候选文本，
其次是候选标签，再其次是理由，最后才回退到类型/操作/目标/候选标签作用域组合；
所有聚类键都带有信号类型、候选操作、目标标签和候选标签作用域，避免跨标签、操作或
边界误合并；`cluster_reason` 说明当前键的来源。

`task_count` 是组内不同来源任务数，也是默认热度排序的第一依据；同一任务上的多条信号
仍只贡献一个不同任务。`signal_count` 是原始信号记录数，用于判断一组里有多少审查项；
它没有分母，不能解释为模型错误率、精确率或召回率。`degraded_count`、状态数量、
分数摘要和任务引用示例只是审核人员的排查线索。排序为 `task_count` 降序、
`confirmed_count` 降序、`latest_signal_at` 降序、`key` 升序；需要判断是否同一问题时，
应继续查看组内任务示例、信号 ID 和 `label ontology show` 详情。

`label ontology quality` 是只读质量/分析报告。它从当前看板的
`label_ontology_observations` 取得可审计分母，并从 `label_ontology_signals`
取得原始分歧数量；不会写入任务、标签、语义、原子或账本操作。JSON 输出包含：

- `denominator.source="label_ontology_observations"`、`observation_count`、
  `distinct_task_count`、一致/降级观察数量、时间范围和
  `sample_task_refs`。
- `disagreement.signal_count`、`disagreement.distinct_task_count`、`by_kind`、
  `by_status`。
- `rates.disagreement_task_rate`：只在分母至少包含一个一致观察时返回；
  只有信号的历史不会输出伪错误率。
- `precision_recall.available=false`，直到项目有带预期标签的独立评估样本群。
  原始信号只能说明记录过分歧，不能单独证明精确率、召回率、漏报率或模型错误率。

生命周期命令写入操作并同步更新信号状态：

- `confirm`：`open` 信号进入 `confirmed`。
- `reject`：把信号标记为 `rejected`。
- `supersede --by`：把重复或过时信号标记为 `superseded`；写入前会沿替代项的
  `superseded_by_signal_id` 链检查，拒绝会回到任一来源信号的环。
- `resolve --no-change`：记录无需修改本体的解决结果。

这些生命周期命令只记录审核/状态变化，不接受规范变更来源字段。
`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
`update_semantics`、`create_label_proposal`、`bootstrap_label`、`revert_ontology_mutation`
和 `validate` 等操作记录，只能由
`label semantics upsert`、`label ontology apply atom`、`label propose`、
提案接受、`label bootstrap`、`label ontology revert`、`label ontology validate`
等专用命令/服务路径在同一
同一事务中写入。通用操作命令不能伪造规范数据前后哈希、结果原子/标签/提案或校验载荷。
生命周期、应用原子、校验和带 `--source-signal` 的提案接受操作都支持
`--actor-type user|agent` 与 `--agent-type <type>`。这些标志只控制本体操作记录的
`created_by_type` / `agent_type`；操作名称仍来自全局 `--actor`。默认为
`--actor-type user` 且不写 `agent_type`。`agent` 操作者必须提供非空
`--agent-type`，`user` 操作者带 `--agent-type` 会被拒绝。

`label ontology apply atom` 只接受 `confirmed` 来源信号。它会读取目标标签的当前语义，
把泛化文本加入对应数组，走现有的语义更新/原子重建路径。如果规范内容实际新增原子，
会写入 `add_positive_atom` 或 `add_negative_atom` 操作，记录生成原子的软引用、
内容哈希、前后哈希、单份变更快照和一个 `added` 原子影响，并把
`validation_requirement` 置为 `required`。如果同内容原子已经存在，则写入仅记录来源的
`adopt_existing_atom` 操作，记录现有原子软引用、相同的前后哈希和来源信号链接；
该操作不修改语义/原子，不把原子索引标记为脏，`validation_requirement=none`
且有效结果为 `not_required`。
默认要求所有带 `target_label_id` 的来源信号都指向被修改标签；不匹配时拒绝并列出
违规信号 ID。原子文本不需要逐字等于来源信号的候选文本，审核人员可以写更泛化的规范
原子。确实需要重定向已确认的同看板信号时，必须传入 `--allow-retarget` 和非空
`--retarget-reason <text>`；操作的 `change_json.retarget_override` 会记录原因、
来源信号的原始目标/候选标签和最终目标标签。重定向不会放宽看板/状态要求。
该命令只有在规范原子实际新增时才把标签原子索引标记为脏；向量索引重建和后续建议校验
仍是第二阶段。

`label ontology revert <action_id>` 为已提交的标签级规范本体变更追加
`revert_ontology_mutation` 操作，并把目标标签语义恢复到被撤销操作的
`canonical_before_hash` / `change_json.before` 快照。当前只支持
`add_positive_atom`、`add_negative_atom` 和 `update_semantics`；不处理引导操作的
标签身份或任务绑定回滚。为避免覆盖后续修改，命令要求当前规范语义哈希仍等于目标操作的
`canonical_after_hash`；传入 `--expected-current-hash <hash>` 时，还会先对调用方持有的
快照执行 CAS 检查。成功后会写入仅追加的撤销操作，`parent_action_id` 指向被撤销操作，
复制原操作的来源信号链接，记录撤销前后快照，为本次撤销实际新增/移除的原子写入原子
影响，把标签原子索引标记为脏，并把 `validation_requirement` 置为 `unsupported`。
原变更操作不会被修改或删除。

所有规范语义/原子变更事务都遵循单根操作合同：同一事务只写一条根变更操作，
`change_json` 只保存一次语义前后快照；实际新增或删除的原子通过
`label_ontology_action_atom_effects` 记录 `added` / `removed` 影响。只修改描述的
补丁会写一条根操作和零个原子影响；空操作补丁不写操作/影响，也不把索引标记为脏。
原子解释优先使用影响记录；旧版逐原子操作仍保持兼容读取。

`label ontology validate` 为一个变更操作追加 `validate` 操作。父操作必须是同一看板上
`validation_requirement=required` 的规范变更操作，并携带规范结果证据（例如原子、
结果标签/提案引用、规范哈希和非空变更快照）。父操作的 `validation_status` 是历史兼容
字段，不再单独表达“是否需要验证”；读取时通过归并器暴露有效结果：
`not_required|unsupported|pending|passed|failed|partial`。

普通 `--input` 路径属于外部证明：CLI 读取调用方提供的 JSON，服务只把提供的载荷、
来源信号用例摘要、任务快照/建议输入哈希对比和父操作结果引用包装进校验封装。
公共的提供/采集载荷只在顶层 `manual` 保存一次；生成的 `cases[]` 使用
`after.manual_case_ref` 引用 `manual.cases[]` 中对应信号的证据，不在每个用例中重复
整份载荷。该路径可记录 `failed` / `partial` 诊断，但不能把 `passed` 写成受信任证明；
即使 JSON 自称
`evidence_type="automated"`，`--status passed` 也会被拒绝，关联信号不会被
关闭。

`--trusted` 路径才是受信任的自动校验。它不接受 `--input`，也不接受调用方手写的受信任
证据 JSON；CLI 只能走内置采集器。“受信任”表示工具在当前父操作、来源信号、规范哈希、
原子索引代次和指定用例/对照上做了机械采集和检查，不表示本体在全局语义上正确。
CLI 必须有可用的标签原子向量工作流适配器（当前轻量 CLI 尚未接入；旧内置
`vector-lancedb` 构建需可解析 `--vector-config` 或默认配置），先在 SQLite 事务外
重建原子索引，再用同一
`--limit` / `--candidate-limit` / `--atom-limit` / `--max-selected-labels` /
`--min-score` 选项对关联来源信号重新运行 `label suggest`，由工具生成
`evidence_type="trusted_automated"`、`collector.source="label_ontology_validate_trusted"`、
`embedding_model`、`solver_options`、干净的 `index.status` / `index.generation`
和逐信号 `cases[]`。写操作时，服务会在短事务内重新核验父操作、来源信号、规范结果哈希、
原子索引脏状态/错误状态和代次，防止查询后规范或派生状态已变化。脏、错误、禁用的索引，
缺失代次或过期代次，都不能产生受信任的通过结果。

`--positive-control <TASK_REF>` 与 `--positive-control-waiver <REASON>` 只用于
负向原子的受信任校验，且二者互斥；非负向父操作携带这些参数会被拒绝。
豁免只能由 `--actor-type user` 提交，原因必须非空。负向原子父操作若两者都缺失，
会在采集前失败。

`cases[]` 的 `case_type` 必须匹配父操作：`positive_atom`、`negative_atom`
或 `bootstrap_label`。正向原子校验要求 `after.degraded=false`、结果原子 ID/内容哈希
出现在 `after.evidence_atoms[]`、目标标签被选中或分数不低于 0.50，且分数/覆盖率
不恶化。负向原子校验要求结果原子 ID/内容哈希出现在
`after.negative_evidence_atoms[]`；在误报任务上，必须证明
`after.target.selected=false`，或前后分数都存在且结果分数低于先前分数；并且必须提供
至少一个 `after.positive_controls[]` 且全部通过、未退化，或提供原因非空的
`after.positive_control_waiver`。引导标签校验要求所有关联来源信号都有通过用例，
新标签/结果标签被选中或分数不低于 0.50，且证据原子来自结果标签。

校验可比性默认使用观察记录的 `suggest_input_hash`；状态、`updated_at`、
`lock_version` 或任务标签绑定只改变完整快照时，写入 `task_metadata_drift` /
`label_binding_drift` 警告，不会让已通过校验过期。标题/描述变化会写入
`suggest_input_drift` 并使该用例不可比较；旧观察缺少 `suggest_input_hash` 时写入
`legacy_suggest_input_hash_missing`，不能静默通过。`--status passed` 会把关联来源
信号转为 `resolved`；`failed` / `partial` 保留历史和证据，来源信号继续等待后续修正
或人工处理。

`label propose --json` 返回结构化尝试结果：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_...",
    "board_id": "b_...",
    "proposal": null,
    "degraded": true,
    "diagnostics": ["label_proposal_provider_unavailable", "vector_store_disabled"],
    "heuristic_coverage": 0.0,
    "heuristic_coverage_cosine": 0.0,
    "heuristic_residual_norm": 1.0,
    "top1_existing_label_id": null,
    "top1_existing_label_name": null
  }
}
```

---

## 9. 评论命令

```bash
kanban comment add <task_ref> (<body>|--body-file <PATH|->) [--kind note|decision] [--author-type user|agent] [--agent-type <type>] [--metadata-json <json>|--metadata-json-file <PATH|->]
kanban comment list <task_ref>
```

`--actor` 提供评论作者的显示身份。省略 `--kind` 时，服务默认为 `note`。
省略 `--author-type` 时，服务默认为 `user`；Codex、现有实验性调度器或其他自动写入方
应传入 `--author-type agent --agent-type <type>`。`signal` 是持久化评论类型，
但用户应通过 `kanban signal record` 创建信号反向链接评论，而不是手动使用
`comment add --kind signal`；这样信号账本和反向链接评论会在同一事务中写入。
`--body-file <PATH|->` 从文件或标准输入读取较长评论正文，并与行内 `<body>` 互斥；
多行或对 shell 敏感的评论文本推荐使用这种方式。`--metadata-json` 默认为 `{}`，
并且必须是 JSON 对象；`--metadata-json-file <PATH|->` 从文件或标准输入读取相同的
JSON 载荷，避免结构化载荷的 shell 引号问题，并与 `--metadata-json` 互斥。
使用 `--kind decision` 时，元数据必须满足结构化决策模式：非空 `options`、
唯一的小写 ASCII 选项 `slug`、与某个 slug 匹配的 `selected`、非空 `reason`，
以及可选但非空的 `risk` / `verification`。

智能体命令失败记录应保存为评论，而不是只留在聊天记录中。使用
`comment add --author-type agent --agent-type <name> --kind note --metadata-json <json>`，
在人类可读正文中写简短摘要，并把结构化记录放入元数据。最小记录载荷是包含以下字段的对象：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "tool": "kanban-cli",
  "command": "kanban task step add",
  "argv": ["kanban", "task", "step", "add", "..."],
  "intent": "添加必需的执行计划步骤",
  "why_selected": "任务需要跟踪执行计划，因此智能体选择了步骤命令",
  "actual_error": "unexpected argument 'true' found",
  "repair": "改用规范的独立 --required，或受支持的 --required true/false 形式重试",
  "product_signal": "面向智能体的布尔标志兼容性缺口",
  "followup_task": "default#123"
}
```

调用方可以添加其他字段，但对于把智能体命令失败记录转化为解析器、文档、技能或测试工作的
工具而言，这些字段名是稳定的最小契约。

面向智能体的富文本输入示例：

```bash
kanban comment add default#12 --body-file - <<'EOF'
正文可以安全包含 $VAR、$(command)、`code`、JSON 和多行文本。
EOF
```

有意义的多选项决策应使用 `--kind decision`。正文保留为人类可读的回退摘要，
结构化选项和选择数据只放在 `--metadata-json` 中：

```text
已决定继续使用 comment metadata 承载结构化决策信息，正文保留为简短结论，方便没有结构化渲染的环境阅读。
```

决策元数据示例：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "options": [
    {
      "slug": "comment-metadata",
      "title": "使用评论元数据",
      "detail": "把结构化决策数据存入 task_comments.metadata_json。"
    },
    {
      "slug": "decision-table",
      "title": "创建决策表",
      "detail": "创建独立的 task_decisions 表和选项记录。"
    }
  ],
  "selected": "comment-metadata",
  "reason": "让决策紧邻任务讨论，避免产生并行时间线。",
  "risk": "元数据模式需要严格校验。",
  "verification": "CLI、API 和桌面端测试覆盖创建、读取、渲染及非法元数据拒绝。"
}
```

琐碎命名、格式调整或纯机械选择无需创建决策评论。

人类可读输出保持紧凑，包含评论 ID、任务 ID、`created_at`、类型、作者身份、
`author_type`、可选的 `agent_type` 和正文：

```text
c_01HX... task=t_01HX... created_at=1717520000000 [note] 用户甲 (user): 可以审核了
c_01HX... task=t_01HX... created_at=1717520000100 [note] codex (agent/root): 测试已通过
```

JSON 输出使用标准封装：`add` 返回契约评论 DTO，`list` 返回该 DTO 列表，并包含自然、
无损的 `metadata` 对象。输入标志名 `--metadata-json` / `--metadata-json-file` 保持不变。
创建评论会写入 `task_events(kind='task.comment.created')`。

---

## 10. 事件命令

```bash
kanban events <task_ref>
kanban events --board default
```

不传 `<task_ref>` 时，按当前看板列出事件。已归档看板的事件仍可通过显式 `--board` 读取。

---

## 11. 运行记录命令

```bash
kanban runs <task_ref>
kanban run show <run_id>
kanban run logs <run_id>
kanban run logs <run_id> --tail-bytes 65536
```

`kanban run logs` 默认最多读取 256 KiB。传入 `--tail-bytes` 时只返回日志末尾的指定
字节数。`task_runs.log_path` 必须解析到受信任日志目录，且文件名匹配 `<run_id>.log`；
可疑路径会被拒绝。

---

## 12. 服务端命令

`kanban serve` 是受支持的本地 API 入口：

```bash
kanban serve
kanban serve --host 127.0.0.1 --port 8721
kanban serve --quiet
kanban serve --log-level warn
kanban serve --search-sync-interval-ms 5000
```

默认地址是 `127.0.0.1:8721`；`--host` 必须解析为回环地址，`--port` 指定端口。
仓库仍保留实验性 `kanban dispatch` 代码，但它不属于公开支持路径，本规范不把其内部
参数作为用户契约。

`kanban serve` 默认把启动诊断、HTTP 请求记录和优雅关闭通知写入标准错误；标准输出
保留给显式机器可读输出，不用于服务日志。使用 `--quiet` 可抑制服务诊断，
`--log-level <off|error|warn|info|debug|trace>` 可简单覆盖详细程度；也可省略两者并设置
`RUST_LOG`，使用高级跟踪过滤器。默认过滤器是
`kanban=info,kanban_cli=info,kanban_server=info,tower_http=info,kanban_desktop=info`。

Ctrl-C/SIGINT 会触发 `kanban serve` 优雅关闭、释放运行锁、以 `0` 退出，且不写标准输出。
`--quiet` 和 `--log-level off` 会抑制优雅关闭通知。关闭期间第二次按下 Ctrl-C 会立即
以代码 `130` 退出。

使用 `tantivy-backend` 构建二进制时，`kanban serve` 会启动保守的后台搜索同步循环。
循环在启动时立即尝试一次，随后每隔 `--search-sync-interval-ms` 毫秒调用
`sync_search_index`（默认 `5000`）。使用 `--search-sync-interval-ms 0` 可禁用。
未启用 `tantivy-backend` 时，该标志仍会被接受，但不会启动后台索引任务。

---

## 13. 搜索命令

### 13.1 `kanban search`

```bash
kanban search <query> [--status ready] [--status review] [--assignee worker-a] [--label backend] [--include-archived] [--limit 20] [--offset 0] [--json]
```

默认 CLI 构建启用 `tantivy-backend`。当 `index/v1/tasks/` 存在可读 Tantivy 索引时，
`kanban search` 使用 Tantivy；索引缺失、损坏、过期，或二进制显式以
`--no-default-features` 构建时，会回退到 SQLite，并在顶层 `meta` 中标记过期。
搜索匹配任务标题、描述、评论、运行摘要/错误，以及事件类型/载荷。

`--label <name-or-id>` 可重复；多个标签使用 AND 语义，并在搜索分页前过滤任务。
带标签过滤的 Tantivy 搜索会回退到 SQLite，以保持当前标签关联关系和分页语义正确。

形似任务引用的查询始终使用 SQLite 精确匹配语义，即使当前存在可用 Tantivy 索引：
纯数字 `12`、`#12` 匹配请求看板内的序号；`board#12` / `board/#12`
只在显式看板与请求看板相同时匹配；`t_...` 只匹配请求看板内的任务 ID。
这些查询不会因为标题、描述或聚合搜索文本包含相同数字/引用片段而返回额外任务。

人类可读输出会紧凑展示公开任务引用、状态、分数、标题，以及可用时的摘要片段。
默认不包含内部 `t_...` 任务 ID；任务 ID 仍可在 JSON 输出和面向诊断/详情的接口中取得。

```text
agent-work#12 [ready] score=60.0 实现状态机 - 就绪规格片段
```

JSON 输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "hits": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "score": 60.0,
        "snippet": "就绪规格片段",
        "task": {
          "id": "t_01HX...",
          "seq": 12,
          "status": "ready",
          "title": "实现状态机"
        }
      }
    ]
  },
  "meta": {
    "backend": "sqlite",
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0
  }
}
```

### 13.2 `kanban index`

```bash
kanban index status
kanban index doctor
kanban index rebuild
kanban index sync
```

默认 CLI 构建启用 `tantivy-backend`，Tantivy 索引是可重建的派生缓存；显式以
`--no-default-features` 构建时保留 SQLite 回退：

- `status` 返回后端和元数据。
- `doctor` 为脚本返回同样的回退健康状态元数据。
- `rebuild` 在 SQLite 数据库旁构建或替换 `index/v1/tasks/`，并在 `app_settings`
  中保存干净的高水位状态。
- `sync` 消费已保存高水位之后的 `task_events.id`，删除并重新索引受影响的任务聚合，
  只有成功提交后才推进高水位。
- 任务变更不会在事务内更新 Tantivy；变更后运行 `kanban index sync`，本地服务端/
  桌面会话也可依赖 `kanban serve` 后台同步，或用 `kanban index rebuild` 替换派生索引。

持久化设置键按看板区分，格式为 `search.tasks.state.<board_id>`。其 JSON 包含
`schema_version`、`index_version`、`backend`、`index_name`、`board_id`、
`last_event_id`、`dirty`、`updated_at` 和可选 `message`；现有 `app_settings`
处理会将它纳入 JSONL 导出/导入。

JSON 数据结构：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "backend": "sqlite",
    "derived_index": false,
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0,
    "message": "SQLite fallback search is active; no derived index exists yet"
  }
}
```

启用 Tantivy 并完成重建后，`backend` 为 `tantivy`，`derived_index` 为 `true`，
`index_version` 为 `tasks-v1`。当前 `MAX(task_events.id)` 大于已保存的
`last_event_id` 时，`stale=true`，`index_lag_events` 报告事件滞后量。索引过期时
搜索会回退到 SQLite，以保证当前结果正确。后台同步错误不会让搜索放行过期的 Tantivy
结果；派生索引落后或不可用时，下一次搜索仍会报告过期/回退元数据，并返回当前 SQLite
结果。

---

## 14. 信号账本

```bash
kanban signal record --board <slug> --input <path|-> --json
kanban signal list --board <slug> [--status open|confirmed|rejected|superseded|resolved]... [--kind <kind>]... [--task <task-ref>] [--include-all] [--limit 100] --json
kanban signal show --board <slug> <signal-id> --json
kanban signal review --board <slug> [--status open|confirmed|rejected|superseded|resolved]... [--kind <kind>]... [--task <task-ref>] [--include-all] [--limit 100] --json
kanban signal confirm [--board <slug>] <signal-id>... (--reason <reason>|--reason-file <PATH|->) [--json]
kanban signal reject [--board <slug>] <signal-id>... (--reason <reason>|--reason-file <PATH|->) [--json]
kanban signal resolve [--board <slug>] <signal-id>... (--reason <reason>|--reason-file <PATH|->) [--json]
kanban signal supersede [--board <slug>] <signal-id>... --by <replacement-signal-id> (--reason <reason>|--reason-file <PATH|->) [--json]
```

`signal list` 和 `signal review` 共享 `status`、`kind`、`task`、`include-all`、
`limit` 查询过滤参数。没有显式 `--status` 时，两者默认只返回 `open` 和
`confirmed`；此时传 `--include-all` 会取消默认状态过滤并返回完整历史。显式
`--status` 始终优先，即使同时传 `--include-all`，结果仍只包含指定状态。
`--status` 和 `--kind` 都可以重复传入。

`record` 输入 JSON 支持 `kind`、`title`、`summary`、`severity`，可选的 `task_ref` /
`task_id` / `run_id` / `comment_id`，以及 `actor`、`agent_type`、`dedupe_key`、
`source`、`evidence` 和可选 `comment.body`。`source` 是标识观察来源的字符串；
`command`、`cwd`、`exit_code`、`stderr` 或相关日志等结构化命令细节应放入自然的
`evidence` 对象。信号响应使用同样的自然对象，而不是转义后的 `evidence_json`
字符串。有任务上下文时，服务会在同一 SQLite 事务中写入信号账本记录和一条
`comment.kind = "signal"` 反向链接。信号反向链接的 `metadata` 包含
`type:"signal_link"`、`signal_id`、`observation_id`、`signal_kind` 和
`signal_status`；通用信号评论元数据保持开放且无损。V1 不会自动创建后续任务。

生命周期转换是 `open -> confirmed|rejected|superseded|resolved` 和
`confirmed -> resolved`。`supersede` 要求替代信号来自同一看板，并拒绝环。
生命周期原因可用 `--reason-file <PATH|->` 从文件或标准输入读取，并与行内
`--reason` 互斥。

## 15. 维护命令

```bash
kanban doctor
kanban stats
kanban backup --out backup.sqlite
kanban export --format jsonl --out board.jsonl
kanban export --format jsonl --out -
kanban import --input board.jsonl --dry-run
kanban import --input board.jsonl --replace
kanban vacuum
kanban checkpoint

kanban entity list [--kind task] [--limit 50]
kanban entity show kb://task/t_...
kanban outbox list [--status pending] [--limit 50]
kanban derived status
kanban graph status
kanban graph rebuild
kanban graph sync
kanban graph neighbors kb://task/t_... [--predicate depends_on] [--limit 50]
kanban graph query (<SPARQL>|--sparql-file <PATH|->) [--limit 50]
kanban vector configure [--provider ollama] [--endpoint http://127.0.0.1:11434] [--model qwen3-embedding:0.6b] [--dimensions 1024] [--skip-check] [--vector-config <toml>]
kanban vector status [--vector-config <toml>]
kanban vector rebuild [--vector-config <toml>]
kanban vector sync [--vector-config <toml>]
kanban vector query-chunks <text> [--limit 10] [--vector-config <toml>]
kanban vector query-label-atoms (<text>|--text-file <PATH|->|--vector-json <json>|--vector-json-file <PATH|->) [--board-id <id>] [--embedding-model <model>] [--polarity positive|negative] [--include-vector] [--limit 10] [--vector-config <toml>]
kanban context build t_... [--lexical-limit 5] [--graph-limit 10] [--vector-limit 5] [--max-items 20] [--vector-config <toml>]
```

`kanban stats --json` 返回状态数量、过期 `running` 领取列表、阻塞原因聚合、
未规划的活动任务数量，以及必需步骤未完成的活动父任务数量，供本地操作人员恢复使用。
`kanban graph query` 的 SPARQL 可用 `--sparql-file <PATH|->` 从文件或标准输入读取，
并与位置参数 `<SPARQL>` 互斥。
`kanban vector query-chunks` 只接受必填的行内短文本；`query-label-atoms` 则要求在
行内文本、文本文件、行内向量 JSON 和向量 JSON 文件四种输入中选择一种。

`kanban backup` 使用 SQLite `VACUUM INTO` 创建一致备份；目标文件已存在时失败，
避免覆盖。`backup --out -` 会被明确拒绝，因为 SQLite 备份需要文件系统路径，
不能安全写入标准输出。

`kanban export --format jsonl` 导出数据库记录；目标文件已存在时失败，避免覆盖旧快照。
`export --out -` 会把 JSONL 快照写入标准输出，不输出人类可读状态文案，也不写标准错误；
该模式不能与 `--json` 组合，因为 JSONL 流和 JSON 封装不能共享标准输出。21 个稳定
判别值的输入/输出分别拥有 42 个精确模式根；每行数据闭合，必需但可为空的键不能省略，
但可显式为 `null`；导出/导入描述符与模式权威来源同源。

JSONL 不复制 `task_runs.log_path` 指向的外部日志文件，导出的运行记录会清空 `log_path`；
导出中的活动 `running` 任务会清除领取并恢复为 `ready`，对应的 `running` 运行记录会
变为 `canceled`，并追加 `task.export_sanitized` 事件解释这次可移植快照改写。需要完整
可恢复副本时使用 `kanban backup`。JSONL 导出包含通用信号账本记录类型
`signal_observation`、`signal`，以及标签本体账本记录类型
`label_ontology_observation`、`label_ontology_signal`、`label_ontology_action`、
`label_ontology_action_atom_effect` 和 `label_ontology_action_signal`；因此可移植 JSONL
与 SQLite 备份都会保留信号、本体观察/信号/操作/影响的来源记录。
JSONL `event.data.payload` 仍按不透明 JSON 保存；39 种类型的联合只属于事件 API/SSE。

`kanban import --dry-run` 会在临时 SQLite 数据库中解析导入文件并运行同一最终
`doctor` 门禁，不替换或创建所选目标数据库；脚本和 CI 可先用它验证快照。上一版导出器的
存储原生快照只作为单向兼容输入：同一记录如果同时出现自然命名的新键与对应格式的
存储原生旧键，会在兼容性规范化前以 `invalid_input` 拒绝，不能由旧值静默覆盖自然命名值。

`kanban import --replace` 是替换式恢复入口，必须显式传入 `--replace`；导入文件必须至少
包含一个看板，且每个看板必须包含列。该命令只能离线运行；运行前必须停止 `kanban serve`
和其他持有活动运行锁的进程；如果检测到活动运行锁会直接拒绝。导入会在同一 SQLite 事务内
执行插入与最终 `doctor` 门禁：基础关系表会校验 `task_labels`、`task_dependencies`、
`task_runs`、`task_comments`、`task_events`、`task_attachments` 的记录看板与所引用的
任务/标签/运行记录看板一致；失败时整个替换事务回滚，不提交部分数据。

本体导入会延迟回填 `label_ontology_signals.superseded_by_signal_id` 与
`label_ontology_actions.parent_action_id`，因此不依赖 JSONL 中同表自引用记录的偶然顺序；
导入后会拒绝跨看板/孤立的通用信号上下文、通用信号替代环、跨看板本体链接、
孤立的操作—信号链接、本体替代环和操作父级环。
`kanban entity`、`kanban outbox`、`kanban derived` 是知识底座的只读维护入口。
SQLite 仍是事实源；这些命令只报告统一实体注册表、派生索引发件箱和派生存储状态，
不改变任务状态或领取。
`kanban entity list --json` 返回 `{"data": [...]}`，`kanban entity show --json` 返回
`{"data": {...}}`；两者共享闭合的公开实体项，并保留
`uri`、`kind`、`source_table`、`source_id`、`created_at`、`updated_at`，以及
必需但可为空的 `board_id`、`task_id`、`title`、`summary`、`content_hash`、
`archived_at`。调用方不能把这些字段缺失解释为 `null`。`list` 的 `--kind` 与
`--limit` 由同一 SQLite 服务查询执行；`show` 继续按精确 URI 查询并保留
`not_found` 错误封装。人类可读输出不变。

`kanban graph` 和 `kanban vector` 是辅助子进程派生层入口。源码默认 feature 图不链接
Oxigraph/LanceDB 重型依赖；Linux release cohort 为统一 maintenance runtime 显式启用
`tantivy-backend,oxigraph-backend`，但 graph/vector 命令仍按辅助进程边界解析
`KANBAN_GRAPH_HELPER` /
`KANBAN_VECTOR_HELPER`、`/usr/lib/kanban/<helper>`、CLI 同目录二进制、
`KANBAN_CARGO_TARGET_ROOT` 或 `CARGO_TARGET_DIR` 的 `release/<helper>`，最后回退到
`PATH` 中的辅助程序。辅助程序缺失或返回非法封装时，`status` 返回禁用/降级状态；
辅助程序错误封装、错误的看板/数据库/配置或载荷/领域错误会作为命令错误返回。
启用后仍只作为可重建的关系/向量存储，不参与任务状态事务。

`kanban vector status --json` 保留 `message` 兼容字段，同时返回结构化
`diagnostics`、`dirty`、`board_dirty` 字段；脏状态/错误判断应使用这些字段，不解析
`message` 文案。
`kanban vector configure` 默认写入全局配置：
`$XDG_CONFIG_HOME/kanban/config.toml`（平台默认通常为
`~/.config/kanban/config.toml`），并默认配置本机 Ollama 嵌入提供程序。
传入 `--vector-config <toml>`（别名 `--config`）时写入指定 TOML。配置命令默认调用
`/api/embed` 做短文本维度校验；校验失败时不写配置；`--skip-check` 只跳过这次
连通性/维度检查。配置格式：

```toml
board = "kanban-tool"

[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "qwen3-embedding:0.6b"
dimensions = 1024
```

项目级 `.kb/config.toml` 可以覆盖全局 `[vector]`；命令行
`--vector-config <toml>` 优先级最高。解析顺序是：显式 `--vector-config`、最近的项目
`.kb/config.toml`、全局配置。`kanban board use <board>` 更新项目配置文件的
`board` 字段时必须保留该文件内已有 `[vector]` 配置。配置有效且辅助程序可用时，
`kanban vector status/rebuild/sync` 使用该提供程序。`kanban context build` 通过 SQLite
补全规范任务，并在可用时合并词法、图和向量命中；图或向量未配置、不可用或失败时，
以降级标记回退到仍可用的结果。失败原因通过有界诊断信息暴露，上下文包本身仍可用。

`kanban outbox list --json` 返回 `{"data": [...]}`，每项保留完整发件箱作业字段，
包括必需但可为空的 `source_event_id` 与 `last_error`；`--status` 与 `--limit` 由同一
SQLite 服务查询执行。`kanban derived status --json` 同样返回 `{"data": [...]}`，
每个存储的 `last_rebuild_at`、`last_sync_at` 与 `last_error` 都是必需但可为空，
调用方不能把字段缺失解释为 `null`。

`kanban derived status` 中的 `last_event_id` 是存储级成功处理水位，不是当前看板的
局部水位。`dirty=true` 表示该存储仍有任意看板的待处理/运行中/失败发件箱作业，
或最近一次派生更新失败；按看板工作的 `kanban index sync`、`kanban graph sync`、
`kanban vector sync` 只清理当前看板的作业，不能因为本看板干净就强制清掉全局脏状态。

语义标签原子使用独立派生存储 `lancedb_label_atoms`，对应 LanceDB 表
`kb_label_atoms`。它不属于普通任务事件发件箱扇出：`kanban vector sync/rebuild`
只维护 `lancedb_chunks` / `kb_chunks`，不会把标签原子存储标记为完成。标签语义服务
写入 `label_semantics` / `label_atoms` 后，会单独把 `lancedb_label_atoms` 标记为脏；
提供程序或功能不可用时，该存储可报告降级，但不影响普通 `kanban label` 增删改查和
`task_labels` 绑定。

### 15.0 `kanban maintenance`

`kanban maintenance status --json` 返回 Projection v2 database identity、singleton
owner 和全部 store 状态。owner 包含实际编译的 `capabilities[]` 与
`build_identity`，但绝不返回 lease token。每个 store 另有闭合的
`runtime_availability`：`available`、`unavailable` 或 `unverified`。当前二进制缺少
backend 时使用 `unavailable` + `backend_unavailable`；活动 owner 未声明该 store
capability 时使用 `unverified` + `maintenance_owner_capability_unverified`。因此
`doctor --strict-derived` 不会把 feature-limited owner 误判为全部派生层健康。
continuous `maintenance run` 只有在当前运行制品声明全部 projection store capability
时才会领取 singleton lease；feature-limited 制品返回 `invalid_input`，且不得留下 owner
或 lease。`run --once` 与定向 `rebuild` 仍可用于该制品实际编译的 store。

`kanban maintenance run --once --json` 和
`kanban maintenance rebuild (--all | <store>) --json` 的 `stores[]` 使用闭合
`result` 联合：成功分支为
`{"status":"succeeded","action":...,"processed":...}`；局部失败分支为
`{"status":"failed","kind":"provider|backend|delivery","message":...}`。
store 局部失败不会阻止同一 pass 尝试后续已编译 store；数据库、owner、lease/fence
或 shutdown 的全局失败仍使命令失败。脚本必须根据 `result.status` 和结构化 `kind`
判断，不解析 `message` 文案。

上述 status/run/rebuild machine contract 都是破坏性替换后的 v2 schema root；旧 v1
artifact 已移除，不提供新旧输出双轨。

### 15.1 `kanban doctor`

检查：

- 数据库文件存在。
- 迁移完整；当前已提交的迁移版本（`schema user_version`）为 29。
- `PRAGMA integrity_check`。
- 孤立的活动运行记录。
- `running` 任务是否缺少领取。
- 过期领取数量。
- 依赖环。
- 已归档依赖边（允许“已归档父任务 → 活动子任务”作为历史；报告“活动父任务 →
  已归档子任务”）。
- 缺失的运行日志文件。
- 可疑运行日志路径。
- `ready/running` 任务带有未完成父依赖。
- `ready/running` 任务缺少可执行规格。
- `ready/running` 任务带有未来的 `scheduled_at`。
- 基础关系表看板一致性：`task_labels`、`task_dependencies`、`task_runs`、
  `task_comments`、`task_events`、`task_attachments` 的记录看板必须和所引用的
  任务/标签/运行记录看板一致。当前模式用按看板区分的复合外键保护
  `task_labels`、`task_dependencies`、`task_runs`、`task_comments` 和
  `task_attachments`；v22+ 还检查 `task_execution_plans` 的任务看板作用域，v23+ 还检查
  `task_steps` 的父任务/链接任务看板作用域。`task_events` 保留可为空的任务/运行记录引用
  与 `ON DELETE SET NULL` 语义，通过 INSERT/UPDATE 触发器校验非空引用的看板作用域。
- SQLite `PRAGMA foreign_key_check`：`doctor` 将每条违规转成硬错误问题；
  JSONL 导入最终门禁也会在提交前运行同一检查，失败时回滚整个替换事务。
- `index_outbox` 积压：`outbox_pending`、`outbox_running`、`outbox_failed`。
- 派生存储健康状态：`derived_dirty_stores`、`derived_error_stores`、
  `derived_stores[]`。每个存储包含 `dirty`、`last_error`，以及按存储目标聚合的
  待处理/运行中/失败发件箱数量。
- 基础关系一致性：人类可读输出包含 `consistency_errors` /
  `consistency_warnings` 计数；`--json` 额外返回 `consistency_issues[]`，每条问题
  包含 `severity`、`code`、`message`、`record_ids`。消息包含 `table`、`row`、
  `row_board`、`referenced` 和 `referenced_board`。非零 `consistency_errors` 会让
  `ok=false`。
- 标签本体账本健康状态：v12+ 数据库必须存在 `label_ontology_observations`、
  `label_ontology_signals`、`label_ontology_actions`、
  `label_ontology_action_atom_effects`、`label_ontology_action_signals`；`doctor`
  会报告观察/信号/操作/操作影响/操作—信号的跨看板链接、孤立链接、父操作异常、
  替代环和可检查的软引用不一致。人类可读输出包含 `ontology_ledger_errors` /
  `ontology_ledger_warnings` 计数；`--json` 额外返回 `ontology_ledger_issues[]`，
  每条问题包含 `severity`、`code`、`message`、`record_ids`。非零
  `ontology_ledger_errors` 会让 `ok=false`；警告用于可重建或可解释的软引用异常，
  不会单独让 `doctor` 变为不健康。

`dirty` / 待处理发件箱表示派生层需要同步或重建，不会改变 SQLite 中的任务事实；
失败的发件箱作业或 `last_error` 用于帮助操作人员判断是否需要运行
`kanban index sync`、`kanban graph sync/rebuild` 或 `kanban vector sync/rebuild`。
`derived_stores[].last_event_id` 表示对应存储已成功提交的全局事件水位；
当 `dirty=true` 时，它仍然只是“已成功处理到哪里”的摘要，不代表所有看板都已经干净。

---

## 16. JSON 契约索引

JSON 输出、运行期 JSON 错误、clap 参数解析阶段错误、标准错误/标准输出数据平面，
以及 JSONL / NDJSON 流式输出边界的权威契约，统一见
[1.3 JSON 输出契约](#13-json-输出契约)。

本节仅保留跳转，避免同一份 CLI 规范出现两个 JSON 契约来源。新增或修改 JSON /
JSONL / 错误码行为时，只更新 1.3 及对应命令章节，并补充测试证据。


---

# 文件：docs/API_SPEC.md

# 本地 Web API 规范

本 API 只面向 Tauri Desktop 和本地脚本。它不是远程协作 API。

独立运行 `kanban serve` 时默认监听：

```text
127.0.0.1:8721
```

Tauri 内嵌运行时绑定 `127.0.0.1:0`，由操作系统选择可用端口。

基础路径：

```text
/api/v1
```

---

## 1. 通用约定

### 1.1 内容类型

请求：

```http
Content-Type: application/json
```

响应：

```http
Content-Type: application/json
```

SSE：

```http
Content-Type: text/event-stream
```

### 1.2 操作者

因为没有多用户系统，`actor` 是审计字段。

来源优先级：

1. 请求体中的 `actor`。
2. 请求头 `X-KB-Actor`。
3. 服务端默认的 `actor`。
4. 操作系统用户名。

#### 1.2.1 请求头契约

除 SSE 事件流外，当前 83 个 HTTP 端点都拥有端点专属、精确且
`deny_unknown_fields` 的请求头契约。每份契约都包含可选的 `Accept-Language`，并按处理器的真实
输入选择语言、语言加操作者、语言加 JSON 内容类型，以及它们允许省略请求体的变体。
`X-KB-Actor` 只出现在会解析操作者的变更处理器中。

必须提供 JSON 请求体的端点要求且只允许一个 `Content-Type`；允许省略请求体的归档、
推进、回收、解除阻塞，以及标签提议、接受和拒绝端点将其建模为可选；没有
请求体的端点不声明 `Content-Type`。这些数量约束属于传输契约，不改变 Axum
对具体媒体类型和格式错误 JSON 的既有 400 行为。

SSE 的 `Last-Event-ID` 仍明确标记为 `Excluded`：当前运行时忽略该请求头，没有续传契约；
不得因为其他端点已经收紧请求头，就把它推断为已采用的输入。

### 1.3 成功响应

成功响应按端点的元数据契约使用以下线上封装：

`DataEnvelope` 仅包含 `data`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {}
}
```

`MetadataEnvelope` 的 `meta` 是必需字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {},
  "meta": {}
}
```

`OptionalMetadataEnvelope` 只在端点产生对应元数据时包含 `meta`；没有元数据时
直接省略该字段，不返回 `"meta": null`。具体端点使用哪一种封装及其
`meta` 字段，由该端点的响应示例和说明定义。

### 1.4 错误响应

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot claim task from status todo"
  }
}
```

`error.code` 是稳定的机器契约。`error.message` 是供人阅读的文案，会根据
`Accept-Language` 在 `zh-CN` 和 `en` 之间选择；未传请求头时保持既有默认值 `en`。
客户端逻辑必须读取 `error.code`，不要解析 `error.message`。

### 1.5 HTTP 状态码映射

| 错误代码 | HTTP 状态码 |
|---|---:|
| `invalid_input` | 400 |
| `not_found` | 404 |
| `conflict` | 409 |
| `dependency_cycle` | 409 |
| `invalid_transition` | 409 |
| `dependency_blocked` | 409 |
| `execution_plan_required` | 409 |
| `steps_incomplete` | 409 |
| `claim_conflict` | 409 |
| `claim_token_mismatch` | 403 |
| `internal` | 500 |

---

## 2. 健康检查

### `GET /health`

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "ok": true,
    "db": "ok",
    "version": "2.1.3",
    "db_path": "/home/alice/.local/share/kb/kb.db",
    "db_fingerprint": "sqlite:131072:1717520000000"
  }
}
```

`db_path` 和 `db_fingerprint` 让本地桌面端或 Web 开发界面能够确认由哪一个
SQLite 运行实例响应了请求。若配置的数据库文件已被删除，`/health` 会返回
`400 invalid_input`，而不是重新创建空的 SQLite 文件。其他 API 路由也会在运行处理器前
执行相同的文件缺失检查，因此过期或已删除的运行实例会明确失败，不会在配置路径上打开
新的空数据库。`/health` 还会验证数据库是否具备预期的迁移后结构；空数据库或未初始化的
SQLite 文件同样返回 `400 invalid_input`。

---

## 3. 看板

### 3.1 列出看板

```http
GET /api/v1/boards?include_archived=false
```

默认隐藏已归档看板；传入 `include_archived=true` 可将其一并返回。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "b_01HX...",
      "slug": "default",
      "name": "默认看板",
      "description": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000,
      "archived_at": null
    }
  ]
}
```

### 3.2 创建看板

```http
POST /api/v1/boards
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "slug": "agent-work",
  "name": "代理工作",
  "description": "本地代理任务看板",
  "actor": "alice"
}
```

成功时返回 `201 Created`。看板 slug 必须唯一且非空，长度不超过 64 字节；首字符必须是
小写 ASCII 字母或数字，后续只能包含小写 ASCII 字母、数字、`.`、`_`、`-`，且不能以
`b_`、`t_`、`r_`、`c_`、`a_`、`l_`、`col_`、`e_` 等保留 ID 前缀开头。slug 重复或
格式无效时返回标准 `400 invalid_input` 错误封装，而不是 `500`。

### 3.3 获取看板

```http
GET /api/v1/boards/{board_slug_or_id}
```

### 3.4 归档看板

```http
POST /api/v1/boards/{board}/archive
```

归档会设置 `archived_at` 并写入 `board.archived` 事件，但不会修改任务。若看板上存在
活跃的 `running` 任务或仍在运行的任务执行记录，操作会返回 `409 invalid_transition`。
归档后，该看板上的普通任务变更会被拒绝；显式指定任务或看板标识时，审计历史端点仍可读取。

### 3.5 看板端点的精确契约

四个看板端点使用端点专属的契约根：列表查询、创建请求、获取或归档路径，以及各自的成功响应。
四个成功响应只共享闭合的 `ApiBoard` 组件；服务端会把 SQLite 应用记录显式映射为线上 DTO，
不会直接序列化 `BoardRecord`。归档请求体继续复用既有的 `ArchiveBoardRequest` 契约。

`include_archived` 默认为 `false`，传入 `true` 时会真实转发给服务层并返回已归档看板。
桌面端的 `listBoards` 调用方会精确校验 `data` 封装和 `ApiBoard` 的全部字段；字段缺失、
类型错误或出现额外字段时返回 `invalid_response`。运行中工作项的归档保护、已归档看板的
审计历史、未找到时的状态码和错误代码，以及依赖语言的消息文案不属于模式文件的权威范围，
继续由服务层和适配器保证。四个端点的路径、查询、请求头、请求体和成功响应契约均已采用，
迁移状态为 `Adopted`。

---

## 4. 任务

### 4.1 列出任务

```http
GET /api/v1/boards/{board}/tasks
```

查询参数：

| 参数 | 说明 |
|---|---|
| `status` | 可重复：`?status=ready&status=running`。 |
| `priority` | 可重复：`?priority=0&priority=2`，值为 P0-P3 的 `0..3`。P0 表示事故、阻塞项或必须立即处理的任务；P3 是普通待办、低优先级和默认值。 |
| `assignee` | 按执行者过滤。 |
| `label` | 按标签名称或 ID 过滤，可重复；多个标签使用 AND 语义。 |
| `plan_filter` | 可重复：`plan_needed` / `has_steps` / `incomplete_required_steps`。 |
| `q` | 搜索标题或描述；任务引用形状按精确匹配处理。 |
| `include_archived` | 布尔值。 |
| `limit` | 默认 100。 |
| `offset` | 分页偏移量。 |
| `sort` | `seq` / `title` / `status` / `position` / `priority` / `assignee` / `scheduled_at` / `due_at` / `created_at` / `updated_at`，前缀 `-` 表示降序。`priority` 按 P0 到 P3 排序；`-priority` 按 P3 到 P0 排序。 |

这两个任务读取端点使用同一套严格的原始查询语法，但各自拥有独立的精确路径与查询契约，
并由两个服务端本地的强类型 Axum 提取器分别绑定真实 `{board}` 路径和唯一的原始 URI
查询消费点；处理器只接收已解析请求，不持有 `RawQuery` 或第二套 `Query<T>` 提取器。
只有 `status`、`priority`、`label`、`plan_filter` 可以重复；不同语义值按 URI 首次出现顺序
保留，任何重复语义值返回 `400 invalid_input`。`assignee`、`q`、`include_archived`、
`limit`、`offset`、`sort` 任一重复也返回 `400`。任何未知 key 失败关闭；旧 `search`
alias 已删除，只接受 `q`。

原始查询最多 8192 字节。参数对上限不是独立字面量，而是由 9 个 `status`、4 个
`priority`、3 个 `plan_filter`、32 个 `label` 和 6 个标量参数推导出的 54。
`q` 最多 1024 个 Unicode 字符，`assignee` 与单个 `label` 最多 128 个。未提供查询时
默认 `include_archived=false`、`limit=100`、`offset=0`、`sort=position`。`limit` 的线上
权威上限是 `kanban-contract` 的 1000；SQLite 服务层的防御性上限直接引用唯一的应用权威值，
服务端对这条实际服务路径建立编译期相等门禁。`offset` 最大为
`i64::MAX`。空的 `q`、`assignee` 归一化为未提供；label 会规范化 Unicode 边缘空白，但必须
包含至少一个非空白字符，且 raw 字符长度不得超过 128；该预算在 trim 前计算，随后会被移除
的 Unicode 边缘空白也计入 128 字符。空或纯 Unicode 空白 `label`、enum、bool、数字或 sort
值无效。
查询使用严格的表单解码：`+` 表示空格，`%HH` 必须完整且解码结果必须是 UTF-8；合法
UTF-8 与 `&`、`/`、`=`、`+`、空格必须由标准表单编码器转义，非法百分号编码
或 UTF-8 返回 `400`。

优先级只表达相对重要性和排序，不表示能否认领。`ready` 才表示任务已显式进入可执行队列；
普通 `ready` 任务可以是 P1、P2 或 P3，不应为了表示“可做”而全部标成 P0。P0 只用于事故、
当前目标阻塞项或必须立即处理的任务；P0 任务若仍缺规格、排期未到或依赖未完成，仍不能被认领。

`q` 对任务引用形状使用精确匹配，而不是文本包含匹配：纯数字 `12` 和
`#12` 匹配 `{board}` 内的序号；`board#12` / `board/#12` 只在显式看板
与 `{board}` 相同时匹配；`t_...` 只匹配 `{board}` 内的任务 ID。其他文本仍执行
标题和描述的模糊搜索。

响应（以下为字段节选；完整、可消费的成功响应以 `schemas/fixtures/api/list-tasks-response.v1.valid.json` 为准）：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "t_01HX...",
      "seq": 12,
      "board_id": "b_01HX...",
      "board_slug": "agent-work",
      "ref": "agent-work#12",
      "title": "实现状态机",
      "description": "...",
      "status": "ready",
      "priority": 1,
      "position": 1024,
      "assignee": null,
      "scheduled_at": null,
      "due_at": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000,
      "labels": [
        {
          "id": "l_01HX...",
          "board_id": "b_01HX...",
          "name": "core",
          "color": null
        }
      ],
      "dependency_blocked": false,
      "unfinished_parent_count": 0
    }
  ],
  "meta": {
    "limit": 100,
    "offset": 0,
    "total": 1
  }
}
```

### 4.2 创建任务

```http
POST /api/v1/boards/{board}/tasks
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "实现状态机",
  "description": "Markdown 规格",
  "status": "ready",
  "assignee": "local-worker",
  "priority": 1,
  "scheduled_at": null,
  "due_at": null,
  "max_retries": 2,
  "depends_on": ["t_01HX..."],
  "labels": ["core"],
  "metadata": {},
  "actor": "alice"
}
```

说明：

- `status` 只能是 `triage|todo|scheduled|ready`。
- 若不传 `status`，服务端计算初始状态。
- 显式请求 `scheduled` 时必须同时提供 `scheduled_at`。
- 显式请求 `ready` 时必须有非空描述，且 `scheduled_at` 不能位于未来。
- 若存在未完成依赖（父任务不是 `done` 或 `archived`），`status=ready` 请求仍可接受，
  但依赖守卫会让最终状态保持为 `todo`。
- 无论显式请求 `ready`，还是省略 `status` 后计算出 `ready`，新任务尚无执行计划时
  都会实际以 `todo` 状态落库；响应会把 `execution_plan_state` 派生为 `unplanned`，
  不会为此写入计划行。添加第一个步骤，或通过
  `/execution-plan/not-required` 明确标记无需计划并填写原因后，服务端才会结合规格、
  排期和依赖等其他保护条件重新计算是否进入 `ready`。
- 任务响应会公开派生的依赖和执行计划字段：`dependency_blocked`、`unfinished_parent_count`、
  `execution_plan_state`，以及必需或可选步骤数量。它们是查询元数据，不是可写任务字段。
- `priority` 是整数等级 `0..3`：`0` = P0 事故、阻塞项或必须立即处理，`1` = P1 近期重点，
  `2` = P2 重要后续，`3` = P3 普通待办、低优先级和默认值。创建时会拒绝非法值。
- `labels` 可选。名称会先去除两端空白；空白名称会被拒绝；所有标签必须已存在于当前看板。
  任一标签缺失时，整个创建请求返回 `400 invalid_input`，且不会写入 `tasks`、`labels`、
  `task_labels` 或 `task_events`。创建任务不提供自动创建缺失标签的模式。
- `priority` 默认为 `3`，`labels` 和 `depends_on` 默认为空数组；其他可空字段可显式传入
  `null`。`metadata` 只接受 JSON 对象或 `null`，对象内容是开放扩展，不在传输层解释。
- 路径、请求与 `201` 成功响应分别由 `CreateTaskPath`、`CreateTaskRequest` 和
  `CreateTaskResponse { data: ApiTask }` 拥有。请求状态使用仅限创建的闭合词汇
  `triage|todo|scheduled|ready`；公开响应不包含 `claim_token`。
- 处理器只负责把契约显式映射到应用输入，并继续单次调用
  `create_task_with_labels_and_dependencies`。标签、依赖、重试策略、元数据有效性和初始就绪
  判断仍在同一 SQLite 事务和服务保护中处理；任一失败都会整体回滚。

### 4.3 按状态列出任务窗口

```http
GET /api/v1/boards/{board}/tasks/by-status?status=triage&status=ready&include_archived=false&limit=50&offset=0&sort=-updated_at
```

这个只读端点把看板列查询合并为一次请求，并接受 4.1 节定义的同一套严格查询语法。
每个重复的 `status` 生成独立任务窗口；`limit` 与 `offset` 分别应用到每个窗口。
响应中的状态顺序与 URI 中重复参数的顺序一致；省略 `status` 时返回空的 `statuses` 数组。

响应（以下为字段节选；完整、可消费的成功响应以 `schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json` 为准）：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "statuses": [
      {
        "status": "ready",
        "tasks": [
          {
            "id": "t_01HX...",
            "ref": "default#12",
            "status": "ready",
            "title": "实现状态机"
          }
        ],
        "page": {
          "limit": 50,
          "offset": 0,
          "total": 3
        }
      }
    ]
  },
  "meta": {
    "limit": 50,
    "offset": 0
  }
}
```

### 4.4 获取任务

```http
GET /api/v1/tasks/{task_id}
```

`task_id` 是全局 `t_...` ID，不受看板作用域限制。响应包含 `board_id`、`board_slug`
和 `ref`，便于客户端展示可复制的 `board#seq` 任务引用。

查询参数：

| 参数 | 说明 |
|---|---|
| `include` | 可选。当前识别 `ontology`；可用逗号分隔，其他 include 值暂时保持兼容性忽略。 |

默认响应只包含 `data: ApiTask`，不返回 `meta`。传 `include=ontology` 时，`data`
保持同一 `ApiTask`，并在 `meta.details.ontology_summary` 返回该任务的标签本体信号摘要；
没有本体信号时为 `null`。该摘要是任务级的只读工作流提示，包含信号、状态、降级、过期和
操作数量，最早的开放或已确认信号时间及距今时长，最新信号或操作时间，当前
`suggest_input_hash`，以及最多 5 条示例信号
（ID、种类、状态、提议操作、分数、过期、降级和操作数量）。完整队列和审核仍使用
`/label-ontology/signals`、`/label-ontology/review` 和
`/label-ontology/signals/{signal_id}`。

当前 API 不包含
`GET /api/v1/tasks/{task_id}/detail?include=dependencies,steps,runs,events,comments,neighborhood`
这类任务详情聚合端点，也不包含面板专属时间线。现有的分面板路由和缓存失效行为稳定后，
可再考虑以聚合端点减少 `TaskDetail` 面板的请求扇出。任务执行上下文已有独立的
`GET /api/v1/tasks/{task_id}/context` 端点，见 4.5 节。

### 4.5 获取任务上下文

```http
GET /api/v1/tasks/{task_id}/context?board=default&lexical_limit=5&graph_limit=10&vector_limit=5&max_items=20
```

这是已经采用精确路径、查询、请求头和成功响应契约的只读端点，迁移状态为 `Adopted`。
`task_id` 为必填路径参数；
查询参数均只能出现一次，未知参数会被拒绝：

| 参数 | 是否必填 | 默认值 | 说明 |
|---|---|---:|---|
| `board` | 否 | `default` | 看板 slug 或 ID。 |
| `lexical_limit` | 否 | `5` | 词法检索条目上限，范围 `0..=1000`。 |
| `graph_limit` | 否 | `10` | 图关系条目上限，范围 `0..=1000`。 |
| `vector_limit` | 否 | `5` | 向量检索条目上限，范围 `0..=1000`。 |
| `max_items` | 否 | `20` | 合并后的上下文条目总上限，范围 `1..=1000`。 |

响应的 `data` 包含 `subject`、回显实际限制的 `policy`、合并后的 `items`、
降级原因 `degraded`，以及有诊断时才出现的 `diagnostics`。图或向量后端不可用时，
端点会保留可用来源的结果并通过这些结构化字段说明降级，不会把派生存储当作权威数据源。

### 4.6 更新任务字段

```http
PATCH /api/v1/tasks/{task_id}
```

允许字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "新的标题",
  "description": "新的描述",
  "assignee": "worker-a",
  "priority": 1,
  "scheduled_at": 1717520000000,
  "due_at": 1717600000000,
  "max_retries": 2,
  "metadata": {},
  "actor": "alice",
  "expected_lock_version": 7
}
```

`priority` 更新会拒绝 `0..3` 以外的值。

`max_retries: null` 会清空重试策略。任务 DTO 包含 `execution_plan_state`、
`required_step_count`、`completed_required_step_count` 和 `optional_step_count`，
因此客户端无需另行列出步骤，也能展示执行计划是否就绪。

禁止字段：

- `status`
- `claim_token`
- `current_run_id`
- `completed_at`

`PATCH` 不能直接设置规范 `status`；状态必须通过状态转换端点修改。允许字段仍会走共享服务路径。
更新 `description`、`scheduled_at` 等影响规格或排期的字段后，服务端可以根据规格、排期和
当前依赖重新计算活跃任务的目标状态，并写入对应事件。依赖边必须通过依赖端点修改；
`max_retries` 只更新重试策略，不会触发状态重算。

---

## 5. 状态转换

状态转换请求使用各端点独立的封闭 DTO，未知顶层字段会导致 `400`，不共享通用的转换或
令牌请求体。推进、回收认领、解除阻塞和任务归档可以完全省略请求体；出现请求体时仍按对应
DTO 校验。`actor` 的解析优先级保持为请求体、`X-KB-Actor`、服务端默认值。认领和心跳省略
`ttl_ms` 时使用 `300000`；回收认领、完成、提交审核、阻塞和归档省略 `force` 时均为
`false`，不能绕过租约、令牌或状态机保护。认领令牌不匹配的响应不会回显客户端提交的错误
令牌，也不会暴露服务端保存的真实令牌。

### 5.1 补全规格

```http
POST /api/v1/tasks/{task_id}/transitions/specify
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "description": "补全后的规格",
  "scheduled_at": null,
  "actor": "alice"
}
```

### 5.2 推进

```http
POST /api/v1/tasks/{task_id}/transitions/promote
```

任务执行计划仍为 `unplanned` 时，推进操作会返回 `409 execution_plan_required`。

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "local-worker"
}
```

### 5.3 认领并开始

```http
POST /api/v1/tasks/{task_id}/transitions/claim
```

任务执行计划仍为 `unplanned` 时，认领并开始操作会返回 `409 execution_plan_required`。

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice",
  "ttl_ms": 300000,
  "worker_profile": null,
  "metadata": {}
}
```

省略 `worker_profile` 或传入 `null` 时，运行时会把本次执行记录的工作进程配置记为
`"manual"`。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "status": "running"
    },
    "run": {
      "id": "r_01HX...",
      "status": "running"
    },
    "claim_token": "claim_01HX...",
    "claim_expires_at": 1717520300000
  }
}
```

### 5.4 心跳

```http
POST /api/v1/tasks/{task_id}/transitions/heartbeat
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "claim_01HX...",
  "ttl_ms": 300000,
  "note": "仍在执行",
  "actor": "worker-default"
}
```

显式心跳仍受支持。对于 `running` 任务，后续合法且属于该任务的活动事件也会刷新任务租约和
当前执行记录的心跳，作为隐式存活信号；这种隐式续期不会额外产生 `task.heartbeat` 事件。
看板级事件和不含 `task_id` 的事件不会续期任务。

### 5.5 完成

```http
POST /api/v1/tasks/{task_id}/transitions/complete
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "claim_01HX...",
  "summary": "实现完成，测试通过",
  "result": {},
  "force": false,
  "actor": "worker-default"
}
```

`result` 是可选的不透明 JSON 值；schema 只约束字段存在形式，不收紧其内部结构。

### 5.6 提交审核

```http
POST /api/v1/tasks/{task_id}/transitions/submit-review
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "claim_01HX...",
  "summary": "等待人工检查",
  "force": false,
  "actor": "worker-default"
}
```

提交审核不接受 `result`；该字段与其他未知顶层字段一样会导致 `400`。

### 5.7 阻塞

```http
POST /api/v1/tasks/{task_id}/transitions/block
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "等待 API schema 确认",
  "claim_token": null,
  "force": false,
  "actor": "alice"
}
```

### 5.8 解除阻塞

```http
POST /api/v1/tasks/{task_id}/transitions/unblock
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice"
}
```

响应中的目标状态由服务端计算，不由客户端指定。

### 5.9 重新打开

```http
POST /api/v1/tasks/{task_id}/transitions/reopen
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "重新执行修正验证失败",
  "actor": "alice"
}
```

只允许重新打开 `done` 任务，`reason` 必填且不能为空。响应中的目标状态由服务端按规格、
排期、依赖和执行计划就绪情况重新计算；`completed_at` 会被清空，`result_summary` 和自然
JSON `result` 会保留（持久层仍存于 `result_json`）。`task.reopened` 事件的载荷包含
`from`、`to`、`reason` 和 `original_completed_at`。

直接依赖该任务的子任务中，仅 `triage|todo|scheduled|ready` 会重新计算；
`running|blocked|review|done|archived` 不会被隐式改写。

### 5.10 回收认领

```http
POST /api/v1/tasks/{task_id}/transitions/reclaim
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "force": false,
  "to_status": "ready",
  "reason": "认领已过期",
  "actor": "local-worker"
}
```

`to_status` 是封闭枚举，只接受 `ready` 或 `blocked`；省略时默认为 `ready`，其他任务
状态会导致 `400`。目标为 `blocked` 时必须提供非空 `reason`。领取尚未过期时，只有
`force=true` 才能回收。

### 5.11 归档

```http
POST /api/v1/tasks/{task_id}/transitions/archive
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "force": false,
  "actor": "alice"
}
```

---

## 6. 依赖与执行计划

### 6.1 添加依赖

```http
POST /api/v1/tasks/{child_task_id}/dependencies
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "parent_task_id": "t_01HX...",
  "actor": "alice"
}
```

插入新依赖边时返回 `201 Created`。重复添加同一父子依赖是幂等操作，会以相同的依赖封装
返回 `200 OK`；不会再次写入 `dependency.added` 事件，也不会重复计算子任务状态。
依赖变化可能把不再合法的 `ready` 子任务降为 `todo`，但不会自动把 `todo` 子任务推进到
`ready`。重新打开 `done` 父任务时，仅当直接子任务处于
`triage|todo|scheduled|ready` 才会重新计算；`running|blocked|review|done|archived`
子任务保持不变。

### 6.2 移除依赖

```http
DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}
```

### 6.3 列出依赖

```http
GET /api/v1/tasks/{task_id}/dependencies
```

添加、移除和列出依赖的端点返回同一种依赖封装。在现有线上结构中，`parents` 和
`children` 是完整的 `ApiTask` 数组；额外的 `task` 和 `edges` 字段提供紧凑且已展开的
关系视图，其中父子对象使用稳定命名。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_child",
      "board_id": "b_default",
      "board_slug": "default",
      "ref": "default#2",
      "title": "子任务",
      "status": "todo"
    },
    "parents": [],
    "children": [],
    "edges": [
      {
        "parent": {
          "id": "t_parent",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#1",
          "title": "父任务",
          "status": "done"
        },
        "child": {
          "id": "t_child",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#2",
          "title": "子任务",
          "status": "todo"
        }
      }
    ]
  }
}
```

### 6.4 步骤与执行计划

步骤是归属于任务的有序执行计划项。步骤可以是纯文本，也可以链接一个现有普通任务作为
上下文。链接任务不等于建立依赖边：链接不会影响依赖就绪判断，链接任务的状态也不会自动
完成该步骤。步骤完成状态通过 `todo | done | skipped` 独立跟踪。

```http
GET /api/v1/tasks/{task_id}/steps
POST /api/v1/tasks/{task_id}/steps
PATCH /api/v1/tasks/{task_id}/steps/{step_id}
DELETE /api/v1/tasks/{task_id}/steps/{step_id}
POST /api/v1/tasks/{task_id}/steps/{step_id}/done
POST /api/v1/tasks/{task_id}/steps/{step_id}/skip
POST /api/v1/tasks/{task_id}/steps/{step_id}/reopen
POST /api/v1/tasks/{task_id}/execution-plan/not-required
```

创建请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "编写验收检查",
  "body": "覆盖依赖和执行计划保护",
  "linked_task_ref": "default#13",
  "position": 2048,
  "required": true,
  "actor": "alice"
}
```

`linked_task_ref` 可选；纯文本步骤应省略它。提供时，它必须解析到同一看板上未归档的任务，
且不能指向父任务本身。

更新请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "编写验收检查",
  "body": null,
  "linked_task_ref": "default#14",
  "unlink_task": false,
  "position": 4096,
  "required": false,
  "actor": "alice"
}
```

步骤状态请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "note": "已经实现并验证",
  "actor": "alice"
}
```

`skip` 和 `reopen` 使用相同的封装，但文本字段名为 `reason`。

标记为不需要执行计划的请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "只是一次小型文本整理",
  "actor": "alice"
}
```

步骤列表和变更响应都会返回父任务的步骤快照：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_parent",
    "steps": [
      {
        "id": "step_01HX...",
        "parent_task_id": "t_parent",
        "title": "编写验收检查",
        "body": "覆盖依赖和执行计划保护",
        "linked_task": { "id": "t_child", "ref": "default#13" },
        "position": 2048,
        "required": true,
        "status": "todo",
        "resolution_note": null,
        "resolved_by": null,
        "resolved_at": null,
        "created_by": "alice",
        "created_at": 1717520000000,
        "updated_by": "alice",
        "updated_at": 1717520000000
      }
    ],
    "execution_plan": {
      "board_id": "b_01HX...",
      "task_id": "t_parent",
      "state": "planned",
      "reason": null,
      "updated_by": "system",
      "updated_at": 0
    }
  }
}
```

`POST /execution-plan/not-required` 直接返回执行计划记录。链接目标不存在时返回
`404 not_found`；链接自身、跨看板链接、链接已归档任务或标题为空时，以标准错误封装返回
`400 invalid_input`。完成或归档仍有必需步骤未完成的父任务时返回
`409 steps_incomplete`。对这项保护而言，必需步骤只有在状态为 `done` 或 `skipped`
时才算完成。

### 6.5 任务邻域

```http
GET /api/v1/tasks/{task_id}/neighborhood?depth=1&limit_nodes=250&include_archived_context=false
```

这个只读端点返回选中的任务、直接依赖父任务、直接依赖子任务、直接步骤链接的父子任务，
以及起点和终点都可见的每一条依赖边或步骤边。V1 只接受 `depth=1`；更深的图展开留待以后实现。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "center_task_id": "t_01HX...",
    "nodes": [
      {
        "task": { "id": "t_01HX...", "ref": "default#12", "status": "ready" },
        "role": "center",
        "context_only": false
      }
    ],
    "edges": [
      {
        "id": "dependency:t_parent->t_child",
        "source_task_id": "t_parent",
        "target_task_id": "t_child",
        "kind": "dependency",
        "required": true,
        "blocking": true
      }
    ],
    "meta": {
      "depth": 1,
      "context_depth": 0,
      "node_count": 1,
      "edge_count": 0,
      "truncated": false,
      "limit_nodes": 250,
      "include_archived_context": false
    }
  }
}
```

`task` 与任务列表和详情响应使用相同的公开任务 DTO，不会暴露 `claim_token`。

### 6.6 看板任务图

```http
GET /api/v1/boards/{board}/task-map?active_only=true&context_depth=1&limit_nodes=250&include_done_context=true&include_archived_context=false&hide_isolated=false
```

这个只读端点返回看板的工作关系图。默认包含所有活跃且未归档的任务
（`triage`、`todo`、`scheduled`、`ready`、`running`、`blocked`、`review`），以及最多一跳的
未归档依赖上下文。默认包含 `done` 上下文并标记为 `context_only`；只有显式请求时才包含
已归档上下文。V1 只接受 `context_depth=0` 或 `context_depth=1`。

活跃看板任务的节点角色为 `active`，一跳上下文的角色为 `context`。只有边的两个端点都可见时
才返回依赖边和步骤边。依赖边使用 `kind=dependency`、`required=true` 和 `blocking=true`；
步骤边使用 `kind=step`，保留步骤的 `required` 标记，并设置 `blocking=false`。纯文本步骤没有
任务节点，因此不会出现在图边中。`meta` 对象报告活跃状态、节点数、边数、是否截断、数量上限
和查询中的上下文选项。


---

## 7. 评论

### 7.1 列出评论

```http
GET /api/v1/tasks/{task_id}/comments
```

评论以任务 ID 为作用域。列出评论属于只读审计历史，因此归档看板仍可调用；在归档看板上
创建评论会被拒绝。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "c_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "author": "alice",
      "author_type": "user",
      "agent_type": null,
      "body": "这里需要确认边界条件。",
      "kind": "note",
      "metadata": {},
      "created_at": 1717520000000
    }
  ]
}
```

### 7.2 添加评论

```http
POST /api/v1/tasks/{task_id}/comments
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "body": "这里需要确认边界条件。",
  "kind": "note",
  "author_type": "user",
  "agent_type": null,
  "author": "alice",
  "metadata": {}
}
```

说明：

- `kind` 默认为 `note`，当前允许 `note|decision|signal`。
- `decision` 记录有实际意义的多选项决策；`body` 始终是可读的后备说明，结构化决策数据放在 `metadata` 中。
- `author_type` 标记评论来源，可取 `user|agent`；省略时服务层默认为 `user`。
- 当 `author_type=agent` 时，`agent_type` 是可选的开放文本，例如 `executor` 或 `reviewer`。
  当 `author_type=user` 时提供非空 `agent_type` 会返回 `400 invalid_input`。
- `metadata` 默认为 `{}`，必须是 JSON 对象；响应同样使用自然 JSON `metadata` 对象。
  普通备注或信号的元数据保持开放且无损，不能因为键名与专用协议碰撞而在事务提交后收紧。
  当 `kind=decision` 时必须包含非空 `options`；每个选项必须有非空的 `slug`、`title` 和
  `detail`；slug 必须是唯一的小写 ASCII slug；`selected` 必须匹配某个选项 slug；
  `reason` 必须非空；`risk` 和 `verification` 如果出现也必须非空。无效的决策元数据返回
  `400 invalid_input`。
- `author` 使用通用的操作者语义；也可以使用 `X-KB-Actor` 或服务端默认操作者。
- 创建评论会写入 `task.comment.created` 事件。


### 7.3 评论端点的精确线上契约

`GET` 与 `POST /api/v1/tasks/{task_id}/comments` 各自拥有独立、闭合的路径与成功响应根；
`POST` 另有独立、闭合的请求根。两者只共享契约拥有的 `ApiComment` 组件和既有共享错误组件。
GET 没有查询或请求体，POST 没有查询；两个端点都已登记并采用精确请求头契约，也已具备
真实路由生产者和契约消费者的精确证据，迁移状态为 `Adopted`。

`ApiComment.author_type` 仅允许 `user|agent`，`kind` 仅允许 `note|decision|signal`，
`agent_type` 是必须出现但可为 `null` 的字段。`metadata` 是开放、无损的响应对象。
创建请求中的 `metadata` 保持为开放 JSON 对象；决策的精确强类型结构由独立的
`metadata.decision.input` / `NoTransport` 契约和真实 CLI 生产者与消费者证据拥有。
运行时原始 JSON 对象继续进入 SQLite 服务层的决策跨字段保护；模式文件不能替代
选中项与选项唯一性、slug、非空值、看板归档以及事务和事件约束。

---

## 8. 执行记录

### 8.1 列出任务执行记录

```http
GET /api/v1/tasks/{task_id}/runs
```

执行记录列表以任务 ID 为作用域，并作为只读审计历史继续对已归档看板开放。

### 8.2 获取执行记录

```http
GET /api/v1/runs/{run_id}
```

### 8.3 获取执行日志

```http
GET /api/v1/runs/{run_id}/log
```

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "run_id": "r_01HX...",
    "content": "执行器输出\n",
    "truncated": false
  }
}
```

说明：

- 响应不包含 `claim_token`。
- 当前最多返回日志末尾 256 KiB；更大的日志会设置 `truncated: true`。
- 若执行记录没有 `log_path` 或文件不存在，返回 `not_found`。
- 若 `log_path` 不在受信任日志目录或文件名不匹配 `<run_id>.log`，返回 `invalid_input`。

### 8.4 读取契约

列表与详情端点分别拥有闭合的路径和成功响应契约，只共享由契约定义的 `ApiRun`。
执行状态是闭合枚举：`running|succeeded|failed|canceled|expired`。
`worker_profile`、`worker_pid`、`finished_at`、`exit_code`、`summary` 和 `error`
都必须出现，但值可以为 `null`。`claim_token` 只出现在显式认领转换的响应中，不进入
执行记录列表或详情；SQLite `log_path` 只供独立日志端点解析受信任文件，也不进入执行记录
列表或详情。上述读取端点均已采用精确契约。

---

## 9. 统计

### 9.1 队列统计

```http
GET /api/v1/stats?board=default
```

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "board_id": "b_01HX...",
    "generated_at": 1717520000000,
    "status_counts": [
      {"status": "ready", "count": 3},
      {"status": "running", "count": 1}
    ],
    "stale_claims": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "title": "执行器已失联",
        "claim_owner": "local-worker",
        "claim_expires_at": 1717520000000,
        "last_heartbeat_at": 1717519900000,
        "current_run_id": "r_01HX...",
        "retry_count": 1,
        "max_retries": 3
      }
    ],
    "blocked_reasons": [
      {"reason": "等待操作人员处理", "count": 2}
    ],
    "unplanned_active_tasks": 4,
    "active_parents_with_incomplete_required_steps": 1
  }
}
```

说明：

- `stale_claims` 只包含 `running` 且 `claim_expires_at <= now` 的任务。
- `blocked_reasons` 按数量降序、reason 升序排序。

---

## 10. 事件

### 10.1 列出事件

```http
GET /api/v1/events?board=default&after=0&limit=100
```

`board` 接受看板 slug 或 ID。归档看板的事件仍可读取，便于客户端检查归档后的审计轨迹。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": 123,
      "event_id": "e_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "run_id": "r_01HX...",
      "kind": "task.claimed",
      "actor": "alice",
      "payload": {"claim_owner":"alice","metadata":{}},
      "created_at": 1717520000000
    }
  ],
  "meta": {
    "next_after": 123
  }
}
```

### 10.2 SSE 事件流

```http
GET /api/v1/stream/events?board=default&after=123
```

SSE 事件：

```text
event: task.claimed
id: 124
data: {"id":124,"event_id":"e_...","board_id":"b_...","task_id":"t_...","run_id":"r_...","kind":"task.claimed","actor":"alice","payload":{"claim_owner":"alice","metadata":{}},"created_at":1717520000000}
```

`board`、`task_id`、`after`、`limit` 是该端点唯一接受的查询键，均只能出现一次；
未知或重复键返回标准 `400 invalid_input` 封装。默认值分别为 `default`、未提供、`0` 和
`100`，运行时会把 `limit` 防御性限制到 `1000`。每个事件严格按 `event`、`id`、`data`
帧顺序输出；`data` 是完整的 `StreamEventData` JSON，不允许额外字段。
`task_id`、`run_id`、`actor` 都是必须存在但可为空：键必须出现，值可以显式为 `null`。
39 个已知事件种类的载荷与种类使用同一个带标签联合；字段缺失、出现额外字段或同级状态
错配时会失败关闭。未来未知事件种类的合法 JSON 载荷保持无损。

重新连接：

- V1 实现会发送当前匹配事件的有限快照后关闭连接；客户端应重新连接，或轮询 `GET /api/v1/events` 获取更新。
- 浏览器客户端可以发送 `Last-Event-ID`，但 V1 只处理 `after` 查询参数。
- V1 有限快照不发送 SSE 注释或心跳帧；因此心跳不是 JSON 载荷契约，`Last-Event-ID`
  也不是已采用的请求头输入契约。这两项只有未来运行时真正实现后才能迁移为强类型契约。
- 若事件已被压缩或清理，客户端应重新获取看板快照。

---

## 11. 看板列与界面设置

### 11.1 列出看板列

```http
GET /api/v1/boards/{board}/columns
```

当前只开放读取接口，服务端没有看板列更新路由，因此暂时不能通过 HTTP API 修改看板列。
返回的列状态仍对应规范任务状态；调用方不得把读取接口推断为可写配置接口。
读取端点的路径、请求头和成功响应精确契约均已采用。

---

## 12. 标签 API

```http
GET /api/v1/boards/{board}/labels
POST /api/v1/boards/{board}/labels
GET /api/v1/boards/{board}/labels/semantics
GET /api/v1/boards/{board}/labels/{label_id}/semantics
PUT /api/v1/boards/{board}/labels/{label_id}/semantics
DELETE /api/v1/boards/{board}/labels/{label_id}/semantics?expected_semantics_hash=<hash>&reason=<text>
GET /api/v1/boards/{board}/labels/atoms
GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain
GET /api/v1/boards/{board}/labels/atom-index/status
POST /api/v1/boards/{board}/labels/atom-index/rebuild
GET /api/v1/boards/{board}/labels/atom-index/query?q=<text>&polarity=positive&limit=24
GET /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels/bootstrap
DELETE /api/v1/tasks/{task_id}/labels/{label_id}
GET /api/v1/boards/{board}/signals
GET /api/v1/boards/{board}/signals/review
GET /api/v1/signals/{signal_id}
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals
GET /api/v1/boards/{board}/label-ontology/review
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/revert
POST /api/v1/boards/{board}/label-ontology/validate
```

看板级标签创建请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "core",
  "color": "blue"
}
```

标签响应结构，用于看板级标签创建和标签列表：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "id": "l_01HX...",
  "board_id": "b_01HX...",
  "name": "core",
  "color": "blue",
  "created_at": 1717520000000,
  "updated_at": 1717520000000
}
```

`POST /api/v1/boards/{board}/labels` 按看板作用域创建标签，并按标签
名称保持幂等。如果该看板上已存在同名标签，响应返回已有标签。空白 `name`
会被拒绝。基础标签标识的增删改查属于词汇表注册表，不属于本体台账；
创建标签标识不会写入 `label_ontology_actions`，也不会创建
`label_semantics` 或 `label_atoms`。

任务标签添加请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "core"
}
```

或批量添加：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "names": ["core", "api"]
}
```

如果需要在绑定时显式创建缺失的标签标识：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "names": ["scratch-label"],
  "create_missing": true
}
```

`POST /api/v1/tasks/{task_id}/labels` 会把 `name` 或 `names` 指定的标签绑定到任务。
`name` 与 `names` 互斥；二者都缺失、二者同时出现或 `names` 为空数组都会返回
`invalid_input`。批量添加在同一事务内执行，并先验证所有标签名称；如果
任一标签为空白或非法，不会创建规范标签，也不会留下部分任务标签绑定。
默认情况下，如果该任务所属看板上还不存在指定名称的标签，请求会返回
`invalid_input`，且不会增加 `labels` 或 `task_labels` 记录。传入
`"create_missing": true` 时，API 只会创建缺失的规范标签标识并绑定到
任务；不会生成 `label_semantics` 或 `label_atoms`。重复绑定已有任务标签关系不会
重复写入。成功响应返回更新后的任务及当前 `labels` 列表；显式创建模式下若
本次创建了标签，响应中的 `meta.created_labels` 会列出新标签。

任务标签引导创建请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "database",
  "description": "数据库持久化工作",
  "applies_when": ["涉及 SQLite 迁移"],
  "excludes_when": ["仅调整界面样式"],
  "positive_examples": ["新增数据表迁移"],
  "negative_examples": ["只修改 CSS"],
  "actor": "alice"
}
```

`POST /api/v1/tasks/{task_id}/labels/bootstrap` 是一次性采用新标签的 API：
它会在同一事务内创建任务所属看板上缺失的规范标签，或复用尚无既有语义的同名标签，
写入该标签的 `label_semantics`，同步重建 SQLite `label_atoms`，标记派生的标签原子
向量索引为脏，并把该标签绑定到任务。`name` 按标签名称解析；空白名称会被拒绝。
语义输入会去除两端空白并丢弃空白值，且必须至少
提供 `description` 或一个非空语义数组值。

引导创建 API 默认不会覆盖已有的 `label_semantics`。如果同名标签已经有语义，
请求会失败，并要求调用方改用专用语义变更或提议与采用路径；只有目标标签仍无语义时，
重复调用同一任务和标签才会保持任务标签绑定幂等。成功响应状态为 `201 Created`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "ref": "default#12",
      "labels": [
        {"id": "l_01HX...", "board_id": "b_01HX...", "name": "database", "color": null}
      ]
    },
    "semantics": {
      "label_id": "l_01HX...",
      "board_id": "b_01HX...",
      "label_name": "database",
      "description": "数据库持久化工作",
      "applies_when": ["涉及 SQLite 迁移"],
      "excludes_when": ["仅调整界面样式"],
      "positive_examples": ["新增数据表迁移"],
      "negative_examples": ["只修改 CSS"],
      "atoms": []
    }
  }
}
```

HTTP 引导创建不包含 CLI `--verify` 的编排：请求体没有向量配置、最低分数或验证标记，
响应也没有 `verification` 字段。该端点不会替调用方重建标签原子向量索引、运行
`label suggest` 或检查分数门槛；需要提交前分阶段验证且失败时零写入的语义时，应使用
CLI `label bootstrap --verify`。API 调用后如需诊断，可显式执行索引重建、建议和审核流程，
但它不具备 CLI 分阶段验证器的同一事务采用契约。

`DELETE /api/v1/tasks/{task_id}/labels/{label_id}` 会移除任务上的指定标签，
`{label_id}` 接受标签 ID 或标签名称。成功响应同样返回更新后的任务及当前 `labels`
列表。只有关联行发生变化时，标签绑定或移除才会写入任务标签事件；该操作不改变任务状态。

### 12.1 标签语义、原子与原子索引

`GET /api/v1/boards/{board}/labels/semantics` 返回当前看板上已定义语义的列表。
`GET /api/v1/boards/{board}/labels/{label_id}/semantics` 返回单个标签的语义；
`{label_id}` 只接受规范的 `l_...` 标签 ID。标签名称允许包含 `/` 等不适合放入路径的字符，
因此语义 API 的路径不支持按标签名称寻址；需要按名称查找时，应先调用
`GET /api/v1/boards/{board}/labels` 获取对应 ID。

`PUT /api/v1/boards/{board}/labels/{label_id}/semantics` 写入已有标签的语义字典，
同步重建该标签的 SQLite `label_atoms`，并标记派生的标签原子向量索引为脏。
请求体：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice",
  "expected_semantics_hash": "optional-current-hash",
  "replace": false,
  "reason": "补充标签审核中反复出现的边界",
  "source_signal_ids": ["los_..."],
  "description": "后端服务工作",
  "applies_when": ["涉及 Rust 服务代码"],
  "excludes_when": ["仅修改 CSS"],
  "positive_examples": ["新增 API 处理器"],
  "negative_examples": ["调整界面间距"],
  "remove_applies_when": [],
  "remove_excludes_when": [],
  "remove_positive_examples": [],
  "remove_negative_examples": []
}
```

默认 `replace=false`，请求按补丁语义处理：`description` 只在提供非空值时覆盖当前描述，
数组字段会追加到对应集合，`remove_*` 数组删除匹配文本；省略字段不会清空已有语义。
传入 `replace=true` 时才完整替换五个语义字段，此时省略的数组视为空数组，并且不能同时传入
任何 `remove_*` 字段。`expected_semantics_hash` 是 CAS 保护条件；如果与当前
`semantics_hash` 不一致，请求返回冲突且不写入。服务会去除两端空白并丢弃空白值。
每次实际改变规范语义或原子的建设性写入，都会在同一 SQLite 事务中写入一条
`update_semantics` 根本体操作，记录操作者、原因、来源信号链接（如有）、前后哈希和一份
变更快照；实际新增或移除的原子通过 `label_ontology_action_atom_effects` 写成
`added` / `removed` 行。仅修改描述的补丁会写一条根操作和零条原子效果；无变化补丁不会写
操作或效果，也不会标记标签原子索引为脏。生成原子时，有描述的标签会生成一个规范
`description` 原子：`label: {name}\ndescription: {description}`；没有描述时才使用
`name` 后备原子。原子文本还会规范化空白：折叠每个非空行内部的空白，但保留规范换行。
同一标签下 `polarity + kind + normalized_text` 相同的原子会去重并保留第一次出现的
`ordinal`；`id` 和 `content_hash` 不包含 `ordinal`，因此仅调整数组顺序不会改变同一文本
原子的标识。响应使用 `DataEnvelope`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "label_id": "l_01HX...",
    "board_id": "b_01HX...",
    "label_name": "backend",
    "description": "后端服务工作",
    "applies_when": ["涉及 Rust 服务代码"],
    "excludes_when": ["仅修改 CSS"],
    "positive_examples": ["新增 API 处理器"],
    "negative_examples": ["调整界面间距"],
    "created_at": 1717520000000,
    "updated_at": 1717520000000,
    "atoms": [
      {
        "id": "la_...",
        "label_id": "l_01HX...",
        "board_id": "b_01HX...",
        "label_name": "backend",
        "polarity": "positive",
        "kind": "applies_when",
        "text": "涉及 Rust 服务代码",
        "ordinal": 2,
        "content_hash": "...",
        "created_at": 1717520000000,
        "updated_at": 1717520000000
      }
    ]
  }
}
```

`DELETE /api/v1/boards/{board}/labels/{label_id}/semantics?expected_semantics_hash=<hash>&reason=<text>`
是受 CAS 保护的语义清除操作：`expected_semantics_hash` 与非空 `reason` 都必填。
它删除该标签的语义与 SQLite 原子，但不删除规范标签标识或任务标签绑定；同一事务会写入一条
`update_semantics` 根本体操作，变更后快照为空，并为实际移除的原子写入 `removed` 效果，
随后标记标签原子索引为脏。哈希不匹配时，规范数据、操作、效果和脏状态均保持不变。成功返回：

```http
DELETE /api/v1/boards/default/labels/l_01HX/semantics?expected_semantics_hash=sem_abc123&reason=%E5%81%9C%E7%94%A8%E8%BF%87%E6%9C%9F%E8%AF%AD%E4%B9%89
X-KB-Actor: alice
```

<!-- schema-doc: contract=api.label-semantics-delete.response fixture=schemas/fixtures/api/delete-response.v1.valid.json -->
```json
{ "data": { "deleted": true } }
```

`GET /api/v1/boards/{board}/labels/atoms` 返回 SQLite `label_atoms` 的物化投影。
它由 `label_semantics` 和标签名称展开，并随语义变更在同一事务内重建；它是
`lancedb_label_atoms` 派生索引的输入，不能把它描述成独立于语义的第二份语义事实源。

`GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain` 按当前原子 ID 或稳定的
`content_hash` 解析原子，并返回 `LabelAtomExplainRecord`：`query`、`atom`、
`current_semantics`、`provenance_actions`、`supporting_signals`、
`validation_history`、`legacy_untracked` 和 `legacy_reason`。当前原子存在但没有
本体来源操作引用其 ID 或内容哈希时，返回 `200` 且 `legacy_untracked=true`；
未知 ID 或哈希返回 `not_found`。

`GET /api/v1/boards/{board}/labels/atom-index/status` 返回标签原子向量索引状态。
服务端的轻量路由通过向量辅助程序适配器报告当前能力。没有向量提供方、适配器不可用或辅助程序
缺失时，仍返回 `200` 和禁用状态。JSON 保留兼容字段 `message`，并额外返回结构化的
`diagnostics: string[]`、`dirty: boolean | null`、`board_dirty: boolean | null`；调用方应使用
结构化字段判断脏状态和错误，不要解析 `message` 文案。相同的 `VectorStoreStatus` 结构也用于
`/api/v1/vector/status`。

`POST /api/v1/boards/{board}/labels/atom-index/rebuild` 通过向量辅助程序适配器调用
标签原子专用的 `rebuild-label-atoms` 命令，重建 `lancedb_label_atoms` 派生索引并更新
`label_atom_index_boards` / `lancedb_label_atoms` 状态。辅助程序或提供方缺失时返回明确的
API 错误，不得写入规范标签事实，也不得把分块存储状态当成标签原子重建成功。
`GET /api/v1/boards/{board}/labels/atom-index/query` 通过向量辅助程序适配器查询派生的
`lancedb_label_atoms` 索引。请求必须提供 `q=<text>` 或 `vector_json=<json-array>` 之一，二者互斥；
`embedding_model` 可选，`include_vector=true` 可要求原始向量命中返回向量，`polarity` 可选且只接受
`positive` / `negative`，`limit` 默认 24。命中项中的 `distance` 是 LanceDB `_distance`，不是
求解器相似度分数。未配置提供方、适配器或辅助程序不可用，或者向量存储不可用时，查询返回明确的
API 错误，且不修改 SQLite 事实。

### 12.2 任务标签建议

```http
GET /api/v1/tasks/{task_id}/labels/suggestions?limit=5&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15
```

返回任务级标签建议。若部署中有可用的标签原子向量存储，服务会使用任务标题和描述的嵌入
查询 `lancedb_label_atoms`：正向原子按残差进行多轮检索，负向原子固定使用原始查询检索并
施加惩罚或抑制。求解器在标签组层执行 Group OMP 选择，再把选中标签的高分正向原子向量
作为基底进行非负重新拟合。`coverage` 和 `residual_norm` 来自原子级拟合向量，其中
`coverage = clamp(1 - residual_norm, 0.0, 1.0)`，因此二者不是两份独立证据；
`coverage_cosine` 是原始查询与拟合向量的余弦相似度，可作为独立补充指标。候选标签只有在
试探性重新拟合后带来足够的残差范数降幅，才会进入结果；覆盖率或残差范数达到停止阈值后，
求解器会提前停止，而不是凑满 `max_selected_labels`。候选组与已选标签的语义向量过度相似时
会被跳过，以减少语义重复的标签同时出现在 `selected_labels`；这不会合并或删除规范标签。
`needs_new_label` 是兼容字段，只表示存在需要人工审核的标签覆盖率诊断；具体原因必须读取
`reason_codes`，并结合证据原子、诊断信息和人工语义判断，不能只凭该布尔值创建新词汇。
接口不会创建新标签，也不会写入 `label_semantics` 或 `label_atoms`。

`limit` 只控制响应中 `selected_labels` 和 `candidates` 的最大条数，不会收窄求解器内部
搜索能力。内部能力由 `candidate_limit`、`atom_limit` 和 `max_selected_labels` 分别控制：
候选标签组数量、每轮原子向量检索上限，以及最多进入非负重新拟合的标签数量。所有数量上限都必须是
`1..=1000`；`min_score` 必须在 `0..=1`。

未配置提供方、标签向量适配器或功能不可用、LanceDB 表缺失、索引为空或索引为脏时，
接口仍返回 `200` 和结构化的降级 JSON；普通标签增删改查、任务列表、搜索、筛选和状态转换
不受影响。脏状态判断来自结构化状态和 SQLite 的 `dirty` 字段，不依赖 `message` 文案。
没有提供方时 `needs_new_label=false`，避免误触发自动创建新标签的流程。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_01HX...",
    "board_id": "b_01HX...",
    "selected_labels": [
      {
        "label_id": "l_01HX...",
        "label_name": "backend",
        "score": 0.82,
        "weight": 0.82,
        "already_applied": false,
        "evidence_atoms": [
          {
            "atom_id": "la_...",
            "label_id": "l_01HX...",
            "label_name": "backend",
            "polarity": "positive",
            "kind": "applies_when",
            "text": "涉及服务端代码",
            "score": 0.91
          }
        ],
        "negative_evidence_atoms": []
      }
    ],
    "candidates": [],
    "coverage": 0.82,
    "coverage_cosine": 0.91,
    "residual_norm": 0.18,
    "needs_new_label": false,
    "reason_codes": ["degraded_result", "label_atom_index_dirty"],
    "degraded": true,
    "diagnostics": ["label_atom_index_dirty"]
  }
}
```

稳定的 `diagnostics` 包括：

- `vector_store_disabled`
- `label_atom_index_dirty`
- `label_atom_index_empty`
- `label_atom_index_error`
- `vector_query_error`

非降级覆盖率审核的稳定 `reason_codes` 包括：

- `no_selected_labels`
- `coverage_below_threshold`
- `residual_above_threshold`
- `unexplained_residual`

### 12.3 标签语义提议

```http
POST /api/v1/tasks/{task_id}/label-proposals?limit=5&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15
GET /api/v1/tasks/{task_id}/label-proposals
GET /api/v1/label-proposals/{proposal_id}
POST /api/v1/label-proposals/{proposal_id}/accept
POST /api/v1/label-proposals/{proposal_id}/reject
```

`POST /api/v1/tasks/{task_id}/label-proposals` 创建一次新的标签提议尝试。
请求体可为空或仅包含 `actor`；此时默认提供方不可用，接口返回 `200`
和降级的尝试结果，不创建规范标签、`label_semantics`、`label_atoms` 或
`task_labels`。

提供方边界：API 当前只支持空的默认提供方，或请求体中显式传入的本地离线候选。
真实 LLM 提供方不在 `kanban-sqlite` 中实现；如果未来服务端支持本机 AI 运行时，
必须在服务端、本地层或独立 AI crate 层实现 `LabelProposalProvider` 适配器，
并把候选交给 SQLite 服务层做确定性校验和持久化。

带本地离线提供方输出时：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "proposal": {
    "name": "database",
    "description": "数据库持久化工作",
    "applies_when": ["涉及 SQLite 迁移"],
    "excludes_when": ["仅调整界面样式"],
    "positive_examples": ["新增数据表迁移"],
    "negative_examples": ["只修改 CSS"]
  },
  "actor": "alice",
  "source_signal_ids": ["los_..."],
  "ontology_actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "allow_retarget": false,
  "retarget_reason": null
}
```

数组字段省略时按空数组处理。服务先读取当前标签建议的启发式 `coverage`、
`coverage_cosine`、`residual_norm` 和现有标签第一名。覆盖率充足时不写提议；
覆盖率不足、候选语义有效，且残差第一名加间隔校验明确通过时，返回 `201` 并持久化
状态为 `proposed` 的提议。候选与现有标签发生规范化名称冲突时，会以 `rejected` 状态
持久化，`diagnostics` 包含 `near_duplicate_label_conflict`。规范化名称冲突忽略大小写、
空白和标点，是确定性的近似重复启发式。

`source_signal_ids` 可选；传入时，提议创建成功后会在同一事务中写入
`create_label_proposal` 本体操作，并通过操作与信号的链接记录哪些已确认的词汇缺口信号
支持该提议。提议行与来源操作要么同时写入，要么一起回滚。来源信号默认必须属于同一看板、
状态为 `confirmed`、种类为 `vocabulary_gap`、`proposed_action` 为 `bootstrap_label`，
且规范化后的 `proposed_label_name` 等于提议名称。`ontology_actor` 只控制
`create_label_proposal` 操作的来源；省略时使用 `actor` 字符串作为 `type=user` 的操作者。
确需重定向同一看板上已确认的来源信号时，必须传入 `allow_retarget=true` 和非空
`retarget_reason`；原因和来源信号原始的目标或提议标签会写入
`change_json.retarget_override`。重定向不会放宽看板和状态要求。

POST 提议路由接受与标签建议相同的查询参数。`limit` 只截断建议输出；
`candidate_limit`、`atom_limit`、`max_selected_labels` 和 `min_score` 用于调节底层求解器的
启发式覆盖率和残差校验。

服务端配置了可用的向量提供方时，提议尝试与标签建议使用同一套 LanceDB 标签原子存储。
覆盖率不足的候选会在持久化前执行残差第一名加间隔校验：候选语义的残差分数和现有标签
第一名都根据返回的原子向量在本地计算余弦相似度，不从 LanceDB 距离推导；候选必须超过
现有标签第一名，且差值达到固定间隔。校验失败时，候选仍会以 `rejected` 提议持久化，
`diagnostics`
包含 `label_proposal_residual_top1_failed` 或
`label_proposal_residual_margin_insufficient`。未配置提供方、功能不可用或向量检索失败时
返回降级尝试，不创建规范标签、`label_semantics`、`label_atoms` 或 `task_labels`。
如果残差校验不可用或已降级，且没有明确通过第一名加间隔校验，本次尝试返回
`proposal=null`，不新增提议行；`diagnostics` 包含
`label_proposal_residual_validation_unavailable` 和具体原因。

尝试响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_...",
    "board_id": "b_...",
    "proposal": null,
    "degraded": true,
    "diagnostics": ["label_proposal_provider_unavailable", "vector_store_disabled"],
    "heuristic_coverage": 0.0,
    "heuristic_coverage_cosine": 0.0,
    "heuristic_residual_norm": 1.0,
    "top1_existing_label_id": null,
    "top1_existing_label_name": null
  }
}
```

接受或拒绝的请求体：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "覆盖率不足，接受新标签",
  "actor": "alice",
  "source_signal_ids": ["los_..."],
  "ontology_actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "allow_retarget": false,
  "retarget_reason": null
}
```

接受操作只允许状态为 `proposed` 的提议。成功后会通过与任务标签引导创建相同的采用原语，
创建规范 `labels` 行以及对应的 `label_semantics` 和 `label_atoms`，标记标签原子索引为脏，
并在同一事务中写入一条 `bootstrap_label` 根本体操作和对应的新增原子效果。提议状态、
规范写入和来源操作要么一起成功，要么一起回滚。它不会自动写入 `task_labels`。
`source_signal_ids` 可选；省略时仍会记录引导创建操作，但没有操作与信号的链接。传入时，
接受操作会通过这些链接记录新标签的引导创建来源。来源信号必须属于同一看板且处于
`confirmed`。`actor` 字符串仍用于提议决策事件；`ontology_actor` 只控制接受操作产生的
`bootstrap_label` 本体操作来源。省略 `ontology_actor` 时，引导创建操作使用 `actor`
字符串作为 `type=user` 的操作者。`type=agent` 必须提供非空 `agent_type`；
`type=user` 不能提供 `agent_type`。来源信号默认还必须是
`vocabulary_gap` 加 `bootstrap_label`，且规范化后的 `proposed_label_name` 必须等于提议名称。
确需重定向同一看板上已确认的来源信号时，必须传入 `allow_retarget=true` 和非空
`retarget_reason`；引导创建操作中的 `change_json.retarget_override` 会记录原因、来源信号
原始目标或提议标签，以及最终提议或结果标签。如果提议已有 `create_label_proposal` 操作，
接受操作产生的 `bootstrap_label` 操作会把 `parent_action_id` 指向该创建操作。重定向不会
放宽看板和状态要求。拒绝操作把提议标记为 `rejected`，不接受 `source_signal_ids`、
`ontology_actor` 或重定向选项。对已接受或已拒绝的提议再次决策，会返回标准
`400 invalid_input` 错误封装。

### 12.4 通用信号台账

通用信号台账 API 提供按看板划分的只读收件箱，用于展示代理或产品在看板工作流中记录的
通用信号，例如 CLI 参数摩擦、提示误导、参数设计问题或操作人员发现。它独立于标签本体台账；
这些端点不会创建、确认、拒绝、解决或取代信号，也不会把通用信号混入本体审核分组。

```http
GET /api/v1/boards/{board}/signals?status=open&kind=agent_cli_friction&task_ref=default%23123&include_all=false&limit=100
GET /api/v1/boards/{board}/signals/review?status=confirmed&kind=agent_cli_friction&task_ref=default%23123&include_all=false&limit=100
GET /api/v1/signals/{signal_id}
```

`GET /api/v1/boards/{board}/signals` 和 `/signals/review` 返回同一只读 DTO；
`review` 端点是桌面端或操作人员控制台的语义化入口。默认只返回 `open`
和 `confirmed`。可重复传 `status` 和 `kind`，并按 `task_ref` 过滤。
`include_all=true` 且没有显式 `status` 时返回完整历史；`limit` 使用普通列表上限。
这些列表和审核路由以看板为作用域，只返回该看板的信号行。
`GET /api/v1/signals/{signal_id}` 是操作人员全局详情查询，用于从反向链接、收件箱行或审计
记录直接打开已知信号。该详情路由不会改变信号的 `board_id` 事实，也不会让按看板划分的
列表或审核接口泄漏其他看板的信号。

`signal_observations.task_id`、`run_id` 和 `comment_id` 是来源与历史的软引用。
当前服务写入路径、诊断命令和导入最终门禁会维护这些引用与观察记录所属看板的一致性；
未来若需要硬化所有来源关系，可迁移为按看板组合的外键。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "sig_...",
      "board_id": "b_...",
      "observation_id": "obs_...",
      "kind": "agent_cli_friction",
      "title": "--require 参数命名不符合 agent 惯用预期",
      "summary": "代理尝试使用 --required/--requires，实际 CLI 只接受 --require。",
      "severity": "medium",
      "status": "open",
      "dedupe_key": "kanban-task-create-require",
      "superseded_by_signal_id": null,
      "reviewed_by": null,
      "reviewed_at": null,
      "review_reason": null,
      "created_at": 1782930000000,
      "updated_at": 1782930000000,
      "observation": {
        "id": "obs_...",
        "board_id": "b_...",
        "task_id": "t_...",
        "task_ref_snapshot": "default#123",
        "run_id": "r_...",
        "comment_id": null,
        "actor": "local-agent",
        "agent_type": "automation",
        "source": "cli-hook",
        "evidence": {"command":"kanban task create --required ..."},
        "created_at": 1782930000000
      }
    }
  ]
}
```

`GET /api/v1/signals/{signal_id}` 返回单条 signal：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "id": "sig_...",
    "observation": {}
  }
}
```

### 12.5 标签本体台账

标签本体台账 API 记录任务标注过程、审核队列、本体变更来源和验证历史。台账不会自动修改
任务标签；规范绑定仍通过任务标签 API 或 CLI 完成。

所有本体操作者对象使用 `{ "name": string, "type": "user"|"agent",
"agent_type": string|null }`。`type=agent` 必须提供非空 `agent_type`；
`type=user` 必须省略或传 `null`。

```http
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals?status=open&kind=false_negative&task_ref=default%2312&target_label_ref=cli&proposed_label_name=database&include_all=false&limit=100
GET /api/v1/boards/{board}/label-ontology/review?group_by=label&include_all=false&limit=100
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/revert
POST /api/v1/boards/{board}/label-ontology/validate
```

`POST /api/v1/tasks/{task_id}/label-ontology/observations` 在一个事务中写入观察记录和
子信号。HTTP 端点不会自行运行 `label suggest`；调用方必须传入由工具采集且未改写的
`suggestion_snapshot`，或在没有建议证据时显式传入空快照。服务端会从快照派生观察指标，
代理或审核者只提交候选、最终判断、信号、候选原子和理由。请求体：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
  "agent_candidates": [],
  "suggestion_snapshot": {
    "selected_labels": [],
    "coverage": 0.61,
    "coverage_cosine": 0.74,
    "residual_norm": 0.39,
    "needs_new_label": false,
    "degraded": false,
    "diagnostics": []
  },
  "final_decision": {},
  "capture_fingerprint": "optional-stable-key",
  "signals": [
    {
      "kind": "false_negative",
      "target_label_ref": "cli",
      "related_labels": [],
      "proposed_action": "add_positive_atom",
      "candidate_atom": {
        "polarity": "positive",
        "kind": "applies_when",
      "text": "扩展 CLI 子命令、命令参数、帮助输出或机器可读的 JSON 行为"
      },
      "proposal": {},
      "agent_selected": true,
      "suggest_state": "candidate",
      "suggest_score": 0.08,
      "suggest_rank": 6,
      "final_selected": true,
      "rationale": "该任务扩展了 CLI 接口。",
      "confidence": 0.9
    }
  ]
}
```

面向新客户端的 HTTP 本体 DTO 使用自然 JSON 字段：`agent_candidates`、
`suggestion_snapshot`、`final_decision`、`diagnostics`，信号中的 `related_labels` 和
`proposal`，操作中的 `change` 和 `validation`，以及验证请求中的 `validation`。
公开 HTTP API 不再接受旧的转义字符串同级请求字段，例如 `related_labels_json`、
`proposal_json`、`change_json`、`validation_json` 和观察记录中的 `*_json` 别名。
出现未知旧字段时会以 `400 invalid_input` 失败关闭；客户端必须发送自然 JSON 字段。
当 `suggestion_snapshot` 包含 `coverage`、`coverage_cosine`、`residual_norm`、
`needs_new_label`、`degraded` 或 `diagnostics` 时，服务端会从快照派生持久化的观察指标。
如果请求同时提供对应的顶层 `suggest_*` 字段或 `diagnostics`，且值发生冲突，则返回
`400 invalid_input`。新客户端不应在顶层重复快照事实。

服务会读取当前任务快照、解析 `target_label_ref`，并计算规范化的提议标签名称、信号键和
候选原子内容哈希。`capture_fingerprint` 为空时会根据任务、快照和信号派生；同一看板上的
重复指纹会被唯一约束拒绝。观察响应返回新建观察记录并展开子 `signals`。观察记录包含用于
完整审计的 `task_snapshot_json.content_hash`，以及只基于标签建议输入
（规范化标题加描述）的 `suggest_input_hash`；后者用于后续验证的可比性判断。

信号输入会在写入前接受本体契约校验。`candidate_atom` 中，`applies_when` 和
`positive_example` 只能使用 `positive` 极性，`excludes_when` 和 `negative_example`
只能使用 `negative` 极性。`add_positive_atom` 必须提供目标标签和正向候选原子；
`add_negative_atom` 必须提供目标标签和负向候选原子；`update_semantics` 必须提供目标标签；
`bootstrap_label` 必须提供 `proposed_label_name`；`rename_label` 必须提供目标标签和
`proposed_label_name`；`split_label` 和 `merge_labels` 必须提供目标标签及非空的
`related_labels`。观察指标 `suggest_coverage`、`suggest_coverage_cosine`、
`suggest_residual_norm` 以及信号指标 `suggest_score` 和 `confidence` 必须是有限的
`0.0..=1.0`；`suggest_rank` 必须为 `null` 或 `>= 1`。违反这些契约的请求返回
`400 invalid_input`，不会写入观察记录或信号。`rename_label`、`split_label` 和
`merge_labels` 当前只作为审核信号的提议操作保存，不能通过公开 HTTP 路由写入规范结构
变更操作；旧的结构计划行只读展示为不受支持的验证要求。

`GET /api/v1/boards/{board}/label-ontology/signals` 默认只返回 `open` 和
`confirmed`。可重复传 `status` 和 `kind`，并按 `task_ref`、`target_label_ref`、
`proposed_label_name`、`include_all`、`limit` 过滤。

`GET /api/v1/boards/{board}/label-ontology/review` 返回只读聚合审核队列。
`group_by` 支持 `label`、`candidate_atom`、`proposed_label`，以及需要显式启用的 `cluster`，
默认 `label`；`include_all=false` 默认只聚合 `open` 和
`confirmed` 信号，`true` 时包含完整历史；`limit` 限制分组数量。响应
`meta` 回显 `group_by`、`include_all` 和 `limit`。每个分组包含：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "group_by": "label",
  "key": "lab_...",
  "label_id": "lab_...",
  "label_name": "cli",
  "candidate_atom_polarity": "positive",
  "candidate_atom_kind": "applies_when",
  "candidate_text": "扩展 CLI 子命令",
  "candidate_content_hash": "14ada47e4b0566c5",
  "proposed_label_name": null,
  "proposed_label_name_normalized": null,
  "cluster_key": null,
  "cluster_reason": null,
  "task_count": 2,
  "signal_count": 3,
  "open_count": 2,
  "confirmed_count": 1,
  "resolved_count": 0,
  "rejected_count": 0,
  "superseded_count": 0,
  "degraded_count": 1,
  "average_score": 0.31,
  "median_score": 0.28,
  "oldest_signal_at": 1781780000000,
  "latest_signal_at": 1781780100000,
  "sample_task_refs": ["default#12"],
  "signal_ids": ["los_..."],
  "action_count": 1,
  "action_ids": ["loa_..."],
  "proposal_ids": [],
  "labels": [{"id": "lab_...", "name": "cli"}],
  "candidate_atom_variants": [
    {
      "content_hash": "14ada47e4b0566c5",
      "polarity": "positive",
      "kind": "applies_when",
      "text": "扩展 CLI 子命令",
      "signal_count": 2
    }
  ]
}
```

分组依次按去重后的 `task_count` 降序、`confirmed_count` 降序、
`latest_signal_at` 降序和 `key` 升序排列。`group_by=cluster` 是可禁用的只读辅助视图：
默认不会启用，不写规范原子，不确认、应用、验证或关闭信号，也不会创建新的 SQLite 事实表。
聚类键会在每次请求时根据已有信号文本重建：优先使用词法规范化后的候选文本，其次使用提议
标签，再其次使用理由，最后退回到种类、操作、目标和提议标签作用域的组合。所有聚类键都带有
信号种类、提议操作、目标标签和提议标签作用域，避免跨标签、操作或边界误合并；
`cluster_reason` 说明键的来源。`GET /api/v1/label-ontology/signals/{signal_id}`
返回：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "signal": {},
    "observation": {},
    "actions": []
  }
}
```

`POST /api/v1/boards/{board}/label-ontology/actions` 写入审核或生命周期操作：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "alice", "type": "user", "agent_type": null},
  "action_type": "confirm",
  "signal_ids": ["los_..."],
  "reason": "在多个独立 CLI 任务中观察到",
  "superseded_by_signal_id": null,
  "parent_action_id": null,
  "target_label_ref": null,
  "result_label_ref": null,
  "result_atom_id": null,
  "result_atom_content_hash": null,
  "result_proposal_id": null,
  "canonical_before_hash": null,
  "canonical_after_hash": null,
  "validation_requirement": null,
  "validation_status": null,
  "validation_effective_outcome": null
}
```

该公共操作端点只接受生命周期操作类型：`confirm`、`reject`、`supersede` 和
`resolve_no_change`，并会同步更新来源信号状态。请求中的
`parent_action_id`、`target_label_ref`、结果字段、规范哈希、`change`、
`validation_requirement`、`validation_status`、
`validation_effective_outcome` 和 `validation` 必须为
`null` 或省略；否则返回 `invalid_input`。`add_positive_atom`、`add_negative_atom`、
`adopt_existing_atom`、`update_semantics`、`create_label_proposal`、`bootstrap_label`、
`revert_ontology_mutation`、`validate` 等变更或验证操作类型，不允许通过该通用端点写入；
规范变更的来源必须由语义 PUT、应用原子、创建或接受提议、任务标签引导创建或验证等专用
路由在同一事务内写入。写入 `supersede` 时会沿替代关系的 `superseded_by_signal_id`
链检查；若链路回到任一来源信号，或替代链本身已有环，则返回 `invalid_input`，不会写入新的
取代操作。

`POST /api/v1/boards/{board}/label-ontology/apply/atom` 对已有标签执行
读取、修改并更新语义，并写入原子来源操作：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "signal_ids": ["los_1", "los_2"],
  "label_ref": "cli",
  "kind": "applies_when",
  "text": "扩展 CLI 子命令、命令参数、帮助输出或机器可读的 JSON 行为",
  "reason": "多个 CLI 接口任务反复出现假阴性信号",
  "allow_retarget": false,
  "retarget_reason": null
}
```

来源信号必须属于同一看板且已 `confirmed`。`kind` 只接受
`applies_when`、`positive_example`、`excludes_when`、`negative_example`。如果规范内容
实际新增了原子，成功后返回 `add_positive_atom` 或 `add_negative_atom` 操作，记录结果
原子的软引用、内容哈希、变更前后规范哈希、一份变更快照和一个 `added` 原子效果，并把
`validation_requirement` 设为 `required`。如果相同内容的原子已经存在，成功后返回
仅记录来源的 `adopt_existing_atom` 操作，记录现有原子的软引用、相同的前后规范哈希和
来源信号链接；该操作不修改语义或原子，不标记原子索引为脏，
`validation_requirement=none`，有效结果为 `not_required`。默认要求所有带
`target_label_id` 的来源信号都指向 `label_ref`；不匹配时返回 `400 invalid_input`
并列出违规信号 ID。审核者可以泛化原子文本，不要求它等于来源信号中的候选文本。
确需重定向同一看板上已确认的信号时，必须传入 `allow_retarget=true` 和非空
`retarget_reason`；操作中的 `change_json.retarget_override` 会记录原因、来源信号原始
目标或提议标签，以及最终目标标签。重定向不会放宽看板和状态要求。只有实际新增规范原子时，
该路由才会标记标签原子索引为脏；向量重建和建议验证在事务外执行。

`POST /api/v1/boards/{board}/label-ontology/revert` 追加可追溯的回滚操作，并把目标标签语义
恢复为被撤销变更操作的变更前快照：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "reviewer", "type": "user", "agent_type": null},
  "target_action_id": "loa_...",
  "expected_current_hash": "optional-current-semantics-hash",
  "reason": "回滚仅用于测试的原子变更"
}
```

当前只支持 `add_positive_atom`、`add_negative_atom` 和 `update_semantics`。路由要求当前
规范语义哈希仍等于 `target_action_id` 的 `canonical_after_hash`；
`expected_current_hash` 非空时还必须等于当前哈希。成功后返回
`revert_ontology_mutation` 操作：`parent_action_id` 指向被撤销操作，来源信号链接从目标
操作复制，`change` 记录被撤销操作、回滚前后快照和 `index_dirty=true`，并为本次回滚实际
新增或移除的原子写入原子效果，随后标记标签原子索引为脏。该操作的
`validation_requirement` 为 `unsupported`，可以记录外部失败或部分诊断，但不会被当作可由
可信验证通过的待验证项。该路由不会删除或修改原操作，也不处理引导创建的标签标识或任务
绑定回滚；CLI 分阶段引导验证的失败路径会在提交前保持零写入，不再依赖提交后的恢复流程。

`POST /api/v1/boards/{board}/label-ontology/validate` 追加外部证明验证操作。HTTP 路由接收
调用方提交的自然 JSON `validation`，但当前不会运行向量重建、索引查询或 `label suggest`，
因此不能产生可信自动化的 `passed`。需要可信自动化验证时，应使用 CLI
`label ontology validate --trusted`，由工具采集索引和建议证据后写入。

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "parent_action_id": "loa_...",
  "signal_ids": ["los_1", "los_2"],
  "reason": "重建原子后，来源任务仍未选中目标标签",
  "validation_status": "failed",
  "validation": {
    "evidence_type": "external_attestation",
    "reviewer": "codex",
    "cases": [
      {
        "signal_id": "los_1",
        "case_type": "positive_atom",
        "passed": false,
        "before": {
          "target": {"label_id": "l_cli", "selected": false, "score": 0.12},
          "coverage": 0.61
        },
        "after": {
          "degraded": false,
          "target": {"label_id": "l_cli", "selected": false, "score": 0.14},
          "coverage": 0.60,
          "notes": "人工审核持久化的建议输出后，结果未达到通过标准"
        }
      }
    ]
  }
}
```

服务会把调用方提供的 `validation` 包入验证封装，并附上来源信号案例、观察时任务快照或
建议输入哈希与当前任务哈希的对比、父操作结果引用和摘要。公开的提供或采集载荷只保存在
顶层 `manual`；生成的 `cases[]` 通过 `after.manual_case_ref` 指向
`manual.cases[]` 中对应信号的原始证据，避免多信号验证把同一载荷重复存入每个案例。
`parent_action_id` 必须指向同一看板上 `validation_requirement=required` 的规范变更操作，
且父操作必须带有规范结果证据，例如原子、结果标签或提议引用、规范哈希和非空变更快照。
HTTP 提供的 JSON 属于外部证明；它可以保存 `failed` 或 `partial` 诊断，但
`validation_status="passed"` 会返回 `invalid_input`，因为验证通过需要工具采集的
`trusted_automated` 证据。`unsupported` 的父操作可以记录外部失败或部分诊断，但不能通过。
结构化字段或字符串 `"automated"` 本身不构成可信来源。

可信自动化验证的持久化载荷由 CLI 采集器生成，而不是由 HTTP 调用方手写：顶层包含
`evidence_type="trusted_automated"`、`collector.source`、非空 `embedding_model`、
对象 `solver_options`、干净的 `index.status`、`index.generation`，以及覆盖每个已链接来源
信号的 `cases[]`。CLI 采集器在较长的 SQLite 事务之外重建原子索引并运行建议；写入操作时，
服务会在短事务中重新核验父操作、来源信号、变更后规范哈希、索引脏或错误状态和代次。
“可信”表示证据由工具采集、当前哈希与索引代次一致，并在指定案例和对照上机械通过；
它不是全局语义正确性的证明。

强类型策略按父操作检查：

- `add_positive_atom`：`case_type="positive_atom"`，`after.degraded=false`；
  `after.evidence_atoms[]` 必须包含父操作的 `result_atom_id` 或
  `result_atom_content_hash`；目标标签必须已选中或分数不低于 0.50；
  分数和覆盖率不能比变更前恶化。
- `add_negative_atom`：`case_type="negative_atom"`，`after.evidence_atoms[]`
  不用于结果负向原子校验；父操作的结果原子必须出现在
  `after.negative_evidence_atoms[]`。假阳性任务必须证明
  `after.target.selected=false`，或变更前后分数都存在且变更后分数低于变更前分数。
  必须提供至少一个 `after.positive_controls[]`，且每个对照都已通过且未退化；
  若没有正向对照，必须提供带非空原因的
  `after.positive_control_waiver`。
- `bootstrap_label`：`case_type="bootstrap_label"`，所有已链接来源信号都必须有通过的
  案例；新标签或结果标签必须已选中或分数不低于 0.50；证据原子必须来自结果标签。

验证可比性默认使用观察记录的 `suggest_input_hash`。状态、`updated_at`、`lock_version`
或任务标签绑定只改变完整快照时，会写入 `task_metadata_drift` 或
`label_binding_drift` 警告，不会让已通过的验证过期。标题或描述变化会写入
`suggest_input_drift` 并使案例不可比较；旧观察记录缺少 `suggest_input_hash` 时写入
`legacy_suggest_input_hash_missing`，不能静默通过。`passed` 会把已链接来源信号转为
`resolved`；`failed` 和 `partial` 会保留信号，供后续修正或人工处理。

---

## 13. 搜索

### 13.1 搜索任务

```http
GET /api/v1/search/tasks?board=default&q=needle&status=ready&label=backend&assignee=worker-a&include_archived=false&limit=20&offset=0
```

默认的 CLI 和服务端构建启用 `tantivy-backend`。SQLite 数据库旁存在 `index/v1/tasks/` 时，
搜索使用 Tantivy 任务索引。Tantivy 索引缺失、损坏或过期，或者二进制显式使用
`--no-default-features` 构建时，会回落到 SQLite，并附带过期元数据。搜索会匹配任务标题、
描述、评论、执行摘要或错误，以及事件种类和载荷。

`label` 按标签名称或 ID 过滤，可重复，并在评分和分页前使用 AND 语义。
带标签过滤的搜索即使存在可用的 Tantivy 索引，也会使用 SQLite 后备路径，
以确保结果反映当前任务标签关联行。

任务引用形状的 `q` 始终使用 SQLite 精确匹配语义，即使当前存在可用的 Tantivy 索引：
纯数字 `12` 和 `#12` 匹配请求看板内的序号；`board#12` 和 `board/#12`
只在显式看板等于请求看板时匹配；`t_...` 只匹配请求看板内的任务 ID。
任务引用形状的查询不会从标题、描述、评论、执行记录或事件中返回模糊匹配。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "hits": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "score": 60.0,
        "snippet": "就绪任务的规格命中内容",
        "task": {
          "id": "t_01HX...",
          "seq": 12,
          "status": "ready",
          "title": "实现状态机"
        }
      }
    ],
    "meta": {
      "backend": "sqlite",
      "stale": false,
      "index_version": null,
      "last_event_id": 42,
      "index_lag_events": 0
    }
  },
  "meta": {
    "limit": 20,
    "offset": 0
  }
}
```

任务变更不会在 SQLite 事务内写入 Tantivy。使用 `tantivy-backend` 运行 `kanban serve` 时，
后台循环会在启动后立即尝试一次 `sync_search_index`，随后默认每
`--search-sync-interval-ms` 毫秒同步一次（默认 `5000`；`0` 表示禁用）。普通任务变更后仍可
手动运行 `kanban index sync`；`kanban index rebuild` 会替换派生索引。Tantivy 状态按看板
存放在 `app_settings` 的 `search.tasks.state.<board_id>` 下，并可随现有导出与导入往返。

### 13.2 按状态搜索任务窗口

```http
GET /api/v1/search/tasks/by-status?board=default&q=needle&status=ready&status=review&include_archived=false&limit=50&offset=0
```

这个只读端点把看板上的多列搜索合并为一个请求。它接受与
`GET /api/v1/search/tasks` 相同的查询文本、看板、标签、执行者、归档和分页参数，
但会为每个重复的 `status` 返回独立搜索窗口。`limit` 和 `offset` 分别作用于每个状态窗口。
响应中的状态顺序与查询参数顺序一致；省略 `status` 时返回空的 `statuses` 数组。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "statuses": [
      {
        "status": "ready",
        "tasks": [
          {
            "id": "t_01HX...",
            "ref": "default#12",
            "status": "ready",
            "title": "实现状态机"
          }
        ],
        "search_meta": {
          "backend": "sqlite",
          "stale": false,
          "index_version": null,
          "last_event_id": 42,
          "index_lag_events": 0
        },
        "page": {
          "limit": 50,
          "offset": 0,
          "total": null
        }
      }
    ]
  },
  "meta": {
    "limit": 50,
    "offset": 0
  }
}
```

### 13.3 搜索索引状态

```http
GET /api/v1/search/status?board=default
```

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "backend": "sqlite",
    "derived_index": false,
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0,
    "message": "SQLite fallback search is active; no derived index exists yet"
  }
}
```

当前的 `MAX(task_events.id)` 大于 Tantivy 持久化的 `last_event_id` 时，
`stale=true`，`index_lag_events` 会报告高水位差值。后台同步被禁用、延迟或失败时，
搜索仍会返回当前 SQLite 后备结果和过期元数据，而不会信任已经落后的派生索引。

---

## 14. 图与向量派生能力

本节三个只读端点均已采用精确查询、请求头和成功响应契约，迁移状态均为 `Adopted`。
SQLite 仍是事实源；图和向量后端是可重建的派生能力。

### 14.1 图后端状态

```http
GET /api/v1/graph/status?board=default
```

`board` 可选，默认为 `default`。响应的 `data` 包含 `backend`、`enabled` 和
供人阅读的 `message`。辅助程序缺失等可降级状态仍以 `200` 返回，并设置
`enabled=false`。

### 14.2 查询图邻居

```http
GET /api/v1/graph/neighbors?board=default&entity_uri=kb%3A%2F%2Ftask%2Ft_example&predicate=depends_on&limit=50
```

| 参数 | 是否必填 | 默认值 | 说明 |
|---|---|---:|---|
| `board` | 否 | `default` | 看板 slug 或 ID。 |
| `entity_uri` | 是 | 无 | 要查询的实体 URI。 |
| `predicate` | 否 | 无 | 可选的关系谓词过滤，取值见下文。 |
| `limit` | 否 | `50` | 返回关系数量上限，范围 `0..=1000`。 |

`predicate` 只接受 `belongs_to_board`、`belongs_to_task`、`depends_on`、`produced_by`、
`generated_by`、`references_artifact`、`related_to`、`uses_skill`、`uses_context`、
`derived_from`、`supersedes`、`similar_to`、`requires_review` 或 `waiting_for_user`。

响应以 `data` 数组返回关系记录，以 `meta.limit` 回显实际数量上限。每条关系包含
`subject_uri`、`predicate`、`object_uri`、`graph_uri`、来源信息、开放的 `metadata`，
以及创建和更新时间。

### 14.3 向量后端状态

```http
GET /api/v1/vector/status?board=default
```

`board` 可选，默认为 `default`。响应的 `data` 包含 `backend`、`enabled`、`message`、
`diagnostics`、必需但可为 `null` 的 `dirty` 和 `board_dirty`，以及有值时才出现的
`generation`。辅助程序缺失或输出不可用时会通过结构化状态说明降级。

---

## 15. 维护

### 15.1 诊断

```http
POST /api/v1/maintenance/doctor
```

响应包含 SQLite 完整性、迁移或用户版本、已过期的运行中任务、孤立执行记录检查、依赖环数量、
已归档依赖边数量、缺失或可疑执行日志数量、依赖、规格和排期违规的可执行状态不变量统计、
基础关系一致性诊断、标签本体台账诊断和知识底座诊断。已归档父任务指向活跃子任务的边属于
允许保留的历史依赖边；活跃父任务指向已归档子任务的边会被计数。

基础关系诊断是只读的：

- `consistency_errors` 和 `consistency_warnings` 汇总基础关系行的看板一致性发现。
- `consistency_issues[]` 以 `severity`、`code`、`message` 和 `record_ids` 报告结构化问题。
- 覆盖的表包括 `task_labels`、`task_dependencies`、`task_steps`、`task_execution_plans`、
  `task_runs`、`task_comments`、`signal_observations`、`signals`、`task_events` 和
  `task_attachments`。
- v24 及以上数据库要求通用信号台账具备 `signal_observations` 和 `signals`。
- 硬错误表示某行的 `board_id` 与它引用的任务、标签、执行记录、评论或观察记录所属看板不同。
  消息包含 `table`、`row`、`row_board`、`referenced` 和 `referenced_board`。
- v25 及以上数据库为 `signals.observation_id` 和 `signals.superseded_by_signal_id`
  添加按看板组合的外键。
- 这些检查补充服务层按看板划分的写入保护。当前结构中，`task_labels`、`task_dependencies`、
  `task_steps`、`task_execution_plans`、`task_runs`、`task_comments`、`signals` 和
  `task_attachments` 受按看板组合的外键保护。`signal_observations` 和 `task_events`
  保留可空的来源引用；诊断和导入流程仍会检查这些看板关系，作为损坏 JSONL 或原始 SQL
  输入的硬错误诊断层。
- `PRAGMA foreign_key_check` 的结果以硬错误 `consistency_issues[]` 呈现，并包含表名、
  行 ID、父表和外键索引。导入会在提交前运行同一门禁，出现违规时回滚。
- `consistency_errors` 非零时，`ok=false`。

本体台账诊断是只读的：

- `ontology_ledger_errors` 和 `ontology_ledger_warnings` 汇总硬错误和警告。
- `ontology_ledger_issues[]` 以 `severity`、`code`、`message` 和 `record_ids` 报告结构化问题。
- v12 及以上数据库要求具备 `label_ontology_observations`、`label_ontology_signals`、
  `label_ontology_actions`、`label_ontology_action_atom_effects` 和
  `label_ontology_action_signals`。
- 硬错误包括跨看板本体链接、孤立的操作信号或操作效果链接、缺失父级或取代引用、
  标签、提议或任务的看板不一致、信号取代环和操作父级环。错误非零时 `ok=false`。
- 警告只用于可重建或可由历史解释的软引用，例如某操作的 `result_atom_id` 所指向的当前
  `label_atoms` 行已在重建中消失。

派生层诊断是只读的：

- `outbox_pending`、`outbox_running` 和 `outbox_failed` 汇总 `index_outbox`。
- `derived_dirty_stores` 统计 `dirty=true` 的存储。
- `derived_error_stores` 统计存在 `last_error` 或发件箱失败项的存储。
- `derived_stores[]` 报告每个存储的 `store_name`、`schema_version`、`last_event_id`、
  `dirty`、`last_error`，以及该存储目标的待处理、运行中和失败发件箱数量。

`derived_stores[].last_event_id` 是存储级成功事件水位，不是看板本地水位。`dirty=true`
表示该存储在某个看板上仍有未完成的发件箱项，或最近一次更新失败。按看板同步或重建可以推进
水位；如果其他看板仍有待处理或失败工作，存储会继续保持脏状态。

这些字段不会让 Tantivy、Oxigraph 或 LanceDB 成为权威数据源。SQLite 仍是事实源，
脏的派生存储仍是可重建缓存。

### 15.2 检查点

```http
POST /api/v1/maintenance/checkpoint
```

运行 `PRAGMA wal_checkpoint(TRUNCATE)`，并返回 `busy`、`log_frames` 和
`checkpointed_frames`。

### 15.3 备份

当前不提供 HTTP 备份；请使用 CLI 备份命令。

---

## 16. Web 界面交互规则

1. 拖拽列时调用状态转换端点。
2. 普通字段编辑调用 `PATCH /tasks/{id}`。
3. Web 界面不显示 `claim_token`，调试模式除外。
4. 对 `running` 任务执行完成或阻塞操作时，若没有令牌，界面使用 `force=true` 并要求确认。
5. `blocked` 任务解除阻塞后的目标列由服务端返回，前端不要预设。
6. SSE 收到事件后，优先重新获取受影响任务，避免客户端状态机漂移。

### 16.1 信号评论

API 评论 DTO 使用 `kind: "signal"` 表示信号台账的反向链接评论。服务生成的反向链接会在
自然 JSON `metadata` 对象中包含 `type:"signal_link"`、`signal_id`、`observation_id`、
`signal_kind` 和 `signal_status`。通用信号评论的元数据保持开放且无损；客户端应把正文
作为可读的后备内容，只有完整的反向链接结构存在时才可链接到信号详情。


## 附录 A. 传输目录实现说明

本规范所列的每个 API 或 SSE 方法与路径，都以 `kanban-contract` 的端点描述目录作为唯一
实现来源。注册处理器时使用稳定的 `operation_id` 与 `adapter_id`；两者分别表示公开端点和
服务端运行时绑定，不是 Rust 类型名、函数地址或由 `stringify!` 推导出的值。


---

# 文件：docs/SCHEMA_CONTRACTS.md

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


---

# 文件：docs/ADR.md

# 架构决策记录

本文件按时间记录 SPEC 的关键架构决策。每条 ADR 的背景、统计值和迁移状态都是
决策时快照，不会随着实现自动改写；当前行为和实时契约覆盖以对应的
`docs/*_SPEC.md` 与 `docs/SCHEMA_CONTRACTS.md` 为准。

---

## ADR-0001：仅使用 SQLite

### 状态

已接受

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

已接受

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
- complete
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

已接受

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

已接受

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

已接受

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
- `label delete` 永不隐式删除 `label_semantics` / `label_atoms`；force 只允许移除 task
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

已接受

### 背景

Agent/Product 的故障和观察需要一个持久的审查生命周期；它不能局限于 label，也不能只是自由格式的评论元数据。

### 决策

新增 board 范围的 `signal_observations` 和 `signals` 表。`kanban signal record` 写入 observation 与 signal 记录；存在 task 上下文时，还会在同一 SQLite 事务中写入简短的 `task_comments.kind = signal` 回链。生命周期审查支持 `open -> confirmed|rejected|superseded|resolved` 和 `confirmed -> resolved`；supersede 要求替代 signal 与原 signal 属于同一 board，并防止成环。V1 不会自动创建后续任务。

### 影响

Signal 账本成为通用 agent/product 信号的权威存储。Label ontology 账本仍然只服务于 label，不复用于通用产品信号。


## ADR-0016：API/SSE 传输描述符作为唯一 method/path 权威

### 状态

已接受

### 背景

此前 `SurfaceOperation` 与 router 各自手写 method/path；即使已有一致性测试，仍存在双写漂移面。

### 决策

在 `kanban-contract` 默认 feature 中保存 API/SSE 描述符；server router 以
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

已接受

### 背景

两个 task-read 端点需要证明各自精确消费 path/query，同时避免 handler 或多个 parser
重复拥有 raw query。

### 决策

`GET /api/v1/boards/:board/tasks` 与 `/tasks/by-status` 分别拥有独立 path/query DTO，
  形成 4 个 `Adopted` 精确 contract。两个 server 本地类型化 Axum extractor 分别绑定对应
  `Path<...>`，并各自从 `parts.uri.query()` 读取一次 raw URI 后进入共享有序解析器；handler
  只接收已绑定的 request，不持有 `RawQuery`、`Query<T>` 或第二个 raw source。
- Query 语法：只有 `status`、`priority`、`label`、`plan_filter` 是
  `RepeatedOrdered`；其余标量重复、未知 key 与旧 `search` 别名均返回
  `400 invalid_input`。54 对上限由 9/4/3/32 个重复参数预算加 6 个标量参数推导；
  raw query 上限为 8192 字节。`q` 是唯一文本搜索 key。label 会 trim Unicode 边缘空白，
  但纯 Unicode 空白失败关闭；percent/UTF-8、枚举、priority、limit、offset 和 sort 边界由
  真实 router URI 矩阵固定。
每个 contract 都有独立的 DTO-to-fixture producer 和 fixture-to-real-router consumer；
  非默认 board 哨兵证明真实 path 消费。AST 测试锁定 DTO 所有权、类型化
  extractor、两个 raw URI 消费点及 handler `&path.board` 到 `list_tasks_page` 的实参，并以显式
  变异覆盖别名、私有 DTO、错误 extractor、双重来源、第二个 raw parser，以及两个
  handler 各自的 `path.board -> default`。producer/consumer 区域保护只证明当前源码区域直接
  分离，不把任意未来共同 helper 间接层夸大为变异完备证明。

### 影响

Desktop/Web/CLI 的 HTTP 调用方必须使用上述语法；现有 Desktop 调用方已使用 `q`
  并保留重复参数顺序。SQLite service 的防御性上限直接引用唯一 application 权威，
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

行为细节以 [API_SPEC](docs/API_SPEC.md#4-任务) 和
[SCHEMA_CONTRACTS](docs/SCHEMA_CONTRACTS.md#2-契约状态) 为准。

## ADR-0021：Oxigraph quick-xml 安全临时 vendor patch

### 状态

已接受（第 2 阶段临时例外）

### 背景

`oxrdfxml 0.2.3` 与 `sparesults 0.3.3` 的 crates.io 版本仍解析到受
RUSTSEC-2026-0194/RUSTSEC-2026-0195 影响的 `quick-xml < 0.41`；仓库当前通过 root
`Cargo.toml` 与对应的 `vendor/` 目录使用上游修复源码，并统一到 `quick-xml 0.41.0`。

### 决策

允许根目录 `Cargo.toml` 中唯一的 `[patch.crates-io]` 例外，且仅接受 `oxrdfxml`/`sparesults` 两个精确仓内 vendor 路径、package name/version 与普通文件目标。`schema_dependency_policy` 对额外 key、非精确 source/path、path traversal、symlink、全部 `[replace]` 保持失败关闭；schema-tool 注册表闭包不变，产品依赖图继续禁止 schema tooling 泄漏。

由安全负责人维护，待 crates.io 上游版本发布并确认 `quick-xml >= 0.41` 后移除 vendor、`[patch]`、lockfile 变更及本 ADR；advisory、provenance 或 vendor digest 变化必须重新审查。复核期限：2026-10-12。


---

# 文件：migrations/001_initial.sql

```sql
-- Kanban Tool initial SQLite schema
-- Time convention: INTEGER unix epoch milliseconds UTC.
-- JSON convention: TEXT with CHECK(json_valid(...)).

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 120000;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;

BEGIN;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL DEFAULT '',
  applied_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS boards (
  id TEXT PRIMARY KEY CHECK(id LIKE 'b_%'),
  slug TEXT NOT NULL UNIQUE CHECK(length(trim(slug)) > 0),
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  description TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER
);

CREATE TABLE IF NOT EXISTS board_columns (
  id TEXT PRIMARY KEY CHECK(id LIKE 'col_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  position INTEGER NOT NULL,
  hidden INTEGER NOT NULL DEFAULT 0 CHECK(hidden IN (0, 1)),
  wip_limit INTEGER CHECK(wip_limit IS NULL OR wip_limit >= 0),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, status),
  UNIQUE(board_id, position)
);

CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY CHECK(id LIKE 't_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL,

  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  description TEXT,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  status_reason TEXT,

  assignee TEXT,
  priority INTEGER NOT NULL DEFAULT 0,
  position INTEGER NOT NULL DEFAULT 0,

  scheduled_at INTEGER,
  due_at INTEGER,

  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  started_at INTEGER,
  completed_at INTEGER,
  archived_at INTEGER,

  claim_token TEXT,
  claim_owner TEXT,
  claim_expires_at INTEGER,
  last_heartbeat_at INTEGER,
  current_run_id TEXT,

  retry_count INTEGER NOT NULL DEFAULT 0 CHECK(retry_count >= 0),
  max_retries INTEGER CHECK(max_retries IS NULL OR max_retries >= 0),

  result_summary TEXT,
  result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),

  lock_version INTEGER NOT NULL DEFAULT 0 CHECK(lock_version >= 0),

  UNIQUE(board_id, seq),
  UNIQUE(id, board_id),
  CHECK(
    (status != 'running') OR
    (claim_token IS NOT NULL AND claim_owner IS NOT NULL AND claim_expires_at IS NOT NULL)
  )
);

CREATE TABLE IF NOT EXISTS task_dependencies (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  child_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(child_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_runs (
  id TEXT PRIMARY KEY CHECK(id LIKE 'r_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,

  status TEXT NOT NULL CHECK(status IN ('running', 'succeeded', 'failed', 'canceled', 'expired')),
  worker_profile TEXT,
  worker_pid INTEGER,

  claim_token TEXT NOT NULL,
  claim_owner TEXT NOT NULL,
  claim_expires_at INTEGER NOT NULL,

  started_at INTEGER NOT NULL,
  last_heartbeat_at INTEGER,
  finished_at INTEGER,

  exit_code INTEGER,
  summary TEXT,
  error TEXT,
  log_path TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),

  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_comments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author TEXT NOT NULL,
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'text' CHECK(kind IN ('text', 'system', 'worker', 'decision')),
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK(event_id LIKE 'e_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  actor TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_attachments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'a_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  filename TEXT NOT NULL CHECK(length(trim(filename)) > 0),
  rel_path TEXT NOT NULL CHECK(length(trim(rel_path)) > 0),
  content_type TEXT,
  size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
  sha256 TEXT,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS labels (
  id TEXT PRIMARY KEY CHECK(id LIKE 'l_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  color TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, name),
  UNIQUE(id, board_id)
);

CREATE TABLE IF NOT EXISTS task_labels (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(task_id, label_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL CHECK(json_valid(value_json)),
  updated_at INTEGER NOT NULL
);

-- Indexes: tasks
CREATE INDEX IF NOT EXISTS idx_tasks_board_status_position
  ON tasks(board_id, status, position);

CREATE INDEX IF NOT EXISTS idx_tasks_board_priority_created
  ON tasks(board_id, priority DESC, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_tasks_assignee_status
  ON tasks(board_id, assignee, status);

CREATE INDEX IF NOT EXISTS idx_tasks_scheduled
  ON tasks(board_id, status, scheduled_at);

CREATE INDEX IF NOT EXISTS idx_tasks_claim_expiry
  ON tasks(board_id, status, claim_expires_at);

CREATE INDEX IF NOT EXISTS idx_tasks_updated
  ON tasks(board_id, updated_at DESC);

-- Indexes: dependencies
CREATE INDEX IF NOT EXISTS idx_deps_child
  ON task_dependencies(child_task_id);

CREATE INDEX IF NOT EXISTS idx_deps_parent
  ON task_dependencies(parent_task_id);

-- Indexes: runs
CREATE INDEX IF NOT EXISTS idx_runs_task_started
  ON task_runs(task_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_runs_status
  ON task_runs(board_id, status, started_at DESC);

-- Indexes: comments
CREATE INDEX IF NOT EXISTS idx_comments_task_created
  ON task_comments(task_id, created_at ASC);

-- Indexes: events
CREATE INDEX IF NOT EXISTS idx_events_board_id
  ON task_events(board_id, id ASC);

CREATE INDEX IF NOT EXISTS idx_events_task_created
  ON task_events(task_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_events_kind_created
  ON task_events(kind, created_at DESC);

-- Indexes: labels
CREATE INDEX IF NOT EXISTS idx_task_labels_label
  ON task_labels(label_id, task_id);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (1, '001_initial', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
```


---

# 文件：migrations/003_comment_author_identity.sql

```sql
-- Add explicit comment author identity while preserving existing kind values.

BEGIN;

ALTER TABLE task_comments
  ADD COLUMN author_type TEXT NOT NULL DEFAULT 'human'
  CHECK(author_type IN ('human', 'agent', 'system'));

UPDATE task_comments
SET author_type = CASE kind
  WHEN 'worker' THEN 'agent'
  WHEN 'system' THEN 'system'
  ELSE 'human'
END
WHERE author_type = 'human';

ALTER TABLE task_comments
  ADD COLUMN agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (3, '003_comment_author_identity', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
```

# Kanban Tool 产品规范

文档类型：当前实现规范

Kanban Tool 是本地优先、单用户的看板与 durable work queue。任务事实只保存在 canonical Turso 数据库中；CLI、MCP、Desktop 和可选 dispatcher 共享同一个 application service、状态机、事务和错误语义。

## 1. 固定执行路径

```text
CLI / MCP / Desktop
        ↓ typed localhost HTTP
      kanban serve
        ↓
ApplicationService + state machine
        ↓
   kanban-store-turso
        ↓
   canonical kanban.db
```

硬性规则：

1. 只有 `kanban serve` 可以打开、初始化和关闭 Turso 数据库。
2. CLI、MCP、Desktop 只能依赖 `kanban-client`（或 Desktop 的 TS HTTP client），不得依赖 `kanban-store-turso`、SQLite 或任何 DB-owning crate。
3. 所有 mutation 必须进入同一组 typed application command；adapter 只解析输入、调用 client、映射结果或渲染输出。
4. 不存在“server 运行时走 RPC、server 停止时直开数据库”的 fallback。
5. 本轮不引入自定义 IPC、runtime protocol、capability catalog、通用 receipt 或第二 backend。

Host 默认监听 `http://127.0.0.1:8721`，默认数据库为 `~/.local/share/kb/kanban.db`。只有
`kanban serve` 会打开、初始化或迁移该数据库；`config show`、`init`、`board use/current`、
completion 和 Codex hook 是不触库的本地 shell 命令。它们可以解析 `--db`/`KANBAN_DB`，但
不会因为解析路径而创建数据库；其他 domain 命令只接受 `--server-url`/`KANBAN_SERVER_URL`
并通过 localhost HTTP 访问 host。

## 2. 产品目标与非目标

### 2.1 目标

- 将 task、plan、依赖、评论、steps、runs 和 events 持久化为可重启恢复的 canonical 数据。
- 由同一状态机保护状态转换、原子 claim、lease/heartbeat 和 run/event 一致性。
- 让 CLI、MCP、Desktop 对同一 task 立即看到相同结果。
- 保持本地单用户语义；`actor` 只用于审计，不承担鉴权。

### 2.2 非目标

以下能力不属于当前 canonical path：

- 多用户、团队、邀请、RBAC、多租户、SaaS、云同步或远程 worker；
- SQLite/Turso 双 backend、旧 SQLite importer、旧 API 完整 parity；
- `kanban-runtime*`、framed IPC、named pipe、跨版本握手、capability negotiation、mutation receipt 和 crash matrix；
- 自动 server supervision、系统服务注册、Windows Job Object、`multiprocess_wal`；
- labels、signals、graph、vector、Tantivy/LanceDB/Oxigraph projection、derived control plane；
- 为未来部署方式预先建设兼容层或通用 backend abstraction。

这些项目可以作为独立后续工作，但不阻塞当前三阶段链路。

## 3. 当前公开 operation

每个 operation 按纵向切片闭合 store → application → HTTP → typed client → adapter → test。

| 阶段 | operation |
| --- | --- |
| Walking skeleton | `board.list`、`board.columns`、`task.create`、`task.list`、`task.show`、`task.plan.not_required`、`task.promote` |
| Durable queue | `task.claim`、`task.heartbeat`、`task.release`、`task.review`、`task.done`、`task.block`；opt-in dispatcher |
| 协作信息 | `comment.create/list`、`step.create/list/update`、`dependency.create/list/remove`、`run.list/show/log`、`event.list` |
| 检索 projection | `search.tasks`、`search.tasks.by_status`、`search.status`、`index.rebuild`、`index.sync` |

`health`、`board.columns`、`stats`、task selector query 和 task search/index status 是只读
query，同样通过 `ApplicationService` 提供。run 不提供独立 create/update mutation；claim 同
事务创建 run，后续 lifecycle command 同事务更新 run 和 event。task search 的 FTS projection
只读 canonical task/comment/run/event，未 ready 时显式回退 canonical SQL。

MCP 使用 `board_list`、`task_*`、`search_tasks`、`search_status`、Stage 2 lifecycle tools
和 Stage 3 collaboration tools。所有 MCP `tools/call` 只调用 typed localhost client；MCP
不启动 host。

## 4. 状态模型

权威 `tasks.status` 为：

```text
triage | todo | scheduled | ready | running | blocked | review | done | archived
```

`board_columns` 只是展示映射。应用层不提供任意 `transition(target_status)`；每个公开 mutation 都是有明确前置条件和输入的 typed command。

| 状态 | 语义 |
| --- | --- |
| `triage` | 规格尚不完整，不能执行。 |
| `todo` | 已定义，但计划、依赖或其他条件尚未满足。 |
| `scheduled` | 等待未来排期。 |
| `ready` | 满足条件，可由人工或显式 dispatcher claim。 |
| `running` | 已 claim，拥有 active run 和 lease。 |
| `blocked` | 因外部依赖、失败或人工输入暂停。 |
| `review` | 执行结果待人工确认。 |
| `done` | 已完成。 |
| `archived` | 隐藏的历史记录，不进入默认调度。 |

`task.plan.not_required` 和 `task.promote` 共同完成 walking-skeleton 的 `todo → ready` 检查；plan、依赖、排期和 archived 保护由 service 重新计算，adapter 不自行判断。

Durable queue 的不变量：

- `ready → running` 只能通过原子 `task.claim`；两个调用方并发 claim 同一 task 时恰好一个成功。
- `running` 必须有 `claim_token`、`claim_owner`、`claim_expires_at` 和一个 active run。
- `heartbeat`、`release`、`review`、`done`、`block` 先校验 task、owner、token 和 lease，再在同一事务更新 task、run、event。
- `release` 只允许 matching claim owner/token 主动释放；成功后 task 回到 `ready`、active run 变为 `canceled`，并写 `task.released`。
- dispatcher 只扫描 `ready`，绝不自动 claim `review`、`todo` 或 `scheduled`。

完整转换表与 guard 见 [`STATE_MACHINE.md`](STATE_MACHINE.md)。

## 5. Canonical 数据与 schema

`kanban-store-turso` 使用 `turso = 0.7.2`、`default-features = false`。schema 是 embedded SQL，使用简单的 `schema_migrations` 表和事务性、可重复执行的初始化。首次 host 启动幂等 seed `default` board 与固定 status columns。

canonical baseline 包含：

- `boards`、`board_columns`；
- `tasks`、`task_execution_plans`、`task_steps`；
- `task_dependencies`；
- `task_runs`；
- `task_comments`；
- `task_events`。

数据库必须保证：

- status、run status、step status、priority 等 CHECK；
- board 与 task/comment/step/dependency/run/event 的复合外键和 board isolation；
- `(board_id, seq)`、实体 id、active run 和实体本地 `idempotency_key` 唯一约束；
- dependency 自身约束、禁止 self-dependency；
- task snapshot 与对应 event 在同一事务提交。

canonical 数据是业务事实。搜索的 FTS projection、图、向量、缓存和其他 projection 只能从
canonical 数据重建，不能成为 mutation path。

## 6. Adapter 规则

### 6.1 CLI

`kanban serve [--db <path>] [--dispatcher-profile <path>]` 是唯一 DB owner。其他 domain 命令
通过 `kanban-client` 访问默认 `http://127.0.0.1:8721`，支持 `--json`、`--board`、`--actor`
和 board-local/global task selector。`kanban search` 与 `kanban index
status|doctor|rebuild|sync` 复用同一 search service。`doctor`、`stats`、`checkpoint`、verified
`backup`、portable `export/import`、`import-v30`、`vacuum` 和 `maintenance
status/run/rebuild/cleanup` 也只通过该 host 执行；它们不在 MCP surface 中。`import-v30`
需要 host 的 `legacy-sqlite-import` feature，未启用时返回 `feature_not_available`。
`kanban config show`、`kanban init`、`kanban board use/current`、`kanban completions`、隐藏
`__complete` 以及 `kanban hook codex ...` 只处理本地配置、completion 或 hook 文件，不打开
数据库；host 不可用时，domain 命令返回 `server_unavailable`。

### 6.2 MCP

MCP 是最小 Rust stdio server，使用官方 `rmcp` tools/stdio transport；不提供 resources/prompts，不拉起 host，不解释状态转换。工具名与 operation 一一对应，参数和响应复用 `kanban-contract` DTO。

### 6.3 Desktop

Desktop 保留已有页面结构和 TS `KanbanApi`，只通过 external host HTTP 工作。`RuntimeConfig`
只有 `apiBaseUrl`、`actor`、`board`；claim token 只在当前会话内保存，不写入磁盘。
labels/signals/neighborhood 等未迁移视图必须隐藏或禁用，不发送失败请求；Desktop search
view 仍需独立 slice 接入。

## 7. Dispatcher

dispatcher 是 `kanban serve` 内的 opt-in、单 worker loop，不是独立 daemon。`--dispatcher-profile <path>` profile 固定包含：

```text
board
worker command
poll interval
claim TTL
heartbeat interval
success/failure policy
log directory
```

loop 停止新 polling 后等待当前 worker 正常结束；第二次中断才强制退出。worker 的 claim、heartbeat、finish 必须调用同一 application command，stdout/stderr 仅写 profile 指定日志路径，数据库保存摘要和路径。

## 8. 配置、错误与重启

- server 只监听 loopback；不提供远程访问和登录。
- DB 解析优先级固定为 `--db` > `KANBAN_DB` > `KB_DB` > 最近项目 `.kb/config.toml` 的
  `db` > XDG 配置目录下 `kanban/config.toml` 的 `db` > XDG data-local 默认路径；项目或
  全局配置中的相对路径以各自 `config.toml` 所在目录为基准。该解析器供 `serve`、`config
  show` 和 `init` 共享，但只有 `serve` 会打开或初始化 Turso。
- board 解析优先级为 `--board` > `KB_BOARD` > 最近项目配置的 `board` > `default`；
  `KANBAN_SERVER_URL`、`--server-url` 只配置 client。
- host 每个 operation 在同一进程中按需获取 Turso connection；不启用 `multiprocess_wal`。
- `error.code`、DTO 和 HTTP status 映射由 `kanban-contract`/server/client 共同维护；adapter 不重新解释 domain error。
- 关闭并重启 host 后，boards、tasks、plans、comments、steps、dependencies、runs 和 events 从 canonical DB 继续可读；不会由 adapter 创建第二个数据库。

## 9. 验收基线

Stage 1 的最小验收：CLI 创建 task，Desktop 从同一 host 读到，MCP 调用 `task_plan_not_required` 和 `task_promote`，CLI show 得到 `ready`；停止并重启 host 后 task、plan、event 仍存在。

Stage 2 的最小验收：并发 claim 恰好一次成功；正确/错误 token 的 heartbeat、release、review、done、block 保持 task/run/event 原子一致；release 后可以再次 claim，dispatcher 不 claim `review`。

Stage 3 的最小验收：comment、step、dependency、run 和 event 在 CLI、MCP、Desktop 之间一致；cross-board、FK、唯一约束和 dependency cycle 被拒绝；重启后历史仍可读。

详细 HTTP/CLI wire contract 分别见 [`API_SPEC.md`](API_SPEC.md) 和 [`CLI_SPEC.md`](CLI_SPEC.md)。

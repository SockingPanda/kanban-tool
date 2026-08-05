# 架构

Kanban Tool 的 storage engine 是实现细节；当前产品真正统一的是 application entry point、状态机和 mutation path。唯一受支持的运行拓扑如下：

```text
┌──────────────┐       ┌──────────────────────┐
│ CLI commands │──┐    │ Desktop TS client    │──┐
└──────────────┘  │    └──────────────────────┘  │
                  ├──▶ typed localhost HTTP ────┤
┌──────────────┐  │                              │
│ MCP stdio    │──┘                              ▼
└──────────────┘                 ┌────────────────────────┐
                                 │ kanban serve           │
                                 │ Axum + ApplicationSvc  │
                                 │ state machine          │
                                 │ optional dispatcher    │
                                 └───────────┬────────────┘
                                             ▼
                                 ┌────────────────────────┐
                                 │ kanban-store-turso     │
                                 │ canonical kanban.db    │
                                 └────────────────────────┘
```

## 1. 进程与 ownership

只有 `kanban serve`（`kanban-server` host）可以打开、初始化和关闭 Turso 数据库。它持有 `AppState`、`TursoStore` 和 `ApplicationService<TursoApplicationStore>`，按 operation 在同一进程内获取 connection。默认路径为 `~/.local/share/kb/kanban.db`，默认监听 `127.0.0.1:8721`。

CLI 二进制包含 `serve` 子命令，因此链接了 server library；`serve` 之外的 CLI 命令只构造 `kanban-client`，不会打开数据库。MCP 和 Desktop 没有 store/server 依赖，不会拉起 host。server 不可用时 client 返回 `server_unavailable`，没有 embedded DB fallback。

不启用 `multiprocess_wal`。单一 OS 进程是数据库 owner，同进程的 operation 可以安全获取独立连接；跨进程直接打开 canonical 文件不属于产品路径。

## 2. Workspace crate 边界

```text
crates/
├── kanban-core          领域类型、错误、纯状态机
├── kanban-application   typed commands/queries 与 ApplicationService
├── kanban-contract      HTTP/CLI/MCP 共用 DTO、error envelope、schema 描述
├── kanban-store-turso   schema、migration、Turso repositories
├── kanban-server        唯一 host、Axum routes、dispatcher
├── kanban-client        typed synchronous localhost HTTP client
├── kanban-cli           薄 CLI adapter + serve wrapper
└── kanban-mcp           薄 stdio tools adapter

apps/
└── desktop              Tauri shell、React UI、TS HTTP client

xtask/
└── 私有离线工具          schema、依赖和 AGENTS 检查
```

根目录 `xtask/` 是 `publish = false` 的私有 workspace leaf，负责离线 contract/schema artifact、catalog 审计与 witness，以及依赖和 AGENTS 检查；不进入 host runtime path。它提供 `schema generate|check|audit|witnesses`、`deps check` 和 `agents check`。开发者入口仍是 `just`；相关 recipe 调用 `xtask`，而 `xtask` 只直接调用必要脚本，不反向调用 `just`。旧的 search、graph、vector、helper 和 projection crate 目前位于 workspace exclude 或仅作为迁移证据保留，不是 active canonical dependency；其业务能力必须迁入目标单 Host crate 与公开 surface，parity ledger 闭合后才允许删除旧目录。

依赖方向固定为：

```text
kanban-server  → kanban-application → kanban-core
       │        → kanban-contract
       └────────→ kanban-store-turso   (唯一 DB-owning edge)

kanban-client  → kanban-contract + kanban-core
kanban-cli     → kanban-client        (+ server library 仅用于 `serve` 子命令)
kanban-mcp     → kanban-client + kanban-contract
Desktop TS     → localhost HTTP/contract（不是 Cargo DB 依赖）
```

`kanban-cli` 的 `serve` wrapper 可以调用 server library，但 CLI command modules、MCP tools 和 Desktop 不得直接依赖 `kanban-store-turso` 或 SQLite。唯一 DB-owning implementation 是 `kanban-server` → `kanban-store-turso`。

## 3. Application service 与状态机

`kanban-application` 暴露显式 typed 方法，例如 `list_boards`、`create_task`、`list_tasks`、`get_task`、`mark_execution_plan_not_required`、`promote_task`、`claim_task`、`heartbeat_task`、`release_task`、`submit_review_task`、`complete_task`、`block_task`、comments/steps/dependencies queries and commands，以及 run/event queries。

不提供 generic `transition(target_status)`。每个 mutation 通过对应 operation 的前置条件、状态机 guard、board resolution、owner/token 校验和 store transaction；Axum handler、CLI、MCP、Desktop 和 dispatcher 都不能复制这些规则。

统一 mutation path：

```text
adapter input
  → kanban-client request / Axum extractor
  → ApplicationService typed command
  → state-machine validation
  → TursoApplicationStore
  → TursoStore connection + transaction
  → task snapshot / run / event commit
  → shared DTO/error response
```

同一 application service 也承载 health、board columns、stats、task selector 和其他只读
query。这样 CLI 创建的 task 会立即被 Desktop 读到，MCP 的 promote 结果也会立即被
CLI show 读到。

## 4. Store 与 canonical schema

`kanban-store-turso` 使用 `turso = 0.7.2`、`default-features = false`。`initialize` 在事务中执行 embedded canonical schema、写入 `schema_migrations` 并幂等 seed `default` board 和固定 columns；重复启动不会重建或分叉数据库。

baseline 表为：

```text
boards, board_columns
tasks, task_execution_plans, task_steps
task_dependencies
task_runs
task_comments
task_events
```

数据库层负责：

- status、step/run status、priority 和 JSON 值域 CHECK；
- board-scoped 外键、复合外键和 cascade；
- `(board_id, seq)`、实体 id、active run、task/comment/step 本地 `idempotency_key` 唯一性；
- dependency self-edge 与 duplicate edge 拒绝。

store 层只保存 canonical 业务事实。event 是 append-only 审计记录，但 task snapshot 与 event 必须在同一事务提交；run 由 claim 创建，不允许 adapter 自由创建或修改。

## 5. HTTP 与 typed client

`kanban-server` 使用 Axum。handlers 只做 path/query/body 解析、actor 提取、调用 ApplicationService 和 DTO/error 映射；路由按 board/task/comment/dependency/step/run/event operation 组织。`kanban-client` 使用同步 `ureq`，复用 `kanban-contract` DTO 和 error envelope，并负责 board-local selector 到全局 `t_...` id 的解析。

公开 HTTP surface 包括 board list/columns、task create/list/show、execution-plan not-required、promote、Stage 2 lifecycle、comments、steps、dependencies、run list/show/log、event list 和健康 query。`task.release` 使用独立的 `POST /api/v1/tasks/{task_id}/transitions/release`；其余路径和精确 wire shape 以 [`API_SPEC.md`](API_SPEC.md) 为准。

不增加第二套 HTTP/IPC 协议、framed transport、named pipe 或跨版本 capability negotiation。MCP 的 stdio transport 只包裹 client 调用，不改变 operation 语义。

## 6. Adapter

### 6.1 CLI

CLI parser 将用户输入转换为 typed client request，并渲染人类文本或稳定 JSON。`--server-url`、`KANBAN_SERVER_URL` 只影响请求地址；`--db`、`KANBAN_DB` 只出现在 `serve`。`init` 与尚未迁移的命令在触库前返回 `feature_not_available`。

### 6.2 MCP

`kanban-mcp` 使用 `rmcp 3.1.0` 的 tools/stdio transport，不提供 resources/prompts，不启动 server。tool 名称按 operation 命名，如 `board_list`、`task_create`、`task_claim`、`run_log` 和 `event_list`；每个 tool 只调用 `kanban-client`。

### 6.3 Desktop

Desktop 保留 Tauri/React 页面结构，运行时配置只包含 `apiBaseUrl`、`actor`、`board`。Tauri command 只提供运行时配置和 board 选择，不拥有数据库、嵌入 Axum、runtime guard 或 DB lookup。claim token 仅存于当前会话；未迁移的 labels/signals/search/neighborhood 视图隐藏或禁用。

## 7. Dispatcher

dispatcher 是 host 内的 opt-in 单 worker loop：

```text
kanban serve --dispatcher-profile profile.toml
```

profile 固定包含 board、worker command、poll interval、claim TTL、heartbeat interval、success/failure policy 和 log directory。loop 只扫描 `ready`，通过同一 ApplicationService claim/heartbeat/finish，绝不自动 claim `review`、`todo` 或 `scheduled`。默认不启动 dispatcher；没有 daemon 注册、Job Object、named pipe 或自动 supervision。

关闭时先停止下一轮 polling，等待当前 worker 正常结束；第二次中断才 force-stop。worker 日志写入 profile 指定目录，数据库保存 run 摘要和可信日志路径。

## 8. 重启、错误与安全

- host 启动先创建 DB 父目录，再 open/initialize；退出时关闭 host，入口不会额外持有连接。
- server 只绑定 loopback，不提供登录、远程访问或多用户权限模型。
- `ApplicationService` 的 domain error 映射到统一 `kanban-contract` error code；adapter 不重新解释状态或错误。
- host 重启后 canonical DB 保留 boards、tasks、plans、comments、steps、dependencies、runs 和 events；不会创建 `kb.db` 或旧 SQLite 路径。
- actor、created_by、claim_owner 是审计字符串，不是鉴权主体。

## 9. 明确后置项

本架构暂不包含：SQLite importer、旧 v2/v3 runtime/receipt/recovery、projection generation、LanceDB/Tantivy/Oxigraph rebuild、labels/signals、semantic search、graph/vector API、自动 server supervision、跨进程数据库访问和完整旧 API 兼容。实现这些能力前，必须先证明它们仍然复用本文件的 single-host application path，而不是重新引入第二条 canonical mutation path。

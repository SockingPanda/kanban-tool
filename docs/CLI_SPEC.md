# `kanban` CLI 规范

`kanban` 是 canonical localhost application host 的薄适配器。除 `kanban serve` 外，
所有命令都只创建 `kanban-client` 并调用 `http://127.0.0.1:8721`；CLI 不打开、初始化或
fallback 到任何数据库。

当前公开命令只覆盖 board、task、comment、dependency、run 和 event。labels、signals、
search、graph、vector、projection、maintenance、旧导入/初始化以及旧的直接数据库命令
不属于本轮 surface。

## 1. 全局选项

```text
kanban [OPTIONS] <COMMAND>
```

| 选项 | 环境变量 | 默认值 | 作用 |
|---|---|---|---|
| `--server-url <URL>` | `KANBAN_SERVER_URL` | `http://127.0.0.1:8721` | client 访问的 loopback host；仅允许 `http://` loopback URL |
| `--board <SLUG-OR-ID>` | `KB_BOARD` | `default` | board-scoped selector 的上下文 |
| `--actor <NAME>` | `KANBAN_ACTOR` | `USER` → `USERNAME` → `local` | CLI 解析后作为 `X-KB-Actor` 发送并用于审计 |
| `--json` | — | 关闭 | 输出稳定 JSON envelope |

`--db` 不是全局选项，只能由 `serve` 使用。相同参数可放在命令前，clap 将其作为 global
option 解析。

### JSON 成功与错误

`--json` 成功输出为 `{ "data": ... }`；CLI 使用自己的 output DTO，不保证保留 HTTP
response 的 pagination/cursor `meta`。运行期错误输出为：

```json
{
  "error": {
    "code": "server_unavailable",
    "message": "server unavailable: connection refused",
    "exit_code": 9
  }
}
```

脚本只依赖 `error.code` 和进程退出码，不解析 message。clap 参数解析失败仍由 clap 写入
stderr 并退出 `2`，不会输出运行期 JSON error。

当前 adapter 重点错误码：

| code | exit code | 语义 |
|---|---:|---|
| `invalid_input` / `invalid_response` | 2 | 参数、selector 或 host 响应无效 |
| `not_found` | 3 | 资源不存在 |
| `invalid_transition`、`execution_plan_required`、`steps_incomplete`、`dependency_cycle` | 4 | application/state machine 拒绝操作 |
| `claim_conflict`、`claim_token_mismatch`、`idempotency_conflict` | 5 | claim 或实体幂等冲突 |
| `dependency_blocked` | 6 | 依赖阻止状态转换 |
| `server_unavailable` | 9 | `kanban serve` 未运行或 loopback 连接失败 |
| `feature_not_available` | 10 | 命令明确未迁移，且未触碰数据库 |

## 2. 唯一数据库 owner：`serve`

```text
kanban serve [--db <PATH>] [--dispatcher-profile <PATH>]
             [--host <LOOPBACK-IP>] [--port <PORT>]
```

- `--db <PATH>` 或 `KANBAN_DB` 只配置 host 使用的 canonical Turso 文件。
- 未指定时，Linux 默认路径为 `~/.local/share/kb/kanban.db`（通过平台 data-local 目录解析）。
- `--host` 默认 `127.0.0.1`，非 loopback 地址直接返回 `invalid_input`。
- `--port` 默认 `8721`。
- 没有 `--dispatcher-profile` 时不自动消费 queue；传入 profile 才启用同进程单 worker dispatcher。
- Ctrl-C 第一次 graceful shutdown，第二次 force stop。

启动后 host 负责初始化 schema、打开/关闭 Turso 和提供全部 HTTP route。其他 CLI 命令在 host
未启动时只返回 `server_unavailable`，不会创建数据库文件。

## 3. Board

```text
kanban board list [--include-archived]
kanban board columns [BOARD]
```

两者都是只读 client query：`board list` 返回 `ListBoardsResponse`，`board columns` 返回
`ListBoardColumnsResponse`。当前 CLI 没有 board create/use/archive/config 命令。

## 4. Task

### 4.1 创建、列表、详情

```text
kanban task create <TITLE>
  [--description <TEXT>] [--status triage|todo|scheduled|ready]
  [--assignee <NAME>] [--priority <0..=3>]
  [--scheduled-at <MS>] [--due-at <MS>] [--max-retries <N>]
  [--metadata <JSON-OBJECT>] [--idempotency-key <KEY>] [--task-id <T_ID>]

kanban task list
  [--status triage|todo|scheduled|ready|running|blocked|review|done|archived]...
  [--priority <0..=3>]...
  [--plan-filter plan_needed|has_steps|incomplete_required_steps]...
  [--assignee <NAME>] [--query <TEXT>] [--include-archived]
  [--limit <N>] [--offset <N>] [--sort <SORT>]

kanban task show <TASK_SELECTOR>
```

`TASK_SELECTOR` 可为全局 `t_...`、`board#seq`、`#seq` 或数字 seq；client 在发 HTTP mutation
前将 board-local ref 解析为全局 ID。`task show --details` 需要 deferred ontology
projection，稳定返回 `feature_not_available`。

`task create` 的 labels/dependencies 不在 CLI surface；`task list` 不接受 label filter。
`--search` 是 `--query` 的隐藏兼容 alias。成功输出分别使用 task create、CLI task-list
（只含 `data`，不保留 HTTP pagination `meta`）和 task show 的公开 output DTO。

`--sort` 使用 HTTP wire 值：`seq|-seq|title|-title|status|-status|position|-position|
priority|-priority|assignee|-assignee|scheduled_at|-scheduled_at|due_at|-due_at|
created_at|-created_at|updated_at|-updated_at`，默认值为 `position`。负号开头的值需要写成
`--sort=-updated_at`，避免被 clap 解释为另一个 option。

### 4.2 Execution plan

```text
kanban task step not-required <TASK_SELECTOR> --reason <TEXT>
```

这会调用 `POST /api/v1/tasks/{task_id}/execution-plan/not-required`，返回
`MarkExecutionPlanNotRequiredResponse`。它是 promote 前的显式 plan gate。

### 4.3 State machine transitions

```text
kanban task promote <TASK_SELECTOR>
kanban task claim <TASK_SELECTOR> [--ttl-ms <MS>]
kanban task heartbeat <TASK_SELECTOR> --claim-token <TOKEN>
    [--ttl-ms <MS>] [--note <TEXT>]
kanban task release <TASK_SELECTOR> --claim-token <TOKEN>
kanban task review <TASK_SELECTOR> [--claim-token <TOKEN>] [--force]
kanban task done <TASK_SELECTOR> [--claim-token <TOKEN>] [--force]
kanban task block <TASK_SELECTOR> [<REASON>|--reason-file <PATH|->]
    [--claim-token <TOKEN>] [--force]
```

`done` 有 visible alias `complete`。claim 是原子 `ready -> running`，并在同一 application
path 创建 run；heartbeat/release/review/done/block 继续复用该 path。release 仅接受当前
owner 的 token，成功后任务回到 ready；错误 token 返回 `claim_token_mismatch`/相应稳定错误码。
`task block` 的 inline reason 与 `--reason-file` 互斥；`-` 从 stdin 读取，输入上限为 1 MiB。

每个命令的人类输出只显示精简 task 状态；`--json` 输出对应 contract response。CLI 不自行
解释状态机或直接修改 run。

### 4.4 Steps

```text
kanban task step add <TASK_SELECTOR> <TITLE>
  [--body <TEXT>] [--link-task <TASK_SELECTOR>] [--position <N>]
  [--required|--optional] [--idempotency-key <KEY>]
kanban task step list <TASK_SELECTOR>
kanban task step update <TASK_SELECTOR> <STEP_SELECTOR>
  [--title <TEXT>] [--body <TEXT>] [--link-task <TASK_SELECTOR>|--unlink-task]
  [--position <N>] [--required|--optional]
```

`STEP_SELECTOR` 可为全局 `step_...` 或该 task 下的 `S<n>`。add/list/update 返回同一
`ApiTaskSteps` shape；add 的 idempotency key 仅在实体 task 内生效。

## 5. Comment

```text
kanban comment add <TASK_SELECTOR> <BODY>
  [--kind note|decision|signal] [--author <NAME>]
  [--author-type user|agent] [--agent-type <TYPE>]
  [--metadata-json <JSON-OBJECT>] [--idempotency-key <KEY>]
kanban comment list <TASK_SELECTOR>
```

add 是 mutation，list 是 query。add 的 key 属于 task；相同 key 与相同 payload 可安全重放，
不同 payload 返回 `idempotency_conflict`。未指定 author 时由 host actor 规则填充。

## 6. Dependency

顶层命令名为 `dep`，`dependency` 是 visible alias：

```text
kanban dep add <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
kanban dep list <TASK_SELECTOR>
kanban dep remove <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
```

add/remove 是 mutation，list 是 query。client 先解析两个 selector；server 负责同 board、FK、
唯一约束和 cycle 检查。dependency create 没有额外 receipt/idempotency flag。

## 7. Runs 与 events（只读）

```text
kanban runs <TASK_SELECTOR>
kanban run show <RUN_ID>
kanban run logs <RUN_ID>
kanban events [TASK_SELECTOR] [--after <ID>] [--limit <N>]
```

- `RUN_ID` 必须是全局 `r_...`。
- `run logs` 有 visible alias `log`，没有 `--tail-bytes` 或其他 tail 参数。
- log 始终读取固定 256 KiB bounded snapshot；JSON 输出包含 `run_id`、`content`、`truncated`
  和兼容 DTO 中固定为 `null` 的 `tail_bytes`，但用户不能指定 tail 大小。
- run 不能由 CLI 创建或修改；只能通过 task claim/heartbeat/release/review/done/block 产生和更新。
- `events` 可按当前 board 和 task selector 过滤，JSON 输出包含事件 `data`，但当前 CLI
  output 不保留 HTTP `meta.next_after`；事件 payload 对未知 kind 保持原 JSON。

## 8. 未迁移命令与停止行为

```text
kanban init
kanban <未列出的旧命令>
```

`init` 和未知的顶层 clap external subcommand 稳定返回：

```json
{
  "error": {
    "code": "feature_not_available",
    "message": "...",
    "exit_code": 10
  }
}
```

该路径不读取配置数据库、不创建文件、不 fallback 到 SQLite。未注册的嵌套子命令（例如
`kanban task archive`）由 clap 在发起任何 HTTP/DB 操作前以参数错误退出 `2`；它们不使用
runtime JSON envelope。labels、signals、search、maintenance、projection、graph、vector
等未知顶层 surface 返回 `feature_not_available`。host 停止或端口不可达时，已迁移命令
返回 `server_unavailable`（exit code `9`），而不是切换到第二条执行路径。

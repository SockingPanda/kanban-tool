# `kanban` CLI 规范

`kanban` 是 canonical localhost application host 的薄适配器。除配置、completion、
Codex hook 和 `kanban serve` 这些本地 shell 命令外，所有命令都只创建 `kanban-client` 并
调用 `http://127.0.0.1:8721`；CLI 不打开、初始化或 fallback 到任何数据库。

当前实现覆盖 board、task、comment、attachment、dependency、run、event、search、index、
context、ontology、signal、host-admin maintenance，以及本节列出的本地 shell 命令；这不
是最终功能边界。labels、graph、vector 的维护能力仍按 parity ledger 恢复为 localhost
client 命令。portable export/import 和 verified backup 已接入；旧的直接数据库执行路径不
会恢复。

## 1. 全局选项

```text
kanban [OPTIONS] <COMMAND>
```

| 选项 | 环境变量 | 默认值 | 作用 |
|---|---|---|---|
| `--server-url <URL>` | `KANBAN_SERVER_URL` | `http://127.0.0.1:8721` | client 访问的 loopback host；仅允许 `http://` loopback URL |
| `--board <SLUG-OR-ID>` | `KB_BOARD` | `default` | board-scoped selector 的上下文；本地 shell 命令只作选择值 |
| `--db <PATH>` | `KANBAN_DB` → `KB_DB` | XDG data-local `kb/kanban.db` | `serve` 的 canonical Turso 路径；配置查看会解析但不打开 |
| `--locale <auto|zh-CN|en>` | `KANBAN_LOCALE` | 系统 locale（默认 `zh-CN`） | 人类输出语言；JSON 字段稳定 |
| `--actor <NAME>` | `KANBAN_ACTOR` | `USER` → `USERNAME` → `local` | CLI 解析后作为 `X-KB-Actor` 发送并用于审计 |
| `--json` | — | 关闭 | 输出稳定 JSON envelope |

配置优先级为 `--db` > `KANBAN_DB` > `KB_DB` > 最近项目 `.kb/config.toml` 的 `db` >
XDG 配置目录下 `kanban/config.toml` 的 `db` > XDG data-local 默认路径。`--board` 的优先级
为 `--board` > `KB_BOARD` > 最近项目配置的 `board` > `default`。项目配置中的相对 `db`
路径以该 `config.toml` 所在目录为基准。

### JSON 成功与错误

`--json` 成功输出为 `{ "data": ... }`；CLI 使用自己的 output DTO，不保证保留 HTTP
response 的 pagination/cursor `meta`。运行期错误输出为：

<!-- schema-doc-ignore: CLI runtime error 的说明性示例，不绑定某个具体 command output contract -->
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

- `--db <PATH>`、`KANBAN_DB` 或 `KB_DB` 只配置 host 使用的 canonical Turso 文件。
- 未指定时，默认路径为 `$XDG_DATA_HOME/kb/kanban.db`；未设置 `XDG_DATA_HOME` 时使用平台
  data-local 目录（Linux 通常为 `~/.local/share`）。
- `--host` 默认 `127.0.0.1`，非 loopback 地址直接返回 `invalid_input`。
- `--port` 默认 `8721`。
- 没有 `--dispatcher-profile` 时不自动消费 queue；传入 profile 才启用同进程单 worker dispatcher。
- Ctrl-C 第一次 graceful shutdown，第二次 force stop。

启动后 host 负责初始化 schema、打开/关闭 Turso 和提供全部 HTTP route。其他 CLI 命令在 host
未启动时只返回 `server_unavailable`，不会创建数据库文件。

### 2.1 Host maintenance

这些命令只通过 `kanban-client` 请求 canonical `kanban serve`，不会打开数据库或 fallback：

```text
kanban doctor
kanban stats [--board <BOARD>]
kanban checkpoint
kanban backup --path <PATH>
kanban export --path <PATH>
kanban import --path <PATH> [--replace]
kanban import-v30 --path <PATH> [--attachment-root <PATH>]
kanban vacuum
kanban maintenance status
kanban maintenance run [--owner <OWNER>]
kanban maintenance rebuild [--owner <OWNER>]
kanban maintenance cleanup [--owner <OWNER>]
```

成功默认输出人类可读摘要，`--json` 输出 `{ "data": ... }`。backup/export 目标不得已存在
（包括 symlink）；导入通过 `import_journal` 记录阶段。`--replace` 在 host 独占窗口内先做
verified backup，再以单个 Turso immediate transaction 原子替换 canonical facts，校验通过后
同步返回 `phase=completed` 并入队 search/vector/graph rebuild；事务失败会 rollback，重复
source fingerprint 幂等。prepare staging 的恢复异常可能返回 `phase=validated` 与
`restart_required`，这不是正常成功结果。maintenance owner lease 忙时返回 `conflict`，MCP
不提供这些命令。
`import-v30` 是 legacy SQLite v30 的 host-admin 入口，仅在构建时启用
`legacy-sqlite-import` feature 后可用；未启用时保持 typed client 路由并返回
`feature_not_available`，不会由 CLI 直接打开 SQLite。

## 3. 本地配置与项目 shell

### 3.1 `config show`

```text
kanban config show
kanban --json config show
```

该命令只解析 `db`、`board`、`locale` 及其来源，不打开、初始化、迁移或创建 Turso
数据库，也不创建 `.kb` 或 XDG 配置文件。JSON 使用 `CliConfigShowOutput`：每个值包含
`value` 和 `source`；`source.kind` 为 `flag`、`env`、`project_config`、`global_config`
或 `default`。TOML 解析失败返回 runtime JSON `invalid_input`（exit `2`），stderr 保持空。

### 3.2 `init`

```text
kanban init [--force]
kanban --json init
```

`init` 是幂等的项目 shell 初始化：在当前目录（或最近已有项目配置）创建/复用
`.kb/config.toml`，仅在缺省时写入 `board = "default"`。它不打开、初始化或创建
canonical 数据库；数据库由 `kanban serve` 首次启动负责。`--force` 是兼容 no-op，不会重置
既有 `db`、`board`、`vector` 或未知扩展字段。JSON 使用 `CliInitOutput`，包含 `db_path`、
`board_slug`、`config_path` 和 `created`；`board_id` 在未连接 host 的配置侧结果中为
`not_initialized`。

### 3.3 Board selection（不访问 host）

```text
kanban board use <BOARD>
kanban board current
```

`board use` 只更新项目 `.kb/config.toml` 的 `board`，保留 `db`、`vector` 和未知 TOML
字段；`board current` 只解析当前选择。两者都不校验 Turso 中是否存在该 board，也不访问
host。JSON 分别使用 `CliBoardUseOutput` 与 `CliBoardCurrentOutput`，返回 board selector、
`config_path`、`source` 以及 `created`/`updated` 标记。若需要校验或创建 domain board，
必须使用 localhost HTTP client 命令。

## 4. Board

```text
kanban board list [--include-archived]
kanban board columns [BOARD]
```

两者都是只读 client query：`board list` 返回 `ListBoardsResponse`，`board columns` 返回
`ListBoardColumnsResponse`。`board use/current` 属于上一节的本地配置 shell，不是 domain
board query。

## 5. Completions

```text
kanban completions bash|zsh|fish|powershell|elvish
kanban __complete task-ref|dependency-task-ref|board|status|comment-kind [PREFIX]
```

静态 completion 由 clap 生成。Bash/Zsh 脚本可调用隐藏的 `__complete` helper；该 helper
不会打开数据库。task/dependency ref 在 host 不可用时安静返回空；board 候选只来自本地
配置解析；status 与 comment kind 使用固定枚举。其它 shell 只生成静态脚本。

## 6. Codex hooks

```text
kanban hook codex install [--handler-command <PREFIX>] [--timeout <SECONDS>]
    [--record-signals]
kanban hook codex status
kanban hook codex uninstall
kanban hook codex handle failure [--record-signals]
kanban hook codex handle task-create
```

`install`、`status`、`uninstall` 只读写 `CODEX_HOME/hooks.json`（未设置时为
`$HOME/.codex/hooks.json`）和 XDG 配置目录下的 `kanban/codex-hooks.json`；不会打开
canonical 数据库。安装项带 `kanbanManaged` marker 和 command fingerprint；重复安装先
移除并重建自身条目，保留用户 hooks。卸载只移除 `type=command`、marker 和 fingerprint
都匹配的条目，篡改或缺少 fingerprint 的条目保持不动。文件写入使用同目录临时文件、
`fsync` 和原子 rename；prompt 配置只在不存在时创建。

`handle` 从 stdin 解析 Codex `PostToolUse`/`Bash` payload：失败 kanban 命令输出
`systemMessage`，成功 `task create` 输出任务 ref 建议；无效、非 Bash 或非 kanban payload
安静退出。即使传入 `--record-signals`，当前 handler 也不直接写库；需要记录 signal 时
应由后续 localhost client operation 完成。

## 7. Task

### 7.1 创建、列表、详情

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
projection，当前暂时返回 `feature_not_available`；ontology 切片恢复后必须改为真实查询。

当前 `task create` 尚未接通 labels/dependencies，`task list` 也尚未接通 label filter；
这些都是 parity ledger 中必须恢复的 CLI surface。
`--search` 是 `--query` 的隐藏兼容 alias。成功输出分别使用 task create、CLI task-list
（只含 `data`，不保留 HTTP pagination `meta`）和 task show 的公开 output DTO。

`--sort` 使用 HTTP wire 值：`seq|-seq|title|-title|status|-status|position|-position|
priority|-priority|assignee|-assignee|scheduled_at|-scheduled_at|due_at|-due_at|
created_at|-created_at|updated_at|-updated_at`，默认值为 `position`。负号开头的值需要写成
`--sort=-updated_at`，避免被 clap 解释为另一个 option。

### 7.2 Search 与 index

```text
kanban search <TEXT>
  [--status triage|todo|scheduled|ready|running|blocked|review|done|archived]...
  [--label <NAME>]... [--assignee <NAME>] [--include-archived]
  [--limit <N>] [--offset <N>]

kanban index status
kanban index doctor
kanban index rebuild
kanban index sync
```

`search` 调用 `/api/v1/search/tasks`，返回 FTS/canonical fallback 的 task hit；`--json`
使用 `cli.search-output`，人类输出显示 task ref、status、title、score 和 highlight
snippet。`index status`/`doctor` 是只读状态查询；`rebuild` 从 canonical facts 重建
Turso `task_search_fts`，`sync` 处理 pending projection job 或 event lag。四个 index 命令
均只调用 localhost host，不打开数据库。

### 7.3 Execution plan

```text
kanban task step not-required <TASK_SELECTOR> --reason <TEXT>
```

这会调用 `POST /api/v1/tasks/{task_id}/execution-plan/not-required`，返回
`MarkExecutionPlanNotRequiredResponse`。它是 promote 前的显式 plan gate。

### 7.4 State machine transitions

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

### 7.5 Steps

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

### 7.6 Context pack（只读）

```text
kanban context build [SUBJECT]
  [--task <TASK_ID>] [--reference <TASK_REFERENCE>] [--query <TEXT>]
  [--depth <N>] [--lexical-limit <N>] [--graph-limit <N>] [--vector-limit <N>]
  [--max-items <N>] [--budget <N>]
```

`SUBJECT` 以 `t_` 开头时作为 `--task`，其他值作为 board-local `--reference`；query-only
调用可省略 positional subject 并提供 `--query`。参数至少需要一个 task/reference/query
selector。命令只通过 `kanban-client` 请求 `GET /api/v1/tasks/{task_id}/context`，不会写入
canonical task、relation 或任何 derived provider。

文本输出按 `rank source entity_uri score reason` 展示 item，并附带 truncation 和 provider
状态；`--json` 使用 `CliContextBuildOutput`，保留 `policy`、`provenance`、`evidence`、
`providers`、`degraded` 与 diagnostics。vector/graph provider 降级时仍返回 lexical
context；跨 board candidate 会被丢弃。

## 8. Attachment

```text
kanban attachment add <TASK_SELECTOR> <FILE> [--filename <NAME>]
  [--content-type <MIME>] [--attachment-id <A_ID>]
kanban attachment list <TASK_SELECTOR>
kanban attachment download <TASK_SELECTOR> <ATTACHMENT_ID> --out <PATH>
kanban attachment remove <TASK_SELECTOR> <ATTACHMENT_ID>
```

`add`、`list`、`remove` 的 `--json` 输出分别是 `CliAttachmentAddOutput`、
`CliAttachmentListOutput`、`CliAttachmentRemoveOutput`。`download` 将 typed client 返回的
raw bytes 写到 `--out`，不伪装成 JSON envelope。所有命令只通过 localhost host；CLI 不读写
attachment root，也不接受任意 host path 作为服务端路径。

## 9. Comment

```text
kanban comment add <TASK_SELECTOR> <BODY>
  [--kind note|decision|signal] [--author <NAME>]
  [--author-type user|agent] [--agent-type <TYPE>]
  [--metadata-json <JSON-OBJECT>] [--idempotency-key <KEY>]
kanban comment list <TASK_SELECTOR>
```

add 是 mutation，list 是 query。add 的 key 属于 task；相同 key 与相同 payload 可安全重放，
不同 payload 返回 `idempotency_conflict`。未指定 author 时由 host actor 规则填充。

## 10. Signal

```text
kanban signal record [--input <JSON-FILE|->]
kanban signal list [--status <STATUS>] [--kind <KIND>] [--task <TASK_SELECTOR>]
  [--include-all] [--limit <N>]
kanban signal show <SIGNAL_ID>
kanban signal review [--status <STATUS>] [--kind <KIND>] [--task <TASK_SELECTOR>]
  [--limit <N>]
kanban signal confirm <SIGNAL_ID>... --reason <TEXT>
kanban signal reject <SIGNAL_ID>... --reason <TEXT>
kanban signal resolve <SIGNAL_ID>... --reason <TEXT>
kanban signal supersede <SIGNAL_ID>... --by <SIGNAL_ID> --reason <TEXT>
```

所有 signal 命令只通过 `kanban-client` 调用 host。record 的 JSON body 使用
`RecordSignalRequest`；list/review/show 使用对应 typed response；四个 lifecycle 命令共享
原子批量 review path。`--json` 输出分别遵循 `CliSignal*Output` contracts。

## 11. Dependency

顶层命令名为 `dep`，`dependency` 是 visible alias：

```text
kanban dep add <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
kanban dep list <TASK_SELECTOR>
kanban dep remove <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
```

add/remove 是 mutation，list 是 query。client 先解析两个 selector；server 负责同 board、FK、
唯一约束和 cycle 检查。dependency create 没有额外 receipt/idempotency flag。

## 12. Runs 与 events（只读）

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

## 13. 未迁移命令与停止行为

```text
kanban <未列出的旧命令>
```

未知的顶层 clap external subcommand 稳定返回：

<!-- schema-doc-ignore: 迁移期间 feature_not_available 的说明性错误示例 -->
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
runtime JSON envelope。尚未完成的 labels、projection、
graph、vector 等顶层 surface 暂时返回 `feature_not_available`；只要该临时路径仍存在，
对应 parity 项就不能标记完成。host 停止或端口不可达时，已迁移命令返回
`server_unavailable`（exit code `9`），而不是切换到第二条执行路径。

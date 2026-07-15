# CLI SPEC

默认 binary 名称：`kanban`

CLI 是一等入口；它与 Web 使用同一套 `kanban-sqlite::service` backed service path
和 SQLite schema。

---

## 1. Global Options

```bash
kanban [GLOBAL_OPTIONS] <COMMAND>
```

| Option | 说明 |
|---|---|
| `--db <path>` | 指定 SQLite DB；优先级高于 env、config 和 XDG 默认路径。 |
| `--board <slug-or-id>` | 显式指定 active board，优先级最高。 |
| `--actor <name>` | 操作 actor。默认 OS username。 |
| `--locale <auto|zh-CN|en>` | human 输出语言。默认 `zh-CN`；`auto`/`system` 使用系统 locale。 |
| `--json` | JSON 输出。 |

SQLite DB path 解析顺序：

1. `--db <path>`。
2. `KANBAN_DB` 环境变量。
3. `KB_DB` 环境变量（兼容短名）。
4. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `db = "<path>"`。
5. 用户全局 config `$XDG_CONFIG_HOME/kanban/config.toml`，读取 `db = "<path>"`。
6. fallback 到 XDG data 默认路径，通常是 `~/.local/share/kb/kb.db`。

Active board 解析顺序：

1. `--board <slug-or-id>`。
2. `KB_BOARD` 环境变量。
3. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `board = "<slug>"`。
4. fallback 到 `default`。

`kanban board use <board>` 会把当前目录写成项目级 `.kb/config.toml`；后续子目录自动继承该 active board。该配置只选择本地项目的 board，不创建新 DB。
如果同一配置文件也包含 `db = "<path>"` 或 `[vector]`，`board use` 必须保留这些字段。配置中的相对 DB 路径按配置文件所在目录解析；环境变量和 `--db` 中的相对路径按当前工作目录解析。

Locale 只影响 human-readable 输出和错误消息，不改变 JSON key、状态枚举、task ref、ID、exit code 或机器可读 diagnostics。选择顺序：

1. `--locale <auto|zh-CN|en>`。
2. `KANBAN_LOCALE`。
3. 默认 `zh-CN`。

`auto` / `system` 会按 `LC_ALL`、`LC_MESSAGES`、`LANG` 解析系统 locale；当前只支持中文和英文。脚本和自动化应优先使用 `--json`，不要依赖 human 文案。

### 1.1 Config inspection

```bash
kanban config show [--json]
```

`config show` 输出当前 CLI 会使用的 SQLite DB path、active board 和 locale，以及每个值的来源。该命令用于 agent/operator 排查 precedence，不会打开、初始化或创建 SQLite DB。

`--json` 输出使用普通 `{ "data": ... }` envelope，`data` 结构如下：

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
| `flag` | 来自显式 CLI flag，例如 `--db`、`--board`、`--locale`。 |
| `env` | 来自环境变量，例如 `KANBAN_DB`、`KB_DB`、`KB_BOARD`、`KANBAN_LOCALE`。 |
| `project_config` | 来自最近的项目级 `.kb/config.toml`。 |
| `global_config` | 来自 `$XDG_CONFIG_HOME/kanban/config.toml`。当前只适用于 DB path。 |
| `default` | 来自 CLI 默认值或 fallback。 |

`locale.value` 是实际解析后的 locale；当输入为 `auto` / `system` 时，`input` 保留原始选择，`value` 保留系统 locale 解析结果。`db.value` 对显式 flag 和环境变量保留调用方传入的路径形态；config 中的相对 DB 路径按 config 文件所在目录解析。

### 1.2 Help output contract

`kanban --help` 和公开 command group 的 `--help` 输出必须为每个公开 command/subcommand 行提供一句简短用途说明；隐藏内部命令（例如 `__complete`）除外。`kanban` 无参或公开 command group 缺少 subcommand 时必须显示同一类简洁帮助，而不是只输出 parser error；这仍属于 clap parse-time 路径，退出码为 2，且不输出 runtime JSON error envelope。全局 options 的 help 必须说明它们影响的是 SQLite DB、active board、actor、locale 或 JSON 输出，不改变 JSON key、状态枚举或 exit code 契约。

关键 agent-facing 输入面必须在命令 help 中优先展示安全路径：多行或 shell-sensitive 文本使用 `--description-file -`、`--body-file -`、`--metadata-json-file <PATH|->`、`--metadata-file <PATH|->` 或 `--input -`，避免 shell expansion / quoting 污染。危险、破坏性或容易误解的 flag 必须在 help 中说明语义，例如 `task archive --force` 绕过普通 archive guard，`import --replace` 是有意 backup/restore flow 的替换式恢复入口；兼容 no-op flag 必须明确写出 no-op。

对 `PATH|-` 文本输入（如 `--reason-file`、`--input`、`--body-file`、`--metadata-json-file`）与其变体，`kanban` 实现上约束单次输入上限为 1MiB。超过上限时返回 `invalid_input`，并在 `--json` 下通过 `error.message` 指明输入长度限制，CLI 端可用更高层分片策略。该约束覆盖 stdin 与文件输入，目的是避免错误输入导致 CLI 服务路径资源异常。

顶层 help 和关键 agent-facing 命令可以包含 `Examples:`，但示例必须保持短小、稳定，并与实际命令语义一致；不要把 CLI_SPEC 的完整说明复制进 help。CLI help contract 由 `crates/kanban-cli/tests/help.rs` 覆盖，防止公开 command 行退化为空描述。

顶层 `kanban --help` 必须包含简洁 `Error codes:` section，覆盖当前公开退出码，帮助 operator 在终端直接发现 parse/runtime error code 边界。该 section 是 human-readable discovery surface；脚本仍应依赖 `--json` 下的 `error.code` 和 `error.exit_code`，不要解析 help 文案。

### 1.3 JSON output contract

所有公开 `--json` 输出使用顶层 envelope：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {},
  "meta": {}
}
```

`meta` 只在需要分页、details 或 diagnostics 时出现。`data` 可以是一个对象，也可以是对象数组；公共输出不得依赖裸 tuple、未命名数组位置、只有内部 id 的临时数组，或只回显输入参数。命令需要表达关系、删除或当前选择时，应返回命名 DTO，例如 `edge.parent`/`edge.child`、`step`、`board`。Task-like DTO 必须带可复制的 `ref`、`id`、`board_id` 或 `board_slug` 中的必要身份字段。

`board current --json` 和 `board use --json` 的 `data.board` 是完整 board 对象；调用方应读取 `data.board.slug`，不要把 `data.board` 当字符串。

#### JSON error output

当 `--json` 已被 clap 成功解析，且错误发生在运行期 service/IO 路径时，CLI 输出稳定错误 envelope 到 stdout，并使用对应 exit code：

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

`error.code` 是脚本可依赖的 ASCII 枚举；`message` 是本地化 human-readable 说明；`exit_code` 与进程退出码一致。运行期 `--json` 错误不写 stderr。

`error.code` 不应依赖业务校验 message 文案推断；普通业务层 `KanbanError::InvalidInput` / `InvalidStatus` 都返回稳定 `invalid_input`。已通过 clap 解析后的用户配置 TOML 解析失败也属于 `invalid_input`：例如 `kanban --json config show` 读取 malformed `.kb/config.toml` 或 `$XDG_CONFIG_HOME/kanban/config.toml` 时，输出 runtime JSON error 到 stdout、退出 2、不写 stderr，并且不打开、初始化或创建 SQLite DB。仅对无结构化层外错误（IO、路径、异常第三方 text）以及穿过 `InvalidInput` 的 SQLite/maintenance lock sentinel 使用降级文本分类作为补充，例如 `sqlite_busy`。

参数解析错误发生在 clap 解析阶段，仍由 clap 输出 stderr 并退出 2；这类错误不输出 JSON envelope。没有 `--json` 时，运行期错误继续输出 human-readable stderr。

### 1.3.1 JSONL / NDJSON streaming boundary

JSONL/NDJSON 只适用于 streaming 或 record-oriented surfaces，例如 portable export/import、watch/event stream，或未来逐条输出的长流命令。该类输出必须满足：stdout 中每一行都是独立 valid JSON object，编码为 UTF-8，记录之间仅用 newline 分隔；human diagnostics、progress、warnings 和 runtime errors 不得混入同一个 stdout 数据流。

有限命令仍使用 `--json` 的 `{data, meta?}` 成功 envelope 或 `{error:{code,message,exit_code}}` runtime error envelope。JSONL/NDJSON 不替代有限命令 envelope，也不能成为未设计的全局 `--jsonl` 快捷方式。若某个命令支持 `--out -` JSONL stream，则它不得与 `--json` 共享 stdout；需要结构化错误时，必须在命令级定义 stream error policy，并用 line-by-line JSON、stdout/stderr purity 和退出码测试覆盖。

当前公开错误 code：

| `error.code` | Exit code | 含义 |
|---|---:|---|
| `generic_error` | 1 | 未分类通用错误。 |
| `invalid_input` | 2 | 参数已通过 clap 解析，但业务输入、值域或 validation 无效。 |
| `not_found` | 3 | board、task、label、step、run 等对象未找到。 |
| `invalid_transition` | 4 | 状态机拒绝该转换，或 required execution plan / steps 未满足。 |
| `claim_conflict` | 5 | claim/heartbeat/finish token 或并发 claim 冲突。 |
| `dependency_blocked` | 6 | 依赖未完成导致任务不能进入 ready/running。 |
| `sqlite_busy` | 7 | SQLite busy/locked 或维护/runtime lock 阻塞。 |
| `integrity_check_failed` | 8 | doctor/import/maintenance 发现 integrity 或 consistency hard failure。 |
| `storage_error` | 1 | 其它存储错误；不保证可按 SQLite lock/integrity 自动恢复。 |

### 1.4 Shell completions

```bash
kanban completions <shell>
kanban __complete <kind> [prefix]
```

`kanban completions <shell>` writes a completion script to stdout. Supported
shells:

```text
bash | zsh | fish | powershell | elvish
```

Static command and option completion is generated for all supported shells.
Bash and zsh scripts additionally include dynamic hooks that call the hidden
internal `kanban __complete` helper for DB-backed candidates:

- task refs for task, comment, event, run, and dependency commands;
- board slugs for `--board` and board identity arguments;
- status values for `--status`;
- comment kind values for `comment add --kind` (`note`, `decision`, `signal`).

`kanban __complete` is an internal newline-delimited helper for shell scripts
and tests. It accepts:

```text
task-ref | dependency-task-ref | board | status | comment-kind
```

The helper must be quiet for completion use: missing DB files, uninitialized
DBs, missing board config, or read/query failures return success with no
candidates and no stderr. Static completion generation itself does not open or
create the SQLite database.

### 1.5 Codex hooks

```bash
kanban hook codex install [--handler-command <command-prefix>] [--timeout 30] [--record-signals] [--json]
kanban hook codex status [--json]
kanban hook codex uninstall [--json]
kanban hook codex handle failure [--record-signals]
kanban hook codex handle task-create
```

`kanban hook codex` manages a Codex lifecycle hook for kanban-aware agent
feedback. Hooks are installed at the Codex user config path:
`$CODEX_HOME/hooks.json`, or `~/.codex/hooks.json` when `CODEX_HOME` is not set.
There is no project-scope install mode, because kanban is intended to provide
the same CLI-aware behavior across workspaces.

Hook prompt text is read from the user kanban config path:
`$XDG_CONFIG_HOME/kanban/codex-hooks.json`, normally `~/.config/kanban/codex-hooks.json`.
`install` creates this file with Chinese default prompts when it is missing, and
never overwrites an existing file. If the prompt file is missing, malformed, has
an unsupported `version`, or points a binding at a missing prompt alias, the
handler falls back to the embedded Chinese defaults instead of failing the Codex
hook.

`install` adds two managed `PostToolUse` command hooks under matcher `^Bash$`:
one for failed `kanban ...` command traces and one for successful
`kanban task create ...` follow-up advice. The managed command prefix defaults
to `kanban hook codex handle`; the installed commands are:

```bash
kanban hook codex handle failure --installed-by kanban-hook-codex [--record-signals]
kanban hook codex handle task-create --installed-by kanban-hook-codex
```

`uninstall` removes only hooks with the hidden marker
`--installed-by kanban-hook-codex` and preserves unrelated user hooks. Re-running
`install` is idempotent: it replaces the previous managed hooks before writing
the new ones.

`handle failure` and `handle task-create` are internal hook commands. They read
Codex hook JSON from stdin and emit either no output or a raw Codex hook
response object such as:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"systemMessage":"检测到 kanban CLI 命令失败。\n\n命令：kanban task list --bad-flag\n退出码：2\n\n继续调整。调整成功后，视情况 spawn fork_turns=3 的 kanban-signal-recorder native agent。"}
```

The `handle` subcommands deliberately do not use the normal `{ "data": ... }`
JSON envelope, because Codex consumes hook stdout directly. The public
management commands `install`, `status`, and `uninstall` do use the normal
`--json` envelope.

Prompt config schema:

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
      "failure.zh-default": "检测到 kanban CLI 命令失败。\n\n命令：{{command}}\n退出码：{{exit_code}}\n\n继续调整。调整成功后，视情况 spawn fork_turns=3 的 kanban-signal-recorder native agent。",
      "task_create.zh-default": "检测到 kanban task 创建成功。\n\n命令：{{command}}\n任务：{{task_ref}}\n\n请考虑为该 task 执行 label/signal follow-up；需要标签时先运行 `kanban label suggest {{task_ref}} --json`。不要自动写 label ontology，除非已有完整 suggestion snapshot 和明确 decision payload。"
    }
  }
}
```

Supported placeholders are deliberately small:

- `failure`: `{{command}}`, `{{exit_code}}`;
- `task_create`: `{{command}}`, `{{task_ref}}`.

`stderr` and `stdout` are not prompt placeholders. For `handle failure
--record-signals`, they remain bounded internal evidence in the recorded generic
signal.

V1 behavior:

- non-`Bash` tools and Bash commands that do not invoke `kanban` are no-op;
- `handle failure` only reports failed `kanban ...` commands with a prompt
  rendered from `codex-hooks.json` or the embedded Chinese default;
- `handle failure --record-signals` also records a generic signal with
  `kind="agent_cli_failure"`, `source="kanban-hook-codex"`, and bounded command
  evidence;
- `handle task-create` only reports successful `kanban task create ...` commands
  with a label/signal follow-up prompt rendered from `codex-hooks.json` or the
  embedded Chinese default;
- the hook never silently starts a Codex native subagent and never writes label
  ontology automatically. It only injects advice; the active Codex session must
  decide whether to spawn a native agent or record ontology observations.

---

## 2. Exit Codes

| Code | 含义 |
|---:|---|
| 0 | 成功。 |
| 1 | 通用错误或未分类 storage error。 |
| 2 | clap 参数错误，或运行期 validation / invalid input。 |
| 3 | 未找到对象。 |
| 4 | 非法状态转换、required execution plan 或 required steps 未满足。 |
| 5 | claim/token/concurrent claim 冲突。 |
| 6 | dependency blocked。 |
| 7 | SQLite busy/locked 或 maintenance/runtime lock。 |
| 8 | integrity check failed 或 consistency hard failure。 |

---

## 3. Init

### 3.1 `kanban init`

初始化本地 DB、默认 board、默认 columns。该命令是幂等的；重复执行只会应用缺失 migration 并确保默认数据存在，不会重置或覆盖已有任务数据。`--force` 是兼容旧脚本的 no-op，不改变 `init` 行为。

```bash
kanban init
kanban init --db .kb/kb.db
kanban init --force
```

`--force` 是 deprecated compatibility no-op：保留用于兼容旧脚本，不改变 `init` 行为，不执行 reset/overwrite，也不会绕过 migration 或 schema 校验。

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
    "db_path": "/home/alice/.local/share/kb/kb.db",
    "board": "default"
  }
}
```

---

## 4. Board Commands

### 4.1 List boards

```bash
kanban board list [--include-archived]
```

### 4.2 Create board

```bash
kanban board create <slug> --name <name> [--description <text>]
```

Example：

```bash
kanban board create agent-work --name "Agent Work"
```

### 4.3 Show board

```bash
kanban board show <slug>
```

### 4.4 Use board

```bash
kanban board use <slug-or-id>
```

Writes:

```toml
board = "agent-work"
```

to `.kb/config.toml` in the current directory.

### 4.5 Current board

```bash
kanban board current
```

Shows the resolved active board after applying `--board`, `KB_BOARD`, project config, and fallback precedence.
Board resolution is independent from DB path resolution: `--db` / `KANBAN_DB` / `KB_DB` choose which SQLite database to open, while `--board` / `KB_BOARD` / `.kb/config.toml` `board` choose the board inside that database.

### 4.6 Archive board

```bash
kanban board archive <slug>
```

Archived boards are hidden from `kanban board list` unless `--include-archived` is passed. Ordinary task writes against archived boards are rejected. Audit history remains readable through task/event/run/comment history commands when the task or board can be resolved explicitly. Archiving a board with active `running` work is rejected; finish, block, or reclaim that work first.

---

## 5. Task Commands

### 5.1 Create task

```bash
kanban task create <title> [OPTIONS]
```

Options：

| Option | 说明 |
|---|---|
| `--description <text>` | Markdown 描述。 |
| `--description-file <PATH|->` | 从文件或 stdin (`-`) 读取 Markdown 描述；与 `--description` 互斥。推荐用于多行或包含 `$`、反引号、JSON 等 shell-sensitive 文本。 |
| `--status <status>` | 显式初始状态：triage/todo/scheduled/ready。 |
| `--assignee <name>` | assignee/worker profile。 |
| `--priority <int>` | Priority level `0..3`: `0` = P0 incident/blocker/must-handle-immediately, `1` = P1 near-term focus, `2` = P2 important follow-up, `3` = P3 ordinary backlog/low/default. Invalid values are rejected. |
| `--scheduled-at <epoch_ms>` | 计划时间，Unix epoch milliseconds。 |
| `--due-at <epoch_ms>` | 截止时间，Unix epoch milliseconds。 |
| `--max-retries <n>` | worker 失败或 reclaim 后最多重试次数。 |
| `--label <name>` | 创建时附加已存在 label，可重复；缺失的 board label 会拒绝整个 create。 |
| `--metadata <json>` | 扩展 JSON。 |
| `--metadata-file <PATH|->` | 从文件或 stdin (`-`) 读取扩展 JSON；与 `--metadata` 互斥。推荐用于避免 JSON shell quoting 问题。 |

Priority 只表达相对重要性和排序，不表达可 claim 状态。`ready` 才表示任务已被显式放入可执行队列；普通 ready 任务通常仍应是 P1/P2/P3，不能为了表示“下一批可做”全部标成 P0。P0 只用于 incident、当前目标 blocker 或必须立即处理的任务；若 P0 task 仍缺规格、排期未到或依赖未完成，它仍保持 `triage` / `scheduled` / `todo`，不能被 claim。

Examples：

```bash
kanban task create "修复 claim 队列阻断回归" --priority 0
kanban task create "实现状态机" --priority 1
kanban task create "补充文档示例" --priority 2
kanban task create "明早检查报告" --scheduled-at 1780640400000
kanban task create "修复 API 回归" --label backend --label p1
```

`--label` 只绑定当前 board 中已存在的 label identity。名称会先 trim；空白名称会被拒绝。
任一 label 缺失时，整个 create 返回 invalid input，且不会写入 `tasks`、`labels`、
`task_labels` 或 `task_events`。需要新 vocabulary identity 时，先显式运行
`kanban label create`，或使用 `kanban label add --create-missing` 这类明确的 identity
创建入口；task create 本身没有 create-missing 模式。

Human output：

```text
agent-work#12 [ready] P1 实现状态机 · plan: planned · steps: 0/0
```

JSON output：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "id": "t_01HX...",
    "board_id": "b_01HX...",
    "board_slug": "agent-work",
    "ref": "agent-work#12",
    "seq": 12,
    "status": "ready",
    "title": "实现状态机",
    "labels": []
  }
}
```

### 5.2 List tasks

```bash
kanban task list [OPTIONS]
```

Options：

| Option | 说明 |
|---|---|
| `--status <status>` | 按状态过滤，可重复。 |
| `--assignee <name>` | 按 assignee。 |
| `--label <name>` | 按 label 名称或 id 过滤，可重复；多个 label 使用 AND 语义。 |
| `--search <query>` | title/description 模糊搜索；task ref 形状按精确匹配处理。 |
| `--include-archived` | 包含 archived。 |
| `--limit <n>` | 限制数量。 |
| `--offset <n>` | 分页偏移。 |
| `--sort <field>` | `seq` / `title` / `status` / `position` / `priority` / `assignee` / `scheduled_at` / `due_at` / `created_at` / `updated_at`。降序可用 `<field>_desc`，也兼容 API 风格 `-<field>`。`priority` sorts P0 -> P3; `priority_desc` / `-priority` sorts P3 -> P0. |
| `--plan-needed` | 只列出 execution plan 仍为 `unplanned` 的 active tasks。 |
| `--has-steps` | 只列出至少有一个 step 的 tasks。 |
| `--incomplete-required-steps` | 只列出存在未完成 required step 的 tasks。 |
| `--plan-filter <filter>` | 可重复：`plan-needed` / `has-steps` / `incomplete-required-steps`。 |

Priority sort does not promote work into `ready`; it only orders tasks within the selected result set.

`--search` 对 task ref 形状使用精确匹配而不是文本 contains 匹配：
纯数字 `12`、`#12` 匹配 active board 内的 seq；`board#12` / `board/#12`
只在该 board 与当前列表请求的 board 相同时匹配；`t_...` 只匹配当前列表请求 board
内的 task id。其他文本仍执行 title/description 模糊搜索。

Examples：

```bash
kanban task list
kanban task list --status ready --status running
kanban task list --label backend --label p1
kanban task list --assignee agent-default --json
kanban task list --plan-needed
kanban task list --plan-filter incomplete-required-steps
```

### 5.3 Show task

```bash
kanban task show <task_ref>
kanban task show <task_ref> --details
```

默认人类可读输出仍是紧凑的单行 task 摘要；默认摘要面向扫描，保留可复制 ref、status、priority、title、labels，以及必要 plan/step 信号，不默认展示内部 `t_...` id：

```text
agent-work#12 [ready] P1 实现状态机 · plan: planned · steps: 0/0
```

`--details` 改变人类可读输出，按 `Task`、`Description`、`Plan`、`Schedule`、`Timestamps`、`Execution`、`Result`、`Metadata` 分组显示易读字段列表。可用时包含 task ref/id/status/title、完整多行 description、assignee、priority、labels、scheduled_at、due_at、created_at、updated_at、execution plan state、required/optional step counts、claim/run、result、metadata 以及其他 task snapshot 字段。
如果该 task 有 label ontology signals，details 输出还会追加紧凑的
`ontology_summary`，列出 signal/status/degraded/stale/action counts、aging 时间和
少量 sample signal ids。

`task show <task_ref> --json` 默认只返回 `{"data": TaskRecord}`。带 `--details`
时，`data` 仍是相同的 `TaskRecord`，但 envelope 会包含
`meta.details.ontology_summary`；没有 ontology signals 时该字段为 `null`。该 summary
只读，不改变 task、labels 或 ontology signal 状态。需要完整 review queue 时继续使用
`label ontology list/show/review`。

`task_ref` 支持：

- `t_...`：全局 task id，忽略 active board。
- `12`：当前 active board 内的 seq。
- `#12`：当前 active board 内的 seq；shell 中需要引号，例如 `'#12'`。
- `agent-work#12`：显式 board slug + seq。
- `agent-work/#12`：兼容 alias/#seq 形式。
- `b_01HX...#12`：显式 board id + seq。

裸 `12` / `#12` 依赖 active board；显式 `board#seq` 和 `t_...` 可跨 active board 使用。跨 board dependency 在当前版本中会被拒绝。

### 5.4 Update task fields

```bash
kanban task update <task_ref> [OPTIONS]
```

允许更新：

- title
- description
- assignee
- priority
- scheduled_at
- due_at
- max_retries
- metadata

不允许通过 update 修改 status；status 必须通过 transition command。允许字段仍由
shared service path 处理，因此修改 description、scheduled_at 等会影响 spec 或
schedule 的字段后，服务会根据 spec、schedule 和当前 dependencies 重新计算
active task 的目标状态并写入对应事件。Dependency edge 通过 `kanban dep`
命令修改；`max_retries` 只更新 retry policy，不是 status recompute 触发器。

Examples：

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

## 6. Transition Commands

### 6.1 Promote

```bash
kanban task promote <task_ref>
```

手动尝试 `todo/scheduled -> ready`。

### 6.2 Start / Claim

```bash
kanban task start <task_ref> [OPTIONS]
kanban task claim <task_ref> [OPTIONS]
```

`start` 是 `claim` 的人类友好 alias。

Options：

| Option | 说明 |
|---|---|
| `--ttl-ms <ms>` | claim TTL。默认 300000。 |

Output：

```text
Claimed t_01HX... token=ct_01HX...
```

JSON 返回 canonical claim snapshot：`data.task` 是闭合的 `ApiTask`，`data.run`
是闭合的 `ApiRun`，token 只允许出现在顶层 `data.claim_token`。下面仅节选 identity
与状态字段；实际对象还包含各自 schema 声明的其余字段：

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
    "claim_token": "ct_01HX...",
    "claim_expires_at": 1717520000000
  }
}
```

### 6.3 Heartbeat

```bash
kanban task heartbeat <task_ref> --claim-token <token>
```

Options：

| Option | 说明 |
|---|---|
| `--ttl-ms <ms>` | 延长 TTL。 |

显式 heartbeat API 保持兼容。除此之外，`running` task 的有效 task-scoped activity event 也会隐式刷新 lease，可作为 liveness signal；该隐式刷新不会再写 `task.heartbeat` event。board-level event 或没有 `task_id` 的 event 不触发续租。

### 6.4 Done / Complete

```bash
kanban task done <task_ref> --claim-token <token>
kanban task complete <task_ref> --claim-token <token>
```

Options：

| Option | 说明 |
|---|---|
| `--claim-token <token>` | active claim token。 |
| `--force` | 强制完成 running task；仅本地人工修复使用。 |

### 6.5 Submit Review

```bash
kanban task review <task_ref> --claim-token <token>
```

使 task 从 `running` 到 `review`。

### 6.6 Block

```bash
kanban task block <task_ref> [<reason>|--reason-file <PATH|->]
```

Options：

| Option | 说明 |
|---|---|
| `--claim-token <token>` | running task block 时需要。 |
| `--force` | 强制 block。 |
| `--reason-file <PATH|->` | 从文件或 stdin (`-`) 读取 block reason；与 positional `<reason>` 互斥。 |

### 6.7 Unblock

```bash
kanban task unblock <task_ref>
```

不会盲目进入 ready，而是根据 spec、schedule、dependencies 重新计算目标状态。

### 6.8 Reopen

```bash
kanban task reopen <task_ref> [--reason <text>|--reason-file <PATH|->]
```

只允许 reopen `done` task，reason 必填且不能为空，可用 `--reason-file <PATH|->`
从文件或 stdin 读取；它与 inline `--reason` 互斥。Reopen 会清空
`completed_at`，保留 `result_summary` / natural JSON `result`（持久层仍存于 `result_json`），并按 spec、schedule、
dependency 和 execution plan readiness 重新计算目标状态。

如果被 reopen 的 task 是其他 task 的 dependency parent，直接 child 中仅 `triage|todo|scheduled|ready` 会重新计算；`running|blocked|review|done|archived` 不隐式改写。

### 6.9 Reclaim

```bash
kanban task reclaim --expired
kanban task reclaim
```

当前 CLI reclaim 处理 active board 内 expired claims；裸 `kanban task reclaim` 与 `kanban task reclaim --expired` 等价。
JSON 输出固定为 `{"data":{"reclaimed":<u64>}}`，且拒绝未声明字段。

### 6.10 Archive

```bash
kanban task archive <task_ref>
```

Options：

| Option | 说明 |
|---|---|
| `--force` | 允许 archive running task，并关闭 active run。 |

---

### 6.10 Step / Execution Plan

```bash
kanban task step list <task_ref>
kanban task step add <task_ref> <title> [--body <text>|--body-file <PATH|->] [--link-task <task_ref>] [--position <n>] [--required|--optional]
kanban task step update <task_ref> <step_ref> [--title <text>] [--body <text>|--body-file <PATH|->|--clear-body] [--link-task <task_ref>|--unlink-task] [--position <n>] [--required|--optional]
kanban task step done <task_ref> <step_ref> [--note <text>|--note-file <PATH|->]
kanban task step skip <task_ref> <step_ref> [--reason <text>|--reason-file <PATH|->]
kanban task step reopen <task_ref> <step_ref> [--reason <text>|--reason-file <PATH|->]
kanban task step remove <task_ref> <step_ref>
kanban task step not-required <task_ref> [--reason <text>|--reason-file <PATH|->]
```

Step 是 execution plan 的一等结构化项目。它可以是纯文本步骤，也可以通过
`--link-task` 引用同一 board 内的普通 task 作为上下文。链接 task 不等于 dependency，
不会让 linked task 的状态自动完成 step。Step 自己的状态是 `todo`、`done` 或
`skipped`。

`step_ref` 支持 step id，也支持父任务列表里的 `S<n>` 序号。`add` 默认创建
required step；`--required` / `--optional` 互斥。Canonical human form is the bare
flag form, but the CLI also accepts bounded agent-generated values for this
specific flag: `--required true`, `--required=false`, and the matching
`--required=true` / `--required false` forms. Only literal `true` / `false` are
consumed as boolean values; ordinary positional text after `--required` remains
positional, and any other extra value remains a parser error. `--body-file
<PATH|->` 从文件或 stdin 读取长正文，与 `--body` 互斥；`update --clear-body`
也与 `--body-file` 互斥。`update` 只有在显式传 `--required` 或 `--optional`
时才改变 required flag。`done`、`skip` 和 `reopen` 必须记录说明文本。
`--note-file <PATH|->` 和 `--reason-file <PATH|->` 从文件或 stdin 读取长
note/reason，分别与 inline `--note` / `--reason` 互斥。

Human list 输出示例：

```text
Execution plan: planned
Required steps: 1/2 done-or-skipped
Optional steps: 1

S1 st_01HX... [done] required pos=1024 Write tests
S2 st_01HY... [todo] required pos=2048 link=default#13 Verify desktop UI
S3 st_01HZ... [todo] optional pos=3072 Release notes
```

`task step not-required` 只在没有 steps 时可用；它记录 reason 并解除 ready/claim 的
execution-plan gate。已有 step 的 task 不能标记为 `not_required`。

---

## 7. Dependency Commands

```bash
kanban dep add <parent_ref> <child_ref>
kanban dep remove <parent_ref> <child_ref>
kanban dep list <task_ref>
```

`--json` 输出使用 hydrated dependency DTO。`dep list --json` 返回以查询 task 为中心的 snapshot：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_child",
      "board_id": "b_default",
      "board_slug": "default",
      "ref": "default#2",
      "title": "child",
      "status": "todo"
    },
    "parents": [
      {
        "id": "t_parent",
        "board_id": "b_default",
        "board_slug": "default",
        "ref": "default#1",
        "title": "parent",
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
          "title": "parent",
          "status": "done"
        },
        "child": {
          "id": "t_child",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#2",
          "title": "child",
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

Human output for add/remove is Chinese-first:

```text
已添加依赖：default#1 -> default#2
已移除依赖：default#1 -> default#2
```

添加 dependency 后：

- 如果 child 当前是 `ready` 且 parent 未完成（不是 `done` 或 `archived`），child 降级为 `todo`。
- parent 完成、归档或 dependency 移除后，child 保持 `todo`；需要 `kanban task promote <task_ref>` 才显式进入 `ready`。归档 parent 不会删除 dependency edge。
- parent 从 `done` reopen 后，直接 child 中仅 `triage|todo|scheduled|ready` 会按 readiness 重算；`running|blocked|review|done|archived` 不隐式改写。
- 重复添加同一 parent/child edge 是 idempotent no-op：不追加新的
  `dependency.added` event，也不再次触发 child 状态重算。
- 如果产生环，返回 exit code 6 或 invalid input。
- 当前版本拒绝跨 board dependency，即使 parent/child 通过全局 `t_...` 或显式 `board#seq` 解析成功。

`task list/show --json` 返回 derived dependency fields：`dependency_blocked`
和 `unfinished_parent_count`。未完成 parent 指状态不是 `done` 或 `archived` 的 parent；这些字段用于区分仍被未完成 parent 阻塞的 `todo`
与已解除依赖但尚未人工 promote 的 `todo`。

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
kanban label semantics delete <label> --expected-semantics-hash <hash> [--reason <text>|--reason-file <PATH|->] [--json]
kanban label atoms list [--json]
kanban label atom explain <atom-id-or-content-hash> [--json]
kanban label atom-index status [--vector-config <toml>] [--json]
kanban label atom-index rebuild [--vector-config <toml>] [--json]
kanban label atom-index query <text> [--polarity positive|negative] [--limit 24] [--vector-config <toml>] [--json]

`label atom-index status`、`rebuild` 和 `query` 复用 vector TOML 解析规则：显式 `--vector-config`/`--config` 优先，其次是最近项目 `.kb/config.toml`，最后是全局 config。helper argv 只在显式传入 `--vector-config` 时附带该参数；省略时由 helper 按默认配置解析。
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
kanban label ontology confirm <signal_id>... [--reason <text>|--reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology reject <signal_id>... [--reason <text>|--reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology supersede <signal_id>... --by <signal_id> [--reason <text>|--reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology resolve <signal_id>... --no-change [--reason <text>|--reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology apply atom <signal_id>... --label <label> --kind applies-when|positive-example|excludes-when|negative-example [--text <text>|--text-file <PATH|->] [--reason <text>|--reason-file <PATH|->] [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology revert <action_id> [--reason <text>|--reason-file <PATH|->] [--expected-current-hash <hash>] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --status passed|failed|partial [--reason <text>|--reason-file <PATH|->] --input <PATH|-> [signal_id]... [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --trusted --status passed|failed|partial [--reason <text>|--reason-file <PATH|->] [signal_id]... [--positive-control <TASK_REF>]... [--positive-control-waiver <REASON>|--positive-control-waiver-file <PATH|->] [--vector-config <toml>] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--actor-type user|agent] [--agent-type <type>] [--json]
```

Label semantics/proposal/ontology 命令中的 `--reason-file <PATH|->`、
`--retarget-reason-file <PATH|->`、`--text-file <PATH|->` 和
`--positive-control-waiver-file <PATH|->` 从文件或 stdin 读取对应长文本，并与同名
inline 参数互斥。`label atom-index query <text>` 的 `<text>` 是短查询标量，不提供
file 输入；需要持久 ontology evidence 时使用 `label ontology record --input <path|->`
或 `label ontology validate --input <PATH|->`。

`label create` 创建当前 board 作用域内的 label；如果同一 board 已存在同名
label，返回已有 label。`label add` 接受 task ref 和一个或多个 label 名称；默认
只绑定 task 所属 board 上已经存在的 canonical label。缺失 label 会返回
invalid input，并提示先用 `label create`、`label bootstrap`、proposal/adoption
路径创建，或在明确接受只创建 canonical identity 的情况下传 `--create-missing`。
`--create-missing` 只创建 `labels` identity 并绑定 task，不生成 `label_semantics`
或 `label_atoms`；JSON 输出改为 `{ "task": <TaskRecord>, "created_labels": [...] }`。
`label remove` 接受 task ref 和 label 名称或 id。空白 label 名称会被拒绝。

`label delete <label>` 删除当前 board 上的 canonical label identity，区别于
`label remove <task_ref> <label>` 的 task-level 解绑。Label identity CRUD 不属于
ontology ledger；create/delete 只写普通 board/task event，不写 ontology mutation
action。默认情况下，如果 label 仍绑定任何 task，会拒绝删除并报告绑定数量；显式传
`--force` 时只移除 task bindings 后删除空 label identity。若 label 仍有
`label_semantics` 或 `label_atoms`，即使传 `--force` 也会拒绝；必须先用
`label semantics delete --expected-semantics-hash <hash> --reason <text>` 清空语义。
JSON 返回 `{ "label": <LabelRecord>, "forced": bool, "removed_task_bindings": n,
"removed_semantics": false, "removed_atoms": 0 }`。删除 canonical label 不改变 task
status；被删除 label 会从 `label list`、`task show/list` 的 labels 和后续 suggest truth
中消失。

Label 变更对 task-label 关联保持幂等。只有关联实际变化时，才追加
`task.label.added` / `task.label.removed` event；该操作不改变 task status。
批量 `label add` 会先验证所有 label 名称；如果任一 label 为空白、非法或缺失且未传
`--create-missing`，不会创建 canonical label，也不会留下部分 task-label 绑定。
显式创建模式与单 label add 相同，只创建缺失的 canonical identity，并在输出中列出
本次新建的 labels。

`label bootstrap` 是一次性 new-label adoption helper：在同一 transaction 内创建
当前 task 所属 board 上缺失的 canonical label，或复用没有既有 semantics 的同名
label，写入该 label 的 `label_semantics`，同步重建 SQLite `label_atoms`，标脏派生
的 label atom vector index，并把该 label 绑定到 task。`<label>` 按名称解析；空白
名称会被拒绝。语义输入会 trim 并丢弃空白值，且必须至少提供 `description` 或一个非空
语义数组值。

Bootstrap 默认不会覆盖已有 `label_semantics`。如果同名 label 已经有 semantics，
命令会失败，并要求改用专用 semantics mutation 或 proposal/adoption 路径；重复执行
同一 task/label 只在目标 label 仍无 semantics 时保持 task-label 绑定幂等。JSON
返回 `{ "task": <TaskRecord>, "semantics": <LabelSemanticsRecord>, "verification": null|<Verification> }`。

当前 no-heavy CLI build 已把 label suggestion/proposal、bootstrap staged verification 和
label atom status/rebuild/query 接到 vector helper subprocess adapter；`kanban vector ...` 仍保留
raw chunk / label-atom 查询入口，helper 内部用 label atom 专用 command 处理
`lancedb_label_atoms`，不复用 chunk store status 伪装 label atom 状态。

传入 `--verify` 或 `--vector-config <toml>` 时，CLI 使用 pre-commit staged
verification：先在 canonical DB transaction 外读取当前 task、target label state 和
board ontology digest，并在隔离的临时 atom store 中加载当前 atoms 与 candidate atoms。
随后对来源 task 运行非 degraded `label suggest`，要求新 label 出现在
`selected_labels` 或 `candidates`，且 score 至少达到 `--min-verify-score`（默认
`0.50`）。rebuild、suggest、threshold、provider 或临时 store 失败时不会写
canonical label、semantics、atoms、task-label binding、ontology action、event 或 dirty
marker。如果 vector helper/provider 不可用会返回明确的 verification error；需要离线验收时也可改走 external attestation `--input` 路径。

验证通过后 CLI 才开启短 `BEGIN IMMEDIATE` transaction，重算 task suggest-input hash、
target label state 和 board ontology digest；任一值变化会返回 conflict 且零写入。成功
路径在一个 transaction 中写 canonical label/semantics/atoms、task binding、普通
task-label event、一个 `bootstrap_label` root ontology action 和对应 added atom
effects。Verification summary 会写入 root action change snapshot 和 CLI output；它不等同于
post-commit trusted validation。无可用 vector provider 时，验证会在写入前失败；不需要
本地 vector 验证时省略 `--verify` 和 `--vector-config`。

示例：

```bash
kanban label create backend --color blue
kanban label delete old-label --json
kanban label delete old-label --force --json
kanban label semantics delete old-label --expected-semantics-hash sem_abc123 --reason "Retire obsolete semantics before deleting identity" --json
kanban label bootstrap default#12 database --description "Database persistence work" --applies-when "touches SQLite migrations" --positive-example "new table migration" --json
kanban label bootstrap default#12 database --description "Database persistence work" --applies-when "touches SQLite migrations" --positive-example "new table migration" --vector-config .kb/vector.toml --min-verify-score 0.50 --json
kanban label add default#12 backend
kanban label create api
kanban label add default#12 backend api
kanban label add --create-missing default#12 scratch-label --json
kanban label remove t_01HX... backend
kanban label list --json
```

人类可读输出使用紧凑 label 行：

```text
backend l_01HX... color=blue
```

Task 的人类可读摘要如果存在 labels，会在末尾追加方括号标签列表：

```text
default#12 [ready] P1 修复 API 回归 [backend,p1] · plan: planned · steps: 0/0
```

`label suggest` 返回 task-level label suggestions。带内置 label atom vector store 的
构建会把 task title +
description embedding 作为 query，使用 `lancedb_label_atoms` 按残差多轮检索正向
label atoms，并用原始 query 检索负向 atoms 做 penalty / suppression。solver 在
label group 层执行 Group OMP 选择，再用选中 label 的 top positive atom vectors 做
non-negative refit；`coverage` / `residual_norm` 来自该 atom-level fitted vector，
其中 `coverage = clamp(1 - residual_norm, 0.0, 1.0)`，因此二者不是两份独立
证据；`coverage_cosine` 是原始 query 与 fitted vector 的 cosine similarity，
可作为独立补充指标。
候选 label 只有在 tentative refit 后带来足够 residual norm 降幅才会进入结果；
coverage 或 residual norm 达到停止阈值后，solver 会提前停止而不是凑满
`--max-selected-labels`。candidate group 与已选 label 语义向量过度相似时会被跳过，
以减少重复语义 label 同时出现在 selected labels；这不会合并或删除 canonical
labels。
`needs_new_label` 是兼容字段，只表示存在需要人工 review 的 label coverage
诊断；具体原因必须读取 `reason_codes`，例如 `no_selected_labels`、
`coverage_below_threshold`、`residual_above_threshold`、`unexplained_residual`，
或 degraded 相关原因。不要把 `coverage` 与 `residual_norm` 重复计票，也不要仅凭
`needs_new_label=true` 创建 vocabulary；必须结合 `reason_codes`、evidence atoms、
diagnostics 和人工语义判断。
它不会自动创建新 label，也不会写入 new-label proposal。应用建议时仍使用现有
`label add <task_ref> <label>...` / API attach 流程。

默认 no-heavy CLI 通过 vector helper adapter 运行 label vector 查询；helper/provider 不可用时命令成功返回
degraded 结果而不是失败，且 `needs_new_label=false`。`--vector-config`
使用与 `kanban vector configure/status` 相同的 TOML 解析规则，并把解析出的 embedding model 传给 helper 查询。`LabelAtomHit.distance`
保留 LanceDB `_distance` 的原始语义；suggestion / proposal 的 score 只根据返回
atom vector 与当前 query/residual 在本地计算 cosine similarity，不从 distance 推导。

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

Human output 简洁列出建议 label、score、weight、already_applied；degraded 时追加
diagnostics 行。

`--limit` 只控制最终输出中 `selected_labels` / `candidates` 的最大条数，不会收窄
solver 内部搜索能力。内部能力由 `--candidate-limit`、`--atom-limit` 和
`--max-selected-labels` 分别控制：候选 label group 数、每轮 atom vector 检索上限、
以及最多进入 non-negative refit 的 label 数。所有 limit 参数都必须是
`1..=1000`；`--min-score` 必须在 `0..=1`。

Label ontology 的长期 regression corpus 目前是本地测试基础设施，不是一个会写生产
DB 的 CLI mutation 流程。修改 label solver、semantics/atom 生成、trusted validation
或重要 label ontology 时，可以运行：

```bash
just test-p kanban-sqlite label_ontology_longitudinal_regression
```

该测试在临时 SQLite DB 中建立固定 important labels、known positive tasks 和
negative-control tasks，重建内存 label atom index，保存 baseline `label suggest`
结果，再模拟一次过宽 atom 变更并比较 selected labels、score 和 evidence atoms。它会
断言正常 corpus run 不修改 `labels`、`task_labels`、`label_semantics`、
`label_atoms` 或 ontology ledger rows；真实项目 corpus 应在积累稳定任务后逐步扩展，
但不应成为每个日常 task label 绑定的默认必跑步骤。

`label semantics` 管理当前 board 上已有 label 的语义字典。`<label>` 接受 label
name 或 `l_...` id。`upsert` 默认是 patch：`--description` 只在提供非空值时覆盖当前
description，数组参数会追加到对应集合，`--remove-*` 只删除匹配的既有文本；未提供的
字段不会被解释为清空。传 `--replace` 时才执行完整替换，此时未提供的数组会成为空
数组，并且不能同时传 `--remove-*`。`--expected-semantics-hash <hash>` 是
compare-and-swap guard：hash 不等于当前 semantics hash 时返回 conflict 且不写入。
`--reason` 和 `--source-signal` 会进入 `update_semantics` ontology action；即使没有
source signal，constructive semantics mutation 也会在同一 transaction 写入 before/after
hash、change snapshot 和 actor provenance。`upsert` 会写入 `label_semantics` 并同步重建
该 label 的 `label_atoms`，随后标脏派生的 label atom vector index。数组参数可重复；空白值会被
trim 后丢弃。生成 atoms 时，有 description 的 label 会生成一个 canonical
`description` atom：`label: {name}\ndescription: {description}`；没有 description 时
才使用 `name` fallback atom。atom text 会进一步规范化 whitespace：每个非空行内部
collapse，canonical 行分隔保留。同一 label 下相同
`polarity + kind + normalized_text` 的 atom 会去重并保留首次 ordinal，`id` /
`content_hash` 不包含 ordinal，因此只调整数组顺序不会改变同一文本 atom identity。
`delete` 是 CAS-protected semantics clear：必须传
`--expected-semantics-hash <hash>` 和非空 `--reason <text>`。它删除该 label 的
semantics 与 SQLite atoms，但不删除 canonical label identity 或 task-label 绑定；同一
transaction 会写一个 `update_semantics` root ontology action，after snapshot 为空，
并为实际 removed atoms 写 `removed` atom effects，随后标脏 label atom index。Hash
mismatch 时 canonical、action、effects 和 dirty state 全不变。成功返回
`{ "data": { "deleted": true } }`。需要在清空后删除 label identity 时，先 clear
semantics，再执行 `label delete`。

`label atoms list` 读取 SQLite `label_atoms` materialized projection。这些 atoms 来自
`label semantics upsert`、`label bootstrap`、`label ontology apply atom` 或接受 label
proposal 后生成的 semantics；它们是 `lancedb_label_atoms` 派生索引的输入，不是派生索引本身。

`label atom explain <atom-id-or-content-hash>` 是 `label atoms explain` 的单数别名，
按当前 board 的 atom id 或稳定 `content_hash` 解析现有 atom，并返回当前 atom、
canonical semantics、provenance actions、supporting signals/source tasks 和
validation history。当前 atom 存在但没有 ontology provenance action 引用其 id 或
content hash 时命令成功返回 `legacy_untracked=true` 和 `legacy_reason`；未知 id/hash
返回 not found。JSON 输出是 `LabelAtomExplainRecord`，包含 `query`、`atom`、
`current_semantics`、`provenance_actions`、`supporting_signals`、
`validation_history`、`legacy_untracked` 和 `legacy_reason`。由于 content hash 不含
ordinal，semantics rebuild 后同语义 atom 的 id 改变时仍可用 content hash 解释历史。

`label atom-index status` 返回 label atom vector index 的状态。未配置 provider 或 helper
不可用时仍成功返回 disabled/degraded 状态。JSON 保留兼容字段 `message`，并返回结构化
`diagnostics: string[]`、`dirty: boolean | null`、`board_dirty: boolean | null`；调用方应使用
结构化字段判断 dirty/error，而不要解析 `message` 文案。`status` 通过 helper 的
`label-atoms-status` command 读取 `LANCEDB_LABEL_ATOMS_STORE` 与 `label_atom_index_boards` 语义；
`query` 通过 helper adapter 查询 label atom vector index，`--polarity` 只接受 `positive` 或
`negative`，human 输出和 JSON hit 都把 LanceDB `_distance` 暴露为 `distance`。`rebuild` 通过
helper 的 `rebuild-label-atoms` command 重建 label atom 派生索引；helper/provider 不可用时返回显式
error，不修改 SQLite canonical label truth，也不标记 chunk store success。

`kanban vector query-label-atoms` 是公开 raw helper 查询入口，支持 text 查询和 raw vector 查询。
输入必须且只能选择一种：positional `<text>`、`--text-file <PATH|->`、`--vector-json <JSON>` 或
`--vector-json-file <PATH|->`。`-` 表示从 stdin 读取。示例：
`kanban vector query-label-atoms --text-file query.txt [--polarity positive|negative] [--limit N] [--embedding-model MODEL] [--vector-config <toml>]`，或
`kanban vector query-label-atoms --vector-json-file vector.json [--include-vector] [--embedding-model MODEL] [--polarity positive|negative] [--limit N]`。
`--include-vector` 只对 helper 支持的 raw vector/vector hit 输出有意义。

`label propose` 是独立的新 label semantics 提案流程，不复用或改变 `label suggest`。
它先读取当前 task-level label suggestions 的 `coverage` / `coverage_cosine` / `residual_norm` /
top1 existing label。没有 `--proposal-json` 时默认 provider 不可用，命令成功返回
degraded attempt，不创建 canonical label、`label_semantics`、`label_atoms` 或
`task_labels`。日常 label suggestion 不依赖该 proposal provider。
`--limit` 只截断 proposal attempt 中复用的 suggestion 输出；`--candidate-limit`、
`--atom-limit`、`--max-selected-labels`、`--min-score` 会在 proposal 持久化前调节底层
label suggestion solver，用于计算 coverage、coverage_cosine、residual_norm 和 top1 existing label。
`--vector-config` 使用与 `label suggest` 相同的 TOML 解析规则。默认 no-heavy CLI
通过 vector helper adapter 运行 residual validation；未配置或 helper/provider 不可用时保持
degraded fallback，不写入普通 label 或 task-label 关联。

Provider boundary：CLI 当前只使用 disabled provider 或 `--proposal-json` 显式传入的
本地/offline candidate。真实 LLM provider 不属于 `kanban-sqlite`；未来若接入本机
AI/runtime，应在 CLI/local runtime 或独立 AI crate 中实现 `LabelProposalProvider`
adapter，再把 candidate 交给 SQLite service 做 deterministic validation 和 persistence。

`--proposal-json` 提供本地/offline provider 输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "database",
  "description": "Database persistence work",
  "applies_when": ["touches SQLite migrations"],
  "excludes_when": ["UI-only polish"],
  "positive_examples": ["new table migration"],
  "negative_examples": ["CSS-only tweak"]
}
```

数组字段缺省时按空数组处理。`name` 不能为空，且 description 或任一语义数组至少
需要提供一个非空值。只有当前启发式 coverage 不足时才持久化 proposal。与现有
label 发生 normalized-name 冲突的候选会写成 `rejected` proposal，并在 diagnostics
中返回 `near_duplicate_label_conflict`；该 normalized-name 检查忽略大小写、空白
和标点，是 deterministic near-duplicate heuristic。
coverage 不足的候选还会执行残差 top1+margin 校验：候选语义的 residual score
和现有 label top1 都按返回 atom vector 在本地计算 cosine similarity，不从
LanceDB distance 推导；候选必须超过现有 label top1，且超过幅度达到固定 margin。
校验失败时 attempt 仍会把候选持久化为 `rejected` proposal，diagnostics 包含
`label_proposal_residual_top1_failed` 或
`label_proposal_residual_margin_insufficient`，用于审计为什么没有进入可接受状态。
如果 residual validation 不可用或 degraded，且没有明确通过 top1+margin 校验，
attempt 返回 `degraded=true`、`proposal=null`，不新增 proposal row，也不创建
canonical label、`label_semantics`、`label_atoms` 或 `task_labels`；diagnostics 包含
`label_proposal_residual_validation_unavailable` 和具体原因。
传入 `--source-signal <los_...>` 时，proposal 创建成功后会在同一 transaction 写入
`create_label_proposal` ontology action，并通过 action-signal links 记录该 proposal
由哪些 confirmed vocabulary-gap signals 支持；proposal row 与 provenance action
要么同时写入，要么一起回滚。Source signals 默认必须是同一 board 上 `confirmed`
的 `vocabulary_gap` + `bootstrap_label` signals，且 normalized `proposed_label_name`
必须等于 proposal name。`--actor-type` / `--agent-type` 控制该
`create_label_proposal` action 的 actor provenance；actor name 仍来自全局 `--actor`。
确实需要把 confirmed same-board source signal retarget 到该 proposal 时，必须同时传
`--allow-retarget` 和非空 `--retarget-reason <text>`；reason 和 source signal 原始
target/proposed label 会写入 `change_json.retarget_override`。Override 不放宽
board/status 要求。

`label proposals accept` 只接受 `proposed` proposal。accept 与单 task bootstrap 共用
同一个 adoption primitive：创建 canonical label、`label_semantics` 与 `label_atoms`，
标脏 label atom index，并写入 `bootstrap_label` ontology action；proposal row、
canonical writes 和 action provenance 要么同一 transaction 成功，要么一起回滚。它不会自动
给来源 task 写入 `task_labels`。未传 `--source-signal` 时仍会记录 bootstrap action，
只是没有 action-signal links；传入 `--source-signal <los_...>` 时会通过 links 记录该
new-label bootstrap 的 signal provenance，且这些 source signals 必须是同一 board 上的
`confirmed` signals。`--actor-type` / `--agent-type` 控制该
`bootstrap_label` action 的 actor provenance；actor name 仍来自全局 `--actor`。
默认是 `user`。`--actor-type agent` 必须提供非空 `--agent-type`；`user` 不能提供
`--agent-type`。Source signals 默认还必须是 `vocabulary_gap` +
`bootstrap_label`，且 normalized `proposed_label_name` 必须等于 proposal name。
如果 proposal 已有 `create_label_proposal` action，accept 产生的 `bootstrap_label`
action 会把 `parent_action_id` 指向该 creation action，形成 proposal creation ->
bootstrap acceptance 链路。
确实需要把 confirmed same-board source signal retarget 到该 proposal 时，必须同时传
`--allow-retarget` 和非空 `--retarget-reason <text>`；该 reason、source signal 原始
target/proposed label 和最终 proposal/result label 会写入 bootstrap action
`change_json.retarget_override`。Override 不放宽 board/status 要求。`label proposals reject`
标记 proposal 为 `rejected`，不接受 `--source-signal`。accepted/rejected proposal 不能再次决策。

`label ontology record` 记录一次 label 判断 observation 并写入其中的 child signals。
推荐输入边界是：工具采集或接收未改写的 `label suggest` snapshot，service 从 snapshot
派生 coverage、residual、degraded、diagnostics 等 observation metrics；agent 只提交
候选、最终判断、signals、candidate atom 和 rationale。CLI 可以用
`--capture-suggest` 在 record 前用同一组 suggest options 运行一次真实 `label suggest`，
也可以用 `--suggestion-snapshot <path|->` 读取已保存的原始 suggest JSON。snapshot
可以是直接的 suggest response，也可以是带 `data` wrapper 的 JSON response。

`--input` 只接受 contract-owned natural JSON shape；旧 `_json` compatibility siblings
（例如 `diagnostics_json`、`related_labels_json`）会作为 unknown field 拒绝。新调用方不应重复手写
`suggest_coverage`、`suggest_residual_norm` 或 `diagnostics`。如果 snapshot 中已有
这些字段而输入又提供冲突的标量或 diagnostics，命令会失败。Service 会读取当前 task
snapshot、解析 target label ref、计算 normalized proposed label name、signal key 和
candidate atom content hash；observation 同时保存完整审计用
`task_snapshot_json.content_hash` 和只基于 label suggest 输入（normalized title +
description）的 `suggest_input_hash`。它只写 ledger，不修改 `task_labels`、
`label_semantics`、`label_atoms`、label atom index 或 proposal。

Signal 输入会在写入前做 ontology contract 校验。`candidate_atom` 的
`applies_when` / `positive_example` 只能使用 `positive` polarity，
`excludes_when` / `negative_example` 只能使用 `negative` polarity。
`add_positive_atom` 必须提供 target label 和 positive candidate atom；
`add_negative_atom` 必须提供 target label 和 negative candidate atom；
`update_semantics` 必须提供 target label；`bootstrap_label` 必须提供
`proposed_label_name`；`rename_label` 必须提供 target label 和
`proposed_label_name`；`split_label` / `merge_labels` 必须提供 target label 和非空
`related_labels`。Observation metric `suggest_coverage`、
`suggest_coverage_cosine`、`suggest_residual_norm` 以及 signal metric
`suggest_score` / `confidence` 必须是 finite `0.0..=1.0`；`suggest_rank` 必须为
`null` 或 `>= 1`。
`rename_label` / `split_label` / `merge_labels` 当前只作为 review signal proposed_action
保存，CLI 不提供写入 canonical structure mutation action 或 structure plan action 的命令；
旧 structure-plan rows 只读展示为 unsupported validation requirement。

使用已保存 suggest snapshot 的推荐输入形状：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
  "agent_candidates": [
    {"label": "cli", "reason": "The task changes CLI behavior."}
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
        "text": "extends CLI subcommands, command arguments, help output, or machine-readable JSON behavior"
      },
      "proposal": {},
      "agent_selected": true,
      "suggest_state": "candidate",
      "suggest_score": 0.08,
      "suggest_rank": 6,
      "final_selected": true,
      "rationale": "The task expands the CLI surface."
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

或者让 CLI 在记录前采集 snapshot：

```bash
kanban label ontology record default#42 \
  --input /tmp/default-42-record.json \
  --capture-suggest \
  --vector-config ./vector.toml \
  --json
```

`label ontology list` 默认只返回 `open` 和 `confirmed` signals。`--include-all`
返回完整历史；`--status`、`--kind` 可重复过滤，`--task`、`--label` 和
`--proposed-label` 用于按来源 task、目标 label 或候选新 label 查询。
`label ontology show` 返回 signal、observation 和关联 actions。`label ontology review`
是只读聚合 review queue 视图，默认只聚合 `open` 和 `confirmed` signals；传
`--include-all` 时包含 resolved/rejected/superseded 历史。`--group-by` 支持按
`label`、`candidate-atom`、`proposed-label` 或 opt-in `cluster` 聚合，`--limit` 限制返回 group
数量。`--json` 每个 group 返回聚合维度、key、相关 label / candidate atom /
proposed label、cluster key/reason（仅 cluster view 有值）、distinct task count、signal/status/degraded/action counts、score
summary、sample task refs、signal ids、action ids 和 proposal ids。排序优先使用
distinct task count，其次 confirmed count、latest signal time 和 key。

Review group 只表示一组 signals 共享同一个聚合键，不证明它们一定来自同一个根因。
`--group-by label` 使用 `target_label_id` 作为 key，缺失目标 label 时使用
`no-target-label`。`--group-by proposed-label` 使用 normalized proposed label name，
缺失候选新 label 时使用 `no-proposed-label`。`--group-by candidate-atom` 优先使用
`candidate_content_hash`；如果 signal 没有 candidate atom，则 key 会包含 signal kind、
target label 或 proposed label、以及 proposed action，例如
`no-candidate-atom|kind:vocabulary_gap|proposed:ontology ledger|action:bootstrap_label`。
这个 fallback 避免把不同 kind、不同 label 或不同 proposed action 的空 candidate
signals 合并到一个全局 bucket。
`--group-by cluster` 是一个只读 review-aid：它不写 canonical atoms，也不会确认、
应用、validate 或关闭 signal。cluster key 每次查询时从已有 signal 文本重建，优先使用
lexical-normalized candidate text，其次 proposed label，再其次 rationale，最后才退回到
kind/action/target/proposed-label scope 组合；所有 cluster key 都带有 signal kind、
proposed action、target label 和 proposed-label scope，避免跨 label/action/boundary 误合并；
`cluster_reason` 说明当前 key 的来源。

`task_count` 是 group 内 distinct source task 数，也是默认热度排序的第一依据；同一 task
上的多条 signals 仍只贡献一个 distinct task。`signal_count` 是原始 signal 行数，
用于判断一组里有多少审查项；它没有 denominator，不能解释为模型错误率、precision
或 recall。`degraded_count`、status counts、score summary 和 sample task refs 只是
reviewer 的排查线索。排序为 `task_count` desc、`confirmed_count` desc、
`latest_signal_at` desc、`key` asc；需要判断是否同一问题时，应继续查看 group 的
sample tasks、signal ids 和 `label ontology show` 详情。

`label ontology quality` 是只读 quality/analytics 报告。它从当前 board 的
`label_ontology_observations` 取得可审计 denominator，并从
`label_ontology_signals` 取得 raw disagreement counts；不会写入 task、label、
semantics、atoms 或 ledger action。JSON 输出包含：

- `denominator.source="label_ontology_observations"`、`observation_count`、
  `distinct_task_count`、agreement/degraded observation counts、时间范围和
  `sample_task_refs`。
- `disagreement.signal_count`、`disagreement.distinct_task_count`、`by_kind`、
  `by_status`。
- `rates.disagreement_task_rate`，只在 denominator 至少包含一个 agreement
  observation 时返回；只有 signals 的历史不会输出伪错误率。
- `precision_recall.available=false`，直到项目有带 expected labels 的独立评估
  cohort。raw signals 只能说明记录过分歧，不能单独证明 precision、recall、miss
  rate 或模型错误率。

Lifecycle commands 写入 action 并同步更新 signal status：

- `confirm`：`open` signal 进入 `confirmed`。
- `reject`：把 signal 标记为 `rejected`。
- `supersede --by`：把重复或过时 signal 标记为 `superseded`；写入前会沿
  replacement `superseded_by_signal_id` 链检查，拒绝会回到任一 source signal 的环。
- `resolve --no-change`：记录无需 ontology 修改的 resolution。

这些 lifecycle commands 只记录 review/status 变化，不接受 canonical mutation
provenance 字段。`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
`update_semantics`、`create_label_proposal`、`bootstrap_label`、`revert_ontology_mutation` 和 `validate` 等 action rows 只能由
`label semantics upsert`、`label ontology apply atom`、`label propose`、
proposal accept、`label bootstrap`、`label ontology revert`、`label ontology validate` 等专用命令/服务路径在同一
transaction 中写入。通用 action command 不能伪造 canonical before/after hash、
result atom/result label/result proposal 或 validation payload。
Lifecycle、apply atom、validate 和带 `--source-signal` 的 proposal accept 都支持
`--actor-type user|agent` 与 `--agent-type <type>`。这些 flag 只控制 ontology action
row 的 `created_by_type` / `agent_type`；action name 仍来自全局 `--actor`。默认
`--actor-type user` 且不写 `agent_type`。`agent` actor 必须提供非空 `--agent-type`，
`user` actor 带 `--agent-type` 会被拒绝。

`label ontology apply atom` 只接受 `confirmed` source signals。它会读取目标 label
当前 semantics，把泛化文本加入对应数组，走现有 semantics upsert/rebuild atoms 路径。
如果 canonical 内容实际新增 atom，会写入 `add_positive_atom` 或 `add_negative_atom`
action，记录生成 atom 的软引用、content hash、before/after hash、单份 change snapshot
和一个 `added` atom effect，并把 `validation_requirement` 置为 `required`。如果同内容 atom 已经存在，则写入
`adopt_existing_atom` provenance-only action，记录 existing atom 软引用、before/after
hash（相同）和 source signal links；该 action 不修改 semantics/atoms、不标脏 atom
index，`validation_requirement=none` 且 effective outcome 为 `not_required`。
默认要求所有带 `target_label_id` 的 source signals 都指向被修改 label；不匹配时拒绝
并列出 offending signal ids。Atom text 不需要逐字等于 source signal 的 candidate
text，reviewer 可以写更泛化的 canonical atom。确实需要 retarget confirmed same-board
signals 时，必须传 `--allow-retarget` 和非空 `--retarget-reason <text>`；action
`change_json.retarget_override` 会记录 reason、source signal 原始 target/proposed label
和最终 target label。Override 不放宽 board/status 要求。
该命令只有在 canonical atom 实际新增时才标脏 label atom index；vector index rebuild
和后续 suggest 验证仍是第二阶段。

`label ontology revert <action_id>` 为已提交的 label-scoped canonical ontology mutation
追加 `revert_ontology_mutation` action，并把目标 label semantics 恢复到被撤销 action
的 `canonical_before_hash` / `change_json.before` snapshot。当前只支持
`add_positive_atom`、`add_negative_atom` 和 `update_semantics`；不处理 bootstrap 的
label identity 或 task binding 回滚。为避免覆盖后续修改，命令要求当前 canonical
semantics hash 仍等于目标 action 的 `canonical_after_hash`；传
`--expected-current-hash <hash>` 时还会先做调用方持有快照的 CAS 检查。成功后会写入
append-only revert action，`parent_action_id` 指向被撤销 action，复制原 action 的
source signal links，记录 before/after revert snapshot，为本次 revert 实际 added/removed
atoms 写 atom effects，标脏 label atom index，并把 `validation_requirement` 置为
`unsupported`。原 mutation
action 不会被修改或删除。

所有 canonical semantics/atom mutation transaction 都遵循 one-root-action 合同：同一
transaction 只写一条 root mutation action，`change_json` 只保存一次 before/after
semantics snapshot；实际新增或删除的 atoms 通过
`label_ontology_action_atom_effects` 记录 `added` / `removed` effects。Description-only
patch 会写一条 root action 和零 atom effects；no-op patch 不写 action/effects，也不标脏
index。Atom explain 优先使用 effect rows；legacy per-atom actions 仍保持兼容读取。

`label ontology validate` 为一个 mutation action 追加 `validate` action。Parent action
必须是同一 board 上 `validation_requirement=required` 的 canonical mutation action，
并携带 canonical result evidence（例如 atom/result label/proposal 引用、canonical hash
和非空 change snapshot）。Parent action 的 `validation_status` 是历史兼容字段，不再单独
表达“是否需要验证”；读取时通过 reducer 暴露 effective outcome：
`not_required|unsupported|pending|passed|failed|partial`。

普通 `--input` 路径是 external attestation：CLI 读取调用方提供的 JSON，service 只把
supplied payload、source signal case 摘要、task snapshot/suggest input hash 对比和
parent action 结果引用包装进 validation envelope。公共 supplied/collected payload
只保存一次在 top-level `manual`；generated `cases[]` 使用 `after.manual_case_ref`
引用 `manual.cases[]` 中对应 signal 的 evidence，不在每个 case 中重复整份 payload。
该路径可记录 `failed` / `partial` 诊断，但不能把 `passed` 写成 trusted proof；即使 JSON 自称
`evidence_type="automated"`，`--status passed` 也会被拒绝，linked signals 不会被
关闭。

`--trusted` 路径才是 trusted automated validation。它不接受 `--input`，也不接受调用方
手写 trusted evidence JSON；CLI 只能走内置 collector。Trusted 表示工具在当前 parent
action、source signals、canonical hash、atom index generation 和指定 cases/controls 上做了
机械采集和检查，不表示 ontology 在全局语义上正确。CLI 必须有可用 label atom vector
workflow adapter（当前 no-heavy CLI 尚未接入；旧内置 `vector-lancedb` build 需可解析 `--vector-config` 或默认 config），先在 SQLite transaction 外 rebuild atom index，再用同一
`--limit` / `--candidate-limit` / `--atom-limit` / `--max-selected-labels` /
`--min-score` options 对 linked source signals 重新运行 `label suggest`，由工具生成
`evidence_type="trusted_automated"`、`collector.source="label_ontology_validate_trusted"`、
`embedding_model`、`solver_options`、clean `index.status` / `index.generation` 和
per-signal `cases[]`。写 action 时 service 会在短 transaction 内重新核验 parent action、
source signals、canonical after hash、atom index dirty/error 状态和 generation，防止
查询后 canonical 或 derived state 已变化。dirty/error/disabled index、缺失 generation
或 stale generation 都不能产生 trusted passed。

`--positive-control <TASK_REF>` 与 `--positive-control-waiver <REASON>` 只用于
negative atom trusted validation，且二者互斥；非 negative parent 携带这些参数会被拒绝。
waiver 只能由 `--actor-type user` 提交，reason 必须非空。Negative atom parent 若两者都
缺失，会在 collection 前失败。

`cases[]` 的 `case_type` 必须匹配 parent action：`positive_atom`、`negative_atom`
或 `bootstrap_label`。Positive atom validation 要求 `after.degraded=false`、
result atom id/content hash 出现在 `after.evidence_atoms[]`、target label selected
或 score >= 0.50，且 score/coverage 不恶化。Negative atom validation 要求 result
atom id/content hash 出现在 `after.negative_evidence_atoms[]`；false-positive task 上
必须证明 `after.target.selected=false`，或 before/after score 都存在且 after score
低于 before score；并且必须提供至少一个 `after.positive_controls[]` 且全部 passed
未 regressed，或提供带非空 reason 的 `after.positive_control_waiver`。Bootstrap
label validation 要求所有 linked source signals 都有 passed case，new/result label
selected 或 score >= 0.50，且 evidence atoms 来自 result label。

Validation comparability 默认使用 observation 的
`suggest_input_hash`；status、`updated_at`、`lock_version` 或 task label binding
只改变完整 snapshot 时写入 `task_metadata_drift` / `label_binding_drift` warning，
不会让 passed validation stale。title/description 变化会写入 `suggest_input_drift`
并使 case incomparable；旧 observation 缺少 `suggest_input_hash` 时写入
`legacy_suggest_input_hash_missing`，不能静默 passed。`--status passed` 会把 linked
source signals 转为 `resolved`；`failed` / `partial` 保留历史和 evidence，source
signals 继续等待后续修正或人工处理。

`label propose --json` 返回结构化 attempt：

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

## 9. Removed DAG Commands

`kanban dag` is no longer a supported command surface. Dependency management
remains available through `kanban dep`, task status transitions, dispatcher
guards, and the shared SQLite command service. Callers that previously consumed
DAG snapshot, ancestors, actionable, or frontier JSON must switch to the
specific dependency, task list, search, context, or graph APIs that match their
use case.

---

## 10. Comment Commands

```bash
kanban comment add <task_ref> [<body>|--body-file <PATH|->] [--kind note|decision] [--author-type user|agent] [--agent-type <type>] [--metadata-json <json>|--metadata-json-file <PATH|->]
kanban comment list <task_ref>
```

`--actor` supplies the comment author display identity. If `--kind` is omitted,
the service default is `note`. If `--author-type` is omitted, the service default
is `user`; pass `--author-type agent --agent-type <type>` for Codex/dispatcher or
other automated writers. `signal` is a persisted comment kind, but users should
create signal backlink comments through `kanban signal record` rather than
manually using `comment add --kind signal`; this keeps the signal ledger and
backlink comment in one transaction. `--body-file <PATH|->` reads long comment
bodies from files or stdin and is mutually exclusive with inline `<body>`; it is the recommended path for multiline or shell-sensitive comment text.
`--metadata-json` defaults to `{}` and must be a JSON object;
`--metadata-json-file <PATH|->` reads the same JSON payload from a file or stdin, avoids shell quoting issues for structured payloads,
and is mutually exclusive with `--metadata-json`. For `--kind decision`,
metadata is required to satisfy the structured
decision schema: non-empty `options`, unique lowercase ASCII option `slug`
values, `selected` matching one slug, non-empty `reason`, and optional
non-empty `risk` / `verification`.

Agent command failure traces should be recorded as comments instead of being
left only in chat transcripts. Use `comment add --author-type agent --agent-type
<name> --kind note --metadata-json <json>` with the human-readable body as a
short summary and the structured trace in metadata. The minimum trace payload is
an object with these fields:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "tool": "kanban-cli",
  "command": "kanban task step add",
  "argv": ["kanban", "task", "step", "add", "..."],
  "intent": "add a required execution-plan step",
  "why_selected": "agent selected the step command because the task needed execution-plan tracking",
  "actual_error": "unexpected argument 'true' found",
  "repair": "retry with canonical bare --required or supported --required true/false form",
  "product_signal": "agent-facing boolean flag compatibility gap",
  "followup_task": "kanban-tool#366"
}
```

Callers may add extra fields, but these names are the stable minimum contract for
tooling that mines failed agent commands into parser, docs, skill, or test work.

Agent-facing rich input example:

```bash
kanban comment add default#12 --body-file - <<'EOF'
正文可以安全包含 $VAR、$(command)、`code`、JSON 和多行文本。
EOF
```

Use `--kind decision` for meaningful multi-option choices. Body remains the
human-readable fallback summary, while structured options and selection data
live only in `--metadata-json`:

```text
已决定继续使用 comment metadata 承载结构化决策信息，正文保留为简短结论，方便没有结构化渲染的环境阅读。
```

Decision metadata example:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
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

Skip decision comments for trivial naming, formatting, or purely mechanical
choices.

Human output is compact and includes comment id, task id, created_at, kind,
author identity, author_type, optional agent_type, and body:

```text
c_01HX... task=t_01HX... created_at=1717520000000 [note] alice (user): ready for review
c_01HX... task=t_01HX... created_at=1717520000100 [note] codex (agent/root): tests passed
```

JSON output uses the standard envelope and returns the contract comment DTO for `add` or
a list of that DTO for `list`, including natural, lossless `metadata` objects. The input flag
names `--metadata-json` / `--metadata-json-file` remain unchanged. Creating a comment
writes `task_events(kind='task.comment.created')`.

---

## 11. Event Commands

```bash
kanban events <task_ref>
kanban events --board default
```

不传 `<task_ref>` 时按 active board 列出 events。Archived board 的 events 仍可通过显式 `--board` 读取。

---

## 12. Run Commands

```bash
kanban runs <task_ref>
kanban run show <run_id>
kanban run logs <run_id>
kanban run logs <run_id> --tail-bytes 65536
```

`kanban run logs` 默认最多读取 256 KiB。传 `--tail-bytes` 时只返回 log 末尾指定字节数。`task_runs.log_path` 必须解析到受信任日志目录且文件名匹配 `<run_id>.log`；可疑路径会被拒绝。

---

## 13. Dispatcher / Server Commands

```bash
kanban serve
kanban serve --quiet
kanban serve --log-level warn
kanban serve --search-sync-interval-ms 5000

kanban dispatch
kanban dispatch --once
kanban dispatch --worker-profile default
kanban dispatch --worker-profile backend --profile-config ./workers.toml
kanban dispatch --max-iterations 10 --poll-interval-ms 1000
```

`kanban dispatch` is a foreground loop. Use `--once` for one pass, or `--max-iterations`
for bounded scripts/tests. `--profile-config` reads the selected `[workers.<name>]`
section and can set `command`, `claim_ttl_ms`, `heartbeat_interval_ms`,
`on_success`, `on_failure`, and `log_dir`. Dispatcher log directories must be
inside a trusted run-log root: the platform default run log directory,
`<db_dir>/logs`, or `<db_dir>/.kb/logs`.

Ctrl-C/SIGINT is an operator stop for the foreground `kanban dispatch` loop.
The current `dispatch_once` / worker iteration is not actively interrupted; the
loop stops before starting another polling iteration, including during the
inter-iteration wait. The command exits `0` after this graceful stop. With
`--json`, stdout remains the normal success envelope and includes
`data.stop_reason="interrupted"`; operator cancellation diagnostics, if emitted,
go to stderr only. A non-interrupted `--max-iterations` exit omits
`data.stop_reason`. A second Ctrl-C during dispatcher shutdown exits
immediately with code `130`.

`kanban serve` writes startup diagnostics, HTTP request traces, and graceful shutdown notices to stderr by default; stdout remains reserved for explicit machine-readable output and is not used for service logs. Use `--quiet` to suppress serve diagnostics, `--log-level <off|error|warn|info|debug|trace>` for a simple verbosity override, or omit both and set `RUST_LOG` for advanced tracing filters. The default filter is `kanban=info,kanban_cli=info,kanban_server=info,tower_http=info,kanban_desktop=info`.

Ctrl-C/SIGINT triggers graceful shutdown for `kanban serve`, releases the runtime
lock, exits `0`, and writes no stdout. `--quiet` and `--log-level off` suppress
the graceful shutdown notice. A second Ctrl-C during shutdown exits immediately
with code `130`.

`kanban serve` starts a conservative background search sync loop when the binary is
built with `tantivy-backend`. The loop makes one prompt startup attempt and then
calls `sync_search_index` every `--search-sync-interval-ms` milliseconds
(default `5000`). Use `--search-sync-interval-ms 0` to disable it. Without
`tantivy-backend`, the flag is accepted and no background index task is started.

---

## 14. Search Commands

### 14.1 `kanban search`

```bash
kanban search <query> [--status ready] [--status review] [--assignee worker-a] [--label backend] [--include-archived] [--limit 20] [--offset 0] [--json]
```

默认 CLI build 启用 `tantivy-backend`。当 `index/v1/tasks/` 存在可读 Tantivy 索引时，`kanban search` 使用 Tantivy；缺失、损坏、过期或二进制显式以 `--no-default-features` 构建时回落 SQLite，并在顶层 `meta` 中标记 stale。搜索匹配 task title、description、comments、run summary/error、event kind/payload。

`--label <name-or-id>` 可重复；多个 label 使用 AND 语义，并在 search
分页前过滤 task。带 label 过滤的 Tantivy search 会回落到 SQLite fallback，
以保持当前 label 关联关系和分页语义正确。

Task ref 形状的 query 始终使用 SQLite 精确匹配语义，即使当前存在可用 Tantivy index：
纯数字 `12`、`#12` 匹配请求 board 内的 seq；`board#12` / `board/#12`
只在显式 board 与请求 board 相同时匹配；`t_...` 只匹配请求 board 内的 task id。
这些 query 不会因为 title、description 或聚合搜索文本包含相同数字/ref 片段而返回额外 task。

Human output compactly includes the public task ref, status, score, title, and snippet when available. It does not include the internal `t_...` task id by default; task id remains available in JSON output and diagnostic/detail-oriented surfaces.

```text
agent-work#12 [ready] score=60.0 实现状态机 - ready spec needle
```

JSON output:

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "hits": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "score": 60.0,
        "snippet": "ready spec needle",
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

### 14.2 `kanban index`

```bash
kanban index status
kanban index doctor
kanban index rebuild
kanban index sync
```

默认 CLI build 启用 `tantivy-backend`，Tantivy index 是可重建 derived cache；显式以 `--no-default-features` 构建时保留 SQLite fallback：

- `status` returns backend/meta.
- `doctor` returns the same fallback health meta for scripts.
- `rebuild` builds/replaces `index/v1/tasks/` beside the SQLite DB and stores a clean high-watermark state in `app_settings`.
- `sync` consumes `task_events.id` after the stored high-watermark, delete+reindexes affected task aggregates, then advances the high-watermark only after a successful commit.
- Task mutations do not update Tantivy inside their transactions; run `kanban index sync` after changes, rely on `kanban serve` background sync for local server/desktop sessions, or use `kanban index rebuild` to replace the derived index.

The persisted setting key is board-scoped as `search.tasks.state.<board_id>`. Its JSON contains `schema_version`, `index_version`, `backend`, `index_name`, `board_id`, `last_event_id`, `dirty`, `updated_at`, and optional `message`; it is included in JSONL export/import through existing `app_settings` handling.

JSON data shape:

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

With Tantivy enabled after rebuild, `backend` is `tantivy`, `derived_index` is `true`, and `index_version` is `tasks-v1`.
When the current `MAX(task_events.id)` is greater than the stored `last_event_id`, `stale=true` and `index_lag_events` reports the event lag. Search falls back to SQLite while stale to preserve current-result correctness.
Background sync errors do not make search fail open to stale Tantivy results; the next search still reports stale/fallback metadata and returns current SQLite results when the derived index is behind or unusable.

---

### Signal Ledger

```bash
kanban signal record --board <slug> --input <path|-> --json
kanban signal list --board <slug> [--status open|confirmed|rejected|superseded|resolved]... [--kind <kind>]... [--task <task-ref>] [--include-all] [--limit 100] --json
kanban signal show --board <slug> <signal-id> --json
kanban signal review --board <slug> [--status open|confirmed|rejected|superseded|resolved]... [--kind <kind>]... [--task <task-ref>] [--include-all] [--limit 100] --json
kanban signal confirm --board <slug> <signal-id>... [--reason <reason>|--reason-file <PATH|->] --json
kanban signal reject --board <slug> <signal-id>... [--reason <reason>|--reason-file <PATH|->] --json
kanban signal resolve --board <slug> <signal-id>... [--reason <reason>|--reason-file <PATH|->] --json
kanban signal supersede --board <slug> <signal-id>... --by <replacement-signal-id> [--reason <reason>|--reason-file <PATH|->] --json
```

`signal list` 和 `signal review` 共享 `status`、`kind`、`task`、`include-all`、
`limit` 查询过滤参数。没有显式 `--status` 时，两者默认只返回 `open` 和
`confirmed`；此时传 `--include-all` 会取消默认状态过滤并返回完整历史。显式
`--status` 始终优先，即使同时传 `--include-all`，结果仍只包含指定状态。
`--status` 和 `--kind` 都可以重复传入。

`record` input JSON supports `kind`, `title`, `summary`, `severity`, optional `task_ref` / `task_id` / `run_id` / `comment_id`, `actor`, `agent_type`, `dedupe_key`, `source`, `evidence`, and optional `comment.body`. `source` is a string identifier for where the observation came from; structured command details such as `command`, `cwd`, `exit_code`, `stderr`, or related logs belong in the natural `evidence` object. Signal responses use the same natural object rather than an escaped `evidence_json` string. When task context is present, the service writes the signal ledger rows and a `comment.kind = "signal"` backlink in one SQLite transaction. Signal backlink `metadata` includes `type:"signal_link"`, `signal_id`, `observation_id`, `signal_kind`, and `signal_status`; generic signal comment metadata remains open and lossless. V1 does not create follow-up tasks automatically.

Lifecycle transitions are `open -> confirmed|rejected|superseded|resolved` and `confirmed -> resolved`. `supersede` requires a same-board replacement signal and rejects cycles. Lifecycle reason 可用 `--reason-file <PATH|->` 从文件或 stdin 读取，并与 inline `--reason` 互斥。

## 15. Maintenance Commands

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
kanban graph neighbors kb://task/t_... [--predicate depends_on] [--limit 50]
kanban graph query [<SPARQL>|--sparql-file <PATH|->] [--limit 50]
kanban vector configure [--provider ollama] [--endpoint http://127.0.0.1:11434] [--model qwen3-embedding:0.6b] [--dimensions 1024] [--skip-check] [--vector-config <toml>]
kanban vector status [--vector-config <toml>]
kanban vector rebuild [--vector-config <toml>]
kanban vector sync [--vector-config <toml>]
kanban context build t_... [--lexical-limit 5] [--vector-config <toml>]
```

`kanban stats --json` 返回 status counts、过期 running claim 列表、blocked reason 聚合、unplanned active task 数量，以及 required steps 未完成的 active parent 数量，用于本地 operator recovery。
`kanban graph query` 的 SPARQL 可用 `--sparql-file <PATH|->` 从文件或 stdin 读取，并与 positional `<SPARQL>` 互斥。

`kanban backup` 使用 SQLite `VACUUM INTO` 创建一致备份；目标文件已存在时失败，避免覆盖。`backup --out -` 会被明确拒绝，因为 SQLite backup 需要 filesystem path，不能安全写入 stdout。
`kanban export --format jsonl` 导出数据库记录；目标文件已存在时失败，避免覆盖旧 snapshot。`export --out -` 会把 JSONL snapshot 写入 stdout，不输出 human status 文案，也不会写 stderr；该模式不能与 `--json` 组合，因为 JSONL stream 和 JSON envelope 不能共享 stdout。21 个稳定 discriminator 的 input/output 分别拥有 42 个 exact schema roots；每行 data 闭合，required-nullable 键不能省略但可显式为 `null`，export/import descriptor 与 schema authority 同源。JSONL 不复制 `task_runs.log_path` 指向的外部日志文件，导出的 run 记录会清空 `log_path`；导出中的 live `running` task 会清除 claim并恢复为 `ready`，对应 running run 会落为 `canceled`，并追加 `task.export_sanitized` 事件解释这次 portable snapshot 改写。需要完整可恢复副本时使用 `kanban backup`。JSONL export 包含 generic signal ledger record types：`signal_observation`、`signal`，以及 label ontology ledger record types：`label_ontology_observation`、`label_ontology_signal`、`label_ontology_action`、`label_ontology_action_atom_effect` 和 `label_ontology_action_signal`；因此 portable JSONL 与 SQLite backup 都会保留 signal、ontology observation/signal/action/effect provenance。JSONL `event.data.payload` 仍按 opaque JSON 保存；39-kind typed union 只属于 events API/SSE。
`kanban import --dry-run` 会在临时 SQLite 数据库中解析导入文件并运行同一 final doctor gate，不替换或创建所选目标 DB；脚本和 CI 可先用它验证 snapshot。上一版 exporter 的 storage-native snapshot 只作为单向兼容输入：同一 record 如果同时出现 natural renamed key 与对应格式的 storage-native renamed key，会在 compatibility normalization 前以 `invalid_input` 拒绝，不能由 legacy 值静默覆盖 natural 值。`kanban import --replace` 是替换式恢复入口，必须显式传 `--replace`；导入文件必须至少包含一个 board，且每个 board 必须包含 columns。`kanban import --replace` 是 offline-only 操作；运行前必须停止 `kanban serve` 和常驻 `kanban dispatch`，如果检测到 active runtime lock 会直接拒绝。Import 在同一 SQLite transaction 内执行插入与 final doctor gate：基础关系表会校验 `task_labels`、`task_dependencies`、`task_runs`、`task_comments`、`task_events`、`task_attachments` 的 row board 与 referenced task / label / run board 一致；失败时整个 replace transaction 回滚，不提交部分数据。Ontology import 会延迟回填 `label_ontology_signals.superseded_by_signal_id` 与 `label_ontology_actions.parent_action_id`，因此不依赖 JSONL 中同表自引用 rows 的偶然顺序；导入后会拒绝跨 board / orphan generic signal context、generic signal supersede cycles、跨 board ontology links、orphan action-signal links、ontology supersede cycles 和 action parent cycles。
`kanban entity`、`kanban outbox`、`kanban derived` 是 Knowledge Substrate 的只读维护入口。SQLite 仍是事实源；这些命令只报告统一 entity registry、派生索引 outbox 和 derived store 状态，不改变 task 状态或 claim。
`kanban entity list --json` 返回 `{"data": [...]}`，`kanban entity show --json` 返回
`{"data": {...}}`；两者共享闭合的公开 entity item，并保留
`uri`、`kind`、`source_table`、`source_id`、`created_at`、`updated_at`，以及
required-nullable `board_id`、`task_id`、`title`、`summary`、`content_hash`、
`archived_at`。调用方不能把这些字段缺失解释为 `null`。`list` 的 `--kind` 与
`--limit` 由同一 SQLite service query 执行；`show` 继续按 exact URI 查询并保留
`not_found` error envelope。human-readable 输出不变。
`kanban graph` 和 `kanban vector` 是 helper subprocess 派生层入口。默认 CLI 不链接
Oxigraph/LanceDB heavy deps；它解析 `KANBAN_GRAPH_HELPER` / `KANBAN_VECTOR_HELPER`、
`/usr/lib/kanban/<helper>`、CLI sibling binary、`KANBAN_CARGO_TARGET_ROOT` 或
`CARGO_TARGET_DIR` 的 `release/<helper>`，最后回退到 `PATH` 中的 helper。helper 缺失或
返回非法 envelope 时，`status` 返回 disabled/degraded status；helper error envelope、
坏 board/db/config 或 payload/domain 错误会作为命令错误返回。启用后仍只作为可重建
relation/vector store，不参与 task 状态事务。
`kanban vector status --json` 保留 `message` 兼容字段，同时返回结构化
`diagnostics`、`dirty`、`board_dirty` 字段；dirty/error 判断应使用这些字段，不解析
`message` 文案。
`kanban vector configure` 默认写入全局 config：`$XDG_CONFIG_HOME/kanban/config.toml`（平台默认通常为 `~/.config/kanban/config.toml`），并默认配置本机 Ollama embedding provider。传 `--vector-config <toml>`（别名 `--config`）时写入指定 TOML。configure 默认调用 `/api/embed` 做短文本维度校验；校验失败时不写配置；`--skip-check` 只跳过这次连通性/维度检查。配置格式：

```toml
board = "kanban-tool"

[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "qwen3-embedding:0.6b"
dimensions = 1024
```

项目级 `.kb/config.toml` 可以覆盖全局 `[vector]`；命令行 `--vector-config <toml>` 优先级最高。解析顺序是：显式 `--vector-config`、最近的项目 `.kb/config.toml`、全局 config。`kanban board use <board>` 更新项目配置文件的 `board` 字段时必须保留该文件内已有 `[vector]` 配置。配置有效且 helper 可用时 `kanban vector status/rebuild/sync` 使用该 provider；未配置或 helper 不可用时保持 disabled/degraded fallback。`kanban context build` 当前仍使用 SQLite/lexical fallback，并通过 degraded markers 报告 graph/vector 不可用。
`kanban context build` 通过 SQLite hydrate canonical task，并合并 lexical、graph、vector hits。graph/vector 不可用或失败时返回 degraded markers；失败原因通过有界 diagnostics 暴露，context pack 本身仍可用。

`kanban outbox list --json` 返回 `{"data": [...]}`，每项保留完整 outbox job 字段，
包括 required-nullable `source_event_id` 与 `last_error`；`--status` 与 `--limit` 由同一
SQLite service 查询执行。`kanban derived status --json` 同样返回 `{"data": [...]}`，
每个 store 的 `last_rebuild_at`、`last_sync_at` 与 `last_error` 都是 required-nullable，
调用方不能把字段缺失解释为 `null`。

`kanban derived status` 中的 `last_event_id` 是 store 级成功处理水位，不是当前 board 的局部水位。`dirty=true` 表示该 store 仍有任意 board 的 pending/running/failed outbox，或最近一次派生更新失败；board-scoped `kanban index sync`、`kanban graph sync`、`kanban vector sync` 只清理当前 board 的 job，不能因为本 board clean 就强制清掉全局 dirty。

语义 label atom 使用独立 derived store `lancedb_label_atoms`，对应 LanceDB 表
`kb_label_atoms`。它不属于普通 task event outbox fanout：`kanban vector sync/rebuild`
只维护 `lancedb_chunks` / `kb_chunks`，不会把 label atom store 标记为完成。label
semantics service 写入 `label_semantics` / `label_atoms` 后单独标脏
`lancedb_label_atoms`；provider 或 feature 不可用时该 store 可报告 degraded，但不影响
普通 `kanban label` CRUD 和 `task_labels` 绑定。

### 15.1 `kanban doctor`

检查：

- DB 文件存在。
- migrations 完整；当前 schema user_version 为 23。
- `PRAGMA integrity_check`。
- orphan active run。
- running task 是否缺 claim。
- expired claim 数量。
- dependency cycle。
- archived dependency edge（archived parent -> active child is allowed history; archived child from active parent is reported）。
- 缺失 run log 文件。
- 可疑 run log 路径。
- `ready/running` task 带有未完成 parent dependency。
- `ready/running` task 缺少可执行 spec。
- `ready/running` task 带有未来 `scheduled_at`。
- 基础关系表 board consistency：`task_labels`、`task_dependencies`、`task_runs`、
  `task_comments`、`task_events`、`task_attachments` 的 row board 必须和 referenced
  task / label / run board 一致。当前 schema 用 board-scoped composite FK 保护
  `task_labels`、`task_dependencies`、`task_runs`、`task_comments` 和
  `task_attachments`；v22+ 还检查 `task_execution_plans` task board scope，v23+ 还检查 `task_steps` parent/linked task board scope。`task_events` 保留 nullable task/run refs 与 `ON DELETE SET NULL`
  语义，通过 INSERT/UPDATE triggers 校验非空 refs 的 board scope。
- SQLite `PRAGMA foreign_key_check`：doctor 将每条 violation 转换为 hard-error issue；
  JSONL import final gate 也会在 commit 前运行同一检查，失败时回滚整个 replace
  transaction。
- `index_outbox` backlog：`outbox_pending`、`outbox_running`、`outbox_failed`。
- derived store health：`derived_dirty_stores`、`derived_error_stores`、`derived_stores[]`，每个 store 包含 `dirty`、`last_error` 和按 store target 聚合的 pending/running/failed outbox 计数。
- foundation relationship consistency：人类输出包含 `consistency_errors` /
  `consistency_warnings` 计数；`--json` 额外返回 `consistency_issues[]`，每条 issue
  包含 `severity`、`code`、`message`、`record_ids`。Message 包含 `table`、`row`、
  `row_board`、`referenced` 和 `referenced_board`。非零 `consistency_errors` 会让
  `ok=false`。
- label ontology ledger health：v12+ 数据库必须存在 `label_ontology_observations`、`label_ontology_signals`、`label_ontology_actions`、`label_ontology_action_atom_effects`、`label_ontology_action_signals`；doctor 会报告 observation/signal/action/action-effect/action-signal 的跨 board link、orphan link、parent action 异常、supersede cycle 和可检查 soft reference 不一致。人类输出包含 `ontology_ledger_errors` / `ontology_ledger_warnings` 计数；`--json` 额外返回 `ontology_ledger_issues[]`，每条 issue 包含 `severity`、`code`、`message`、`record_ids`。非零 `ontology_ledger_errors` 会让 `ok=false`；warning 用于 rebuildable 或可解释的软引用异常，不单独让 doctor unhealthy。

`dirty` / pending outbox 表示派生层需要 sync/rebuild，不会改变 SQLite task truth；failed outbox 或 `last_error` 用于 operator 判断是否需要 `kanban index sync`、`kanban graph sync/rebuild` 或 `kanban vector sync/rebuild`。`derived_stores[].last_event_id` 表示对应 store 已成功提交的全局 event watermark；当 `dirty=true` 时，它仍然只是“已成功处理到哪里”的摘要，不代表所有 board 都已经干净。

---

## 16. JSON contract reference

JSON 输出、运行期 JSON error、clap parse-time error、stderr/stdout 数据平面和 JSONL / NDJSON streaming boundary 的权威契约统一见 [1.3 JSON output contract](#13-json-output-contract)。

本节仅保留跳转，避免同一份 CLI_SPEC 出现两个 JSON 契约来源。新增或修改 JSON / JSONL / error-code 行为时，只更新 1.3 及对应命令章节，并补充测试证据。

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
| `--db <path>` | 指定 SQLite DB。默认从 config 读取。 |
| `--board <slug-or-id>` | 显式指定 active board，优先级最高。 |
| `--actor <name>` | 操作 actor。默认 OS username。 |
| `--locale <auto|zh-CN|en>` | human 输出语言。默认 `zh-CN`；`auto`/`system` 使用系统 locale。 |
| `--json` | JSON 输出。 |

Active board 解析顺序：

1. `--board <slug-or-id>`。
2. `KB_BOARD` 环境变量。
3. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `board = "<slug>"`。
4. fallback 到 `default`。

`kanban board use <board>` 会把当前目录写成项目级 `.kb/config.toml`；后续子目录自动继承该 active board。该配置只选择本地项目的 board，不创建新 DB。

Locale 只影响 human-readable 输出和错误消息，不改变 JSON key、状态枚举、task ref、ID、exit code 或机器可读 diagnostics。选择顺序：

1. `--locale <auto|zh-CN|en>`。
2. `KANBAN_LOCALE`。
3. 默认 `zh-CN`。

`auto` / `system` 会按 `LC_ALL`、`LC_MESSAGES`、`LANG` 解析系统 locale；当前只支持中文和英文。脚本和自动化应优先使用 `--json`，不要依赖 human 文案。

### 1.1 JSON output contract

所有公开 `--json` 输出使用顶层 envelope：

```json
{
  "data": {},
  "meta": {}
}
```

`meta` 只在需要分页、details 或 diagnostics 时出现。`data` 可以是一个对象，也可以是对象数组；公共输出不得依赖裸 tuple、未命名数组位置、只有内部 id 的临时数组，或只回显输入参数。命令需要表达关系、删除或当前选择时，应返回命名 DTO，例如 `edge.parent`/`edge.child`、`step`、`board`。Task-like DTO 必须带可复制的 `ref`、`id`、`board_id` 或 `board_slug` 中的必要身份字段。

`board current --json` 和 `board use --json` 的 `data.board` 是完整 board 对象；调用方应读取 `data.board.slug`，不要把 `data.board` 当字符串。

### 1.2 Shell completions

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
- comment kind values for `comment add --kind`.

`kanban __complete` is an internal newline-delimited helper for shell scripts
and tests. It accepts:

```text
task-ref | dependency-task-ref | board | status | comment-kind
```

The helper must be quiet for completion use: missing DB files, uninitialized
DBs, missing board config, or read/query failures return success with no
candidates and no stderr. Static completion generation itself does not open or
create the SQLite database.

---

## 2. Exit Codes

| Code | 含义 |
|---:|---|
| 0 | 成功。 |
| 1 | 通用错误。 |
| 2 | 参数错误。 |
| 3 | 未找到对象。 |
| 4 | 非法状态转换。 |
| 5 | claim 冲突。 |
| 6 | dependency blocked。 |
| 7 | SQLite busy/locked。 |
| 8 | integrity check failed。 |

---

## 3. Init

### 3.1 `kanban init`

初始化本地 DB、默认 board、默认 columns。

```bash
kanban init
kanban init --db .kb/kb.db
kanban init --force
```

输出：

```text
Initialized Kanban database at ~/.local/share/kb/kb.db
Default board: default
```

JSON：

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
| `--description-file <PATH|->` | 从文件或 stdin (`-`) 读取 Markdown 描述；与 `--description` 互斥。 |
| `--status <status>` | 显式初始状态：triage/todo/scheduled/ready。 |
| `--assignee <name>` | assignee/worker profile。 |
| `--priority <int>` | Priority level `0..3`: `0` = P0 incident/blocker/must-handle-immediately, `1` = P1 near-term focus, `2` = P2 important follow-up, `3` = P3 ordinary backlog/low/default. Invalid values are rejected. |
| `--scheduled-at <epoch_ms>` | 计划时间，Unix epoch milliseconds。 |
| `--due-at <epoch_ms>` | 截止时间，Unix epoch milliseconds。 |
| `--max-retries <n>` | worker 失败或 reclaim 后最多重试次数。 |
| `--label <name>` | 创建时附加已存在 label，可重复；缺失的 board label 会拒绝整个 create。 |
| `--metadata <json>` | 扩展 JSON。 |
| `--metadata-file <PATH|->` | 从文件或 stdin (`-`) 读取扩展 JSON；与 `--metadata` 互斥。 |

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
agent-work#12 t_01HX... [ready] 实现状态机
```

JSON output：

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

默认人类可读输出仍是紧凑的单行 task 摘要：

```text
agent-work#12 t_01HX... [ready] 实现状态机
```

`--details` 改变人类可读输出，显示为易读字段列表。可用时包含 task
ref/id/status/title、完整多行 description、assignee、priority、labels、
scheduled_at、due_at、created_at、updated_at、execution_plan_state、required/optional step counts，以及其他 task snapshot 字段。
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

JSON：

```json
{
  "data": {
    "task_id": "t_01HX...",
    "run_id": "r_01HX...",
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
kanban task block <task_ref> <reason>
```

Options：

| Option | 说明 |
|---|---|
| `--claim-token <token>` | running task block 时需要。 |
| `--force` | 强制 block。 |

### 6.7 Unblock

```bash
kanban task unblock <task_ref>
```

不会盲目进入 ready，而是根据 spec、schedule、dependencies 重新计算目标状态。

### 6.8 Reopen

```bash
kanban task reopen <task_ref> --reason <text>
```

只允许 reopen `done` task，`--reason` 必填且不能为空。Reopen 会清空 `completed_at`，保留 `result_summary` / `result_json`，并按 spec、schedule、dependency 和 execution plan readiness 重新计算目标状态。

如果被 reopen 的 task 是其他 task 的 dependency parent，直接 child 中仅 `triage|todo|scheduled|ready` 会重新计算；`running|blocked|review|done|archived` 不隐式改写。

### 6.9 Reclaim

```bash
kanban task reclaim --expired
kanban task reclaim
```

当前 CLI reclaim 处理 active board 内 expired claims；裸 `kanban task reclaim` 与 `kanban task reclaim --expired` 等价。

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
kanban task step done <task_ref> <step_ref> --note <text>
kanban task step skip <task_ref> <step_ref> --reason <text>
kanban task step reopen <task_ref> <step_ref> --reason <text>
kanban task step remove <task_ref> <step_ref>
kanban task step not-required <task_ref> --reason <text>
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
kanban label semantics upsert <label> [--expected-semantics-hash <hash>] [--replace] [--reason <text>] [--source-signal <signal_id>]... [--description <text>] [--applies-when <text>]... [--excludes-when <text>]... [--positive-example <text>]... [--negative-example <text>]... [--remove-applies-when <text>]... [--remove-excludes-when <text>]... [--remove-positive-example <text>]... [--remove-negative-example <text>]... [--json]
kanban label semantics delete <label> --expected-semantics-hash <hash> --reason <text> [--json]
kanban label atoms list [--json]
kanban label atom explain <atom-id-or-content-hash> [--json]
kanban label atom-index status [--vector-config <toml>] [--json]
kanban label atom-index rebuild [--vector-config <toml>] [--json]
kanban label atom-index query <text> [--polarity positive|negative] [--limit 24] [--vector-config <toml>] [--json]

`label atom-index status`、`rebuild` 和 `query` 复用 vector TOML 解析规则：显式 `--vector-config`/`--config` 优先，其次是最近项目 `.kb/config.toml`，最后是全局 config。helper argv 只在显式传入 `--vector-config` 时附带该参数；省略时由 helper 按默认配置解析。
kanban label suggest <task_ref> [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label propose <task_ref> [--proposal-json <path>] [--source-signal <signal_id>]... [--allow-retarget] [--retarget-reason <text>] [--actor-type user|agent] [--agent-type <type>] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label proposals list [--task <task_ref>] [--status proposed|accepted|rejected] [--json]
kanban label proposals show <proposal_id> [--json]
kanban label proposals accept <proposal_id> [--reason <text>] [--source-signal <signal_id>]... [--allow-retarget] [--retarget-reason <text>] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label proposals reject <proposal_id> [--reason <text>] [--json]
kanban label ontology record <task_ref> --input <path|-> [--suggestion-snapshot <path|-> | --capture-suggest] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label ontology list [--status open|confirmed|resolved|rejected|superseded]... [--kind false_negative|false_positive|vocabulary_gap|name_issue|boundary_issue|structure_issue]... [--task <task_ref>] [--label <label>] [--proposed-label <name>] [--include-all] [--limit 100] [--json]
kanban label ontology show <signal_id> [--json]
kanban label ontology review [--group-by label|candidate-atom|proposed-label|cluster] [--include-all] [--limit 100] [--json]
kanban label ontology quality [--sample-limit 20] [--json]
kanban label ontology confirm <signal_id>... --reason <text> [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology reject <signal_id>... --reason <text> [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology supersede <signal_id>... --by <signal_id> --reason <text> [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology resolve <signal_id>... --no-change --reason <text> [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology apply atom <signal_id>... --label <label> --kind applies-when|positive-example|excludes-when|negative-example --text <text> --reason <text> [--allow-retarget] [--retarget-reason <text>] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology revert <action_id> --reason <text> [--expected-current-hash <hash>] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --status passed|failed|partial --reason <text> --input <path|-> [signal_id]... [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --trusted --status passed|failed|partial --reason <text> [signal_id]... [--positive-control <task_ref>]... [--positive-control-waiver <reason>] [--vector-config <toml>] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--actor-type user|agent] [--agent-type <type>] [--json]
```

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
default#12 t_01HX... [ready] 修复 API 回归 [backend,p1]
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

`kanban vector query-label-atoms` 是公开 raw helper 查询入口，支持 text 查询和 raw vector 查询：
`kanban vector query-label-atoms <text> [--polarity positive|negative] [--limit N] [--embedding-model MODEL] [--vector-config <toml>]`，或
`kanban vector query-label-atoms --vector-json '[0.1,0.2]' [--include-vector] [--embedding-model MODEL] [--polarity positive|negative] [--limit N]`。
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

旧的 service-shaped `--input` 仍作为兼容入口保留；新调用方不应重复手写
`suggest_coverage`、`suggest_residual_norm` 或 `diagnostics_json`。如果 snapshot 中已有
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
`related_labels_json`。Observation metric `suggest_coverage`、
`suggest_coverage_cosine`、`suggest_residual_norm` 以及 signal metric
`suggest_score` / `confidence` 必须是 finite `0.0..=1.0`；`suggest_rank` 必须为
`null` 或 `>= 1`。
`rename_label` / `split_label` / `merge_labels` 当前只作为 review signal proposed_action
保存，CLI 不提供写入 canonical structure mutation action 或 structure plan action 的命令；
旧 structure-plan rows 只读展示为 unsupported validation requirement。

使用已保存 suggest snapshot 的推荐输入形状：

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

`--positive-control <task_ref>` 与 `--positive-control-waiver <reason>` 只用于
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
other automated writers. `--body-file <PATH|->` reads long comment bodies from
files or stdin and is mutually exclusive with inline `<body>`. `--metadata-json`
defaults to `{}` and must be a JSON object; `--metadata-json-file <PATH|->` reads
the same JSON payload from a file or stdin and is mutually exclusive with
`--metadata-json`. For `--kind decision`, metadata is required to satisfy the structured
decision schema: non-empty `options`, unique lowercase ASCII option `slug`
values, `selected` matching one slug, non-empty `reason`, and optional
non-empty `risk` / `verification`.

Agent command failure traces should be recorded as comments instead of being
left only in chat transcripts. Use `comment add --author-type agent --agent-type
<name> --kind note --metadata-json <json>` with the human-readable body as a
short summary and the structured trace in metadata. The minimum trace payload is
an object with these fields:

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
  "followup_task": "default#123"
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

JSON output uses the standard envelope and returns `CommentRecord` for `add` or
`Vec<CommentRecord>` for `list`, including `metadata_json`. Creating a comment
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

默认 CLI build 启用 `tantivy-backend`。当 `index/v1/tasks/` 存在可读 Tantivy 索引时，`kanban search` 使用 Tantivy；缺失、损坏、过期或二进制显式以 `--no-default-features` 构建时回落 SQLite，并在 meta 中标记 stale。搜索匹配 task title、description、comments、run summary/error、event kind/payload。

`--label <name-or-id>` 可重复；多个 label 使用 AND 语义，并在 search
分页前过滤 task。带 label 过滤的 Tantivy search 会回落到 SQLite fallback，
以保持当前 label 关联关系和分页语义正确。

Task ref 形状的 query 始终使用 SQLite 精确匹配语义，即使当前存在可用 Tantivy index：
纯数字 `12`、`#12` 匹配请求 board 内的 seq；`board#12` / `board/#12`
只在显式 board 与请求 board 相同时匹配；`t_...` 只匹配请求 board 内的 task id。
这些 query 不会因为 title、description 或聚合搜索文本包含相同数字/ref 片段而返回额外 task。

Human output compactly includes seq/id, status, score, title, and snippet when available:

```text
#12 t_01HX... [ready] score=60.0 实现状态机 - ready spec needle
```

JSON output:

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
    ],
    "meta": {
      "backend": "sqlite",
      "stale": false,
      "index_version": null,
      "last_event_id": 42,
      "index_lag_events": 0
    }
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

## 15. Maintenance Commands

```bash
kanban doctor
kanban stats
kanban backup --out backup.sqlite
kanban export --format jsonl --out board.jsonl
kanban import --input board.jsonl --replace
kanban vacuum
kanban checkpoint

kanban entity list [--kind task] [--limit 50]
kanban entity show kb://task/t_...
kanban outbox list [--status pending] [--limit 50]
kanban derived status
kanban graph status
kanban graph neighbors kb://task/t_... [--predicate depends_on] [--limit 50]
kanban vector configure [--provider ollama] [--endpoint http://127.0.0.1:11434] [--model qwen3-embedding:0.6b] [--dimensions 1024] [--skip-check] [--vector-config <toml>]
kanban vector status [--vector-config <toml>]
kanban vector rebuild [--vector-config <toml>]
kanban vector sync [--vector-config <toml>]
kanban context build t_... [--lexical-limit 5] [--vector-config <toml>]
```

`kanban stats --json` 返回 status counts、过期 running claim 列表、blocked reason 聚合、unplanned active task 数量，以及 required steps 未完成的 active parent 数量，用于本地 operator recovery。

`kanban backup` 使用 SQLite `VACUUM INTO` 创建一致备份；目标文件已存在时失败，避免覆盖。
`kanban export --format jsonl` 导出数据库记录；目标文件已存在时失败，避免覆盖旧 snapshot。JSONL 不复制 `task_runs.log_path` 指向的外部日志文件，导出的 run 记录会清空 `log_path`；导出中的 live `running` task 会清除 claim 并恢复为 `ready`，对应 running run 会落为 `canceled`，并追加 `task.export_sanitized` 事件解释这次 portable snapshot 改写。需要完整可恢复副本时使用 `kanban backup`。JSONL export 包含 label ontology ledger record types：`label_ontology_observation`、`label_ontology_signal`、`label_ontology_action`、`label_ontology_action_atom_effect` 和 `label_ontology_action_signal`；因此 portable JSONL 与 SQLite backup 都会保留 ontology observation/signal/action/effect provenance。
`kanban import` 是替换式恢复入口，必须显式传 `--replace`；导入文件必须至少包含一个 board，且每个 board 必须包含 columns。`kanban import --replace` 是 offline-only 操作；运行前必须停止 `kanban serve` 和常驻 `kanban dispatch`，如果检测到 active runtime lock 会直接拒绝。Import 在同一 SQLite transaction 内执行插入与 final doctor gate：基础关系表会校验 `task_labels`、`task_dependencies`、`task_runs`、`task_comments`、`task_events`、`task_attachments` 的 row board 与 referenced task / label / run board 一致；失败时整个 replace transaction 回滚，不提交部分数据。Ontology import 会延迟回填 `label_ontology_signals.superseded_by_signal_id` 与 `label_ontology_actions.parent_action_id`，因此不依赖 JSONL 中同表自引用 rows 的偶然顺序；导入后会拒绝跨 board ontology links、orphan action-signal links、supersede cycles 和 action parent cycles。
`kanban entity`、`kanban outbox`、`kanban derived` 是 Knowledge Substrate 的只读维护入口。SQLite 仍是事实源；这些命令只报告统一 entity registry、派生索引 outbox 和 derived store 状态，不改变 task 状态或 claim。
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
`kanban vector configure` 默认写入全局 config：`$XDG_CONFIG_HOME/kb/config.toml`（平台默认通常为 `~/.config/kb/config.toml`），并默认配置本机 Ollama embedding provider。传 `--vector-config <toml>`（别名 `--config`）时写入指定 TOML。configure 默认调用 `/api/embed` 做短文本维度校验；校验失败时不写配置；`--skip-check` 只跳过这次连通性/维度检查。配置格式：

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

## 16. JSON Output Contract

成功：

```json
{
  "data": {},
  "meta": {}
}
```

失败：

```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot claim task from status todo",
    "details": {
      "task_id": "t_...",
      "status": "todo"
    }
  }
}
```

stderr：

- human 模式：错误写 stderr。
- JSON 模式：错误 JSON 写 stdout 或 stderr 需要固定；建议 stderr，stdout 保持空。

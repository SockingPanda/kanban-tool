# CLI SPEC

默认 binary 名称：`kanban`

CLI 是一等入口；它与 Web 使用同一套 command service 和 SQLite schema。

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
| `--json` | JSON 输出。 |

Active board 解析顺序：

1. `--board <slug-or-id>`。
2. `KB_BOARD` 环境变量。
3. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `board = "<slug>"`。
4. fallback 到 `default`。

`kanban board use <board>` 会把当前目录写成项目级 `.kb/config.toml`；后续子目录自动继承该 active board。该配置只选择本地项目的 board，不创建新 DB。

### 1.1 Shell completions

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
| `--status <status>` | 显式初始状态：triage/todo/scheduled/ready。 |
| `--assignee <name>` | assignee/worker profile。 |
| `--priority <int>` | Priority level `0..3`: `0` = P0 incident/blocker/must-handle-immediately, `1` = P1 near-term focus, `2` = P2 important follow-up, `3` = P3 ordinary backlog/low/default. Invalid values are rejected. |
| `--scheduled-at <epoch_ms>` | 计划时间，Unix epoch milliseconds。 |
| `--due-at <epoch_ms>` | 截止时间，Unix epoch milliseconds。 |
| `--max-retries <n>` | worker 失败或 reclaim 后最多重试次数。 |
| `--label <name>` | 创建时附加 label，可重复；缺失的 board label 会按名称创建。 |
| `--metadata <json>` | 扩展 JSON。 |

Priority 只表达相对重要性和排序，不表达可 claim 状态。`ready` 才表示任务已被显式放入可执行队列；普通 ready 任务通常仍应是 P1/P2/P3，不能为了表示“下一批可做”全部标成 P0。P0 只用于 incident、当前目标 blocker 或必须立即处理的任务；若 P0 task 仍缺规格、排期未到或依赖未完成，它仍保持 `triage` / `scheduled` / `todo`，不能被 claim。

Examples：

```bash
kanban task create "修复 claim 队列阻断回归" --priority 0
kanban task create "实现状态机" --priority 1
kanban task create "补充文档示例" --priority 2
kanban task create "明早检查报告" --scheduled-at 1780640400000
kanban task create "修复 API 回归" --label backend --label p1
```

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

`--details` 只改变人类可读输出，显示为易读字段列表。可用时包含 task
ref/id/status/title、完整多行 description、assignee、priority、labels、
scheduled_at、due_at、created_at、updated_at，以及其他 task snapshot 字段。
`--json task show` 无论是否带 `--details`，都返回相同的 `TaskRecord` envelope。

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
command service 处理，因此修改 description、scheduled_at 等会影响 spec 或
schedule 的字段后，服务会根据 spec、schedule 和当前 dependencies 重新计算
active task 的目标状态并写入对应事件。Dependency edge 通过 `kanban dep`
命令修改；`max_retries` 只更新 retry policy，不是 status recompute 触发器。

Examples：

```bash
kanban task update 12 --priority 1
kanban task update t_01HX --description "新的规格"
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

### 6.8 Reclaim

```bash
kanban task reclaim --expired
kanban task reclaim
```

当前 CLI reclaim 处理 active board 内 expired claims；裸 `kanban task reclaim` 与 `kanban task reclaim --expired` 等价。

### 6.9 Archive

```bash
kanban task archive <task_ref>
```

Options：

| Option | 说明 |
|---|---|
| `--force` | 允许 archive running task，并关闭 active run。 |

---

## 7. Dependency Commands

```bash
kanban dep add <parent_ref> <child_ref>
kanban dep remove <parent_ref> <child_ref>
kanban dep list <task_ref>
```

Human output for add/remove is Chinese-first:

```text
已添加依赖：default#1 -> default#2
已移除依赖：default#1 -> default#2
```

添加 dependency 后：

- 如果 child 当前是 `ready` 且 parent 未完成（不是 `done` 或 `archived`），child 降级为 `todo`。
- parent 完成、归档或 dependency 移除后，child 保持 `todo`；需要 `kanban task promote <task_ref>` 才显式进入 `ready`。归档 parent 不会删除 dependency edge。
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
kanban label add <task_ref> <label>
kanban label remove <task_ref> <label>
kanban label suggest <task_ref> [--limit 5] [--atom-limit 24] [--min-score 0.15] [--vector-config <toml>] [--json]
```

`label create` 创建当前 board 作用域内的 label；如果同一 board 已存在同名
label，返回已有 label。`label add` 接受 task ref 和 label 名称或 id；如果按
name 指定的 label 不存在，会先在该 task 所属 board 创建 label，再绑定到 task。
`label remove` 接受 task ref 和 label 名称或 id。空白 label 名称会被拒绝。

Label 变更对 task-label 关联保持幂等。只有关联实际变化时，才追加
`task.label.added` / `task.label.removed` event；该操作不改变 task status。

示例：

```bash
kanban label create backend --color blue
kanban label add default#12 backend
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

`label suggest` 返回 task-level label suggestions。当前实现使用
`lancedb_label_atoms` atom hit aggregation：用 task title + description 查询语义
label atoms，按 label 聚合正向 similarity，并按负向 atom similarity 惩罚分数。
它不会调用 full label solver refit，不会自动创建新 label，也不会写入 new-label
proposal。应用建议时仍使用现有 `label add <task_ref> <label>` / API attach 流程。

默认未配置 vector provider 或二进制未启用 `vector-lancedb` 时，命令成功返回
degraded 结果而不是失败；无 provider 时 `needs_new_label=false`。`--vector-config`
使用与 `kanban vector configure/status` 相同的 TOML 解析规则。`LabelAtomHit.score`
来自 LanceDB `_distance`，CLI/API suggestion 分数会先转换为
`1 / (1 + max(distance, 0))` similarity 再聚合。

JSON 输出：

```json
{
  "data": {
    "task_id": "t_01HX...",
    "board_id": "b_01HX...",
    "selected_labels": [],
    "candidates": [],
    "coverage": 0.0,
    "residual_norm": 1.0,
    "needs_new_label": false,
    "degraded": true,
    "diagnostics": ["solver_refit_unavailable", "vector_store_disabled"]
  }
}
```

Human output 简洁列出建议 label、score、weight、already_applied；degraded 时追加
diagnostics 行。

---

## 9. DAG Commands

```bash
kanban dag show
kanban dag show --json
kanban dag ancestors <task_ref>
kanban dag ancestors <task_ref> --json
```

`kanban dag show` returns an LLM-friendly snapshot for the active board. The
CLI calls the SQLite service query; it does not assemble graph SQL in the CLI.
Human output is a concise summary. JSON output is the stable contract and uses
the standard envelope:

```json
{
  "data": {
    "board": {
      "id": "b_...",
      "slug": "default",
      "name": "Default"
    },
    "snapshot": {
      "generated_at": 1717520000000,
      "node_count": 3,
      "edge_count": 1,
      "sort": [
        "priority asc",
        "due_at asc nulls last",
        "scheduled_at asc nulls last",
        "dependency fan-out desc",
        "created_at asc",
        "ref asc",
        "id asc"
      ]
    },
    "raw": {
      "nodes": [
        {
          "id": "t_...",
          "ref": "default#1",
          "seq": 1,
          "title": "Implement state machine",
          "status": "ready",
          "priority": 1,
          "due_at": null,
          "scheduled_at": null,
          "created_at": 1717520000000,
          "archived_at": null,
          "why": "default#1 当前状态为 ready"
        }
      ],
      "edges": [
        {
          "parent": "t_parent",
          "child": "t_child",
          "why": "t_parent 必须先完成，t_child 才能执行"
        }
      ]
    },
    "derived": {
      "blocked_by": [
        {
          "task_id": "t_child",
          "tasks": ["t_parent"],
          "why": "default#2 被以下前置任务阻塞：default#1"
        }
      ],
      "unblocks": [
        {
          "task_id": "t_parent",
          "tasks": ["t_child"],
          "why": "default#1 解除后会放行：default#2"
        }
      ],
      "actionable": [
        {
          "task_id": "t_ready",
          "ref": "default#3",
          "why": "default#3 状态为 ready，没有未完成的前置依赖"
        }
      ],
      "frontier": [
        {
          "task_id": "t_ready",
          "ref": "default#3",
          "why": "default#3 是 frontier：状态为 ready，且前置依赖已完成或不存在"
        }
      ]
    }
  }
}
```

Frontier v1 includes only unarchived `todo` and `ready` tasks with no unfinished
parent dependencies, where unfinished means not `done` or `archived`. It excludes `done`, `archived`, `blocked`, `running`, and
`review` tasks. Nodes and frontier entries use the documented stable sort:
priority ascending (P0 -> P3), due date ascending with nulls last, scheduled time
ascending with nulls last, dependency fan-out descending, created time
ascending, then task ref and id.

`kanban dag ancestors <task_ref>` returns the target task plus all unarchived
ancestor tasks reachable through parent -> child dependency edges. Ancestors are
ordered before descendants and the target appears last. Same-level ordering
follows the `dag show` node order where practical. Human output is
LLM-friendly Markdown:

```markdown
# Ancestors for default#3

Target: default#3 `t_target` [todo] Implement feature
Generated at: 1717520000000

## Ordered Tasks
- [1] default#1 `t_root` [done] Root prerequisite
- [2] default#2 `t_middle` [ready] Middle prerequisite
- [3] default#3 `t_target` [todo] Implement feature

## Dependency Edges
- `t_root` -> `t_middle`: t_root 必须先完成，t_middle 才能执行
- `t_middle` -> `t_target`: t_middle 必须先完成，t_target 才能执行
```

JSON output uses the standard envelope:

```json
{
  "data": {
    "target": {
      "id": "t_target",
      "ref": "default#3",
      "seq": 3,
      "title": "Implement feature",
      "status": "todo",
      "priority": 1,
      "due_at": null,
      "scheduled_at": null,
      "created_at": 1717520000000,
      "archived_at": null,
      "why": "default#3 当前状态为 todo"
    },
    "nodes": [
      {
        "id": "t_root",
        "ref": "default#1",
        "seq": 1,
        "title": "Root prerequisite",
        "status": "done",
        "priority": 1,
        "due_at": null,
        "scheduled_at": null,
        "created_at": 1717510000000,
        "archived_at": null,
        "why": "default#1 当前状态为 done"
      },
      {
        "id": "t_middle",
        "ref": "default#2",
        "seq": 2,
        "title": "Middle prerequisite",
        "status": "ready",
        "priority": 1,
        "due_at": null,
        "scheduled_at": null,
        "created_at": 1717515000000,
        "archived_at": null,
        "why": "default#2 当前状态为 ready"
      },
      {
        "id": "t_target",
        "ref": "default#3",
        "seq": 3,
        "title": "Implement feature",
        "status": "todo",
        "priority": 1,
        "due_at": null,
        "scheduled_at": null,
        "created_at": 1717520000000,
        "archived_at": null,
        "why": "default#3 当前状态为 todo"
      }
    ],
    "edges": [
      {
        "parent": "t_root",
        "child": "t_middle",
        "why": "t_root 必须先完成，t_middle 才能执行"
      },
      {
        "parent": "t_middle",
        "child": "t_target",
        "why": "t_middle 必须先完成，t_target 才能执行"
      }
    ],
    "ordered_refs": ["default#1", "default#2", "default#3"],
    "generated_at": 1717520000000
  }
}
```

---

## 10. Comment Commands

```bash
kanban comment add <task_ref> <body> [--kind note|decision] [--author-type user|agent] [--agent-type <type>] [--metadata-json <json>]
kanban comment list <task_ref>
```

`--actor` supplies the comment author display identity. If `--kind` is omitted,
the service default is `note`. If `--author-type` is omitted, the service default
is `user`; pass `--author-type agent --agent-type <type>` for Codex/dispatcher or
other automated writers. `--metadata-json` defaults to `{}` and must be a JSON
object. For `--kind decision`, it is required to satisfy the structured
decision schema: non-empty `options`, unique lowercase ASCII option `slug`
values, `selected` matching one slug, non-empty `reason`, and optional
non-empty `risk` / `verification`.

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

默认实现使用 SQLite fallback，不依赖外部/派生索引。启用 `tantivy-backend` feature 且 `index/v1/tasks/` 存在可读 Tantivy 索引时，`kanban search` 使用 Tantivy；缺失或损坏时回落 SQLite，并在 meta 中标记 stale。搜索匹配 task title、description、comments、run summary/error、event kind/payload。

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

默认 backend 是 SQLite fallback。启用 `tantivy-backend` feature 时，Tantivy index 是可重建 derived cache：

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

`kanban stats --json` 返回 status counts、过期 running claim 列表和 blocked reason 聚合，用于本地 operator recovery。

`kanban backup` 使用 SQLite `VACUUM INTO` 创建一致备份；目标文件已存在时失败，避免覆盖。
`kanban export --format jsonl` 导出数据库记录；目标文件已存在时失败，避免覆盖旧 snapshot。JSONL 不复制 `task_runs.log_path` 指向的外部日志文件，导出的 run 记录会清空 `log_path`；导出中的 live `running` task 会清除 claim 并恢复为 `ready`，对应 running run 会落为 `canceled`，并追加 `task.export_sanitized` 事件解释这次 portable snapshot 改写。需要完整可恢复副本时使用 `kanban backup`。
`kanban import` 是替换式恢复入口，必须显式传 `--replace`；导入文件必须至少包含一个 board，且每个 board 必须包含 columns。`kanban import --replace` 是 offline-only 操作；运行前必须停止 `kanban serve` 和常驻 `kanban dispatch`，如果检测到 active runtime lock 会直接拒绝。
`kanban entity`、`kanban outbox`、`kanban derived` 是 Knowledge Substrate 的只读维护入口。SQLite 仍是事实源；这些命令只报告统一 entity registry、派生索引 outbox 和 derived store 状态，不改变 task 状态或 claim。
`kanban graph` 和 `kanban vector` 是 feature-gated 派生层入口：未启用 `graph-oxigraph` / `vector-lancedb` 或缺少 embedding provider 时返回 disabled/degraded status；启用后仍只作为可重建 relation/vector store，不参与 task 状态事务。
`kanban vector configure` 默认写入全局 config：`$XDG_CONFIG_HOME/kb/config.toml`（平台默认通常为 `~/.config/kb/config.toml`），并默认配置本机 Ollama embedding provider。传 `--vector-config <toml>`（别名 `--config`）时写入指定 TOML。configure 默认调用 `/api/embed` 做短文本维度校验；校验失败时不写配置；`--skip-check` 只跳过这次连通性/维度检查。配置格式：

```toml
board = "kanban-tool"

[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "qwen3-embedding:0.6b"
dimensions = 1024
```

项目级 `.kb/config.toml` 可以覆盖全局 `[vector]`；命令行 `--vector-config <toml>` 优先级最高。解析顺序是：显式 `--vector-config`、最近的项目 `.kb/config.toml`、全局 config。`kanban board use <board>` 更新项目配置文件的 `board` 字段时必须保留该文件内已有 `[vector]` 配置。配置有效且启用 `vector-lancedb` 时，`kanban vector status/rebuild/sync` 和 `kanban context build` 使用该 provider；未配置时保持 disabled/degraded fallback。
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
- migrations 完整。
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
- `index_outbox` backlog：`outbox_pending`、`outbox_running`、`outbox_failed`。
- derived store health：`derived_dirty_stores`、`derived_error_stores`、`derived_stores[]`，每个 store 包含 `dirty`、`last_error` 和按 store target 聚合的 pending/running/failed outbox 计数。

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

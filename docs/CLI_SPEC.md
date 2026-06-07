# CLI SPEC

默认 binary 名称：`kb`

CLI 是一等入口；它与 Web 使用同一套 command service 和 SQLite schema。

---

## 1. Global Options

```bash
kb [GLOBAL_OPTIONS] <COMMAND>
```

| Option | 说明 |
|---|---|
| `--db <path>` | 指定 SQLite DB。默认从 config 读取。 |
| `--board <slug>` | 指定 board。默认 `default`。 |
| `--actor <name>` | 操作 actor。默认 OS username。 |
| `--json` | JSON 输出。 |
| `--no-color` | 禁用颜色。 |
| `--config <path>` | 指定 config.toml。 |
| `-v/--verbose` | 输出调试信息。 |

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

### 3.1 `kb init`

初始化本地 DB、默认 board、默认 columns。

```bash
kb init
kb init --db .kb/kb.db
kb init --force
```

输出：

```text
Initialized kb database at ~/.local/share/kb/kb.db
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
kb board list
```

### 4.2 Create board

```bash
kb board create <slug> --name <name>
```

Example：

```bash
kb board create agent-work --name "Agent Work"
```

### 4.3 Show board

```bash
kb board show <slug>
```

### 4.4 Archive board

```bash
kb board archive <slug>
```

---

## 5. Task Commands

### 5.1 Create task

```bash
kb task create <title> [OPTIONS]
```

Options：

| Option | 说明 |
|---|---|
| `--description <text>` | Markdown 描述。 |
| `--file <path>` | 从文件读取描述。 |
| `--status <status>` | 显式初始状态：triage/todo/scheduled/ready。 |
| `--assignee <name>` | assignee/worker profile。 |
| `--priority <int>` | 优先级，默认 0。 |
| `--scheduled-at <datetime>` | 计划时间。 |
| `--due-at <datetime>` | 截止时间。 |
| `--depends-on <task_id>` | 添加 parent dependency，可重复。 |
| `--label <name>` | 添加 label，可重复。 |
| `--metadata <json>` | 扩展 JSON。 |

Examples：

```bash
kb task create "实现状态机" --priority 10
kb task create "跑集成测试" --depends-on t_01HXABC
kb task create "明早检查报告" --scheduled-at "2026-06-05T09:00:00-07:00"
```

Human output：

```text
Created task #12 t_01HX... [ready] 实现状态机
```

JSON output：

```json
{
  "data": {
    "id": "t_01HX...",
    "seq": 12,
    "status": "ready",
    "title": "实现状态机"
  }
}
```

### 5.2 List tasks

```bash
kb task list [OPTIONS]
```

Options：

| Option | 说明 |
|---|---|
| `--status <status>` | 按状态过滤，可重复。 |
| `--assignee <name>` | 按 assignee。 |
| `--label <name>` | 按 label。 |
| `--search <query>` | title/description 模糊搜索。 |
| `--include-archived` | 包含 archived。 |
| `--limit <n>` | 限制数量。 |
| `--sort <field>` | priority/created/updated/position。 |

Examples：

```bash
kb task list
kb task list --status ready --status running
kb task list --assignee agent-default --json
```

### 5.3 Show task

```bash
kb task show <task_ref>
```

`task_ref` 支持：

- `t_...`
- `#12`

Options：

| Option | 说明 |
|---|---|
| `--events` | 展示 event timeline。 |
| `--comments` | 展示 comments。 |
| `--runs` | 展示 run history。 |

### 5.4 Update task fields

```bash
kb task update <task_ref> [OPTIONS]
```

允许更新：

- title
- description
- assignee
- priority
- scheduled_at
- due_at
- metadata

不允许通过 update 修改 status；status 必须通过 transition command。

Examples：

```bash
kb task update #12 --priority 20
kb task update t_01HX --description "新的规格"
```

---

## 6. Transition Commands

### 6.1 Specify

```bash
kb task specify <task_ref> --description <text>
kb task specify <task_ref> --file spec.md
```

用于 `triage -> todo/scheduled/ready`。

### 6.2 Promote

```bash
kb task promote <task_ref>
```

手动尝试 `todo/scheduled -> ready`。

### 6.3 Start / Claim

```bash
kb task start <task_ref> [OPTIONS]
kb task claim <task_ref> [OPTIONS]
```

`start` 是 `claim` 的人类友好 alias。

Options：

| Option | 说明 |
|---|---|
| `--ttl-ms <ms>` | claim TTL。默认 300000。 |
| `--worker-profile <name>` | worker profile。 |

Output：

```text
Claimed #12 t_01HX... as alice
Claim token: ct_...
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

### 6.4 Heartbeat

```bash
kb task heartbeat <task_ref> --claim-token <token>
```

Options：

| Option | 说明 |
|---|---|
| `--ttl-ms <ms>` | 延长 TTL。 |
| `--note <text>` | 可选备注。 |

### 6.5 Done / Complete

```bash
kb task done <task_ref> --claim-token <token>
kb task complete <task_ref> --claim-token <token>
```

Options：

| Option | 说明 |
|---|---|
| `--summary <text>` | 完成摘要。 |
| `--result-json <json>` | 结构化结果。 |
| `--force` | 强制完成 running task；仅本地人工修复使用。 |

### 6.6 Submit Review

```bash
kb task review <task_ref> --claim-token <token> --summary <text>
```

使 task 从 `running` 到 `review`。

### 6.7 Block

```bash
kb task block <task_ref> <reason>
```

Options：

| Option | 说明 |
|---|---|
| `--claim-token <token>` | running task block 时需要。 |
| `--force` | 强制 block。 |

### 6.8 Unblock

```bash
kb task unblock <task_ref>
```

不会盲目进入 ready，而是根据 spec、schedule、dependencies 重新计算目标状态。

### 6.9 Reclaim

```bash
kb task reclaim <task_ref>
kb task reclaim --expired
```

Options：

| Option | 说明 |
|---|---|
| `--force` | 强制 reclaim running task。 |
| `--to blocked|ready` | 指定目标状态。 |

### 6.10 Archive

```bash
kb task archive <task_ref>
```

Options：

| Option | 说明 |
|---|---|
| `--force` | 允许 archive running task，并关闭 active run。 |

---

## 7. Dependency Commands

```bash
kb dep add <parent_ref> <child_ref>
kb dep remove <parent_ref> <child_ref>
kb dep list <task_ref>
```

Alias：

```bash
kb task link <parent_ref> <child_ref>
kb task unlink <parent_ref> <child_ref>
```

添加 dependency 后：

- 如果 child 当前是 `ready` 且 parent 未完成，child 降级为 `todo`。
- 如果产生环，返回 exit code 6 或 invalid input。

---

## 8. Comment Commands

```bash
kb comment add <task_ref> <body>
kb comment list <task_ref>
```

也可：

```bash
kb task comment <task_ref> <body>
```

---

## 9. Event Commands

```bash
kb events <task_ref>
kb events --board default --after 120
kb events watch --board default
```

`watch` 持续输出新 events。

---

## 10. Run Commands

```bash
kb runs <task_ref>
kb run show <run_id>
kb run logs <run_id>
```

---

## 11. Dispatcher / Server Commands

```bash
kb serve
kb serve --open
kb serve --dispatcher

kb dispatch
kb dispatch --once
kb dispatch --worker-profile default
kb dispatch --worker-profile backend --profile-config ./workers.toml
kb dispatch --max-iterations 10 --poll-interval-ms 1000
```

`kb dispatch` is a foreground loop. Use `--once` for one pass, or `--max-iterations`
for bounded scripts/tests. `--profile-config` reads the selected `[workers.<name>]`
section and can set `command`, `claim_ttl_ms`, `heartbeat_interval_ms`,
`on_success`, `on_failure`, and `log_dir`.

---

## 12. Maintenance Commands

```bash
kb doctor
kb backup --out backup.sqlite
kb export --format jsonl --out board.jsonl
kb vacuum
kb checkpoint
```

### 12.1 `kb doctor`

检查：

- DB 文件存在。
- migrations 完整。
- `PRAGMA integrity_check`。
- orphan active run。
- running task 是否缺 claim。
- expired claim 数量。
- dependency cycle。

---

## 13. JSON Output Contract

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

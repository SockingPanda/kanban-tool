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
| `--max-retries <n>` | worker 失败或 reclaim 后最多重试次数。 |
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
| `--offset <n>` | 分页偏移。 |
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
- max_retries
- metadata

不允许通过 update 修改 status；status 必须通过 transition command。

Examples：

```bash
kb task update #12 --priority 20
kb task update t_01HX --description "新的规格"
kb task update t_01HX --max-retries 2
kb task update t_01HX --clear-max-retries
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
kb run logs <run_id> --tail-bytes 65536
```

`kb run logs` 默认最多读取 256 KiB。传 `--tail-bytes` 时只返回 log 末尾指定字节数。

---

## 11. Dispatcher / Server Commands

```bash
kb serve
kb serve --open
kb serve --dispatcher
kb serve --search-sync-interval-ms 5000

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

`kb serve` starts a conservative background search sync loop when the binary is
built with `tantivy-backend`. The loop makes one prompt startup attempt and then
calls `sync_search_index` every `--search-sync-interval-ms` milliseconds
(default `5000`). Use `--search-sync-interval-ms 0` to disable it. Without
`tantivy-backend`, the flag is accepted and no background index task is started.

---

## 12. Search Commands

### 12.1 `kb search`

```bash
kb search <query> [--status ready] [--status review] [--assignee worker-a] [--include-archived] [--limit 20] [--offset 0] [--json]
```

默认实现使用 SQLite fallback，不依赖外部/派生索引。启用 `tantivy-backend` feature 且 `index/v1/tasks/` 存在可读 Tantivy 索引时，`kb search` 使用 Tantivy；缺失或损坏时回落 SQLite，并在 meta 中标记 stale。搜索匹配 task title、description、comments、run summary/error、event kind/payload。

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

### 12.2 `kb index`

```bash
kb index status
kb index doctor
kb index rebuild
kb index sync
```

默认 backend 是 SQLite fallback。启用 `tantivy-backend` feature 时，Tantivy index 是可重建 derived cache：

- `status` returns backend/meta.
- `doctor` returns the same fallback health meta for scripts.
- `rebuild` builds/replaces `index/v1/tasks/` beside the SQLite DB and stores a clean high-watermark state in `app_settings`.
- `sync` consumes `task_events.id` after the stored high-watermark, delete+reindexes affected task aggregates, then advances the high-watermark only after a successful commit.
- Task mutations do not update Tantivy inside their transactions; run `kb index sync` after changes, rely on `kb serve` background sync for local server/desktop sessions, or use `kb index rebuild` to replace the derived index.

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

## 13. Maintenance Commands

```bash
kb doctor
kb stats
kb backup --out backup.sqlite
kb export --format jsonl --out board.jsonl
kb import --input board.jsonl --replace
kb vacuum
kb checkpoint

kb entity list [--kind task] [--limit 50]
kb entity show kb://task/t_...
kb outbox list [--status pending] [--limit 50]
kb derived status
kb graph status
kb graph neighbors kb://task/t_... [--predicate depends_on] [--limit 50]
kb vector status
kb context build t_... [--lexical-limit 5]
```

`kb stats --json` 返回 status counts、过期 running claim 列表和 blocked reason 聚合，用于本地 operator recovery。

`kb backup` 使用 SQLite `VACUUM INTO` 创建一致备份；目标文件已存在时失败，避免覆盖。
`kb export --format jsonl` 导出数据库记录；目标文件已存在时失败，避免覆盖旧 snapshot。JSONL 不复制 `task_runs.log_path` 指向的外部日志文件，导出的 run 记录会清空 `log_path`；导出中的 live `running` task 会清除 claim 并恢复为 `ready`，对应 running run 会落为 `canceled`，并追加 `task.export_sanitized` 事件解释这次 portable snapshot 改写。需要完整可恢复副本时使用 `kb backup`。
`kb import` 是替换式恢复入口，必须显式传 `--replace`；导入文件必须至少包含一个 board，且每个 board 必须包含 columns。`kb import --replace` 是 offline-only 操作；运行前必须停止 `kb serve` 和常驻 `kb dispatch`，如果检测到 active runtime lock 会直接拒绝。
`kb entity`、`kb outbox`、`kb derived` 是 Knowledge Substrate 的只读维护入口。SQLite 仍是事实源；这些命令只报告统一 entity registry、派生索引 outbox 和 derived store 状态，不改变 task 状态或 claim。
`kb graph` 和 `kb vector` 是 feature-gated 派生层入口：未启用 `graph-oxigraph` / `vector-lancedb` 或缺少 embedding provider 时返回 disabled/degraded status；启用后仍只作为可重建 relation/vector store，不参与 task 状态事务。
`kb context build` 通过 SQLite hydrate canonical task，并合并 lexical、graph、vector hits。graph/vector 不可用或失败时返回 degraded markers；失败原因通过有界 diagnostics 暴露，context pack 本身仍可用。

### 13.1 `kb doctor`

检查：

- DB 文件存在。
- migrations 完整。
- `PRAGMA integrity_check`。
- orphan active run。
- running task 是否缺 claim。
- expired claim 数量。
- dependency cycle。
- archived dependency edge。
- 缺失 run log 文件。
- `ready/running` task 带有未完成 parent dependency。
- `ready/running` task 缺少可执行 spec。
- `ready/running` task 带有未来 `scheduled_at`。
- `index_outbox` backlog：`outbox_pending`、`outbox_running`、`outbox_failed`。
- derived store health：`derived_dirty_stores`、`derived_error_stores`、`derived_stores[]`，每个 store 包含 `dirty`、`last_error` 和按 store target 聚合的 pending/running/failed outbox 计数。

`dirty` / pending outbox 表示派生层需要 sync/rebuild，不会改变 SQLite task truth；failed outbox 或 `last_error` 用于 operator 判断是否需要 `kb index sync`、`kb graph sync/rebuild` 或 `kb vector sync/rebuild`。

---

## 14. JSON Output Contract

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

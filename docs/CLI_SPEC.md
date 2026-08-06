# `kanban` CLI 规范

`kanban` 是 canonical localhost application host 的薄适配器。除 `serve`、配置/init、completion 和 Codex hook 外，命令都创建 `kanban-client` 并请求 `http://127.0.0.1:8721`；CLI 不打开、初始化或 fallback 到数据库。

## 1. 全局选项与错误

```text
kanban [OPTIONS] <COMMAND>
```

| 选项 | 来源/默认 | 作用 |
| --- | --- | --- |
| `--server-url <URL>` | `KANBAN_SERVER_URL` / `http://127.0.0.1:8721` | loopback host |
| `--board <SLUG-OR-ID>` | `KB_BOARD` / `default` | board-scoped selector context |
| `--db <PATH>` | `KANBAN_DB` → `KB_DB` → XDG data-local | 只供 `serve`/配置解析 |
| `--locale <auto|zh-CN|en>` | `KANBAN_LOCALE` / system | 人类输出语言 |
| `--actor <NAME>` | `KANBAN_ACTOR` / local user | `X-KB-Actor` 审计值 |
| `--json` | 关闭 | 稳定 JSON output |

JSON 成功通常为 `{ "data": ... }`；运行期错误使用 `error.code` 和 exit code，clap 参数错误仍退出 `2`。常见 exit code：`not_found=3`、状态/依赖 guard `=4`、claim/idempotency conflict `=5`、dependency blocked `=6`、`server_unavailable=9`、`feature_not_available=10`。

<!-- schema-doc-ignore: CLI 错误 envelope 说明性示例。 -->
```json
{"error":{"code":"server_unavailable","message":"服务端不可用：请检查服务端 URL，并确认已运行 `kanban serve`","exit_code":9}}
```

task selector 支持全局 `t_...`、`board#seq`、`#seq` 和当前 board 数字 seq；typed client 在需要全局 path 时先解析 selector。`--board` > `KB_BOARD` > 最近项目 `.kb/config.toml` > `default`；`--db` > `KANBAN_DB` > `KB_DB` > 项目/global config > XDG 默认路径。

## 2. 唯一 host 与本地 shell

```text
kanban serve [--db <PATH>] [--host <LOOPBACK-IP>] [--port <PORT>]
             [--dispatcher-profile <PATH>]
kanban init [--force]
kanban config show
kanban board use <BOARD>
kanban board current
kanban completions bash|zsh|fish|powershell|elvish
kanban __complete ...
```

只有 `serve` 打开/初始化/迁移/关闭 Turso。默认 `--host=127.0.0.1`、`--port=8721`；非 loopback 直接 `invalid_input`。无 profile 不启动 dispatcher；有 profile 才运行同进程单 worker。

`init` 幂等创建/复用 `.kb/config.toml`，`config show` 只解析值和来源，`board use/current` 只读写本地选择；它们不校验或创建 canonical board。completion 和隐藏 `__complete` 只处理静态/本地候选，不触库。

Codex hook：

```text
kanban hook codex install|status|uninstall
kanban hook codex handle failure|task-create
```

managed marker/fingerprint、原子写入和 handler stdin/stdout 协议由 CLI 负责；handler 不直接写 Turso。

## 3. Board、task、steps、dependency

```text
kanban board list [--include-archived]
kanban board columns [BOARD]
kanban board create <SLUG> <NAME>
kanban board show <BOARD>
kanban board archive <BOARD>

kanban task create <TITLE> [--description <TEXT>] [--status <STATUS>]
  [--assignee <NAME>] [--priority <0..=3>] [--scheduled-at <MS>] [--due-at <MS>]
  [--max-retries <N>] [--metadata <JSON>] [--labels <NAME>...]
  [--depends-on <TASK_SELECTOR>...] [--idempotency-key <KEY>] [--task-id <T_ID>]
kanban task list [filters...] [--limit <N>] [--offset <N>] [--sort <SORT>]
kanban task show <TASK_SELECTOR> [--details]
kanban task update <TASK_SELECTOR> [fields...]
```

`task.status` 只能通过显式 lifecycle command 改变；`task update` 只更新 service 允许的内容/排期/metadata 字段。list 支持 status、priority、plan_filter、assignee、query、archive 和完整 sort contract。

execution plan：

```text
kanban task step add <TASK_SELECTOR> <TITLE> [--body <TEXT>] [--link-task <TASK_SELECTOR>]
kanban task step list <TASK_SELECTOR>
kanban task step update <TASK_SELECTOR> <STEP_SELECTOR> [fields...]
kanban task step done <TASK_SELECTOR> <STEP_SELECTOR> --note <TEXT>
kanban task step skip <TASK_SELECTOR> <STEP_SELECTOR> --reason <TEXT>
kanban task step reopen <TASK_SELECTOR> <STEP_SELECTOR>
kanban task step remove <TASK_SELECTOR> <STEP_SELECTOR>
kanban task step not-required <TASK_SELECTOR> --reason <TEXT>
```

`STEP_SELECTOR` 为全局 `step_...` 或 task-local `S<n>`。step required/linked-task/position/status 都由 service 校验。

```text
kanban dep add <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
kanban dep list <TASK_SELECTOR>
kanban dep remove <CHILD_TASK_SELECTOR> <PARENT_TASK_SELECTOR>
```

`dependency` 是 `dep` visible alias；server 负责同 board、FK、唯一约束和 cycle 检查。

## 4. Lifecycle

```text
kanban task promote <TASK_SELECTOR>
kanban task specify <TASK_SELECTOR> [--description <TEXT>] [--scheduled-at <MS>]
kanban task claim <TASK_SELECTOR> [--ttl-ms <MS>] [--worker-profile <PROFILE>]
kanban task heartbeat <TASK_SELECTOR> --claim-token <TOKEN> [--ttl-ms <MS>] [--note <TEXT>]
kanban task release <TASK_SELECTOR> --claim-token <TOKEN>
kanban task review <TASK_SELECTOR> [--claim-token <TOKEN>] [--force]
kanban task done <TASK_SELECTOR> [--claim-token <TOKEN>] [--force]
kanban task block <TASK_SELECTOR> [<REASON>|--reason-file <PATH|->] [--claim-token <TOKEN>] [--force]
kanban task unblock <TASK_SELECTOR> [--reason <TEXT>]
kanban task reopen <TASK_SELECTOR> [--reason <TEXT>]
kanban task reclaim <TASK_SELECTOR> [--force]
kanban task archive <TASK_SELECTOR> [--force]
```

claim 是原子 `ready → running`，同时创建 run/event；heartbeat/release/review/done/block/reclaim 复用 owner/token、lease、CAS 和同一 transaction。`block` reason inline 与 `--reason-file` 互斥。

## 5. Labels、ontology、signals

```text
kanban label|labels|ontology list|create|add|remove
kanban label semantics list|show|upsert|delete
kanban label atoms list|explain
kanban label atom-index status|rebuild|query
kanban label suggest <TASK_SELECTOR>
kanban label propose <TASK_SELECTOR>
kanban label proposals list|show|accept|reject
kanban label ontology record|list|show|review|quality|confirm|reject|resolve|supersede|apply|revert|validate
```

label identity/binding、semantics CAS、atom index、proposal 和 ontology ledger 都经 host typed API；atom/index 是可重建 derived state，CAS/review/action event 由 service 负责。

```text
kanban signal record [--input <JSON-FILE|->]
kanban signal list [filters...]
kanban signal show <SIGNAL_ID>
kanban signal review [filters...]
kanban signal confirm|reject|resolve <SIGNAL_ID>... --reason <TEXT>
kanban signal supersede <SIGNAL_ID>... --by <SIGNAL_ID> --reason <TEXT>
```

signal record/backlink/review/lifecycle 共享一条事务 path；`--json` output 使用 `kanban-protocol` DTO。

## 6. Search、graph、vector、context

```text
kanban search <TEXT> [--status <STATUS>...] [--label <NAME>...] [--assignee <NAME>]
  [--include-archived] [--limit <N>] [--offset <N>]
kanban index status|doctor|rebuild|sync

kanban entity list|show|upsert ...
kanban graph status|neighbors|query|neighborhood|map|rebuild|sync ...

kanban vector configure|status|rebuild|sync|query-chunks|query-label-atoms ...
kanban context build [SUBJECT] [--task <TASK_ID>] [--reference <REF>] [--query <TEXT>]
  [--depth <N>] [--lexical-limit <N>] [--graph-limit <N>] [--vector-limit <N>]
  [--max-items <N>] [--budget <N>]
```

search 使用 Turso FTS，未 ready/stale 时回退 canonical SQL；graph 使用 canonical relations 的 bounded BFS；vector 使用 Turso `vector32` + host Ollama；context 是只读 bounded pack，按 provenance/rank/budget 去重，provider degraded 仍返回可用 lexical/canonical 结果。

## 7. Comments、attachments、runs、events

```text
kanban comment add <TASK_SELECTOR> <BODY> [--kind note|decision|signal] [options...]
kanban comment list <TASK_SELECTOR>

kanban attachment add <TASK_SELECTOR> <FILE> [--filename <NAME>] [options...]
kanban attachment list <TASK_SELECTOR>
kanban attachment download <TASK_SELECTOR> <ATTACHMENT_ID> --out <PATH>
kanban attachment remove <TASK_SELECTOR> <ATTACHMENT_ID>

kanban runs <TASK_SELECTOR>
kanban run show <RUN_ID>
kanban run logs|log <RUN_ID>
kanban events [TASK_SELECTOR] [--after <ID>] [--limit <N>]
```

attachment add/remove 只请求 host，download 写 raw bytes 到用户指定 output；run 不能独立 create/update，log 是固定 256 KiB bounded snapshot；events 保留未知 payload，CLI 可显示 task/event data。

## 8. Host-admin maintenance

```text
kanban doctor
kanban stats [--board <BOARD>]
kanban backup --path <PATH>
kanban export --path <PATH>
kanban import --path <PATH> [--replace]
kanban import-v30 --path <PATH> [--attachment-root <PATH>]
kanban checkpoint
kanban vacuum
kanban maintenance status
kanban maintenance run|rebuild|cleanup [--owner <OWNER>]
```

这些命令只通过 host 管理 API 执行。portable import/replace 使用 `import_journal`、verified backup、atomic transaction 和 derived rebuild；`import-v30` 未启用 `legacy-sqlite-import` 时 typed 返回 `feature_not_available`。MCP 不提供这些命令。

## 9. 停止行为和 gate 边界

host 停止或端口不可达时，已注册 command 返回 `server_unavailable`（exit `9`）；未知顶层 command 使用 external catch-all 返回 `feature_not_available`（exit `10`），不触碰存储。没有直接 DB fallback。

`kanban-protocol` 的 operation/surface catalog、fixture 和 adoption witness 是机器契约；本文件只描述实际 clap adapter。schema surface audit、adoption/full、Desktop package、release、push 和 PR 不因 CLI 文档同步自动运行或变绿。

### Canonical leaf 口径

`kanban-protocol::surface_operation_catalog()` 与 Clap 的 canonical `get_name()` 一一对应；
visible alias 不会产生第二个 leaf contract。当前新增并已接入的 leaf 包括 `board columns`、
`entity upsert`、`task specify`、`graph neighborhood`、`graph map`、`index rebuild` 和
`index sync`。旧 projection/admin、独立 lifecycle leaf 与旧 task-read path 不在 active
catalog 中；完整 exact 列表以 `schemas/json-schema/draft-2020-12/surface-operations.json`
和对应源代码为准。

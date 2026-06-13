# Implementation Plan

本文档给出分阶段实现计划、验收标准和测试策略。

---

## Phase 0：Repository Skeleton

目标：建立 Rust workspace 和基本工程纪律。

交付：

- `Cargo.toml` workspace。
- crates：`kanban-core`、`kanban-sqlite`、`kanban-cli`。
- migrations 目录。
- lint/format/test workflow。
- error type。
- ID/time utility。

验收：

- `cargo test` 通过。
- `cargo fmt --check` 通过。
- `cargo clippy` 无关键 warning。

---

## Phase 1：SQLite Schema + Core Domain

目标：数据结构和 migration 可用。

交付：

- 执行 `001_initial.sql`。
- `kanban init`。
- 默认 board。
- 默认 columns。
- 领域类型：Board、Task、Status、Run、Event。
- status enum 与 parse/serialize。

验收：

- 新建 DB 后 `PRAGMA integrity_check` 返回 ok。
- 重复运行 `kanban init` 不破坏已有 DB。
- schema version 可查询。

测试：

- migration test。
- schema smoke test。
- enum roundtrip test。

---

## Phase 2：Task CRUD + Events

目标：任务可创建、查询、更新，事件可记录。

交付：

- `kanban task create/list/show/update`。
- `task_events` 写入。
- `--json` 输出。
- `expected_lock_version` 乐观锁。

验收：

- 创建 task 后有 `task.created` event。
- update 不允许修改 status。
- board task list 默认隐藏 archived。

测试：

- create/list/show integration test。
- event transaction test。
- invalid input test。

---

## Phase 3：State Machine Transitions

目标：核心状态机可用。

交付：

- specify。
- promote。
- claim/start。
- heartbeat。
- complete/done。
- block。
- unblock。
- reclaim。
- archive。

验收：

- 非法 transition 被拒绝。
- 每个 transition 写 event。
- running task 必须有 claim token。
- complete 后清理 claim fields。

测试：

- transition matrix unit tests。
- block/unblock target recomputation tests。
- token mismatch tests。

---

## Phase 4：Dependencies

目标：支持 parent/child 依赖和显式 manual promotion。

交付：

- `kanban dep add/remove/list`。
- cycle detection。
- dependency-aware create/promote/claim。
- parent complete 后不自动 promote children；child 保持 `todo`，由 derived dependency fields 表达是否仍被阻塞。

验收：

- child 依赖未完成 parent 时不能 ready/running。
- parent 完成后 child 可被手动 promotion。
- cycle 添加失败。

测试：

- direct cycle。
- indirect cycle。
- child demotion when dependency added。
- manual promotion after completion。

---

## Phase 5：Runs + Dispatcher MVP

目标：可执行任务并恢复崩溃任务。

交付：

- `task_runs` 写入。
- `kanban dispatch --once`。
- `kanban dispatch` loop。
- worker profile command。
- heartbeat wrapper。
- expired reclaim。
- run logs。

验收：

- ready task 被 dispatcher claim 并执行。
- command exit 0 后 task done/review。
- command exit non-zero 后 task blocked/ready。
- worker timeout 后 reclaim/block。
- dispatcher crash 后 task 可被 reclaim。

测试：

- claim race test，多线程同时 claim 同 task 只有一个成功。
- worker success integration。
- worker failure integration。
- expired claim reclaim test。

---

## Phase 6：Local Web API

目标：Web 端可通过 HTTP 操作 board/task。

交付：

- `kanban serve`。
- REST endpoints。
- unified error response。
- SSE event stream。
- health endpoint。

验收：

- Web API 能完成 CLI 同等生命周期。
- SSE 收到 task events。
- API 不能 PATCH status。
- 默认只监听 127.0.0.1。

测试：

- route integration tests。
- API transition tests。
- SSE reconnect test。

---

## Phase 6.5：Board Lifecycle MVP

目标：让单 SQLite DB 内的多个 board 可通过 CLI/API 创建、选择、归档，并让 task ref 兼容未来聚合视图。

交付：

- `kanban board list/create/show/use/current/archive`。
- `POST /api/v1/boards` 与 `POST /api/v1/boards/{board}/archive`。
- 项目级 `.kb/config.toml` active board，解析顺序为 `--board`、`KB_BOARD`、最近项目 config、`default`。
- task ref 支持全局 `t_...`、当前 board 的裸 seq / `#seq`、显式 `board#seq` / `board/#seq`。
- CLI/API task 输出包含 `board_slug` 和可复制 `ref`。

验收：

- `board create` 创建默认 columns 并写 `board.created` event。
- `board use` 写入项目级 `.kb/config.toml`。
- `t_...` 可跨 active board resolve，`board#seq` 可显式跨 board resolve。
- Archived board 默认隐藏且拒绝普通写入；events/runs/comments 历史仍可读。
- Board archive 在存在 `running` task/run 时被拒绝。
- Duplicate board slug 返回 user-facing invalid input / HTTP 400。

---

## Phase 7：Web UI MVP

目标：基本看板 UI 可用。

交付：

- Board columns。
- Task cards。
- Task detail drawer。
- Create/update task。
- Drag/drop 调用 transition。
- Comments。
- Event timeline。
- Run history。
- SSE live refresh。

验收：

- UI 不直接修改 status。
- blocked unblock 后根据服务端返回移动列。
- running complete force 时有确认。

---

## Phase 8：Maintenance & Hardening

目标：本地数据可靠性。

交付：

- `kanban doctor`。
- `kanban backup`。
- `kanban checkpoint`。
- `kanban vacuum`。
- JSONL export。
- orphan run 检查。

验收：

- backup 可恢复。
- integrity check 可报告问题。
- expired running task 可列出并 reclaim。

---

## Testing Strategy

### Unit Tests

- status parse/format。
- transition guard。
- initial status computation。
- unblock target computation。
- dependency cycle detection。

### SQLite Integration Tests

- migration fresh DB。
- migration idempotency。
- FK constraints。
- JSON validation constraints。
- CAS claim affected rows。

### Concurrency Tests

- 10/50/100 concurrent claim attempts on one task。
- CLI + server simultaneous writes。
- busy timeout behavior。

### Dispatcher Tests

- successful worker。
- failed worker。
- timeout worker。
- heartbeat extension。
- reclaim expired。

### CLI Golden Tests

- human output stable enough。
- JSON output exact contract。
- exit code mapping。

### API Tests

- task lifecycle。
- invalid transition HTTP 409。
- error body format。
- SSE stream ordering。

---

## Definition of Done for MVP

MVP 完成定义：

1. `kanban init` 创建可用 DB。
2. `kanban board list/create/show/use/current/archive` 可用。
3. `kanban task create/list/show/update` 可用，输出包含 `board#seq` ref。
4. `kanban task start/heartbeat/done/block/unblock/archive` 可用。
5. dependencies 可用。
6. events 可用。
7. runs 可用。
8. dispatcher 可执行本地命令。
9. web API 覆盖核心 lifecycle。
10. web UI 可视化 board。
11. 并发 claim 测试稳定通过。
12. `kanban doctor` 能发现基本数据异常。

---

## Recommended First Milestone

最小可用 milestone：

```text
Phase 0 + Phase 1 + Phase 2 + Phase 3 + 部分 Phase 4
```

即：

- SQLite schema。
- CLI task lifecycle。
- 状态机。
- events。
- dependencies 基础。

先不要做 Web UI 和 dispatcher，直到状态机与 schema 稳定。

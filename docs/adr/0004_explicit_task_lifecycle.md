# Explicit task lifecycle

## Status

Accepted

## Context

任务状态转换需要 owner/token、lease、依赖、required step 和 event 的一致性；通用
`transition(target_status)` 无法表达这些前置条件。

## Decision

使用显式 lifecycle commands（例如 claim、heartbeat、release、review、done、block、unblock、reopen、
reclaim 和 archive）。service 在单一事务中验证 guard、更新 snapshot/run、写 event，并在需要时 enqueue
projection。

## Consequences

transport adapter 只映射 typed command；并发 claim、失败回滚和审计 event 可以由同一 application path
保证。精确 HTTP/CLI/MCP surface 由 protocol catalog、router 和 Clap 持有。


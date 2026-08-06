# Canonical and derived data

## Status

Accepted

## Context

FTS、vector、graph/context 和 projection worker 需要可重建索引，但任务、历史和关系事实必须在索引
不可用时仍然可读写。

## Decision

业务事实、追加事件和 import journal 属于 canonical data；搜索、向量、图、context、projection jobs、
缓存和 capability probe 属于 derived/runtime data。derived data 必须可以删除并从 canonical facts 重建，
不能反向改变业务事实。

## Consequences

provider 或 projection 降级只影响对应查询和后台任务，不改变 canonical mutation。维护命令必须明确区分
rebuild、degraded 和失败。


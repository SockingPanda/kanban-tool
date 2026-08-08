# kanban Web

`apps/web` 是 kanban-tool 的 browser-first 产品前端。Browser 与 Linux Tauri Desktop 最终都加载
`kanban serve` 同源托管在 `/app/` 的同一份构建产物；Web 不直连 Turso，也不依赖 Tauri API。

当前迁移范围和完成证据由 [`docs/capability-ledger.md`](docs/capability-ledger.md) 持有。公开 wire
类型、schema 与 operation catalog 由 `kanban-protocol` 生成，手写 Web transport、query cache 和
UI intent 只能消费生成边界。

开发入口：

- `pnpm --filter @kanban-tool/web dev`
- `pnpm --filter @kanban-tool/web typecheck`
- `pnpm --filter @kanban-tool/web lint`
- `pnpm --filter @kanban-tool/web test`
- `pnpm --filter @kanban-tool/web build`

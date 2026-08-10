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

Stage09 fixed candidate 上的根 `just ci-full` 与 09A、09C、09D、09E formal proof 均分别通过；
`ci-full` 是独立 built-in gate，不编排这四条 proof lane。09A 的 real-host lane 由根
`just release-proof-09a` 编排：它先构建同一 `apps/web/dist`，再以临时 DB/显式 loopback port
启动 `kanban serve --web-dir apps/web/dist`，经 canonical CLI seed、重启 host 后运行 Chromium
full 与 Firefox key 的 [`release-proof.spec.ts`](tests/release-proof.spec.ts)。现有 `web-e2e`
的 Preview/fixture specs 是独立 mock lane，不计入这些 proof receipt；本 README 不构成
release/publish-ready 声明。

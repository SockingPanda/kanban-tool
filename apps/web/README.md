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

## Storybook 与 agent 入口

本地组件工作台使用 `pnpm --filter @kanban-tool/web storybook` 启动，默认地址为
`http://127.0.0.1:6006`。Storybook 同时在 `http://127.0.0.1:6006/mcp` 暴露官方
`@storybook/addon-mcp` endpoint；当前启用 docs 与 development toolset，未启用依赖
`@storybook/addon-vitest` 的 testing toolset。

仓库提供两条 Storybook CLI 路径：

- `pnpm --filter @kanban-tool/web storybook:ai --help`：查看官方 `storybook ai` 命令；
- `pnpm --filter @kanban-tool/web storybook:cli --help`：连接运行在 6006 的 Storybook，列出 MCP
  暴露的 agent commands；例如追加 `list-all-documentation` 可从终端读取组件与 docs 索引。

MCP 与 CLI 都只面向本地开发和 Storybook fixture。它们不连接 Turso、生产 API/SSE 或 canonical
mutation path；静态 Storybook build 仍只输出到根目录 `output/storybook/`。

09C formal proof 使用固定的浏览器 context：`colorScheme=light`、`reducedMotion=reduce`、Web
preference `theme=dark`、Astryx theme `astryx`。`system` 仍是有效的用户偏好选项，但不是 09C
formal aggregate 的基线；`neutral` 是 Astryx 的继承主题，不是运行时 theme identity。

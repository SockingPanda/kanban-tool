# kanban-mcp

`kanban-mcp` 通过 stdio 提供 MCP adapter。它使用 `kanban-client` 的原生 async gRPC 访问唯一
localhost host，并以 `kanban-protocol` 的 MCP catalog 注册 typed tools；不打开数据库，不复制
service 状态机。

stdio JSON-RPC、tool 输入和输出保持 MCP 契约。每个 tool 从 selector 解析到业务 RPC 连续 await，
共享 client 的连接；MCP 取消通知会丢弃在途调用 Future，取消对应 gRPC 请求。

启动前确认 `kanban serve` 已运行。host-admin 操作仍由 host/CLI 边界管理，MCP 不因 catalog 扩展而取得
第二条 mutation path。精确 tool 名称和输入 schema 以 MCP catalog 与 protocol schema 为准。

## 协议与启动

此入口仅支持 MCP `2026-07-28`。每次请求在 `_meta` 提供
`io.modelcontextprotocol/protocolVersion` 和 `io.modelcontextprotocol/clientCapabilities`；
可先调用 `server/discover` 获取版本、能力和身份。旧 `initialize` 握手不受支持。
该选择对应 [MCP 官方变更](https://modelcontextprotocol.io/specification/2026-07-28/changelog)。
客户端也必须支持这一协议；仓库中的 stdio 测试不代表已安装的日常客户端兼容。

无参数运行 stdio server；`kanban-mcp --inspect` 输出脱敏后的生效配置及目录，不连接 Host。
`KANBAN_MCP_CONFIG` 指向 JSON 配置文件，最小示例见
[work 配置](examples/mcp-config.work.json)，字段见 [配置 schema](examples/mcp-config.schema.json)。
启动时依次应用默认值、文件、显式环境变量。`KANBAN_SERVER_URL`、`KANBAN_ACTOR`、`KB_BOARD`
和 `KANBAN_MCP_PROFILE` 分别覆盖 Host 地址、actor、默认看板和 profile。
配置中的未知字段、无效边界或不存在的禁用工具会阻止启动。

默认 `work` 包含 boards、tasks、comments、context、attachments、dependencies、events、runs、
search、steps、lifecycle 和 stats。`all` 包含 canonical catalog 的全部领域工具；`read_only`
只包含 catalog 可证明为读取的工具。`disabled_tools` 在三种 profile 上继续缩小范围；工具目录、
执行、资源和 prompt 共享同一过滤策略。默认看板不是访问控制机制。

默认允许 8 个在途调用，整次调用超时 30 秒，参数最多 64 KiB，结果最多 256 KiB。
目录每页最多 32 项，需沿 `nextCursor` 读取；游标绑定当前配置和目录，不可跨目录使用。
目录及资源响应提供缓存元数据，动态业务资源不建议缓存。参数和结果预算用于业务 envelope，
不是 stdio 原始输入帧的内存上限。大附件使用 Host 文件上传入口；MCP 保留现有附件工具及 DTO，
受参数、base64 和结果总预算共同约束。

## 资源、提示模板与失败恢复

`kanban://server/instructions` 与 `kanban://server/runtime` 提供操作说明和运行配置；
`kanban://boards/{board}` 与 `kanban://tasks/{task_id}` 提供只读业务快照。
`plan_task` 和 `handoff_task` 仅生成包含任务资源链接的草稿，不写入、不 claim。
禁用对应读取工具后，资源模板和 prompt 也不可使用。

成功工具结果保持领域 DTO。业务失败返回 `isError=true`，并在文本及
`_meta.io.github.sockingpanda.kanban-tool/error` 中提供 `code`、`operation_status`、`retry_safe`
和 `next_step`；错误不占用成功 `outputSchema` 的 `structuredContent`。
`not_started` 表示调用尚未开始，`rejected` 表示明确业务拒绝，`unknown` 表示写入结果仍待核对。
结果超预算时，`succeeded` 仍表示业务已成功。超时、断连和取消不能证明回滚；先读取现状并核对
原请求标识，不能自动重试结果未知的写入。取消会丢弃原生 gRPC Future，并取消对应 HTTP/2 stream。

本入口不提供 MCP HTTP transport、订阅、MRTR、completion 或 MCP tasks extension。
对象、模块和周期的通用 CLI/MCP 工具不在此入口中；Web 与原生 client 可使用正式扩展 RPC。

MCP 的 domain tool 覆盖任务、labels/ontology/proposals、signals、search、graph、vector、context、
附件、steps、dependencies、runs/events 和 boards。契约完整性由 protocol catalog、MCP integration
tests 和 CI 验证，不能由 catalog 条目数量推断。

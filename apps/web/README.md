# kanban Web

`apps/web` 是 Browser 与 Linux Tauri Desktop 共用的产品前端。两者加载 `kanban serve` 在
`/app/` 同源提供的同一份构建产物，通过 binary gRPC-Web 查询和操作本地服务。

## 开始使用

启动 Host 后打开 `/app/`，从侧栏选择项目。任务工作区提供看板、列表和依赖图，任务详情在右侧
展开；窄屏使用模态详情。筛选、搜索、排序、分页和选中的任务保存在 URL 中，刷新或浏览器前进、
后退可以恢复。主题、语言和侧栏收展是本机偏好。

- 任务详情提供编辑、合法状态操作、依赖、步骤、讨论、普通标签、建议和附件。
- 运行记录提供真实 Run 与日志；项目动态显示服务事件。
- 侧栏保留任务、项目动态、运行记录、设置和项目切换，收起后保留图标入口。
- 设置分为外观、操作身份和连接与诊断；数据库路径、指纹和构建信息放在默认收起的技术详情中，
  健康与数据维护入口保留服务诊断及确认流程。
- 模块、迭代、项目能力地图和 Git AI 追溯入口暂不展示；任务列表显示已有负责人、步骤和依赖提示。
- 已移除页面的旧链接会回到同项目任务列表并显示提示。

看板按“待开始、进行中、待验收、已完成”分组显示；各任务仍保留服务的真实状态，详情和筛选可以
区分待分诊、已排期、已就绪、已阻塞与已归档。标题与说明在失去焦点时保存。`C` 打开创建表单，
`Ctrl/Cmd + K` 聚焦搜索，`Escape` 关闭浮层并恢复触发位置的焦点。

服务连接失败时，页面显示连接错误和重试入口。保存操作等待服务确认；失败时保留表单草稿。
任务创建成功而首个步骤失败时，表单只重试步骤；已提交的任务不会重复创建。
`tasks.status`、字段版本、claim token 和执行计划约束由服务决定，看板拖动与详情按钮使用同一套
合法动作。

## 开发与验证

在仓库根目录恢复依赖，再启动 Vite：

```bash
pnpm install --frozen-lockfile
pnpm --filter @kanban-tool/web dev
```

Vite 开发与 preview 都将 正式 RPC、健康检查和 bootstrap metadata 代理到同一个
`kanban serve`，默认地址为 `http://127.0.0.1:8721`。Host 使用其他端口时，在启动命令前设置
`KANBAN_HOST_URL`，或写入 `apps/web/.env.local`：

```bash
KANBAN_HOST_URL=http://127.0.0.1:18721 pnpm --filter @kanban-tool/web dev
KANBAN_HOST_URL=http://127.0.0.1:18721 pnpm --filter @kanban-tool/web preview
```

该变量只接受 `127.0.0.1`、`localhost` 或 `[::1]` 的 HTTP(S) origin，不接受凭据、路径、query
或 fragment。页面继续使用 `/app/` 与同源请求；preview 保持 strict CSP。代理校验入站 Host 和
Origin 后再使用上游 Host 的身份转发，流式响应随消费进度传递，取消请求会关闭上游连接。
`runtime.json` 和 `manifest.json` 来自配置的 Host，本地页面代码与样式仍由 Vite 提供；开发
代理中的 Host artifact 身份不能用来证明本地页面已经部署。

开发 runtime 的连接规则由 [`vite.config.ts`](vite.config.ts) 持有。生产入口从
`/app/runtime.json` 读取并校验配置，再显式注入 Host 数据源；测试夹具不会进入生产启动路径。

```bash
just web-check
just web-react-doctor-diff v4
just web-e2e
```

`web-check` 编排生成契约、类型、lint、React Doctor、单元及架构边界测试、构建和 artifact 检查。
React Doctor 固定为 `0.9.13`，完整扫描与增量扫描都以 warning 阻断，关闭 score 和 supply-chain
网络扫描；增量检查包含未跟踪的新源码。浏览器 fixture 测试与真实 Host 验收分别记录证据，构建
成功不代表浏览器或 Desktop 已通过验收。

`just release-proof-09d` 在同一个隔离 Host、Turso DB 和候选版本上运行真实浏览器验收；正式
证据要求开始与结束均为 clean worktree。Chromium 的 20 次性能采样、初始 Brotli 体积和
QueryService 时延预算由该入口执行，Firefox 另外验证关键交互。

2k/5k 负载使用 Atlas 当前的全局分页：列表和看板每页最多 100 项，看板列头数字是当前页的
卡片数。验收读取实际 footer、页码、列头和任务 ID，与同 Host 的 `ListTasks`、`GetStats`
逐项对照；列表翻完全部页，检查精确总量、无重复和无遗漏，同时检查状态/搜索筛选与排序窗口。
5k 另外通过实际控件切换 50→100 项，验证重置页码与窗口一致，再检查 Map 的 240 节点上限
和 100%→115% 缩放。证据保留真实 DOM 文本，不把当前页列计数当成全列总量。

## 架构与数据流

| 目录 | 职责 |
| --- | --- |
| `src/app` | 启动后的应用组合、路由呈现、侧栏和全局浮层 |
| `src/features` | 任务、运行记录、动态、健康、维护与设置的领域组件 |
| `src/application` | 数据源接口、订阅读取与展示映射、异步操作、通知和导航 |
| `src/domain` | 展示模型、纯查询与操作意图 |
| `src/adapters/host` | named RPC、共享查询流与 cursor、诊断查询和生成契约的实际接入 |
| `src/components` | 基础控件、布局和浮层 |
| `src/styles`、`src/platform` | 静态主题、浏览器能力和本机偏好 |

功能之间通过显式公开出口组合，功能内部使用直接导入。组件经 application 操作数据，不直接读写
网络或存储。查询按项目和条件隔离；切换项目时清理订阅并拒绝旧请求的迟到结果。

生产数据源通过 `QueryService.WatchQueries` 订阅已挂载的完整业务查询。同一数据源将目录、任务
分页、详情及其展开区、依赖图、运行记录、项目动态和诊断查询复用到一条 binary gRPC-Web 连接。
相同查询共享结果；查询集合随页面与折叠区的生命周期调整，无消费者后释放。项目动态订阅最近
的有界事件窗口，日志追加也通过查询结果进入页面。

完整快照与 byte splice delta 在结束帧通过大小、SHA-256 和具名类型校验后，才同时提交数据和
cursor。组件从自己依赖的已提交结果映射展示模型；流更新保留当前 URL、表单草稿、焦点与滚动。
断流丢弃未完成的暂存数据，重连携带已提交 cursor；明确的重试和写后同步等待服务端 `Ready`。查询归属、取消和恢复规则见 [查询订阅](docs/query-subscriptions.md)。
业务写入与一次性用户操作使用 `KanbanService`，`/health` 保留为启动探测。

`src/lib/api/generated` 由 `kanban-protocol` 生成，保留页面业务 DTO、错误与启动配置的 value
contract；类型和运行时 validator 保持同源，手写 adapter 在 `unknown` 边界完成验证。
DTO 中的 path/query 表示参数值，网络方法和路径由正式 RPC descriptor 决定。正式 Protobuf client 位于 `src/generated/rpc`，由根 `proto` 生成，
通过 `src/lib/rpc` 转换为页面使用的 DTO。基础控件使用 React 和静态 CSS，浅色、深色与响应式布局共享主题
token；不在运行时注入样式。依赖图按需加载。

64 位整数在 JavaScript 安全范围内使用 `number`，超出范围时保留 `bigint`，从查询结果到页面和
写入请求都不经过浮点舍入。超出日期控件范围的时间戳显示完整十进制值；保存其他属性时保留原始
时间戳。整数校验、动态 JSON、排序和编辑规则见 [数值契约](docs/numeric-values.md)。

Web artifact manifest、runtime 身份与 strict CSP 是 Browser/Desktop 共用的启动边界，详见
[架构决策](../../docs/adr/0006_browser_first_web_ui.md)。

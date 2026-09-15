# kanban Web

`apps/web` 是 Browser 与 Linux Tauri Desktop 共用的产品前端。两者加载 `kanban serve` 在
`/app/` 同源提供的同一份构建产物，通过 HTTP 和 SSE 操作现有 3.1.0 服务。

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

## 架构与数据流

| 目录 | 职责 |
| --- | --- |
| `src/app` | 启动后的应用组合、路由呈现、侧栏和全局浮层 |
| `src/features` | 任务、运行记录、动态、健康、维护与设置的领域组件 |
| `src/application` | 数据源接口、查询状态、异步操作、通知、导航和 SSE 同步编排 |
| `src/domain` | 展示模型、纯查询与操作意图 |
| `src/adapters/host` | typed HTTP、SSE 和生成契约的实际接入 |
| `src/components` | 基础控件、布局和浮层 |
| `src/styles`、`src/platform` | 静态主题、浏览器能力和本机偏好 |

功能之间通过显式公开出口组合，功能内部使用直接导入。组件经 application 操作数据，不直接读写
网络或存储。查询按项目和条件隔离；切换项目时清理订阅并拒绝旧请求的迟到结果。SSE、断线补读和
保守刷新规则见 [`docs/sse-invalidation.md`](docs/sse-invalidation.md)。

`src/lib/api/generated` 由 `kanban-protocol` 生成，类型和运行时 validator 保持同源；手写 adapter
在 `unknown` 边界完成验证。基础控件使用 React 和静态 CSS，浅色、深色与响应式布局共享主题
token；不在运行时注入样式。依赖图按需加载。

Web artifact manifest、runtime 身份与 strict CSP 是 Browser/Desktop 共用的启动边界，详见
[架构决策](../../docs/adr/0006_browser_first_web_ui.md)。

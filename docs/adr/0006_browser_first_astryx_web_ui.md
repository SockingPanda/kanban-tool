# Browser-first 的 Astryx 统一 Web UI

## 状态

Accepted

## 背景

当前产品 UI 位于 Desktop 的 React/Tauri 组合中，Tauri runtime command 与 Vite 开发配置已经形成
两种启动分支；若再增加独立浏览器入口，渲染、配置和事件同步会继续漂移。我们需要在保留现有业务
语义的前提下，用同一份 Web artifact 覆盖浏览器和 Tauri，并让 `kanban serve` 成为唯一的运行时
装配点。此次改造是直接升级而不是长期兼容层：旧 UI 只保留在历史 tag、构建产物或回滚材料中，
数据库事实和 application service 不随 UI 重写改变。

## 决策

### 统一入口与 wire 边界

- UI 采用 browser-first 架构，由 `kanban serve` 同源托管 `/app/`；浏览器和 Tauri 都加载同一份
  Web artifact，不维护两套渲染实现。Tauri 只负责窗口、sidecar 生命周期和本机连接，不复制业务
  状态机。
- Web 只使用当前 HTTP API 边界：同源 `/api/v1` 作为业务 API，根健康端点是 `/health`；`GET /`
  可以以 `307` 重定向到 `/app/`，但其他 UI 与 API 路由仍保持隔离。`/app` 资源路由与 protocol
  API catalog 分开，SPA fallback 不得吞掉 API 或静态资源错误。
- `kanban-protocol` 是 Web wire 的唯一事实源。Rust DTO、catalog 和生成的 Draft 2020 schema 生成
  Web 所需的 rendered API、error、runtime 和 SSE 类型、运行时 validator、fixture 与 contract hash。
  transport、路径/query 组装、SSE 生命周期、query invalidation 和 UI intent 保持手写，但任何跨
  generated boundary 的数据都必须经过生成类型和 validator；禁止未经检查的泛型请求或类型断言。
  runtime contract 也由 protocol owner 生成，不另设 Web 私有协议。

### Runtime 与持久 SSE

- `GET /app/runtime.json` 提供空的同源 `apiBaseUrl`、`/app/` base path、actor、默认 board、server
  与 protocol 版本和 Web build 标识。生产浏览器与 Tauri 都通过该 runtime 配置连接，开发环境可
  由 dev server 明确覆盖。
- 现有 `/api/v1/stream/events` 升级为持久 SSE，SSE 是主同步路径：事件按 active board 有序、至少
  一次投递；客户端以初始 `after` 和重连时的 `Last-Event-ID` 请求，服务端先补 catch-up 再接入 live
  流，不能产生空洞。客户端去重；未知事件类型或检测到序列缺口时保守地 refetch 受影响 board。
  连接每 15 秒发送 heartbeat；正常负载下 mutation-to-browser 延迟目标为 p95 不超过 1 秒、p99
  不超过 2 秒，断线后 5 秒内开始重连。
- 连接断开立即 refetch，并在等待重连期间使用 5 秒 polling fallback；连接恢复且完成 catch-up 后
  切回 SSE。UI 的 query cache 只作可重建投影，不改变 canonical 状态。

### 视觉基线与平台范围

- 视觉基线采用 Astryx `0.3.0` Beta。实现顺序是官方 composition/template，无法覆盖的领域组合
  才使用普通 React/CSS；不得 swizzle、fork 或引入第二套通用组件库。若核心能力仍阻塞目标，必须
  保留证据并阻塞该阶段，不能静默降低质量门槛或替换基线。
- 首个发布范围是 Linux desktop/browser（Chromium、Firefox 与 Tauri WebKitGTK）；不承诺移动端和
  其他桌面平台，但组件与路由不得为未来扩展设置结构性障碍。功能语义保持现有 rendered UI 能力，
  不借此增加新的领域操作。
- Tauri 使用私有、同版本的 `kanban serve` sidecar 和同一 Web dist。客户端先探测固定 loopback
  端口（默认 `8721`）并 attach；无兼容 host 时再 spawn 自己的 host。端口冲突必须走可诊断的恢复
  路径，不随机改端口。关闭窗口默认隐藏并保留 host；显式 Quit 只优雅停止本进程拥有的 child，超时
  才 force stop，外部 host 永不被杀死。浏览器只通过 HTTP attach，不启动或管理 sidecar。

### Cutover 与回滚

- 改造期间允许旧壳与新 Web 分阶段并存，但每个阶段完成后都以新 artifact 为唯一继续演进的实现；
  不保留历史 UI 的运行时兼容、localStorage 迁移或双写路径。新偏好使用新的 `kb:web:*` 命名空间，
  cutover 时可直接重置主题、语言和侧栏状态。
- 本次 UI cutover 不做数据库 schema 或 migration 重写；若实现需要改变 canonical schema，必须另开
  migration 阶段和 ADR。回滚只接受精确匹配版本的 host、Web dist、desktop/CLI deb 与 manifest
  组合，数据库保持不变；不得用“旧 UI + 新 host”或“新 UI + 旧 host”的未验证混搭作为回滚方案。

## 取舍与后果

统一 Web artifact 和同源 host 减少了桌面/浏览器行为漂移，并把协议验证、SSE 重连和 runtime 版本
检查集中到可测试的边界；代价是 `kanban serve` 的静态资源装配、CSP、Host/Origin 校验和 build
一致性成为所有入口的共同前置条件。持久 SSE 提供及时更新和可恢复的事件序列，但需要维护 catch-up、
去重、未知事件和 polling fallback 的状态机，不能把“连上 SSE”当作数据一致性的证明。

直接升级省去历史兼容代码和迁移成本，却要求切换前完成 contract、artifact 和 packaged desktop
的成套验证；精确 artifact 回滚与数据库不变约束降低了失败半径，同时意味着旧版本只能从明确的
历史材料恢复。Astryx 官方组合优先保留基线升级路径，普通 React/CSS fallback 保证领域 UI 可交付，
但核心能力被 blocker policy 约束，不能以未审计的替代库掩盖设计系统风险。

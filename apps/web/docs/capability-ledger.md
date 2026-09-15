# Web 能力验收契约

Browser 与 Desktop 加载同一份 `/app/` 构建。此表列出保留能力的验收边界；机器入口由
[检查配置](capability-ledger.release.json) 持有，每次通过与否以绑定实际构建标识的运行证据为准。
配置中的检查入口不代表当前工作区已完成验收。界面架构和使用路径见 [Web 指南](../README.md)。

| 能力 | 用户结果 | 验证边界 |
| --- | --- | --- |
| `shell.runtime` | 运行时、构建身份与同源启动 | 真实 Host 与 Chromium、Firefox 验收 |
| `task.collection` | 任务集合与 URL 查询 | 真实 Host 与 Chromium、Firefox 验收 |
| `board.view` | 看板与合法跨列动作 | 真实 Host 与 Chromium、Firefox 验收 |
| `list.view` | 列表、搜索、筛选、排序及分页 | 真实 Host 与 Chromium、Firefox 验收 |
| `map.view` | 任务依赖图 | 真实 Host 与 Chromium、Firefox 验收 |
| `runs.view` | 运行记录与日志 | 真实 Host 与 Chromium、Firefox 验收 |
| `events.view` | 项目动态及事件游标 | 真实 Host 与 Chromium、Firefox 验收 |
| `health.view` | 健康诊断 | 真实 Host 与 Chromium、Firefox 验收 |
| `maintenance.view` | 维护与确认流程 | 真实 Host 与 Chromium、Firefox 验收 |
| `settings.view` | 设置与本机偏好 | 真实 Host 与 Chromium、Firefox 验收 |
| `task.detail` | 右侧任务详情 | 真实 Host 与 Chromium、Firefox 验收 |
| `task.create` | 任务创建 | 真实 Host 与 Chromium、Firefox 验收 |
| `task.transition` | 任务生命周期与原子 claim | 真实 Host 与 Chromium、Firefox 验收 |
| `task.comments` | 讨论 | 真实 Host 与 Chromium、Firefox 验收 |
| `task.dependencies` | 任务依赖 | 真实 Host 与 Chromium、Firefox 验收 |
| `task.steps` | 步骤与执行计划 | 真实 Host 与 Chromium、Firefox 验收 |
| `task.labels` | 普通标签与建议 | 真实 Host 与 Chromium、Firefox 验收 |
| `task.attachments` | 附件上传、下载及删除 | 真实 Host 与 Chromium、Firefox 验收 |
| `desktop.host` | Desktop 连接同版本 Host | 同一 artifact 的 Desktop 验收 |
| `desktop.lifecycle` | Desktop 窗口与 Host 生命周期 | 同一 artifact 的 Desktop 验收 |
| `desktop.package` | Desktop artifact 装配 | 同一 artifact 的 Desktop 验收 |
| `desktop.smoke` | Desktop 实际页面加载 | 同一 artifact 的 Desktop 验收 |

所有任务操作等待 3.1.0 服务确认，失败保留草稿；状态、字段版本、执行计划和 claim token 均沿用
服务语义。项目切换、分页、断线重连和迟到响应应维持项目隔离。普通标签继续使用现有服务能力。

已移除页面的旧链接返回同项目任务列表并显示提示，不恢复原页面或请求其专属数据。模块、迭代、
项目能力地图和 Git AI 追溯只提供标明“尚未接入”的禁用导航。

实际验收同时记录 runtime、manifest 与页面中的 Web build 标识，区分源码检查、构建、模拟数据的
浏览器测试、真实服务和 Desktop 页面证据。未运行的入口不得标为通过。

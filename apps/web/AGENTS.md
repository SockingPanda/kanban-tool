# Web UI 工作约定

## 产品边界

- `apps/web` 是 Browser 与 Tauri 共用的 product UI owner；两者加载 `kanban serve` 提供的同一
  `/app/` artifact。
- canonical mutation 经 generated contract 校验后的 localhost HTTP client 进入共享 service path；
  `tasks.status` 和 server transition 结果是生命周期事实。
- URL 持有 board、view、task inspector、filter、sort、search 和 pagination；用户偏好只写
  `kb:web:*` localStorage。

## 架构、组件与样式

- 基础组件由 `components/ui` 持有，领域组合由 `features` 持有，跨功能组合放在 `app`。
  公共出口显式列出，功能内部使用直接导入。
- `application` 持有数据源接口、异步操作和查询状态；`adapters/host` 接入现有生成契约、HTTP
  和 SSE。`domain` 只保留展示模型与纯计算，功能组件不直接访问网络或存储。
- 纸本浅深色、组件样式和 CSS Modules 使用静态 CSS；保持 strict CSP 和同源资源。
- 前端只保存 `kb:web:*` 用户偏好；生产入口明确注入 Host 数据源，连接失败显示错误及重试。
- `apps/web` 不导入 `@tauri-apps/*`；Host、tray、single-instance 和 deep link 属于 Desktop shell。

## Contract 与交付

- `src/lib/api/generated/` 只由 `xtask web-contracts generate` 写入；手写 transport 从 `unknown` 经
  generated validator 得到 typed value，不使用 unchecked generic request 或 wire type assertion。
- Map/ELK、Markdown 和其他重依赖按 route 或 inspector section lazy-load；共享模块使用直接 import，
  不通过宽泛 barrel 扩大 bundle。
- 每项功能的迁移包含状态、错误、键盘操作、浏览器测试及刷新证据。实现进度和审查发现写入
  Kanban task；功能边界和使用方式见本应用 README。

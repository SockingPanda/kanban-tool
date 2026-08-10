# Web UI 工作约定

## 产品边界

- `apps/web` 是 Browser 与 Tauri 共用的 product UI owner；两者加载 `kanban serve` 提供的同一
  `/app/` artifact。
- canonical mutation 经 generated contract 校验后的 localhost HTTP client 进入共享 service path；
  `tasks.status` 和 server transition 结果是生命周期事实。
- URL 持有 board、view、task inspector、filter、sort、search 和 pagination；用户偏好只写
  `kb:web:*` localStorage。

## 组件与样式

- 先查询已安装 Astryx CLI，并从官方 component、composition 或 ready template 选择通用 UI。
- Astryx 使用精确版本和 subpath import；本地组件只表达 kanban-tool 领域概念。
- 普通 React、CSS Modules 和静态 token CSS 承担领域组合与 fallback；保持 strict CSP，不产生
  runtime style injection。
- 不引入 Shadcn、直接 Radix wrapper、Tailwind、CVA、Astryx swizzle/fork 或第二套通用组件库。
- `apps/web` 不导入 `@tauri-apps/*`；Host、tray、single-instance 和 deep link 属于 Desktop shell。

## Contract 与交付

- `src/lib/api/generated/` 只由 `xtask web-contracts generate` 写入；手写 transport 从 `unknown` 经
  generated validator 得到 typed value，不使用 unchecked generic request 或 wire type assertion。
- Map/ELK、Markdown 和其他重依赖按 route 或 inspector section lazy-load；共享模块使用直接 import，
  不通过宽泛 barrel 扩大 bundle。
- 一次完成一个 capability-ledger 纵向切片；同一切片包含 states、errors、keyboard/a11y、Playwright
  与 invalidation evidence。实现进度和 review finding 写入 Kanban task，不复制进本文件。

# Desktop 布局边界

Desktop 的窗口只承载同源 `/app/` Web artifact；视觉和交互布局归 `apps/web` owner，Tauri 不维护
第二套视图或旧 Vite 入口。

Linux 桌面人工冒烟应覆盖：bootstrap loading/recovery、固定 `127.0.0.1:8721` host、从 bootstrap
打开 `/app/`，以及窗口关闭和托盘生命周期。Web 页面本身的键盘、滚动和窄窗口约束由
`apps/web` 的 Playwright 与 Web Interface Guidelines gate 验证。

# kanban Desktop 桌面端

Desktop 是 Linux-only 的 Tauri shell：它只负责窗口、托盘、固定 loopback host 的生命周期与
bootstrap recovery。用户界面由 `apps/web` 构建的唯一 Web artifact 提供；Desktop 不再包含第二套
React/Vite surface，也不直连 Turso。

## 运行链路

- `apps/desktop/bootstrap/` 是静态启动页，启动并探测固定的
  `http://127.0.0.1:8721/app/`。
- `apps/desktop/src-tauri` 负责启动 sidecar `kanban`、准备 app data 路径、托管窗口和系统托盘。
- `apps/desktop/src-tauri/tauri.conf.json` 将 `apps/web/dist/` 作为 `web/` 资源，并将生成的
  `bin/kanban` sidecar 放在资源根目录；Browser 与 Tauri 使用同一 Web artifact。

## 安全边界

产品是 local single-user 工具。Desktop 只 attach loopback，并用 `serverVersion`、
`protocolVersion` 和 Web `buildId` 的 exact identity probe 防止误连或版本漂移；这属于同一用户的
cooperative trust boundary，不提供对同 UID 恶意端口重绑的 cryptographic host pinning。固定端口
probe 到随后 navigation 的极窄 race 不引入第二套 auth，也不扩大 Browser/external attach 语义。

## 开发与打包

- `just desktop-dev-prep` 构建 Web artifact 和 debug `kanban` sidecar，供 Tauri bootstrap 使用。
- `just desktop-check` 运行 Web artifact、sidecar 和 Tauri Rust contract 检查。
- `just desktop-package` 构建 Linux `.deb`；`just desktop-package-layout` 验证唯一 Desktop
  binary、唯一 sidecar 和与 `apps/web/dist` 字节一致的 Web artifact。
- `just desktop-packaged-smoke` 在 extracted Deb 的 WebKitGTK/Xvfb/dbus 环境验证固定 host、`/app/`
  page load 与 normal Quit cleanup。

精确命令以根 `justfile` 和 Tauri config 为准。旧 Desktop React/Vite 入口已删除，不提供历史兼容层。

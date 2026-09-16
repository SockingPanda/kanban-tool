# Atlas 接入指南

## 1. 应用前

建立固定 f53e7b884b440f782020587c6f08a7e451757484 的独立工作树，将本包复制为 migration/grpc-framework。不要覆盖旧框架目录。运行 integration/apply-atlas.mjs 的 --check，再运行 --write。完整命令见根 README。

10 个原文件和 5 个新增文件由 integration/patch-plan.json 管理。检查包括目标 HEAD、完整原文件 Git blob、暂存区、替换锚点数量、符号链接、重复目标和安装位置。其他目录的未提交修改不阻止应用。

依赖 path 指向仓库内 migration/grpc-framework，因此应用器要求框架已经位于该目录。源码包不是原仓库的完整替换目录。

## 2. 联编

以下命令从目标仓库根目录执行。首次依赖解析可能更新锁文件，G01 必须审查并提交真实解析结果，之后再使用 --locked。当前包不含伪造锁文件。

```bash
scripts/cargo-build-lock.sh -- cargo check -p kanban-server --features grpc-framework --tests
scripts/cargo-build-lock.sh -- cargo test --manifest-path migration/grpc-framework/Cargo.toml --workspace
```

框架 TS 在 migration/grpc-framework/web 中安装 package.json 所列依赖，再运行 npm run generate、npm run typecheck、npm test。该步骤是独立包的生成验证；接入正式 apps/web 时，依赖与生成物需要纳入原 pnpm/xtask 流程，不建立第二份手写模型。

## 3. 装配现有 host

新增的 kanban_server::grpc::workspace_rpc_mount 接收 &AppState 和 Vec<HeaderValue>，返回 WorkspaceRpcMount { router, runtime }。

将 router 合并到原产品 router。标准路径是 /kanban.framework.v1.WorkspaceService/WatchChanges。浏览器与原生 gRPC 使用同一服务路径。/app/ 仍由原 Web artifact 服务，Vite 需要为该 RPC 前缀配置同源代理，不能通过放宽 CSP 连接另一个端口。

feature grpc-framework 同时启用 axum/http2。当前 helper 返回 Tower service，不直接混合两版 Axum Router。G01 应检查 tonic/axum/http-body trait 的实际兼容性。

允许 Origin 从真实监听地址、已验证 runtime 和开发配置产生。关闭时先拒绝新调用，再调用 runtime.stop().await，让活动刷新流退出，然后沿用原 HTTP/dispatcher graceful 或 force shutdown。不要 detached spawn 第二个 canonical host。

## 4. 注入 Atlas

框架 web/src/atlas-client.ts 导出 createAtlasRpcRealtime(baseUrl, documentUrl)。生产选择同源根地址，例如 window.location.origin。该工厂返回与 application BoardRealtimeSource 结构匹配的对象。

在现有 bootstrap 的 Host 数据源组合处增加 boardRealtime。传入 application 的仍是 WorkspaceDataSource，页面无需导入 generated Protobuf。当前安装变换没有修改 bootstrap，G04 需要在 G02 服务就绪后显式执行这一步。

如果旧 source 仍包含 streamTransport，registry 也只选择 boardRealtime 分支，失败不会回退。应在全量切换完成后删除这些旧字段，而非永久保留两套生产实时消费者。

## 5. 查询与任务命令

makeResource 保留 includeTasks:false。rpc-refresh-required 进入 session-events 的保守刷新编排，不能进入 generated audit event parser。旧 telemetry cursor 字段填 0 仅为控制消息占位；不得拿它更新 Last-Event-ID 或 task_events.id。

当前七字段 WatchBoard 不用于覆盖 BoardReadModel。完整任务页、详情、地图、评论、步骤、运行记录等由 G03/G07 补齐专属 typed snapshot/delta。

## 6. 接入后必须验证

执行原 just web-check、just web-react-doctor-diff v4 和原浏览器验收。确认筛选排序分页、URL 恢复、草稿、claim token、跨项目切换、同一页面多个消费者和关闭清理仍正确。真实 CLI/MCP 写入到页面更新由 G08 验证。

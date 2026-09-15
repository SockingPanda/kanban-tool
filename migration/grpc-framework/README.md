# kanban-tool Atlas gRPC 迁移框架

基于 `codex/v4-atlas-paper`，固定提交 `f53e7b884b440f782020587c6f08a7e451757484`。框架版本 0.2.0。

本包包含完整的迁移框架源码、Atlas 分支接线变换、测试、参考资料和剩余任务。它不是完整 kanban-tool 仓库镜像，也不是已发布应用。无需下载上一版框架才能使用本包。

## 已交付

- 保留 GetBoard、WatchBoard、UpdateTaskTitle 的命名 Protobuf RPC 和快照/增量核心。
- 新增 WorkspaceService.WatchChanges，使用原生 gRPC 或 gRPC-Web 传递 typed 查询失效通知。它覆盖尚未迁移到完整投影的 Atlas 查询，不模拟 SSE envelope。
- 新增 application 实时会话接口及绑定器。Atlas 会话显式选择注入的 RPC 数据源；RPC 失败不会创建 SSE fallback。
- 增加当前 kanban-server 的可选装配模块，复用既有 AppState/KanbanService。该模块不打开数据库、不绑定第二个端口。
- 固定分支应用器修改 10 个原文件，新增 5 个文件。没有修改 features、components、styles、app 页面、URL 查询模型或前端 bootstrap。

## 当前运行边界

默认生产 bootstrap 和 serve 装配点未切换。应用补丁后，旧 HTTP/SSE 路径仍按原方式运行，直到 G02 显式装配 RPC、G04 注入 boardRealtime。这样不会把正在重写的界面切到不完整的业务 API。

选择 RPC 后的实时通道只有 gRPC-Web。查询读取和完整业务命令仍需 G03/G04/G05 迁移；最终删除 REST/SSE 由 G09 验收。本包不将这些任务标记为完成。

Atlas 当前使用分页任务查询。7 字段 TaskCard 不能填充完整 BoardTask 或 Inspector。本次提供的 WatchChanges 触发原查询重新读取；完整数据通过 WatchBoard 的快照/增量模板或后续 query-scoped 流传递。两种流语义及限制见协议文档。

## 文档入口

| 文件 | 用途 |
| --- | --- |
| [分支核对](docs/00-atlas-branch-audit.md) | 固定提交、实际目录、旧框架不匹配点 |
| [范围与验收](docs/01-scope.md) | 已实现、未激活、后续任务的边界 |
| [架构](docs/02-architecture.md) | single host、service、RPC 与 Atlas application 关系 |
| [原投影流协议](docs/03-stream-protocol.md) | SnapshotBegin/Chunk/Commit、Delta、恢复 cursor |
| [取舍记录](docs/04-decisions.md) | 为什么保留查询边界、采用独立刷新流 |
| [接入指南](docs/05-integration.md) | 应用变换、host 装配、前端注入 |
| [任务清单](docs/06-tasks.md) | G01 至 G09 的可执行输入与验收 |
| [验证记录](docs/07-validation.md) | 105 项实际通过测试及未运行的 Rust/整仓检查 |
| [参考资料](docs/08-references.md) | 官方资料和分支固定链接 |
| [迁移映射](docs/09-migration-map.md) | 新旧路径、职责和退出条件 |
| [调试手册](docs/10-debugging.md) | 断线、失效、跨源、版本与日志排查 |
| [Atlas 刷新协议](docs/11-atlas-refresh-protocol.md) | 新增 WatchChanges 的完整语义 |

机器任务计划在 [tasks/tasks.json](tasks/tasks.json)。它不是 kanban import 格式，没有写入你的实际看板。

## 应用到独立工作树

在已有仓库中创建固定版本的独立工作树，然后把本目录复制到指定位置。不要把两个框架包叠加到同一个目录。

```bash
# 从已有 kanban-tool 仓库执行；该提交必须已在本地存在。
git worktree add --detach ../kanban-atlas-grpc f53e7b884b440f782020587c6f08a7e451757484
mkdir -p ../kanban-atlas-grpc/migration
cp -a /path/to/kanban-grpc-atlas-framework ../kanban-atlas-grpc/migration/grpc-framework
node ../kanban-atlas-grpc/migration/grpc-framework/integration/apply-atlas.mjs ../kanban-atlas-grpc --check
node ../kanban-atlas-grpc/migration/grpc-framework/integration/apply-atlas.mjs ../kanban-atlas-grpc --write
git -C ../kanban-atlas-grpc diff --check
```

应用器不提交、推送、启动服务或操作数据库。目标 HEAD、目标 blob、暂存区和锚点必须匹配。其他文件的未提交改动保留。并发编辑期间的回退仅为协作环境下尽力回退，不提供敌对进程下的原子事务保证。

## 验证状态

本轮实际执行了 68 项框架前端测试、10 项 Protobuf 编解码测试、13 项应用器夹具测试、14 项 Atlas 接线/边界测试，共 105 项通过。Atlas 接线测试包含真实修改区块的执行和声明形状夹具的类型检查，不等于完整应用联编。

当前环境没有 Rust/Cargo，且无法下载完整仓库归档。Rust 编译、Rust 测试、生成 TS 客户端联编、完整分支应用检查、浏览器与 Desktop 端到端验证均未运行。依赖锁文件未解析，不手写或伪造 Cargo.lock、pnpm-lock.yaml。

源码清单、内容哈希与验证分类在 [delivery-manifest.json](delivery-manifest.json)。

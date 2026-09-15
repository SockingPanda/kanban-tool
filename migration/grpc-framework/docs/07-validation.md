# 本轮验证记录

基线：codex/v4-atlas-paper，f53e7b884b440f782020587c6f08a7e451757484。以下结果来自本轮本地执行，不沿用上一包的通过数量。

| 检查 | 结果 | 范围 |
| --- | --- | --- |
| 框架 TS core 严格类型检查 | 通过 | model/reducer/watch/realtime-port/changes/endpoint |
| 前端行为测试 | 68 项通过 | 原快照恢复 43 项，加 Atlas 刷新/取消/同源等 25 项 |
| Protobuf 编译与编解码 | 10 项通过 | 命名服务、uint64、presence、oneof、刷新帧 |
| 应用器夹具 | 13 项通过 | dirty/staged 文件、固定 HEAD、重复目标、安装路径、回退 |
| Atlas 接线与边界测试 | 14 项通过 | 变换目标、RPC/SSE 选择、控制消息、状态映射、类型形状 |

总计 105 项实际通过，0 项失败。日志保存在 validation/，offline-results.json 记录每步命令、日志和退出状态。

## 夹具边界

Atlas registry 测试执行 patch-plan 中实际的控制器选择区块，依赖使用夹具。bind 类型检查使用已核对接口形状的声明夹具。应用器测试使用临时 Git 仓库，不是完整 Atlas checkout。这些结果不能说明原项目全部 imports、React 页面、生成 validator 或 Rust 服务均可编译。

原文件校验依据是本轮连接器返回的固定 SHA 和 Git blob。完整目标归档下载失败，因此没有在本地真实分支执行 --write 和整仓 diff-check。

## 未运行

Rust/Cargo 不在当前环境中，Rust 编译、Rust 测试、rustfmt、clippy 未运行。完整生成客户端依赖没有恢复，因此 atlas-client.ts 和原 client.ts 的生成类型联编未运行。真实 gRPC/gRPC-Web 互通、Atlas Web、CLI/MCP、Desktop、生产 artifact/CSP、锁文件解析均未运行。

包中保留原 Rust 测试，并新增 5 项真实刷新流互通测试源码，等待 G01/G08 执行。protobuf 的实际编译不代表 tonic Rust bindings 已联编。

## 可复现离线检查

安装 TypeScript 并让 protoc 在 PATH，执行 node scripts/check-offline.mjs。typescript 通过 web/node_modules 解析；单独使用全局安装时可显式设置 TYPESCRIPT_MODULE。之后执行 node scripts/check-package.mjs。

首次依赖解析完成后，G01 必须将真实锁文件加入目标仓库，并把 --locked 构建记录附到任务，不通过手工改日志或降低门禁宣称迁移完成。

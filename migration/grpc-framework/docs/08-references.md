# 参考资料

## 目标分支的代码证据

所有仓库链接固定本次读取的提交，不依赖未来 branch HEAD。

- [Atlas Web 说明](https://github.com/SockingPanda/kanban-tool/blob/f53e7b884b440f782020587c6f08a7e451757484/apps/web/README.md)
- [WorkspaceDataSource](https://github.com/SockingPanda/kanban-tool/blob/f53e7b884b440f782020587c6f08a7e451757484/apps/web/src/application/workspace/data-source.ts)
- [会话注册表](https://github.com/SockingPanda/kanban-tool/blob/f53e7b884b440f782020587c6f08a7e451757484/apps/web/src/application/workspace/board-session-registry.ts)
- [includeTasks:false 的实际创建点](https://github.com/SockingPanda/kanban-tool/blob/f53e7b884b440f782020587c6f08a7e451757484/apps/web/src/application/workspace/use-board-session.tsx)
- [会话控制消息与查询刷新](https://github.com/SockingPanda/kanban-tool/blob/f53e7b884b440f782020587c6f08a7e451757484/apps/web/src/application/workspace/session-events.ts)
- [现有 SSE 查询同步边界](https://github.com/SockingPanda/kanban-tool/blob/f53e7b884b440f782020587c6f08a7e451757484/apps/web/docs/sse-invalidation.md)
- [single host 项目契约](https://github.com/SockingPanda/kanban-tool/blob/f53e7b884b440f782020587c6f08a7e451757484/AGENTS.md)

## 本轮核对的官方技术资料

[tonic-web 0.14.6](https://docs.rs/tonic-web/0.14.6/tonic_web/) 说明可以直接包装 tonic service，无需外部代理，并支持 unary/server-streaming。它不替我们实现查询一致性、业务游标或历史保存。

[Connect Web 协议选择](https://connectrpc.com/docs/web/choosing-a-protocol/) 明确区分 createGrpcWebTransport 与 createConnectTransport。本包只使用前者；依赖库名称包含 Connect 不意味着使用 Connect wire protocol。

## 上轮架构来源与进一步阅读

[Anytype 桌面 API 文档](https://github.com/anyproto/anytype-ts/blob/develop/docs/src/ts/lib/api/README.md) 是采用命令调用和独立事件流的参考。该链接为 develop，会继续变化；不能当成本包验证过的固定协议版本。

[gRPC 核心概念](https://grpc.io/docs/what-is-grpc/core-concepts/) 用于理解 unary 和流式调用；[gRPC flow control](https://grpc.io/docs/guides/flow-control/) 用于理解传输背压。它们不提供 UI exactly-once 或断线后的业务恢复保证。

本包新增 RefreshSource、Atlas source 注入方式、查询刷新语义和预算是针对本项目的设计决定，不是上述项目的原样实现，也没有复制 Anytype 源码。

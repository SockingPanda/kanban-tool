# 范围与验收

本次保持“核心迁移框架 + 剩余任务”的约定。基线改为 codex/v4-atlas-paper 的 f53e7b8，沿用已经重写的前端。

交付验收是：真实框架源码存在；Protobuf 中有命名 unary 和 server-streaming；application 没有 Protobuf 依赖；server 装配复用现有 service；Atlas 通过显式 source 选择实时控制器；旧界面与查询模型不被替换；失败边界有测试；剩余工作明确到文件、步骤和验收。

当前代码完成上述源码和离线验证范围。框架未激活为默认产品路径，Rust 与完整分支联编仍须 G01/G02 处理。

非目标包括：一次迁移所有业务接口、重写纸本界面、添加模块/迭代的假后端、全量换状态库、增加多用户或远程访问、引入另一份 canonical 数据库、自动提交/推送。

以下事项不能作为“框架已提供”的隐含结论：所有 105 个 MCP 工具已迁移、全部业务 API 已转为 gRPC、原 SSE 已删除、分页查询已支持 server-side live query、Desktop 已打包通过。

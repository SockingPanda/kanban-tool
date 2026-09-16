# 架构决策

这里只保留仍然影响当前架构的长期决定。每个决定独立成文；实现进度、测试结果、已完成 migration
ledger、旧 recovery runbook、聚合快照、baseline 和一次性 workaround 不进入 ADR 或 active 文档树；
历史由 Git/tag、release asset 或 task record 持有。

- [0001 单 Host 的 Turso 所有权](0001_single_host_turso.md)
- [0002 状态是规范事实](0002_status_is_canonical.md)
- [0003 规范事实与派生数据](0003_canonical_and_derived_data.md)
- [0004 显式任务生命周期](0004_explicit_task_lifecycle.md)
- [0005 文档事实源与领域语言分层](0005_documentation_sources_of_truth.md)
- [0006 Browser-first 的统一 Web UI](0006_browser_first_web_ui.md)
- [0007 唯一 Host 的本地 gRPC](0007_local_grpc.md)
- [0008 对象关系作为唯一关系事实](0008_object_relations.md)
- [0009 文件身份与对象关联分离](0009_file_objects.md)
- [0010 协作规则、技能路由与验证入口](0010_repository_collaboration.md)

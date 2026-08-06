# Codex 环境配置

`.config/codex` 保存仓库使用的 Codex 环境配置和脚本入口。它服务于开发、测试和审阅，不改变
kanban-tool 的本地优先产品边界，也不是部署或远程 host 配置。

当前环境的 toolchain、安装脚本、缓存和网络权限以 Codex 环境设置及仓库脚本为准；不要把临时
环境变量、一次性 prompt、gate 结果或 migration 进度写入长期产品指南。项目任务仍从根
[`AGENTS.md`](../../AGENTS.md) 和当前 `justfile` 选择验证。


# Codex 环境配置

`.config/codex` 保存仓库使用的 Codex 环境配置和脚本入口。它服务于开发、测试和审阅，不改变
kanban-tool 的本地优先产品边界，也不是部署或远程 host 配置。

当前环境的 toolchain、安装脚本、缓存和网络权限以 Codex 环境设置及仓库脚本为准；不要把临时
环境变量、一次性 prompt、gate 结果或 migration 进度写入长期产品指南。项目任务仍从根
[`AGENTS.md`](../../AGENTS.md) 和当前 `justfile` 选择验证。

## Impeccable

仓库将 Impeccable 作为 project-local Codex skill 安装在
`.agents/skills/impeccable/`，并通过 `.codex/hooks.json` 运行项目级设计 Hook。当前安装使用
CLI `3.5.0` 取得 Skill `4.0.4`；Skill 的精确内容由 Git 持有，更新必须重新审查完整 diff。
对应上游 tag 为 `cli-v3.5.0`（`c0a3775266a5ef8306b426b77588ca2c7c6fd6fa`）和
`skill-v4.0.4`（`fb0942f57736841580a65088637f94da4a4ba87c`）。第三方 Skill 不按项目自有文案
规则改写；安装后只规范化 `diff-check` 拒绝的尾随空白和末尾空行。许可和派生内容归属见根
[`NOTICE.md`](../../NOTICE.md)。

最小安装与启用路径：

1. 使用 Node `22.18.0` 或更高版本运行
   `npx impeccable@3.5.0 install --providers=codex --scope=project`。
2. 重新载入 Codex，在 `/hooks` 中批准项目 Hook。
3. 使用 `$impeccable init` 建立产品上下文，再按 Web UI 任务选择 `document`、`shape`、
   `critique` 或 `audit`。

Impeccable 的产品与视觉上下文不替代 kanban-tool 的领域和架构事实源。自动更新检查、telemetry
和外部图片生成默认不属于项目工作流；运行时使用 `IMPECCABLE_NO_UPDATE_CHECK=1`、
`IMPECCABLE_NO_TELEMETRY=1`、`DO_NOT_TRACK=1`，且不为普通设计任务提供 `OPENAI_API_KEY`。

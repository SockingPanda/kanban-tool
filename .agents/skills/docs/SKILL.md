---
name: docs
description: 维护 kanban-tool 的文档事实源与同步边界；当产品行为、crate/依赖、状态机、数据模型、HTTP/CLI、schema、Desktop layout 或安装流程变化，或需要判断应读和应改哪些文档时使用；不负责 Rust 编码风格、长文措辞、底层检查命令或提交。
---

# 项目文档事实源

## 行为契约

能力：根据改动影响面选择最小充分的 canonical source，核对当前实现与承诺，并只同步会变得错误或缺失的文档。

触发：

- 改变产品范围、用户可见行为、入口、配置、错误、状态、claim/lease、数据表、schema、HTTP、CLI、Desktop shell 或 crate/依赖方向。
- 新增或修改 migration、机器契约、fixture、生成文档或长期架构决策。
- 用户问“读哪份文档”“改动后要同步哪里”或发现代码、测试、schema 与文档冲突。

不触发：

- 只改句子、段落结构或示例表达而不改变事实，使用 `$prose`。
- 只改 Rust 实现且公开行为、架构边界和文档仍正确，使用 `$style` 与 `$check`。
- 只选择执行命令，使用 `$check`；只写 commit message 或提交，使用 `$commit`。

成功标准：

- 先从真实入口、直接调用链、相关测试和实际 schema/migration 确认影响，再读取对应的单一主文档。
- 更新后的文档与当前实现、contract、测试和生成 artifact 一致；冲突被明确报告，不被静默选择。
- 只改必要源文件；生成 bundle 由源文件重新生成并检查，不能直接手改生成物。
- 最终报告列出同步文件、实际 recipe 和未运行的宽 gate，不把 roadmap 当现行事实。

事实源路由：

- 产品范围、目标、用户行为：`docs/SPEC.md`；安装、首次使用和公开示例：`README.md`。
- 进程、crate、依赖方向、service path、ownership：`docs/ARCHITECTURE.md`。
- status、transition、guard、claim、lease、heartbeat、recompute、dispatcher：`docs/STATE_MACHINE.md`。
- 表、字段、ID、约束、事件、query 和 canonical/derived 边界：`docs/DATA_MODEL.md` 及负责该结构的 migration。
- HTTP/SSE、错误 envelope 和本机 API：`docs/API_SPEC.md`；CLI 参数、输出、JSON 和退出码：`docs/CLI_SPEC.md`。
- machine-readable schema、operation catalog、fixture、adoption、dependency isolation：`docs/SCHEMA_CONTRACTS.md` 和对应 schema source/tool。
- Desktop shell/sidebar/header/viewport/layout smoke：`docs/DESKTOP_LAYOUT_SMOKE.md`；长期真实取舍：`docs/ADR.md` 的相关条目。

硬约束：

- 当前代码、实际 schema/migration 和可执行测试说明“现在做什么”；SPEC、contract 和 ADR 说明承诺或长期决定；二者冲突必须写出差异。
- `KANBAN_SPEC_BUNDLE.md` 是生成产物，不是独立权威。修改其源文档后使用 `just spec-bundle-generate`（仅在明确需要生成时）并用 `just spec-bundle-check` 检查。
- 不因“跨 crate”“文件很多”或“看起来复杂”通读全部文档；只有跨公开 surface、责任边界、canonical 数据、状态机、contract、migration、milestone 或 release 才扩大阅读。
- 不为内部等价重构、私有命名或不改变公开行为的 bug fix 扩写 README、SPEC 或 ADR。
- 不新增第二套规范、兼容叙事或过期 operation 清单；不存在的 recipe、字段和路径不得写进文档。

决策规则：

- 产品行为改变时先更新 `docs/SPEC.md`，若影响入门再同步 `README.md`；架构/依赖变化同步 `ARCHITECTURE`。
- 状态或事务边界变化同步 `STATE_MACHINE`，schema/migration 变化同步 `DATA_MODEL`；wire/CLI 变化同步对应公开 contract。
- 生成源变化时先定向测试，再按 `$check` 选择 `just schema-check`、`just schema-docs` 或 `just schema-contract`；不要为了“保险”跑无关 full gate。
- 需要新长期决策才写 ADR；实现日志、review finding、临时 workaround 留在任务或专门 runbook。

质量标准：

- 事实源唯一、阅读范围可解释、同步面最小、生成物可重建、冲突和未验证状态可审计。

受保护的自主空间：

- 可自行选择先读代码还是主文档、更新多个相关源的顺序、README 是否需要同步、是否需要 ADR，以及如何组织交叉链接。
- 可根据受众与风险选择表格、示例或简短说明，不被固定文档模板限制。

非目标：

- 不自行修复代码、schema、recipe 或生成器；只判定文档责任和同步计划。
- 不把 project-local 事实晋升为全局 skill，也不代替 `skill-governance` 做 skill library 决策。

## 验证案例

- 典型触发：新增 API response 字段，读取 handler/DTO/测试，更新 `docs/API_SPEC.md` 和必要 schema source，再运行相应 check。
- 边界：代码与 `docs/STATE_MACHINE.md` 冲突时分别记录 current implementation、document contract 和本次目标，停止静默取舍。
- 失败回归：拒绝只编辑 `KANBAN_SPEC_BUNDLE.md`；要求回到源文档并说明生成检查。
- 近似误触发：私有函数重命名且行为不变时不改 README、SPEC 或 ADR。
- 自由度：同一 API 事实可用表格或步骤文档表达，只要 source、范围和链接不漂移。

写作交给 `$prose`，命令和结果交给 `$check`；不要在本 skill 复制长 shell 命令。

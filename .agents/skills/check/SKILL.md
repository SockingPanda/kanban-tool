---
name: check
description: 为 kanban-tool 的 Rust、Web、Desktop、schema、文档和仓库规则改动选择并执行当前 justfile 中最小充分的验证；区分文档结构与 rustdoc 验证，核对 base、未跟踪文件、技能路由和真实证据。不负责功能实现、发布或 Git 提交。
---

# 项目验证

## 行为契约

把当前改动映射到实际存在的 recipe，保留命令、目标 revision 或 diff、退出状态和未覆盖范围。

先读根 `AGENTS.md`、受影响目录的约定、`git status --short`、staged/unstaged diff 和未跟踪文件。
用 `just --summary` 与对应 recipe 核对命令。`justfile` 是入口事实源；本 skill 只说明选取原则。

使用 affected planner 时显式给出本任务的比较基线。版本开发通常比较对应版本分支，派生子任务比较
直接父分支或任务记录中的起点 SHA。不要把 recipe 的默认 `main` 当成当前任务基线，也不要通过
分支名猜测父分支。生成物、依赖锁文件和集成入口必须进入影响判断。

## 验证选择

| 改动 | 最小起点 | 何时升级 |
| --- | --- | --- |
| Markdown 内容、导航或纯文档目录 | `just docs-structure-check`、`just diff-check` | 修改 rustdoc/include、Rust 示例或公开 Rust 契约时运行 `just docs-check` |
| `AGENTS.md`、repo skill | 上述检查与 `just agents-check` | 改动 guard 时补 `just check-p xtask`、`just test-p xtask`、`just clippy-p xtask` |
| Cargo owner、仓库工具或协作规则 | `just repo-check` | 工具行为变化补 xtask 对应测试；共享构建锁变化补 `just target-tools` |
| 单 Rust package | 对应 `check-p`、`test-p`、`clippy-p` | 跨 package 或公开契约变化再扩大测试 |
| Web UI 或 application | 对应 `web-typecheck`、`web-lint`、`web-test` | Host、查询订阅、artifact 或实际交互变化按验收补 build/真实 Host 证据 |
| Protobuf、wire 或生成契约 | 对应 owner 测试、`grpc-contracts-check` 或 `web-contracts-check` | schema/catalog 变化补 `schema-check` 和必要的 surface 测试 |
| Desktop 或打包布局 | 对应 Desktop recipe | 真正改变包内容时补 package/smoke；普通 UI 文案不机械打完整包 |

表中均为 `just` recipe 名。以本次 checkout 的 `justfile` 为准；尚未集成的新功能 recipe 必须先核实，
不得把方案中的命令直接报告为已运行。

## 执行与证据

会写 Cargo target 的 recipe 必须走现有 build lock。全机 worktree 共用已约定的 target root；不自建
任务级 target/cache，不 `cargo clean`，不通过多个 shell 并行写 target。CI 使用 runner 专属的同一
root；本地 root 的选择见 [协作指南](../../../docs/collaboration.md)。

先做可用的窄验证。工具缺失、依赖离线或编译失败时记录阻塞；不要降低 gate、改 fixture、删 snapshot
或扩大 allowlist 来得到绿色结果。失败归因需要证据，无法确认就写未确定。

每个报告区分 `passed`、`failed`、`blocked`、`not_run`。测试源码已添加、静态规则已检查、模拟器通过、
真实 Host 通过是不同证据。合并冲突改变内容后补测受影响路径，不复用旧 diff 的成功结论。

`git diff --check` 不覆盖未跟踪文件。新文件还需进入对应 owner 检查并人工审阅；没有提交授权时不得为了
让 diff 好看而擅自 stage。不得把目录存在、命令存在或测试名称存在当作行为正确。

## 停止条件

验收覆盖足够，剩余风险和未运行项已写清，最终 diff 已检查后停止。默认由执行 agent 完成实现与验证；
独立审查仅在用户明确要求时启动，不因日期、版本、风险或任务完成自动安排 reviewer。

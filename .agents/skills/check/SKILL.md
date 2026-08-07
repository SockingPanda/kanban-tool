---
name: check
description: 为 kanban-tool 的 Rust、Web、Desktop、schema、文档和仓库配置改动选择并执行当前 justfile 中最小充分的验证；报告真实命令、失败归因、未运行的宽 gate 和剩余风险。不负责写代码、写文档正文、发布或创建提交。
---

# 项目验证

## 行为契约

能力：把 diff 影响面映射到真实 recipe，并用可审计证据收口，不凭记忆发明命令。

硬边界：

- 先读 `just --summary`/recipe 和 `git diff`；`justfile` 是唯一命令事实源。
- 文件修改至少运行 `just diff-check`；owner 文档或 skill 改动补 `just docs-check`。
- 只有 protocol/schema contract 或 generated artifact 变化才运行 `just schema-check`；Rust、Web、Desktop 和 package gate 按实际影响升级。
- 会写 Cargo target 的 recipe 使用仓库 build lock；不自设 target/cache，不 `cargo clean`，不并行写 target。
- 检查失败先判断是否由当前 diff 引起；不得改 gate、fixture、snapshot 或 allowlist 掩盖失败。
- 未运行的 gate 不得表述为通过、完整兼容、migration closed 或 release ready。

## 最小映射

文档/skill → `docs-check`、`diff-check`；单 Rust package → `check-p`/`test-p`/`clippy-p`；跨 core/service
→ 对应 core gate；Desktop → `desktop-check`；protocol/schema → `schema-check`。生成和打包 recipe 需要
明确授权，完成后审阅 diff。

## 受保护自由度

可选择 test filter、nextest fallback、命令顺序和 acceptance path，只要覆盖目标行为并报告未覆盖范围。

## 验证案例

- 只改 Markdown 时不机械运行 schema/full gate。
- protocol DTO 或 schema artifact 改动时补 `schema-check`。
- recipe 不存在时回到 `just --summary`，不执行或编造旧名称。

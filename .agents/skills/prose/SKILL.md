---
name: prose
description: 为 kanban-tool 编写或重写简体中文 README、owner guide、ADR、错误、示例和 Markdown；要求描述当前事实、从最小使用路径进入细节、保留精确代码/API 字面量并使用可核对链接。不负责文档 placement、Rust 风格、验证命令或 Git 提交。
---

# 项目文案

## 行为契约

能力：把 owner 提供的当前事实写成清楚、可行动且可验证的简体中文文案。

硬边界：

- 项目说明以简体中文为主；命令、路径、crate、API/JSON 字段、枚举和库名保留精确 literal。
- 先给最小可用路径，再链接高级细节；已有 canonical 定义时只写影响和链接，不复制第二份规范。
- 只写当前状态。精确 endpoint/flag/schema、测试名、gate、baseline 和迁移进度由代码、生成 artifact、CI 或任务记录持有。
- 示例必须能由当前 help、代码、测试或 schema 核对；不确定时标记待核对，不用猜测填空。
- 错误说明发生了什么和用户能做什么，但脚本仍依赖稳定 machine code/typed contract，不依赖 human message。

## 受保护自由度

可按读者选择标题、表格、步骤、示例、篇幅和中文/英文比例；不要求固定模板、文风、标题数量或推理轨迹。

## 验证案例

- README 快速开始应使用当前 `kanban serve` 和 CLI help 可核对的命令。
- owner guide 可用段落或表格解释同一事实，只要链接可追溯且不维护 inventory。
- 近似误触发：只问“哪个文档是事实源”先用 `$docs`，不直接重写长文。

---
name: commit
description: 为 kanban-tool 起草或创建本地 Conventional Commit；只有当前用户明确授权 commit 时才执行写入，先检查 staged diff 和匹配验证，保持单一意图与中文/Unicode 字面量正确；不执行 push、PR、merge、rebase、release 或其他远程操作。
---

# 本地提交

## 行为契约

能力：在明确授权后把当前范围内、已验证的改动组织成可审阅的本地提交；未授权时只可起草 message 或给出 commit plan，不写 Git history。

触发：

- 用户明确说“commit/提交/创建本地 commit”，或明确要求起草 Conventional Commit message。
- 用户指定提交边界、type、scope 或希望检查 staged diff。

不触发：

- “完成”“收尾”“准备好”或普通实现请求没有 commit 授权，不触发 `git commit`。
- push、开 PR、merge、rebase、release、tag、远端同步属于本 skill 的 non-goal；仓库没有 PR skill。
- 只选择验证，使用 `$check`；只写正文，使用 `$prose`；只改 Rust 结构，使用 `$style`。

成功标准：

- 每个 commit 对应一个完整意图，行为变更与纯机械重排、迁移或生成物边界清楚。
- 提交前核对 `git status`、unstaged/staged diff、文件范围和 `$check` 证据；不混入用户既有 dirty work。
- message 使用 Conventional Commits，简洁说明 what/why；commit 只落在当前本地仓库，不产生远程副作用。

硬约束：

- 没有当前用户对 commit 的明确授权，绝不执行 `git commit`；README 的“本地 commit”说明不是本轮授权，不能从“完成/收尾”推断。
- 永不 push、开 PR、merge、rebase、release 或改写远端历史；不创建 PR skill 来绕过该边界。
- 只 stage 当前任务文件；保护既有 dirty work，不使用破坏性 `reset --hard` 或 `checkout` 清理冲突。
- 提交前必须有与改动影响面匹配的 `$check` 结果，或明确报告未运行/失败；不得把未运行的 gate 写成通过。
- 标题格式为 `<type>(<scope>): <subject>`，header 必须单行、subject 使用祈使现在时、首字母按仓库约定、无尾句号；scope 可省略但应对应稳定子系统。
- 允许简体中文和 Unicode 出现在 subject/body/footer，但保留 Conventional Commit 的 ASCII 分隔符、type、scope 和 parser 所需字面量；不得把 API、路径或字段改写成中文别名。
- 生成源和对应生成物只有在同一意图、明确需要且 diff 已审阅时才同提交；否则分别保留或停止询问。

决策规则：

- 只有 message 请求：不 stage、不 commit，输出一个或多个可选标题并说明 type/scope 依据。
- 明确 commit 请求：先检查状态与 diff，再调用 `$check`；通过后按单一意图 stage 并创建本地 commit，完成后 read back `git show --stat`。
- 改动跨多个独立意图时，优先拆 commit；若拆分会丢失原子性，保留一个 commit 并在 body 解释边界。
- 推荐 type：`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`build`、`ci`、`chore`、`revert`。推荐 scope：`core`、`service`、`protocol`、`client`、`server`、`cli`、`mcp`、`desktop`、`docs`、`tooling`、`deps`；不匹配时可省略或使用更准确的稳定子系统。
- body 仅在 what/why 不显然时添加；breaking change 使用 `!` 和标准 footer，不能隐瞒迁移影响。

质量标准：

- 意图单一、范围干净、标题可解析、why 足够、验证真实、历史可回溯、无远程副作用。

受保护的自主空间：

- 可自行判断 type/scope、是否需要 body/footer、如何拆分同一功能的 commit，以及 message 使用中文还是英文。
- 可选择 `git diff --cached`、`git show --stat` 或等价只读审阅方式，不被固定命令顺序限制。

非目标：

- 不负责代码、文档或 schema 实现，不代替 `$check` 运行 gate，不发布 package 或维护 release notes。
- 不替用户决定是否授权 commit；缺授权时停止在草拟、检查或报告层。

## 验证案例

- 典型触发：用户明确要求“只提交当前 AGENTS 和 skills 改动”，检查 staged diff 与 `just diff-check` 后创建一个本地 `docs(...)` 或 `chore(...)` commit，并 read back。
- 边界：用户只要求“给我 Conventional Commit message”，不写 index 或 history。
- 失败回归：用户说“完成后记得提交”但当前请求未明确授权时保持未提交并说明原因。
- 近似误触发：用户要求开 PR、push 或 merge 时拒绝远程操作，最多给出本地 commit 建议。
- 对抗：dirty worktree 有无关文件时只 stage 当前范围，不 reset 或覆盖它。
- 自由度：同一意图可用中文 subject 或英文 subject，只要格式、范围、验证和字面量契约相同。

提交后停止；不要自动 push、开 PR 或开始下一阶段。

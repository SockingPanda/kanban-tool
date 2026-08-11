# Plane 适配视觉参考

这组图片记录了用户已批准的 kanban-tool 前端方向，不代表已经实现的产品行为，也不是逐像素
实现规格。它们借鉴 Plane 对高密度工作信息的组织方法，但使用 kanban-tool 自己的领域语义、
Astryx 中性视觉和 strict CSP 边界。

## 最终参考图

1. `01-board-overview-v2.png`：P0 看板总览与收起态项目切换器。确定卡片密度、合并后的
   header、filter row、状态色和 hover/focus 动作。
2. `04-project-switcher-open.png`：P0 项目切换展开态。确定搜索、最近项目、全部项目、
   当前项目和键盘切换的层级。
3. `02-task-side-peek.png`：P0 任务快速详情。确定不离开看板的合法动作、属性层级、
   readiness、依赖和活动。
4. `03-list-command-palette.png`：P1 列表与命令入口概念。确定专家检索和快捷导航的方向；
   command palette 不是当前已实现能力。

`01-board-overview.png` 已由 v2 取代；`02-task-side-peek-draft.png` 是第一轮被放弃的详情构图。
二者仅保留作迭代对照，不属于最终参考图。

## 项目切换语义

- UI 使用“项目”作为现有 canonical board 的用户语言，不新增 `workspace`、组织或另一层项目实体。
- 项目切换器位于全局左侧壳顶端，因此 Board、List、Map、Runs、Events、Signals 和 Ontology
  都能看到并使用同一个入口。
- 切换时更新 canonical board selector 与 URL，释放旧 board live session，再为新 board
  建立隔离的 session；不同时保持多个 live board。
- 切换后清理不再有效的 task selection、draft、filter、claim token 和错误状态，不能把旧项目
  projection 带入新项目。
- 首版只消费现有 boards read，支持搜索与选择；不顺带暴露 `createBoard`、`archiveBoard`
  或任何团队/权限管理。

## 共同视觉语言

- cool off-white canvas、白色分层 surface、graphite 文字和 hairline border；
- cobalt 只用于主动作、选中与 focus，amber/red/green 只表达风险、失败与完成；
- 工作型 sans + monospace ID，紧凑但不牺牲可读性；
- 卡片负责扫描，详情负责解释；次要动作渐进披露；
- 项目切换属于全局壳，board 内视图切换属于当前项目上下文；
- 无 workspace、团队、RBAC、cycles、modules、epics、SaaS 或第二状态机。

## 非字面实现项

- 图中任务 ID、标题、时间和计数是 synthetic content。
- 图片生成可能产生轻微字形或 icon 偏差；实现时使用真实文案、现有 icon 语法和可访问控件。
- 所有 lifecycle action 最终必须来自现有 typed legal options，不能照图硬编码。
- `01-board-overview-v2.png` 是内层页面必须继承的 shell reference；早期详情和列表图中的旧
  header 不能按字面实现。
- P1 command palette、列显隐和 saved view 需要单独确认能力范围。

每张最终 PNG 都内嵌了生成 prompt；同目录的 `*.prompt.txt` 和 `*.png.json` 提供可读副本与
后续审批状态。

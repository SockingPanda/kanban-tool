# Plane-only Web Surface Brief

## Goal

将 kanban-tool Web/Desktop 重构为 Plane-faithful 的 agent-first 人类可观察控制面。用户能在多个
canonical board/project 间切换，通过 Projects、Project Overview 和 Tasks 的多种真实视图快速
理解当前状态并在必要时介入。

## Stable product truth

- AI agents 是主要执行者，人类主要观察并在必要时操作。
- `project` 只映射 canonical board；没有 workspace/team。
- `tasks.status` 是事实，Board/List/Table/Map 只是 projection。
- 所有 mutation 继续经过 typed application/service path。
- Projects collection 不挂默认 board session 或 SSE。
- Project Overview 第一版只展示真实 identity/description/archive 信息。
- Timeline/Gantt 在没有 canonical read model 前不得作为可用功能。

## Committed visual world

- Plane 是唯一产品 craft 参考；不混入 Linear、Notion、GitHub 等视觉语言。
- 固定 product rail + Projects context sidebar + compact project header + dense main surface。
- 默认深色、完整浅色，canvas/surface/layer 逐级建立层次。
- operational blue 只用于 action/focus/selected，status 使用独立语义色。
- 14px UI、紧凑 toolbar/table/card、8px 左右 corner、hairline 分隔、overlay 才使用 shadow。
- 无装饰插画、无 project cover、无虚假指标、无 emoji 图标。

## Comps

待生成并评审：

1. Projects collection：测试双层导航、项目切换与诚实的 identity-only project rows/cards。
2. Project Overview：测试信息稀疏时的层级与 diagnostics 发现性。
3. Tasks workspace：测试 Board/List/Table/Map view family、filter/display controls 与 side-peek。

批准后在此记录 approved comp path、可组合部分，以及不得字面化的生成细节。

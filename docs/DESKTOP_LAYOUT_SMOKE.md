# 桌面端布局滚动冒烟检查

Desktop 当前保留十个可导航视图：`board`、`list`、`map`、`events`、`runs`、`signals`、
`ontology`、`maintenance`、`health`、`settings`。所有视图和 task detail 都通过
`KanbanApi` 请求 `kanban serve`；本清单不把旧 external projection/helper 或直连数据库路径
当作布局依赖。board 列来自 host 的 `board columns` 展示映射，map/context 来自 typed
graph/context API；维护操作的 phase/degraded/restart 结果必须按 host response 展示。

本清单是可重复使用的桌面端验证指南，用于补充
`apps/desktop/src/app/layout-scroll-contract.test.ts`
中的 Vitest 自动布局契约。在调整滚动条淡出效果，或修改会影响看板、详情面板和侧边栏
溢出行为的外壳布局前，应当执行这些检查。

## 自动契约

- 看板的横向溢出只由看板滚动容器承载，该层隐藏纵向溢出。
- 看板列保留 `min-h-0`、`overflow-hidden`，正文使用纵向 `ScrollArea`。
- 任务详情面板保持固定与弹性布局，正文滚动区域由 `TaskDetail` 管理。
- 侧边栏宽度过渡期间应裁切内容，并在过渡完成前隐藏展开后的文字标签。

## 窄窗口人工冒烟检查

以 390—480 像素的窄浏览器宽度运行桌面端 Web 外壳，并连接本地 API；
准备足够多的任务，使至少一列发生溢出。

- 看板横向滚动：看板可以横向滚动，页面本身不会出现第二条横向滚动条，最后一列仍可访问。
- 列内纵向滚动：高列只在自身正文区域滚动；列标题保持可见，相邻列不会被撑到超过视口高度。
- 任务详情正文滚动：打开详情面板后，面板标题保持可见，正文可以滚动，右边缘保持在 `100vw - 32px` 以内。
- 侧边栏过渡与裁切：收起或展开侧边栏时，文字标签在过渡期间被裁切；图标按钮仍可使用；窄窗口下标签不会遮挡主标题栏。

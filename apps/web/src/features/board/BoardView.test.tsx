import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { BoardView } from "./BoardView"
import { BOARD_PAGE_SIZE, boardPageWindow } from "./board-pagination"
import { boardColumnsForAttention } from "./board-attention"
import { validateBoardViewModel } from "./types"
import { englishBoardMessages } from "./types"
import type { BoardTaskViewModel, BoardViewModel } from "./types"

const model: BoardViewModel = {
  board: { id: "b-1", slug: "roadmap", name: "产品路线图" },
  columns: [
    { id: "c-hidden", status: "done", title: "已完成", position: 3, hidden: true },
    { id: "c-running", status: "running", title: "进行中", position: 2, hidden: false },
    { id: "c-ready", status: "ready", title: "待执行", position: 1, hidden: false },
  ],
  tasksByStatus: {
    ready: [
      {
        id: "t-later",
        seq: 2,
        ref: "KB-2",
        title: "后面的任务",
        description: null,
        status: "ready",
        position: 20,
        dueAt: null,
        scheduledAt: null,
        lastHeartbeatAt: null,
        statusReason: null,
        labels: [],
        lockVersion: 4,
        priority: 1,
        assignee: null,
        readiness: {
          dependencyBlocked: false,
          unfinishedParentCount: 0,
          executionPlanState: "not_required",
          requiredStepCount: 0,
          completedRequiredStepCount: 0,
          optionalStepCount: 0,
        },
      },
      {
        id: "t-first",
        seq: 1,
        ref: "KB-1",
        title: "先显示的任务",
        description: null,
        status: "ready",
        position: 10,
        dueAt: 1767225600000,
        scheduledAt: 1767139200000,
        lastHeartbeatAt: 1767052800000,
        statusReason: "等待审批",
        labels: [
          { id: "l-ui", name: "界面", color: "#123456" },
          { id: "l-review", name: "需要复核", color: null },
        ],
        lockVersion: 5,
        priority: 3,
        assignee: "worker-1",
        readiness: {
          dependencyBlocked: true,
          unfinishedParentCount: 1,
          executionPlanState: "planned",
          requiredStepCount: 2,
          completedRequiredStepCount: 1,
          optionalStepCount: 1,
        },
      },
    ],
    running: [],
  },
}

function expectInvalid(candidate: unknown) {
  let validation: ReturnType<typeof validateBoardViewModel> | undefined
  expect(() => {
    validation = validateBoardViewModel(candidate as BoardViewModel)
  }).not.toThrow()
  expect(validation?.valid).toBe(false)
}

function runningTask(overrides: Partial<BoardTaskViewModel> = {}): BoardTaskViewModel {
  return {
    ...model.tasksByStatus.ready[0],
    id: "t-running",
    ref: "KB-RUNNING",
    title: "运行中的任务",
    status: "running",
    ...overrides,
  }
}

describe("BoardView", () => {
  test("将大列限制为有界初始窗口并暴露语义分页", () => {
    const tasks = Array.from({ length: 205 }, (_, index) => ({
      ...model.tasksByStatus.ready[0],
      id: `t-page-${index + 1}`,
      seq: index + 1,
      ref: `KB-PAGE-${index + 1}`,
      position: index + 1,
      title: `分页任务 ${index + 1}`,
    }))
    const largeModel: BoardViewModel = {
      ...model,
      tasksByStatus: { ...model.tasksByStatus, ready: tasks },
    }

    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model: largeModel }} />)

    expect((markup.match(/data-testid="board-task"/g) ?? []).length).toBe(BOARD_PAGE_SIZE)
    expect(markup).toContain('data-testid="board-task-total"')
    expect(markup).toContain('data-total="205"')
    expect(markup).toContain('data-testid="board-column-pagination"')
    expect(markup).toContain('data-page="1"')
    expect(markup).toContain('data-range-start="1"')
    expect(markup).toContain('data-range-end="100"')
    expect(markup).toContain("1–100 / 205")
    expect(markup).toContain('aria-label="待执行的上一页"')
    expect(markup).toContain('aria-label="待执行的下一页"')
  })

  test("分页窗口对 page 0、末页和数据缩小保持确定性", () => {
    expect(boardPageWindow(205, 0)).toEqual({ page: 1, totalPages: 3, start: 0, end: 100 })
    expect(boardPageWindow(205, 1)).toEqual({ page: 1, totalPages: 3, start: 0, end: 100 })
    expect(boardPageWindow(205, 99)).toEqual({ page: 3, totalPages: 3, start: 200, end: 205 })
    expect(boardPageWindow(101, 3)).toEqual({ page: 2, totalPages: 2, start: 100, end: 101 })
    expect(boardPageWindow(0, 4)).toEqual({ page: 1, totalPages: 1, start: 0, end: 0 })
  })

  test("只渲染服务端可见列，并按 position 排序任务", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} />)

    expect(markup).toContain('data-state="ready"')
    expect(markup).toContain("产品路线图")
    expect(markup).toContain('data-column-id="c-ready"')
    expect(markup).toContain('data-column-id="c-running"')
    expect(markup).not.toContain('data-column-id="c-hidden"')
    expect(markup.indexOf('data-column-id="c-ready"')).toBeLessThan(markup.indexOf('data-column-id="c-running"'))
    expect(markup.indexOf("先显示的任务")).toBeLessThan(markup.indexOf("后面的任务"))
    expect(markup).toContain("状态")
    expect(markup).toContain('data-testid="board-attention-lens"')
    expect(markup).toContain('data-testid="board-attention-ready"')
    expect(markup).toContain('data-testid="board-attention-count-ready"')
    expect(markup).toContain('data-testid="board-attention-count-running"')
    expect(markup).toContain("优先级 P3")
    expect(markup).toContain("依赖阻塞")
    expect(markup).toContain("必需步骤")
    expect(markup).toContain("1 / 2")
    expect(markup).toContain("截止时间")
    expect(markup).toContain("排期时间")
    expect(markup).toContain("最近心跳")
    expect(markup).toContain("状态原因")
    expect(markup).toContain("等待审批")
    expect(markup).toContain("界面")
    expect(markup).toContain("需要复核")
  })

  test("attention lens 只筛选卡片并保留跨列拖放目标", () => {
    const filtered = boardColumnsForAttention(model, "ready")

    expect(filtered.map(({ column }) => column.status)).toEqual(["ready", "running"])
    expect(filtered.find(({ column }) => column.status === "ready")?.tasks).toHaveLength(2)
    expect(filtered.find(({ column }) => column.status === "running")?.tasks).toHaveLength(0)
  })

  test("board header keeps slug visible and moves canonical id into a details reveal", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} />)

    expect(markup).toContain('data-testid="board-identity-slug"')
    expect(markup).toContain('data-testid="board-identity-details"')
    expect(markup).toMatch(/<details[^>]*data-testid="board-identity-details"[\s\S]*b-1[\s\S]*<\/details>/)
  })

  test("embedded presentation keeps an accessible heading without repeating board identity chrome", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} presentation="embedded" />)

    expect(markup).toContain('data-presentation="embedded"')
    expect(markup).toContain('data-board-id="b-1"')
    expect(markup).toContain('data-board-slug="roadmap"')
    expect(markup).toMatch(/<h[1-3][^>]*class="[^"]*sr-only[^"]*"[^>]*>产品路线图<\/h[1-3]>/)
    expect(markup).not.toContain('data-testid="board-identity-slug"')
    expect(markup).not.toContain('data-testid="board-identity-details"')
  })

  test("task card keeps agent summary facts and places low-frequency facts in a details reveal", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} />)

    expect(markup).toContain('data-testid="board-task-summary"')
    const secondaryTag = markup.match(/<details[^>]*data-testid="board-task-secondary"[^>]*>/)?.[0]
    expect(secondaryTag).toBeDefined()
    expect(secondaryTag).not.toContain(" open")
    expect(markup).toMatch(/<details[^>]*data-testid="board-task-secondary"[\s\S]*截止时间[\s\S]*标签[\s\S]*可选步骤[\s\S]*<\/details>/)
  })

  test("卡片在排期、截止、心跳、状态原因和标签为空时使用简洁占位", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} />)

    expect(markup).toMatch(/data-testid="board-task-scheduled">—<\/dd>/)
    expect(markup).toMatch(/data-testid="board-task-due">—<\/dd>/)
    expect(markup).toMatch(/data-testid="board-task-heartbeat">—<\/dd>/)
    expect(markup).toMatch(/data-testid="board-task-status-reason">—<\/dd>/)
    expect(markup).toMatch(/data-testid="board-task-labels">无标签<\/dd>/)
  })

  test("日期事实使用消息指定的 locale，并保留可审计 ISO 时间", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} messages={englishBoardMessages} />)

    expect(markup).toContain('dateTime="2026-01-01T00:00:00.000Z"')
    expect(markup).toContain('dateTime="2025-12-31T00:00:00.000Z"')
    expect(markup).toContain("Jan")
  })

  test("提供选择回调时将任务标题暴露为 Inspector opener", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} onSelectTask={vi.fn()} />)

    expect(markup).toContain("<button")
    expect(markup).toContain("先显示的任务")
  })

  test("遇到没有 server column 的非空 status 时 fail closed", () => {
    const modelWithOrphan = {
      ...model,
      tasksByStatus: {
        ...model.tasksByStatus,
        review: [model.tasksByStatus.ready[0]],
      },
    }

    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model: modelWithOrphan }} />)

    expect(markup).toContain('data-state="error"')
    expect(markup).toContain('data-anomaly="board-model"')
    expect(markup).toContain("服务端返回的看板数据暂时无法显示，请重试。")
    expect(markup).not.toContain("任务状态 review 没有对应的服务端列")
    expect(markup).not.toContain('data-status="review"')
  })

  test("invalid ready model 的空白 board name 使用非空 fallback heading", () => {
    const invalidModel: BoardViewModel = {
      ...model,
      board: { ...model.board, name: "  " },
    }

    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model: invalidModel }} />)

    expect(markup).toContain('data-state="error"')
    expect(markup).toMatch(/<h1[^>]*>看板<\/h1>/)
    expect(markup).not.toMatch(/<h1[^>]*><\/h1>/)
  })

  test("覆盖 loading、empty、error 和 offline 状态，并保留 retry seam", () => {
    const retry = vi.fn()
    const states = [
      { state: { kind: "loading" as const }, text: "正在加载看板…" },
      {
        state: { kind: "empty" as const, board: model.board },
        text: "看板暂无列",
      },
      { state: { kind: "error" as const, message: "columns request failed" }, text: "columns request failed" },
      { state: { kind: "offline" as const, message: "网络不可用" }, text: "网络不可用" },
    ]

    for (const { state, text } of states) {
      const markup = renderToStaticMarkup(<BoardView state={state} onRetry={retry} />)
      expect(markup).toContain(`data-state="${state.kind}"`)
      expect(markup).toContain(text)
      if (state.kind === "error" || state.kind === "offline") expect(markup).toContain("重试")
    }
    expect(retry).not.toHaveBeenCalled()
  })

  test("无看板空态不伪造 identity，并可完整切换英文文案", () => {
    const markup = renderToStaticMarkup(
      <BoardView
        state={{ kind: "empty", detail: englishBoardMessages.noBoardsDescription }}
        messages={englishBoardMessages}
      />,
    )

    expect(markup).toContain("No boards available")
    expect(markup).toContain("The server returned no available boards")
    expect(markup).not.toContain("看板")
  })

  test("ready board keeps rendering while sync status is stale or recovering", () => {
    const stale = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} syncStatus="stale" />)
    const recovering = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} syncStatus="recovering" />)

    expect(stale).toContain('data-testid="board-sync-banner"')
    expect(stale).toContain('data-sync-state="stale"')
    expect(stale).toContain("产品路线图")
    expect(recovering).toContain('data-sync-state="recovering"')
    expect(recovering).toContain("产品路线图")
  })

  test("可见列没有任务时显示真实空列状态", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} />)

    expect(markup).toContain('data-column-id="c-running"')
    expect(markup).toContain("此列暂无任务。")
    expect(markup).not.toMatch(/<li[^>]*role="status"/)
    expect(markup).toContain('role="status" aria-live="polite">此列暂无任务。</p>')
  })

  test("所有 server columns hidden 时显示与空看板不同的空态", () => {
    const hiddenModel: BoardViewModel = {
      ...model,
      columns: model.columns.map((column) => ({ ...column, hidden: true })),
    }

    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model: hiddenModel }} />)

    expect(markup).toContain("看板没有可见列")
    expect(markup).not.toContain("看板暂无列")
  })

  test("hidden column 的合法任务保持隐藏且不会触发 anomaly", () => {
    const hiddenTaskModel: BoardViewModel = {
      ...model,
      tasksByStatus: {
        ...model.tasksByStatus,
        done: [runningTask({ id: "t-done", ref: "KB-DONE", title: "隐藏完成任务", status: "done" })],
      },
    }

    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model: hiddenTaskModel }} />)

    expect(markup).toContain('data-state="ready"')
    expect(markup).not.toContain('data-anomaly="board-model"')
    expect(markup).not.toContain("隐藏完成任务")
  })

  test("文案可以由 props 注入而不改变 presentation model", () => {
    const markup = renderToStaticMarkup(
      <BoardView
        state={{ kind: "ready", model }}
        messages={{
          boardEyebrow: "ASTRYX BOARD",
          emptyColumn: "No tasks in this column.",
        }}
      />,
    )

    expect(markup).toContain("No tasks in this column.")
    expect(markup).toContain("产品路线图")
  })

  test("键盘可到达列 section、heading 和横向滚动 region", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} />)

    expect(markup).toMatch(/<div class="[^"]*astryx-stack[^"]*"[^>]*id="astryx-board-columns"/)
    expect(markup).toContain('role="region"')
    expect(markup).toContain('aria-label="看板列内容"')
    expect(markup).toContain('role="region" aria-label="看板列内容" tabindex="0"')
    expect(markup).toMatch(/tabindex="-1"[^>]*data-testid="board-column"/)
    expect((markup.match(/tabindex="-1"/g) ?? []).length).toBeGreaterThanOrEqual(3)
  })

  test("presentation 校验拒绝空白身份和异常 server columns", () => {
    const invalidModels: readonly BoardViewModel[] = [
      { ...model, board: { ...model.board, id: " " } },
      { ...model, board: { ...model.board, slug: "\t" } },
      { ...model, board: { ...model.board, name: "\n" } },
      { ...model, columns: model.columns.map((column) => ({ ...column, id: " " })) },
      { ...model, columns: model.columns.map((column) => ({ ...column, title: "\t" })) },
      { ...model, columns: [{ ...model.columns[0], id: model.columns[1].id }, ...model.columns.slice(1)] },
      { ...model, columns: [{ ...model.columns[0], status: model.columns[1].status }, ...model.columns.slice(1)] },
      { ...model, columns: [{ ...model.columns[0], position: model.columns[1].position }, ...model.columns.slice(1)] },
      { ...model, columns: model.columns.map((column) => ({ ...column, position: Number.MAX_SAFE_INTEGER + 1 })) },
    ]

    for (const candidate of invalidModels) expectInvalid(candidate)
  })

  test("presentation 校验跨所有 task groups 拒绝重复、空白和非法 position", () => {
    const invalidModels: readonly BoardViewModel[] = [
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          done: [{ ...runningTask(), status: "done", id: model.tasksByStatus.ready[0].id }],
        },
      },
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          running: [runningTask({ id: " " })],
        },
      },
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          running: [runningTask({ ref: "\t" })],
        },
      },
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          running: [runningTask({ title: "\n" })],
        },
      },
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          running: [runningTask({ position: Number.MAX_SAFE_INTEGER + 1 })],
        },
      },
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          running: [runningTask({ status: "ready" })],
        },
      },
    ]

    for (const candidate of invalidModels) expectInvalid(candidate)
  })

  test("presentation seam 拒绝缺失 board card facts 及 malformed labels", () => {
    const source = model.tasksByStatus.ready[0]
    const withoutDue = { ...source } as Record<string, unknown>
    delete withoutDue.dueAt
    const invalidModels: readonly BoardViewModel[] = [
      {
        ...model,
        tasksByStatus: { ...model.tasksByStatus, ready: [withoutDue as unknown as BoardTaskViewModel] },
      },
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          ready: [{ ...source, labels: [{ id: " ", name: "invalid", color: null }] }],
        },
      },
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          ready: [{ ...source, labels: [{ id: "duplicate", name: "one", color: null }, { id: "duplicate", name: "two", color: null }] }],
        },
      },
    ]

    for (const candidate of invalidModels) expectInvalid(candidate)
  })

  test("presentation 校验在遍历前拒绝非 record 列、任务和继承字段标签", () => {
    const source = model.tasksByStatus.ready[0]
    const inheritedIdLabel = Object.assign(Object.create({ id: "inherited-id" }) as Record<string, unknown>, {
      name: "标签",
      color: null,
    })
    const inheritedNameLabel = Object.assign(Object.create({ name: "inherited-name" }) as Record<string, unknown>, {
      id: "label-name",
      color: null,
    })
    const invalidModels: readonly unknown[] = [
      { ...model, columns: [null] },
      { ...model, columns: ["not-a-column"] },
      { ...model, tasksByStatus: { ...model.tasksByStatus, ready: [null] } },
      { ...model, tasksByStatus: { ...model.tasksByStatus, ready: [42] } },
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          ready: [{ ...source, labels: [inheritedIdLabel] }],
        },
      },
      {
        ...model,
        tasksByStatus: {
          ...model.tasksByStatus,
          ready: [{ ...source, labels: [inheritedNameLabel] }],
        },
      },
    ]

    for (const candidate of invalidModels) expectInvalid(candidate)
  })
})

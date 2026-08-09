import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { BoardView } from "./BoardView"
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
        ref: "KB-2",
        title: "后面的任务",
        status: "ready",
        position: 20,
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
        ref: "KB-1",
        title: "先显示的任务",
        status: "ready",
        position: 10,
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

function expectInvalid(candidate: BoardViewModel) {
  expect(validateBoardViewModel(candidate).valid).toBe(false)
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
    expect(markup).toContain("优先级 P3")
    expect(markup).toContain("依赖阻塞")
    expect(markup).toContain("必需步骤")
    expect(markup).toContain("1 / 2")
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
})

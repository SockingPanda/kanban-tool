import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import type { BoardTaskMutationClient, BoardTaskMutationSurface } from "./task-mutation-state"
import { BoardView } from "./BoardView"
import { englishBoardMessages, type BoardViewModel } from "./types"

const model: BoardViewModel = {
  board: { id: "b_default", slug: "default", name: "Default" },
  columns: [
    { id: "c_todo", status: "todo", title: "Todo", position: 1, hidden: false },
    { id: "c_ready", status: "ready", title: "Ready", position: 2, hidden: false },
    { id: "c_running", status: "running", title: "Running", position: 3, hidden: false },
  ],
  tasksByStatus: {
    todo: [{
      id: "t_1",
      seq: 1,
      ref: "default#1",
      title: "Draft",
      description: "Draft specification",
      status: "todo",
      position: 1,
      scheduledAt: null,
      dueAt: null,
      lastHeartbeatAt: null,
      statusReason: null,
      labels: [],
      lockVersion: 3,
      priority: 2,
      assignee: null,
      readiness: {
        dependencyBlocked: false,
        unfinishedParentCount: 0,
        executionPlanState: "not_required",
        requiredStepCount: 0,
        completedRequiredStepCount: 0,
        optionalStepCount: 0,
      },
    }],
    ready: [],
    running: [],
  },
}

function surface(): BoardTaskMutationSurface {
  const client: BoardTaskMutationClient = {
    createTask: vi.fn(),
    createStep: vi.fn(),
    updateTask: vi.fn(),
    transitionTask: vi.fn(),
  }
  return { client, onCanonicalReload: vi.fn() }
}

describe("Board task mutation surface", () => {
  test("renders create/edit affordances and legal transition actions", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} taskMutations={surface()} />)

    expect(markup).toContain("新建任务")
    expect(markup).toContain('data-testid="task-edit-t_1"')
    expect(markup).toContain('data-testid="task-transition-promote-t_1"')
    expect(markup).toContain('data-testid="task-transition-block-t_1"')
    expect(markup).not.toContain('data-testid="task-transition-claim-t_1"')
  })

  test("exposes pointer and keyboard drag semantics without inline style", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model }} taskMutations={surface()} />)

    expect(markup).toContain('draggable="true"')
    expect(markup).toContain('aria-grabbed="false"')
    expect(markup).toContain('aria-roledescription="可拖动任务卡片"')
    expect(markup).toContain('data-testid="board-drop-target-ready"')
    expect(markup).not.toContain("style=")
  })

  test("supports English mutation copy", () => {
    const markup = renderToStaticMarkup(
      <BoardView state={{ kind: "ready", model }} messages={englishBoardMessages} taskMutations={surface()} />,
    )

    expect(markup).toContain("Create task")
    expect(markup).toContain("Edit task")
    expect(markup).toContain("Grab task")
    expect(markup).toContain('aria-roledescription="Draggable task card"')
  })
})

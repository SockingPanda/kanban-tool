import { readFileSync } from "node:fs"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { createBoardTaskClaimTokenStore, type BoardTaskMutationClient, type BoardTaskMutationSurface } from "./task-mutation-state"
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

const runningModel: BoardViewModel = {
  ...model,
  tasksByStatus: {
    ...model.tasksByStatus,
    running: [{
      ...model.tasksByStatus.todo[0],
      id: "t_running",
      ref: "default#2",
      title: "Running task",
      status: "running",
    }],
    todo: [],
  },
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
    const source = readFileSync(new URL("./BoardTaskMutations.tsx", import.meta.url), "utf8")

    expect(markup).toContain('draggable="true"')
    expect(markup).toContain('aria-grabbed="false"')
    expect(markup).toContain('aria-roledescription="可拖动任务卡片"')
    expect(markup).toContain('data-testid="board-drop-target-ready"')
    expect(markup).not.toContain("style=")
    expect(markup).not.toContain("Spinner")
    expect(markup).not.toContain("isLoading")
    expect(source).not.toContain("isLoading")
    expect(source).not.toContain("<Spinner")
  })

  test("keeps confirmation dialogs cancel-first and retry notices safe while pending", () => {
    const source = readFileSync(new URL("./BoardTaskMutations.tsx", import.meta.url), "utf8")

    expect(source).toContain("initialFocusRef={isConfirmationDialog ? cancelRef : undefined}")
    expect(source).toContain("ref={cancelRef}")
    expect(source).toContain('data-autofocus={isConfirmationDialog ? "true" : undefined}')
    expect(source).toContain("isDisabled={controller.isMutationPending}")
    expect(source).toContain("controller.isMutationPending ? copy.mutationPending : retryLabel")
    expect(source).toContain("aria-busy={controller.isMutationPending || undefined}")
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

  test("renders a localized return-to-ready action with an accurate disabled reason", () => {
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model: runningModel }} taskMutations={surface()} />)

    expect(markup).toContain("释放回就绪")
    expect(markup).toContain("需要当前任务的本地认领令牌才能释放回就绪。")
    expect(markup).not.toMatch(/>release<|>Release<|>release task<|>Release task</i)
  })

  test("renders the return-to-ready action in English when a shared store has no token", () => {
    const store = createBoardTaskClaimTokenStore()
    const markup = renderToStaticMarkup(<BoardView state={{ kind: "ready", model: runningModel }} messages={englishBoardMessages} taskMutations={{ ...surface(), claimTokens: store }} />)

    expect(markup).toContain("Return to ready")
    expect(markup).toContain("A local claim token for this task is required to return it to ready.")
    expect(markup).not.toMatch(/>release<|>Release<|>release task<|>Release task</i)
  })
})

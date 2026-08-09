import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import {
  TaskInspectorRelationsPanel,
  type TaskInspectorRelationsPanelProps,
} from "./TaskInspectorRelationsPanel"
import { __test } from "./TaskInspectorRelationsPanel.logic"

const handlers = {
  saveTask: vi.fn(async () => undefined),
  transition: vi.fn(async () => undefined),
  addDependency: vi.fn(async () => undefined),
  removeDependency: vi.fn(async () => undefined),
  createStep: vi.fn(async () => undefined),
  linkStep: vi.fn(async () => undefined),
  markPlanNotRequired: vi.fn(async () => undefined),
  addLabel: vi.fn(async () => undefined),
  removeLabel: vi.fn(async () => undefined),
  applySuggestedLabel: vi.fn(async () => undefined),
  addComment: vi.fn(async () => undefined),
  uploadAttachment: vi.fn(async () => undefined),
  downloadAttachment: vi.fn(async () => null),
  deleteAttachment: vi.fn(async () => undefined),
}

function props(overrides: Partial<TaskInspectorRelationsPanelProps> = {}): TaskInspectorRelationsPanelProps {
  return {
    taskId: "t_current",
    comments: [
      {
        id: "c_1",
        author: "alice",
        kind: "decision",
        body: "**keep** this note",
        createdAt: 10,
        metadata: { selected: "keep", reason: "safer" },
      },
    ],
    dependencies: {
      parents: [{ id: "t_parent", ref: "default#1", title: "Parent", status: "done" }],
      children: [{ id: "t_child", ref: "default#3", title: "Child", status: "todo" }],
    },
    steps: {
      executionPlan: { state: "planned", reason: null },
      steps: [
        {
          id: "s_1",
          title: "Verify",
          body: "Read output",
          required: true,
          status: "todo",
          linkedTask: { id: "t_linked", ref: "default#2", title: "Linked", status: "ready" },
        },
      ],
    },
    handlers,
    snapshot: {
      scope: { identity: "runtime\u0000b_default\u0000t_current", boardId: "b_default", taskId: "t_current" },
      generation: 1,
      pending: new Set(),
      errors: new Map(),
      retries: new Map(),
    },
    onSelectTask: vi.fn(),
    ...overrides,
  }
}

describe("TaskInspectorRelationsPanel", () => {
  test("renders comments, dependency links, linked steps, and metadata", () => {
    const markup = renderToStaticMarkup(<TaskInspectorRelationsPanel {...props()} />)

    expect(markup).toContain('data-testid="task-inspector-relations"')
    expect(markup).toContain('data-testid="task-inspector-comments"')
    expect(markup).toContain('data-testid="task-inspector-dependencies"')
    expect(markup).toContain('data-testid="task-inspector-steps"')
    expect(markup).toContain("alice")
    expect(markup).toContain("**keep** this note")
    expect(markup).toContain('{&quot;selected&quot;:&quot;keep&quot;')
    expect(markup).toContain("Parent")
    expect(markup).toContain("Child")
    expect(markup).toContain("Linked")
    expect(markup).toContain("必需")
    expect(markup).toContain("已规划")
  })

  test("renders safe text for comment bodies and metadata", () => {
    const markup = renderToStaticMarkup(
      <TaskInspectorRelationsPanel
        {...props({
          comments: [{ ...props().comments[0]!, body: "<script>alert(1)</script>" }],
        })}
      />,
    )

    expect(markup).toContain("&lt;script&gt;alert(1)&lt;/script&gt;")
    expect(markup).not.toContain("<script>alert(1)</script>")
  })

  test("renders empty states without mutation controls pretending data exists", () => {
    const markup = renderToStaticMarkup(
      <TaskInspectorRelationsPanel
        {...props({
          comments: [],
          dependencies: { parents: [], children: [] },
          steps: { executionPlan: { state: "unplanned", reason: null }, steps: [] },
        })}
      />,
    )

    expect(markup).toContain("暂无评论。")
    expect(markup).toContain("暂无依赖。")
    expect(markup).toContain("暂无步骤。")
    expect(markup).toContain('data-testid="task-inspector-comment-author"')
    expect(markup).toContain('data-testid="task-inspector-step-title"')
  })

  test("shows pending action labels and cycle/scope errors from the 05C snapshot", () => {
    const snapshot = props().snapshot
    const markup = renderToStaticMarkup(
      <TaskInspectorRelationsPanel
        {...props({
          snapshot: {
            ...snapshot,
            pending: new Set(["addComment:t_current", "addDependency:t_current", "createStep:t_current"]),
            errors: new Map([
              ["addDependency:t_current", { operation: "addDependency", taskId: "t_current", kind: "conflict", message: "Dependency cycle detected", status: 409, code: "dependency_cycle", recoverable: true }],
              ["createStep:t_current", { operation: "createStep", taskId: "t_current", kind: "error", message: "Task is outside board scope", status: 422, code: "scope_mismatch", recoverable: true }],
            ]),
          },
        })}
      />,
    )

    expect(markup).toContain("正在添加评论…")
    expect(markup).toContain("正在添加依赖…")
    expect(markup).toContain("正在创建步骤…")
    expect(markup).toContain("Dependency cycle detected")
    expect(markup).toContain("dependency_cycle")
    expect(markup).toContain("Task is outside board scope")
    expect(markup).toContain("scope_mismatch")
    expect(markup).toContain('role="alert"')
  })

  test("keeps comments paginated locally and sort changes independent from URL state", () => {
    const comments = Array.from({ length: 12 }, (_, index) => ({
      id: `c_${index + 1}`,
      author: "alice",
      kind: "note" as const,
      body: `Comment ${index + 1}`,
      createdAt: index + 1,
      metadata: {},
    }))
    const markup = renderToStaticMarkup(<TaskInspectorRelationsPanel {...props({ comments })} />)

    expect(markup.indexOf("Comment 12")).toBeLessThan(markup.indexOf("Comment 3"))
    expect(markup).toContain("第 1 / 2 页")
    expect(markup).toContain('data-testid="task-inspector-comments-next"')
    expect(markup).toContain('data-testid="task-inspector-comments-sort"')
    expect(markup).not.toContain(">Comment 1<")
  })
})

describe("TaskInspectorRelationsPanel input seams", () => {
  test("builds exact typed comment input", () => {
    expect(__test.commentInput(" alice ", " decision ", " Keep this ")).toEqual({
      author: "alice",
      kind: "decision",
      body: "Keep this",
    })
  })

  test("builds exact typed step and plan inputs", () => {
    expect(__test.stepInput(" Verify ", " Body ", true)).toEqual({ title: "Verify", body: "Body", required: true })
    expect(__test.stepInput(" Link ", "", false, " default#2 ")).toEqual({ title: "Link", required: false, linked_task_ref: "default#2" })
    expect(__test.planInput(" manual execution ")).toEqual({ reason: "manual execution" })
  })
})

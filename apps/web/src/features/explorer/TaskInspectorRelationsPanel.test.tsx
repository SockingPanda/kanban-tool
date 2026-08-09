import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import {
  TaskInspectorRelationsPanel,
  type TaskInspectorRelationsPanelProps,
} from "./TaskInspectorRelationsPanel"
import { __test } from "./TaskInspectorRelationsPanel.logic"

const handlers = {
  saveTask: vi.fn(async () => ({ committed: true, reconciled: true })),
  transition: vi.fn(async () => ({ committed: true, reconciled: true })),
  addDependency: vi.fn(async () => ({ committed: true, reconciled: true })),
  removeDependency: vi.fn(async () => ({ committed: true, reconciled: true })),
  createStep: vi.fn(async () => ({ committed: true, reconciled: true })),
  linkStep: vi.fn(async () => ({ committed: true, reconciled: true })),
  markPlanNotRequired: vi.fn(async () => ({ committed: true, reconciled: true })),
  addLabel: vi.fn(async () => ({ committed: true, reconciled: true })),
  removeLabel: vi.fn(async () => ({ committed: true, reconciled: true })),
  applySuggestedLabel: vi.fn(async () => ({ committed: true, reconciled: true })),
  addComment: vi.fn(async () => ({ committed: true, reconciled: true })),
  uploadAttachment: vi.fn(async () => ({ committed: true, reconciled: true })),
  downloadAttachment: vi.fn(async () => null),
  deleteAttachment: vi.fn(async () => ({ committed: true, reconciled: true })),
  suggestLabels: vi.fn(async () => null),
  retry: vi.fn(async () => ({ committed: false, reconciled: true })),
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
    resolveTaskSelector: (selector) => selector === "default#1" ? "t_parent" : null,
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
    expect(markup).toContain('dateTime="1970-01-01T00:00:00.010Z"')
    expect(markup).not.toContain(">10<")
    expect(markup).toContain("评论排序")
    expect(markup).toContain(">备注</option>")
    expect(markup).toContain(">决策</option>")
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
              ["removeDependency:t_current", { operation: "removeDependency", taskId: "t_current", kind: "conflict", message: "Dependency removal rejected", status: 409, code: "dependency_remove_conflict", recoverable: true }],
              ["createStep:t_current", { operation: "createStep", taskId: "t_current", kind: "error", message: "Task is outside board scope", status: 422, code: "scope_mismatch", recoverable: true }],
              ["linkStep:t_current", { operation: "linkStep", taskId: "t_current", kind: "error", message: "Linked task is outside board scope", status: 422, code: "link_scope_mismatch", recoverable: true }],
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
    expect(markup).toContain("Dependency removal rejected")
    expect(markup).toContain("dependency_remove_conflict")
    expect(markup).toContain("Task is outside board scope")
    expect(markup).toContain("scope_mismatch")
    expect(markup).toContain("Linked task is outside board scope")
    expect(markup).toContain("link_scope_mismatch")
    expect(markup).toMatch(/data-testid="task-inspector-create-step"[^>]*disabled=""/)
    expect(markup).toMatch(/data-testid="task-inspector-link-step"[^>]*disabled=""/)
    expect(markup).toContain('role="alert"')
  })

  test("disables all relation writes while reload is pending", () => {
    const snapshot = props().snapshot
    const markup = renderToStaticMarkup(
      <TaskInspectorRelationsPanel
        {...props({ snapshot: { ...snapshot, pending: new Set(["reload:t_current"]) } })}
      />,
    )

    expect(markup).toMatch(/<button type="submit" disabled="">添加评论<\/button>/)
    expect(markup).toMatch(/<button type="submit" disabled="">添加父依赖<\/button>/)
    expect(markup).toMatch(/disabled=""[^>]*aria-label="移除父依赖：Parent"/)
    expect(markup).toMatch(/data-testid="task-inspector-create-step"[^>]*disabled=""/)
    expect(markup).toMatch(/data-testid="task-inspector-link-step"[^>]*disabled=""/)
    expect(markup).toMatch(/data-testid="task-inspector-mark-plan-not-required"[^>]*disabled=""/)
    expect(markup).toContain('role="status" aria-live="polite">正在重试…</p>')
  })

  test("keeps exact retry intents visible while preserving diverged drafts", () => {
    const snapshot = props().snapshot
    const addCommentKey = "addComment:t_current"
    const reloadKey = "reload:t_current"
    const markup = renderToStaticMarkup(
      <TaskInspectorRelationsPanel
        {...props({
          snapshot: {
            ...snapshot,
            errors: new Map([
              [addCommentKey, { operation: "addComment", taskId: "t_current", kind: "error", message: "comment failed", status: 503, code: "unavailable", recoverable: true }],
              [reloadKey, { operation: "reload", taskId: "t_current", kind: "stale", message: "stale", status: 503, code: "reload_failed", recoverable: true }],
            ]),
            retries: new Map([
              [addCommentKey, { operation: "addComment", taskId: "t_current", input: { body: "retry me" } }],
              [reloadKey, { operation: "reload", taskId: "t_current" }],
            ]),
          },
        })}
      />,
    )

    expect(markup).toContain('data-retry-key="addComment:t_current"')
    expect(markup).toContain('data-retry-key="reload:t_current"')
    expect(markup).toContain("comment failed")
    expect(markup).toContain("stale")
    expect(markup).not.toMatch(/type="submit" disabled=""[^>]*>添加评论<\/button>/)
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
    expect(__test.commentInput(" decision ", " Keep this ")).toEqual({
      kind: "decision",
      body: "Keep this",
    })
  })

  test("builds exact typed step and plan inputs", () => {
    expect(__test.stepInput(" Verify ", " Body ", true)).toEqual({ title: "Verify", body: "Body", required: true })
    expect(__test.stepInput(" Link ", "", false, " default#2 ")).toEqual({ title: "Link", required: false, linked_task_ref: "default#2" })
    expect(__test.stepSubmission(" Create ", " Body ", true, "default#2", "create")).toEqual({ operation: "createStep", input: { title: "Create", body: "Body", required: true } })
    expect(__test.stepSubmission(" Link ", "", false, "t_2", "link")).toEqual({ operation: "linkStep", input: { title: "Link", required: false, linked_task_ref: "t_2" } })
    expect(__test.stepSubmission(" ", "Body", true, "t_2", "create")).toBeNull()
    expect(__test.stepSubmission("Link", "Body", true, "", "link")).toBeNull()
    expect(__test.planInput(" manual execution ")).toEqual({ reason: "manual execution" })
  })

  test("sorts and pages comments through the same seam used by the controls", () => {
    const comments = [
      { id: "c_old", createdAt: 10 },
      { id: "c_new", createdAt: 20 },
      { id: "c_mid", createdAt: 15 },
    ]
    expect(__test.commentPageState(comments, 0, 2, "newest").comments.map((comment) => comment.id)).toEqual(["c_new", "c_mid"])
    expect(__test.commentPageState(comments, 1, 2, "newest").comments.map((comment) => comment.id)).toEqual(["c_old"])
    expect(__test.commentPageState(comments, 0, 2, "oldest").comments.map((comment) => comment.id)).toEqual(["c_old", "c_mid"])
  })

  test("formats comment timestamps as localized labels with valid machine values", () => {
    const rendered = __test.formatCommentDateTime(0, "zh")
    expect(rendered.iso).toBe("1970-01-01T00:00:00.000Z")
    expect(rendered.label).not.toBe("0")
    expect(__test.formatCommentDateTime(Number.NaN, "en")).toEqual({ label: "—", iso: "" })
  })

  test("clears drafts only after commit, including committed-but-unreconciled writes", () => {
    expect(__test.shouldClearDraft({ committed: false, reconciled: false })).toBe(false)
    expect(__test.shouldClearDraft({ committed: false, reconciled: true })).toBe(false)
    expect(__test.shouldClearDraft({ committed: true, reconciled: false })).toBe(true)
    expect(__test.shouldClearDraft({ committed: true, reconciled: true })).toBe(true)
    expect(__test.shouldClearDraft({ committed: true, reconciled: true }, false)).toBe(false)
    expect(__test.shouldClearRetryDraft({ committed: false, reconciled: false }, true)).toBe(false)
    expect(__test.shouldClearRetryDraft({ committed: true, reconciled: false }, true)).toBe(true)
    expect(__test.shouldClearRetryDraft({ committed: true, reconciled: false }, false)).toBe(false)
    expect(__test.shouldClearRetryDraft({ committed: true, reconciled: true }, false)).toBe(false)
  })

  test("matches retry intents only while the current draft and scope epoch remain equal", () => {
    const resolver = (selector: string) => selector === "default#2" ? "t_2" : null
    expect(__test.commentDraftMatchesRetry("note", "retry me", { body: "retry me", kind: "note" })).toBe(true)
    expect(__test.commentDraftMatchesRetry("note", "edited", { body: "retry me", kind: "note" })).toBe(false)
    expect(__test.dependencyDraftMatchesRetry("default#2", "t_2", resolver)).toBe(true)
    expect(__test.dependencyDraftMatchesRetry("default#3", "t_2", resolver)).toBe(false)
    expect(__test.stepDraftMatchesRetry("Link", "Body", true, "default#2", { title: "Link", body: "Body", required: true, linked_task_ref: "t_2" }, resolver)).toBe(true)
    expect(__test.stepDraftMatchesRetry("Changed", "Body", true, "default#2", { title: "Link", body: "Body", required: true, linked_task_ref: "t_2" }, resolver)).toBe(false)
    expect(__test.planDraftMatchesRetry("why", "why")).toBe(true)
    expect(__test.planDraftMatchesRetry("changed", "why")).toBe(false)
    expect(__test.scopeEpochMatches({ identity: "runtime\u0000t1", generation: 1, taskId: "t1" }, { identity: "runtime\u0000t1", generation: 2, taskId: "t1" })).toBe(false)
    expect(__test.scopeEpochMatches({ identity: "runtime\u0000t1", generation: 1, taskId: "t1" }, { identity: "runtime\u0000t1", generation: 1, taskId: "t2" })).toBe(false)
    expect(__test.scopeEpochMatches({ identity: "runtime\u0000t1", generation: 1, taskId: "t1" }, { identity: "runtime\u0000t1", generation: 1, taskId: "t1" })).toBe(true)
  })

  test("resolves only active-board ids/refs before a mutation, with no call for unresolved input", () => {
    const resolver = vi.fn((selector: string) => selector === "t_known" ? "t_known" : selector === "default#2" ? "t_2" : null)
    expect(__test.resolveTaskSelector(" t_unknown ", resolver)).toBeNull()
    expect(resolver).toHaveBeenCalledWith("t_unknown")
    expect(__test.resolveTaskSelector(" t_known ", resolver)).toBe("t_known")
    expect(__test.resolveTaskSelector(" default#2 ", resolver)).toBe("t_2")
    const addDependency = vi.fn()
    const unresolved = __test.resolveTaskSelector("t_cross_board", resolver)
    if (unresolved) addDependency(unresolved)
    expect(addDependency).not.toHaveBeenCalled()
  })
})

import { describe, expect, test } from "vitest"

import { classifyEvent, fullRefetchPlan, targetKey } from "./invalidation"
import type { ValidatedBusinessEvent } from "./contracts"
import { knownSseEventKinds } from "../api/generated/sse"

function event(overrides: Partial<ValidatedBusinessEvent> = {}): ValidatedBusinessEvent {
  return {
    id: 7,
    eventId: "e_7",
    boardId: "board-a",
    taskId: "task-a",
    runId: "run-a",
    kind: "task.updated",
    createdAt: 1_700_000_000,
    raw: {},
    scope: { taskId: "task-a" },
    canonicalFingerprint: "fingerprint-7",
    known: true,
    ...overrides,
  }
}

describe("literal event invalidation plans", () => {
  test("target keys retain every semantic dimension and observed mode", () => {
    const live = targetKey({ root: "task-detail", boardId: "board-a", taskId: "task-a" })
    const observed = targetKey({ root: "task-detail", boardId: "board-a", taskId: "task-a", observedOnly: true })
    expect(live).not.toBe(observed)
    expect(live).toContain('board="board-a"')
    expect(live).toContain('task="task-a"')
  })

  test("maps task.updated through the exact known literal", () => {
    const plan = classifyEvent(event({ kind: "task.updated" }))

    expect(plan.fullRefetch).toBe(false)
    expect(plan.timeline).toBe(true)
    expect(plan.targets.map(targetKey)).toEqual(
      expect.arrayContaining([
        expect.stringContaining('events(board="board-a")'),
        expect.stringContaining('task-events(task="task-a")'),
        expect.stringContaining('tasks(board="board-a")'),
        expect.stringContaining('stats(board="board-a")'),
        expect.stringContaining('search-status(board="board-a")'),
        expect.stringContaining('board-task-map(board="board-a")'),
        expect.stringContaining('task-detail(task="task-a")'),
        expect.stringContaining('task-label-suggestions(task="task-a")'),
      ]),
    )
    expect(plan.targets.find((target) => target.root === "maintenance-status")).toMatchObject({ observedOnly: true })
    expect(plan.targets.find((target) => target.root === "maintenance-status")).not.toHaveProperty("boardId")
    expect(plan.targets.map(targetKey)).toContain('task-neighborhood(board="board-a")|observed=true')
  })

  test("keeps board switcher and global maintenance outside active-board aliases", () => {
    const created = classifyEvent(event({ kind: "board.created", taskId: null, scope: { taskId: null } }))
    expect(created.targets.map(targetKey)).not.toContain("boards(board-a)")
    expect(created.targets.map(targetKey)).toContain('columns(board="board-a")|observed=false')
    expect(created.targets.map(targetKey)).not.toContain('maintenance-status(board="board-a")|observed=false')

    const archived = classifyEvent(event({ kind: "board.archived", taskId: null, scope: { taskId: null } }))
    expect(archived.targets).toContainEqual({ root: "boards" })
    expect(archived.targets.map(targetKey)).not.toContain('boards(board="board-a")|observed=false')
  })

  test("only uses run_id-bearing run targets", () => {
    const withoutRun = classifyEvent(event({ kind: "task.updated", runId: null }))
    expect(withoutRun.targets.map(targetKey)).not.toContain('task-runs(task="task-a")|observed=true')
    expect(withoutRun.targets.map(targetKey)).not.toContain('task-run-log(task="task-a")|observed=true')
    const withRun = classifyEvent(event({ kind: "task.updated", runId: "run-2" }))
    expect(withRun.targets.map(targetKey)).toContain('task-runs(task="task-a")|observed=true')
    expect(withRun.targets.map(targetKey)).toContain('task-run-log(run="run-2")|observed=true')
  })

  test("includes both dependency endpoints and linked step neighborhood", () => {
    const dependency = classifyEvent(
      event({
        kind: "dependency.added",
        taskId: "child",
        scope: { taskId: "child", parentTaskId: "parent" },
      }),
    )
    expect(dependency.targets.map(targetKey)).toEqual(
      expect.arrayContaining([
        expect.stringContaining('task-detail(task="child")'),
        expect.stringContaining('task-dependencies(task="child")'),
        expect.stringContaining('task-neighborhood(task="child")'),
        expect.stringContaining('task-detail(task="parent")'),
        expect.stringContaining('task-dependencies(task="parent")'),
        expect.stringContaining('task-neighborhood(task="parent")'),
      ]),
    )
    expect(dependency.targets.some((target) => target.root === "stats")).toBe(false)

    const step = classifyEvent(
      event({
        kind: "task.step.updated",
        scope: { taskId: "parent", linkedTaskId: "linked" },
      }),
    )
    expect(step.targets.map(targetKey)).toContain('task-neighborhood(task="linked")|observed=false')
  })

  test("does not upgrade future prefix lookalikes", () => {
    const plan = classifyEvent(event({ kind: "task.step.future", known: false }))
    expect(plan.fullRefetch).toBe(true)
    expect(plan.kind).toBe("unknown")
  })

  test("returns the conservative board plan for unknown events", () => {
    const plan = classifyEvent(event({ kind: "task.attachment.created", known: false }))
    expect(plan).toEqual(fullRefetchPlan("task.attachment.created", "unknown", "board-a"))
    expect(plan.targets).toContainEqual({ root: "signal", boardId: "board-a", observedOnly: true })
  })

  test("covers every generated known literal without prefix matching", () => {
    for (const kind of knownSseEventKinds) {
      const plan = classifyEvent(event({ kind }))
      expect(plan.kind, kind).toBe("known")
      expect(plan.fullRefetch, kind).toBe(false)
    }
  })
})

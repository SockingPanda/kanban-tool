import { describe, expect, test } from "vitest"

import type { BoardViewModel } from "./types"
import {
  isMutationConflict,
  moveTaskOptimistically,
  transitionForTarget,
  transitionOptionsForStatus,
  updateTaskOptimistically,
} from "./task-mutation-state"
import { keyboardTransitionForDirection, updatePendingMutation } from "./task-mutation-controller"

const model: BoardViewModel = {
  board: { id: "b_default", slug: "default", name: "Default" },
  columns: [
    { id: "c_todo", status: "todo", title: "Todo", position: 1, hidden: false },
    { id: "c_ready", status: "ready", title: "Ready", position: 2, hidden: false },
  ],
  tasksByStatus: {
    todo: [{
      id: "t_1",
      ref: "default#1",
      title: "Draft",
      status: "todo",
      position: 1,
      priority: 2,
      assignee: null,
      lockVersion: 4,
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
  },
}

describe("board task mutation state", () => {
  test("exposes only state-machine transitions for a target column", () => {
    expect(transitionForTarget("todo", "ready")).toMatchObject({ action: "promote", targetStatus: "ready" })
    expect(transitionForTarget("todo", "running")).toBeNull()
    expect(transitionOptionsForStatus("archived")).toEqual([])
    expect(transitionOptionsForStatus("running").map((option) => option.targetStatus)).toEqual(["review", "done", "blocked"])
  })

  test("moves a task optimistically without mutating the canonical snapshot", () => {
    const next = moveTaskOptimistically(model, "t_1", "ready")

    expect(next.tasksByStatus.todo).toEqual([])
    expect(next.tasksByStatus.ready?.[0]).toMatchObject({ id: "t_1", status: "ready", position: 0 })
    expect(model.tasksByStatus.todo?.[0]).toMatchObject({ id: "t_1", status: "todo", position: 1 })
  })

  test("does not optimistically move through an illegal status edge", () => {
    expect(moveTaskOptimistically(model, "t_1", "running")).toBe(model)
  })

  test("updates a title optimistically while preserving lock version for the mutation", () => {
    const next = updateTaskOptimistically(model, "t_1", "Renamed")

    expect(next.tasksByStatus.todo?.[0]).toMatchObject({ id: "t_1", title: "Renamed", lockVersion: 4 })
    expect(model.tasksByStatus.todo?.[0]?.title).toBe("Draft")
  })

  test("classifies typed conflict responses for canonical reload and retry", () => {
    expect(isMutationConflict({ status: 409 })).toBe(true)
    expect(isMutationConflict({ apiError: { code: "conflict" } })).toBe(true)
    expect(isMutationConflict({ status: 422 })).toBe(false)
  })

  test("deduplicates a pending mutation key before the component schedules work", () => {
    const first = updatePendingMutation(new Set(), "edit:t_1", true)
    const duplicate = updatePendingMutation(first.next, "edit:t_1", true)

    expect(first.accepted).toBe(true)
    expect(duplicate.accepted).toBe(false)
    expect(duplicate.next).toBe(first.next)
  })

  test("keyboard direction resolves only a legal adjacent status edge", () => {
    expect(keyboardTransitionForDirection(model.columns, "todo", "next")).toMatchObject({ action: "promote", targetStatus: "ready" })
    expect(keyboardTransitionForDirection(model.columns, "todo", "previous")).toBeNull()
  })
})

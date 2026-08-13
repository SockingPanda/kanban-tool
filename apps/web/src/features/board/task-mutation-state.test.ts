import { describe, expect, test } from "vitest"

import type { BoardViewModel } from "./types"
import {
  isMutationConflict,
  isClaimTokenConflict,
  moveTaskOptimistically,
  rollbackTaskOptimistically,
  transitionCommandForTask,
  transitionForTarget,
  transitionForTaskTarget,
  transitionOptionsForTask,
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
    expect(transitionOptionsForStatus("running").map((option) => option.targetStatus)).toEqual(["running", "ready", "review", "done", "blocked", "archived"])
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
    expect(isMutationConflict({ status: 403, apiError: { code: "claim_token_mismatch" } })).toBe(true)
    expect(isClaimTokenConflict({ status: 403, apiError: { code: "claim_token_mismatch" } })).toBe(true)
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

  test("builds exact typed lifecycle payloads and force/token policy", () => {
    const task = model.tasksByStatus.todo?.[0]
    if (!task) throw new Error("fixture task missing")
    const promote = transitionCommandForTask(task, transitionForTaskTarget(task, "ready")!)
    expect(promote).toEqual({ action: "promote", input: {} })
    const claim = transitionCommandForTask({ ...task, status: "ready" }, transitionForTaskTarget({ ...task, status: "ready" }, "running")!)
    expect(claim).toEqual({ action: "claim", input: { ttl_ms: 300_000, worker_profile: "manual" } })
    const specify = transitionCommandForTask({ ...task, status: "triage" }, transitionForTaskTarget({ ...task, status: "triage" }, "todo")!, { description: "  details " })
    expect(specify).toEqual({ action: "specify", input: { description: "details" } })
    const running = { ...task, status: "running" as const }
    const forced = transitionCommandForTask(running, transitionForTaskTarget(running, "done")!, { confirmed: true })
    expect(forced).toEqual({ action: "complete", input: { force: true } })
    const blocked = transitionCommandForTask(running, transitionForTaskTarget(running, "blocked")!, { reason: "  waiting ", confirmed: true })
    expect(blocked).toEqual({ action: "block", input: { force: true, reason: "waiting" } })
    const tokenized = transitionCommandForTask(running, transitionForTaskTarget(running, "done", "claim-token")!, { claimToken: "claim-token" })
    expect(tokenized).toEqual({ action: "complete", input: { claim_token: "claim-token" } })
    const heartbeat = transitionCommandForTask(running, transitionForTaskTarget(running, "running", "claim-token")!, { claimToken: "claim-token" })
    expect(heartbeat).toEqual({ action: "heartbeat", input: { claim_token: "claim-token", ttl_ms: 300_000 } })
    expect(transitionForTaskTarget(running, "ready")).toBeNull()
    const releaseOption = transitionForTaskTarget(running, "ready", "claim-token")
    if (!releaseOption) throw new Error("running release fixture missing")
    expect(transitionCommandForTask(running, releaseOption, { claimToken: "claim-token" })).toEqual({ action: "release", input: { claim_token: "claim-token" } })
    expect(transitionCommandForTask(running, { ...releaseOption, targetStatus: "done" }, { claimToken: "claim-token" })).toBeNull()
    const archive = transitionCommandForTask(task, transitionForTaskTarget(task, "archived")!, { confirmed: true })
    expect(archive).toEqual({ action: "archive", input: { force: true } })
    const runningArchive = transitionForTaskTarget(running, "archived", "claim-token")
    if (!runningArchive) throw new Error("running archive fixture missing")
    expect(transitionCommandForTask(running, runningArchive, { claimToken: "claim-token", confirmed: true })).toEqual({ action: "archive", input: { force: true } })
  })

  test("hides promote and completion actions when canonical readiness facts fail", () => {
    const task = model.tasksByStatus.todo?.[0]
    if (!task) throw new Error("todo fixture missing")
    expect(transitionOptionsForStatus("todo")).toContainEqual(expect.objectContaining({ action: "promote" }))
    expect(transitionOptionsForTask({ ...task, description: null })).not.toContainEqual(expect.objectContaining({ action: "promote" }))
    expect(transitionOptionsForTask({ ...task, readiness: { ...task.readiness, dependencyBlocked: true } })).not.toContainEqual(expect.objectContaining({ action: "promote" }))
    expect(transitionOptionsForTask({ ...task, readiness: { ...task.readiness, executionPlanState: "unplanned" } })).not.toContainEqual(expect.objectContaining({ action: "promote" }))
    const running = { ...task, status: "running" as const, readiness: { ...task.readiness, requiredStepCount: 2, completedRequiredStepCount: 1 } }
    expect(transitionOptionsForTask(running, "claim-token")).not.toContainEqual(expect.objectContaining({ action: "complete" }))
    expect(transitionOptionsForTask(running, "claim-token")).not.toContainEqual(expect.objectContaining({ action: "release" }))
    const review = { ...running, status: "review" as const }
    expect(transitionOptionsForTask(review, "claim-token")).not.toContainEqual(expect.objectContaining({ action: "complete" }))
    expect(transitionOptionsForTask(running)).toContainEqual(expect.objectContaining({ action: "archive", requiresConfirmation: true }))
    const runningArchiveWithoutToken = transitionForTaskTarget(running, "archived")
    if (!runningArchiveWithoutToken) throw new Error("running archive without token fixture missing")
    expect(transitionCommandForTask(running, runningArchiveWithoutToken, { confirmed: true })).toEqual({ action: "archive", input: { force: true } })
  })

  test("does not pretend unblock has a canonical target and rolls back only one task", () => {
    const firstTask = model.tasksByStatus.todo?.[0]
    if (!firstTask) throw new Error("todo fixture missing")
    const blockedModel: BoardViewModel = {
      ...model,
      tasksByStatus: {
        blocked: [{ ...firstTask, status: "blocked" }],
        todo: [],
        ready: [],
      },
    }
    const blockedTask = blockedModel.tasksByStatus.blocked?.[0]
    if (!blockedTask) throw new Error("blocked fixture missing")
    expect(transitionForTaskTarget(blockedTask, "todo")).toMatchObject({ action: "unblock", targetStatus: "todo" })
    expect(transitionForTaskTarget(blockedTask, "scheduled")).toBeNull()
    expect(transitionForTaskTarget(blockedTask, "ready")).toBeNull()
    expect(transitionForTaskTarget(blockedTask, "running")).toBeNull()
    expect(moveTaskOptimistically(blockedModel, blockedTask.id, "ready")).toBe(blockedModel)
    const second = { ...firstTask, id: "t_2", title: "Second" }
    const concurrentBase = { ...model, tasksByStatus: { ...model.tasksByStatus, todo: [firstTask, second] } }
    const concurrent = updateTaskOptimistically(moveTaskOptimistically(concurrentBase, "t_1", "ready"), "t_2", "new")
    const rolledBack = rollbackTaskOptimistically(concurrent, concurrentBase, "t_1")
    expect(rolledBack.tasksByStatus.todo?.map((task) => task.id)).toEqual(["t_1", "t_2"])
    expect(rolledBack.tasksByStatus.todo?.[1]?.title).toBe("new")
  })
})

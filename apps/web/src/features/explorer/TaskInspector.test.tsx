import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import {
  TaskInspector,
  type TaskInspectorViewModel,
} from "./TaskInspector"
import {
  buildInspectorTransitionCommand,
  buildInspectorSaveTaskInput,
  inspectorActionIds,
  inspectorActionLabels,
  type InspectorEditDraft,
} from "./TaskInspector.edit-actions"
import { createInspectorAsyncFence } from "./TaskInspector.lazy"
import type { TaskInspectorMutationHandlers } from "./task-inspector-mutation-state"

const model: TaskInspectorViewModel = {
  task: {
    id: "t_fixture",
    ref: "default#1",
    title: "Inspect the task",
    status: "running",
    lockVersion: 7,
    scheduledAt: null,
    dueAt: null,
    priority: 2,
    description: "A task description.",
    statusReason: "handoff",
    assignee: "worker-a",
    executionPlanState: "planned",
    dependencyBlocked: true,
    unfinishedParentCount: 1,
    requiredStepCount: 2,
    completedRequiredStepCount: 1,
    optionalStepCount: 1,
    metadata: { source: "fixture" },
    claimOwner: "runner",
    claimExpiresAt: 20,
    lastHeartbeatAt: 19,
    currentRunId: "r_active",
    retryCount: 1,
    maxRetries: 3,
    createdAt: 1,
    updatedAt: 2,
  },
  steps: [{ id: "s_1", title: "Verify output", status: "todo", required: true, body: "Read the result." }],
  parents: [{ id: "t_parent", ref: "default#0", title: "Parent task", status: "done" }],
  children: [{ id: "t_child", ref: "default#2", title: "Child task", status: "todo" }],
  comments: [{ id: "c_1", author: "alice", kind: "note", body: "A note", createdAt: 1 }],
  runs: [{ id: "r_active", status: "running", workerProfile: "manual", claimOwner: "runner", startedAt: 2, finishedAt: null, exitCode: null, error: null, hasLog: false }],
  events: [{ id: 42, kind: "task.claimed", actor: "runner", createdAt: 2 }],
  runtime: { actor: "web", apiBaseUrl: "/", serverVersion: "3.0.0", protocolVersion: "v1", webBuildId: "test" },
}

describe("TaskInspector", () => {
  test("renders the editor trigger and the exact nine legal transition labels", () => {
    const handlers = {} as TaskInspectorMutationHandlers
    const markup = renderToStaticMarkup(<TaskInspector model={model} onSelectTask={vi.fn()} locale="en" mutationHandlers={handlers} claimToken="claim_1" />)

    expect(markup).toContain("Edit task")
    expect(markup).toContain('data-testid="inspector-actions"')
    expect(inspectorActionIds).toEqual(["specify", "promote", "claim", "heartbeat", "complete", "submit-review", "block", "unblock", "archive"])
    for (const label of [inspectorActionLabels.en[3], inspectorActionLabels.en[4], inspectorActionLabels.en[5], inspectorActionLabels.en[6], inspectorActionLabels.en[8]]) {
      expect(markup).toContain(label)
    }
    expect(markup).not.toContain("Release")
    expect(markup).not.toContain("Reopen")
  })

  test("builds a typed save input with the current lock version", () => {
    const draft: InspectorEditDraft = {
      title: " Renamed ",
      description: " Details ",
      assignee: " worker-b ",
      priority: 1,
      scheduledAt: "2026-08-09T09:10",
      dueAt: "",
    }

    expect(buildInspectorSaveTaskInput(model.task, draft)).toEqual({
      title: "Renamed",
      description: "Details",
      assignee: "worker-b",
      priority: 1,
      scheduled_at: Date.parse("2026-08-09T09:10"),
      due_at: null,
      expected_lock_version: 7,
    })
  })

  test("uses shared transition policy for claim, review, force, and reason inputs", () => {
    expect(buildInspectorTransitionCommand(model.task, "submit-review", {}, "claim_1")).toEqual({
      action: "submit-review",
      input: { claim_token: "claim_1" },
    })
    expect(buildInspectorTransitionCommand(model.task, "block", { reason: " needs changes ", confirmed: true }, null)).toEqual({
      action: "block",
      input: { force: true, reason: "needs changes" },
    })
    expect(buildInspectorTransitionCommand(model.task, "heartbeat", {}, null)).toBeNull()
  })

  test("renders every read-only inspector section and claim/runtime facts", () => {
    const markup = renderToStaticMarkup(<TaskInspector model={model} onSelectTask={vi.fn()} />)

    expect(markup).toContain('data-testid="task-inspector"')
    for (const id of ["inspector-metadata", "inspector-claim", "inspector-steps", "inspector-dependencies", "inspector-comments", "inspector-runs", "inspector-events", "inspector-runtime"]) {
      expect(markup).toContain(`data-testid="${id}"`)
    }
    expect(markup).toContain("Inspect the task")
    expect(markup).toContain("runner")
    expect(markup).toContain("Verify output")
    expect(markup).toContain("Parent task")
    expect(markup).toContain("A note")
    expect(markup).toContain("task.claimed")
    expect(markup).toContain("3.0.0")
    expect(markup).not.toContain("Add comment")
    expect(markup).not.toContain("Create step")
  })

  test("does not invoke lazy detail loaders until a disclosure is opened", () => {
    const onLoadRuns = vi.fn(async () => model.runs)
    const onLoadEvents = vi.fn(async () => model.events)
    const onLoadNeighborhood = vi.fn(async () => ({ centerTaskId: model.task.id, nodes: [], edges: [] }))
    renderToStaticMarkup(
      <TaskInspector
        model={{ ...model, runs: [], events: [], neighborhood: undefined }}
        onSelectTask={vi.fn()}
        onLoadRuns={onLoadRuns}
        onLoadEvents={onLoadEvents}
        onLoadNeighborhood={onLoadNeighborhood}
      />,
    )

    expect(onLoadRuns).not.toHaveBeenCalled()
    expect(onLoadEvents).not.toHaveBeenCalled()
    expect(onLoadNeighborhood).not.toHaveBeenCalled()
  })

  test("keeps the Chinese inspector copy localized", () => {
    const markup = renderToStaticMarkup(<TaskInspector model={model} onSelectTask={vi.fn()} locale="zh" />)

    expect(markup).toContain("任务检查器")
    expect(markup).toContain("认领者")
    expect(markup).not.toContain("TASK INSPECTOR")
    expect(markup).not.toContain("Claim owner")
    expect(markup).not.toContain("Last heartbeat")
  })

  test("retains lazy values and exposes an offline notice", () => {
    const markup = renderToStaticMarkup(
      <TaskInspector
        model={model}
        onSelectTask={vi.fn()}
        online={false}
        refreshOffline
        refreshError="offline"
      />,
    )

    expect(markup).toContain("当前离线，保留最近一次任务数据。")
    expect(markup).toContain("r_active")
  })

  test("aborts and rejects a late lazy result after task/session identity changes", async () => {
    const fence = createInspectorAsyncFence()
    let resolveOld!: (value: string) => void
    const oldResult = new Promise<string>((resolve) => { resolveOld = resolve })
    const oldSignal = fence.begin("session-a|t_1")
    let accepted: string | null = null
    void oldResult.then((value) => {
      if (fence.isCurrent("session-a|t_1", oldSignal)) accepted = value
    })

    const newSignal = fence.begin("session-a|t_2")
    expect(oldSignal.aborted).toBe(true)
    resolveOld("stale")
    await oldResult

    expect(accepted).toBeNull()
    expect(fence.isCurrent("session-a|t_1", oldSignal)).toBe(false)
    expect(fence.isCurrent("session-a|t_2", newSignal)).toBe(true)
    fence.abort()
    expect(newSignal.aborted).toBe(true)
  })
})

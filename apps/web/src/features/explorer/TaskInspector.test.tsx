import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import {
  TaskInspector,
  type TaskInspectorViewModel,
} from "./TaskInspector"
import {
  buildInspectorTransitionCommand,
  buildInspectorSaveTaskInput,
  inspectorEditDraft,
  inspectorMutationCommitted,
  inspectorRetryIntentMatches,
  inspectorRetryUserIntentMatches,
  inspectorActionIds,
  inspectorActionLabels,
  type InspectorEditDraft,
} from "./TaskInspector.edit-actions"
import { createInspectorAsyncFence } from "./TaskInspector.lazy"
import { inspectorMutationKey, type TaskInspectorMutationHandlers, type TaskInspectorMutationSnapshot } from "./task-inspector-mutation-state"

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
    expect(markup).toContain("Required steps are incomplete")
    expect(markup).toContain("aria-describedby=\"inspector-action-reason-complete\"")
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

  test("rebuilds the editor draft from a same-id canonical refresh", () => {
    const refreshedTask = { ...model.task, title: "Canonical title", lockVersion: 8 }

    expect(inspectorEditDraft(refreshedTask)).toMatchObject({ title: "Canonical title" })
    expect(buildInspectorSaveTaskInput(refreshedTask, inspectorEditDraft(refreshedTask)).expected_lock_version).toBe(8)
  })

  test("uses shared transition policy for claim, review, force, and reason inputs", () => {
    expect(buildInspectorTransitionCommand(model.task, "submit-review", {}, "claim_1")).toEqual({
      action: "submit-review",
      input: { claim_token: "claim_1" },
    })
    expect(buildInspectorTransitionCommand(model.task, "block", { reason: " needs changes " }, null)).toBeNull()
    expect(buildInspectorTransitionCommand(model.task, "block", { reason: " needs changes ", confirmed: true }, null)).toEqual({
      action: "block",
      input: { force: true, reason: "needs changes" },
    })
    expect(buildInspectorTransitionCommand(model.task, "heartbeat", {}, null)).toBeNull()
  })

  test("only committed outcomes close mutation surfaces and retry keys stay exact", () => {
    expect(inspectorMutationCommitted({ committed: false, reconciled: false })).toBe(false)
    expect(inspectorMutationCommitted({ committed: false, reconciled: true })).toBe(false)
    expect(inspectorMutationCommitted({ committed: true, reconciled: false })).toBe(true)
    expect(inspectorMutationCommitted({ committed: true, reconciled: true })).toBe(true)
    expect(inspectorMutationKey("saveTask", model.task.id)).toBe("saveTask:t_fixture")
    expect(inspectorMutationKey("transition", model.task.id)).toBe("transition:t_fixture")
    expect(inspectorMutationKey("reload", model.task.id)).toBe("reload:t_fixture")
  })

  test("compares the current editor/action intent with a retained retry", () => {
    const saveInput = buildInspectorSaveTaskInput(model.task, inspectorEditDraft(model.task))
    const saveIntent = { operation: "saveTask" as const, taskId: model.task.id, input: saveInput }
    expect(inspectorRetryIntentMatches(saveIntent, "saveTask", saveInput)).toBe(true)
    expect(inspectorRetryIntentMatches(saveIntent, "saveTask", { ...saveInput, title: "A different title" })).toBe(false)
    expect(inspectorRetryIntentMatches(saveIntent, "saveTask", { ...saveInput, expected_lock_version: 8 })).toBe(false)
    expect(inspectorRetryUserIntentMatches(saveIntent, "saveTask", { ...saveInput, expected_lock_version: 8 })).toBe(true)

    const transition = buildInspectorTransitionCommand(model.task, "heartbeat", {}, "claim_1")
    expect(transition).not.toBeNull()
    const transitionIntent = { operation: "transition" as const, taskId: model.task.id, command: transition! }
    expect(inspectorRetryIntentMatches(transitionIntent, "transition", transition)).toBe(true)
    const changedTransition = buildInspectorTransitionCommand(model.task, "heartbeat", {}, "claim_2")
    expect(inspectorRetryIntentMatches(transitionIntent, "transition", changedTransition)).toBe(false)
    expect(inspectorRetryUserIntentMatches(transitionIntent, "transition", changedTransition)).toBe(true)
    expect(inspectorRetryIntentMatches(transitionIntent, "transition", null)).toBe(false)
  })

  test("renders an exact action retry even before a current dialog command exists", () => {
    const command = buildInspectorTransitionCommand(model.task, "block", { reason: "Needs review", confirmed: true }, null)
    expect(command).not.toBeNull()
    const key = inspectorMutationKey("transition", model.task.id)
    const snapshot: TaskInspectorMutationSnapshot = {
      scope: { identity: "runtime", boardId: "default", taskId: model.task.id },
      generation: 1,
      pending: new Set(),
      errors: new Map([[key, { operation: "transition", taskId: model.task.id, kind: "error", message: "failed", status: null, code: null, recoverable: true }]]),
      retries: new Map([[key, { operation: "transition", taskId: model.task.id, command: command! }]]),
    }
    const markup = renderToStaticMarkup(<TaskInspector model={model} onSelectTask={vi.fn()} locale="en" mutationHandlers={{} as TaskInspectorMutationHandlers} mutationSnapshot={snapshot} />)
    expect(markup).toContain("Retry")
  })

  test("keeps a save retry visible when the editor is not mounted", () => {
    const input = buildInspectorSaveTaskInput(model.task, inspectorEditDraft(model.task))
    const key = inspectorMutationKey("saveTask", model.task.id)
    const snapshot: TaskInspectorMutationSnapshot = {
      scope: { identity: "runtime", boardId: "default", taskId: model.task.id },
      generation: 1,
      pending: new Set(),
      errors: new Map([[key, { operation: "saveTask", taskId: model.task.id, kind: "error", message: "save failed", status: null, code: null, recoverable: true }]]),
      retries: new Map([[key, { operation: "saveTask", taskId: model.task.id, input }]]),
    }
    const markup = renderToStaticMarkup(<TaskInspector model={model} onSelectTask={vi.fn()} locale="en" mutationHandlers={{} as TaskInspectorMutationHandlers} mutationSnapshot={snapshot} />)
    expect(markup).toContain("save failed")
    expect(markup).toContain("Retry")
  })

  test("treats a pending reload as a write-wide disabled state", () => {
    const snapshot: TaskInspectorMutationSnapshot = {
      scope: { identity: "runtime", boardId: "default", taskId: model.task.id },
      generation: 1,
      pending: new Set([inspectorMutationKey("reload", model.task.id)]),
      errors: new Map(),
      retries: new Map(),
    }
    const markup = renderToStaticMarkup(<TaskInspector model={model} onSelectTask={vi.fn()} locale="en" mutationHandlers={{} as TaskInspectorMutationHandlers} mutationSnapshot={snapshot} />)
    expect(markup).toMatch(/class="[^"]*editButton[^"]*"[^>]*disabled/)
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

    const returnedSignal = fence.begin("session-a|t_1")
    expect(newSignal.aborted).toBe(true)
    expect(fence.isCurrent("session-a|t_1", oldSignal)).toBe(false)
    expect(fence.isCurrent("session-a|t_1", returnedSignal)).toBe(true)
    fence.abort()
    expect(returnedSignal.aborted).toBe(true)
  })
})

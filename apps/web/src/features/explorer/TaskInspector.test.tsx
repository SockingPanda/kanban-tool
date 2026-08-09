import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { TaskInspector, type TaskInspectorViewModel } from "./TaskInspector"

const model: TaskInspectorViewModel = {
  task: {
    id: "t_fixture",
    ref: "default#1",
    title: "Inspect the task",
    status: "running",
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
})

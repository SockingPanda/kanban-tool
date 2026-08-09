import { describe, expect, test, vi } from "vitest"

import type { DownloadedAttachment } from "../../lib/api/attachment-download"
import {
  createTaskClaimTokenStore,
  type InspectorTaskMutationClient,
  type TaskInspectorMutationScope,
  type TaskInspectorMutationSurface,
} from "./task-inspector-mutation-state"
import { TaskInspectorMutationController } from "./task-inspector-mutation-controller"

interface Deferred<T> {
  readonly promise: Promise<T>
  readonly resolve: (value: T) => void
  readonly reject: (error: unknown) => void
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

function scope(taskId = "t_1", boardId = "b_default"): TaskInspectorMutationScope {
  return { identity: `runtime\u0000${boardId}\u0000${taskId}`, boardId, taskId }
}

function response<T = never>(data = {} as T) {
  return { data }
}

function client(overrides: Partial<InspectorTaskMutationClient> = {}): InspectorTaskMutationClient {
  const transitionTask = vi.fn(async () => response()) as unknown as InspectorTaskMutationClient["transitionTask"]
  const defaults = {
    updateTask: vi.fn(async () => response()),
    transitionTask,
    addDependency: vi.fn(async () => response()),
    removeDependency: vi.fn(async () => response()),
    createStep: vi.fn(async () => response()),
    markExecutionPlanNotRequired: vi.fn(async () => response()),
    addTaskLabel: vi.fn(async () => response()),
    removeTaskLabel: vi.fn(async () => response()),
    createComment: vi.fn(async () => response()),
    createAttachment: vi.fn(async () => response()),
    deleteAttachment: vi.fn(async () => response()),
  } as unknown as InspectorTaskMutationClient
  return { ...defaults, ...overrides }
}

function surface(
  mutationClient: InspectorTaskMutationClient,
  currentScope = scope(),
  options: Partial<TaskInspectorMutationSurface> = {},
): TaskInspectorMutationSurface {
  return {
    scope: currentScope,
    client: mutationClient,
    ...options,
  }
}

describe("Task Inspector mutation controller", () => {
  test("keeps pending scoped and emits commit before awaited canonical reload", async () => {
    const mutationClient = client()
    const order: string[] = []
    const reload = vi.fn(async () => {
      order.push("reload")
    })
    const committed = vi.fn(() => {
      order.push("committed")
    })
    const controller = new TaskInspectorMutationController(surface(mutationClient, scope(), {
      onCanonicalReload: reload,
      onMutationCommitted: committed,
    }))

    const pending = controller.saveTask({ title: "Renamed", expected_lock_version: 3 })
    expect(controller.isPending("saveTask", "t_1")).toBe(true)
    expect(controller.isPending("saveTask", "t_2")).toBe(false)
    await pending

    expect(order).toEqual(["committed", "reload"])
    expect(committed).toHaveBeenCalledWith({ kind: "edit", taskId: "t_1" })
    expect(reload).toHaveBeenCalledWith({ kind: "edit", taskId: "t_1" }, scope())
    expect(controller.snapshot.pending.size).toBe(0)
    expect(controller.snapshot.retries.size).toBe(0)
  })

  test("never replays a committed write when canonical reload fails", async () => {
    const mutationClient = client()
    const reload = vi.fn()
      .mockRejectedValueOnce(new Error("read failed"))
      .mockResolvedValueOnce(undefined)
    const committed = vi.fn()
    const controller = new TaskInspectorMutationController(surface(mutationClient, scope(), {
      onCanonicalReload: reload,
      onMutationCommitted: committed,
    }))

    await controller.saveTask({ title: "Renamed", expected_lock_version: 3 })
    expect(mutationClient.updateTask).toHaveBeenCalledTimes(1)
    expect(committed).toHaveBeenCalledTimes(1)
    expect(controller.errorFor("reload", "t_1")).toMatchObject({ kind: "stale", recoverable: true })
    expect(controller.retryIntentFor("reload", "t_1")).toMatchObject({ operation: "reload", taskId: "t_1" })

    await expect(controller.retry()).resolves.toBe(true)
    expect(mutationClient.updateTask).toHaveBeenCalledTimes(1)
    expect(reload).toHaveBeenCalledTimes(2)
    expect(controller.errorFor("reload", "t_1")).toBeNull()
  })

  test("fences a late mutation result after a board/task switch", async () => {
    const write = deferred<ReturnType<typeof response>>()
    const mutationClient = client({ updateTask: vi.fn(() => write.promise) as unknown as InspectorTaskMutationClient["updateTask"] })
    const committed = vi.fn()
    const reload = vi.fn()
    const controller = new TaskInspectorMutationController(surface(mutationClient, scope(), {
      onCanonicalReload: reload,
      onMutationCommitted: committed,
    }))

    const first = controller.saveTask({ title: "Old task", expected_lock_version: 3 })
    expect(controller.isPending("saveTask", "t_1")).toBe(true)
    controller.setSurface(surface(client(), scope("t_2"), { onCanonicalReload: reload, onMutationCommitted: committed }))
    expect(controller.snapshot.scope.taskId).toBe("t_2")
    expect(controller.snapshot.pending.size).toBe(0)
    write.resolve(response())
    await first

    expect(committed).not.toHaveBeenCalled()
    expect(reload).not.toHaveBeenCalled()
    expect(controller.snapshot.errors.size).toBe(0)
  })

  test("retains a recoverable write error and retries only the failed intent", async () => {
    const updateTask = vi.fn()
      .mockRejectedValueOnce({ status: 500 })
      .mockResolvedValueOnce(response())
    const mutationClient = client({ updateTask })
    const controller = new TaskInspectorMutationController(surface(mutationClient))

    await controller.saveTask({ title: "Retry me", expected_lock_version: 3 })
    expect(controller.errorFor("saveTask", "t_1")).toMatchObject({ kind: "error", recoverable: true, status: 500 })
    expect(controller.retryIntentFor("saveTask", "t_1")).toMatchObject({ operation: "saveTask", taskId: "t_1" })
    await expect(controller.retry()).resolves.toBe(true)
    expect(updateTask).toHaveBeenCalledTimes(2)
    expect(controller.errorFor("saveTask", "t_1")).toBeNull()
  })

  test("allows a typed partial task update without manufacturing a title", async () => {
    const updateTask = vi.fn(async () => response())
    const controller = new TaskInspectorMutationController(surface(client({ updateTask })))

    await controller.saveTask({ expected_lock_version: 3 })
    expect(updateTask).toHaveBeenCalledWith("t_1", { expected_lock_version: 3 }, expect.any(Object))
  })

  test("shares claim tokens across transition calls and removes them on terminal actions", async () => {
    const transitionTask = vi.fn()
      .mockResolvedValueOnce(response({ claim_token: "claim-token" }))
      .mockResolvedValueOnce(response())
      .mockResolvedValueOnce(response())
    const mutationClient = client({ transitionTask: transitionTask as unknown as InspectorTaskMutationClient["transitionTask"] })
    const claimTokens = createTaskClaimTokenStore()
    const controller = new TaskInspectorMutationController(surface(mutationClient, scope(), { claimTokens }))

    await controller.transition({ action: "claim", input: { worker_profile: "manual", ttl_ms: 300_000 } })
    expect(claimTokens.get("t_1")).toBe("claim-token")
    await controller.transition({ action: "heartbeat", input: { claim_token: "", ttl_ms: 300_000 } })
    expect(transitionTask.mock.calls[1]?.[2]).toMatchObject({ claim_token: "claim-token", ttl_ms: 300_000 })
    await controller.transition({ action: "complete", input: { claim_token: "", force: false } })
    expect(claimTokens.get("t_1")).toBeNull()
  })

  test("passes non-tokenized legal completion commands through to the canonical transition client", async () => {
    const transitionTask = vi.fn(async () => response()) as unknown as InspectorTaskMutationClient["transitionTask"]
    const controller = new TaskInspectorMutationController(surface(client({ transitionTask })))

    await controller.transition({ action: "complete", input: {} })
    expect(transitionTask).toHaveBeenCalledTimes(1)
  })

  test("does not replay a rejected claim token on retry", async () => {
    const transitionTask = vi.fn()
      .mockRejectedValueOnce({ status: 403, apiError: { code: "claim_token_mismatch" } })
      .mockResolvedValueOnce(response())
    const claimTokens = createTaskClaimTokenStore({ t_1: "stale-token" })
    const controller = new TaskInspectorMutationController(surface(client({ transitionTask: transitionTask as unknown as InspectorTaskMutationClient["transitionTask"] }), scope(), { claimTokens }))

    await controller.transition({ action: "heartbeat", input: { claim_token: "", ttl_ms: 300_000 } })
    expect(claimTokens.get("t_1")).toBeNull()
    await expect(controller.retry()).resolves.toBe(false)
    expect(transitionTask).toHaveBeenCalledTimes(1)
  })

  test("clears task claim tokens when a runtime identity changes", () => {
    const claimTokens = createTaskClaimTokenStore({ t_1: "old-token" })
    const controller = new TaskInspectorMutationController(surface(client(), scope(), { claimTokens }))
    controller.setSurface(surface(client(), scope("t_1", "b_other"), { claimTokens }))
    expect(claimTokens.get("t_1")).toBeNull()
  })

  test("emits exact event kinds for detail write handlers", async () => {
    const committed = vi.fn()
    const mutationClient = client()
    const controller = new TaskInspectorMutationController(surface(mutationClient, scope(), { onMutationCommitted: committed }))

    await controller.addDependency("t_parent")
    await controller.removeDependency("t_parent")
    await controller.createStep({ title: "step" })
    await controller.linkStep({ title: "linked", linked_task_ref: "default#2" })
    await controller.markPlanNotRequired({ reason: "not needed" })
    await controller.addLabel({ name: "backend", create_missing: false })
    await controller.removeLabel("l_1")
    await controller.applySuggestedLabel({ name: "suggested", create_missing: false })
    await controller.addComment({ body: "note" })
    await controller.uploadAttachment({ filename: "a.txt", content: [1] })
    await controller.deleteAttachment("a_1")

    expect(committed.mock.calls.map(([event]) => event)).toEqual([
      { kind: "dependency", taskId: "t_1" },
      { kind: "dependency", taskId: "t_1" },
      { kind: "step", taskId: "t_1" },
      { kind: "step", taskId: "t_1" },
      { kind: "step", taskId: "t_1" },
      { kind: "label", taskId: "t_1" },
      { kind: "label", taskId: "t_1" },
      { kind: "label", taskId: "t_1" },
      { kind: "comment", taskId: "t_1" },
      { kind: "attachment", taskId: "t_1" },
      { kind: "attachment", taskId: "t_1" },
    ])
  })

  test("keeps downloads read-only and fenced", async () => {
    const downloaded: DownloadedAttachment = { content_type: "text/plain", attachment_id: "a_1", sha256: null, content: new Uint8Array([1]) }
    const download = vi.fn(async () => downloaded)
    const committed = vi.fn()
    const reload = vi.fn()
    const controller = new TaskInspectorMutationController(surface(client(), scope(), {
      attachmentDownload: { downloadAttachment: download },
      onMutationCommitted: committed,
      onCanonicalReload: reload,
    }))

    await expect(controller.downloadAttachment("a_1")).resolves.toEqual(downloaded)
    expect(download).toHaveBeenCalledWith("t_1", "a_1", expect.objectContaining({ signal: expect.any(AbortSignal) }))
    expect(committed).not.toHaveBeenCalled()
    expect(reload).not.toHaveBeenCalled()
  })

  test("provides a fenced, retryable label suggestion read", async () => {
    const suggestions = vi.fn()
      .mockRejectedValueOnce({ status: 503 })
      .mockResolvedValueOnce(response({ task_id: "t_1" }))
    const controller = new TaskInspectorMutationController(surface(client(), scope(), { suggestTaskLabels: suggestions }))

    await expect(controller.suggestLabels({ limit: 5 })).resolves.toBeNull()
    expect(controller.errorFor("suggestLabels", "t_1")).toMatchObject({ status: 503, recoverable: true })
    await expect(controller.retry()).resolves.toBe(true)
    expect(suggestions).toHaveBeenCalledTimes(2)
  })
})

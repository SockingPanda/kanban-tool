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
    onCanonicalReload: async () => undefined,
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
    await expect(pending).resolves.toEqual({ committed: true, reconciled: true })

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

    await expect(controller.saveTask({ title: "Renamed", expected_lock_version: 3 })).resolves.toEqual({ committed: true, reconciled: false })
    expect(mutationClient.updateTask).toHaveBeenCalledTimes(1)
    expect(committed).toHaveBeenCalledTimes(1)
    expect(controller.errorFor("reload", "t_1")).toMatchObject({ kind: "stale", recoverable: true })
    expect(controller.retryIntentFor("reload", "t_1")).toMatchObject({ operation: "reload", taskId: "t_1" })

    await expect(controller.retry()).resolves.toEqual({ committed: false, reconciled: true })
    expect(mutationClient.updateTask).toHaveBeenCalledTimes(1)
    expect(reload).toHaveBeenCalledTimes(2)
    expect(controller.errorFor("reload", "t_1")).toBeNull()
  })

  test("reports an uncommitted conflict after canonical reload without retaining the write intent", async () => {
    const updateTask = vi.fn().mockRejectedValue({ status: 409, apiError: { code: "conflict" } })
    const reload = vi.fn(async () => undefined)
    const controller = new TaskInspectorMutationController(surface(client({ updateTask }), scope(), { onCanonicalReload: reload }))

    await expect(controller.saveTask({ title: "Conflict", expected_lock_version: 3 })).resolves.toEqual({ committed: false, reconciled: true })
    expect(reload).toHaveBeenCalledWith({ kind: "edit", taskId: "t_1" }, scope())
    expect(controller.errorFor("saveTask", "t_1")).toMatchObject({ kind: "conflict" })
    expect(controller.retryIntentFor("saveTask", "t_1")).toBeNull()
    await expect(controller.retry()).resolves.toEqual({ committed: false, reconciled: false })
    expect(updateTask).toHaveBeenCalledTimes(1)
  })

  test("keeps only a canonical reload retry when conflict reconciliation fails", async () => {
    const updateTask = vi.fn().mockRejectedValue({ status: 409, apiError: { code: "conflict" } })
    const reload = vi.fn()
      .mockRejectedValueOnce(new Error("stale read"))
      .mockResolvedValueOnce(undefined)
    const controller = new TaskInspectorMutationController(surface(client({ updateTask }), scope(), { onCanonicalReload: reload }))

    await expect(controller.saveTask({ title: "Conflict stale", expected_lock_version: 3 })).resolves.toEqual({ committed: false, reconciled: false })
    expect(controller.errorFor("saveTask", "t_1")).toMatchObject({ kind: "conflict" })
    expect(controller.retryIntentFor("saveTask", "t_1")).toBeNull()
    expect(controller.errorFor("reload", "t_1")).toMatchObject({ kind: "stale" })
    expect(controller.retryIntentFor("reload", "t_1")).toMatchObject({ operation: "reload" })
    await expect(controller.retry()).resolves.toEqual({ committed: false, reconciled: true })
    expect(updateTask).toHaveBeenCalledTimes(1)
    expect(reload).toHaveBeenCalledTimes(2)
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
    await expect(first).resolves.toEqual({ committed: true, reconciled: false })

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

    await expect(controller.saveTask({ title: "Retry me", expected_lock_version: 3 })).resolves.toEqual({ committed: false, reconciled: false })
    expect(controller.errorFor("saveTask", "t_1")).toMatchObject({ kind: "error", recoverable: true, status: 500 })
    expect(controller.retryIntentFor("saveTask", "t_1")).toMatchObject({ operation: "saveTask", taskId: "t_1" })
    await expect(controller.retry()).resolves.toEqual({ committed: true, reconciled: true })
    expect(updateTask).toHaveBeenCalledTimes(2)
    expect(controller.errorFor("saveTask", "t_1")).toBeNull()
  })

  test("returns the full committed-but-unreconciled outcome from a write retry", async () => {
    const updateTask = vi.fn()
      .mockRejectedValueOnce({ status: 503 })
      .mockResolvedValueOnce(response())
    const reload = vi.fn().mockRejectedValueOnce(new Error("read failed"))
    const controller = new TaskInspectorMutationController(surface(client({ updateTask }), scope(), { onCanonicalReload: reload }))

    await expect(controller.saveTask({ title: "Retry commit", expected_lock_version: 3 })).resolves.toEqual({ committed: false, reconciled: false })
    await expect(controller.retry()).resolves.toEqual({ committed: true, reconciled: false })
    expect(updateTask).toHaveBeenCalledTimes(2)
    expect(controller.errorFor("reload", "t_1")).toMatchObject({ kind: "stale", recoverable: true })
  })

  test("returns an empty outcome when no retry candidate exists", async () => {
    const controller = new TaskInspectorMutationController(surface(client()))

    await expect(controller.retry()).resolves.toEqual({ committed: false, reconciled: false })
    await expect(controller.retry("saveTask:t_1")).resolves.toEqual({ committed: false, reconciled: false })
  })

  test("retries the exact requested key without consuming another intent", async () => {
    const updateTask = vi.fn()
      .mockRejectedValueOnce({ status: 503 })
      .mockResolvedValueOnce(response())
    const createComment = vi.fn()
      .mockRejectedValueOnce({ status: 503 })
      .mockResolvedValueOnce(response())
    const controller = new TaskInspectorMutationController(surface(client({ updateTask, createComment }), scope()))

    await expect(controller.saveTask({ title: "keep this retry", expected_lock_version: 3 })).resolves.toEqual({ committed: false, reconciled: false })
    await expect(controller.addComment({ body: "run this retry" })).resolves.toEqual({ committed: false, reconciled: false })
    await expect(controller.retry("addComment:t_1")).resolves.toEqual({ committed: true, reconciled: true })
    expect(controller.retryIntentFor("saveTask", "t_1")).toMatchObject({ operation: "saveTask" })
    await expect(controller.retry("saveTask:t_1")).resolves.toEqual({ committed: true, reconciled: true })
    expect(updateTask).toHaveBeenCalledTimes(2)
    expect(createComment).toHaveBeenCalledTimes(2)
  })

  test("clears a stale reload retry after a later write reconciles", async () => {
    const updateTask = vi.fn(async () => response())
    const reload = vi.fn()
      .mockRejectedValueOnce(new Error("read failed"))
      .mockResolvedValueOnce(undefined)
    const controller = new TaskInspectorMutationController(surface(client({ updateTask }), scope(), { onCanonicalReload: reload }))

    await expect(controller.saveTask({ title: "write A", expected_lock_version: 3 })).resolves.toEqual({ committed: true, reconciled: false })
    expect(controller.errorFor("reload", "t_1")).toMatchObject({ kind: "stale" })
    expect(controller.retryIntentFor("reload", "t_1")).toMatchObject({ operation: "reload" })
    await expect(controller.saveTask({ title: "write B", expected_lock_version: 4 })).resolves.toEqual({ committed: true, reconciled: true })
    expect(controller.errorFor("reload", "t_1")).toBeNull()
    expect(controller.retryIntentFor("reload", "t_1")).toBeNull()
  })

  test("allows a typed partial task update without manufacturing a title", async () => {
    const updateTask = vi.fn(async () => response())
    const controller = new TaskInspectorMutationController(surface(client({ updateTask })))

    await expect(controller.saveTask({ expected_lock_version: 3 })).resolves.toEqual({ committed: true, reconciled: true })
    expect(updateTask).toHaveBeenCalledWith("t_1", { expected_lock_version: 3 }, expect.any(Object))
  })

  test("omits optional undefined fields before transport validation", async () => {
    const updateTaskMock = vi.fn(async (...args: unknown[]) => {
      void args
      return response()
    })
    const createStepMock = vi.fn(async (...args: unknown[]) => {
      void args
      return response()
    })
    const updateTask = updateTaskMock as unknown as InspectorTaskMutationClient["updateTask"]
    const createStep = createStepMock as unknown as InspectorTaskMutationClient["createStep"]
    const mutationClient = client({ updateTask, createStep })
    const controller = new TaskInspectorMutationController(surface(mutationClient))

    await expect(controller.saveTask({ title: undefined, description: undefined, expected_lock_version: 3 })).resolves.toEqual({ committed: true, reconciled: true })
    expect(updateTaskMock.mock.calls[0]?.[1]).toEqual({ expected_lock_version: 3 })
    await expect(controller.createStep({ title: "step", body: undefined, linked_task_ref: undefined })).resolves.toEqual({ committed: true, reconciled: true })
    expect(createStepMock.mock.calls[0]?.[1]).toMatchObject({ title: "step", idempotency_key: expect.any(String) })
    expect(Object.prototype.hasOwnProperty.call(createStepMock.mock.calls[0]?.[1], "body")).toBe(false)
    expect(Object.prototype.hasOwnProperty.call(createStepMock.mock.calls[0]?.[1], "linked_task_ref")).toBe(false)
  })

  test("keeps non-idempotent write keys stable across retries", async () => {
    const createStep = vi.fn()
      .mockRejectedValueOnce({ status: 503 })
      .mockResolvedValueOnce(response())
    const createComment = vi.fn()
      .mockRejectedValueOnce({ status: 503 })
      .mockResolvedValueOnce(response())
    const createAttachment = vi.fn()
      .mockRejectedValueOnce({ status: 503 })
      .mockResolvedValueOnce(response())
    const mutationClient = client({ createStep, createComment, createAttachment })
    const controller = new TaskInspectorMutationController(surface(mutationClient))

    await expect(controller.createStep({ title: "retry step" })).resolves.toEqual({ committed: false, reconciled: false })
    await expect(controller.retry()).resolves.toEqual({ committed: true, reconciled: true })
    expect(createStep.mock.calls[0]?.[1].idempotency_key).toBeTruthy()
    expect(createStep.mock.calls[1]?.[1].idempotency_key).toBe(createStep.mock.calls[0]?.[1].idempotency_key)

    await expect(controller.linkStep({ title: "retry link", linked_task_ref: "default#2" })).resolves.toEqual({ committed: true, reconciled: true })
    expect(createStep.mock.calls[2]?.[1].idempotency_key).toBeTruthy()

    await expect(controller.addComment({ body: "retry comment" })).resolves.toEqual({ committed: false, reconciled: false })
    await expect(controller.retry()).resolves.toEqual({ committed: true, reconciled: true })
    expect(createComment.mock.calls[1]?.[1].idempotency_key).toBe(createComment.mock.calls[0]?.[1].idempotency_key)

    await expect(controller.uploadAttachment({ filename: "retry.txt", content: [1] })).resolves.toEqual({ committed: false, reconciled: false })
    await expect(controller.retry()).resolves.toEqual({ committed: true, reconciled: true })
    expect(createAttachment.mock.calls[0]?.[1].id).toMatch(/^a_/)
    expect(createAttachment.mock.calls[1]?.[1].id).toBe(createAttachment.mock.calls[0]?.[1].id)
  })

  test("returns an uncommitted outcome for a duplicate pending write", async () => {
    const write = deferred<ReturnType<typeof response>>()
    const updateTask = vi.fn(() => write.promise) as unknown as InspectorTaskMutationClient["updateTask"]
    const controller = new TaskInspectorMutationController(surface(client({ updateTask })))

    const first = controller.saveTask({ title: "Only once", expected_lock_version: 3 })
    await expect(controller.saveTask({ title: "Duplicate", expected_lock_version: 3 })).resolves.toEqual({ committed: false, reconciled: false })
    write.resolve(response())
    await expect(first).resolves.toEqual({ committed: true, reconciled: true })
    expect(updateTask).toHaveBeenCalledTimes(1)
  })

  test("allows reads but rejects every second write while one write is pending", async () => {
    const write = deferred<ReturnType<typeof response>>()
    const updateTask = vi.fn(() => write.promise) as unknown as InspectorTaskMutationClient["updateTask"]
    const createComment = vi.fn(async () => response())
    const mutationClient = client({ updateTask, createComment })
    const controller = new TaskInspectorMutationController(surface(mutationClient))

    const first = controller.saveTask({ title: "Single flight", expected_lock_version: 3 })
    await expect(controller.addComment({ body: "must wait" })).resolves.toEqual({ committed: false, reconciled: false })
    expect(createComment).not.toHaveBeenCalled()
    write.resolve(response())
    await expect(first).resolves.toEqual({ committed: true, reconciled: true })
  })

  test("uses the latest callbacks when a same-scope surface object is replaced", async () => {
    const write = deferred<ReturnType<typeof response>>()
    const updateTask = vi.fn(() => write.promise) as unknown as InspectorTaskMutationClient["updateTask"]
    const oldCommitted = vi.fn()
    const oldReload = vi.fn()
    const newCommitted = vi.fn()
    const newReload = vi.fn()
    const controller = new TaskInspectorMutationController(surface(client({ updateTask }), scope(), {
      onMutationCommitted: oldCommitted,
      onCanonicalReload: oldReload,
    }))

    const first = controller.saveTask({ title: "Replace surface", expected_lock_version: 3 })
    controller.setSurface(surface(client(), scope(), {
      onMutationCommitted: newCommitted,
      onCanonicalReload: newReload,
    }))
    write.resolve(response())

    await expect(first).resolves.toEqual({ committed: true, reconciled: true })
    expect(oldCommitted).not.toHaveBeenCalled()
    expect(oldReload).not.toHaveBeenCalled()
    expect(newCommitted).toHaveBeenCalledWith({ kind: "edit", taskId: "t_1" })
    expect(newReload).toHaveBeenCalledWith({ kind: "edit", taskId: "t_1" }, scope())
  })

  test("subscriber errors cannot prevent pending state from settling", async () => {
    const controller = new TaskInspectorMutationController(surface(client()))
    controller.subscribe(() => {
      throw new Error("subscriber failed")
    })

    await expect(controller.saveTask({ title: "settle", expected_lock_version: 3 })).resolves.toEqual({ committed: true, reconciled: true })
    expect(controller.snapshot.pending.size).toBe(0)
  })

  test("returns false for an abort without exposing a retry error", async () => {
    const aborted = Object.assign(new Error("aborted"), { name: "AbortError" })
    const updateTask = vi.fn().mockRejectedValue(aborted)
    const controller = new TaskInspectorMutationController(surface(client({ updateTask })))

    await expect(controller.saveTask({ title: "Abort me", expected_lock_version: 3 })).resolves.toEqual({ committed: false, reconciled: false })
    expect(controller.errorFor("saveTask", "t_1")).toBeNull()
    expect(controller.retryIntentFor("saveTask", "t_1")).toBeNull()
  })

  test("returns false for invalid input and an absent mutation surface", async () => {
    const controller = new TaskInspectorMutationController(null)
    await expect(controller.saveTask({ title: "No surface", expected_lock_version: 3 })).resolves.toEqual({ committed: false, reconciled: false })

    const active = new TaskInspectorMutationController(surface(client()))
    await expect(active.addDependency("   ")).resolves.toEqual({ committed: false, reconciled: false })
  })

  test("shares claim tokens across transition calls and removes them on terminal actions", async () => {
    const transitionTask = vi.fn()
      .mockResolvedValueOnce(response({ claim_token: "claim-token" }))
      .mockResolvedValueOnce(response())
      .mockResolvedValueOnce(response())
    const mutationClient = client({ transitionTask: transitionTask as unknown as InspectorTaskMutationClient["transitionTask"] })
    const claimTokens = createTaskClaimTokenStore()
    const controller = new TaskInspectorMutationController(surface(mutationClient, scope(), { claimTokens }))

    await expect(controller.transition({ action: "claim", input: { worker_profile: "manual", ttl_ms: 300_000 } })).resolves.toEqual({ committed: true, reconciled: true })
    expect(claimTokens.get("t_1")).toBe("claim-token")
    await expect(controller.transition({ action: "heartbeat", input: { claim_token: "", ttl_ms: 300_000 } })).resolves.toEqual({ committed: true, reconciled: true })
    expect(transitionTask.mock.calls[1]?.[2]).toMatchObject({ claim_token: "claim-token", ttl_ms: 300_000 })
    await expect(controller.transition({ action: "complete", input: { claim_token: "", force: false } })).resolves.toEqual({ committed: true, reconciled: true })
    expect(claimTokens.get("t_1")).toBeNull()
  })

  test("passes non-tokenized legal completion commands through to the canonical transition client", async () => {
    const transitionTask = vi.fn(async () => response()) as unknown as InspectorTaskMutationClient["transitionTask"]
    const controller = new TaskInspectorMutationController(surface(client({ transitionTask })))

    await expect(controller.transition({ action: "complete", input: {} })).resolves.toEqual({ committed: true, reconciled: true })
    expect(transitionTask).toHaveBeenCalledTimes(1)
  })

  test("does not replay a rejected claim token on retry", async () => {
    const transitionTask = vi.fn()
      .mockRejectedValueOnce({ status: 403, apiError: { code: "claim_token_mismatch" } })
      .mockResolvedValueOnce(response())
    const claimTokens = createTaskClaimTokenStore({ t_1: "stale-token" })
    const controller = new TaskInspectorMutationController(surface(client({ transitionTask: transitionTask as unknown as InspectorTaskMutationClient["transitionTask"] }), scope(), { claimTokens }))

    await expect(controller.transition({ action: "heartbeat", input: { claim_token: "", ttl_ms: 300_000 } })).resolves.toEqual({ committed: false, reconciled: true })
    expect(claimTokens.get("t_1")).toBeNull()
    expect(controller.errorFor("transition", "t_1")).toMatchObject({ kind: "conflict", recoverable: true })
    expect(controller.retryIntentFor("transition", "t_1")).toBeNull()
    await expect(controller.retry()).resolves.toEqual({ committed: false, reconciled: false })
    expect(transitionTask).toHaveBeenCalledTimes(1)
  })

  test("preserves a shared claim token across inspector unmount and same-identity reopen", async () => {
    const transitionTask = vi.fn()
      .mockResolvedValueOnce(response({ claim_token: "shared-token" }))
      .mockResolvedValueOnce(response())
    const claimTokens = createTaskClaimTokenStore()
    const mutationClient = client({ transitionTask: transitionTask as unknown as InspectorTaskMutationClient["transitionTask"] })
    const controller = new TaskInspectorMutationController(surface(mutationClient, scope(), { claimTokens }))

    await expect(controller.transition({ action: "claim", input: { worker_profile: "manual", ttl_ms: 300_000 } })).resolves.toEqual({ committed: true, reconciled: true })
    controller.setSurface(null)
    controller.setSurface(surface(mutationClient, scope(), { claimTokens }))
    await expect(controller.transition({ action: "heartbeat", input: { claim_token: "", ttl_ms: 300_000 } })).resolves.toEqual({ committed: true, reconciled: true })

    expect(transitionTask.mock.calls[1]?.[2]).toMatchObject({ claim_token: "shared-token" })
  })

  test("preserves a fallback claim token while switching between tasks", async () => {
    const transitionTask = vi.fn()
      .mockResolvedValueOnce(response({ claim_token: "fallback-token" }))
      .mockResolvedValueOnce(response())
    const mutationClient = client({ transitionTask: transitionTask as unknown as InspectorTaskMutationClient["transitionTask"] })
    const controller = new TaskInspectorMutationController(surface(mutationClient, scope("t_1")))

    await expect(controller.transition({ action: "claim", input: { worker_profile: "manual", ttl_ms: 300_000 } })).resolves.toEqual({ committed: true, reconciled: true })
    controller.setSurface(surface(mutationClient, scope("t_2")))
    controller.setSurface(surface(mutationClient, scope("t_1")))
    await expect(controller.transition({ action: "heartbeat", input: { claim_token: "", ttl_ms: 300_000 } })).resolves.toEqual({ committed: true, reconciled: true })

    expect(transitionTask.mock.calls[1]?.[2]).toMatchObject({ claim_token: "fallback-token" })
  })

  test("clears task claim tokens when the same task is rebound to a new runtime identity", () => {
    const claimTokens = createTaskClaimTokenStore({ t_1: "old-token" })
    const controller = new TaskInspectorMutationController(surface(client(), scope(), { claimTokens }))
    controller.setSurface(surface(client(), scope("t_1", "b_other"), { claimTokens }))
    expect(claimTokens.get("t_1")).toBeNull()
  })

  test("clears the fallback claim token when the same task is rebound to a new runtime identity", async () => {
    const transitionTask = vi.fn()
      .mockResolvedValueOnce(response({ claim_token: "old-token" }))
    const mutationClient = client({ transitionTask: transitionTask as unknown as InspectorTaskMutationClient["transitionTask"] })
    const controller = new TaskInspectorMutationController(surface(mutationClient, scope("t_1")))

    await expect(controller.transition({ action: "claim", input: { worker_profile: "manual", ttl_ms: 300_000 } })).resolves.toEqual({ committed: true, reconciled: true })
    controller.setSurface(surface(mutationClient, scope("t_1", "b_other")))
    await expect(controller.transition({ action: "heartbeat", input: { claim_token: "", ttl_ms: 300_000 } })).resolves.toEqual({ committed: false, reconciled: false })

    expect(transitionTask).toHaveBeenCalledTimes(1)
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

  test("keeps download failures visible without registering a retry", async () => {
    const downloaded: DownloadedAttachment = { content_type: "text/plain", attachment_id: "a_1", sha256: null, content: new Uint8Array([1]) }
    const download = vi.fn()
      .mockRejectedValueOnce({ status: 503 })
      .mockResolvedValueOnce(downloaded)
    const controller = new TaskInspectorMutationController(surface(client(), scope(), {
      attachmentDownload: { downloadAttachment: download },
    }))

    await expect(controller.downloadAttachment("a_1")).resolves.toBeNull()
    expect(controller.errorFor("downloadAttachment", "t_1")).toMatchObject({ status: 503, recoverable: true })
    expect(controller.retryIntentFor("downloadAttachment", "t_1")).toBeNull()
    await expect(controller.retry()).resolves.toEqual({ committed: false, reconciled: false })
    await expect(controller.downloadAttachment("a_1")).resolves.toEqual(downloaded)
    expect(download).toHaveBeenCalledTimes(2)
  })

  test("keeps suggestion read failures visible without registering a retry", async () => {
    const suggestions = vi.fn()
      .mockRejectedValueOnce({ status: 503 })
      .mockResolvedValueOnce(response({ task_id: "t_1" }))
    const controller = new TaskInspectorMutationController(surface(client(), scope(), { suggestTaskLabels: suggestions }))

    await expect(controller.suggestLabels({ limit: 5 })).resolves.toBeNull()
    expect(controller.errorFor("suggestLabels", "t_1")).toMatchObject({ status: 503, recoverable: true })
    expect(controller.retryIntentFor("suggestLabels", "t_1")).toBeNull()
    await expect(controller.retry()).resolves.toEqual({ committed: false, reconciled: false })
    await expect(controller.suggestLabels({ limit: 5 })).resolves.toEqual(response({ task_id: "t_1" }))
    expect(suggestions).toHaveBeenCalledTimes(2)
  })
})

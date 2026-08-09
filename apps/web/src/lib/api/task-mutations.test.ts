import { describe, expect, test, vi } from "vitest"

import { assertCanonicalBoardSlug } from "../board-slug"
import type { WebRuntimeConfig } from "../runtime"
import type { HttpTransport } from "./http-transport"
import { createTaskMutationClient, type BlockTaskIntent, type ClaimTaskIntent, type CompleteTaskIntent, type SubmitReviewTaskIntent } from "./task-mutations"

const runtime = {
  apiBaseUrl: "/__kb_api__",
  webBasePath: "/app/",
  actor: "web-user",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "sha256:test",
} satisfies WebRuntimeConfig

const activeBoard = assertCanonicalBoardSlug("default")

const claimInput: ClaimTaskIntent = { worker_profile: "default" }
const completeInput: CompleteTaskIntent = { claim_token: "claim-token", summary: "done" }
const blockInput: BlockTaskIntent = { reason: "waiting" }
const reviewInput: SubmitReviewTaskIntent = { summary: "review" }

function task(id = "t_1", status: "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review" | "done" | "archived" = "todo") {
  return {
    id,
    board_id: "b_default",
    board_slug: "default",
    ref: "default#1",
    seq: 1,
    title: "Task",
    description: null,
    status,
    status_reason: null,
    assignee: null,
    priority: 3,
    position: 1024,
    scheduled_at: null,
    due_at: null,
    created_by: "web-user",
    created_at: 1,
    updated_at: 2,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    current_run_id: null,
    retry_count: 0,
    max_retries: null,
    result_summary: null,
    result: null,
    metadata: {},
    lock_version: 1,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "not_required" as const,
    required_step_count: 0,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
  }
}

const attachment = {
  id: "a_1",
  board_id: "b_default",
  task_id: "t_1",
  filename: "hello.txt",
  rel_path: "attachments/b_default/t_1/hello.txt",
  content_type: "text/plain",
  size_bytes: 2,
  sha256: "sha256-fixture",
  created_by: "web-user",
  created_at: 1,
}

function transitionResponse(action: string) {
  if (action === "claim") {
    return {
      payload: {
        data: {
          task: task("t_1", "running"),
          run: {
            id: "r_1",
            task_id: "t_1",
            status: "running",
            worker_profile: "default",
            worker_pid: null,
            claim_owner: "web-user",
            started_at: 1,
            finished_at: null,
            exit_code: null,
            summary: null,
            error: null,
            has_log: false,
            metadata: {},
          },
          claim_token: "claim-token",
          claim_expires_at: 10,
        },
      },
      bytes: 1,
    }
  }

  const statusByAction: Record<string, Parameters<typeof task>[1]> = {
    specify: "todo",
    promote: "ready",
    heartbeat: "running",
    complete: "done",
    "submit-review": "review",
    block: "blocked",
    unblock: "todo",
    archive: "archived",
  }
  return { payload: { data: task("t_1", statusByAction[action]) }, bytes: 1 }
}

describe("task mutation operations", () => {
  test("validates and sends a create intent through the generated path/body contracts", async () => {
    const request = vi.fn<HttpTransport["request"]>(async () => ({
      payload: { data: task("t_created", "triage") },
      bytes: 1,
    }))
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    const result = await client.createTask({
      title: "New task",
      description: "Details",
      idempotency_key: "task.create:test",
    })

    expect(result.data.id).toBe("t_created")
    expect(request).toHaveBeenCalledWith({
      method: "POST",
      path: "/api/v1/boards/default/tasks",
      body: {
        actor: "web-user",
        title: "New task",
        description: "Details",
        idempotency_key: "task.create:test",
      },
      headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" },
    })
  })

  test("binds task creation to the active canonical board selector", async () => {
    const request = vi.fn<HttpTransport["request"]>(async () => ({
      payload: { data: task("t_created", "triage") },
      bytes: 1,
    }))
    const client = createTaskMutationClient(runtime, assertCanonicalBoardSlug("team-two"), { transport: { request } })

    await client.createTask({ title: "Team task" })

    expect(request).toHaveBeenCalledWith(expect.objectContaining({
      path: "/api/v1/boards/team-two/tasks",
    }))
  })

  test("uses expected_lock_version for update and parses the canonical task response", async () => {
    const request = vi.fn<HttpTransport["request"]>(async () => ({
      payload: { data: task("t_1", "todo") },
      bytes: 1,
    }))
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    await client.updateTask("t_1", { title: "Renamed", expected_lock_version: 7 })

    expect(request).toHaveBeenCalledWith({
      method: "PATCH",
      path: "/api/v1/tasks/t_1",
      body: { actor: "web-user", title: "Renamed", expected_lock_version: 7 },
      headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" },
    })
  })

  test("adds and removes task labels through the generated path and body contracts", async () => {
    const request = vi.fn<HttpTransport["request"]>(async ({ method }) => ({
      payload: method === "POST"
        ? { data: task("t_1", "todo"), meta: null }
        : { data: task("t_1", "todo") },
      bytes: 1,
    }))
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    await client.addTaskLabel("t_1", { name: "urgent", create_missing: true })
    await client.removeTaskLabel("t_1", "l_label/one")

    expect(request).toHaveBeenNthCalledWith(1, {
      method: "POST",
      path: "/api/v1/tasks/t_1/labels",
      body: { actor: "web-user", name: "urgent", create_missing: true },
      headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" },
    })
    expect(request).toHaveBeenNthCalledWith(2, {
      method: "DELETE",
      path: "/api/v1/tasks/t_1/labels/l_label%2Fone",
      headers: { "X-KB-Actor": "web-user" },
    })
  })

  test("keeps heartbeat claim_token required at the public type boundary", () => {
    const request = vi.fn<HttpTransport["request"]>()
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    const invalidCall = () => {
      // @ts-expect-error heartbeat requests must carry the generated claim_token field
      return client.transitionTask("t_1", "heartbeat", {})
    }

    expect(invalidCall).toBeTypeOf("function")
  })

  test("routes each legal transition to its own generated request/response validator", async () => {
    const request = vi.fn<HttpTransport["request"]>(async ({ path }) => ({
      payload: { data: task("t_1", path.endsWith("/block") ? "blocked" : "todo") },
      bytes: 1,
    }))
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    const result = await client.transitionTask("t_1", "block", blockInput)

    expect(result.data.status).toBe("blocked")
    expect(request).toHaveBeenCalledWith({
      method: "POST",
      path: "/api/v1/tasks/t_1/transitions/block",
      body: { actor: "web-user", reason: "waiting" },
      headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" },
    })
  })

  test("covers every rendered transition route, body, actor header, and response branch", async () => {
    const request = vi.fn<HttpTransport["request"]>(async ({ path }) => ({
      ...transitionResponse(path.split("/").at(-1) ?? ""),
    }))
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })
    const transitionCases = [
      { action: "specify", run: () => client.transitionTask("t_1", "specify", { description: "ready" }) },
      { action: "promote", run: () => client.transitionTask("t_1", "promote") },
      { action: "claim", run: () => client.transitionTask("t_1", "claim", claimInput) },
      { action: "heartbeat", run: () => client.transitionTask("t_1", "heartbeat", { claim_token: "claim-token" }) },
      { action: "complete", run: () => client.transitionTask("t_1", "complete", completeInput) },
      { action: "submit-review", run: () => client.transitionTask("t_1", "submit-review", reviewInput) },
      { action: "block", run: () => client.transitionTask("t_1", "block", blockInput) },
      { action: "unblock", run: () => client.transitionTask("t_1", "unblock") },
      { action: "archive", run: () => client.transitionTask("t_1", "archive") },
    ]

    for (const transition of transitionCases) await transition.run()

    expect(request).toHaveBeenCalledTimes(9)
    for (const [index, transition] of transitionCases.entries()) {
      expect(request).toHaveBeenNthCalledWith(index + 1, expect.objectContaining({
        method: "POST",
        path: `/api/v1/tasks/t_1/transitions/${transition.action}`,
        headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" },
      }))
    }
    expect(request.mock.calls[2]?.[0]).toMatchObject({ body: { actor: "web-user", worker_profile: "default" } })
    expect(request.mock.calls[3]?.[0]).toMatchObject({ body: { actor: "web-user", claim_token: "claim-token" } })
    expect(request.mock.calls[6]?.[0]).toMatchObject({ body: { actor: "web-user", reason: "waiting" } })
  })

  test("rejects invalid response payloads before exposing a mutation result", async () => {
    const request = vi.fn<HttpTransport["request"]>(async () => ({
      payload: { data: { id: "missing-fields" } },
      bytes: 1,
    }))
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    await expect(client.createTask({ title: "New task" })).rejects.toMatchObject({
      name: "ContractValidationError",
      contractId: "api.create-task.response",
    })
  })

  test("covers dependency, step, and comment intents without exposing untyped requests", async () => {
    const request = vi.fn<HttpTransport["request"]>(async ({ path }) => {
      if (path.endsWith("/dependencies")) return { payload: { data: { task: { id: "t_1", board_id: "b_default", board_slug: "default", ref: "default#1", title: "Task", status: "todo" }, parents: [], children: [], edges: [] } }, bytes: 1 }
      if (path.endsWith("/steps")) return { payload: { data: { task_id: "t_1", steps: [], execution_plan: { board_id: "b_default", task_id: "t_1", state: "planned", reason: null, updated_by: "web-user", updated_at: 1 } } }, bytes: 1 }
      return { payload: { data: { id: "c_1", board_id: "b_default", task_id: "t_1", author: "web-user", author_type: "user", agent_type: null, body: "note", kind: "note", metadata: {}, created_at: 1 } }, bytes: 1 }
    })
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    await client.addDependency("t_1", "t_parent")
    await client.createStep("t_1", { title: "Verify" })
    await client.createComment("t_1", { body: "note" })

    expect(request).toHaveBeenNthCalledWith(1, { method: "POST", path: "/api/v1/tasks/t_1/dependencies", body: { actor: "web-user", parent_task_id: "t_parent" }, headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" } })
    expect(request).toHaveBeenNthCalledWith(2, { method: "POST", path: "/api/v1/tasks/t_1/steps", body: { actor: "web-user", title: "Verify" }, headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" } })
    expect(request).toHaveBeenNthCalledWith(3, { method: "POST", path: "/api/v1/tasks/t_1/comments", body: { author: "web-user", body: "note" }, headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" } })
  })

  test("covers read/remove dependency, step, plan, and comment branches", async () => {
    const dependencyTask = { id: "t_1", board_id: "b_default", board_slug: "default", ref: "default#1", title: "Task", status: "todo" }
    const request = vi.fn<HttpTransport["request"]>(async ({ method, path }) => {
      if (path.endsWith("/dependencies") && method === "GET") {
        return { payload: { data: { task: dependencyTask, parents: [], children: [], edges: [] } }, bytes: 1 }
      }
      if (path.includes("/dependencies/") && method === "DELETE") {
        return { payload: { data: { task: dependencyTask, parents: [], children: [], edges: [] } }, bytes: 1 }
      }
      if (path.endsWith("/steps")) {
        return { payload: { data: { task_id: "t_1", steps: [], execution_plan: { board_id: "b_default", task_id: "t_1", state: "planned", reason: null, updated_by: "web-user", updated_at: 1 } } }, bytes: 1 }
      }
      if (path.endsWith("/execution-plan/not-required")) {
        return { payload: { data: { board_id: "b_default", task_id: "t_1", state: "not_required", reason: "not needed", updated_by: "web-user", updated_at: 1 } }, bytes: 1 }
      }
      if (path.endsWith("/comments")) return { payload: { data: [] }, bytes: 1 }
      throw new Error(`unexpected request: ${method} ${path}`)
    })
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    await client.listDependencies("t_1")
    await client.removeDependency("t_1", "t_parent")
    await client.listSteps("t_1")
    await client.markExecutionPlanNotRequired("t_1", { reason: "not needed" })
    await client.listComments("t_1")

    expect(request).toHaveBeenNthCalledWith(1, expect.objectContaining({ method: "GET", path: "/api/v1/tasks/t_1/dependencies", headers: {} }))
    expect(request).toHaveBeenNthCalledWith(2, expect.objectContaining({ method: "DELETE", path: "/api/v1/tasks/t_1/dependencies/t_parent", headers: { "X-KB-Actor": "web-user" } }))
    expect(request).toHaveBeenNthCalledWith(3, expect.objectContaining({ method: "GET", path: "/api/v1/tasks/t_1/steps", headers: {} }))
    expect(request).toHaveBeenNthCalledWith(4, expect.objectContaining({ method: "POST", path: "/api/v1/tasks/t_1/execution-plan/not-required", body: { actor: "web-user", reason: "not needed" }, headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" } }))
    expect(request).toHaveBeenNthCalledWith(5, expect.objectContaining({ method: "GET", path: "/api/v1/tasks/t_1/comments", headers: {} }))
  })

  test("covers attachment metadata list/create/delete operations", async () => {
    const request = vi.fn<HttpTransport["request"]>(async ({ method, path }) => {
      if (method === "GET") return { payload: { data: [attachment] }, bytes: 1 }
      if (method === "POST") return { payload: { data: attachment }, bytes: 1 }
      if (path.endsWith("/attachments/a_1")) return { payload: { data: { deleted: true } }, bytes: 1 }
      throw new Error(`unexpected request: ${method} ${path}`)
    })
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    await expect(client.listAttachments("t_1")).resolves.toEqual({ data: [attachment] })
    await expect(client.createAttachment("t_1", {
      filename: "hello.txt",
      content: [104, 105],
      content_type: "text/plain",
    })).resolves.toEqual({ data: attachment })
    await expect(client.deleteAttachment("t_1", "a_1")).resolves.toEqual({ data: { deleted: true } })

    expect(request).toHaveBeenNthCalledWith(1, {
      method: "GET",
      path: "/api/v1/tasks/t_1/attachments",
      headers: {},
    })
    expect(request).toHaveBeenNthCalledWith(2, {
      method: "POST",
      path: "/api/v1/tasks/t_1/attachments",
      body: {
        actor: "web-user",
        filename: "hello.txt",
        content: [104, 105],
        content_type: "text/plain",
      },
      headers: { "Content-Type": "application/json", "X-KB-Actor": "web-user" },
    })
    expect(request).toHaveBeenNthCalledWith(3, {
      method: "DELETE",
      path: "/api/v1/tasks/t_1/attachments/a_1",
      headers: { "X-KB-Actor": "web-user" },
    })
  })

  test("requests manual label suggestions with generated query contracts and preserves degraded reasons", async () => {
    const request = vi.fn<HttpTransport["request"]>(async () => ({
      payload: {
        data: {
          task_id: "t_1",
          board_id: "b_default",
          selected_labels: [],
          candidates: [],
          coverage: 0,
          coverage_cosine: 0,
          residual_norm: 1,
          needs_new_label: false,
          reason_codes: ["degraded_result", "vector_store_disabled"],
          degraded: true,
          diagnostics: ["vector_store_disabled"],
        },
      },
      bytes: 1,
    }))
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })
    const signal = new AbortController().signal

    const result = await client.suggestTaskLabels("t_1", {
      limit: 3,
      candidate_limit: 32,
      atom_limit: 80,
      max_selected_labels: 4,
      min_score: 0.15,
    }, { signal })

    expect(result.data).toMatchObject({
      task_id: "t_1",
      reason_codes: ["degraded_result", "vector_store_disabled"],
      degraded: true,
      diagnostics: ["vector_store_disabled"],
    })
    expect(request).toHaveBeenCalledWith({
      method: "GET",
      path: "/api/v1/tasks/t_1/labels/suggestions?limit=3&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15",
      headers: {},
      signal,
    })
  })

  test("rejects a malformed label suggestion response through its generated validator", async () => {
    const request = vi.fn<HttpTransport["request"]>(async () => ({
      payload: {
        data: {
          task_id: "t_1",
          board_id: "b_default",
          selected_labels: [],
          candidates: [],
          coverage: "unknown",
          coverage_cosine: 0,
          residual_norm: 1,
          needs_new_label: false,
          reason_codes: [],
          degraded: false,
          diagnostics: [],
        },
      },
      bytes: 1,
    }))
    const client = createTaskMutationClient(runtime, activeBoard, { transport: { request } })

    await expect(client.suggestTaskLabels("t_1")).rejects.toMatchObject({
      name: "ContractValidationError",
      contractId: "api.suggest-task-labels.response",
    })
  })
})

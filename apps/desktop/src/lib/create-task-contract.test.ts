import { readFileSync } from "node:fs"
import { afterEach, describe, expect, it, vi } from "vitest"
import { KanbanApi } from "./api"

const config = { apiBaseUrl: "http://127.0.0.1:8721", actor: "desktop-test", board: "project" }
const fixture = JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/create-task-response.v1.valid.json", import.meta.url), "utf8"))
const response = (value: unknown, status = 201) => new Response(JSON.stringify(value), { status, headers: { "Content-Type": "application/json" } })

afterEach(() => vi.unstubAllGlobals())

describe("create task exact response contract", () => {
  it("consumes the committed response fixture", async () => {
    const fetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => response(fixture))
    vi.stubGlobal("fetch", fetch)
    const task = await new KanbanApi(config, { locale: "zh-CN" }).createTask({ title: "Contract child", status: "ready" })
    expect(task.ref).toBe("project#2")
    expect(task.status).toBe("todo")
    const body = JSON.parse(String(fetch.mock.calls[0]?.[1]?.body))
    expect(body.task_id).toMatch(/^t_[0-9A-F]{32}$/)
    expect(body.idempotency_key).toBe(`task.create:${body.task_id}`)
  })

  it("preserves caller identifiers for an explicit retry", async () => {
    const fetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => response(fixture))
    vi.stubGlobal("fetch", fetch)
    await new KanbanApi(config).createTask({
      title: "Retry",
      taskId: "t_retry",
      idempotencyKey: "retry-key",
    })
    const body = JSON.parse(String(fetch.mock.calls[0]?.[1]?.body))
    expect(body.task_id).toBe("t_retry")
    expect(body.idempotency_key).toBe("retry-key")
  })

  for (const [name, mutate] of [
    ["extra envelope", (value: any) => ({ ...value, meta: {} })],
    ["extra task field", (value: any) => ({ data: { ...value.data, claim_token: "secret" } })],
    ["missing nullable", (value: any) => { const data = { ...value.data }; delete data.description; return { data } }],
    ["invalid status", (value: any) => ({ data: { ...value.data, status: "unknown" } })],
    ["invalid priority", (value: any) => ({ data: { ...value.data, priority: 4 } })],
  ] as const) {
    it(`rejects ${name}`, async () => {
      vi.stubGlobal("fetch", vi.fn(async () => response(mutate(structuredClone(fixture)))))
      await expect(new KanbanApi(config).createTask({ title: "x" })).rejects.toMatchObject({ code: "invalid_response" })
    })
  }
})

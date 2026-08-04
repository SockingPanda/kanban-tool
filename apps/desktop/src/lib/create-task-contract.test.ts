import { readFileSync } from "node:fs"
import { afterEach, describe, expect, it, vi } from "vitest"
import { KanbanApi } from "./api"

const config = { apiBaseUrl: "http://127.0.0.1:8721", actor: "desktop-test", board: "project" }
const fixture = JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/create-task-response.v1.valid.json", import.meta.url), "utf8"))
const response = (value: unknown, status = 201) => new Response(JSON.stringify(value), { status, headers: { "Content-Type": "application/json" } })

afterEach(() => vi.unstubAllGlobals())

describe("create task exact response contract", () => {
  it("consumes the committed response fixture", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => response(fixture)))
    const task = await new KanbanApi(config, { locale: "zh-CN" }).createTask({ title: "Contract child", status: "ready" })
    expect(task.ref).toBe("project#2")
    expect(task.status).toBe("todo")
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

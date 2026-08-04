import { readFileSync } from "node:fs"
import { afterEach, describe, expect, it, vi } from "vitest"
import { KanbanApi } from "./api"

const config = { apiBaseUrl: "http://127.0.0.1:8721", actor: "desktop-test", board: "default" }
const list = JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/list-runs-response.v1.valid.json", import.meta.url), "utf8"))
const get = JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/get-run-response.v1.valid.json", import.meta.url), "utf8"))
function response(value: unknown, status = 200) { return new Response(JSON.stringify(value), { status, headers: { "Content-Type": "application/json" } }) }

afterEach(() => vi.unstubAllGlobals())

describe("runs exact contracts", () => {
  it("consumes committed list and get fixtures", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(response(list)).mockResolvedValueOnce(response(get)))
    const api = new KanbanApi(config, { locale: "zh-CN" })
    expect(await api.listRuns("t_fixture")).toHaveLength(2)
    expect((await api.getRun("r_finished")).status).toBe("succeeded")
  })

  it("sends exact GET transport and locale without actor header", async () => {
    const fetch = vi.fn().mockResolvedValueOnce(response(list)).mockResolvedValueOnce(response(get))
    vi.stubGlobal("fetch", fetch)
    const api = new KanbanApi(config, { locale: "zh-CN" })
    await api.listRuns("t_fixture")
    await api.getRun("r_active")
    expect(fetch.mock.calls).toEqual([
      ["http://127.0.0.1:8721/api/v1/tasks/t_fixture/runs", expect.objectContaining({ method: "GET", headers: { "Accept-Language": "zh-CN" } })],
      ["http://127.0.0.1:8721/api/v1/runs/r_active", expect.objectContaining({ method: "GET", headers: { "Accept-Language": "zh-CN" } })],
    ])
    for (const [, init] of fetch.mock.calls as [string, RequestInit][]) expect((init.headers as Record<string, string>)["X-KB-Actor"]).toBeUndefined()
  })

  for (const [name, mutate] of [
    ["claim token", (value: any) => ({ data: { ...value.data, claim_token: "secret" } })],
    ["log path", (value: any) => ({ data: { ...value.data, log_path: "/private/run.log" } })],
    ["unknown status", (value: any) => ({ data: { ...value.data, status: "unknown" } })],
    ["missing nullable", (value: any) => { const data = { ...value.data }; delete data.finished_at; return { data } }],
    ["unsafe timestamp", (value: any) => ({ data: { ...value.data, started_at: Number.MAX_SAFE_INTEGER + 1 } })],
    ["extra envelope", (value: any) => ({ ...value, meta: {} })],
  ] as const) it(`get rejects ${name}`, async () => {
    vi.stubGlobal("fetch", vi.fn(async () => response(mutate(structuredClone(get)))))
    await expect(new KanbanApi(config).getRun("r_active")).rejects.toMatchObject({ code: "invalid_response" })
  })

  it("list rejects a hostile nested run", async () => {
    const hostile = structuredClone(list); hostile.data[0].claim_token = "secret"
    vi.stubGlobal("fetch", vi.fn(async () => response(hostile)))
    await expect(new KanbanApi(config).listRuns("t_fixture")).rejects.toMatchObject({ code: "invalid_response" })
  })

  for (const [name, hostile] of [
    ["extra outer envelope", { ...structuredClone(list), meta: {} }],
    ["object data", { data: {} }],
    ["null data", { data: null }],
  ] as const) it(`list rejects ${name}`, async () => {
    vi.stubGlobal("fetch", vi.fn(async () => response(hostile)))
    await expect(new KanbanApi(config).listRuns("t_fixture")).rejects.toMatchObject({ code: "invalid_response" })
  })

  it("consumes exact non-2xx error envelope", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => response({ error: { code: "not_found", message: "run missing", details: { run_id: "r_missing" } } }, 404)))
    await expect(new KanbanApi(config).getRun("r_missing")).rejects.toMatchObject({ code: "not_found", message: "run missing", details: { run_id: "r_missing" } })
  })
})

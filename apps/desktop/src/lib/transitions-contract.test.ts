import { readFileSync } from "node:fs"
import { afterEach, describe, expect, it, vi } from "vitest"
import { KanbanApi, type Task } from "./api"

const config = { apiBaseUrl: "http://127.0.0.1:8721", actor: "desktop-test", board: "default" }
const actions = ["specify", "promote", "reopen", "unblock", "archive"] as const
const fixtures = Object.fromEntries(actions.map((action) => [
  action,
  JSON.parse(readFileSync(new URL(`../../../../schemas/fixtures/api/${action}-task-response.v1.valid.json`, import.meta.url), "utf8")),
])) as Record<(typeof actions)[number], { data: Task }>

function response(value: unknown) {
  return new Response(JSON.stringify(value), { status: 200, headers: { "Content-Type": "application/json" } })
}

afterEach(() => vi.unstubAllGlobals())

describe("task transition exact contracts", () => {
  for (const action of actions) it(`consumes committed ${action} response fixture`, async () => {
    vi.stubGlobal("fetch", vi.fn(async () => response(fixtures[action])))
    const result = await new KanbanApi(config, { locale: "zh-CN" }).transition(fixtures[action].data, action)
    expect(result.id).toBe("t_fixture")
  })

  for (const action of actions) {
    for (const [name, mutate] of [
      ["extra outer envelope", (value: any) => ({ ...value, meta: {} })],
      ["claim token leak", (value: any) => ({ data: { ...value.data, claim_token: "secret" } })],
      ["wrong data type", () => ({ data: [] })],
      ["missing required nullable", (value: any) => { const data = { ...value.data }; delete data.claim_owner; return { data } }],
    ] as const) it(`${action} rejects ${name}`, async () => {
      vi.stubGlobal("fetch", vi.fn(async () => response(mutate(structuredClone(fixtures[action])))))
      await expect(new KanbanApi(config).transition(fixtures[action].data, action)).rejects.toMatchObject({ code: "invalid_response" })
    })
  }

  it("preserves exact POST transport, locale, and actor body", async () => {
    const fetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => response(fixtures.promote))
    vi.stubGlobal("fetch", fetch)
    await new KanbanApi(config, { locale: "zh-CN" }).transition(fixtures.promote.data, "promote")
    const [url, init] = fetch.mock.calls[0]!
    expect(url).toBe("http://127.0.0.1:8721/api/v1/tasks/t_fixture/transitions/promote")
    expect(init).toMatchObject({ method: "POST", headers: { "Accept-Language": "zh-CN", "Content-Type": "application/json" } })
    expect(JSON.parse(init!.body as string)).toEqual({ actor: "desktop-test" })
  })
})

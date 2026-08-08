import { describe, expect, test, vi } from "vitest"

import validRuntime from "./api/generated/fixtures/runtime-web-config-output.valid.json"
import {
  loadWebRuntimeConfig,
  runtimeEndpointUrl,
} from "./runtime"

function response(body: string, status = 200) {
  return new Response(body, {
    status,
    headers: { "Content-Type": "application/json" },
  })
}

describe("Web runtime bootstrap", () => {
  test("derives same-origin runtime URL from the /app/ base on deep routes", () => {
    expect(runtimeEndpointUrl("https://kanban.test/app/boards/default/board", "/app/")).toBe(
      "https://kanban.test/app/runtime.json",
    )
    expect(runtimeEndpointUrl("https://kanban.test/app/", "/app/")).toBe(
      "https://kanban.test/app/runtime.json",
    )
  })

  test("rejects a cross-origin runtime base before issuing fetch", () => {
    expect(() => runtimeEndpointUrl("https://kanban.test/app/", "https://evil.test/app/")).toThrow(
      "必须通过当前页面的同源",
    )
  })

  test("loads and returns generated-contract validated runtime metadata", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => response(JSON.stringify(validRuntime)))

    await expect(
      loadWebRuntimeConfig({
        fetch: fetcher,
        documentBaseURI: "https://kanban.test/app/boards/default/board",
        webBasePath: "/app/",
      }),
    ).resolves.toEqual(validRuntime)
    expect(fetcher).toHaveBeenCalledWith(
      "https://kanban.test/app/runtime.json",
      expect.objectContaining({
        credentials: "same-origin",
        headers: { Accept: "application/json" },
      }),
    )
  })

  test("fails closed on a non-success response", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => response("service unavailable", 503))

    await expect(loadWebRuntimeConfig({ fetch: fetcher, documentBaseURI: "https://kanban.test/app/" })).rejects.toMatchObject({
      kind: "http",
      status: 503,
    })
  })

  test("fails closed when runtime response is not JSON", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => response("{"))

    await expect(loadWebRuntimeConfig({ fetch: fetcher, documentBaseURI: "https://kanban.test/app/" })).rejects.toMatchObject({
      kind: "invalid_json",
    })
  })

  test("fails closed when generated runtime schema drifts", async () => {
    const fetcher = vi.fn<typeof fetch>(async () =>
      response(JSON.stringify({ ...validRuntime, protocolVersion: undefined, unexpected: true })),
    )

    await expect(loadWebRuntimeConfig({ fetch: fetcher, documentBaseURI: "https://kanban.test/app/" })).rejects.toMatchObject({
      kind: "invalid_contract",
    })
  })

  test("does not provide a silent development fallback when fetch fails", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => {
      throw new TypeError("Failed to fetch")
    })

    await expect(loadWebRuntimeConfig({ fetch: fetcher, documentBaseURI: "https://kanban.test/app/" })).rejects.toMatchObject({
      kind: "network",
    })
    expect(fetcher).toHaveBeenCalledTimes(1)
  })
})

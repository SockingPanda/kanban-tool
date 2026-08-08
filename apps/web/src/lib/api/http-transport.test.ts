import { describe, expect, test, vi } from "vitest"

import type { WebRuntimeConfig } from "../runtime"
import { createHttpTransport, HttpTransportError } from "./http-transport"

const runtime = {
  apiBaseUrl: "/__kb_api__",
  webBasePath: "/app/",
  actor: "test-actor",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "sha256:test",
} satisfies WebRuntimeConfig

describe("same-origin Web HTTP transport", () => {
  test("applies a same-origin runtime API path prefix and credentials", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => new Response(JSON.stringify({ data: [] }), { status: 200 }))
    const transport = createHttpTransport(runtime, {
      fetcher,
      documentBaseURI: "https://kanban.test/app/boards/default/board",
    })

    await expect(transport.get("/api/v1/boards")).resolves.toEqual({ data: [] })
    expect(fetcher).toHaveBeenCalledWith(
      "https://kanban.test/__kb_api__/api/v1/boards",
      expect.objectContaining({ credentials: "same-origin", method: "GET" }),
    )
  })

  test("rejects absolute and scheme-relative paths before fetch", async () => {
    const fetcher = vi.fn<typeof fetch>()
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })

    await expect(transport.get("https://evil.test/api")).rejects.toMatchObject({ kind: "cross_origin" })
    await expect(transport.get("//evil.test/api")).rejects.toMatchObject({ kind: "cross_origin" })
    expect(fetcher).not.toHaveBeenCalled()
  })

  test("parses the generated API error contract for non-success responses", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => new Response(
      JSON.stringify({ error: { code: "not_found", message: "board not found" } }),
      { status: 404 },
    ))
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })

    await expect(transport.get("/api/v1/boards/missing/columns")).rejects.toMatchObject({
      name: "HttpTransportError",
      kind: "http",
      status: 404,
      apiError: { code: "not_found", message: "board not found" },
    } satisfies Partial<HttpTransportError>)
  })
})

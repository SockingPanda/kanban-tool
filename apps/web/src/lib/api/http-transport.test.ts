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

function sameOriginResponse(body: BodyInit | null, init: ResponseInit, url = "https://kanban.test/api/v1/boards"): Response {
  const response = new Response(body, init)
  Object.defineProperty(response, "url", { value: url })
  return response
}

describe("same-origin Web HTTP transport", () => {
  test("applies a same-origin runtime API path prefix and credentials", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => sameOriginResponse(JSON.stringify({ data: [] }), {
      status: 200,
      headers: { "content-type": "application/json" },
    }))
    const transport = createHttpTransport(runtime, {
      fetcher,
      documentBaseURI: "https://kanban.test/app/boards/default/board",
    })

    await expect(transport.get("/api/v1/boards")).resolves.toMatchObject({
      payload: { data: [] },
      bytes: expect.any(Number),
    })
    expect(fetcher).toHaveBeenCalledWith(
      "https://kanban.test/__kb_api__/api/v1/boards",
      expect.objectContaining({ cache: "no-store", credentials: "same-origin", method: "GET", mode: "same-origin", redirect: "error" }),
    )
  })

  test("posts JSON through the same origin and returns the typed response body", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => sameOriginResponse(JSON.stringify({ data: { ok: true } }), {
      status: 200,
      headers: { "content-type": "application/json" },
    }, "https://kanban.test/api/v1/boards/b_1/label-ontology/actions"))
    const transport = createHttpTransport(runtime, {
      fetcher,
      documentBaseURI: "https://kanban.test/app/",
    })

    await expect(transport.post?.("/api/v1/boards/b_1/label-ontology/actions", { reason: "keep" })).resolves.toMatchObject({
      payload: { data: { ok: true } },
    })
    expect(fetcher).toHaveBeenCalledWith(
      "https://kanban.test/__kb_api__/api/v1/boards/b_1/label-ontology/actions",
      expect.objectContaining({
        body: JSON.stringify({ reason: "keep" }),
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        method: "POST",
      }),
    )
  })

  test("rejects absolute and scheme-relative paths before fetch", async () => {
    const fetcher = vi.fn<typeof fetch>()
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })

    await expect(transport.get("https://evil.test/api")).rejects.toMatchObject({ kind: "cross_origin" })
    await expect(transport.get("//evil.test/api")).rejects.toMatchObject({ kind: "cross_origin" })
    expect(fetcher).not.toHaveBeenCalled()
  })

  test("rejects traversal, encoded separators, backslashes, and NUL paths", async () => {
    const fetcher = vi.fn<typeof fetch>()
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })
    let nestedDot = ".."
    for (let index = 0; index < 8; index += 1) nestedDot = encodeURIComponent(nestedDot)

    for (const path of [
      "/api/./v1/boards",
      "/api/%2e%2e/v1/boards",
      "/api/%252e%252e/v1/boards",
      `/api/${nestedDot}/v1/boards`,
      "/api/%2f/v1/boards",
      "/api/%5c/v1/boards",
      "/api/%00/v1/boards",
    ]) {
      await expect(transport.get(path)).rejects.toMatchObject({ kind: "cross_origin" })
    }
    expect(fetcher).not.toHaveBeenCalled()
  })

  test("rejects malformed runtime URL and a final cross-origin response URL", async () => {
    const fetcher = vi.fn<typeof fetch>()
    expect(() => createHttpTransport({ ...runtime, apiBaseUrl: "http://[bad" }, { fetcher })).toThrow(
      expect.objectContaining({ kind: "malformed_url" }),
    )

    const response = sameOriginResponse(JSON.stringify({ data: [] }), {
      status: 200,
      headers: { "content-type": "application/json" },
    }, "https://evil.test/final")
    fetcher.mockResolvedValueOnce(response)
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })
    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ kind: "cross_origin" })
  })

  test("requires JSON content types and enforces a declared byte cap", async () => {
    const fetcher = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(sameOriginResponse("{}", { status: 200, headers: { "content-type": "text/plain" } }))
      .mockResolvedValueOnce(sameOriginResponse("{}", {
        status: 200,
        headers: { "content-type": "application/json", "content-length": String(16 * 1024 * 1024 + 1) },
      }))
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })
    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ kind: "invalid_content_type" })
    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ kind: "response_too_large" })
  })

  test("cancels a streamed response as soon as the byte cap is exceeded", async () => {
    let canceled = false
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new Uint8Array(16 * 1024 * 1024 + 1))
      },
      cancel() {
        canceled = true
      },
    })
    const fetcher = vi.fn<typeof fetch>(async () => sameOriginResponse(stream, {
      status: 200,
      headers: { "content-type": "application/json" },
    }))
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })
    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ kind: "response_too_large" })
    expect(canceled).toBe(true)
  })

  test("cancels early rejected bodies and preserves reader AbortError", async () => {
    let canceled = false
    const oversizedStream = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new Uint8Array([123]))
      },
      cancel() {
        canceled = true
      },
    })
    const fetcher = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(sameOriginResponse(oversizedStream, {
        status: 200,
        headers: { "content-type": "application/json", "content-length": String(16 * 1024 * 1024 + 1) },
      }))
      .mockResolvedValueOnce(sameOriginResponse("{}", { status: 200, headers: { "content-type": "text/plain" } }))
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })
    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ kind: "response_too_large" })
    expect(canceled).toBe(true)
    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ kind: "invalid_content_type" })

    let pulls = 0
    const abortedStream = new ReadableStream<Uint8Array>({
      pull(controller) {
        pulls += 1
        if (pulls === 1) controller.enqueue(new Uint8Array([123]))
        else controller.error(new DOMException("aborted", "AbortError"))
      },
    })
    fetcher.mockResolvedValueOnce(sameOriginResponse(abortedStream, {
      status: 200,
      headers: { "content-type": "application/json" },
    }))
    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ name: "AbortError" })
  })

  test("parses the generated API error contract for non-success responses", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => sameOriginResponse(
      JSON.stringify({ error: { code: "not_found", message: "board not found" } }),
      { status: 404, headers: { "content-type": "application/json" } },
    ))
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })

    await expect(transport.get("/api/v1/boards/missing/columns")).rejects.toMatchObject({
      name: "HttpTransportError",
      kind: "http",
      status: 404,
      apiError: { code: "not_found", message: "board not found" },
    } satisfies Partial<HttpTransportError>)
  })

  test("rejects an empty final response URL and cancels its body", async () => {
    let canceled = false
    const body = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new Uint8Array([123]))
      },
      cancel() {
        canceled = true
      },
    })
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(new Response(body, {
      status: 200,
      headers: { "content-type": "application/json" },
    }))
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })

    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ kind: "cross_origin" })
    expect(canceled).toBe(true)
  })
})

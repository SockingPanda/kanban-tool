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
    const fetcher = vi.fn<typeof fetch>(async () => new Response(JSON.stringify({ data: [] }), {
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
      expect.objectContaining({ credentials: "same-origin", method: "GET", mode: "same-origin", redirect: "error" }),
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

    const response = new Response(JSON.stringify({ data: [] }), {
      status: 200,
      headers: { "content-type": "application/json" },
    })
    Object.defineProperty(response, "url", { value: "https://evil.test/final" })
    fetcher.mockResolvedValueOnce(response)
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })
    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ kind: "cross_origin" })
  })

  test("requires JSON content types and enforces a declared byte cap", async () => {
    const fetcher = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(new Response("{}", { status: 200, headers: { "content-type": "text/plain" } }))
      .mockResolvedValueOnce(new Response("{}", {
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
    const fetcher = vi.fn<typeof fetch>(async () => new Response(stream, {
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
      .mockResolvedValueOnce(new Response(oversizedStream, {
        status: 200,
        headers: { "content-type": "application/json", "content-length": String(16 * 1024 * 1024 + 1) },
      }))
      .mockResolvedValueOnce(new Response("{}", { status: 200, headers: { "content-type": "text/plain" } }))
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
    fetcher.mockResolvedValueOnce(new Response(abortedStream, {
      status: 200,
      headers: { "content-type": "application/json" },
    }))
    await expect(transport.get("/api/v1/boards")).rejects.toMatchObject({ name: "AbortError" })
  })

  test("parses the generated API error contract for non-success responses", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => new Response(
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

  test("sends raw JSON mutation requests without a generic response cast", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => new Response(JSON.stringify({ data: { ok: true } }), {
      status: 200,
      headers: { "content-type": "application/json" },
    }))
    const transport = createHttpTransport(runtime, {
      fetcher,
      documentBaseURI: "https://kanban.test/app/",
    })

    await expect(transport.request({ method: "POST", path: "/api/v1/tasks/t_1/transitions/block", body: { actor: "test-actor", reason: "blocked" } }))
      .resolves.toMatchObject({ payload: { data: { ok: true } }, bytes: expect.any(Number) })
    expect(fetcher).toHaveBeenCalledWith(
      "https://kanban.test/__kb_api__/api/v1/tasks/t_1/transitions/block",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ actor: "test-actor", reason: "blocked" }),
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        credentials: "same-origin",
        mode: "same-origin",
        redirect: "error",
      }),
    )
  })

  test("does not add a request Content-Type for bodyless DELETE", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => new Response(JSON.stringify({ data: [] }), {
      status: 200,
      headers: { "content-type": "application/json" },
    }))
    const transport = createHttpTransport(runtime, {
      fetcher,
      documentBaseURI: "https://kanban.test/app/",
    })

    await transport.request({ method: "DELETE", path: "/api/v1/tasks/t_1/dependencies/t_2" })
    expect(fetcher.mock.calls[0]?.[1]).toEqual(expect.objectContaining({
      method: "DELETE",
      headers: { Accept: "application/json" },
    }))
  })

  test("forwards generated actor headers while keeping Accept transport-owned", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => new Response(JSON.stringify({ data: { deleted: true } }), {
      status: 200,
      headers: { "content-type": "application/json" },
    }))
    const transport = createHttpTransport(runtime, {
      fetcher,
      documentBaseURI: "https://kanban.test/app/",
    })

    await transport.request({
      method: "DELETE",
      path: "/api/v1/tasks/t_1/attachments/a_1",
      headers: { "X-KB-Actor": "test-actor" },
    })

    expect(fetcher).toHaveBeenCalledWith(
      "https://kanban.test/__kb_api__/api/v1/tasks/t_1/attachments/a_1",
      expect.objectContaining({
        headers: { Accept: "application/json", "X-KB-Actor": "test-actor" },
      }),
    )
  })

  test("rejects unknown, duplicate, and invalid JSON headers before fetch", async () => {
    const fetcher = vi.fn<typeof fetch>()
    const transport = createHttpTransport(runtime, {
      fetcher,
      documentBaseURI: "https://kanban.test/app/",
    })

    await expect(transport.request({
      method: "POST",
      path: "/api/v1/tasks/t_1",
      body: { title: "new" },
      headers: { Accept: "application/json" },
    })).rejects.toMatchObject({ kind: "invalid_headers" })
    await expect(transport.request({
      method: "POST",
      path: "/api/v1/tasks/t_1",
      body: { title: "new" },
      headers: { "Content-Type": "application/json", "content-type": "application/json" },
    })).rejects.toMatchObject({ kind: "invalid_headers" })
    await expect(transport.request({
      method: "POST",
      path: "/api/v1/tasks/t_1",
      body: { title: "new" },
      headers: { "Content-Type": "text/plain" },
    })).rejects.toMatchObject({ kind: "invalid_headers" })
    await expect(transport.request({
      method: "DELETE",
      path: "/api/v1/tasks/t_1",
      headers: { "Content-Type": "application/json" },
    })).rejects.toMatchObject({ kind: "invalid_headers" })
    expect(fetcher).not.toHaveBeenCalled()
  })

  test("raw mutation requests preserve generated API errors", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => new Response(
      JSON.stringify({ error: { code: "invalid_transition", message: "task cannot be blocked" } }),
      { status: 409, headers: { "content-type": "application/json" } },
    ))
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })

    await expect(transport.request({ method: "POST", path: "/api/v1/tasks/t_1/transitions/block", body: { reason: "no" } })).rejects.toMatchObject({
      kind: "http",
      status: 409,
      apiError: { code: "invalid_transition", message: "task cannot be blocked" },
    })
  })

  test("rejects circular request bodies before issuing a fetch", async () => {
    const fetcher = vi.fn<typeof fetch>()
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })
    const body: Record<string, unknown> = {}
    body.self = body

    await expect(transport.request({ method: "POST", path: "/api/v1/tasks", body })).rejects.toMatchObject({ kind: "invalid_json" })
    expect(fetcher).not.toHaveBeenCalled()
  })

  test("omits Content-Type when a method has no JSON body and preserves AbortError", async () => {
    const fetcher = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(new Response(JSON.stringify({ data: [] }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }))
      .mockRejectedValueOnce(new DOMException("aborted", "AbortError"))
    const transport = createHttpTransport(runtime, { fetcher, documentBaseURI: "https://kanban.test/app/" })

    await transport.request({ method: "POST", path: "/api/v1/tasks", body: undefined })
    expect(fetcher.mock.calls[0]?.[1]).toEqual(expect.objectContaining({
      headers: { Accept: "application/json" },
    }))
    await expect(transport.request({ method: "POST", path: "/api/v1/tasks" })).rejects.toMatchObject({ name: "AbortError" })
  })
})

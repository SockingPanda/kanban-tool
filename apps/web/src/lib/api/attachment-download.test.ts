import { describe, expect, test, vi } from "vitest"

import type { WebRuntimeConfig } from "../runtime"
import type { HttpTransport } from "./http-transport"
import { createAttachmentDownloadClient } from "./attachment-download"

const runtime = {
  apiBaseUrl: "/__kb_api__",
  webBasePath: "/app/",
  actor: "web-user",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "sha256:test",
} satisfies WebRuntimeConfig

describe("attachment download operation", () => {
  test("validates generated path and headers before requesting bytes", async () => {
    const requestBytes = vi.fn<HttpTransport["requestBytes"]>(async () => ({
      bytes: new Uint8Array([104, 105]),
      contentType: "text/plain",
      attachmentId: "a_%",
      sha256: "sha256-fixture",
    }))
    const client = createAttachmentDownloadClient(runtime, { transport: { requestBytes } })

    await expect(client.downloadAttachment("t_1", "a_%")).resolves.toEqual({
      content_type: "text/plain",
      attachment_id: "a_%",
      sha256: "sha256-fixture",
      content: new Uint8Array([104, 105]),
    })
    expect(requestBytes).toHaveBeenCalledWith({
      method: "GET",
      path: "/api/v1/tasks/t_1/attachments/a_%25",
      headers: {},
    })
  })

  test("downloads through the production transport and preserves a literal percent id", async () => {
    const response = new Response(new Uint8Array([104, 105]), {
      status: 200,
      headers: {
        "content-type": "text/plain",
        "content-length": "2",
        "x-kb-attachment-id": "a_%",
        "x-kb-attachment-sha256": "sha256-fixture",
      },
    })
    Object.defineProperty(response, "url", { value: "https://kanban.test/__kb_api__/api/v1/tasks/t_1/attachments/a_%25" })
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(response)
    const client = createAttachmentDownloadClient(runtime, {
      fetcher,
      documentBaseURI: "https://kanban.test/app/",
    })

    await expect(client.downloadAttachment("t_1", "a_%")).resolves.toEqual({
      content_type: "text/plain",
      attachment_id: "a_%",
      sha256: "sha256-fixture",
      content: new Uint8Array([104, 105]),
    })
    expect(fetcher).toHaveBeenCalledWith(
      "https://kanban.test/__kb_api__/api/v1/tasks/t_1/attachments/a_%25",
      expect.objectContaining({
        method: "GET",
        headers: { Accept: "application/octet-stream" },
        credentials: "same-origin",
        mode: "same-origin",
        redirect: "error",
        cache: "no-store",
      }),
    )
  })
})

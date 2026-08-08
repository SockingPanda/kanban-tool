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
      attachmentId: "a_1",
      sha256: "sha256-fixture",
    }))
    const client = createAttachmentDownloadClient(runtime, { transport: { requestBytes } })

    await expect(client.downloadAttachment("t_1", "a_1/part")).resolves.toEqual({
      content_type: "text/plain",
      attachment_id: "a_1",
      sha256: "sha256-fixture",
      content: new Uint8Array([104, 105]),
    })
    expect(requestBytes).toHaveBeenCalledWith({
      method: "GET",
      path: "/api/v1/tasks/t_1/attachments/a_1%2Fpart",
      headers: {},
    })
  })
})

import { afterEach, describe, expect, it, vi } from "vitest"

import { KanbanApi } from "./api"

const config = { apiBaseUrl: "http://127.0.0.1:8721", actor: "desktop-test", board: "default" }
const attachment = {
  id: "a_fixture",
  board_id: "b_default",
  task_id: "t_fixture",
  filename: "hello.txt",
  rel_path: "attachments/b_default/t_fixture/hello.txt",
  content_type: "text/plain",
  size_bytes: 2,
  sha256: "sha256-fixture",
  created_by: "desktop-test",
  created_at: 1,
}

function jsonResponse(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), { status, headers: { "Content-Type": "application/json" } })
}

afterEach(() => vi.unstubAllGlobals())

describe("attachments exact contracts", () => {
  it("lists metadata through the task-scoped endpoint", async () => {
    const fetch = vi.fn(async () => jsonResponse({ data: [attachment] }))
    vi.stubGlobal("fetch", fetch)

    await expect(new KanbanApi(config, { locale: "zh-CN" }).listAttachments("t_fixture")).resolves.toEqual([attachment])
    expect(fetch).toHaveBeenCalledWith(
      "http://127.0.0.1:8721/api/v1/tasks/t_fixture/attachments",
      expect.objectContaining({ method: "GET", headers: { "Accept-Language": "zh-CN" } }),
    )
  })

  it("creates metadata with JSON bytes and the actor header", async () => {
    const fetch = vi.fn(async () => jsonResponse({ data: attachment }, 201))
    vi.stubGlobal("fetch", fetch)
    const api = new KanbanApi(config, { locale: "zh-CN" })

    await expect(
      api.createAttachment("t_fixture", {
        id: "a_fixture",
        filename: "hello.txt",
        content: [104, 105],
        content_type: "text/plain",
      }),
    ).resolves.toEqual(attachment)

    const [url, init] = fetch.mock.calls[0] as unknown as [RequestInfo | URL, RequestInit]
    expect(url).toBe("http://127.0.0.1:8721/api/v1/tasks/t_fixture/attachments")
    expect(init).toMatchObject({
      method: "POST",
      headers: {
        "Accept-Language": "zh-CN",
        "Content-Type": "application/json",
        "X-KB-Actor": "desktop-test",
      },
    })
    expect(JSON.parse(init.body as string)).toEqual({
      id: "a_fixture",
      filename: "hello.txt",
      content: [104, 105],
      content_type: "text/plain",
    })
  })

  it("downloads raw bytes and preserves attachment response headers", async () => {
    const fetch = vi.fn(async () =>
      new Response(new Uint8Array([104, 105]), {
        status: 200,
        headers: {
          "Content-Type": "text/plain",
          "X-KB-Attachment-ID": "a_fixture",
          "X-KB-Attachment-SHA256": "sha256-fixture",
        },
      }),
    )
    vi.stubGlobal("fetch", fetch)

    await expect(new KanbanApi(config).downloadAttachment("t_fixture", "a_fixture")).resolves.toMatchObject({
      content_type: "text/plain",
      attachment_id: "a_fixture",
      sha256: "sha256-fixture",
      content: new Uint8Array([104, 105]),
    })
    expect(fetch).toHaveBeenCalledWith(
      "http://127.0.0.1:8721/api/v1/tasks/t_fixture/attachments/a_fixture",
      expect.objectContaining({ method: "GET", headers: { "Accept-Language": "en" } }),
    )
  })

  it("deletes by task and attachment id with the actor header", async () => {
    const fetch = vi.fn(async () => jsonResponse({ data: { deleted: true } }))
    vi.stubGlobal("fetch", fetch)

    await expect(new KanbanApi(config).deleteAttachment("t_fixture", "a_fixture")).resolves.toBe(true)
    expect(fetch).toHaveBeenCalledWith(
      "http://127.0.0.1:8721/api/v1/tasks/t_fixture/attachments/a_fixture",
      expect.objectContaining({
        method: "DELETE",
        headers: { "Accept-Language": "en", "X-KB-Actor": "desktop-test" },
      }),
    )
  })

  it("rejects unknown metadata fields instead of widening the wire type", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => jsonResponse({ data: { ...attachment, unexpected: true } }, 200)))

    await expect(new KanbanApi(config).listAttachments("t_fixture")).rejects.toMatchObject({ code: "invalid_response" })
  })
})

import { describe, expect, test, vi } from "vitest"

import type { HttpTransportResponse } from "./http-transport"
import { HttpTransportError } from "./http-transport"
import { readHealth } from "./health-read-model"

function response(payload: unknown): HttpTransportResponse {
  return { payload, bytes: JSON.stringify(payload).length }
}

describe("health read model", () => {
  test("loads and projects the generated health report", async () => {
    const get = vi.fn(async (path: string) => {
      expect(path).toBe("/health")
      return response({
        data: {
          ok: true,
          db: "ok",
          version: "3.0.0",
          db_path: "/tmp/kanban.db",
          db_fingerprint: "sha256:test",
        },
      })
    })

    await expect(readHealth({ transport: { get } })).resolves.toEqual({
      ok: true,
      db: "ok",
      version: "3.0.0",
      db_path: "/tmp/kanban.db",
      db_fingerprint: "sha256:test",
    })
  })

  test("rejects schema drift with a typed local error", async () => {
    await expect(readHealth({ transport: { get: vi.fn(async () => response({ data: { ok: true } })) } })).rejects.toMatchObject({
      kind: "invalid_contract",
      contractId: "api.health.response",
    })
  })

  test("keeps typed API code and status without exposing the server message", async () => {
    const serverMessage = "database path /srv/private/kanban.db is unavailable"
    const transport = {
      get: vi.fn(async () => {
        throw new HttpTransportError("http", serverMessage, {
          status: 503,
          apiError: { code: "server_unavailable", message: serverMessage },
        })
      }),
    }

    const error = await readHealth({ transport }).catch((cause: unknown) => cause)
    expect(error).toMatchObject({
      kind: "http",
      status: 503,
      code: "server_unavailable",
      apiErrorCode: "server_unavailable",
    })
    expect(error).not.toMatchObject({ message: expect.stringContaining("/srv/private") })
  })
})

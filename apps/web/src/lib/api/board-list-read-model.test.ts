import { describe, expect, test } from "vitest"

import {
  BoardListReadError,
  createBoardListQuery,
  loadBoardList,
} from "./board-list-read-model"
import type { HttpTransportResponse } from "./http-transport"
import type { WebRuntimeConfig } from "../runtime"

const runtime: WebRuntimeConfig = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "test",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "test",
}

function board(overrides: Record<string, unknown> = {}) {
  return {
    id: "b_default",
    slug: "default",
    name: "Default Board",
    description: null,
    created_at: 1,
    updated_at: 2,
    archived_at: null,
    ...overrides,
  }
}

function transport(payload: unknown): { get(path: string, signal?: AbortSignal): Promise<HttpTransportResponse> } {
  return {
    async get(path) {
      expect(path).toBe("/api/v1/boards?include_archived=false")
      return { payload, bytes: 1 }
    },
  }
}

describe("global board list read model", () => {
  test("loads and freezes the validated board identity list through the generated path", async () => {
    const result = await loadBoardList(runtime, {
      transport: transport({ data: [board()] }),
    })

    expect(result).toEqual([{
      id: "b_default",
      slug: "default",
      name: "Default Board",
      description: null,
      archivedAt: null,
    }])
    expect(Object.isFrozen(result)).toBe(true)
    expect(Object.isFrozen(result[0])).toBe(true)
  })

  test.each([
    ["duplicate id", [board(), board({ slug: "other" })]],
    ["duplicate slug", [board(), board({ id: "b_other" })]],
    ["cross-type selector collision", [board(), board({ id: "b_other", slug: "b_default" })]],
    ["unsafe id path", [board({ id: "b_../x" })]],
    ["unsafe id query", [board({ id: "b_?" })]],
    ["unsafe id fragment", [board({ id: "b_#" })]],
    ["unsafe id backslash", [board({ id: "b_\\x" })]],
    ["invalid slug", [board({ slug: "../unsafe" })]],
    ["blank name", [board({ name: "   " })]],
  ])("fails closed for %s", async (_label, boards) => {
    await expect(loadBoardList(runtime, {
      transport: transport({ data: boards }),
    })).rejects.toMatchObject({ name: "BoardListReadError", kind: "anomaly" })
  })

  test("preserves a typed offline boundary", async () => {
    const expected = new BoardListReadError("offline", "offline")
    const failingTransport = {
      get: async () => { throw expected },
    }

    await expect(loadBoardList(runtime, { transport: failingTransport })).rejects.toBe(expected)
  })

  test("does not let a stale generation repopulate a reloaded query", async () => {
    let resolveFirst!: (value: HttpTransportResponse) => void
    let calls = 0
    const query = createBoardListQuery(runtime, {
      transport: {
        get: async () => {
          calls += 1
          if (calls === 1) return new Promise<HttpTransportResponse>((resolve) => { resolveFirst = resolve })
          return { payload: { data: [board({ id: "b_new", slug: "new", name: "New Board" })] }, bytes: 1 }
        },
      },
    })

    const first = query.load()
    const second = query.reload()
    resolveFirst({ payload: { data: [board()] }, bytes: 1 })

    await expect(first).rejects.toMatchObject({ name: "AbortError" })
    await expect(second).resolves.toEqual([expect.objectContaining({ slug: "new" })])
    await expect(query.load()).resolves.toEqual([expect.objectContaining({ slug: "new" })])
  })
})

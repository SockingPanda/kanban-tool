import { afterEach, describe, expect, test, vi } from "vitest"

import type { BoardReadModel, BoardReadQuery } from "../../lib/api/board-read-model"
import { asCanonicalBoardId, type StreamContractAdapter } from "../../lib/sync"
import type { WebRuntimeConfig } from "../../lib/runtime"
import type { BoardViewModel } from "./types"
import {
  acquireBoardSession,
  activeBoardSessionCount,
  resetBoardSessionsForTests,
  type BoardReadResource,
} from "./board-session-registry"

const runtime: WebRuntimeConfig = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "test",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "test",
}

const model: BoardViewModel = {
  board: { id: "b_default", slug: "default", name: "Default" },
  columns: [],
  tasksByStatus: {},
}

const readModel = {
  identity: {
    selector: "default",
    canonicalBoardId: asCanonicalBoardId("b_default"),
    slug: "default",
    name: "Default",
  },
  columns: [],
  tasksByStatus: {},
} satisfies BoardReadModel

function resource(query: BoardReadQuery): BoardReadResource {
  const adapter = {
    parseEnvelope: () => ({ status: "invalid", code: "test" }),
    parsePollingEnvelope: () => ({ status: "invalid", code: "test" }),
    validateBusiness: () => ({ status: "invalid", code: "test" }),
    isControlFrame: () => false,
    validateControl: () => ({ status: "invalid", code: "test" }),
  } satisfies StreamContractAdapter
  return {
    selector: "default",
    transport: { get: vi.fn() },
    query,
    adapter,
  }
}

describe("Board canonical session registry", () => {
  afterEach(() => resetBoardSessionsForTests())

  test("shares one controller per runtime and canonical board, then stops on last release", () => {
    const query = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const start = vi.fn()
    const stop = vi.fn()
    const retry = vi.fn()
    const createController = vi.fn(() => ({ start, stop, retry }))
    const listener = vi.fn()
    const telemetry = vi.fn()

    const first = acquireBoardSession(runtime, model, resource(query), listener, telemetry, { createController })
    const second = acquireBoardSession(runtime, model, resource(query), listener, telemetry, { createController })

    expect(createController).toHaveBeenCalledTimes(1)
    expect(activeBoardSessionCount()).toBe(1)
    expect(start).toHaveBeenCalledTimes(2)

    first.release()
    expect(activeBoardSessionCount()).toBe(1)
    expect(stop).not.toHaveBeenCalled()
    expect(listener).not.toHaveBeenCalled()
    first.retry()
    expect(retry).not.toHaveBeenCalled()
    second.retry()
    expect(retry).toHaveBeenCalledTimes(1)
    second.release()
    expect(activeBoardSessionCount()).toBe(0)
    expect(stop).toHaveBeenCalledTimes(1)
    expect(query.invalidate).toHaveBeenCalledTimes(1)
  })

  test("does not leak a session across runtime/build identity changes", () => {
    const query = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const createController = vi.fn(() => ({ start: vi.fn(), stop: vi.fn(), retry: vi.fn() }))
    const otherRuntime = { ...runtime, webBuildId: "other" }

    const first = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), { createController })
    const second = acquireBoardSession(otherRuntime, model, resource(query), vi.fn(), vi.fn(), { createController })

    expect(activeBoardSessionCount()).toBe(2)
    expect(createController).toHaveBeenCalledTimes(2)
    first.release()
    second.release()
    expect(activeBoardSessionCount()).toBe(0)
  })
})

import { afterEach, describe, expect, test, vi } from "vitest"

import type { BoardReadModel, BoardReadQuery } from "../../lib/api/board-read-model"
import { asCanonicalBoardId, type StreamContractAdapter } from "../../lib/sync"
import type { WebRuntimeConfig } from "../../lib/runtime"
import type { BoardViewModel } from "./types"
import {
  acquireBoardSession,
  activeBoardSessionCount,
  resourceIdentityKey,
  resetBoardSessionsForTests,
  runtimeIdentityKey,
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

function resource(
  query: BoardReadQuery,
  resourceRuntime: WebRuntimeConfig = runtime,
  selector = "default",
  boardId = asCanonicalBoardId("b_default"),
): BoardReadResource {
  const adapter = {
    parseEnvelope: () => ({ status: "invalid", code: "test" }),
    parsePollingEnvelope: () => ({ status: "invalid", code: "test" }),
    validateBusiness: () => ({ status: "invalid", code: "test" }),
    isControlFrame: () => false,
    validateControl: () => ({ status: "invalid", code: "test" }),
  } satisfies StreamContractAdapter
  return {
    selector,
    transport: { get: vi.fn() },
    query,
    adapter,
    runtimeKey: runtimeIdentityKey(resourceRuntime),
    identityKey: resourceIdentityKey(resourceRuntime, selector, boardId),
    canonicalBoardId: boardId,
    resolvedSlug: "default",
    sessionGeneration: 0,
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
    const second = acquireBoardSession(otherRuntime, model, resource(query, otherRuntime), vi.fn(), vi.fn(), { createController })

    expect(activeBoardSessionCount()).toBe(2)
    expect(createController).toHaveBeenCalledTimes(2)
    first.release()
    second.release()
    expect(activeBoardSessionCount()).toBe(0)
  })

  test("keeps a registry query alive until the last release across distinct resources", () => {
    const queryOne = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const queryTwo = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const stop = vi.fn()
    const createController = vi.fn(() => ({ start: vi.fn(), stop, retry: vi.fn() }))

    const first = acquireBoardSession(runtime, model, resource(queryOne), vi.fn(), vi.fn(), { createController })
    const second = acquireBoardSession(runtime, model, resource(queryTwo), vi.fn(), vi.fn(), { createController })

    first.release()
    expect(stop).not.toHaveBeenCalled()
    expect(queryOne.invalidate).not.toHaveBeenCalled()
    expect(queryTwo.invalidate).not.toHaveBeenCalled()

    second.release()
    expect(stop).toHaveBeenCalledTimes(1)
    expect(queryOne.invalidate).toHaveBeenCalledTimes(1)
    expect(queryTwo.invalidate).not.toHaveBeenCalled()
  })

  test("old reset handles cannot release or retry a replacement same-key session", () => {
    const query = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const firstStop = vi.fn()
    const secondStop = vi.fn()
    const firstRetry = vi.fn()
    const secondRetry = vi.fn()
    const createController = vi.fn()
      .mockImplementationOnce(() => ({ start: vi.fn(), stop: firstStop, retry: firstRetry }))
      .mockImplementationOnce(() => ({ start: vi.fn(), stop: secondStop, retry: secondRetry }))

    const oldHandle = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), { createController })
    resetBoardSessionsForTests()
    const newHandle = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), { createController })

    oldHandle.release()
    oldHandle.retry()
    expect(secondStop).not.toHaveBeenCalled()
    expect(secondRetry).not.toHaveBeenCalled()
    expect(activeBoardSessionCount()).toBe(1)

    newHandle.release()
    expect(secondStop).toHaveBeenCalledTimes(1)
  })

  test("builds a same-origin stream URL with the runtime API prefix", () => {
    const query = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const createController = vi.fn((options: ConstructorParameters<typeof import("../../lib/sync").WebSyncController>[0]) => ({
      start: vi.fn(),
      stop: vi.fn(),
      retry: vi.fn(),
      options,
    }))

    const prefixedRuntime = { ...runtime, apiBaseUrl: "/gateway" }
    const handle = acquireBoardSession(
      prefixedRuntime,
      model,
      resource(query, prefixedRuntime),
      vi.fn(),
      vi.fn(),
      { createController },
    )

    expect(createController).toHaveBeenCalledWith(expect.objectContaining({ streamUrl: "/gateway/api/v1/stream/events" }))
    handle.release()

    const unprefixedHandle = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), { createController })
    expect(createController).toHaveBeenLastCalledWith(expect.objectContaining({ streamUrl: "/api/v1/stream/events" }))
    unprefixedHandle.release()
  })
})

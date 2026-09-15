import { createHostDataSource } from "../../adapters/host/data-source";
import { afterEach, describe, expect, test, vi } from "vitest"

import type { BoardReadModel, BoardReadQuery } from "../data/board-read-model";
import { asCanonicalBoardId, type StreamContractAdapter, type SyncTelemetryEntry } from "../sync/contracts";
import type { WebRuntimeConfig } from "../../lib/runtime"
import type { BoardViewModel } from "../../domain/tasks/board"
import type { BoardRealtimeContext, BoardRealtimeSource, RealtimeState } from "../realtime/source"
import {
  acquireBoardSession,
  activeBoardSessionCount,
  hasActiveBoardSession,
  reconnectActiveBoardSession,
  resourceIdentityKey,
  resetBoardSessionsForTests,
  runtimeIdentityKey,
  subscribeBoardSessionTelemetry,
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
    streamUrl: `${resourceRuntime.apiBaseUrl}/api/v1/stream/events`,
    transport: { call: vi.fn() },
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

  test("production source 仅配置正式 RPC，runtime prefix 进入同一个 endpoint", () => {
    const source = createHostDataSource({ ...runtime, apiBaseUrl: "/gateway" }, { documentBaseURI: "http://127.0.0.1:1421/app/" })
    expect(source.boardRealtime?.key).toBe("grpc-web:http://127.0.0.1:1421/gateway/:kanban.v1.WorkspaceService:1")
    expect(source.transport).toHaveProperty("call")
    expect(source.streamTransport).toBeUndefined()
    expect(source.streamUrl).toBeUndefined()
  })

  test("读取中的多次 RPC 失效合并为后续重读，旧快照不发布", async () => {
    const resolvers: Array<(value: BoardReadModel) => void> = []
    const query = { load: vi.fn(async () => readModel), reload: vi.fn(() => new Promise<BoardReadModel>(resolve => resolvers.push(resolve))), invalidate: vi.fn() }
    let context: BoardRealtimeContext | undefined
    const source: BoardRealtimeSource = { key: "rpc", create: value => {
      context = value
      return { start: vi.fn(), stop: vi.fn(), retry: vi.fn(), snapshot: () => ({ state: "live" }) }
    } }
    const publish = vi.fn()
    const handle = acquireBoardSession(runtime, model, { ...resource(query), boardRealtime: source }, publish, vi.fn())
    context?.onRefresh()
    context?.onRefresh()
    context?.onRefresh()
    expect(query.reload).toHaveBeenCalledOnce()
    resolvers[0]?.(readModel)
    await vi.waitFor(() => expect(query.reload).toHaveBeenCalledTimes(2))
    expect(publish).not.toHaveBeenCalled()
    const fresh = { ...readModel, identity: { ...readModel.identity, name: "更新后" } }
    const pending = handle.refresh()
    resolvers[1]?.(fresh)
    await pending
    expect(publish).toHaveBeenCalledExactlyOnceWith(fresh)
    handle.release()
  })

  test("RPC 失效读取释放后不触碰重新获取的会话或另一看板", async () => {
    const contexts: BoardRealtimeContext[] = []
    const source: BoardRealtimeSource = { key: "rpc", create: value => {
      contexts.push(value)
      return { start: vi.fn(), stop: vi.fn(), retry: vi.fn(), snapshot: () => ({ state: "live" }) }
    } }
    let resolveFirst: ((value: BoardReadModel) => void) | undefined
    const firstQuery = { load: vi.fn(async () => readModel), reload: vi.fn(() => new Promise<BoardReadModel>(resolve => { resolveFirst = resolve })), invalidate: vi.fn() }
    const freshQuery = { load: vi.fn(async () => readModel), reload: vi.fn(async () => readModel), invalidate: vi.fn() }
    const firstPublish = vi.fn(), freshPublish = vi.fn(), otherPublish = vi.fn()
    const first = acquireBoardSession(runtime, model, { ...resource(firstQuery), boardRealtime: source }, firstPublish, vi.fn())
    contexts[0]?.onRefresh()
    first.release()
    const fresh = acquireBoardSession(runtime, model, { ...resource(freshQuery), boardRealtime: source }, freshPublish, vi.fn())
    const otherModel = { ...model, board: { id: "b_other", slug: "other", name: "Other" } }
    const other = acquireBoardSession(runtime, otherModel, { ...resource(freshQuery, runtime, "other", asCanonicalBoardId("b_other")), resolvedSlug: "other", boardRealtime: source }, otherPublish, vi.fn())
    contexts[0]?.onRefresh()
    resolveFirst?.(readModel)
    await new Promise(resolve => setImmediate(resolve))
    expect(firstPublish).not.toHaveBeenCalled()
    expect(freshPublish).not.toHaveBeenCalled()
    expect(otherPublish).not.toHaveBeenCalled()
    expect(freshQuery.reload).not.toHaveBeenCalled()
    contexts[1]?.onRefresh()
    await vi.waitFor(() => expect(freshPublish).toHaveBeenCalledOnce())
    expect(otherPublish).not.toHaveBeenCalled()
    fresh.release(); other.release()
  })

  test("显式 RPC source 共享一个 canonical 会话，并在最后一个引用释放时停止", () => {
    const query = {
      load: vi.fn(async () => readModel), reload: vi.fn(async () => readModel), invalidate: vi.fn(),
    } satisfies BoardReadQuery
    let context: BoardRealtimeContext | undefined
    let state: RealtimeState = "stopped"
    const start = vi.fn(() => { state = "connecting" })
    const stop = vi.fn(() => { state = "stopped" })
    const retry = vi.fn(() => { state = "connecting" })
    const create = vi.fn((value: BoardRealtimeContext) => {
      context = value
      return { start, stop, retry, snapshot: () => ({ state }) }
    })
    const source = { key: "grpc-generated", create } satisfies BoardRealtimeSource
    const legacy = vi.fn(() => { throw new Error("不应创建 SSE controller") })
    const firstTelemetry = vi.fn()
    const secondTelemetry = vi.fn()
    const first = acquireBoardSession(runtime, model, { ...resource(query), boardRealtime: source }, vi.fn(), firstTelemetry, { createController: legacy })
    const second = acquireBoardSession(runtime, model, { ...resource(query, runtime, "b_default"), boardRealtime: source }, vi.fn(), secondTelemetry, { createController: legacy })

    expect(create).toHaveBeenCalledOnce()
    expect(context?.boardId).toBe("b_default")
    expect(context?.boardSelector).toBe("default")
    expect(legacy).not.toHaveBeenCalled()
    context?.onRefresh()
    expect(firstTelemetry).toHaveBeenCalledWith(expect.objectContaining({ type: "rpc-refresh-required", cursor: 0 }))
    expect(secondTelemetry).toHaveBeenCalledOnce()
    expect(query.load).not.toHaveBeenCalled()
    expect(query.reload).toHaveBeenCalledOnce()
    first.release()
    context?.onRefresh()
    expect(firstTelemetry).toHaveBeenCalledOnce()
    expect(secondTelemetry).toHaveBeenCalledTimes(2)
    expect(stop).not.toHaveBeenCalled()
    second.release()
    context?.onRefresh()
    expect(secondTelemetry).toHaveBeenCalledTimes(2)
    expect(stop).toHaveBeenCalledOnce()
    expect(query.invalidate).toHaveBeenCalledOnce()
    expect(activeBoardSessionCount()).toBe(0)
  })

  test("显式 source 不能借用旧 SSE 会话，即使 source key 恰好叫 legacy-sse", () => {
    const query = {
      load: vi.fn(async () => readModel), reload: vi.fn(async () => readModel), invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const createController = vi.fn(() => ({ start: vi.fn(), stop: vi.fn(), retry: vi.fn() }))
    const first = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), { createController })
    const source: BoardRealtimeSource = { key: "legacy-sse", create: vi.fn(() => { throw new Error("不得复用") }) }
    expect(() => acquireBoardSession(runtime, model, { ...resource(query), boardRealtime: source }, vi.fn(), vi.fn()))
      .toThrow(/其他实时数据源/)
    expect(source.create).not.toHaveBeenCalled()
    expect(activeBoardSessionCount()).toBe(1)
    first.release()
  })

  test("RPC source 构造失败不回退 SSE，也不泄漏 canonical 会话", () => {
    const query = {
      load: vi.fn(async () => readModel), reload: vi.fn(async () => readModel), invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const source: BoardRealtimeSource = { key: "grpc-invalid", create: () => { throw new Error("RPC 配置无效") } }
    const createController = vi.fn(() => ({ start: vi.fn(), stop: vi.fn(), retry: vi.fn() }))
    expect(() => acquireBoardSession(runtime, model, { ...resource(query), boardRealtime: source }, vi.fn(), vi.fn(), { createController }))
      .toThrow("RPC 配置无效")
    expect(createController).not.toHaveBeenCalled()
    expect(activeBoardSessionCount()).toBe(0)
  })

  test("RPC 非 live 状态允许显式重连，live 状态保持原连接", () => {
    const query = {
      load: vi.fn(async () => readModel), reload: vi.fn(async () => readModel), invalidate: vi.fn(),
    } satisfies BoardReadQuery
    let state: RealtimeState = "connecting"
    const retry = vi.fn()
    const source: BoardRealtimeSource = { key: "grpc-reconnect", create: () => ({
      start: vi.fn(), stop: vi.fn(), retry, snapshot: () => ({ state }),
    }) }
    const handle = acquireBoardSession(runtime, model, { ...resource(query), boardRealtime: source }, vi.fn(), vi.fn())
    expect(reconnectActiveBoardSession(runtime, "default")).toBe("reconnecting")
    state = "live"
    expect(reconnectActiveBoardSession(runtime, "default")).toBe("already-live")
    state = "failed"
    expect(reconnectActiveBoardSession(runtime, "default")).toBe("reconnecting")
    expect(retry).toHaveBeenCalledTimes(2)
    handle.release()
  })

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

  test("reconnects the retained session without creating a second controller", () => {
    const query = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const retry = vi.fn()
    let state: "live" | "circuit-open" = "live"
    const createController = vi.fn(() => ({
      start: vi.fn(),
      stop: vi.fn(),
      retry,
      snapshot: () => ({ state }),
    }))
    const handle = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), { createController })
    expect(hasActiveBoardSession(runtime, "default")).toBe(true)
    expect(reconnectActiveBoardSession(runtime, "default")).toBe("already-live")
    expect(retry).not.toHaveBeenCalled()
    state = "circuit-open"
    expect(reconnectActiveBoardSession(runtime, "default")).toBe("reconnecting")
    expect(retry).toHaveBeenCalledTimes(1)
    expect(createController).toHaveBeenCalledTimes(1)
    handle.release()
    expect(hasActiveBoardSession(runtime, "default")).toBe(false)
    expect(reconnectActiveBoardSession(runtime, "default")).toBe("unavailable")
  })

  test("preserves a mounted telemetry observer across release/reacquire and drops it after unsubscribe", () => {
    const query = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const createController = vi.fn((options: ConstructorParameters<typeof import("../sync/index").WebSyncController>[0]) => ({
      start: vi.fn(),
      stop: vi.fn(),
      retry: vi.fn(),
      options,
    }))
    const observer = vi.fn()
    const unsubscribe = subscribeBoardSessionTelemetry(runtime, asCanonicalBoardId("b_default"), observer)
    const first = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), { createController })
    const firstRecord = createController.mock.calls[0]?.[0]?.telemetry?.record
    const entry = { type: "event", cursor: 1, details: {} } as SyncTelemetryEntry
    firstRecord?.(entry)
    expect(observer).toHaveBeenCalledTimes(1)

    first.release()
    const second = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), { createController })
    const secondRecord = createController.mock.calls[1]?.[0]?.telemetry?.record
    secondRecord?.({ ...entry, cursor: 2 })
    expect(observer).toHaveBeenCalledTimes(2)

    unsubscribe()
    second.release()
    const third = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), { createController })
    const thirdRecord = createController.mock.calls[2]?.[0]?.telemetry?.record
    thirdRecord?.({ ...entry, cursor: 3 })
    expect(observer).toHaveBeenCalledTimes(2)
    third.release()
  })

  test("refreshes the existing canonical query and publishes without creating another stream", async () => {
    const query = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const createController = vi.fn(() => ({ start: vi.fn(), stop: vi.fn(), retry: vi.fn() }))
    const listener = vi.fn()
    const handle = acquireBoardSession(runtime, model, resource(query), listener, vi.fn(), { createController })

    await handle.refresh()

    expect(query.reload).toHaveBeenCalledTimes(1)
    expect(listener).toHaveBeenCalledWith(readModel)
    expect(createController).toHaveBeenCalledTimes(1)
    handle.release()
  })

  test("coalesces concurrent refresh calls so mutations cannot abort one another", async () => {
    let resolveReload: (value: BoardReadModel) => void = () => undefined
    const reload = vi.fn(() => new Promise<BoardReadModel>((resolve) => { resolveReload = resolve }))
    const query = {
      load: vi.fn(async () => readModel),
      reload,
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const handle = acquireBoardSession(runtime, model, resource(query), vi.fn(), vi.fn(), {
      createController: vi.fn(() => ({ start: vi.fn(), stop: vi.fn(), retry: vi.fn() })),
    })
    const first = handle.refresh()
    const second = handle.refresh()
    expect(second).toBe(first)
    expect(reload).toHaveBeenCalledTimes(1)
    resolveReload(readModel)
    await Promise.all([first, second])
    handle.release()
  })

  test("publishes a refresh started by a released owner to a retained session owner", async () => {
    let resolveReload: (value: BoardReadModel) => void = () => undefined
    const reload = vi.fn(() => new Promise<BoardReadModel>((resolve) => { resolveReload = resolve }))
    const query = {
      load: vi.fn(async () => readModel),
      reload,
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const createController = vi.fn(() => ({ start: vi.fn(), stop: vi.fn(), retry: vi.fn() }))
    const firstListener = vi.fn()
    const secondListener = vi.fn()
    const first = acquireBoardSession(runtime, model, resource(query), firstListener, vi.fn(), { createController })
    const second = acquireBoardSession(runtime, model, resource(query), secondListener, vi.fn(), { createController })

    const refresh = first.refresh()
    first.release()
    expect(activeBoardSessionCount()).toBe(1)
    resolveReload(readModel)
    await refresh

    expect(firstListener).not.toHaveBeenCalled()
    expect(secondListener).toHaveBeenCalledWith(readModel)
    second.release()
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

  test("shares one canonical session across slug and canonical-id selectors", () => {
    const queryBySlug = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const queryById = {
      load: vi.fn(async () => ({ ...readModel, identity: { ...readModel.identity, selector: "b_default" } })),
      reload: vi.fn(async () => ({ ...readModel, identity: { ...readModel.identity, selector: "b_default" } })),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const start = vi.fn()
    const stop = vi.fn()
    const createController = vi.fn(() => ({ start, stop, retry: vi.fn() }))
    const idResource = resource(queryById, runtime, "b_default")
    const idModel = { ...model, board: { ...model.board, slug: "default" } }

    const slugHandle = acquireBoardSession(runtime, model, resource(queryBySlug, runtime, "default"), vi.fn(), vi.fn(), { createController })
    const idHandle = acquireBoardSession(runtime, idModel, idResource, vi.fn(), vi.fn(), { createController })

    expect(createController).toHaveBeenCalledTimes(1)
    expect(activeBoardSessionCount()).toBe(1)
    expect(start).toHaveBeenCalledTimes(2)

    slugHandle.release()
    expect(stop).not.toHaveBeenCalled()
    expect(queryBySlug.invalidate).not.toHaveBeenCalled()
    idHandle.release()
    expect(stop).toHaveBeenCalledTimes(1)
    expect(queryBySlug.invalidate).toHaveBeenCalledTimes(1)
    expect(queryById.invalidate).not.toHaveBeenCalled()
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

  test("retains the explicitly injected legacy stream URL until its owner migrates", () => {
    const query = {
      load: vi.fn(async () => readModel),
      reload: vi.fn(async () => readModel),
      invalidate: vi.fn(),
    } satisfies BoardReadQuery
    const createController = vi.fn((options: ConstructorParameters<typeof import("../sync/index").WebSyncController>[0]) => ({
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

import type { BoardReadModel, BoardReadQuery } from "../../lib/api/board-read-model"
import { resolveHttpRequestURL, type HttpTransport } from "../../lib/api/http-transport"
import {
  createBoardSyncSink,
  createEventsApiClient,
  createFetchSseTransport,
  WebSyncController,
  asCanonicalBoardId,
  type CanonicalBoardId,
  type SseTransport,
  type StreamContractAdapter,
  type SyncTelemetryEntry,
} from "../../lib/sync"
import type { WebRuntimeConfig } from "../../lib/runtime"
import type { BoardViewModel } from "./types"

export interface BoardReadResource {
  readonly selector: string
  readonly transport: HttpTransport
  readonly query: BoardReadQuery
  readonly adapter: StreamContractAdapter
  readonly runtimeKey: string
  identityKey: string
  canonicalBoardId: CanonicalBoardId | null
  resolvedSlug: string | null
  sessionGeneration: number
}

interface BoardSession {
  readonly key: string
  readonly generation: number
  readonly query: BoardReadQuery
  readonly controller: Pick<WebSyncController, "start" | "stop" | "retry">
  readonly resource: BoardReadResource
  readonly resources: Set<BoardReadResource>
  readonly listeners: Set<(model: BoardReadModel) => void>
  readonly telemetryListeners: Set<(entry: SyncTelemetryEntry) => void>
  refs: number
  disposed: boolean
}

export interface BoardSessionHandle {
  readonly release: () => void
  readonly retry: () => void
  /** Explicit user-requested reconnect; shares the canonical session controller. */
  readonly reconnect: () => void
  readonly generation: number
}

export interface BoardSessionTestDependencies {
  readonly streamTransport?: SseTransport
  readonly createController?: (options: ConstructorParameters<typeof WebSyncController>[0]) => Pick<WebSyncController, "start" | "stop" | "retry">
}

const sessions = new Map<string, BoardSession>()

export function runtimeIdentityKey(runtime: WebRuntimeConfig): string {
  return `${runtime.apiBaseUrl}\u0000${runtime.webBasePath}\u0000${runtime.webBuildId}`
}

export function resourceIdentityKey(
  runtime: WebRuntimeConfig,
  selector: string,
  canonicalBoardId: CanonicalBoardId | null,
): string {
  return `${runtimeIdentityKey(runtime)}\u0000${selector}\u0000${canonicalBoardId ?? "?"}`
}

/** Canonical session ownership is independent from the selector used to read a board. */
function sessionKey(runtime: WebRuntimeConfig, canonicalBoardId: CanonicalBoardId): string {
  return `${runtimeIdentityKey(runtime)}\u0000${canonicalBoardId}`
}

export function routeResourceContextKey(runtime: WebRuntimeConfig, selector: string, routeKind: string, boardSlug = ""): string {
  return `${resourceIdentityKey(runtime, selector, null)}\u0000${routeKind}\u0000${boardSlug}`
}

export function bindBoardResourceIdentity(
  runtime: WebRuntimeConfig,
  resource: BoardReadResource,
  readModel: BoardReadModel,
): string {
  const canonicalBoardId = asCanonicalBoardId(readModel.identity.canonicalBoardId)
  const expectedRuntimeKey = runtimeIdentityKey(runtime)
  if (resource.runtimeKey !== expectedRuntimeKey) throw new Error("board resource runtime identity changed")
  if (resource.selector !== readModel.identity.selector) throw new Error("board resource selector changed")
  resource.canonicalBoardId = canonicalBoardId
  resource.resolvedSlug = readModel.identity.slug
  resource.identityKey = resourceIdentityKey(runtime, resource.selector, canonicalBoardId)
  return resource.identityKey
}

function validatedStreamURL(runtime: WebRuntimeConfig): string {
  const resolved = new URL(resolveHttpRequestURL(runtime, "/api/v1/stream/events"))
  return `${resolved.pathname}${resolved.search}${resolved.hash}`
}

/** Number of live canonical-board sessions; used by focused registry tests. */
export function activeBoardSessionCount(): number {
  return sessions.size
}

/**
 * Ask the currently active canonical session for this runtime to reconnect.
 * Settings uses this seam instead of constructing a second SSE transport.
 */
export function reconnectActiveBoardSession(runtime: WebRuntimeConfig): boolean {
  const runtimeKey = runtimeIdentityKey(runtime)
  for (const session of sessions.values()) {
    if (!session.disposed && session.key.startsWith(`${runtimeKey}\u0000`)) {
      session.controller.retry()
      return true
    }
  }
  return false
}

/** Test-only cleanup for aborted test mounts; production unmounts use release(). */
export function resetBoardSessionsForTests(): void {
  for (const session of sessions.values()) {
    session.disposed = true
    session.listeners.clear()
    session.telemetryListeners.clear()
    session.controller.stop()
    for (const resource of session.resources) resource.sessionGeneration += 1
    session.query.invalidate()
  }
  sessions.clear()
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => resetBoardSessionsForTests())
}

export function acquireBoardSession(
  runtime: WebRuntimeConfig,
  model: BoardViewModel,
  resource: BoardReadResource,
  onModel: (model: BoardReadModel) => void,
  onTelemetry: (entry: SyncTelemetryEntry) => void,
  dependencies: BoardSessionTestDependencies = {},
): BoardSessionHandle {
  const boardId = asCanonicalBoardId(model.board.id)
  const expectedKey = resourceIdentityKey(runtime, resource.selector, boardId)
  if (
    resource.runtimeKey !== runtimeIdentityKey(runtime)
    || resource.canonicalBoardId !== boardId
    || resource.resolvedSlug !== model.board.slug
    || resource.identityKey !== expectedKey
  ) {
    throw new Error("board session resource identity mismatch")
  }
  const key = sessionKey(runtime, boardId)
  let session = sessions.get(key)
  if (session?.disposed === true) {
    if (sessions.get(key) === session) sessions.delete(key)
    session = undefined
  }
  if (session === undefined) {
    const listeners = new Set<(nextModel: BoardReadModel) => void>()
    const telemetryListeners = new Set<(entry: SyncTelemetryEntry) => void>()
    const eventsApi = createEventsApiClient({ transport: resource.transport })
    const sink = createBoardSyncSink({
      identity: { canonicalBoardId: boardId, selector: model.board.slug },
      query: resource.query,
      eventsApi,
      adapter: resource.adapter,
      publish: (nextModel) => {
        for (const listener of listeners) listener(nextModel)
      },
    })
    const controllerOptions = {
      boardSelector: model.board.slug,
      canonicalBoardId: boardId,
      streamUrl: validatedStreamURL(runtime),
      transport: dependencies.streamTransport ?? createFetchSseTransport(),
      adapter: resource.adapter,
      sink,
      telemetry: {
        record: (entry: SyncTelemetryEntry) => {
          for (const listener of telemetryListeners) listener(entry)
        },
      },
    } satisfies ConstructorParameters<typeof WebSyncController>[0]
    const controller = dependencies.createController?.(controllerOptions) ?? new WebSyncController(controllerOptions)
    resource.sessionGeneration += 1
    session = {
      key,
      generation: resource.sessionGeneration,
      query: resource.query,
      resource,
      resources: new Set([resource]),
      controller,
      listeners,
      telemetryListeners,
      refs: 0,
      disposed: false,
    }
    sessions.set(key, session)
  }

  resource.sessionGeneration = session.generation
  session.resources.add(resource)
  session.refs += 1
  // Each acquire owns a wrapper, so two mounts that happen to pass the same
  // callback cannot remove one another's subscription on first release.
  const modelListener = (nextModel: BoardReadModel) => onModel(nextModel)
  const telemetryListener = (entry: SyncTelemetryEntry) => onTelemetry(entry)
  session.listeners.add(modelListener)
  session.telemetryListeners.add(telemetryListener)
  session.controller.start()

  let released = false
  const release = () => {
    if (released) return
    released = true
    if (session === undefined || sessions.get(key) !== session || session.key !== key) return
    session.listeners.delete(modelListener)
    session.telemetryListeners.delete(telemetryListener)
    session.refs -= 1
    if (session.refs > 0) return
    session.disposed = true
    session.listeners.clear()
    session.telemetryListeners.clear()
    for (const resource of session.resources) resource.sessionGeneration += 1
    session.controller.stop()
    session.query.invalidate()
    if (sessions.get(key) === session) sessions.delete(key)
  }
  return {
    release,
    retry: () => {
      if (!released && session !== undefined && !session.disposed && sessions.get(key) === session) session.controller.retry()
    },
    reconnect: () => {
      if (!released && session !== undefined && !session.disposed && sessions.get(key) === session) session.controller.retry()
    },
    generation: session.generation,
  }
}

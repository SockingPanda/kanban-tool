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
  refreshPromise: Promise<void> | null
  refs: number
  disposed: boolean
}

export interface BoardSessionHandle {
  readonly release: () => void
  readonly retry: () => void
  /** Reload canonical board data through the existing session and publish it to subscribers. */
  readonly refresh: () => Promise<void>
  readonly generation: number
}

export interface BoardSessionTestDependencies {
  readonly streamTransport?: SseTransport
  readonly createController?: (options: ConstructorParameters<typeof WebSyncController>[0]) => Pick<WebSyncController, "start" | "stop" | "retry">
}

const sessions = new Map<string, BoardSession>()
/** Feature readers observe the canonical session; they never open another stream. */
const sessionObservers = new Map<string, Set<(model: BoardReadModel) => void>>()
const sessionTelemetryObservers = new Map<string, Set<(entry: SyncTelemetryEntry) => void>>()

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
  sessionObservers.clear()
  sessionTelemetryObservers.clear()
}

/**
 * Subscribe to model publications for one canonical board session.
 *
 * The observer is intentionally keyed separately from acquire/release refs so
 * a feature route can subscribe before BoardLive has finished resolving its
 * board identity. The pending observer is attached automatically when the
 * session starts, and no second SSE/polling source is created.
 */
export function subscribeBoardSession(
  runtime: WebRuntimeConfig,
  canonicalBoardId: CanonicalBoardId,
  listener: (model: BoardReadModel) => void,
): () => void {
  const key = sessionKey(runtime, canonicalBoardId)
  let observers = sessionObservers.get(key)
  if (observers === undefined) {
    observers = new Set()
    sessionObservers.set(key, observers)
  }
  observers.add(listener)
  let active = true
  return () => {
    if (!active) return
    active = false
    const current = sessionObservers.get(key)
    if (current === undefined) return
    current.delete(listener)
    if (current.size === 0 && sessions.get(key) === undefined) sessionObservers.delete(key)
  }
}
/** Subscribe to the already-validated SSE/recovery telemetry of one session. */
export function subscribeBoardSessionTelemetry(
  runtime: WebRuntimeConfig,
  canonicalBoardId: CanonicalBoardId,
  listener: (entry: SyncTelemetryEntry) => void,
): () => void {
  const key = sessionKey(runtime, canonicalBoardId)
  let observers = sessionTelemetryObservers.get(key)
  if (observers === undefined) {
    observers = new Set()
    sessionTelemetryObservers.set(key, observers)
  }
  observers.add(listener)
  let active = true
  return () => {
    if (!active) return
    active = false
    const current = sessionTelemetryObservers.get(key)
    if (current === undefined) return
    current.delete(listener)
    if (current.size === 0 && sessions.get(key) === undefined) sessionTelemetryObservers.delete(key)
  }
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
        for (const observer of sessionObservers.get(key) ?? []) observer(nextModel)
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
          for (const observer of sessionTelemetryObservers.get(key) ?? []) observer(entry)
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
      refreshPromise: null,
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
    refresh: () => {
      if (released || session === undefined || session.disposed || sessions.get(key) !== session) return Promise.resolve()
      if (session.refreshPromise !== null) return session.refreshPromise
      const current = session
      const generation = current.generation
      const refreshPromise = (async () => {
        const nextModel = await current.query.reload()
        // The refresh belongs to the canonical session entry, not the handle
        // that happened to start it. A released handle must not suppress a
        // publish when another owner still retains the same session.
        if (current.disposed || current.generation !== generation || sessions.get(key) !== current) return
        for (const listener of current.listeners) listener(nextModel)
        for (const observer of sessionObservers.get(key) ?? []) observer(nextModel)
      })()
      current.refreshPromise = refreshPromise
      void refreshPromise.then(() => {
        if (current.refreshPromise === refreshPromise) current.refreshPromise = null
      }, () => {
        if (current.refreshPromise === refreshPromise) current.refreshPromise = null
      })
      return refreshPromise
    },
    generation: session.generation,
  }
}

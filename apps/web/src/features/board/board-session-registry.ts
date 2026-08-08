import type { BoardReadModel, BoardReadQuery } from "../../lib/api/board-read-model"
import type { HttpTransport } from "../../lib/api/http-transport"
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
}

interface BoardSession {
  readonly query: BoardReadQuery
  readonly controller: Pick<WebSyncController, "start" | "stop" | "retry">
  readonly listeners: Set<(model: BoardReadModel) => void>
  readonly telemetryListeners: Set<(entry: SyncTelemetryEntry) => void>
  refs: number
}

export interface BoardSessionHandle {
  readonly release: () => void
  readonly retry: () => void
}

export interface BoardSessionTestDependencies {
  readonly streamTransport?: SseTransport
  readonly createController?: (options: ConstructorParameters<typeof WebSyncController>[0]) => Pick<WebSyncController, "start" | "stop" | "retry">
}

const sessions = new Map<string, BoardSession>()

function sessionKey(runtime: WebRuntimeConfig, boardId: CanonicalBoardId): string {
  return `${runtime.apiBaseUrl}\u0000${runtime.webBasePath}\u0000${runtime.webBuildId}\u0000${boardId}`
}

/** Number of live canonical-board sessions; used by focused registry tests. */
export function activeBoardSessionCount(): number {
  return sessions.size
}

/** Test-only cleanup for aborted test mounts; production unmounts use release(). */
export function resetBoardSessionsForTests(): void {
  for (const session of sessions.values()) {
    session.controller.stop()
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
  const key = sessionKey(runtime, boardId)
  let session = sessions.get(key)
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
      streamUrl: "/api/v1/stream/events",
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
    session = { query: resource.query, controller, listeners, telemetryListeners, refs: 0 }
    sessions.set(key, session)
  }

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
    session?.listeners.delete(modelListener)
    session?.telemetryListeners.delete(telemetryListener)
    if (session === undefined) return
    session.refs -= 1
    if (session.refs > 0) return
    session.controller.stop()
    session.query.invalidate()
    sessions.delete(key)
  }
  return {
    release,
    retry: () => {
      if (!released) session?.controller.retry()
    },
  }
}

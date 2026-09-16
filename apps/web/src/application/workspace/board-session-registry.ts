import type { BoardReadModel, BoardReadQuery } from "../data/board-read-model";
import { asCanonicalBoardId, type CanonicalBoardId } from "../../domain/board-id";
import type { WebRuntimeConfig } from "../../lib/runtime"
import type { BoardSyncStatus, BoardViewModel } from "../../domain/tasks/board"

export interface BoardReadResource {
  readonly selector: string
  readonly query: BoardReadQuery
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
  readonly resources: Set<BoardReadResource>
  readonly listeners: Set<(model: BoardReadModel) => void>
  readonly stateListeners: Set<(state: BoardSyncStatus) => void>
  readonly abort: AbortController
  releaseConnection: () => void
  state: BoardSyncStatus
  refs: number
  disposed: boolean
}

export type BoardReconnectResult = "reconnecting" | "already-live" | "unavailable"

export interface BoardSessionHandle {
  readonly release: () => void
  readonly retry: () => void
  /** Reload canonical board data through the existing session and publish it to subscribers. */
  readonly refresh: () => Promise<void>
  /** Explicit user-requested reconnect; shares the canonical session controller. */
  readonly reconnect: () => void
  readonly generation: number
}

const sessions = new Map<string, BoardSession>()
const sessionListeners = new Set<() => void>()
let sessionRevision = 0

function notifySessionListeners(): void {
  sessionRevision += 1
  for (const listener of sessionListeners) listener()
}

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

/** 写后刷新由 QueryService Ready 确认；会话仅负责把完整模型交给仍挂载的消费者。 */
async function refreshSession(session: BoardSession): Promise<void> {
  if (!isCurrent(session)) return
  const nextModel = await session.query.reload(session.abort.signal)
  if (isCurrent(session)) for (const listener of session.listeners) listener(nextModel)
}

function isCurrent(session: BoardSession): boolean {
  return !session.disposed && sessions.get(session.key) === session
}

function setConnectionState(session: BoardSession, state: BoardSyncStatus): void {
  if (!isCurrent(session)) return
  session.state = state
  for (const listener of session.stateListeners) listener(state)
}

function retrySession(session: BoardSession): void {
  void refreshSession(session).catch(() => setConnectionState(session, 'stale'))
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


/** Number of live canonical-board sessions; used by focused registry tests. */
export function activeBoardSessionCount(): number {
  return sessions.size
}

/** 会话归属改变时通知订阅者，重连控件使用同一个会话事实。 */
export function subscribeBoardSessions(listener: () => void): () => void {
  sessionListeners.add(listener)
  return () => sessionListeners.delete(listener)
}

/** Monotonic snapshot for React's external-store subscription. */
export function boardSessionRevision(): number {
  return sessionRevision
}

/** Return whether a live canonical session exists for this runtime and optional board slug. */
export function hasActiveBoardSession(runtime: WebRuntimeConfig, boardSlug?: string): boolean {
  const runtimeKey = runtimeIdentityKey(runtime)
  for (const session of sessions.values()) {
    if (session.disposed || !session.key.startsWith(`${runtimeKey}\u0000`)) continue
    if (boardSlug === undefined || [...session.resources].some((resource) => resource.resolvedSlug === boardSlug)) return true
  }
  return false
}

/** Ask the currently active canonical session for this runtime to reconnect. */
export function reconnectActiveBoardSession(runtime: WebRuntimeConfig, boardSlug?: string): BoardReconnectResult {
  const runtimeKey = runtimeIdentityKey(runtime)
  for (const session of sessions.values()) {
    if (
      !session.disposed
      && session.key.startsWith(`${runtimeKey}\u0000`)
      && (boardSlug === undefined || [...session.resources].some((resource) => resource.resolvedSlug === boardSlug))
    ) {
      if (session.state === "live") return "already-live"
      retrySession(session)
      return "reconnecting"
    }
  }
  return "unavailable"
}

/** Test-only cleanup for aborted test mounts; production unmounts use release(). */
export function resetBoardSessionsForTests(): void {
  for (const session of sessions.values()) {
    session.disposed = true
    session.listeners.clear()
    session.stateListeners.clear()
    session.abort.abort()
    session.releaseConnection()
    for (const resource of session.resources) resource.sessionGeneration += 1
    session.query.invalidate()
  }
  sessions.clear()
  notifySessionListeners()
}
if (import.meta.hot) {
  import.meta.hot.dispose(() => resetBoardSessionsForTests())
}

export function acquireBoardSession(
  runtime: WebRuntimeConfig,
  model: BoardViewModel,
  resource: BoardReadResource,
  onModel: (model: BoardReadModel) => void,
  onState: (state: BoardSyncStatus) => void,
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
    resource.sessionGeneration += 1
    session = {
      key,
      generation: resource.sessionGeneration,
      query: resource.query,
      resources: new Set([resource]),
      listeners: new Set(),
      stateListeners: new Set(),
      abort: new AbortController(),
      releaseConnection: () => undefined,
      state: 'connecting',
      refs: 0,
      disposed: false,
    }
    sessions.set(key, session)
    notifySessionListeners()
  }

  resource.sessionGeneration = session.generation
  session.resources.add(resource)
  session.refs += 1
  // Each acquire owns a wrapper, so two mounts that happen to pass the same
  // callback cannot remove one another's subscription on first release.
  const modelListener = (nextModel: BoardReadModel) => onModel(nextModel)
  const stateListener = (state: BoardSyncStatus) => onState(state)
  session.listeners.add(modelListener)
  session.stateListeners.add(stateListener)
  if (session.refs === 1) {
    const owned = session
    try {
      owned.query.observe(owned.abort.signal,
        nextModel => { if (isCurrent(owned)) for (const listener of owned.listeners) listener(nextModel) },
        () => setConnectionState(owned, 'stale'))
      owned.releaseConnection = owned.query.subscribeConnection(next => setConnectionState(owned, next === 'offline' ? 'stale' : next))
    } catch (error) {
      owned.disposed = true
      owned.abort.abort()
      owned.releaseConnection()
      owned.query.invalidate()
      sessions.delete(key)
      notifySessionListeners()
      throw error
    }
  } else onState(session.state)

  let released = false
  const release = () => {
    if (released) return
    released = true
    if (session === undefined || sessions.get(key) !== session || session.key !== key) return
    session.listeners.delete(modelListener)
    session.stateListeners.delete(stateListener)
    session.refs -= 1
    if (session.refs > 0) return
    session.disposed = true
    session.listeners.clear()
    session.stateListeners.clear()
    for (const resource of session.resources) resource.sessionGeneration += 1
    session.abort.abort()
    session.releaseConnection()
    session.query.invalidate()
    if (sessions.get(key) === session) sessions.delete(key)
    if (sessions.get(key) === undefined) notifySessionListeners()
  }
  return {
    release,
    retry: () => {
      if (!released && session !== undefined && !session.disposed && sessions.get(key) === session) retrySession(session)
    },
    reconnect: () => {
      if (!released && session !== undefined && !session.disposed && sessions.get(key) === session) retrySession(session)
    },
    refresh: () => {
      if (released || session === undefined || session.disposed || sessions.get(key) !== session) return Promise.resolve()
      return refreshSession(session)
    },
    generation: session.generation,
  }
}

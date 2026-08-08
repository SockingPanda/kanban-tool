import { parseApiListEventsQuery } from "../api/generated/contracts/api-list-events-query"
import type { BoardReadModel } from "../api/board-read-model"
import type { BoardReadQuery } from "../api/board-read-model"
import type { EventsApiClient } from "./events-api"
import type {
  InvalidationPlan,
  PollEventsPage,
  RecoveryBoundary,
  RecoveryMode,
  RecoveryResult,
  StreamContractAdapter,
  SyncQuerySink,
  SyncToken,
  ValidatedBusinessEvent,
} from "./contracts"
import type { CanonicalBoardId } from "./contracts"

const EVENT_PAGE_SIZE = 100
const MAX_RECOVERY_EVENTS = 2_048
const MAX_RECOVERY_BYTES = 8 * 1024 * 1024

export interface BoardSyncSinkIdentity {
  readonly canonicalBoardId: CanonicalBoardId
  readonly selector: string
}

export interface BoardSyncSinkOptions {
  readonly identity: BoardSyncSinkIdentity
  readonly query: BoardReadQuery
  readonly publish: (model: BoardReadModel) => void | Promise<void>
  readonly eventsApi: EventsApiClient
  readonly adapter: StreamContractAdapter
}

export class BoardSyncSinkError extends Error {
  readonly code:
    | "invalid_options"
    | "invalid_cursor"
    | "invalid_page"
    | "board_isolation"
    | "duplicate_event"
    | "fingerprint_conflict"
    | "recovery_budget"
    | "invalid_event"

  constructor(code: BoardSyncSinkError["code"], message: string) {
    super(message)
    this.name = "BoardSyncSinkError"
    this.code = code
  }
}

interface OperationSignal {
  readonly controller: AbortController
  readonly signal: AbortSignal
  readonly cleanup: () => void
}

interface PublishedBoundary {
  readonly token: SyncToken
  readonly revision: number
}

interface CollectedEvents {
  readonly events: readonly ValidatedBusinessEvent[]
  readonly highWatermark: number
  readonly byId: ReadonlyMap<number, string>
  readonly byEventId: ReadonlyMap<string, string>
}

function linkSignal(external: AbortSignal | undefined, controller: AbortController): OperationSignal {
  const onAbort = (): void => controller.abort()
  if (external?.aborted === true) controller.abort()
  else external?.addEventListener("abort", onAbort, { once: true })
  return {
    controller,
    signal: controller.signal,
    cleanup: () => external?.removeEventListener("abort", onAbort),
  }
}

function tokenEqual(left: SyncToken, right: SyncToken): boolean {
  return left.boardId === right.boardId
    && left.connectionEpoch === right.connectionEpoch
    && left.generation === right.generation
}

function tokenNewer(left: SyncToken, right: SyncToken): boolean {
  if (left.connectionEpoch !== right.connectionEpoch) return left.connectionEpoch > right.connectionEpoch
  return left.generation > right.generation
}

function safeCursor(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
}

function byteLength(value: unknown): number {
  try {
    const serialized = JSON.stringify(value)
    if (serialized === undefined) return Number.POSITIVE_INFINITY
    const encoded = new TextEncoder().encode(serialized)
    return encoded.byteLength
  } catch {
    return Number.POSITIVE_INFINITY
  }
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function emptyRecovery(token: SyncToken, revision: number, after: number): RecoveryResult {
  return {
    confirmedCursor: after,
    noGap: false,
    boundary: {
      highWatermark: after,
      byId: new Map(),
      byEventId: new Map(),
      events: [],
      token,
      revision,
      published: false,
    },
  }
}

function relevantPlan(plan: InvalidationPlan, boardId: CanonicalBoardId): boolean {
  return plan.fullRefetch || plan.targets.some((target) =>
    (target.root === "columns" || target.root === "tasks") && target.boardId === boardId)
}

function resolveIdentity(options: BoardSyncSinkOptions): BoardSyncSinkIdentity {
  const canonicalBoardId = options.identity.canonicalBoardId
  if (canonicalBoardId === undefined || canonicalBoardId.length === 0) {
    throw new BoardSyncSinkError("invalid_options", "canonical board identity is required")
  }
  const selector = options.identity.selector
  if (selector.trim().length === 0) {
    throw new BoardSyncSinkError("invalid_options", "board selector is required")
  }
  return { canonicalBoardId, selector }
}

function resolveQuery(options: BoardSyncSinkOptions): BoardReadQuery {
  return options.query
}

function resolveEventsApi(options: BoardSyncSinkOptions): EventsApiClient {
  return options.eventsApi
}

async function collectEvents(
  eventsApi: EventsApiClient,
  adapter: StreamContractAdapter,
  selector: string,
  canonicalBoardId: CanonicalBoardId,
  after: number,
  signal: AbortSignal,
): Promise<CollectedEvents> {
  let cursor = after
  let pages = 0
  let bytes = 0
  const events: ValidatedBusinessEvent[] = []
  const byId = new Map<number, string>()
  const byEventId = new Map<string, string>()

  while (true) {
    if (signal.aborted) return { events, highWatermark: cursor, byId, byEventId }
    const query = parseApiListEventsQuery({ board: selector, after: cursor, limit: EVENT_PAGE_SIZE })
    const response = await eventsApi.listEvents(query, signal)
    if (response.data.length > EVENT_PAGE_SIZE) {
      throw new BoardSyncSinkError("invalid_page", "events API returned more than the requested page size")
    }
    const nextAfter = response.meta.next_after
    if (!safeCursor(nextAfter)) throw new BoardSyncSinkError("invalid_cursor", "events API returned an invalid next_after")

    let previousId = cursor
    for (const rawEvent of response.data) {
      const rawBytes = byteLength(rawEvent)
      if (!Number.isFinite(rawBytes) || bytes > MAX_RECOVERY_BYTES - rawBytes) {
        throw new BoardSyncSinkError("recovery_budget", "recovery events exceeded the byte budget")
      }
      bytes += rawBytes
      const parsed = adapter.parsePollingEnvelope(rawEvent)
      if (parsed.status === "invalid") throw new BoardSyncSinkError("invalid_event", parsed.code)
      const envelope = parsed.envelope
      if (envelope.boardId !== canonicalBoardId) {
        throw new BoardSyncSinkError("board_isolation", "recovery event belongs to another board")
      }
      if (!safeCursor(envelope.id)) {
        throw new BoardSyncSinkError("invalid_page", "recovery events are not strictly ascending")
      }
      if (events.length >= MAX_RECOVERY_EVENTS) {
        throw new BoardSyncSinkError("recovery_budget", "recovery events exceeded the event budget")
      }
      const business = adapter.validateBusiness(envelope)
      if (business.status === "invalid") throw new BoardSyncSinkError("invalid_event", business.code)
      const event = business.event
      if (event.boardId !== canonicalBoardId) {
        throw new BoardSyncSinkError("board_isolation", "validated recovery event belongs to another board")
      }
      const idFingerprint = byId.get(event.id)
      if (idFingerprint !== undefined) {
        if (idFingerprint !== event.canonicalFingerprint) throw new BoardSyncSinkError("fingerprint_conflict", "event id fingerprint conflict")
        throw new BoardSyncSinkError("duplicate_event", "duplicate event id in recovery")
      }
      const eventFingerprint = byEventId.get(event.eventId)
      if (eventFingerprint !== undefined) {
        if (eventFingerprint !== event.canonicalFingerprint) throw new BoardSyncSinkError("fingerprint_conflict", "event_id fingerprint conflict")
        throw new BoardSyncSinkError("duplicate_event", "duplicate event_id in recovery")
      }
      if (event.id <= previousId) {
        throw new BoardSyncSinkError("invalid_page", "recovery events are not strictly ascending")
      }
      byId.set(event.id, event.canonicalFingerprint)
      byEventId.set(event.eventId, event.canonicalFingerprint)
      events.push(event)
      previousId = event.id
    }

    if (response.data.length === 0) {
      if (nextAfter !== cursor) throw new BoardSyncSinkError("invalid_cursor", "empty events page advanced next_after")
      break
    }
    if (nextAfter !== previousId || nextAfter <= cursor) {
      throw new BoardSyncSinkError("invalid_cursor", "events page next_after does not match its last event")
    }
    cursor = nextAfter
    pages += 1
    if (pages > Math.ceil(MAX_RECOVERY_EVENTS / EVENT_PAGE_SIZE) + 1) {
      throw new BoardSyncSinkError("recovery_budget", "recovery pagination exceeded the page budget")
    }
    if (response.data.length < EVENT_PAGE_SIZE) break
  }

  return { events, highWatermark: cursor, byId, byEventId }
}

export function createBoardSyncSink(options: BoardSyncSinkOptions): SyncQuerySink {
  const identity = resolveIdentity(options)
  const query = resolveQuery(options)
  const eventsApi = resolveEventsApi(options)
  let currentToken: SyncToken | null = null
  let activeOperation: AbortController | null = null
  let published: PublishedBoundary | null = null

  function isCurrent(token: SyncToken, signal?: AbortSignal): boolean {
    return signal?.aborted !== true && currentToken !== null && tokenEqual(currentToken, token)
  }

  function acceptToken(token: SyncToken): boolean {
    if (token.boardId !== identity.canonicalBoardId) return false
    if (currentToken === null) {
      currentToken = token
      return true
    }
    if (tokenEqual(currentToken, token)) return true
    if (!tokenNewer(token, currentToken)) return false
    activeOperation?.abort()
    activeOperation = null
    published = null
    currentToken = token
    return true
  }

  function operation(signal: AbortSignal | undefined): OperationSignal {
    const controller = new AbortController()
    const linked = linkSignal(signal, controller)
    activeOperation = controller
    return linked
  }

  function finishOperation(controller: AbortController, cleanup: () => void): void {
    cleanup()
    if (activeOperation === controller) activeOperation = null
  }

  const sink: SyncQuerySink = {
    async onEvent(event, plan, token, signal) {
      if (event.boardId !== identity.canonicalBoardId || !acceptToken(token) || !relevantPlan(plan, identity.canonicalBoardId)) return
      const run = operation(signal)
      published = null
      try {
        const model = await query.reload(run.signal)
        if (!isCurrent(token, run.signal)) return
        await options.publish(model)
      } finally {
        finishOperation(run.controller, run.cleanup)
      }
    },

    async refetchObserved(_mode: RecoveryMode, token, after, expectedRevision, signal) {
      if (!safeCursor(after) || !Number.isSafeInteger(expectedRevision) || expectedRevision <= 0 || !acceptToken(token)) {
        return emptyRecovery(token, expectedRevision, safeCursor(after) ? after : 0)
      }
      const run = operation(signal)
      published = null
      try {
        const model = await query.reload(run.signal)
        if (!isCurrent(token, run.signal)) return emptyRecovery(token, expectedRevision, after)
        await options.publish(model)
        if (!isCurrent(token, run.signal)) return emptyRecovery(token, expectedRevision, after)
        const collected = await collectEvents(eventsApi, options.adapter, identity.selector, identity.canonicalBoardId, after, run.signal)
        if (!isCurrent(token, run.signal)) return emptyRecovery(token, expectedRevision, after)
        const boundary: RecoveryBoundary = {
          highWatermark: collected.highWatermark,
          byId: collected.byId,
          byEventId: collected.byEventId,
          events: collected.events,
          token,
          revision: expectedRevision,
          published: true,
        }
        published = { token, revision: expectedRevision }
        return { confirmedCursor: collected.highWatermark, noGap: true, boundary }
      } catch (error) {
        if (isAbortError(error)) throw error
        if (run.signal.aborted || !isCurrent(token)) return emptyRecovery(token, expectedRevision, after)
        throw error
      } finally {
        finishOperation(run.controller, run.cleanup)
      }
    },

    async pollEvents(queryInput, signal): Promise<PollEventsPage> {
      const query = parseApiListEventsQuery(queryInput)
      const after = query.after ?? 0
      if (signal.aborted) return { events: [], nextAfter: after, hasMore: false, noGap: false }
      const response = await eventsApi.listEvents(query, signal)
      if (signal.aborted) return { events: [], nextAfter: after, hasMore: false, noGap: false }
      const nextAfter = response.meta.next_after
      const limit = query.limit ?? EVENT_PAGE_SIZE
      let noGap = safeCursor(after)
        && safeCursor(nextAfter)
        && nextAfter >= after
        && query.board === identity.selector
        && Number.isSafeInteger(limit)
        && limit >= 0
        && response.data.length <= limit
      let previousId = after
      const pageEvents: ValidatedBusinessEvent[] = []
      const pageById = new Map<number, string>()
      const pageByEventId = new Map<string, string>()
      for (const rawEvent of response.data) {
        const parsed = options.adapter.parsePollingEnvelope(rawEvent)
        if (parsed.status === "invalid") {
          noGap = false
          break
        }
        if (parsed.envelope.boardId !== identity.canonicalBoardId || !safeCursor(parsed.envelope.id) || parsed.envelope.id <= previousId) {
          noGap = false
          break
        }
        const business = options.adapter.validateBusiness(parsed.envelope)
        if (business.status === "invalid" || business.event.boardId !== identity.canonicalBoardId) {
          noGap = false
          break
        }
        const event = business.event
        const idFingerprint = pageById.get(event.id)
        const eventFingerprint = pageByEventId.get(event.eventId)
        if (idFingerprint !== undefined && idFingerprint !== event.canonicalFingerprint) noGap = false
        if (eventFingerprint !== undefined && eventFingerprint !== event.canonicalFingerprint) noGap = false
        if (idFingerprint !== undefined || eventFingerprint !== undefined) noGap = false
        if (!noGap) break
        pageById.set(event.id, event.canonicalFingerprint)
        pageByEventId.set(event.eventId, event.canonicalFingerprint)
        pageEvents.push(event)
        previousId = event.id
      }
      if (response.data.length === 0) noGap = noGap && nextAfter === after
      else noGap = noGap && nextAfter === previousId
      const hasMore = noGap && response.data.length === limit && nextAfter > after
      const page: PollEventsPage = { events: response.data, nextAfter, hasMore, noGap }
      if (noGap && query.board === identity.selector && published !== null && currentToken !== null && tokenEqual(published.token, currentToken)) {
        const boundary: RecoveryBoundary = {
          highWatermark: nextAfter,
          byId: pageById,
          byEventId: pageByEventId,
          events: pageEvents,
          token: published.token,
          revision: published.revision,
          published: true,
        }
        return { ...page, boundary, confirmedCursor: nextAfter }
      }
      return page
    },
  }
  return sink
}

export type BoardSyncSink = SyncQuerySink

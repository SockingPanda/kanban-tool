import { useCallback, useEffect, useRef, useState } from "react"
import { InternationalizationProvider } from "@astryxdesign/core/i18n"
import { Theme } from "@astryxdesign/core/theme"
import { neutralTheme } from "@astryxdesign/theme-neutral/built"

import { ProductShell } from "./ProductShell"
import { BoardLive } from "./features/board/BoardLive"
import { boardSyncStatusForTelemetry } from "./features/board/board-live-state"
import type { BoardSyncStatus } from "./features/board/types"
import { appendExplorerEventBatch, coalesceExplorerBoundary, explorerEventInvalidation } from "./App.logic"
import { parseBoardEvent, type BoardEventsBatch, type ExplorerEvent } from "./lib/api/explorer-read-model"
import type { SyncTelemetryEntry } from "./lib/sync"
import type { CanonicalBoardId } from "./lib/sync/contracts"
import { usePreferences } from "./lib/use-preferences"
import { PreferencesProvider } from "./lib/preferences-provider"
import { useAppRouter } from "./lib/router"
import { useWebRuntime } from "./lib/runtime-context"
import { astryxMessages, astryxOverrides } from "./lib/i18n"

const explorerInvalidationTelemetry = new Set([
  "connection-live",
  "recovery-start",
  "recovery-connection-retry",
  "event-applied",
  "recovery-complete",
  "poll-complete",
  "poll-boundary-complete",
  "protocol-anomaly",
  "isolation-anomaly",
  "poll-protocol-anomaly",
  "protocol-anomaly-suppressed",
  "stalled",
  "transport-failure",
  "sink-effect-failure",
  "recovery-failure",
  "poll-failure",
  "circuit-open",
  "detached-async-failure",
])

const EVENT_APPLIED_DEBOUNCE_MS = 200

function RuntimeThemedShell() {
  const runtime = useWebRuntime()
  const preferences = usePreferences()
  const router = useAppRouter({
    basePath: runtime.webBasePath,
    defaultBoard: runtime.defaultBoard,
  })
  const boardRoute = router.route.kind === "home" || router.route.kind === "board" ? router.route : null
  // The canonical BoardLive remains mounted for every board route as the
  // single session/SSE owner, while Explorer owns the visible board view.
  const liveBoardVisible = boardRoute?.kind === "home"
  const sessionKey = boardRoute === null
    ? "none"
    : `${runtime.apiBaseUrl}\u0000${runtime.webBasePath}\u0000${runtime.webBuildId}\u0000${boardRoute.kind === "board" ? boardRoute.boardSlug : ""}`
  const sessionKeyRef = useRef(sessionKey)
  sessionKeyRef.current = sessionKey
  const [sessionState, setSessionState] = useState<{ readonly key: string; readonly boardRevision: number; readonly inspectorRevision: number; readonly runsRevision: number; readonly eventsRefreshRevision: number }>(() => ({
    key: sessionKey,
    boardRevision: 0,
    inspectorRevision: 0,
    runsRevision: 0,
    eventsRefreshRevision: 0,
  }))
  const [syncStatus, setSyncStatus] = useState<BoardSyncStatus>("connecting")
  const [eventsBatchState, setEventsBatchState] = useState<{ readonly key: string; readonly batch: BoardEventsBatch | null }>(() => ({ key: sessionKey, batch: null }))
  const eventAppliedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const boundaryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pendingEventsRef = useRef<ExplorerEvent[]>([])
  const pendingBoardIdRef = useRef<CanonicalBoardId | null>(null)
  const pendingBoundaryRef = useRef(false)
  const pendingBoundaryTypesRef = useRef<Set<string>>(new Set())
  const pendingEventInvalidationRef = useRef({ board: false, inspector: false, runs: false, fullRefetch: false })
  const pendingEventBoundarySourceRef = useRef(false)

  const bumpExplorerRevision = useCallback((targets: { readonly board?: boolean; readonly inspector?: boolean; readonly runs?: boolean; readonly events?: boolean }) => {
    setSessionState((current) => {
      const key = sessionKeyRef.current
      if (current.key !== key) return current
      return {
        key,
        boardRevision: current.boardRevision + (targets.board === true ? 1 : 0),
        inspectorRevision: current.inspectorRevision + (targets.inspector === true ? 1 : 0),
        runsRevision: current.runsRevision + (targets.runs === true ? 1 : 0),
        eventsRefreshRevision: current.eventsRefreshRevision + (targets.events === true ? 1 : 0),
      }
    })
  }, [])

  const flushEventBatch = useCallback(() => {
    const pending = pendingEventsRef.current
    const boardId = pendingBoardIdRef.current
    const invalidation = pendingEventInvalidationRef.current
    const boundarySource = pendingEventBoundarySourceRef.current
    pendingEventsRef.current = []
    pendingBoardIdRef.current = null
    pendingEventInvalidationRef.current = { board: false, inspector: false, runs: false, fullRefetch: false }
    pendingEventBoundarySourceRef.current = false
    if (pending.length === 0 || boardId === null) return
    try {
      const events = appendExplorerEventBatch([], pending, boardId)
      const last = events.at(-1)
      if (!last) return
      setEventsBatchState({ key: sessionKeyRef.current, batch: { boardId, events, nextAfter: last.id } })
      if (!pendingBoundaryRef.current && !boundarySource && !invalidation.fullRefetch) {
        bumpExplorerRevision(invalidation)
      }
    } catch {
      // A malformed batch is a recovery boundary; EventsView must not receive
      // an untrusted partial append.
      setEventsBatchState({ key: sessionKeyRef.current, batch: null })
      pendingBoundaryRef.current = true
      pendingBoundaryTypesRef.current.add("protocol-anomaly")
      setSyncStatus("stale")
      if (boundaryTimerRef.current === null) {
        boundaryTimerRef.current = setTimeout(() => {
          boundaryTimerRef.current = null
          if (!pendingBoundaryRef.current) return
          pendingBoundaryRef.current = false
          pendingBoundaryTypesRef.current.clear()
          bumpExplorerRevision({ board: true, inspector: true, runs: true, events: true })
        }, 0)
      }
    }
  }, [bumpExplorerRevision])

  const scheduleBoundaryRefresh = useCallback((type = "poll-complete") => {
    pendingBoundaryRef.current = true
    pendingBoundaryTypesRef.current.add(type)
    if (eventAppliedTimerRef.current !== null) {
      clearTimeout(eventAppliedTimerRef.current)
      eventAppliedTimerRef.current = null
      flushEventBatch()
    }
    if (boundaryTimerRef.current !== null) return
    boundaryTimerRef.current = setTimeout(() => {
      boundaryTimerRef.current = null
      if (!pendingBoundaryRef.current) return
      if (eventAppliedTimerRef.current !== null) {
        clearTimeout(eventAppliedTimerRef.current)
        eventAppliedTimerRef.current = null
        flushEventBatch()
      }
      pendingBoundaryRef.current = false
      const boundary = coalesceExplorerBoundary([...pendingBoundaryTypesRef.current])
      pendingBoundaryTypesRef.current.clear()
      setEventsBatchState({ key: sessionKeyRef.current, batch: null })
      if (boundary.invalidationDelta === 1) bumpExplorerRevision({ board: true, inspector: true, runs: true, events: boundary.eventsRefreshDelta === 1 })
    }, 0)
  }, [bumpExplorerRevision, flushEventBatch])

  useEffect(() => {
    if (eventAppliedTimerRef.current !== null) {
      clearTimeout(eventAppliedTimerRef.current)
      eventAppliedTimerRef.current = null
    }
    if (boundaryTimerRef.current !== null) {
      clearTimeout(boundaryTimerRef.current)
      boundaryTimerRef.current = null
    }
    pendingEventsRef.current = []
    pendingBoardIdRef.current = null
    pendingBoundaryRef.current = false
    pendingBoundaryTypesRef.current.clear()
    pendingEventInvalidationRef.current = { board: false, inspector: false, runs: false, fullRefetch: false }
    pendingEventBoundarySourceRef.current = false
    setSyncStatus("connecting")
    setSessionState((current) => current.key === sessionKey ? current : { key: sessionKey, boardRevision: 0, inspectorRevision: 0, runsRevision: 0, eventsRefreshRevision: 0 })
    setEventsBatchState((current) => current.key === sessionKey ? current : { key: sessionKey, batch: null })
  }, [sessionKey])

  useEffect(() => () => {
    if (eventAppliedTimerRef.current !== null) clearTimeout(eventAppliedTimerRef.current)
    if (boundaryTimerRef.current !== null) clearTimeout(boundaryTimerRef.current)
  }, [])

  const onSessionTelemetry = useCallback((entry: SyncTelemetryEntry) => {
    if (!explorerInvalidationTelemetry.has(entry.type)) return
    const nextSyncStatus = boardSyncStatusForTelemetry(entry.type)
    if (nextSyncStatus !== null) setSyncStatus(nextSyncStatus)
    if (entry.type === "connection-live" || entry.type === "recovery-start" || entry.type === "recovery-connection-retry") return
    if (entry.type === "event-applied") {
      const event = parseBoardEvent(entry.details?.event)
      if (!event || event.board_id !== entry.boardId) {
        scheduleBoundaryRefresh("protocol-anomaly")
        return
      }
      const invalidation = explorerEventInvalidation(event, entry.boardId)
      pendingEventInvalidationRef.current = {
        board: pendingEventInvalidationRef.current.board || invalidation.board,
        inspector: pendingEventInvalidationRef.current.inspector || invalidation.inspector,
        runs: pendingEventInvalidationRef.current.runs || invalidation.runs,
        fullRefetch: pendingEventInvalidationRef.current.fullRefetch || invalidation.fullRefetch,
      }
      const source = typeof entry.details?.source === "string" ? entry.details.source : null
      if (source === "recovery" || source === "poll" || source === "poll-boundary") pendingEventBoundarySourceRef.current = true
      if (invalidation.fullRefetch) scheduleBoundaryRefresh("protocol-anomaly")
      if (pendingBoardIdRef.current !== null && pendingBoardIdRef.current !== entry.boardId) flushEventBatch()
      pendingBoardIdRef.current = entry.boardId
      pendingEventsRef.current.push(event)
      if (eventAppliedTimerRef.current !== null) return
      eventAppliedTimerRef.current = setTimeout(() => {
        eventAppliedTimerRef.current = null
        flushEventBatch()
      }, EVENT_APPLIED_DEBOUNCE_MS)
      return
    }
    // Recovery/poll completion and protocol anomalies are conservative
    // refresh boundaries; they are intentionally not tied to each event.
    scheduleBoundaryRefresh(entry.type)
  }, [flushEventBatch, scheduleBoundaryRefresh])

  const currentEventsBatch = eventsBatchState.key === sessionKey ? eventsBatchState.batch : null

  return (
    <InternationalizationProvider locale={preferences.locale} messages={astryxMessages} overrides={astryxOverrides}>
      <Theme theme={neutralTheme} mode={preferences.theme}>
        <ProductShell
          runtime={runtime}
          route={router.route}
          canonicalBoardSlug={router.route.kind === "board" ? router.route.boardSlug : undefined}
          boundary={router.error ? "error" : undefined}
          error={router.error instanceof Error ? router.error.message : undefined}
          onNavigate={router.navigate}
          onRetry={() => window.location.reload()}
          invalidationRevision={sessionState.key === sessionKey ? sessionState.boardRevision : 0}
          boardRevision={sessionState.key === sessionKey ? sessionState.boardRevision : 0}
          inspectorRevision={sessionState.key === sessionKey ? sessionState.inspectorRevision : 0}
          runsRevision={sessionState.key === sessionKey ? sessionState.runsRevision : 0}
          eventsRefreshRevision={sessionState.key === sessionKey ? sessionState.eventsRefreshRevision : 0}
          eventsBatch={currentEventsBatch}
          syncStatus={sessionState.key === sessionKey ? syncStatus : "connecting"}
        >
          {boardRoute ? (
            <BoardLive
              runtime={runtime}
              route={boardRoute}
              onNavigate={router.navigate}
              renderBoard={liveBoardVisible}
              onSessionTelemetry={onSessionTelemetry}
            />
          ) : null}
        </ProductShell>
      </Theme>
    </InternationalizationProvider>
  )
}

export default function App() {
  return (
    <PreferencesProvider>
      <RuntimeThemedShell />
    </PreferencesProvider>
  )
}

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react"
import { InternationalizationProvider } from "@astryxdesign/core/i18n"
import { Theme } from "@astryxdesign/core/theme"

import { ProductShell, type BoardListSurface } from "./ProductShell"
import { astryxTheme } from "./theme/astryx.js"
import { BoardFeatureRoute, type FeatureRoute } from "./features/BoardFeatureRoute"
import { BoardLive } from "./features/board/BoardLive"
import { boardSyncStatusForTelemetry } from "./features/board/board-live-state"
import type { BoardTaskCanonicalReloadHandler, BoardTaskCanonicalReloadOptions, BoardTaskMutationCommitted, BoardTaskMutationSurface } from "./features/board/task-mutation-state"
import type { BoardSyncStatus } from "./features/board/types"
import {
  appendExplorerEventBatch,
  coalesceExplorerBoundary,
  EVENT_APPLIED_DEBOUNCE_MS,
  EXPLORER_EVENT_BATCH_OVERFLOW_BOUNDARY,
  applyCanonicalSnapshotHandoff,
  explorerEventInvalidation,
  shouldRecoverExplorerEventBatch,
} from "./App.logic"
import { parseBoardEvent, type BoardEventsBatch, type ExplorerEvent } from "./lib/api/explorer-read-model"
import type { SyncTelemetryEntry } from "./lib/sync"
import type { CanonicalBoardId } from "./lib/sync/contracts"
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "./lib/board-slug"
import { usePreferences } from "./lib/use-preferences"
import { PreferencesProvider } from "./lib/preferences-provider"
import { routePath, useAppRouter } from "./lib/router"
import { useWebRuntime } from "./lib/runtime-context"
import type { WebRuntimeConfig } from "./lib/runtime"
import { astryxMessages, astryxOverrides } from "./lib/i18n"
import { boardSessionRevision, hasActiveBoardSession, reconnectActiveBoardSession, subscribeBoardSessions } from "./features/board/board-session-registry"
import { BoardListReadError, createBoardListQuery, type BoardListReadQuery } from "./lib/api/board-list-read-model"
import type { CanonicalSnapshotHandoffState } from "./App.logic"
import type { BoardCanonicalSnapshot } from "./features/board/board-canonical-snapshot"

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

function boardListRuntimeKey(runtime: WebRuntimeConfig): string {
  return [runtime.apiBaseUrl, runtime.webBasePath, runtime.webBuildId, runtime.serverVersion, runtime.protocolVersion].join("\u0000")
}

function boardListError(reason: unknown): BoardListReadError {
  if (reason instanceof BoardListReadError) return reason
  return new BoardListReadError("anomaly", "看板列表读取失败。", { cause: reason })
}

function isAbortError(reason: unknown): boolean {
  return reason instanceof Error && reason.name === "AbortError"
}

function useBoardListSurface(runtime: WebRuntimeConfig, enabled: boolean): BoardListSurface {
  const runtimeKey = boardListRuntimeKey(runtime)
  const queryRef = useRef<{ readonly key: string; readonly query: BoardListReadQuery } | null>(null)
  let queryState = queryRef.current
  if (queryState === null || queryState.key !== runtimeKey) {
    queryState = { key: runtimeKey, query: createBoardListQuery(runtime, { includeArchived: true }) }
    queryRef.current = queryState
  }
  const query = queryState.query
  const requestRef = useRef(0)
  const [surface, setSurface] = useState<BoardListSurface>(() => ({
    status: enabled ? "loading" : "ready",
    items: [],
    error: null,
    isRefreshing: false,
    onRetry: () => undefined,
  }))

  const load = useCallback((reload: boolean) => {
    if (!enabled) return
    const requestId = ++requestRef.current
    setSurface((current) => ({
      ...current,
      status: current.items.length > 0 ? "ready" : "loading",
      error: null,
      isRefreshing: true,
    }))
    const promise = reload ? query.reload() : query.load()
    void promise.then((items) => {
      if (requestRef.current !== requestId) return
      setSurface((current) => ({ ...current, status: "ready", items, error: null, isRefreshing: false }))
    }).catch((reason: unknown) => {
      if (requestRef.current !== requestId || isAbortError(reason)) return
      const error = boardListError(reason)
      setSurface((current) => ({
        ...current,
        status: error.kind === "offline" ? "offline" : "error",
        error,
        isRefreshing: false,
      }))
    })
  }, [enabled, query])

  const retry = useCallback(() => load(true), [load])

  useEffect(() => {
    if (!enabled) {
      requestRef.current += 1
      query.invalidate()
      setSurface({ status: "ready", items: [], error: null, isRefreshing: false, onRetry: retry })
      return
    }
    load(false)
    return () => {
      requestRef.current += 1
      query.invalidate()
    }
  }, [enabled, load, query, retry, runtimeKey])

  useEffect(() => {
    if (!enabled || typeof window === "undefined") return
    const onFocus = () => load(true)
    window.addEventListener("focus", onFocus)
    return () => window.removeEventListener("focus", onFocus)
  }, [enabled, load])

  return { ...surface, onRetry: retry }
}

function RuntimeThemedShell() {
  const runtime = useWebRuntime()
  const preferences = usePreferences()
  useSyncExternalStore(subscribeBoardSessions, boardSessionRevision, boardSessionRevision)
  const router = useAppRouter({
    basePath: runtime.webBasePath,
    defaultBoard: runtime.defaultBoard,
  })
  const navigate = router.navigate
  const routeRef = useRef(router.route)
  routeRef.current = router.route
  const lastBoardSlugRef = useRef<CanonicalBoardSlug | null>(null)
  const routeBoardSlug = router.route.kind === "board" || router.route.kind === "project-overview" || router.route.kind === "health" || router.route.kind === "maintenance"
    ? router.route.boardSlug
    : null
  if (router.route.kind === "board" || router.route.kind === "health" || router.route.kind === "maintenance") {
    lastBoardSlugRef.current = router.route.boardSlug
  }
  const sessionCandidateSlug = router.route.kind === "board" || router.route.kind === "health" || router.route.kind === "maintenance"
    ? router.route.boardSlug
    : router.route.kind === "settings"
      ? lastBoardSlugRef.current
      : null
  const retainedSessionSlug = sessionCandidateSlug !== null && hasActiveBoardSession(runtime, sessionCandidateSlug)
    ? sessionCandidateSlug
    : null
  const boardList = useBoardListSurface(runtime, true)
  const reloadProjects = boardList.onRetry
  const boardListReady = boardList.status === "ready"
  const boardRouteCandidate = router.route.kind === "board"
    ? router.route
    : (router.route.kind === "health" || router.route.kind === "maintenance") && retainedSessionSlug !== null
      ? { kind: "board" as const, boardSlug: router.route.boardSlug, pathname: routePath({ kind: "board", boardSlug: router.route.boardSlug }, { basePath: runtime.webBasePath }) }
      : router.route.kind === "settings" && retainedSessionSlug !== null
        ? { kind: "board" as const, boardSlug: retainedSessionSlug, pathname: routePath({ kind: "board", boardSlug: retainedSessionSlug }, { basePath: runtime.webBasePath }) }
        : null
  const boardRouteProject = boardRouteCandidate === null
    ? undefined
    : boardList.items.find((item) => item.slug === boardRouteCandidate.boardSlug)
  const boardRoute = boardRouteCandidate !== null && (
    boardRouteProject?.archivedAt === null
    || (!boardListReady && boardRouteProject === undefined)
  )
    ? boardRouteCandidate
    : null
  const retainedSessionAvailable = retainedSessionSlug !== null
  const shellCanonicalBoardSlug = router.route.kind === "settings"
    ? retainedSessionSlug
    : routeBoardSlug
  // The canonical BoardLive remains mounted for every board route as the
  // single session/SSE owner, while Explorer owns the visible board view.
  const liveBoardVisible = false
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
  const [taskMutationState, setTaskMutationState] = useState<{ readonly key: string; readonly surface?: BoardTaskMutationSurface }>(() => ({ key: sessionKey }))
  const [canonicalSnapshotState, setCanonicalSnapshotState] = useState<CanonicalSnapshotHandoffState>(() => ({ key: sessionKey, snapshot: null }))
  const visibleCanonicalReloadRef = useRef<BoardTaskCanonicalReloadHandler | null>(null)
  const [syncStatus, setSyncStatus] = useState<BoardSyncStatus>("connecting")
  const [eventsBatchState, setEventsBatchState] = useState<{ readonly key: string; readonly batch: BoardEventsBatch | null }>(() => ({ key: sessionKey, batch: null }))
  const eventAppliedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const eventAppliedTimerGenerationRef = useRef(0)
  const boundaryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const boundaryTimerGenerationRef = useRef(0)
  const pendingEventsRef = useRef<ExplorerEvent[]>([])
  const pendingBoardIdRef = useRef<CanonicalBoardId | null>(null)
  const pendingBoundaryRef = useRef(false)
  const pendingBoundaryTypesRef = useRef<Set<string>>(new Set())
  const pendingEventInvalidationRef = useRef({ projects: false, board: false, inspector: false, runs: false, fullRefetch: false })
  const pendingEventBoundarySourceRef = useRef(false)
  const dropPendingEventsUntilBoundaryRef = useRef(false)

  const clearEventAppliedTimer = useCallback(() => {
    eventAppliedTimerGenerationRef.current += 1
    if (eventAppliedTimerRef.current !== null) clearTimeout(eventAppliedTimerRef.current)
    eventAppliedTimerRef.current = null
  }, [])

  const clearBoundaryTimer = useCallback(() => {
    boundaryTimerGenerationRef.current += 1
    if (boundaryTimerRef.current !== null) clearTimeout(boundaryTimerRef.current)
    boundaryTimerRef.current = null
  }, [])

  const discardPendingEventBatch = useCallback(() => {
    clearEventAppliedTimer()
    pendingEventsRef.current = []
    pendingBoardIdRef.current = null
    pendingEventInvalidationRef.current = { projects: false, board: false, inspector: false, runs: false, fullRefetch: false }
    pendingEventBoundarySourceRef.current = false
    setEventsBatchState((current) => current.key === sessionKeyRef.current ? { key: current.key, batch: null } : current)
  }, [clearEventAppliedTimer])

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

  const onTaskMutationsChange = useCallback((surface: BoardTaskMutationSurface | undefined, releasedSurface?: BoardTaskMutationSurface) => {
    if (surface === undefined) {
      if (releasedSurface === undefined) return
      setTaskMutationState((current) => {
        // A delayed effect cleanup may belong to an older BoardLive surface;
        // never clear a replacement surface that already owns this key.
        if (current.surface !== releasedSurface) return current
        return { key: current.key, surface: undefined }
      })
      return
    }
    if (sessionKeyRef.current !== sessionKey) return
    setTaskMutationState({ key: sessionKey, surface })
  }, [sessionKey])

  const onCanonicalSnapshotChange = useCallback((snapshot: BoardCanonicalSnapshot | undefined, releasedSnapshot?: BoardCanonicalSnapshot) => {
    setCanonicalSnapshotState((current) => {
      const scoped = current.key === sessionKey ? current : { key: sessionKey, snapshot: null }
      return applyCanonicalSnapshotHandoff(scoped, sessionKey, snapshot, releasedSnapshot)
    })
  }, [sessionKey])

  const onMutationCommitted = useCallback((event: BoardTaskMutationCommitted) => {
    if (sessionKeyRef.current !== sessionKey) return
    if (event.kind !== "create") return
    const boardSlug = parseCanonicalBoardSlug(event.boardSlug)
    if (boardSlug === null) return
    const currentRoute = routeRef.current
    const view = currentRoute.kind === "board" && currentRoute.boardSlug === boardSlug ? currentRoute.view ?? "board" : "board"
    const query = new URLSearchParams(currentRoute.kind === "board" && currentRoute.boardSlug === boardSlug ? currentRoute.query ?? "" : "")
    query.set("task", event.taskId)
    const target = routePath({ kind: "board", boardSlug, view, query: query.toString() }, { basePath: runtime.webBasePath })
    void Promise.resolve(navigate(target)).catch(() => undefined)
  }, [navigate, runtime.webBasePath, sessionKey])

  const onVisibleCanonicalReloadChange = useCallback((reload: BoardTaskCanonicalReloadHandler | undefined, releasedReload?: BoardTaskCanonicalReloadHandler) => {
    if (reload !== undefined) {
      visibleCanonicalReloadRef.current = reload
      return
    }
    if (releasedReload === undefined || visibleCanonicalReloadRef.current === releasedReload) visibleCanonicalReloadRef.current = null
  }, [])

  const onCanonicalReload = useCallback(async (options?: BoardTaskCanonicalReloadOptions) => {
    if (sessionKeyRef.current !== sessionKey) return
    // BoardLive has already awaited the canonical session refresh. Refresh the
    // visible board projection and await the currently mounted Inspector reads
    // through the shared useAsyncRead reload seam.
    bumpExplorerRevision({ board: true, runs: options?.mutationKind === "transition" })
    const reloadVisibleCanonical = visibleCanonicalReloadRef.current
    if (reloadVisibleCanonical !== null) await reloadVisibleCanonical(options)
  }, [bumpExplorerRevision, sessionKey])

  const flushEventBatch = useCallback(() => {
    const pending = pendingEventsRef.current
    const boardId = pendingBoardIdRef.current
    const invalidation = pendingEventInvalidationRef.current
    const boundarySource = pendingEventBoundarySourceRef.current
    pendingEventsRef.current = []
    pendingBoardIdRef.current = null
    pendingEventInvalidationRef.current = { projects: false, board: false, inspector: false, runs: false, fullRefetch: false }
    pendingEventBoundarySourceRef.current = false
    if (pending.length === 0 || boardId === null) return
    try {
      const events = appendExplorerEventBatch([], pending, boardId)
      const last = events.at(-1)
      if (!last) return
      setEventsBatchState({ key: sessionKeyRef.current, batch: { boardId, events, nextAfter: last.id } })
      if (invalidation.projects) reloadProjects()
      if (!pendingBoundaryRef.current && !boundarySource && !invalidation.fullRefetch) {
        bumpExplorerRevision(invalidation)
      }
    } catch {
      // A malformed batch is a recovery boundary; EventsView must not receive
      // an untrusted partial append.
      setEventsBatchState((current) => current.key === sessionKeyRef.current ? { key: current.key, batch: null } : current)
      pendingBoundaryRef.current = true
      pendingBoundaryTypesRef.current.add("protocol-anomaly")
      setSyncStatus("stale")
      if (boundaryTimerRef.current === null) {
        const timerGeneration = ++boundaryTimerGenerationRef.current
        const timerSessionKey = sessionKeyRef.current
        boundaryTimerRef.current = setTimeout(() => {
          if (timerGeneration !== boundaryTimerGenerationRef.current) return
          boundaryTimerRef.current = null
          if (timerSessionKey !== sessionKeyRef.current) return
          if (!pendingBoundaryRef.current) return
          pendingBoundaryRef.current = false
          pendingBoundaryTypesRef.current.clear()
          bumpExplorerRevision({ board: true, inspector: true, runs: true, events: true })
        }, 0)
      }
    }
  }, [bumpExplorerRevision, reloadProjects])

  const scheduleBoundaryRefresh = useCallback((type = "poll-complete") => {
    pendingBoundaryRef.current = true
    pendingBoundaryTypesRef.current.add(type)
    if (eventAppliedTimerRef.current !== null) {
      clearEventAppliedTimer()
      flushEventBatch()
    }
    if (boundaryTimerRef.current !== null) return
    const timerGeneration = ++boundaryTimerGenerationRef.current
    const timerSessionKey = sessionKeyRef.current
    boundaryTimerRef.current = setTimeout(() => {
      if (timerGeneration !== boundaryTimerGenerationRef.current) return
      boundaryTimerRef.current = null
      if (timerSessionKey !== sessionKeyRef.current) return
      if (!pendingBoundaryRef.current) return
      if (eventAppliedTimerRef.current !== null) {
        clearEventAppliedTimer()
        flushEventBatch()
      }
      pendingBoundaryRef.current = false
      const boundary = coalesceExplorerBoundary([...pendingBoundaryTypesRef.current])
      pendingBoundaryTypesRef.current.clear()
      pendingEventsRef.current = []
      pendingBoardIdRef.current = null
      pendingEventInvalidationRef.current = { projects: false, board: false, inspector: false, runs: false, fullRefetch: false }
      pendingEventBoundarySourceRef.current = false
      dropPendingEventsUntilBoundaryRef.current = false
      setEventsBatchState((current) => current.key === sessionKeyRef.current ? { key: current.key, batch: null } : current)
      if (boundary.invalidationDelta === 1) bumpExplorerRevision({ board: true, inspector: true, runs: true, events: boundary.eventsRefreshDelta === 1 })
    }, 0)
  }, [bumpExplorerRevision, clearEventAppliedTimer, flushEventBatch])

  useEffect(() => {
    clearEventAppliedTimer()
    clearBoundaryTimer()
    pendingEventsRef.current = []
    pendingBoardIdRef.current = null
    pendingBoundaryRef.current = false
    pendingBoundaryTypesRef.current.clear()
    pendingEventInvalidationRef.current = { projects: false, board: false, inspector: false, runs: false, fullRefetch: false }
    pendingEventBoundarySourceRef.current = false
    dropPendingEventsUntilBoundaryRef.current = false
    setSyncStatus("connecting")
    setSessionState((current) => current.key === sessionKey ? current : { key: sessionKey, boardRevision: 0, inspectorRevision: 0, runsRevision: 0, eventsRefreshRevision: 0 })
    setTaskMutationState((current) => current.key === sessionKey ? current : { key: sessionKey, surface: undefined })
    setCanonicalSnapshotState((current) => current.key === sessionKey ? current : { key: sessionKey, snapshot: null })
    setEventsBatchState((current) => current.key === sessionKey ? current : { key: sessionKey, batch: null })
  }, [clearBoundaryTimer, clearEventAppliedTimer, sessionKey])

  useEffect(() => () => {
    clearEventAppliedTimer()
    clearBoundaryTimer()
  }, [clearBoundaryTimer, clearEventAppliedTimer])

  const onSessionTelemetry = useCallback((entry: SyncTelemetryEntry) => {
    if (sessionKeyRef.current !== sessionKey) return
    if (!explorerInvalidationTelemetry.has(entry.type)) return
    const nextSyncStatus = boardSyncStatusForTelemetry(entry.type)
    if (nextSyncStatus !== null) setSyncStatus(nextSyncStatus)
    if (entry.type === "connection-live" || entry.type === "recovery-start" || entry.type === "recovery-connection-retry") return
    if (entry.type === "event-applied") {
      if (dropPendingEventsUntilBoundaryRef.current) return
      const event = parseBoardEvent(entry.details?.event)
      if (!event || event.board_id !== entry.boardId) {
        scheduleBoundaryRefresh("protocol-anomaly")
        return
      }
      const invalidation = explorerEventInvalidation(event, entry.boardId)
      if (pendingBoardIdRef.current !== null && pendingBoardIdRef.current !== entry.boardId) flushEventBatch()
      if (shouldRecoverExplorerEventBatch(pendingEventsRef.current.length)) {
        const needsProjectReload = pendingEventInvalidationRef.current.projects || invalidation.projects
        discardPendingEventBatch()
        dropPendingEventsUntilBoundaryRef.current = true
        setSyncStatus("stale")
        if (needsProjectReload) reloadProjects()
        scheduleBoundaryRefresh(EXPLORER_EVENT_BATCH_OVERFLOW_BOUNDARY)
        return
      }
      pendingEventInvalidationRef.current = {
        projects: pendingEventInvalidationRef.current.projects || invalidation.projects,
        board: pendingEventInvalidationRef.current.board || invalidation.board,
        inspector: pendingEventInvalidationRef.current.inspector || invalidation.inspector,
        runs: pendingEventInvalidationRef.current.runs || invalidation.runs,
        fullRefetch: pendingEventInvalidationRef.current.fullRefetch || invalidation.fullRefetch,
      }
      const source = typeof entry.details?.source === "string" ? entry.details.source : null
      if (source === "recovery" || source === "poll" || source === "poll-boundary") pendingEventBoundarySourceRef.current = true
      if (invalidation.fullRefetch) scheduleBoundaryRefresh("protocol-anomaly")
      pendingBoardIdRef.current = entry.boardId
      pendingEventsRef.current.push(event)
      if (eventAppliedTimerRef.current !== null) return
      const timerGeneration = ++eventAppliedTimerGenerationRef.current
      const timerSessionKey = sessionKeyRef.current
      eventAppliedTimerRef.current = setTimeout(() => {
        if (timerGeneration !== eventAppliedTimerGenerationRef.current) return
        eventAppliedTimerRef.current = null
        if (timerSessionKey !== sessionKeyRef.current) return
        flushEventBatch()
      }, EVENT_APPLIED_DEBOUNCE_MS)
      return
    }
    // Recovery/poll completion and protocol anomalies are conservative
    // refresh boundaries; they are intentionally not tied to each event.
    scheduleBoundaryRefresh(entry.type)
  }, [discardPendingEventBatch, flushEventBatch, reloadProjects, scheduleBoundaryRefresh, sessionKey])

  const currentEventsBatch = eventsBatchState.key === sessionKey ? eventsBatchState.batch : null
  const taskMutations = taskMutationState.key === sessionKey ? taskMutationState.surface : undefined
  const canonicalSnapshot = canonicalSnapshotState.key === sessionKey ? canonicalSnapshotState.snapshot : null
  const featureRoute = router.route.kind === "board" && (router.route.view === "signals" || router.route.view === "ontology")
    ? router.route as FeatureRoute
    : null

  return (
    <InternationalizationProvider locale={preferences.locale} messages={astryxMessages} overrides={astryxOverrides}>
      <Theme theme={astryxTheme} mode={preferences.theme}>
        <ProductShell
          runtime={runtime}
          route={router.route}
          canonicalBoardSlug={shellCanonicalBoardSlug ?? undefined}
          boardList={boardList}
          boundary={router.error ? "error" : undefined}
          error={router.error instanceof Error ? router.error.message : undefined}
          onNavigate={router.navigate}
          onReconnect={retainedSessionAvailable ? () => reconnectActiveBoardSession(runtime, retainedSessionSlug) : undefined}
          onRetry={() => window.location.reload()}
          invalidationRevision={sessionState.key === sessionKey ? sessionState.boardRevision : 0}
          boardRevision={sessionState.key === sessionKey ? sessionState.boardRevision : 0}
          inspectorRevision={sessionState.key === sessionKey ? sessionState.inspectorRevision : 0}
          runsRevision={sessionState.key === sessionKey ? sessionState.runsRevision : 0}
          eventsRefreshRevision={sessionState.key === sessionKey ? sessionState.eventsRefreshRevision : 0}
          eventsBatch={currentEventsBatch}
          syncStatus={sessionState.key === sessionKey ? syncStatus : "connecting"}
          taskMutations={taskMutations}
          canonicalSnapshot={canonicalSnapshot}
          onVisibleCanonicalReloadChange={onVisibleCanonicalReloadChange}
        >
          {boardRoute ? (
            <BoardLive
              runtime={runtime}
              route={boardRoute}
              onNavigate={router.navigate}
              renderBoard={liveBoardVisible}
              onSessionTelemetry={onSessionTelemetry}
              onSyncStatusChange={setSyncStatus}
              onTaskMutationsChange={onTaskMutationsChange}
              onMutationCommitted={onMutationCommitted}
              onCanonicalReload={onCanonicalReload}
              onCanonicalSnapshotChange={onCanonicalSnapshotChange}
            />
          ) : null}
          {featureRoute ? <BoardFeatureRoute runtime={runtime} route={featureRoute} onNavigate={router.navigate} /> : null}
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

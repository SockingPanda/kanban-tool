import { useLayoutEffect } from "react";
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react"

import { ProductShell } from "./shell/app-shell"
// 功能只公开显式出口；Vite 静态树摇保留所用导出；最小复现见 build/react-doctor-regressions.test.ts。
// react-doctor-disable-next-line react-doctor/no-barrel-import
import { BoardLive } from "../features/tasks/index"
import { boardSyncStatusForTelemetry } from "../application/workspace/board-live-state"
import type { BoardTaskCanonicalReloadHandler, BoardTaskCanonicalReloadOptions, BoardTaskMutationCommitted, BoardTaskMutationSurface } from "../application/tasks/task-mutation-state"
import type { BoardSyncStatus } from "../domain/tasks/board"
import { appendExplorerEventBatch, classifyExplorerSessionTelemetry, coalesceExplorerBoundary, explorerEventInvalidation } from "../application/workspace/session-events"
import { parseBoardEvent, type BoardEventsBatch, type ExplorerEvent } from "../application/data/explorer-read-model";
import type { SyncTelemetryEntry } from "../application/sync/contracts";
import type { CanonicalBoardId } from "../application/sync/contracts"
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "../domain/board-slug"
import { PreferencesProvider } from "../platform/preferences/preferences-provider"
import { routePath, useAppRouter } from "../application/navigation/router"
import { useWebRuntime } from "../lib/runtime-context"
import { boardSessionRevision, hasActiveBoardSession, reconnectActiveBoardSession, subscribeBoardSessions } from "../application/workspace/board-session-registry"

const EVENT_APPLIED_DEBOUNCE_MS = 200

function useRuntimeThemedShellState() {
  const runtime = useWebRuntime()
  useSyncExternalStore(subscribeBoardSessions, boardSessionRevision, boardSessionRevision)
  const router = useAppRouter({
    basePath: runtime.webBasePath,
    defaultBoard: runtime.defaultBoard,
  })
  const navigate = router.navigate
  const routeRef = useRef(router.route)
  useLayoutEffect(() => { routeRef.current = router.route });
  const lastBoardSlugRef = useRef<CanonicalBoardSlug | null>(null)
  const routeBoardSlug = router.route.kind === "board" || router.route.kind === "health" || router.route.kind === "maintenance"
    ? router.route.boardSlug
    : null
  useLayoutEffect(() => { if (routeBoardSlug !== null) lastBoardSlugRef.current = routeBoardSlug });
  const retainedBoardSlug = routeBoardSlug ?? lastBoardSlugRef.current
  const retainedSessionSlug = retainedBoardSlug !== null && hasActiveBoardSession(runtime, retainedBoardSlug)
    ? retainedBoardSlug
    : null
  const boardRoute = router.route.kind === "home" || router.route.kind === "board"
    ? router.route
    : retainedSessionSlug !== null
      ? { kind: "board" as const, boardSlug: retainedSessionSlug, pathname: routePath({ kind: "board", boardSlug: retainedSessionSlug }, { basePath: runtime.webBasePath }) }
      : null
  // The canonical BoardLive remains mounted for every board route as the
  // single session/SSE owner, while Explorer owns the visible board view.
  const liveBoardVisible = router.route.kind === "home"
  const sessionKey = boardRoute === null
    ? "none"
    : `${runtime.apiBaseUrl}\u0000${runtime.webBasePath}\u0000${runtime.webBuildId}\u0000${boardRoute.kind === "board" ? boardRoute.boardSlug : ""}`
  const sessionKeyRef = useRef(sessionKey)
  useLayoutEffect(() => { sessionKeyRef.current = sessionKey });
  const [sessionState, setSessionState] = useState<{ readonly key: string; readonly boardRevision: number; readonly inspectorRevision: number; readonly runsRevision: number; readonly eventsRefreshRevision: number }>(() => ({
    key: sessionKey,
    boardRevision: 0,
    inspectorRevision: 0,
    runsRevision: 0,
    eventsRefreshRevision: 0,
  }))
  const [taskMutationState, setTaskMutationState] = useState<{ readonly key: string; readonly surface?: BoardTaskMutationSurface }>(() => ({ key: sessionKey }))
  const visibleCanonicalReloadRef = useRef<BoardTaskCanonicalReloadHandler | null>(null)
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
    // 项目会话与当前页独立回读，返回当前分页模型供看板拖动协调结果。
    bumpExplorerRevision({ board: true, runs: options?.mutationKind === "transition" })
    const reloadVisibleCanonical = visibleCanonicalReloadRef.current
    if (reloadVisibleCanonical !== null) return await reloadVisibleCanonical(options)
  }, [bumpExplorerRevision, sessionKey])

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
    setTaskMutationState((current) => current.key === sessionKey ? current : { key: sessionKey, surface: undefined })
    setEventsBatchState((current) => current.key === sessionKey ? current : { key: sessionKey, batch: null })
  }, [sessionKey])

  useEffect(() => () => {
    if (eventAppliedTimerRef.current !== null) clearTimeout(eventAppliedTimerRef.current)
    if (boundaryTimerRef.current !== null) clearTimeout(boundaryTimerRef.current)
  }, [])

  const onSessionTelemetry = useCallback((entry: SyncTelemetryEntry) => {
    const kind = classifyExplorerSessionTelemetry(entry.type)
    if (kind === null) return
    const nextSyncStatus = boardSyncStatusForTelemetry(entry.type)
    if (nextSyncStatus !== null) setSyncStatus(nextSyncStatus)
    if (kind === "state") return
    if (kind === "event") {
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
  const taskMutations = taskMutationState.key === sessionKey ? taskMutationState.surface : undefined

  return {
    runtime,
    router,
    retainedBoardSlug,
    retainedSessionSlug,
    sessionState,
    sessionKey,
    currentEventsBatch,
    syncStatus,
    taskMutations,
    onVisibleCanonicalReloadChange,
    boardRoute,
    liveBoardVisible,
    onSessionTelemetry,
    setSyncStatus,
    onTaskMutationsChange,
    onMutationCommitted,
    onCanonicalReload
  };
}

function RuntimeThemedShell() {
  const {
    runtime,
    router,
    retainedBoardSlug,

    retainedSessionSlug,
    sessionState,
    sessionKey,
    currentEventsBatch,
    syncStatus,
    taskMutations,
    onVisibleCanonicalReloadChange,
    boardRoute,
    liveBoardVisible,
    onSessionTelemetry,
    setSyncStatus,
    onTaskMutationsChange,
    onMutationCommitted,
    onCanonicalReload
  } = useRuntimeThemedShellState();
  return (
    <>
        <ProductShell
          runtime={runtime}
          route={router.route}
          canonicalBoardSlug={retainedBoardSlug ?? undefined}
          boundary={router.error ? "error" : undefined}
          error={router.error instanceof Error ? router.error.message : undefined}
          onNavigate={router.navigate}
          onReconnect={retainedSessionSlug !== null ? () => reconnectActiveBoardSession(runtime, retainedSessionSlug) : undefined}
          onRetry={() => window.location.reload()}
          invalidationRevision={sessionState.key === sessionKey ? sessionState.boardRevision : 0}
          boardRevision={sessionState.key === sessionKey ? sessionState.boardRevision : 0}
          inspectorRevision={sessionState.key === sessionKey ? sessionState.inspectorRevision : 0}
          runsRevision={sessionState.key === sessionKey ? sessionState.runsRevision : 0}
          eventsRefreshRevision={sessionState.key === sessionKey ? sessionState.eventsRefreshRevision : 0}
          eventsBatch={currentEventsBatch}
          syncStatus={sessionState.key === sessionKey ? syncStatus : "connecting"}
          taskMutations={taskMutations}
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
            />
          ) : null}
        </ProductShell>
    </>
  )
}


export default function App() {
  return (
    <PreferencesProvider>
      <RuntimeThemedShell />
    </PreferencesProvider>
  )
}

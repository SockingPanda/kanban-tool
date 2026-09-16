import { useLayoutEffect } from "react";
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react"

import { ProductShell } from "./shell/app-shell"
// 功能只公开显式出口；Vite 静态树摇保留所用导出；最小复现见 build/react-doctor-regressions.test.ts。
// react-doctor-disable-next-line react-doctor/no-barrel-import
import { BoardLive } from "../features/tasks/index"
import type { BoardTaskCanonicalReloadHandler, BoardTaskCanonicalReloadOptions, BoardTaskMutationCommitted, BoardTaskMutationSurface } from "../application/tasks/task-mutation-state"
import type { BoardSyncStatus } from "../domain/tasks/board"
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "../domain/board-slug"
import { PreferencesProvider } from "../platform/preferences/preferences-provider"
import { installPageFocusRecovery } from "../platform/browser/page-focus-recovery"
import { routePath, useAppRouter } from "../application/navigation/router"
import { useWebRuntime } from "../lib/runtime-context"
import { boardSessionRevision, hasActiveBoardSession, reconnectActiveBoardSession, subscribeBoardSessions } from "../application/workspace/board-session-registry"

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
  // BoardLive 持有项目身份与写入能力；可见模型共用数据源的完整查询。
  const liveBoardVisible = router.route.kind === "home"
  const sessionKey = boardRoute === null
    ? "none"
    : `${runtime.apiBaseUrl}\u0000${runtime.webBasePath}\u0000${runtime.webBuildId}\u0000${boardRoute.kind === "board" ? boardRoute.boardSlug : ""}`
  const sessionKeyRef = useRef(sessionKey)
  useLayoutEffect(() => { sessionKeyRef.current = sessionKey });
  const [taskMutationState, setTaskMutationState] = useState<{ readonly key: string; readonly surface?: BoardTaskMutationSurface }>(() => ({ key: sessionKey }))
  const visibleCanonicalReloadRef = useRef<BoardTaskCanonicalReloadHandler | null>(null)
  const [syncSnapshot, setSyncSnapshot] = useState({ key: sessionKey, status: 'connecting' as BoardSyncStatus })
  const syncStatus = syncSnapshot.key === sessionKey ? syncSnapshot.status : 'connecting'
  const setSyncStatus = useCallback((status: BoardSyncStatus) => setSyncSnapshot({ key: sessionKeyRef.current, status }), [])

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
    const reloadVisibleCanonical = visibleCanonicalReloadRef.current
    if (reloadVisibleCanonical !== null) return await reloadVisibleCanonical(options)
  }, [sessionKey])

  const taskMutations = taskMutationState.key === sessionKey ? taskMutationState.surface : undefined

  return {
    runtime,
    router,
    retainedBoardSlug,
    retainedSessionSlug,
    syncStatus,
    taskMutations,
    onVisibleCanonicalReloadChange,
    boardRoute,
    liveBoardVisible,
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
    syncStatus,
    taskMutations,
    onVisibleCanonicalReloadChange,
    boardRoute,
    liveBoardVisible,
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
          syncStatus={syncStatus}
          taskMutations={taskMutations}
          onVisibleCanonicalReloadChange={onVisibleCanonicalReloadChange}
        >
          {boardRoute ? (
            <BoardLive
              runtime={runtime}
              route={boardRoute}
              onNavigate={router.navigate}
              renderBoard={liveBoardVisible}
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
  useEffect(() => installPageFocusRecovery(window), [])
  return (
    <PreferencesProvider>
      <RuntimeThemedShell />
    </PreferencesProvider>
  )
}

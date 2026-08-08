import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import type { AppNavigationTarget, AppRoute } from "../../lib/router"
import { parseCanonicalBoardSlug } from "../../lib/board-slug"
import {
  BoardReadError,
  createBoardReadQuery,
} from "../../lib/api/board-read-model"
import { createHttpTransport } from "../../lib/api/http-transport"
import { createTranslator } from "../../lib/i18n"
import { usePreferences } from "../../lib/use-preferences"
import { createGeneratedStreamContractAdapter } from "../../lib/sync"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { BoardView } from "./BoardView"
import {
  boardMessagesForLocale,
  type BoardSyncStatus,
  type BoardViewModel,
  type BoardViewState,
} from "./types"
import { toBoardViewModel } from "./board-adapter"
import { boardSyncStatusForTelemetry } from "./board-live-state"
import {
  acquireBoardSession,
  type BoardReadResource,
} from "./board-session-registry"

type BoardRoute = Extract<AppRoute, { kind: "home" | "board" }>

export interface BoardLiveProps {
  readonly runtime: WebRuntimeConfig
  readonly route: BoardRoute
  readonly onNavigate: (target: AppNavigationTarget, options?: { replace?: boolean }) => void | Promise<unknown>
}

function makeResource(runtime: WebRuntimeConfig, selector: string): BoardReadResource {
  const transport = createHttpTransport(runtime)
  const query = createBoardReadQuery(runtime, selector, { dependencies: { transport } })
  return {
    selector,
    transport,
    query,
    adapter: createGeneratedStreamContractAdapter(),
  }
}

function errorState(error: unknown, translator: ReturnType<typeof createTranslator>): BoardViewState {
  if (error instanceof BoardReadError && error.reason === "no-boards") {
    return { kind: "empty", detail: translator("boardNoBoardsDescription") }
  }
  if (error instanceof BoardReadError && error.kind === "offline") {
    return { kind: "offline", message: translator("boardOfflineDescription") }
  }
  return { kind: "error", message: translator("boardLoadErrorDescription") }
}

function modelForRoute(model: BoardViewModel | null, route: BoardRoute): boolean {
  return route.kind === "board" && model?.board.slug === route.boardSlug
}

export function BoardLive({ runtime, route, onNavigate }: BoardLiveProps) {
  const preferences = usePreferences()
  const translator = useMemo(() => createTranslator(preferences.locale), [preferences.locale])
  const boardMessages = boardMessagesForLocale(preferences.locale)
  const selector = route.kind === "board" ? route.boardSlug : runtime.defaultBoard
  const resourcesRef = useRef(new Map<string, BoardReadResource>())
  const modelRef = useRef<BoardViewModel | null>(null)
  const resourceRef = useRef<BoardReadResource | null>(null)
  const loadAbortRef = useRef<AbortController | null>(null)
  const retryRequestedRef = useRef(false)
  const activeRef = useRef(true)
  const redirectedBoardRef = useRef<string | null>(null)
  const sessionRetryRef = useRef<(() => void) | null>(null)
  const [retryVersion, setRetryVersion] = useState(0)
  const [state, setState] = useState<BoardViewState>({ kind: "loading" })
  const [syncStatus, setSyncStatus] = useState<BoardSyncStatus>("connecting")

  useEffect(() => {
    activeRef.current = true
    const resources = resourcesRef.current
    return () => {
      activeRef.current = false
      loadAbortRef.current?.abort()
      loadAbortRef.current = null
      for (const resource of resources.values()) resource.query.invalidate()
      resources.clear()
    }
  }, [])

  useEffect(() => {
    const retained = modelRef.current
    const retryRequested = retryRequestedRef.current
    retryRequestedRef.current = false
    if (!retryRequested && retained !== null && modelForRoute(retained, route)) {
      setState({ kind: "ready", model: retained })
      return
    }

    const resourceKey = `${runtime.apiBaseUrl}\u0000${runtime.webBasePath}\u0000${runtime.webBuildId}\u0000${selector}`
    let resource = resourcesRef.current.get(resourceKey)
    if (resource === undefined) {
      try {
        resource = makeResource(runtime, selector)
        resourcesRef.current.set(resourceKey, resource)
      } catch (error) {
        setState(errorState(error, translator))
        return
      }
    }
    resourceRef.current = resource
    for (const [key, cached] of resourcesRef.current) {
      if (key !== resourceKey) {
        cached.query.invalidate()
        resourcesRef.current.delete(key)
      }
    }
    const abort = new AbortController()
    loadAbortRef.current?.abort()
    loadAbortRef.current = abort
    let current = true
    const preserveReadyBoard = retryRequested && retained !== null && modelForRoute(retained, route)
    if (!preserveReadyBoard) setState({ kind: "loading" })

    const read = retryRequested ? resource.query.reload(abort.signal) : resource.query.load(abort.signal)
    void read.then(
      (readModel) => {
        if (!current || !activeRef.current || abort.signal.aborted) return
        try {
          const viewModel = toBoardViewModel(readModel)
          modelRef.current = viewModel
          setState({ kind: "ready", model: viewModel })
        } catch (error) {
          if (preserveReadyBoard && retained !== null) {
            setState({ kind: "ready", model: retained })
            setSyncStatus("stale")
          } else setState(errorState(error, translator))
        }
      },
      (error: unknown) => {
        if (!current || !activeRef.current || abort.signal.aborted) return
        if (preserveReadyBoard && retained !== null) {
          setState({ kind: "ready", model: retained })
          setSyncStatus(boardSyncStatusForTelemetry(error instanceof BoardReadError && error.kind === "offline" ? "transport-failure" : "recovery-failure") ?? "stale")
        } else setState(errorState(error, translator))
      },
    )

    return () => {
      current = false
      abort.abort()
      if (loadAbortRef.current === abort) loadAbortRef.current = null
    }
  }, [retryVersion, route, runtime, selector, translator])

  useEffect(() => {
    setSyncStatus("connecting")
  }, [selector])

  useEffect(() => {
    if (route.kind !== "home" || state.kind !== "ready") return
    const slug = parseCanonicalBoardSlug(state.model.board.slug)
    if (slug === null) return
    if (redirectedBoardRef.current === slug) return
    redirectedBoardRef.current = slug
    void Promise.resolve(onNavigate({ kind: "board", boardSlug: slug }, { replace: true })).catch(() => {
      redirectedBoardRef.current = null
    })
  }, [onNavigate, route.kind, state])

  const canonicalBoardId = state.kind === "ready" ? state.model.board.id : null
  useEffect(() => {
    if (canonicalBoardId === null || resourceRef.current === null || modelRef.current === null) return
    const handle = acquireBoardSession(
      runtime,
      modelRef.current,
      resourceRef.current,
      (readModel) => {
        if (!activeRef.current) return
        try {
          const viewModel = toBoardViewModel(readModel)
          modelRef.current = viewModel
          setState({ kind: "ready", model: viewModel })
        } catch {
          setSyncStatus("stale")
        }
      },
      (entry) => {
        if (!activeRef.current) return
        const nextStatus = boardSyncStatusForTelemetry(entry.type)
        if (nextStatus !== null) setSyncStatus(nextStatus)
      },
    )
    sessionRetryRef.current = handle.retry
    return () => {
      handle.release()
      if (sessionRetryRef.current === handle.retry) sessionRetryRef.current = null
    }
  }, [canonicalBoardId, runtime])

  useEffect(() => {
    if (state.kind !== "ready") return
    const onOffline = () => setSyncStatus("stale")
    const onOnline = () => {
      setSyncStatus("recovering")
      sessionRetryRef.current?.()
    }
    window.addEventListener("offline", onOffline)
    window.addEventListener("online", onOnline)
    return () => {
      window.removeEventListener("offline", onOffline)
      window.removeEventListener("online", onOnline)
    }
  }, [canonicalBoardId, state.kind])

  const retry = useCallback(() => {
    const circuitOpen = syncStatus === "circuit-open"
    setSyncStatus("recovering")
    sessionRetryRef.current?.()
    if (!circuitOpen) {
      retryRequestedRef.current = true
      setRetryVersion((version) => version + 1)
    }
  }, [syncStatus])

  return (
    <BoardView
      state={state}
      messages={boardMessages}
      syncStatus={state.kind === "ready" ? syncStatus : undefined}
      onRetry={retry}
      id="astryx-board"
    />
  )
}

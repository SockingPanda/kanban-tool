import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import type { AppNavigationTarget, AppRoute } from "../../lib/router"
import { parseCanonicalBoardSlug } from "../../lib/board-slug"
import {
  BoardReadError,
  createBoardReadQuery,
  type BoardReadModel,
} from "../../lib/api/board-read-model"
import { createHttpTransport } from "../../lib/api/http-transport"
import { createTaskMutationClient } from "../../lib/api/task-mutations"
import { createTranslator } from "../../lib/i18n"
import { usePreferences } from "../../lib/use-preferences"
import { createGeneratedStreamContractAdapter, asCanonicalBoardId, type SyncTelemetryEntry } from "../../lib/sync"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { BoardView } from "./BoardView"
import {
  boardMessagesForLocale,
  type BoardSyncStatus,
  type BoardViewModel,
  type BoardViewState,
} from "./types"
import { toBoardViewModel } from "./board-adapter"
import { boardSyncStatusForTelemetry, subscribeBrowserConnectivity } from "./board-live-state"
import {
  createBoardTaskClaimTokenStore,
  type BoardTaskCanonicalReloadOptions,
  type BoardTaskMutationCommitted,
  type BoardTaskMutationSurface,
} from "./task-mutation-state"
import {
  acquireBoardSession,
  bindBoardResourceIdentity,
  resourceIdentityKey,
  routeResourceContextKey,
  runtimeIdentityKey,
  type BoardReadResource,
  type BoardSessionHandle,
} from "./board-session-registry"

type BoardRoute = Extract<AppRoute, { kind: "board" }>

export interface BoardLiveProps {
  readonly runtime: WebRuntimeConfig
  readonly route: BoardRoute
  readonly onNavigate: (target: AppNavigationTarget, options?: { replace?: boolean }) => void | Promise<unknown>
  /** Keep the canonical session mounted while Explorer owns the visible route. */
  readonly renderBoard?: boolean
  /** Existing fenced telemetry seam for Explorer/Events invalidation. */
  readonly onSessionTelemetry?: (entry: SyncTelemetryEntry) => void
  /** Propagate browser connectivity changes to the App-level Explorer status. */
  readonly onSyncStatusChange?: (status: BoardSyncStatus) => void
  /** Expose the canonical session's typed mutation surface to the rendered Explorer board. */
  readonly onTaskMutationsChange?: (surface: BoardTaskMutationSurface | undefined, releasedSurface?: BoardTaskMutationSurface) => void
  /** Report a committed mutation so the App can invalidate Explorer readers/navigation. */
  readonly onMutationCommitted?: (event: BoardTaskMutationCommitted) => void
  /** Await visible Explorer readers after the canonical session has reloaded. */
  readonly onCanonicalReload?: (options?: BoardTaskCanonicalReloadOptions) => Promise<void> | void
}

function makeResource(runtime: WebRuntimeConfig, selector: string): BoardReadResource {
  const transport = createHttpTransport(runtime)
  const query = createBoardReadQuery(runtime, selector, { dependencies: { transport } })
  return {
    selector,
    transport,
    query,
    adapter: createGeneratedStreamContractAdapter(),
    runtimeKey: runtimeIdentityKey(runtime),
    identityKey: resourceIdentityKey(runtime, selector, null),
    canonicalBoardId: null,
    resolvedSlug: null,
    sessionGeneration: 0,
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

function modelIdentityKey(model: BoardViewModel, runtime: WebRuntimeConfig, selector: string): string | null {
  try {
    return resourceIdentityKey(runtime, selector, asCanonicalBoardId(model.board.id))
  } catch {
    return null
  }
}

/** Route and resource identity are checked together at every render seam. */
function modelMatchesRoute(
  model: BoardViewModel | null,
  identityKey: string | null,
  runtime: WebRuntimeConfig,
  selector: string,
  routeKind: BoardRoute["kind"],
  routeBoardSlug = "",
): boolean {
  if (model === null || identityKey === null || modelIdentityKey(model, runtime, selector) !== identityKey) return false
  return routeKind !== "board" || model.board.slug === routeBoardSlug
}

function findResource(
  resources: Map<string, BoardReadResource>,
  runtime: WebRuntimeConfig,
  selector: string,
): BoardReadResource | undefined {
  const provisionalKey = resourceIdentityKey(runtime, selector, null)
  const direct = resources.get(provisionalKey)
  if (direct !== undefined) return direct
  const runtimeKey = runtimeIdentityKey(runtime)
  for (const resource of resources.values()) {
    if (resource.runtimeKey === runtimeKey && resource.selector === selector) return resource
  }
  return undefined
}

function retainResourceKey(resources: Map<string, BoardReadResource>, resource: BoardReadResource): void {
  for (const [key, value] of resources) {
    if (value === resource && key !== resource.identityKey) resources.delete(key)
  }
  resources.set(resource.identityKey, resource)
}

export function BoardLive({ runtime, route, renderBoard = true, onSessionTelemetry, onSyncStatusChange, onTaskMutationsChange, onMutationCommitted, onCanonicalReload }: BoardLiveProps) {
  const preferences = usePreferences()
  const translator = useMemo(() => createTranslator(preferences.locale), [preferences.locale])
  const boardMessages = boardMessagesForLocale(preferences.locale)
  const selector = route.boardSlug
  const contextKey = routeResourceContextKey(runtime, selector, route.kind, route.boardSlug)
  const routeBoardSlug = route.boardSlug
  const resourcesRef = useRef(new Map<string, BoardReadResource>())
  const modelRef = useRef<BoardViewModel | null>(null)
  const resourceRef = useRef<BoardReadResource | null>(null)
  const loadAbortRef = useRef<AbortController | null>(null)
  const retryRequestedRef = useRef(false)
  const activeRef = useRef(true)
  const activeContextRef = useRef(contextKey)
  const stateContextKeyRef = useRef(contextKey)
  const readyResourceKeyRef = useRef<string | null>(null)
  const sessionHandleRef = useRef<BoardSessionHandle | null>(null)
  const sessionRetryRef = useRef<(() => void) | null>(null)
  const claimTokenStoreRef = useRef(createBoardTaskClaimTokenStore())
  const claimTokenBoardRef = useRef<string | null>(null)
  const [retryVersion, setRetryVersion] = useState(0)
  const [state, setState] = useState<BoardViewState>({ kind: "loading" })
  const [syncStatus, setSyncStatus] = useState<BoardSyncStatus>("connecting")
  const reportSyncStatus = useCallback((status: BoardSyncStatus) => {
    setSyncStatus(status)
    onSyncStatusChange?.(status)
  }, [onSyncStatusChange])

  // This render-time fence closes the A → B gap before effects have a chance to run.
  activeContextRef.current = contextKey

  useEffect(() => {
    activeRef.current = true
    const resources = resourcesRef.current
    return () => {
      activeRef.current = false
      loadAbortRef.current?.abort()
      loadAbortRef.current = null
      sessionHandleRef.current?.release()
      sessionHandleRef.current = null
      sessionRetryRef.current = null
      // BoardLive owns only its request abort. Registry-owned queries stay alive
      // while another mount still references their canonical session.
      resources.clear()
    }
  }, [])

  useEffect(() => {
    const contextAtStart = contextKey
    const retained = modelRef.current
    const retryRequested = retryRequestedRef.current
    retryRequestedRef.current = false

    let resource = findResource(resourcesRef.current, runtime, selector)
    if (resource === undefined) {
      try {
        resource = makeResource(runtime, selector)
        resourcesRef.current.set(resource.identityKey, resource)
      } catch (error) {
        stateContextKeyRef.current = contextAtStart
        setState(errorState(error, translator))
        return
      }
    }
    resourceRef.current = resource

    const retainedIdentityKey = retained === null ? null : modelIdentityKey(retained, runtime, selector)
    const retainedForRoute = modelMatchesRoute(retained, readyResourceKeyRef.current, runtime, selector, route.kind, routeBoardSlug)
    if (!retryRequested && retained !== null && retainedForRoute && retainedIdentityKey === readyResourceKeyRef.current) {
      stateContextKeyRef.current = contextAtStart
      setState({ kind: "ready", model: retained })
      return
    }

    const abort = new AbortController()
    loadAbortRef.current?.abort()
    loadAbortRef.current = abort
    let current = true
    const preserveReadyBoard = retryRequested && retainedForRoute && retained !== null
    if (!preserveReadyBoard) {
      stateContextKeyRef.current = contextAtStart
      setState({ kind: "loading" })
    }

    const read = retryRequested ? resource.query.reload(abort.signal) : resource.query.load(abort.signal)
    void read.then(
      (readModel) => {
        if (!current || !activeRef.current || abort.signal.aborted || activeContextRef.current !== contextAtStart) return
        try {
          const viewModel = toBoardViewModel(readModel)
          const candidateIdentityKey = modelIdentityKey(viewModel, runtime, selector)
          if (!modelMatchesRoute(viewModel, candidateIdentityKey, runtime, selector, route.kind, routeBoardSlug)) {
            if (preserveReadyBoard && retained !== null) {
              stateContextKeyRef.current = contextAtStart
              setState({ kind: "ready", model: retained })
              setSyncStatus("stale")
            } else {
              stateContextKeyRef.current = contextAtStart
              setState(errorState(new BoardReadError("anomaly", "board read identity mismatch"), translator))
            }
            return
          }
          const identityKey = bindBoardResourceIdentity(runtime, resource!, readModel)
          retainResourceKey(resourcesRef.current, resource!)
          readyResourceKeyRef.current = identityKey
          modelRef.current = viewModel
          stateContextKeyRef.current = contextAtStart
          setState({ kind: "ready", model: viewModel })
        } catch (error) {
          if (preserveReadyBoard && retained !== null) {
            stateContextKeyRef.current = contextAtStart
            setState({ kind: "ready", model: retained })
            setSyncStatus("stale")
          } else {
            stateContextKeyRef.current = contextAtStart
            setState(errorState(error, translator))
          }
        }
      },
      (error: unknown) => {
        if (!current || !activeRef.current || abort.signal.aborted || activeContextRef.current !== contextAtStart) return
        if (preserveReadyBoard && retained !== null) {
          stateContextKeyRef.current = contextAtStart
          setState({ kind: "ready", model: retained })
          setSyncStatus(boardSyncStatusForTelemetry(error instanceof BoardReadError && error.kind === "offline" ? "transport-failure" : "recovery-failure") ?? "stale")
        } else {
          stateContextKeyRef.current = contextAtStart
          setState(errorState(error, translator))
        }
      },
    )

    return () => {
      current = false
      abort.abort()
      if (loadAbortRef.current === abort) loadAbortRef.current = null
    }
  }, [contextKey, retryVersion, route.kind, routeBoardSlug, runtime, selector, translator])

  useEffect(() => {
    setSyncStatus("connecting")
  }, [contextKey])

  const contextState = stateContextKeyRef.current === contextKey ? state : null
  const visibleState = useMemo<BoardViewState>(() => {
    if (contextState === null) return { kind: "loading" }
    if (contextState.kind === "ready" && !modelMatchesRoute(contextState.model, readyResourceKeyRef.current, runtime, selector, route.kind, routeBoardSlug)) {
      return { kind: "loading" }
    }
    return contextState
  }, [contextState, route.kind, routeBoardSlug, runtime, selector])
  const visibleStateKind = visibleState.kind
  const visibleBoardSlug = visibleState.kind === "ready" ? visibleState.model.board.slug : null
  const mutationBoardSlug = visibleBoardSlug === null ? null : parseCanonicalBoardSlug(visibleBoardSlug)

  useEffect(() => {
    if (claimTokenBoardRef.current !== mutationBoardSlug) {
      claimTokenStoreRef.current.clear()
      claimTokenBoardRef.current = mutationBoardSlug
    }
  }, [mutationBoardSlug])

  const refreshCanonical = useCallback(async () => {
    const handle = sessionHandleRef.current
    if (handle === null) throw new Error("canonical board session is unavailable")
    await handle.refresh()
    return modelRef.current
  }, [])

  const taskMutations = useMemo<BoardTaskMutationSurface | undefined>(() => {
    if (mutationBoardSlug === null) return undefined
    try {
      const client = createTaskMutationClient(runtime, mutationBoardSlug, { actor: preferences.actor || runtime.actor })
      return {
        client,
        claimTokens: claimTokenStoreRef.current,
        inspectorClient: client,
        resolveTaskSelector: (selector) => {
          const value = selector.trim()
          if (!value) return null
          const model = modelRef.current
          if (model === null) return null
          for (const tasks of Object.values(model.tasksByStatus)) {
            const candidate = tasks.find((task) => task.id === value || task.ref === value)
            if (candidate !== undefined) return candidate.id
          }
          return null
        },
        onCanonicalReload: async (options) => {
          const canonical = await refreshCanonical()
          await onCanonicalReload?.(options)
          return canonical
        },
        onMutationCommitted: (event) => onMutationCommitted?.({ ...event, boardSlug: mutationBoardSlug }),
      }
    } catch {
      return undefined
    }
  }, [mutationBoardSlug, onCanonicalReload, onMutationCommitted, preferences.actor, refreshCanonical, runtime])

  useEffect(() => {
    onTaskMutationsChange?.(taskMutations)
    return () => onTaskMutationsChange?.(undefined, taskMutations)
  }, [onTaskMutationsChange, taskMutations])

  const canonicalBoardId = visibleState.kind === "ready" ? visibleState.model.board.id : null
  useEffect(() => {
    if (
      canonicalBoardId === null
      || visibleStateKind !== "ready"
      || stateContextKeyRef.current !== contextKey
      || resourceRef.current === null
    ) return
    const resource = resourceRef.current
    const model = modelRef.current
    if (model === null || !modelMatchesRoute(model, readyResourceKeyRef.current, runtime, selector, route.kind, routeBoardSlug)) return
    const sessionIdentityKey = resource.identityKey
    let sessionGeneration: number | null = null
    let handle: BoardSessionHandle
    try {
      handle = acquireBoardSession(
        runtime,
        model,
        resource,
        (readModel: BoardReadModel) => {
          if (
            !activeRef.current
            || activeContextRef.current !== contextKey
            || stateContextKeyRef.current !== contextKey
            || resourceRef.current !== resource
            || resource.identityKey !== sessionIdentityKey
            || sessionGeneration === null
            || resource.sessionGeneration !== sessionGeneration
          ) return
          try {
            const viewModel = toBoardViewModel(readModel)
            const candidateIdentityKey = modelIdentityKey(viewModel, runtime, selector)
            if (candidateIdentityKey !== sessionIdentityKey || candidateIdentityKey !== readyResourceKeyRef.current || !modelMatchesRoute(viewModel, candidateIdentityKey, runtime, selector, route.kind, routeBoardSlug)) return
            retainResourceKey(resourcesRef.current, resource)
            modelRef.current = viewModel
            stateContextKeyRef.current = contextKey
            setState({ kind: "ready", model: viewModel })
          } catch {
            setSyncStatus("stale")
          }
        },
        (entry) => {
          if (
            !activeRef.current
            || activeContextRef.current !== contextKey
            || stateContextKeyRef.current !== contextKey
            || resourceRef.current !== resource
            || resource.identityKey !== sessionIdentityKey
            || sessionGeneration === null
            || resource.sessionGeneration !== sessionGeneration
          ) return
          const nextStatus = boardSyncStatusForTelemetry(entry.type)
          if (nextStatus !== null) setSyncStatus(nextStatus)
          onSessionTelemetry?.(entry)
        },
      )
    } catch {
      return
    }
    sessionGeneration = handle.generation
    sessionHandleRef.current = handle
    sessionRetryRef.current = handle.retry
    return () => {
      handle.release()
      if (sessionHandleRef.current === handle) {
        sessionHandleRef.current = null
        sessionRetryRef.current = null
      }
    }
  }, [canonicalBoardId, contextKey, onSessionTelemetry, route.kind, routeBoardSlug, runtime, selector, visibleStateKind])

  useEffect(() => {
    const onOffline = () => reportSyncStatus("offline")
    const onOnline = () => {
      reportSyncStatus("recovering")
      sessionRetryRef.current?.()
    }
    if (typeof window !== "undefined" && !window.navigator.onLine) onOffline()
    return subscribeBrowserConnectivity(window, onOffline, onOnline)
  }, [contextKey, reportSyncStatus])

  const retry = useCallback(() => {
    setSyncStatus("recovering")
    sessionRetryRef.current?.()
    retryRequestedRef.current = true
    setRetryVersion((version) => version + 1)
  }, [])

  if (!renderBoard) return null

  return (
    <BoardView
      state={visibleState}
      messages={boardMessages}
      syncStatus={visibleState.kind === "ready" ? syncStatus : undefined}
      onRetry={retry}
      taskMutations={taskMutations}
      id="astryx-board"
    />
  )
}

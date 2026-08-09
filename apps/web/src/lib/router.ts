import { useCallback, useEffect, useRef, useState } from "react"

import {
  CanonicalBoardSlugError,
  parseCanonicalBoardSlug,
  validateCanonicalBoardSlug,
  type CanonicalBoardSlug,
} from "./board-slug"

export type InvalidBoardRoute = {
  kind: "error"
  code: "invalid-board-slug"
  pathname: string
  error: CanonicalBoardSlugError
}

export type BoardView = "signals" | "ontology"

export interface SignalsRouteFilters {
  readonly status?: "review" | "all" | "open" | "confirmed" | "resolved" | "rejected" | "superseded"
  readonly kinds?: readonly string[]
  readonly task?: string
  readonly signal?: string
}

export interface OntologyRouteFilters {
  readonly includeAll?: boolean
  readonly groupBy?: "label" | "candidate_atom" | "proposed_label" | "cluster"
  readonly signal?: string
  readonly atom?: string
}

export type BoardRouteFilters = SignalsRouteFilters | OntologyRouteFilters

export type AppRoute =
  | { kind: "home"; pathname: string }
  | {
      kind: "board"
      boardSlug: CanonicalBoardSlug
      pathname: string
      view?: BoardView
      filters?: BoardRouteFilters
    }
  | { kind: "settings"; pathname: string }
  | { kind: "not-found"; pathname: string }
  | InvalidBoardRoute

export type AppNavigationTarget =
  | AppRoute
  | { kind: "home" }
  | {
      kind: "board"
      boardSlug: CanonicalBoardSlug
      view?: BoardView
      filters?: BoardRouteFilters
    }
  | { kind: "settings" }
  | string

export type AppHistory = Pick<History, "pushState" | "replaceState">

export type AppNavigationOptions = {
  basePath?: string
  /** Runtime selector; this is not a canonical board slug and is never put in a URL. */
  defaultBoard?: string
  history?: AppHistory
  replace?: boolean
  resolveBoard?: (selector: string) => string | null | undefined | Promise<string | null | undefined>
}

export type AppRouterOptions = Omit<AppNavigationOptions, "history" | "replace"> & {
  location?: string
}

const DEFAULT_BASE_PATH = "/app/"

function normalizeBasePath(basePath = DEFAULT_BASE_PATH): string {
  const path = basePath.split(/[?#]/, 1)[0] || "/"
  const prefixed = path.startsWith("/") ? path : `/${path}`
  const segments = prefixed.split("/").filter(Boolean)
  return segments.length === 0 ? "/" : `/${segments.join("/")}/`
}

function pathnameFromInput(input: string): string {
  try {
    return new URL(input, "http://kanban-tool.invalid").pathname || "/"
  } catch {
    return input.split(/[?#]/, 1)[0] || "/"
  }
}

function searchFromInput(input: string): string {
  try {
    return new URL(input, "http://kanban-tool.invalid").search
  } catch {
    return ""
  }
}

function canonicalPathname(pathname: string, basePath: string): string {
  const withoutTrailingSlash = pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname
  const baseWithoutTrailingSlash = basePath.length > 1 ? basePath.slice(0, -1) : basePath
  return withoutTrailingSlash === baseWithoutTrailingSlash ? basePath : withoutTrailingSlash
}

function invalidBoardRoute(pathname: string, value: unknown): InvalidBoardRoute {
  const error = validateCanonicalBoardSlug(value) ?? new CanonicalBoardSlugError(value, "invalid-character")
  return {
    kind: "error",
    code: "invalid-board-slug",
    pathname,
    error,
  }
}

function decodeBoardSlug(value: string, pathname: string): CanonicalBoardSlug | InvalidBoardRoute {
  let decoded: string
  try {
    decoded = decodeURIComponent(value)
  } catch {
    return invalidBoardRoute(pathname, value)
  }
  return parseCanonicalBoardSlug(decoded) ?? invalidBoardRoute(pathname, decoded)
}

function parseSignalsFilters(search: string): SignalsRouteFilters {
  const params = new URLSearchParams(search)
  const rawStatus = params.get("status")
  const status = rawStatus === "all" || rawStatus === "open" || rawStatus === "confirmed" || rawStatus === "resolved" || rawStatus === "rejected" || rawStatus === "superseded"
    ? rawStatus
    : rawStatus === "review"
      ? "review"
      : undefined
  const kinds = params
    .getAll("kind")
    .flatMap((value) => value.split(","))
    .map((value) => value.trim())
    .filter(Boolean)
  const task = params.get("task")?.trim() || undefined
  const signal = params.get("signal")?.trim() || undefined
  return {
    ...(status === undefined ? {} : { status }),
    ...(kinds.length === 0 ? {} : { kinds }),
    ...(task === undefined ? {} : { task }),
    ...(signal === undefined ? {} : { signal }),
  }
}

function parseOntologyFilters(search: string): OntologyRouteFilters {
  const params = new URLSearchParams(search)
  const rawGroupBy = params.get("group_by")
  const groupBy = rawGroupBy === "candidate_atom" || rawGroupBy === "proposed_label"
    ? rawGroupBy
    : "label"
  const signal = params.get("signal")?.trim() || undefined
  const atom = params.get("atom")?.trim() || undefined
  return {
    includeAll: params.get("include_all") === "true",
    groupBy,
    ...(signal === undefined ? {} : { signal }),
    ...(atom === undefined ? {} : { atom }),
  }
}

function appendFeatureQuery(params: URLSearchParams, view: BoardView, filters: BoardRouteFilters | undefined): void {
  if (view === "signals") {
    const signalFilters = filters as SignalsRouteFilters | undefined
    if (signalFilters?.status !== undefined && signalFilters.status !== "review") params.set("status", signalFilters.status)
    for (const kind of signalFilters?.kinds ?? []) {
      const trimmed = kind.trim()
      if (trimmed) params.append("kind", trimmed)
    }
    const task = signalFilters?.task?.trim()
    if (task) params.set("task", task)
    const signal = signalFilters?.signal?.trim()
    if (signal) params.set("signal", signal)
  } else {
    const ontologyFilters = filters as OntologyRouteFilters | undefined
    if (ontologyFilters?.includeAll === true) params.set("include_all", "true")
    if (ontologyFilters?.groupBy !== undefined && ontologyFilters.groupBy !== "label") params.set("group_by", ontologyFilters.groupBy)
    const signal = ontologyFilters?.signal?.trim()
    if (signal) params.set("signal", signal)
    const atom = ontologyFilters?.atom?.trim()
    if (atom) params.set("atom", atom)
  }
}

function featurePathname(baseWithoutTrailingSlash: string, boardSlug: string, view: BoardView): string {
  return `${baseWithoutTrailingSlash}/boards/${encodeURIComponent(boardSlug)}/${view}`
}

export function parseAppRoute(
  input = typeof window === "undefined" ? DEFAULT_BASE_PATH : window.location.href,
  options: { basePath?: string } = {},
): AppRoute {
  const basePath = normalizeBasePath(options.basePath)
  const pathname = canonicalPathname(pathnameFromInput(input), basePath)
  const baseWithoutTrailingSlash = basePath.length > 1 ? basePath.slice(0, -1) : basePath

  if (pathname === basePath || pathname === baseWithoutTrailingSlash) {
    return { kind: "home", pathname: basePath }
  }
  if (pathname === `${baseWithoutTrailingSlash}/settings`) {
    return { kind: "settings", pathname: `${baseWithoutTrailingSlash}/settings` }
  }

  const boardPrefix = `${basePath}boards/`
  const featureView: BoardView | null = pathname.endsWith("/signals")
    ? "signals"
    : pathname.endsWith("/ontology")
      ? "ontology"
      : null
  const suffix = featureView === null ? "/board" : `/${featureView}`
  if (pathname.startsWith(boardPrefix) && pathname.endsWith(suffix)) {
    const slug = pathname.slice(boardPrefix.length, -suffix.length)
    const boardSlug = decodeBoardSlug(slug, pathname)
    if (typeof boardSlug === "string") {
      const baseRoute = { kind: "board" as const, boardSlug, pathname: routePath({ kind: "board", boardSlug }, options) }
      if (featureView === null) return baseRoute
      const search = searchFromInput(input)
      const filters = featureView === "signals" ? parseSignalsFilters(search) : parseOntologyFilters(search)
      return {
        ...baseRoute,
        pathname: featurePathname(baseWithoutTrailingSlash, boardSlug, featureView),
        view: featureView,
        filters,
      }
    }
    return { ...boardSlug, pathname }
  }

  return { kind: "not-found", pathname }
}

type RoutePathInput =
  | AppRoute
  | { kind: "home" }
  | {
      kind: "board"
      boardSlug: CanonicalBoardSlug
      view?: BoardView
      filters?: BoardRouteFilters
    }
  | { kind: "settings" }

function normalizedTarget(target: AppNavigationTarget, options: AppNavigationOptions): AppRoute {
  if (typeof target === "string") return parseAppRoute(target, options)
  switch (target.kind) {
    case "home":
      return { kind: "home", pathname: routePath(target, options) }
    case "board":
      if (!parseCanonicalBoardSlug(target.boardSlug)) return invalidBoardRoute(routePath({ kind: "home" }, options), target.boardSlug)
      return {
        kind: "board",
        boardSlug: target.boardSlug,
        pathname: target.view === undefined
          ? routePath(target, options)
          : featurePathname(normalizeBasePath(options.basePath).replace(/\/$/, ""), target.boardSlug, target.view),
        ...(target.view === undefined ? {} : { view: target.view, filters: target.filters ?? (target.view === "signals" ? {} : { includeAll: false, groupBy: "label" }) }),
      }
    case "settings":
      return { kind: "settings", pathname: routePath(target, options) }
    case "not-found":
    case "error":
      return target
  }
}

export function routePath(route: RoutePathInput, options: { basePath?: string } = {}): string {
  const basePath = normalizeBasePath(options.basePath)
  switch (route.kind) {
    case "home":
      return basePath
    case "settings":
      return `${basePath.replace(/\/$/, "")}/settings`
    case "board": {
      const boardSlug = parseCanonicalBoardSlug(route.boardSlug)
      if (!boardSlug) {
        throw validateCanonicalBoardSlug(route.boardSlug) ?? new CanonicalBoardSlugError(route.boardSlug, "invalid-character")
      }
      if (route.view === undefined) return `${basePath}boards/${encodeURIComponent(boardSlug)}/board`
      const pathname = featurePathname(basePath.replace(/\/$/, ""), boardSlug, route.view)
      const params = new URLSearchParams()
      appendFeatureQuery(params, route.view, route.filters)
      const search = params.toString()
      return search.length > 0 ? `${pathname}?${search}` : pathname
    }
    case "not-found":
    case "error":
      return route.pathname
  }
}

async function resolveTarget(target: AppNavigationTarget, options: AppNavigationOptions): Promise<AppRoute> {
  const route = normalizedTarget(target, options)
  if (route.kind !== "home" || !options.defaultBoard || !options.resolveBoard) return route

  const resolved = await options.resolveBoard(options.defaultBoard)
  const boardSlug = parseCanonicalBoardSlug(resolved)
  if (!boardSlug) return invalidBoardRoute(route.pathname, resolved)
  return {
    kind: "board",
    boardSlug,
    pathname: routePath({ kind: "board", boardSlug }, options),
  }
}

export async function resolveAppRoute(target: AppNavigationTarget, options: AppNavigationOptions = {}): Promise<AppRoute> {
  return resolveTarget(target, options)
}

function commitAppRoute(route: AppRoute, options: AppNavigationOptions): void {
  if (route.kind === "home" && options.defaultBoard) return
  if (route.kind === "error") return
  const history = options.history ?? (typeof window === "undefined" ? null : window.history)
  if (!history) return
  history[options.replace ? "replaceState" : "pushState"]({}, "", routePath(route, options))
}

export async function navigateApp(target: AppNavigationTarget, options: AppNavigationOptions = {}): Promise<AppRoute> {
  const route = await resolveTarget(target, options)
  const originalRoute = normalizedTarget(target, options)
  if (originalRoute.kind === "home" && options.defaultBoard && route.kind === "home") return route
  commitAppRoute(route, { ...options, replace: options.replace ?? (route.kind === "board" && originalRoute.kind === "home") })
  return route
}

/** Navigate after the integration layer has already resolved a canonical board slug. */
export async function navigateDefaultBoard(
  canonicalBoardSlug: CanonicalBoardSlug,
  options: Omit<AppNavigationOptions, "defaultBoard"> = {},
): Promise<AppRoute> {
  return navigateApp(
    {
      kind: "board",
      boardSlug: canonicalBoardSlug,
      pathname: routePath({ kind: "board", boardSlug: canonicalBoardSlug }, options),
    },
    { ...options, replace: true },
  )
}

function currentAppRoute(basePath: string): AppRoute | null {
  if (typeof window === "undefined") return null
  return parseAppRoute(window.location.href, { basePath })
}

export function useAppRouter(options: AppRouterOptions = {}) {
  const basePath = options.basePath ?? DEFAULT_BASE_PATH
  const defaultBoard = options.defaultBoard
  const resolveBoard = options.resolveBoard
  const location = options.location ?? (typeof window === "undefined" ? DEFAULT_BASE_PATH : window.location.href)
  const [route, setRoute] = useState<AppRoute>(() => parseAppRoute(location, { basePath }))
  const [error, setError] = useState<unknown>(null)
  const routeRef = useRef(route)
  const epochRef = useRef(0)
  const resolutionKeyRef = useRef<string | null>(null)
  const resolverRef = useRef(resolveBoard)
  const resolutionCacheRef = useRef<{ key: string; promise: Promise<AppRoute> } | null>(null)

  const setCurrentRoute = useCallback((nextRoute: AppRoute) => {
    routeRef.current = nextRoute
    setRoute(nextRoute)
  }, [])

  useEffect(() => {
    const onPopState = () => {
      epochRef.current += 1
      setError(null)
      setCurrentRoute(parseAppRoute(window.location.href, { basePath }))
    }
    window.addEventListener("popstate", onPopState)
    return () => {
      epochRef.current += 1
      window.removeEventListener("popstate", onPopState)
    }
  }, [basePath, setCurrentRoute])

  useEffect(() => {
    const resolutionKey = `${basePath}|${defaultBoard ?? ""}`
    if (resolverRef.current !== resolveBoard) {
      resolverRef.current = resolveBoard
      resolutionCacheRef.current = null
      epochRef.current += 1
    }
    if (resolutionKeyRef.current !== resolutionKey) {
      resolutionKeyRef.current = resolutionKey
      resolutionCacheRef.current = null
      epochRef.current += 1
    }
    if (route.kind !== "home" || !defaultBoard || !resolveBoard) return

    const requestEpoch = epochRef.current
    const expectedPathname = route.pathname
    const cache =
      resolutionCacheRef.current?.key === resolutionKey
        ? resolutionCacheRef.current
        : {
            key: resolutionKey,
            promise: resolveAppRoute(route, { basePath, defaultBoard, resolveBoard }),
          }
    resolutionCacheRef.current = cache
    let active = true
    void cache.promise
      .then((nextRoute) => {
        const stillCurrent =
          active &&
          requestEpoch === epochRef.current &&
          routeRef.current.kind === "home" &&
          routeRef.current.pathname === expectedPathname &&
          currentAppRoute(basePath)?.kind === "home" &&
          currentAppRoute(basePath)?.pathname === expectedPathname
        if (!stillCurrent) return
        commitAppRoute(nextRoute, { basePath, replace: true })
        setCurrentRoute(nextRoute)
      })
      .catch((reason: unknown) => {
        if (active && requestEpoch === epochRef.current) setError(reason)
      })
    return () => {
      active = false
    }
  }, [basePath, defaultBoard, resolveBoard, route, setCurrentRoute])

  const navigate = useCallback(
    async (
      target: AppNavigationTarget,
      navigationOptions: Omit<AppNavigationOptions, "basePath" | "defaultBoard" | "resolveBoard"> = {},
    ) => {
      const requestEpoch = ++epochRef.current
      const expectedPathname = routeRef.current.pathname
      setError(null)
      try {
        const nextRoute = await resolveAppRoute(target, {
          ...navigationOptions,
          basePath,
          defaultBoard,
          resolveBoard,
        })
        const browserRoute = currentAppRoute(basePath)
        if (requestEpoch !== epochRef.current || (browserRoute && browserRoute.pathname !== expectedPathname && routeRef.current.pathname === expectedPathname)) {
          return routeRef.current
        }
        commitAppRoute(nextRoute, {
          ...navigationOptions,
          basePath,
          defaultBoard,
          replace: navigationOptions.replace ?? (nextRoute.kind === "board" && routeRef.current.kind === "home"),
        })
        setCurrentRoute(nextRoute)
        return nextRoute
      } catch (reason) {
        if (requestEpoch === epochRef.current) setError(reason)
        return routeRef.current
      }
    },
    [basePath, defaultBoard, resolveBoard, setCurrentRoute],
  )

  return { route, navigate, error }
}

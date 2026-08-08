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

export type BoardRouteView = "board" | "list" | "map" | "runs" | "events"

export type AppRoute =
  | { kind: "home"; pathname: string }
  | { kind: "board"; boardSlug: CanonicalBoardSlug; pathname: string; view?: BoardRouteView; query?: string }
  | { kind: "settings"; pathname: string }
  | { kind: "not-found"; pathname: string }
  | InvalidBoardRoute

export type AppNavigationTarget =
  | AppRoute
  | { kind: "home" }
  | { kind: "board"; boardSlug: CanonicalBoardSlug }
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

function queryFromInput(input: string): string {
  try {
    return new URL(input, "http://kanban-tool.invalid").search.replace(/^\?/, "")
  } catch {
    return input.split("?", 2)[1]?.split("#", 1)[0] ?? ""
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

export function parseAppRoute(
  input = typeof window === "undefined" ? DEFAULT_BASE_PATH : window.location.href,
  options: { basePath?: string } = {},
): AppRoute {
  const basePath = normalizeBasePath(options.basePath)
  const pathname = canonicalPathname(pathnameFromInput(input), basePath)
  const query = queryFromInput(input)
  const baseWithoutTrailingSlash = basePath.length > 1 ? basePath.slice(0, -1) : basePath

  if (pathname === basePath || pathname === baseWithoutTrailingSlash) {
    return { kind: "home", pathname: basePath }
  }
  if (pathname === `${baseWithoutTrailingSlash}/settings`) {
    return { kind: "settings", pathname: `${baseWithoutTrailingSlash}/settings` }
  }

  const boardPrefix = `${basePath}boards/`
  if (pathname.startsWith(boardPrefix)) {
    const tail = pathname.slice(boardPrefix.length)
    const separator = tail.lastIndexOf("/")
    const view = separator > 0 ? tail.slice(separator + 1) : ""
    if (!(["board", "list", "map", "runs", "events"] as const).includes(view as BoardRouteView)) {
      return { kind: "not-found", pathname }
    }
    const slug = tail.slice(0, separator)
    const boardSlug = decodeBoardSlug(slug, pathname)
    if (typeof boardSlug === "string") {
      const route = { kind: "board" as const, boardSlug, pathname: routePath({ kind: "board", boardSlug, view: view as BoardRouteView }, options) }
      if (view === "board" && query.length === 0) return route
      return { ...route, view: view as BoardRouteView, ...(query.length > 0 ? { query } : {}) }
    }
    return { ...boardSlug, pathname }
  }

  return { kind: "not-found", pathname }
}

type RoutePathInput =
  | AppRoute
  | { kind: "home" }
  | { kind: "board"; boardSlug: CanonicalBoardSlug; view?: BoardRouteView; query?: string }
  | { kind: "settings" }

function normalizedTarget(target: AppNavigationTarget, options: AppNavigationOptions): AppRoute {
  if (typeof target === "string") return parseAppRoute(target, options)
  switch (target.kind) {
    case "home":
      return { kind: "home", pathname: routePath(target, options) }
    case "board":
      if (!parseCanonicalBoardSlug(target.boardSlug)) return invalidBoardRoute(routePath({ kind: "home" }, options), target.boardSlug)
      return { kind: "board", boardSlug: target.boardSlug, pathname: routePath(target, options) }
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
      {
        const view = route.view ?? "board"
        const path = `${basePath}boards/${encodeURIComponent(boardSlug)}/${view}`
        return route.query ? `${path}?${route.query.replace(/^\?/, "")}` : path
      }
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

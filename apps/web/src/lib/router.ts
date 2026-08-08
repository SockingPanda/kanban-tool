import { useCallback, useEffect, useState } from "react"

export type AppRoute =
  | { kind: "home"; pathname: string }
  | { kind: "board"; boardSlug: string; pathname: string }
  | { kind: "settings"; pathname: string }
  | { kind: "not-found"; pathname: string }

export type AppNavigationTarget =
  | AppRoute
  | { kind: "home" }
  | { kind: "board"; boardSlug: string }
  | { kind: "settings" }
  | string

export type AppHistory = Pick<History, "pushState" | "replaceState">

export type AppNavigationOptions = {
  basePath?: string
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

function decodeBoardSlug(value: string): string | null {
  if (!value || value.includes("/")) return null
  try {
    const decoded = decodeURIComponent(value)
    if (!decoded || decoded.includes("/") || decoded === "." || decoded === "..") return null
    return decoded
  } catch {
    return null
  }
}

function canonicalPathname(pathname: string, basePath: string): string {
  const withoutTrailingSlash = pathname.length > 1 ? pathname.replace(/\/+$/, "") : pathname
  const baseWithoutTrailingSlash = basePath.length > 1 ? basePath.slice(0, -1) : basePath
  return withoutTrailingSlash === baseWithoutTrailingSlash ? basePath : withoutTrailingSlash
}

export function parseAppRoute(input = typeof window === "undefined" ? DEFAULT_BASE_PATH : window.location.href, options: { basePath?: string } = {}): AppRoute {
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
  if (pathname.startsWith(boardPrefix) && pathname.endsWith("/board")) {
    const slug = pathname.slice(boardPrefix.length, -"/board".length)
    const boardSlug = decodeBoardSlug(slug)
    if (boardSlug) {
      return { kind: "board", boardSlug, pathname: routePath({ kind: "board", boardSlug }, options) }
    }
  }

  return { kind: "not-found", pathname }
}

type RoutePathInput =
  | AppRoute
  | { kind: "home" }
  | { kind: "board"; boardSlug: string }
  | { kind: "settings" }

function normalizedTarget(target: AppNavigationTarget, options: AppNavigationOptions): AppRoute {
  if (typeof target === "string") return parseAppRoute(target, options)
  switch (target.kind) {
    case "home":
      return { kind: "home", pathname: routePath(target, options) }
    case "board":
      return { kind: "board", boardSlug: target.boardSlug, pathname: routePath(target, options) }
    case "settings":
      return { kind: "settings", pathname: routePath(target, options) }
    case "not-found":
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
    case "board":
      return `${basePath}boards/${encodeURIComponent(route.boardSlug ?? "")}/board`
    case "not-found":
      return route.pathname ?? basePath
  }
}

async function resolveTarget(target: AppNavigationTarget, options: AppNavigationOptions): Promise<AppRoute> {
  const route = normalizedTarget(target, options)
  if (route.kind !== "home" || !options.defaultBoard || !options.resolveBoard) return route

  const resolved = await options.resolveBoard?.(options.defaultBoard)
  const boardSlug = resolved?.trim()
  if (!boardSlug) return route
  return {
    kind: "board",
    boardSlug,
    pathname: routePath({ kind: "board", boardSlug }, options),
  }
}

export async function navigateApp(target: AppNavigationTarget, options: AppNavigationOptions = {}): Promise<AppRoute> {
  const route = await resolveTarget(target, options)
  const path = routePath(route, options)
  const originalRoute = normalizedTarget(target, options)
  if (originalRoute.kind === "home" && options.defaultBoard && route.kind === "home") return route
  const history = options.history ?? (typeof window === "undefined" ? null : window.history)
  if (!history) return route

  const isDefaultBoardRedirect = route.kind === "board" && originalRoute.kind === "home"
  const replace = options.replace ?? isDefaultBoardRedirect
  history[replace ? "replaceState" : "pushState"]({}, "", path)
  return route
}

/** Navigate after the integration layer has already resolved a canonical board slug. */
export async function navigateDefaultBoard(canonicalBoardSlug: string, options: Omit<AppNavigationOptions, "defaultBoard"> = {}): Promise<AppRoute> {
  return navigateApp(
    { kind: "board", boardSlug: canonicalBoardSlug, pathname: routePath({ kind: "board", boardSlug: canonicalBoardSlug }, options) },
    { ...options, replace: true },
  )
}

export function useAppRouter(options: AppRouterOptions = {}) {
  const basePath = options.basePath ?? DEFAULT_BASE_PATH
  const defaultBoard = options.defaultBoard
  const resolveBoard = options.resolveBoard
  const location = options.location ?? (typeof window === "undefined" ? DEFAULT_BASE_PATH : window.location.href)
  const [route, setRoute] = useState<AppRoute>(() => parseAppRoute(location, { basePath }))
  const [error, setError] = useState<unknown>(null)

  const [defaultBoardResolutionStarted, setDefaultBoardResolutionStarted] = useState(false)

  useEffect(() => {
    const onPopState = () => {
      setDefaultBoardResolutionStarted(false)
      setError(null)
      setRoute(parseAppRoute(window.location.href, { basePath }))
    }
    window.addEventListener("popstate", onPopState)
    return () => window.removeEventListener("popstate", onPopState)
  }, [basePath])

  useEffect(() => {
    if (route.kind !== "home" || !defaultBoard || !resolveBoard || defaultBoardResolutionStarted) return
    setDefaultBoardResolutionStarted(true)
    let cancelled = false
    void navigateApp(route, { basePath, defaultBoard, resolveBoard, replace: true })
      .then((nextRoute) => {
        if (!cancelled) setRoute(nextRoute)
      })
      .catch((reason: unknown) => {
        if (!cancelled) setError(reason)
      })
    return () => {
      cancelled = true
    }
  }, [basePath, defaultBoard, defaultBoardResolutionStarted, resolveBoard, route])

  const navigate = useCallback(
    async (target: AppNavigationTarget, navigationOptions: Omit<AppNavigationOptions, "basePath" | "defaultBoard" | "resolveBoard"> = {}) => {
      setError(null)
      try {
        const nextRoute = await navigateApp(target, {
          ...navigationOptions,
          basePath,
          defaultBoard,
          resolveBoard,
        })
        setRoute(nextRoute)
        return nextRoute
      } catch (reason) {
        setError(reason)
        throw reason
      }
    },
    [basePath, defaultBoard, resolveBoard],
  )

  return { route, navigate, error }
}

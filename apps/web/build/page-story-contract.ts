/** Formal Storybook page metadata contract and build-time catalog gates. */

export const PAGE_STORY_ROUTES = [
  { path: "/app/", kind: "home", routerKind: "home" },
  { path: "/app/boards/:boardSlug/overview", kind: "project-overview", routerKind: "project-overview" },
  { path: "/app/boards/:boardSlug/board", kind: "board", routerKind: "board", view: "board" },
  { path: "/app/boards/:boardSlug/list", kind: "list", routerKind: "board", view: "list" },
  { path: "/app/boards/:boardSlug/map", kind: "map", routerKind: "board", view: "map" },
  { path: "/app/boards/:boardSlug/runs", kind: "runs", routerKind: "board", view: "runs" },
  { path: "/app/boards/:boardSlug/events", kind: "events", routerKind: "board", view: "events" },
  { path: "/app/boards/:boardSlug/signals", kind: "signals", routerKind: "board", view: "signals" },
  { path: "/app/boards/:boardSlug/ontology", kind: "ontology", routerKind: "board", view: "ontology" },
  { path: "/app/boards/:boardSlug/health", kind: "health", routerKind: "health" },
  { path: "/app/boards/:boardSlug/maintenance", kind: "maintenance", routerKind: "maintenance" },
  { path: "/app/settings", kind: "settings", routerKind: "settings" },
] as const

export type PageStoryRoute = (typeof PAGE_STORY_ROUTES)[number] & Readonly<{ productionOwner: string }>
export type PageStoryKind = PageStoryRoute["kind"]
export type PageStoryFrame = "content" | "workspace"

export type PageStoryContract = Readonly<{
  route: PageStoryRoute
  fixture: Readonly<{
    canonicalSafe: true
    api: false
    sse: false
    mutation: false
  }>
  responsive: Readonly<{ wide: string; narrow: string }>
  states: readonly string[]
  frame: PageStoryFrame
  astryx: Readonly<{
    package: string
    commands: readonly string[]
    adopted: readonly string[]
  }>
}>

export function definePageStoryContract<const Contract extends PageStoryContract>(contract: Contract): Contract {
  return contract
}

export type PageStoryDiagnosticCode =
  | "missing-field"
  | "invalid-type"
  | "invalid-value"
  | "network-enabled"
  | "duplicate-value"
  | "unknown-route"
  | "missing-route"
  | "missing-ready-story"
  | "missing-boundary"

export type PageStoryDiagnostic = Readonly<{
  path: string
  code: PageStoryDiagnosticCode
  message: string
  severity: "error"
}>

export type PageStoryValidationResult = Readonly<{
  ok: boolean
  valid: boolean
  contract?: PageStoryContract
  diagnostics: readonly PageStoryDiagnostic[]
  errors: readonly PageStoryDiagnostic[]
}>

export type PageStoryRouterAlias = Readonly<{
  aliasKind: "default-board"
  path: string
  canonicalPath: string
}>

export type PageStoryRouterBoundary = Readonly<{
  pathPattern: string
  kind: "not-found" | "invalid-board-slug"
  expectedKind: "not-found" | "error"
  errorCode?: "invalid-board-slug"
}>

export const PAGE_STORY_ROUTER_ALIASES: readonly PageStoryRouterAlias[] = Object.freeze([
  { aliasKind: "default-board", path: "/app/boards/:boardSlug", canonicalPath: "/app/boards/:boardSlug/board" },
])

export const PAGE_STORY_ROUTER_BOUNDARIES: readonly PageStoryRouterBoundary[] = Object.freeze([
  { pathPattern: "/app/not-a-route", kind: "not-found", expectedKind: "not-found" },
  { pathPattern: "/app/boards/%2F/board", kind: "invalid-board-slug", expectedKind: "error", errorCode: "invalid-board-slug" },
])

type UnknownRecord = Record<string, unknown>

function record(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function requiredString(value: unknown, path: string, issues: PageStoryDiagnostic[]): value is string {
  if (typeof value !== "string" || value.trim().length === 0) {
    issues.push({ path, code: typeof value === "string" ? "invalid-value" : "invalid-type", message: `${path} must be a non-empty string`, severity: "error" })
    return false
  }
  return true
}

function stringList(value: unknown, path: string, issues: PageStoryDiagnostic[]): value is readonly string[] {
  if (!Array.isArray(value)) {
    issues.push({ path, code: "invalid-type", message: `${path} must be an array of strings`, severity: "error" })
    return false
  }
  if (value.length === 0 || value.some((entry) => typeof entry !== "string" || entry.trim().length === 0)) {
    issues.push({ path, code: "invalid-value", message: `${path} must contain non-empty strings`, severity: "error" })
    return false
  }
  if (new Set(value).size !== value.length) {
    issues.push({ path, code: "duplicate-value", message: `${path} entries must be unique`, severity: "error" })
    return false
  }
  return true
}

function requiredObject(value: unknown, path: string, issues: PageStoryDiagnostic[]): UnknownRecord | undefined {
  if (!record(value)) {
    issues.push({ path, code: "invalid-type", message: `${path} must be an object`, severity: "error" })
    return undefined
  }
  return value
}

/** Runtime validator for the canonical shape. It never consults legacy fields. */
export function validatePageStoryContract(input: unknown): PageStoryValidationResult {
  const issues: PageStoryDiagnostic[] = []
  if (!record(input)) {
    issues.push({ path: "$", code: "invalid-type", message: "$ must be an object", severity: "error" })
    return { ok: false, valid: false, diagnostics: issues, errors: issues }
  }

  const route = requiredObject(input.route, "route", issues)
  const fixture = requiredObject(input.fixture, "fixture", issues)
  const responsive = requiredObject(input.responsive, "responsive", issues)
  const astryx = requiredObject(input.astryx, "astryx", issues)

  const pathOk = route !== undefined && requiredString(route.path, "route.path", issues)
  const kindOk = route !== undefined && requiredString(route.kind, "route.kind", issues)
  const ownerOk = route !== undefined && requiredString(route.productionOwner, "route.productionOwner", issues)
  const wideOk = responsive !== undefined && requiredString(responsive.wide, "responsive.wide", issues)
  const narrowOk = responsive !== undefined && requiredString(responsive.narrow, "responsive.narrow", issues)
  const packageOk = astryx !== undefined && requiredString(astryx.package, "astryx.package", issues)
  const commandsOk = astryx !== undefined && stringList(astryx.commands, "astryx.commands", issues)
  const adoptedOk = astryx !== undefined && stringList(astryx.adopted, "astryx.adopted", issues)
  const statesOk = stringList(input.states, "states", issues)

  if (fixture !== undefined) {
    if (fixture.canonicalSafe !== true) issues.push({ path: "fixture.canonicalSafe", code: "invalid-value", message: "fixture.canonicalSafe must be true", severity: "error" })
    for (const key of ["api", "sse", "mutation"] as const) {
      if (fixture[key] !== false) issues.push({ path: `fixture.${key}`, code: fixture[key] === true ? "network-enabled" : "invalid-value", message: `fixture.${key} must be false`, severity: "error" })
    }
  }
  if (input.frame !== "content" && input.frame !== "workspace") issues.push({ path: "frame", code: input.frame === undefined ? "missing-field" : "invalid-value", message: "frame must be content or workspace", severity: "error" })

  let contract: PageStoryContract | undefined
  const routeSpec = pathOk && route !== undefined ? PAGE_STORY_ROUTES.find((entry) => entry.path === route.path) : undefined
  if (pathOk && routeSpec === undefined) issues.push({ path: "route.path", code: "unknown-route", message: `route.path is not formal: ${route?.path}`, severity: "error" })
  const routeSpecView = routeSpec !== undefined && "view" in routeSpec ? routeSpec.view : undefined
  if (routeSpec !== undefined && kindOk && (route?.kind !== routeSpec.kind || route?.routerKind !== routeSpec.routerKind || route?.view !== routeSpecView)) {
    issues.push({ path: "route", code: "invalid-value", message: `route metadata does not match ${routeSpec.path}`, severity: "error" })
  }
  if (route !== undefined && fixture !== undefined && responsive !== undefined && astryx !== undefined && routeSpec !== undefined
    && pathOk && kindOk && ownerOk && wideOk && narrowOk && packageOk && commandsOk && adoptedOk && statesOk && issues.length === 0) {
    contract = input as PageStoryContract
  }
  return { ok: issues.length === 0, valid: issues.length === 0, ...(contract === undefined ? {} : { contract }), diagnostics: issues, errors: issues }
}

export function isValidPageStoryContract(input: unknown): input is PageStoryContract {
  return validatePageStoryContract(input).ok
}

export type PageStoryCatalogEntry = Readonly<{ contract: PageStoryContract; readyStoryId: string }>
export type PageStoryCatalogInput = Readonly<{
  routes: readonly PageStoryCatalogEntry[]
  aliases: readonly PageStoryRouterAlias[]
  boundaries: readonly PageStoryRouterBoundary[]
}>

export type PageStoryCatalogValidationResult = Readonly<{
  ok: boolean
  valid: boolean
  entries: readonly PageStoryCatalogEntry[]
  diagnostics: readonly PageStoryDiagnostic[]
  errors: readonly PageStoryDiagnostic[]
}>

function isAlias(value: unknown): value is PageStoryRouterAlias {
  return record(value)
    && value.aliasKind === "default-board"
    && typeof value.path === "string"
    && typeof value.canonicalPath === "string"
}

function exactAlias(actual: unknown, expected: PageStoryRouterAlias): boolean {
  return isAlias(actual) && actual.aliasKind === expected.aliasKind && actual.path === expected.path && actual.canonicalPath === expected.canonicalPath
}

function isBoundary(value: unknown): value is PageStoryRouterBoundary {
  return record(value)
    && typeof value.pathPattern === "string"
    && (value.kind === "not-found" || value.kind === "invalid-board-slug")
    && (value.expectedKind === "not-found" || value.expectedKind === "error")
    && (value.errorCode === undefined || value.errorCode === "invalid-board-slug")
}

function exactBoundary(actual: unknown, expected: PageStoryRouterBoundary): boolean {
  return isBoundary(actual)
    && actual.pathPattern === expected.pathPattern
    && actual.kind === expected.kind
    && actual.expectedKind === expected.expectedKind
    && actual.errorCode === expected.errorCode
}

export function validatePageStoryCatalog(input: unknown, options: Readonly<{ requireFormalRoutes?: boolean; requireRouterCoverage?: boolean }> = { requireFormalRoutes: true, requireRouterCoverage: true }): PageStoryCatalogValidationResult {
  const issues: PageStoryDiagnostic[] = []
  const objectInput = record(input) ? input : undefined
  const entries = objectInput !== undefined && Array.isArray(objectInput.routes) ? objectInput.routes as readonly PageStoryCatalogEntry[] : []
  const aliases = objectInput !== undefined && Array.isArray(objectInput.aliases) ? objectInput.aliases : []
  const boundaries = objectInput !== undefined && Array.isArray(objectInput.boundaries) ? objectInput.boundaries : []
  const requireFormalRoutes = options.requireFormalRoutes !== false
  const requireRouterCoverage = options.requireRouterCoverage !== false
  if (objectInput === undefined) issues.push({ path: "$", code: "invalid-type", message: "catalog must be an object", severity: "error" })
  for (const key of ["routes", "aliases", "boundaries"] as const) {
    if (objectInput !== undefined && !Array.isArray(objectInput[key])) issues.push({ path: key, code: "invalid-type", message: `${key} must be an array`, severity: "error" })
  }
  const firstIndex = new Map<string, number>()
  entries.forEach((entry, index) => {
    const path = entry?.contract?.route?.path
    if (typeof path !== "string") {
      issues.push({ path: `routes[${index}]`, code: "invalid-type", message: "catalog entry must contain a contract and readyStoryId", severity: "error" })
      return
    }
    const first = firstIndex.get(path)
    if (first !== undefined) issues.push({ path: `routes[${index}].contract.route.path`, code: "duplicate-value", message: `duplicates routes[${first}]`, severity: "error" })
    else firstIndex.set(path, index)
    if (typeof entry.readyStoryId !== "string" || entry.readyStoryId.trim().length === 0) issues.push({ path: `routes[${index}].readyStoryId`, code: "missing-ready-story", message: "readyStoryId is required", severity: "error" })
    const result = validatePageStoryContract(entry.contract)
    issues.push(...result.errors.map((error) => ({ ...error, path: `routes[${index}].${error.path}` })))
  })
  if (requireFormalRoutes) {
    for (const route of PAGE_STORY_ROUTES) if (!firstIndex.has(route.path)) issues.push({ path: "routes", code: "missing-route", message: `missing ${route.path}`, severity: "error" })
  }
  if (requireRouterCoverage) {
    for (const expected of PAGE_STORY_ROUTER_ALIASES) if (!aliases.some((actual) => exactAlias(actual, expected))) issues.push({ path: "aliases", code: "missing-boundary", message: `missing alias ${expected.path}`, severity: "error" })
    for (const expected of PAGE_STORY_ROUTER_BOUNDARIES) if (!boundaries.some((actual) => exactBoundary(actual, expected))) issues.push({ path: "boundaries", code: "missing-boundary", message: `missing boundary ${expected.pathPattern}`, severity: "error" })
    aliases.forEach((actual, index) => {
      if (!isAlias(actual)) {
        issues.push({ path: `aliases[${index}]`, code: "invalid-type", message: "router alias must declare aliasKind, path, and canonicalPath", severity: "error" })
        return
      }
      if (!PAGE_STORY_ROUTER_ALIASES.some((expected) => exactAlias(actual, expected))) issues.push({ path: `aliases[${index}]`, code: "unknown-route", message: "unknown router alias", severity: "error" })
      const first = aliases.findIndex((candidate) => exactAlias(candidate, actual))
      if (first !== index) issues.push({ path: `aliases[${index}]`, code: "duplicate-value", message: `duplicate router alias; first seen at aliases[${first}]`, severity: "error" })
    })
    boundaries.forEach((actual, index) => {
      if (!isBoundary(actual)) {
        issues.push({ path: `boundaries[${index}]`, code: "invalid-type", message: "router boundary must declare pathPattern, kind, and expectedKind", severity: "error" })
        return
      }
      if (!PAGE_STORY_ROUTER_BOUNDARIES.some((expected) => exactBoundary(actual, expected))) issues.push({ path: `boundaries[${index}]`, code: "unknown-route", message: "unknown router boundary", severity: "error" })
      const first = boundaries.findIndex((candidate) => exactBoundary(candidate, actual))
      if (first !== index) issues.push({ path: `boundaries[${index}]`, code: "duplicate-value", message: `duplicate router boundary; first seen at boundaries[${first}]`, severity: "error" })
    })
  }
  return { ok: issues.length === 0, valid: issues.length === 0, entries, diagnostics: issues, errors: issues }
}

export function validatePageStoryRouterCoverage(input: unknown): PageStoryValidationResult {
  const issues: PageStoryDiagnostic[] = []
  const aliases = record(input) && Array.isArray(input.aliases) ? input.aliases : []
  const boundaries = record(input) && Array.isArray(input.boundaries) ? input.boundaries : []
  if (!record(input)) issues.push({ path: "$", code: "invalid-type", message: "router coverage must be an object", severity: "error" })
  for (const expected of PAGE_STORY_ROUTER_ALIASES) if (!aliases.some((actual) => exactAlias(actual, expected))) issues.push({ path: "aliases", code: "missing-boundary", message: `missing alias ${expected.path}`, severity: "error" })
  for (const expected of PAGE_STORY_ROUTER_BOUNDARIES) if (!boundaries.some((actual) => exactBoundary(actual, expected))) issues.push({ path: "boundaries", code: "missing-boundary", message: `missing boundary ${expected.pathPattern}`, severity: "error" })
  aliases.forEach((actual, index) => {
    if (!isAlias(actual)) {
      issues.push({ path: `aliases[${index}]`, code: "invalid-type", message: "router alias must declare aliasKind, path, and canonicalPath", severity: "error" })
      return
    }
    if (!PAGE_STORY_ROUTER_ALIASES.some((expected) => exactAlias(actual, expected))) issues.push({ path: `aliases[${index}]`, code: "unknown-route", message: "unknown router alias", severity: "error" })
    const first = aliases.findIndex((candidate) => exactAlias(candidate, actual))
    if (first !== index) issues.push({ path: `aliases[${index}]`, code: "duplicate-value", message: `duplicate router alias; first seen at aliases[${first}]`, severity: "error" })
  })
  boundaries.forEach((actual, index) => {
    if (!isBoundary(actual)) {
      issues.push({ path: `boundaries[${index}]`, code: "invalid-type", message: "router boundary must declare pathPattern, kind, and expectedKind", severity: "error" })
      return
    }
    if (!PAGE_STORY_ROUTER_BOUNDARIES.some((expected) => exactBoundary(actual, expected))) issues.push({ path: `boundaries[${index}]`, code: "unknown-route", message: "unknown router boundary", severity: "error" })
    const first = boundaries.findIndex((candidate) => exactBoundary(candidate, actual))
    if (first !== index) issues.push({ path: `boundaries[${index}]`, code: "duplicate-value", message: `duplicate router boundary; first seen at boundaries[${first}]`, severity: "error" })
  })
  return { ok: issues.length === 0, valid: issues.length === 0, diagnostics: issues, errors: issues }
}

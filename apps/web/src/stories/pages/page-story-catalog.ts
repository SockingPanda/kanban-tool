/**
 * Canonical page-story inventory.
 *
 * This module is intentionally data-only: the Storybook stories, build-time
 * validator, and route evidence docs all consume the same typed entries. It
 * does not import a production route, query, transport, or mutation owner.
 */

export type PageStoryKind =
  | "home"
  | "project-overview"
  | "board"
  | "list"
  | "map"
  | "runs"
  | "events"
  | "signals"
  | "ontology"
  | "health"
  | "maintenance"
  | "settings"

export type PageStoryView = "board" | "list" | "map" | "runs" | "events" | "signals" | "ontology"
export type PageStoryRouterKind = "home" | "project-overview" | "board" | "health" | "maintenance" | "settings"
export type PageStoryFrame = "content" | "workspace"

export type PageStoryState = "ready" | "loading" | "empty" | "error" | "offline" | "stale" | "recovering" | "confirm"

export type PageStoryRoute = Readonly<{
  path: string
  kind: PageStoryKind
  routerKind: PageStoryRouterKind
  view?: PageStoryView
  productionOwner: string
}>

export type PageStoryContract = Readonly<{
  route: PageStoryRoute
  fixture: Readonly<{
    canonicalSafe: true
    api: false
    sse: false
    mutation: false
  }>
  responsive: Readonly<{ wide: string; narrow: string }>
  states: readonly PageStoryState[]
  frame: PageStoryFrame
  astryx: Readonly<{
    package: string
    commands: readonly string[]
    adopted: readonly string[]
  }>
}>

export type PageStoryCatalogEntry = Readonly<{
  contract: PageStoryContract
  readyStoryId: string
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

export type PageStoryCatalogInput = Readonly<{
  routes: readonly PageStoryCatalogEntry[]
  aliases: readonly PageStoryRouterAlias[]
  boundaries: readonly PageStoryRouterBoundary[]
}>

const storybookFixture = {
  canonicalSafe: true,
  api: false,
  sse: false,
  mutation: false,
} as const

const responsiveEvidence = {
  wide: "1440×900：四区 shell 与 dense workspace 保持可扫读。",
  narrow: "390×844：sidebar 转 drawer；table/map 只在自身 region 横向滚动。",
} as const

const astryxEvidence = {
  package: "@astryxdesign/core@0.3.0",
  commands: ["pnpm exec astryx build \"static product page Storybook evidence\""],
  adopted: ["PageFrame", "SafeVStack", "SafeHStack", "Grid", "SafeCard"],
} as const

function entry(
  route: PageStoryRoute,
  states: readonly PageStoryState[],
  frame: PageStoryFrame,
  readyStoryId: string,
): PageStoryCatalogEntry {
  return {
    contract: {
      route,
      fixture: storybookFixture,
      responsive: responsiveEvidence,
      states,
      frame,
      astryx: astryxEvidence,
    },
    readyStoryId,
  }
}

/** The only formal list of the twelve production page routes. */
export const PAGE_STORY_CATALOG = {
  routes: [
    entry({ path: "/app/", kind: "home", routerKind: "home", productionOwner: "features/projects/ProjectsCollection" }, ["ready", "loading", "empty", "error"], "content", "pages-routes--home-ready"),
    entry({ path: "/app/boards/:boardSlug/overview", kind: "project-overview", routerKind: "project-overview", productionOwner: "features/projects/ProjectOverview" }, ["ready", "offline", "error"], "content", "pages-routes--project-overview-ready"),
    entry({ path: "/app/boards/:boardSlug/board", kind: "board", routerKind: "board", view: "board", productionOwner: "features/board/BoardView" }, ["ready", "loading", "empty", "error"], "workspace", "pages-routes--board-ready"),
    entry({ path: "/app/boards/:boardSlug/list", kind: "list", routerKind: "board", view: "list", productionOwner: "features/explorer/TaskListView" }, ["ready", "loading", "empty", "error"], "workspace", "pages-routes--list-ready"),
    entry({ path: "/app/boards/:boardSlug/map", kind: "map", routerKind: "board", view: "map", productionOwner: "features/explorer/TaskMapView" }, ["ready", "loading", "empty", "error"], "workspace", "pages-routes--map-ready"),
    entry({ path: "/app/boards/:boardSlug/runs", kind: "runs", routerKind: "board", view: "runs", productionOwner: "features/explorer/TaskRunsView" }, ["ready", "loading", "empty", "error"], "content", "pages-routes--runs-ready"),
    entry({ path: "/app/boards/:boardSlug/events", kind: "events", routerKind: "board", view: "events", productionOwner: "features/explorer/EventsView" }, ["ready", "offline", "recovering", "error"], "content", "pages-routes--events-ready"),
    entry({ path: "/app/boards/:boardSlug/signals", kind: "signals", routerKind: "board", view: "signals", productionOwner: "features/signals/SignalsScreen" }, ["ready", "loading", "empty", "error"], "workspace", "pages-routes--signals-ready"),
    entry({ path: "/app/boards/:boardSlug/ontology", kind: "ontology", routerKind: "board", view: "ontology", productionOwner: "features/ontology/OntologyScreen" }, ["ready", "loading", "empty", "error"], "workspace", "pages-routes--ontology-ready"),
    entry({ path: "/app/boards/:boardSlug/health", kind: "health", routerKind: "health", productionOwner: "features/health/HealthPage" }, ["ready", "loading", "error"], "content", "pages-routes--health-ready"),
    entry({ path: "/app/boards/:boardSlug/maintenance", kind: "maintenance", routerKind: "maintenance", productionOwner: "features/maintenance/MaintenancePage" }, ["ready", "loading", "error", "confirm"], "content", "pages-routes--maintenance-ready"),
    entry({ path: "/app/settings", kind: "settings", routerKind: "settings", productionOwner: "features/settings/SettingsPage" }, ["ready", "loading", "error"], "content", "pages-routes--settings-ready"),
  ],
  aliases: [
    { aliasKind: "default-board", path: "/app/boards/:boardSlug", canonicalPath: "/app/boards/:boardSlug/board" },
  ],
  boundaries: [
    { pathPattern: "/app/not-a-route", kind: "not-found", expectedKind: "not-found" },
    { pathPattern: "/app/boards/%2F/board", kind: "invalid-board-slug", expectedKind: "error", errorCode: "invalid-board-slug" },
  ],
} as const satisfies PageStoryCatalogInput

/** Convenience projection for build validators and route-shape tests. */
export const PAGE_STORY_ROUTES: readonly PageStoryRoute[] = PAGE_STORY_CATALOG.routes.map(({ contract }) => contract.route)
export const PAGE_STORY_ROUTER_ALIASES: readonly PageStoryRouterAlias[] = PAGE_STORY_CATALOG.aliases
export const PAGE_STORY_ROUTER_BOUNDARIES: readonly PageStoryRouterBoundary[] = PAGE_STORY_CATALOG.boundaries

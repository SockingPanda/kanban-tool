import { Button } from "@astryxdesign/core/Button"
import { useEffect, useState, type MouseEvent, type ReactNode } from "react"

import { ProjectOverview, type ProjectOverviewStatus } from "./features/projects/ProjectOverview"
import { ProjectsCollection } from "./features/projects/ProjectsCollection"
import { ExplorerPage } from "./features/explorer/ExplorerPage"
import { HealthPage } from "./features/health/HealthPage"
import { MaintenancePage } from "./features/maintenance/MaintenancePage"
import { SettingsPage as OperatorSettingsPage } from "./features/settings/SettingsPage"
import type { BoardEventsBatch } from "./lib/api/explorer-read-model"
import type { BoardListItem, BoardListReadError } from "./lib/api/board-list-read-model"
import type { BoardTaskCanonicalReloadHandler, BoardTaskMutationSurface } from "./features/board/task-mutation-state"
import type { BoardSyncStatus } from "./features/board/types"
import type { CanonicalBoardSlug } from "./lib/board-slug"
import { routePath, type AppNavigationTarget, type AppRoute } from "./lib/router"
import type { WebRuntimeConfig } from "./lib/runtime"
import { createTranslator } from "./lib/i18n"
import { usePreferences } from "./lib/use-preferences"
import { BrowserConnectivityProvider } from "./lib/browser-connectivity-provider"
import { ProductRail, ProjectsSidebar, ResourceHeader, type NavigationProject, type ProductRailItem, type ProjectPickerStatus, type ProjectSurface, type ResourceHeaderMoreItem } from "./ui/navigation"
import navigationStyles from "./ui/navigation/navigation.module.css"
import styles from "./shell.module.css"
import type { BoardReconnectResult } from "./features/board/board-session-registry"

export type ShellBoundary = "ready" | "loading" | "error" | "offline"

export type BoardListSurface = {
  readonly status: "loading" | "ready" | "error" | "offline" | "stale"
  readonly items: readonly BoardListItem[]
  readonly error: BoardListReadError | null
  readonly isRefreshing: boolean
  readonly onRetry: () => void
}

export type ProductShellProps = {
  runtime: WebRuntimeConfig
  route: AppRoute
  canonicalBoardSlug?: CanonicalBoardSlug
  boardList?: BoardListSurface
  children?: ReactNode
  boundary?: ShellBoundary
  error?: ReactNode
  onNavigate?: (target: AppNavigationTarget, options?: { readonly replace?: boolean }) => AppRoute | Promise<AppRoute>
  onReconnect?: () => BoardReconnectResult | boolean | void | Promise<BoardReconnectResult | boolean | void>
  onRetry?: () => void
  invalidationRevision?: number
  boardRevision?: number
  inspectorRevision?: number
  runsRevision?: number
  eventsRefreshRevision?: number
  eventsBatch?: BoardEventsBatch | null
  syncStatus?: BoardSyncStatus
  taskMutations?: BoardTaskMutationSurface
  onVisibleCanonicalReloadChange?: (reload: BoardTaskCanonicalReloadHandler | undefined, releasedReload?: BoardTaskCanonicalReloadHandler) => void
}

function activeProjectSlug(route: AppRoute): CanonicalBoardSlug | undefined {
  return route.kind === "board" || route.kind === "project-overview" || route.kind === "health" || route.kind === "maintenance"
    ? route.boardSlug
    : undefined
}

function usesTasksWorkspaceChrome(route: AppRoute): boolean {
  return route.kind === "board"
    && (route.view === undefined || route.view === "board" || route.view === "list" || route.view === "map" || route.view === "runs" || route.view === "events")
}

function routeProject(route: AppRoute, projects: readonly BoardListItem[] | undefined): BoardListItem | undefined {
  const slug = activeProjectSlug(route)
  return slug === undefined ? undefined : projects?.find((project) => project.slug === slug)
}

function projectPickerStatus(surface: BoardListSurface | undefined): ProjectPickerStatus {
  if (surface === undefined) return "loading"
  if (surface.isRefreshing && surface.status === "ready") return "recovering"
  return surface.status
}

function projectSurface(route: AppRoute): ProjectSurface | undefined {
  if (route.kind === "project-overview") return "overview"
  if (route.kind === "board" || route.kind === "health" || route.kind === "maintenance") return "tasks"
  return undefined
}

function projectOverviewStatus(surface: BoardListSurface | undefined): ProjectOverviewStatus {
  if (surface === undefined) return "ready"
  if (surface.isRefreshing && surface.status === "ready") return "recovering"
  if (surface.status === "loading") return "ready"
  return surface.status
}

function routeTitle(route: AppRoute, project: BoardListItem | undefined, t: ReturnType<typeof createTranslator>): string {
  if (route.kind === "home") return t("projects")
  if (route.kind === "settings") return t("settings")
  if (route.kind === "project-overview") return project?.name ?? t("overview")
  if (route.kind === "board") {
    if (route.view === "runs") return t("runs")
    if (route.view === "events") return t("events")
    if (route.view === "signals") return t("signals")
    if (route.view === "ontology") return t("ontology")
    return t("tasks")
  }
  if (route.kind === "health") return t("health")
  if (route.kind === "maintenance") return t("maintenance")
  if (route.kind === "not-found" || route.kind === "error") return t("notFound")
  return t("notFound")
}

function ShellNavigation({
  route,
  boardList,
  basePath,
  onNavigate,
  sidebarOpen,
  onSidebarOpenChange,
}: {
  readonly route: AppRoute
  readonly boardList?: BoardListSurface
  readonly basePath: string
  readonly onNavigate?: ProductShellProps["onNavigate"]
  readonly sidebarOpen: boolean
  readonly onSidebarOpenChange: (open: boolean) => void
}) {
  const { locale } = usePreferences()
  const t = createTranslator(locale)
  const [isNarrow, setIsNarrow] = useState(false)
  const routeSlug = activeProjectSlug(route)
  const selectedProject = routeProject(route, boardList?.items)
  const selectedSurface = projectSurface(route)
  const projects = (boardList?.items ?? []).filter((project) => project.archivedAt === null || project.slug === routeSlug)

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return
    const media = window.matchMedia("(max-width: 60rem)")
    const update = () => setIsNarrow(media.matches)
    update()
    media.addEventListener?.("change", update)
    return () => media.removeEventListener?.("change", update)
  }, [])

  const navigate = (target: AppNavigationTarget) => {
    if (onNavigate === undefined) return
    void Promise.resolve(onNavigate(target)).then(() => {
      if (isNarrow) onSidebarOpenChange(false)
    }).catch(() => undefined)
  }

  const openProject = (project: NavigationProject) => navigate({ kind: "project-overview", boardSlug: project.slug })
  const openSurface = (project: NavigationProject, surface: Exclude<ProjectSurface, "projects">) => {
    navigate(surface === "overview"
      ? { kind: "project-overview", boardSlug: project.slug }
      : { kind: "board", boardSlug: project.slug, view: "board" })
  }
  const activeRail: ProductRailItem = route.kind === "settings" ? "settings" : "projects"

  return (
    <div className={styles.productNavigationFrame}>
      <ProductRail
        activeItem={activeRail}
        hrefs={{
          projects: routePath({ kind: "home" }, { basePath }),
          settings: routePath({ kind: "settings" }, { basePath }),
        }}
        onNavigate={(item) => navigate(item === "settings" ? { kind: "settings" } : { kind: "home" })}
        labels={{ productNavigation: t("productNavigation"), projects: t("projects"), settings: t("settings") }}
      />
      <ProjectsSidebar
        projects={projects}
        activeProjectSlug={routeSlug}
        activeSurface={selectedSurface}
        activeSection={selectedProject === undefined ? "projects" : "project"}
        projectStatus={projectPickerStatus(boardList)}
        projectsHref={routePath({ kind: "home" }, { basePath })}
        basePath={basePath}
        projectSnapshotAvailable={projects.length > 0 || boardList?.status === "ready"}
        isRefreshing={boardList?.isRefreshing}
        onProjectSelect={openProject}
        onSurfaceSelect={openSurface}
        onSectionSelect={(section) => {
          if (section === "projects") navigate({ kind: "home" })
        }}
        onProjectRetry={boardList?.onRetry}
        open={isNarrow ? sidebarOpen : undefined}
        onClose={() => onSidebarOpenChange(false)}
        drawerId="product-projects-sidebar"
        labels={{
          productNavigation: t("productNavigation"),
          projects: t("projects"),
          settings: t("settings"),
          home: t("homePlanned"),
          projectPicker: t("projectPicker"),
          projectSearch: t("projectSearch"),
          projectSearchPlaceholder: t("projectSearchPlaceholder"),
          clearSearch: t("clearSearch"),
          projectSearchEmpty: t("projectSearchEmpty"),
          projectsEmpty: t("projectsEmpty"),
          projectLoading: t("projectCollectionLoading"),
          projectOffline: t("projectCollectionOfflineDetail"),
          projectError: t("projectCollectionErrorDetail"),
          projectStale: t("projectCollectionStaleDetail"),
          projectRecovering: t("projectCollectionRecoveringDetail"),
          retry: t("retry"),
          close: t("close"),
          openProjectNavigation: t("openProjectNavigation"),
          closeProjectNavigation: t("closeProjectNavigation"),
          collapse: t("collapse"),
          expand: t("expand"),
          overview: t("overview"),
          tasks: t("tasks"),
          archived: t("archived"),
          more: t("more"),
          breadcrumb: t("breadcrumb"),
        }}
      />
    </div>
  )
}

function shouldUseSpaNavigation(event: MouseEvent<HTMLAnchorElement | HTMLButtonElement>): boolean {
  return event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey
}

type StructuredNavigationTarget = Exclude<AppNavigationTarget, string>

function moreItems(
  runtime: WebRuntimeConfig,
  slug: CanonicalBoardSlug | undefined,
  t: ReturnType<typeof createTranslator>,
  onNavigate: ProductShellProps["onNavigate"],
): readonly ResourceHeaderMoreItem[] {
  if (slug === undefined) return []
  const navigationItem = (
    id: string,
    label: string,
    target: StructuredNavigationTarget,
    icon?: ReactNode,
  ): ResourceHeaderMoreItem => ({
    id,
    label,
    icon,
    href: routePath(target, { basePath: runtime.webBasePath }),
    onSelect: onNavigate === undefined
      ? undefined
      : (event) => {
          if (!shouldUseSpaNavigation(event)) return
          event.preventDefault()
          void onNavigate(target)
        },
  })
  const items = ([
    ["runs", t("runs"), "activity"],
    ["events", t("events"), "activity"],
    ["signals", t("signals"), "activity"],
    ["ontology", t("ontology"), "activity"],
  ] as const).map(([id, label, icon]) => navigationItem(
    id,
    label,
    { kind: "board", boardSlug: slug, view: id },
    <span aria-hidden="true"><span className={styles.diagnosticDot} data-icon={icon} /></span>,
  ))
  return [
    ...items,
    navigationItem("health", t("health"), { kind: "health", boardSlug: slug }),
    navigationItem("maintenance", t("maintenance"), { kind: "maintenance", boardSlug: slug }),
  ]
}

function RouteContent({
  runtime,
  route,
  canonicalBoardSlug,
  boardList,
  children,
  boundary,
  error,
  onNavigate,
  onReconnect,
  onRetry,
  invalidationRevision = 0,
  boardRevision = invalidationRevision,
  inspectorRevision = invalidationRevision,
  runsRevision = invalidationRevision,
  eventsRefreshRevision = invalidationRevision,
  eventsBatch,
  syncStatus,
  taskMutations,
  onVisibleCanonicalReloadChange,
}: ProductShellProps) {
  const preferences = usePreferences()
  const t = createTranslator(preferences.locale)
  const [isOnline, setIsOnline] = useState(() => typeof navigator === "undefined" || navigator.onLine)
  const effectiveBoundary = boundary ?? (isOnline ? "ready" : "offline")
  const childNavigate = onNavigate === undefined
    ? undefined
    : async (target: AppNavigationTarget, options?: { readonly replace?: boolean }): Promise<void> => {
        await onNavigate(target, options)
      }
  const project = routeProject(route, boardList?.items)
  const requiresProject = route.kind === "board" || route.kind === "project-overview"
  const missingReadyProject = requiresProject && boardList?.status === "ready" && boardList.isRefreshing !== true && project === undefined
  const archivedTasksUnavailable = route.kind === "board" && project !== undefined && project.archivedAt !== null

  useEffect(() => {
    const handleOnline = () => setIsOnline(true)
    const handleOffline = () => setIsOnline(false)
    window.addEventListener("online", handleOnline)
    window.addEventListener("offline", handleOffline)
    return () => {
      window.removeEventListener("online", handleOnline)
      window.removeEventListener("offline", handleOffline)
    }
  }, [])

  if (missingReadyProject) {
    return (
      <section className={styles.boundary} role="alert" data-testid="shell-project-not-found">
        <p className={styles.eyebrow}>{t("routeBoundary")}</p>
        <h1>{t("notFound")}</h1>
        <p>{t("projectNotFoundDescription")}</p>
        <code translate="no">{route.pathname}</code>
      </section>
    )
  }
  if (archivedTasksUnavailable && project !== undefined) {
    const overviewTarget = { kind: "project-overview" as const, boardSlug: project.slug }
    const overviewHref = routePath(overviewTarget, { basePath: runtime.webBasePath })
    return (
      <section className={styles.boundary} role="alert" data-testid="shell-project-archived">
        <p className={styles.eyebrow}>{t("routeBoundary")}</p>
        <h1>{t("archived")}</h1>
        <p>{t("archivedTasksUnavailable")}</p>
        <a
          className={styles.secondaryAction}
          href={overviewHref}
          onClick={(event) => {
            if (childNavigate === undefined || !shouldUseSpaNavigation(event)) return
            event.preventDefault()
            void childNavigate(overviewTarget)
          }}
        >
          {t("overview")}
        </a>
      </section>
    )
  }
  const ownsFeatureRoute = route.kind === "board" && (route.view === "signals" || route.view === "ontology")
  if (ownsFeatureRoute && children) {
    return <BrowserConnectivityProvider online={isOnline}>{children}</BrowserConnectivityProvider>
  }
  if (effectiveBoundary === "loading") {
    return <section className={styles.boundary} role="status" aria-live="polite" data-testid="shell-loading"><p className={styles.eyebrow}>{t("routeBoundary")}</p><h1>{t("loading")}</h1></section>
  }
  if (effectiveBoundary === "error") {
    return (
      <section className={styles.boundary} role="alert" data-testid="shell-error">
        <p className={styles.eyebrow}>{t("routeBoundary")}</p>
        <h1>{t("error")}</h1>
        <p>{error ?? t("errorDescription")}</p>
        {onRetry ? <Button label={t("retry")} variant="secondary" onClick={onRetry} /> : null}
      </section>
    )
  }
  if (effectiveBoundary === "offline" && route.kind !== "board" && route.kind !== "project-overview" && route.kind !== "health" && route.kind !== "maintenance" && route.kind !== "settings" && route.kind !== "home") {
    return <section className={styles.boundary} role="status" aria-live="polite" data-testid="shell-offline"><p className={styles.eyebrow}>{t("routeBoundary")}</p><h1>{t("offline")}</h1><p>{t("offlineDescription")}</p></section>
  }
  if (route.kind === "home") {
    return <ProjectsCollection projects={boardList?.items ?? []} basePath={runtime.webBasePath} status={projectPickerStatus(boardList)} isRefreshing={boardList?.isRefreshing} onRetry={boardList?.onRetry} onOpenProject={(next) => void childNavigate?.({ kind: "project-overview", boardSlug: next.slug })} />
  }
  if (route.kind === "not-found") {
    return <section className={styles.boundary} role="alert" data-testid="shell-not-found"><p className={styles.eyebrow}>{t("routeBoundary")}</p><h1>{t("notFound")}</h1><p>{t("notFoundDescription")}</p><code translate="no">{route.pathname}</code></section>
  }
  if (route.kind === "error") {
    return <section className={styles.boundary} role="alert" data-testid="shell-route-error"><p className={styles.eyebrow}>{t("routeBoundary")}</p><h1>{t("invalidBoardSlug")}</h1><p>{t("invalidBoardSlugDescription")}</p><code translate="no">{route.pathname}</code></section>
  }

  const hiddenSession = children ? <div hidden aria-hidden="true" data-testid="board-live-session">{children}</div> : null
  if (route.kind === "settings") return <>{hiddenSession}<OperatorSettingsPage runtime={runtime} boardSlug={canonicalBoardSlug} onNavigate={childNavigate} onReconnect={onReconnect} /></>
  if (route.kind === "health") return <>{hiddenSession}<HealthPage runtime={runtime} /></>
  if (route.kind === "maintenance") return <>{hiddenSession}<MaintenancePage runtime={runtime} boardSlug={route.boardSlug} /></>
  if (route.kind === "project-overview") {
    if (project !== undefined) {
      return <ProjectOverview project={project} basePath={runtime.webBasePath} status={projectOverviewStatus(boardList)} onRetry={boardList?.onRetry} onOpenTasks={childNavigate ? () => void childNavigate({ kind: "board", boardSlug: project.slug, view: "board" }) : undefined} />
    }
    if (boardList?.status === "offline" || boardList?.status === "error" || boardList?.status === "stale") {
      const detail = boardList.status === "offline"
        ? t("projectCollectionOfflineDetail")
        : boardList.status === "stale"
          ? t("projectCollectionStaleDetail")
          : t("projectCollectionErrorDetail")
      return (
        <section className={styles.boundary} role={boardList.status === "error" || boardList.status === "offline" ? "alert" : "status"} data-testid="project-overview-unavailable" data-status={boardList.status}>
          <p className={styles.eyebrow}>{t("routeBoundary")}</p>
          <h1>{t("projects")}</h1>
          <p>{detail}</p>
          {boardList.onRetry !== undefined ? <Button label={t("retry")} variant="secondary" onClick={boardList.onRetry} /> : null}
        </section>
      )
    }
    return <section className={styles.boundary} role="status" data-testid="project-overview-loading"><h1>{t("loading")}</h1></section>
  }
  return (
    <>
      {children ? <div hidden aria-hidden="true" data-testid="board-live-session">{children}</div> : null}
      <ExplorerPage runtime={runtime} route={route} onNavigate={childNavigate} online={isOnline} invalidationRevision={invalidationRevision} boardRevision={boardRevision} inspectorRevision={inspectorRevision} runsRevision={runsRevision} eventsRefreshRevision={eventsRefreshRevision} eventsBatch={eventsBatch} syncStatus={syncStatus} taskMutations={taskMutations} onVisibleCanonicalReloadChange={onVisibleCanonicalReloadChange} />
    </>
  )
}

export function ProductShell({
  runtime,
  route,
  canonicalBoardSlug,
  boardList,
  children,
  boundary,
  error,
  onNavigate,
  onReconnect,
  onRetry,
  invalidationRevision = 0,
  boardRevision = invalidationRevision,
  inspectorRevision = invalidationRevision,
  runsRevision = invalidationRevision,
  eventsRefreshRevision = invalidationRevision,
  eventsBatch,
  syncStatus,
  taskMutations,
  onVisibleCanonicalReloadChange,
}: ProductShellProps) {
  const preferences = usePreferences()
  const t = createTranslator(preferences.locale)
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const project = routeProject(route, boardList?.items)
  const slug = activeProjectSlug(route)
  const title = routeTitle(route, project, t)
  const breadcrumbs = route.kind === "home"
      ? [{ label: t("projects") }]
      : route.kind === "settings"
        ? [{ label: t("settings") }]
        : project !== undefined && slug !== undefined
        ? [{ label: project.name, href: routePath({ kind: "project-overview", boardSlug: slug }, { basePath: runtime.webBasePath }) }, { label: route.kind === "project-overview" ? t("overview") : title }]
        : [{ label: title }]
  const actions = route.kind === "project-overview" && project !== undefined && project.archivedAt === null && onNavigate !== undefined
    ? [{ id: "open-tasks", label: t("tasks"), kind: "primary" as const, onSelect: () => void onNavigate({ kind: "board", boardSlug: project.slug, view: "board" }) }]
    : []
  const headerMoreItems = !usesTasksWorkspaceChrome(route) && (boardList === undefined || project?.archivedAt === null)
    ? moreItems(runtime, slug, t, onNavigate)
    : []

  return (
    <div className={navigationStyles.navigationRoot} data-theme={preferences.theme === "dark" ? "dark" : preferences.theme === "light" ? "light" : undefined} data-density={preferences.density} data-testid="product-shell">
      <ShellNavigation route={route} boardList={boardList} basePath={runtime.webBasePath} onNavigate={onNavigate} sidebarOpen={sidebarOpen} onSidebarOpenChange={setSidebarOpen} />
      <main className={styles.productNavigationMain} aria-label={t("productName")}>
        <ResourceHeader
          breadcrumbs={breadcrumbs}
          projectSwitchLabel={project === undefined || route.kind === "settings" ? undefined : project.name}
          projectSwitchAriaLabel={t("projectSwitcher")}
          onProjectSwitch={onNavigate === undefined ? undefined : () => void onNavigate({ kind: "home" })}
          actions={actions}
          moreItems={headerMoreItems}
          onMenuToggle={() => setSidebarOpen((open) => !open)}
          menuOpen={sidebarOpen}
          menuControlsId="product-projects-sidebar"
          labels={{
            projects: t("projects"),
            settings: t("settings"),
            overview: t("overview"),
            tasks: t("tasks"),
            more: t("more"),
            productNavigation: t("productNavigation"),
            projectPicker: t("projectPicker"),
            openProjectNavigation: t("openProjectNavigation"),
            closeProjectNavigation: t("closeProjectNavigation"),
            breadcrumb: t("breadcrumb"),
          }}
        />
        <div className={styles.mainFrame} data-runtime-api-base-url={runtime.apiBaseUrl} data-runtime-actor={runtime.actor} data-runtime-default-board={runtime.defaultBoard} data-runtime-server-version={runtime.serverVersion} data-runtime-protocol-version={runtime.protocolVersion} data-runtime-web-build-id={runtime.webBuildId} data-runtime-web-base-path={runtime.webBasePath}>
          <RouteContent runtime={runtime} route={route} canonicalBoardSlug={canonicalBoardSlug} boardList={boardList} boundary={boundary} error={error} onNavigate={onNavigate} onReconnect={onReconnect} onRetry={onRetry} invalidationRevision={invalidationRevision} boardRevision={boardRevision} inspectorRevision={inspectorRevision} runsRevision={runsRevision} eventsRefreshRevision={eventsRefreshRevision} eventsBatch={eventsBatch} syncStatus={syncStatus} taskMutations={taskMutations} onVisibleCanonicalReloadChange={onVisibleCanonicalReloadChange}>
            {children}
          </RouteContent>
        </div>
      </main>
    </div>
  )
}

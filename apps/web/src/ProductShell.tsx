import { AppShell } from "@astryxdesign/core/AppShell"
import { Button } from "@astryxdesign/core/Button"
import { Layout } from "@astryxdesign/core/Layout"
import { LayoutContent } from "@astryxdesign/core/Layout"
import { SideNav } from "@astryxdesign/core/SideNav"
import { SideNavHeading, SideNavItem, SideNavSection } from "@astryxdesign/core/SideNav"
import { useEffect, useState, type MouseEvent, type ReactNode } from "react"

import type { CanonicalBoardSlug } from "./lib/board-slug"
import type { BoardEventsBatch } from "./lib/api/explorer-read-model"
import type { BoardTaskCanonicalReloadHandler, BoardTaskMutationSurface } from "./features/board/task-mutation-state"
import type { BoardSyncStatus } from "./features/board/types"
import type { WebRuntimeConfig } from "./lib/runtime"
import { routePath, type AppNavigationTarget, type AppRoute } from "./lib/router"
import { usePreferences } from "./lib/use-preferences"
import { createTranslator } from "./lib/i18n"
import { BrowserConnectivityProvider } from "./lib/browser-connectivity-provider"
import { ExplorerPage } from "./features/explorer/ExplorerPage"
import { HealthPage } from "./features/health/HealthPage"
import { MaintenancePage } from "./features/maintenance/MaintenancePage"
import { SettingsPage as OperatorSettingsPage } from "./features/settings/SettingsPage"
import { type BoardReconnectResult } from "./features/board/board-session-registry"
import styles from "./shell.module.css"

export type ShellBoundary = "ready" | "loading" | "error" | "offline"

export type ProductShellProps = {
  runtime: WebRuntimeConfig
  route: AppRoute
  canonicalBoardSlug?: CanonicalBoardSlug
  children?: ReactNode
  boundary?: ShellBoundary
  error?: ReactNode
  onNavigate?: (target: AppNavigationTarget) => void | Promise<unknown>
  onReconnect?: () => BoardReconnectResult | boolean | void | Promise<BoardReconnectResult | boolean | void>
  onRetry?: () => void
  /** 现有 persistent SSE integration 的可选只读 seam。 */
  invalidationRevision?: number
  boardRevision?: number
  inspectorRevision?: number
  runsRevision?: number
  /** 仅 recovery/gap/poll boundaries 触发 Events catch-up read。 */
  eventsRefreshRevision?: number
  eventsBatch?: BoardEventsBatch | null
  syncStatus?: BoardSyncStatus
  taskMutations?: BoardTaskMutationSurface
  /** Register the currently visible Inspector reads for awaited canonical reloads. */
  onVisibleCanonicalReloadChange?: (reload: BoardTaskCanonicalReloadHandler | undefined, releasedReload?: BoardTaskCanonicalReloadHandler) => void
}

function appRoutePathname(route: AppRoute): string {
  return route.pathname
}

function navPath(runtime: WebRuntimeConfig, boardSlug?: CanonicalBoardSlug): string {
  return boardSlug
    ? routePath({ kind: "board", boardSlug }, { basePath: runtime.webBasePath })
    : routePath({ kind: "home" }, { basePath: runtime.webBasePath })
}

function featureNavPath(runtime: WebRuntimeConfig, boardSlug: CanonicalBoardSlug, view: "signals" | "ontology"): string {
  return routePath({ kind: "board", boardSlug, view }, { basePath: runtime.webBasePath })
}

function StaticIcon({ children }: { children: ReactNode }) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 24 24" width="1.25em" height="1.25em">
      {children}
    </svg>
  )
}

function BoardIcon() {
  return (
    <StaticIcon>
      <path d="M4 5.5A1.5 1.5 0 0 1 5.5 4h13A1.5 1.5 0 0 1 20 5.5v13a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 18.5v-13Zm3 1.5v10h3V7H7Zm5 0v10h5V7h-5Z" fill="currentColor" />
    </StaticIcon>
  )
}

function HealthIcon() {
  return (
    <StaticIcon>
      <path d="M12 3.5a8.5 8.5 0 1 0 8.5 8.5A8.51 8.51 0 0 0 12 3.5Zm0 2a6.5 6.5 0 1 1-6.5 6.5A6.51 6.51 0 0 1 12 5.5Zm-.9 2.2v3.4H7.7v1.8h3.4v3.4h1.8v-3.4h3.4v-1.8h-3.4V7.7Z" fill="currentColor" />
    </StaticIcon>
  )
}

function MaintenanceIcon() {
  return (
    <StaticIcon>
      <path d="m14.8 4.1 1.1 1.1-5.4 5.4 2.9 2.9 5.4-5.4 1.1 1.1-1.1 4-3.4 3.4-4-1.1-5.1 5.1a1.6 1.6 0 0 1-2.3-2.3l5.1-5.1-1.1-4 3.4-3.4 3.4-1.7Zm-2.5 2-1.8.9-2 2 .6 2.1 1.4 1.4 2.1.6 2-2 .9-1.8-1.5-1.5-1.7.3Z" fill="currentColor" />
    </StaticIcon>
  )
}

function SettingsIcon() {
  return (
    <StaticIcon>
      <path d="m12 3 1.1 1.9 2.2.7 2-.9 1.8 1.8-.9 2 .7 2.2L21 12l-2.1 1.1-.7 2.2.9 2-1.8 1.8-2-.9-2.2.7L12 21l-1.1-2.1-2.2-.7-2 .9-1.8-1.8.9-2L5.1 13 3 12l2.1-1.1.7-2.2-.9-2 1.8-1.8 2 .9 2.2-.7L12 3Zm0 5.5A3.5 3.5 0 1 0 12 15a3.5 3.5 0 0 0 0-6.5Z" fill="currentColor" />
    </StaticIcon>
  )
}

function CompactNavItem({
  label,
  href,
  icon,
  isSelected,
  isDisabled,
  onClick,
  testId,
}: {
  label: string
  href: string
  icon: ReactNode
  isSelected: boolean
  isDisabled: boolean
  onClick: (event: MouseEvent) => void
  testId: string
}) {
  const itemProps: {
    "aria-current": "page" | undefined
    "aria-label": string
    "data-testid": string
    title: string
  } = {
    "aria-current": isSelected ? "page" : undefined,
    "aria-label": label,
    "data-testid": testId,
    title: label,
  }
  return (
    <div className={styles.compactNavItemWrapper}>
      {isDisabled ? (
        <button type="button" className={styles.compactNavItem} disabled {...itemProps}>
          {icon}
        </button>
      ) : (
        <a className={styles.compactNavItem} href={href} onClick={onClick} {...itemProps}>
          {icon}
        </a>
      )}
    </div>
  )
}

function ShellNav({ runtime, route, canonicalBoardSlug, onNavigate }: Pick<ProductShellProps, "runtime" | "route" | "canonicalBoardSlug" | "onNavigate">) {
  const { sidebarExpanded, setSidebarExpanded, locale } = usePreferences()
  const t = createTranslator(locale)
  const handleNavigate = (target: string) => (event: MouseEvent) => {
    if (!onNavigate) return
    event.preventDefault()
    void Promise.resolve()
      .then(() => onNavigate(target))
      .catch(() => undefined)
  }
  const settingsPath = routePath({ kind: "settings" }, { basePath: runtime.webBasePath })
  const routeBoardSlug = route.kind === "board" || route.kind === "health" || route.kind === "maintenance" ? route.boardSlug : undefined
  const activeBoardSlug = canonicalBoardSlug ?? routeBoardSlug
  const boardPath = navPath(runtime, activeBoardSlug)
  const healthPath = activeBoardSlug
    ? routePath({ kind: "health", boardSlug: activeBoardSlug }, { basePath: runtime.webBasePath })
    : boardPath
  const maintenancePath = activeBoardSlug
    ? routePath({ kind: "maintenance", boardSlug: activeBoardSlug }, { basePath: runtime.webBasePath })
    : boardPath

  return (
    <SideNav
      data-testid="product-side-nav"
      header={
        <SideNavHeading
          superheading="KANBAN TOOL"
          heading={t("productName")}
          headingHref={runtime.webBasePath}
          onClick={handleNavigate(runtime.webBasePath)}
        />
      }
      collapsible={{
        isCollapsed: !sidebarExpanded,
        onCollapsedChange: (isCollapsed) => setSidebarExpanded(!isCollapsed),
        buttonLabel: sidebarExpanded ? t("collapseSidebar") : t("expandSidebar"),
      }}
    >
      {sidebarExpanded ? (
        <>
          <SideNavSection title={t("workspace")}>
            <SideNavItem
              label={t("board")}
              icon={<BoardIcon />}
              selectedIcon={<BoardIcon />}
              href={boardPath}
              isSelected={route.kind === "board" && route.view !== "signals" && route.view !== "ontology"}
              isDisabled={!activeBoardSlug}
              onClick={handleNavigate(boardPath)}
              data-testid="nav-board"
            />
          </SideNavSection>
          <SideNavSection title={t("navigation")}>
            {activeBoardSlug ? (
              <>
                <SideNavItem
                  label={t("signals")}
                  icon={<BoardIcon />}
                  selectedIcon={<BoardIcon />}
                  href={featureNavPath(runtime, activeBoardSlug, "signals")}
                  isSelected={route.kind === "board" && route.view === "signals"}
                  onClick={handleNavigate(featureNavPath(runtime, activeBoardSlug, "signals"))}
                  data-testid="nav-signals"
                />
                <SideNavItem
                  label={t("ontology")}
                  icon={<BoardIcon />}
                  selectedIcon={<BoardIcon />}
                  href={featureNavPath(runtime, activeBoardSlug, "ontology")}
                  isSelected={route.kind === "board" && route.view === "ontology"}
                  onClick={handleNavigate(featureNavPath(runtime, activeBoardSlug, "ontology"))}
                  data-testid="nav-ontology"
                />
                <SideNavItem
                  label={t("health")}
                  icon={<HealthIcon />}
                  selectedIcon={<HealthIcon />}
                  href={healthPath}
                  isSelected={route.kind === "health"}
                  onClick={handleNavigate(healthPath)}
                  data-testid="nav-health"
                />
                <SideNavItem
                  label={t("maintenance")}
                  icon={<MaintenanceIcon />}
                  selectedIcon={<MaintenanceIcon />}
                  href={maintenancePath}
                  isSelected={route.kind === "maintenance"}
                  onClick={handleNavigate(maintenancePath)}
                  data-testid="nav-maintenance"
                />
              </>
            ) : null}
            <SideNavItem
              label={t("settings")}
              icon={<SettingsIcon />}
              selectedIcon={<SettingsIcon />}
              href={settingsPath}
              isSelected={route.kind === "settings"}
              onClick={handleNavigate(settingsPath)}
              data-testid="nav-settings"
            />
          </SideNavSection>
        </>
      ) : (
        <>
          <CompactNavItem
            label={t("board")}
            icon={<BoardIcon />}
            href={boardPath}
            isSelected={route.kind === "board" && route.view !== "signals" && route.view !== "ontology"}
            isDisabled={!activeBoardSlug}
            onClick={handleNavigate(boardPath)}
            testId="nav-board"
          />
          {activeBoardSlug ? (
            <>
              <CompactNavItem
                label={t("signals")}
                icon={<BoardIcon />}
                href={featureNavPath(runtime, activeBoardSlug, "signals")}
                isSelected={route.kind === "board" && route.view === "signals"}
                isDisabled={false}
                onClick={handleNavigate(featureNavPath(runtime, activeBoardSlug, "signals"))}
                testId="nav-signals"
              />
              <CompactNavItem
                label={t("ontology")}
                icon={<BoardIcon />}
                href={featureNavPath(runtime, activeBoardSlug, "ontology")}
                isSelected={route.kind === "board" && route.view === "ontology"}
                isDisabled={false}
                onClick={handleNavigate(featureNavPath(runtime, activeBoardSlug, "ontology"))}
                testId="nav-ontology"
              />
              <CompactNavItem
                label={t("health")}
                icon={<HealthIcon />}
                href={healthPath}
                isSelected={route.kind === "health"}
                isDisabled={false}
                onClick={handleNavigate(healthPath)}
                testId="nav-health"
              />
              <CompactNavItem
                label={t("maintenance")}
                icon={<MaintenanceIcon />}
                href={maintenancePath}
                isSelected={route.kind === "maintenance"}
                isDisabled={false}
                onClick={handleNavigate(maintenancePath)}
                testId="nav-maintenance"
              />
            </>
          ) : null}
          <CompactNavItem
            label={t("settings")}
            icon={<SettingsIcon />}
            href={settingsPath}
            isSelected={route.kind === "settings"}
            onClick={handleNavigate(settingsPath)}
            isDisabled={false}
            testId="nav-settings"
          />
        </>
      )}
    </SideNav>
  )
}


function RouteContent({ runtime, route, canonicalBoardSlug, children, boundary, error, onNavigate, onReconnect, onRetry, invalidationRevision = 0, boardRevision = invalidationRevision, inspectorRevision = invalidationRevision, runsRevision = invalidationRevision, eventsRefreshRevision = invalidationRevision, eventsBatch, syncStatus, taskMutations, onVisibleCanonicalReloadChange }: ProductShellProps) {
  const preferences = usePreferences()
  const t = createTranslator(preferences.locale)
  const [isOnline, setIsOnline] = useState(() => typeof navigator === "undefined" || navigator.onLine)
  const effectiveBoundary = boundary ?? (isOnline ? "ready" : "offline")

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

  const ownsLiveBoardRoute = route.kind === "home"
  const ownsFeatureRoute = route.kind === "board" && (route.view === "signals" || route.view === "ontology")
  // BoardLive owns loading, empty, stale and offline presentation. Keep the
  // child mounted before generic boundaries; for board routes it remains a
  // hidden session owner and Explorer owns the visible route content below.
  if ((ownsLiveBoardRoute || ownsFeatureRoute) && children) {
    return <BrowserConnectivityProvider online={isOnline}>{children}</BrowserConnectivityProvider>
  }

  if (effectiveBoundary === "loading") {
    return (
      <section className={styles.boundary} role="status" aria-live="polite" data-testid="shell-loading">
        <p className={styles.eyebrow}>{t("routeBoundary")}</p>
        <h1>{t("loading")}</h1>
      </section>
    )
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
  // Explorer owns stale/offline presentation for every board view so a last
  // usable snapshot and the current route remain mounted while connectivity
  // drops. The hidden BoardLive session still owns recovery and retry.
  // Operator pages retain their own stale/error/retry state while offline; a
  // generic shell boundary would unmount their last snapshot.
  if (effectiveBoundary === "offline" && route.kind !== "board" && route.kind !== "health" && route.kind !== "maintenance" && route.kind !== "settings") {
    return (
      <section className={styles.boundary} role="status" aria-live="polite" data-testid="shell-offline">
        <p className={styles.eyebrow}>{t("routeBoundary")}</p>
        <h1>{t("offline")}</h1>
        <p>{t("offlineDescription")}</p>
      </section>
    )
  }

  if (route.kind === "home") {
    return (
      <section className={styles.boundary} role="status" aria-live="polite" data-testid="shell-home-loading">
        <p className={styles.eyebrow}>{t("routeBoundary")}</p>
        <h1>{t("homeLoading")}</h1>
        <p>{t("homeLoadingDescription")}</p>
      </section>
    )
  }
  if (route.kind === "not-found") {
    return (
      <section className={styles.boundary} role="alert" data-testid="shell-not-found">
        <p className={styles.eyebrow}>{t("routeBoundary")}</p>
        <h1>{t("notFound")}</h1>
        <p>{t("notFoundDescription")}</p>
        <code translate="no">{route.pathname}</code>
      </section>
    )
  }
  if (route.kind === "error") {
    return (
      <section className={styles.boundary} role="alert" data-testid="shell-route-error">
        <p className={styles.eyebrow}>{t("routeBoundary")}</p>
        <h1>{t("invalidBoardSlug")}</h1>
        <p>{t("invalidBoardSlugDescription")}</p>
        <code translate="no">{route.pathname}</code>
      </section>
    )
  }
  const hiddenSession = children ? <div hidden aria-hidden="true" data-testid="board-live-session">{children}</div> : null
  if (route.kind === "settings") return <>{hiddenSession}<OperatorSettingsPage runtime={runtime} boardSlug={canonicalBoardSlug} onNavigate={onNavigate} onReconnect={onReconnect} /></>
  if (route.kind === "health") return <>{hiddenSession}<HealthPage runtime={runtime} /></>
  if (route.kind === "maintenance") return <>{hiddenSession}<MaintenancePage runtime={runtime} boardSlug={route.boardSlug} /></>
  if (route.kind === "board") return (
    <>
      {children ? <div hidden aria-hidden="true" data-testid="board-live-session">{children}</div> : null}
      <ExplorerPage runtime={runtime} route={route} onNavigate={onNavigate} online={isOnline} invalidationRevision={invalidationRevision} boardRevision={boardRevision} inspectorRevision={inspectorRevision} runsRevision={runsRevision} eventsRefreshRevision={eventsRefreshRevision} eventsBatch={eventsBatch} syncStatus={syncStatus} taskMutations={taskMutations} onVisibleCanonicalReloadChange={onVisibleCanonicalReloadChange} />
    </>
  )

  return (
    <section className={styles.page} aria-labelledby="board-placeholder-heading" data-testid="board-placeholder">
      <div className={styles.pageHeading}>
        <p className={styles.eyebrow}>{t("productKicker")}</p>
        <h1 id="board-placeholder-heading">{t("boardPlaceholder")}</h1>
        <p className={styles.lede}>{t("boardPlaceholderDescription")}</p>
      </div>
      <p className={styles.routePath} translate="no">{appRoutePathname(route)}</p>
    </section>
  )
}

export function ProductShell({ runtime, route, canonicalBoardSlug, children, boundary, error, onNavigate, onReconnect, onRetry, invalidationRevision = 0, boardRevision = invalidationRevision, inspectorRevision = invalidationRevision, runsRevision = invalidationRevision, eventsRefreshRevision = invalidationRevision, eventsBatch, syncStatus, taskMutations, onVisibleCanonicalReloadChange }: ProductShellProps) {
  const preferences = usePreferences()
  const t = createTranslator(preferences.locale)

  return (
      <AppShell
        variant="elevated"
        height="fill"
        contentPadding={0}
        mobileNav={false}
        sideNav={<ShellNav runtime={runtime} route={route} canonicalBoardSlug={canonicalBoardSlug} onNavigate={onNavigate} />}
        data-testid="product-shell"
      >
        <Layout
          height="auto"
          content={
            <LayoutContent isScrollable={false} padding={6} role="region" label={t("productName")}>
              <div
                className={styles.mainFrame}
                data-runtime-api-base-url={runtime.apiBaseUrl}
                data-runtime-actor={runtime.actor}
                data-runtime-default-board={runtime.defaultBoard}
                data-runtime-server-version={runtime.serverVersion}
                data-runtime-protocol-version={runtime.protocolVersion}
                data-runtime-web-build-id={runtime.webBuildId}
                data-runtime-web-base-path={runtime.webBasePath}
              >
                <RouteContent runtime={runtime} route={route} canonicalBoardSlug={canonicalBoardSlug} boundary={boundary} error={error} onNavigate={onNavigate} onReconnect={onReconnect} onRetry={onRetry} invalidationRevision={invalidationRevision} boardRevision={boardRevision} inspectorRevision={inspectorRevision} runsRevision={runsRevision} eventsRefreshRevision={eventsRefreshRevision} eventsBatch={eventsBatch} syncStatus={syncStatus} taskMutations={taskMutations} onVisibleCanonicalReloadChange={onVisibleCanonicalReloadChange}>
                  {children}
                </RouteContent>
              </div>
            </LayoutContent>
          }
        />
      </AppShell>
  )
}

import { AppShell } from "@astryxdesign/core/AppShell"
import { Button } from "@astryxdesign/core/Button"
import { Layout } from "@astryxdesign/core/Layout"
import { LayoutContent } from "@astryxdesign/core/Layout"
import { SideNav } from "@astryxdesign/core/SideNav"
import { SideNavHeading, SideNavItem, SideNavSection } from "@astryxdesign/core/SideNav"
import { useEffect, useState, type MouseEvent, type ReactNode } from "react"

import type { CanonicalBoardSlug } from "./lib/board-slug"
import type { BoardEventsBatch } from "./lib/api/explorer-read-model"
import type { WebRuntimeConfig } from "./lib/runtime"
import { routePath, type AppNavigationTarget, type AppRoute } from "./lib/router"
import { parseLocalePreference, parseThemePreference } from "./lib/preferences"
import { usePreferences } from "./lib/use-preferences"
import { createTranslator, type MessageKey } from "./lib/i18n"
import { ExplorerPage } from "./features/explorer/ExplorerPage"
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
  onRetry?: () => void
  /** 现有 persistent SSE integration 的可选只读 seam。 */
  invalidationRevision?: number
  eventsBatch?: BoardEventsBatch | null
}

function safeText(value: string): string {
  return value.trim() || "—"
}

function appRoutePathname(route: AppRoute): string {
  return route.pathname
}

function navPath(runtime: WebRuntimeConfig, boardSlug?: CanonicalBoardSlug): string {
  return boardSlug
    ? routePath({ kind: "board", boardSlug }, { basePath: runtime.webBasePath })
    : routePath({ kind: "home" }, { basePath: runtime.webBasePath })
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
  const boardPath = navPath(runtime, canonicalBoardSlug ?? (route.kind === "board" ? route.boardSlug : undefined))

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
              isSelected={route.kind === "board"}
              isDisabled={!canonicalBoardSlug && route.kind !== "board"}
              onClick={handleNavigate(boardPath)}
              data-testid="nav-board"
            />
          </SideNavSection>
          <SideNavSection title={t("navigation")}>
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
            isSelected={route.kind === "board"}
            isDisabled={!canonicalBoardSlug && route.kind !== "board"}
            onClick={handleNavigate(boardPath)}
            testId="nav-board"
          />
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

function RuntimeFacts({ runtime, t }: { runtime: WebRuntimeConfig; t: (key: MessageKey) => string }) {
  const facts = [
    [t("actor"), runtime.actor],
    [t("api"), runtime.apiBaseUrl || "/"],
    [t("server"), runtime.serverVersion],
    [t("protocol"), runtime.protocolVersion],
    [t("build"), runtime.webBuildId],
  ] as const
  return (
    <dl className={styles.runtimeFacts} data-testid="runtime-facts">
      {facts.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd translate="no">{safeText(value)}</dd>
        </div>
      ))}
    </dl>
  )
}

function SettingsPage({ runtime }: { runtime: WebRuntimeConfig }) {
  const preferences = usePreferences()
  const t = createTranslator(preferences.locale)
  return (
    <section className={styles.page} aria-labelledby="settings-heading" data-testid="settings-page">
      <div className={styles.pageHeading}>
        <p className={styles.eyebrow}>{t("productKicker")}</p>
        <h1 id="settings-heading">{t("settingsHeading")}</h1>
        <p className={styles.lede}>{t("settingsDescription")}</p>
      </div>
      <div className={styles.settingsGrid}>
        <div className={styles.settingsSection}>
          <h2>{t("theme")}</h2>
          <label className={styles.field} htmlFor="theme-preference">
            <span>{t("theme")}</span>
            <select
              id="theme-preference"
              name="theme"
              autoComplete="off"
              value={preferences.theme}
              onChange={(event) => {
                const theme = parseThemePreference(event.currentTarget.value)
                if (theme) preferences.setTheme(theme)
              }}
              data-testid="theme-preference"
            >
              <option value="light">{t("lightTheme")}</option>
              <option value="dark">{t("darkTheme")}</option>
            </select>
          </label>
        </div>
        <div className={styles.settingsSection}>
          <h2>{t("language")}</h2>
          <label className={styles.field} htmlFor="locale-preference">
            <span>{t("language")}</span>
            <select
              id="locale-preference"
              name="locale"
              autoComplete="language"
              value={preferences.locale}
              onChange={(event) => {
                const locale = parseLocalePreference(event.currentTarget.value)
                if (locale) preferences.setLocale(locale)
              }}
              data-testid="locale-preference"
            >
              <option value="zh">{t("chinese")}</option>
              <option value="en">{t("english")}</option>
            </select>
          </label>
        </div>
        <div className={styles.settingsSection}>
          <h2>{t("workspace")}</h2>
          <p className={styles.muted}>
            {t("defaultBoard")}: <code className={styles.codeValue} translate="no">{safeText(runtime.defaultBoard)}</code>
          </p>
          <p className={styles.muted}>
            {preferences.sidebarExpanded ? t("sidebarExpanded") : t("sidebarCollapsed")}
          </p>
        </div>
      </div>
      <div className={styles.runtimeSection}>
        <h2>{t("runtime")}</h2>
        <RuntimeFacts runtime={runtime} t={t} />
      </div>
    </section>
  )
}

function RouteContent({ runtime, route, children, boundary, error, onNavigate, onRetry, invalidationRevision = 0, eventsBatch }: ProductShellProps) {
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
  // Events owns a stale/offline presentation so its last usable snapshot can
  // remain mounted while connectivity drops. Other board views keep the
  // shell-level offline boundary (and therefore their existing live semantics).
  if (effectiveBoundary === "offline" && !(route.kind === "board" && route.view === "events")) {
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
  if (route.kind === "settings") return <SettingsPage runtime={runtime} />
  if (children) return <>{children}</>
  if (route.kind === "board") return <ExplorerPage runtime={runtime} route={route} onNavigate={onNavigate} online={isOnline} invalidationRevision={invalidationRevision} eventsBatch={eventsBatch} />

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

export function ProductShell({ runtime, route, canonicalBoardSlug, children, boundary, error, onNavigate, onRetry, invalidationRevision = 0, eventsBatch }: ProductShellProps) {
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
                <RouteContent runtime={runtime} route={route} boundary={boundary} error={error} onNavigate={onNavigate} onRetry={onRetry} invalidationRevision={invalidationRevision} eventsBatch={eventsBatch}>
                  {children}
                </RouteContent>
              </div>
            </LayoutContent>
          }
        />
      </AppShell>
  )
}

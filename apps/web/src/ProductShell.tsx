import { AppShell } from "@astryxdesign/core/AppShell"
import { Layout } from "@astryxdesign/core/Layout"
import { LayoutContent } from "@astryxdesign/core/Layout"
import { SideNav } from "@astryxdesign/core/SideNav"
import { SideNavHeading, SideNavItem, SideNavSection } from "@astryxdesign/core/SideNav"
import { useEffect, useState, type MouseEvent, type ReactNode } from "react"

import type { WebRuntimeConfig } from "./lib/runtime"
import type { AppNavigationTarget, AppRoute } from "./lib/router"
import { usePreferences } from "./lib/use-preferences"
import { createTranslator, type MessageKey } from "./lib/i18n"
import styles from "./shell.module.css"

export type ShellBoundary = "ready" | "loading" | "error" | "offline"

export type ProductShellProps = {
  runtime: WebRuntimeConfig
  route: AppRoute
  canonicalBoardSlug?: string
  children?: ReactNode
  boundary?: ShellBoundary
  error?: ReactNode
  onNavigate?: (target: AppNavigationTarget) => void | Promise<unknown>
}

function safeText(value: string): string {
  return value.trim() || "—"
}

function navPath(runtime: WebRuntimeConfig, boardSlug?: string): string {
  const basePath = runtime.webBasePath.endsWith("/") ? runtime.webBasePath : `${runtime.webBasePath}/`
  return boardSlug ? `${basePath}boards/${encodeURIComponent(boardSlug)}/board` : basePath
}

function ShellNav({ runtime, route, canonicalBoardSlug, onNavigate }: Pick<ProductShellProps, "runtime" | "route" | "canonicalBoardSlug" | "onNavigate">) {
  const { sidebarExpanded, setSidebarExpanded, locale } = usePreferences()
  const t = createTranslator(locale)
  const handleNavigate = (target: string) => (event: MouseEvent) => {
    if (!onNavigate) return
    event.preventDefault()
    void onNavigate(target)
  }
  const settingsPath = `${runtime.webBasePath.replace(/\/$/, "")}/settings`
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
      <SideNavSection title={t("workspace")}>
        <SideNavItem
          label={t("board")}
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
          href={settingsPath}
          isSelected={route.kind === "settings"}
          onClick={handleNavigate(settingsPath)}
          data-testid="nav-settings"
        />
      </SideNavSection>
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
          <dd>{safeText(value)}</dd>
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
              value={preferences.theme}
              onChange={(event) => preferences.setTheme(event.currentTarget.value as typeof preferences.theme)}
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
              value={preferences.locale}
              onChange={(event) => preferences.setLocale(event.currentTarget.value as typeof preferences.locale)}
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
            {t("defaultBoard")}: <code>{safeText(runtime.defaultBoard)}</code>
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

function RouteContent({ runtime, route, children, boundary, error }: Omit<ProductShellProps, "onNavigate">) {
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
      </section>
    )
  }
  if (effectiveBoundary === "offline") {
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
        <code>{route.pathname}</code>
      </section>
    )
  }
  if (route.kind === "settings") return <SettingsPage runtime={runtime} />
  if (children) return <>{children}</>

  return (
    <section className={styles.page} aria-labelledby="board-placeholder-heading" data-testid="board-placeholder">
      <div className={styles.pageHeading}>
        <p className={styles.eyebrow}>{t("productKicker")}</p>
        <h1 id="board-placeholder-heading">{t("boardPlaceholder")}</h1>
        <p className={styles.lede}>{t("boardPlaceholderDescription")}</p>
      </div>
      <p className={styles.routePath}>{route.pathname}</p>
    </section>
  )
}

export function ProductShell({ runtime, route, canonicalBoardSlug, children, boundary, error, onNavigate }: ProductShellProps) {
  const preferences = usePreferences()
  const t = createTranslator(preferences.locale)
  const mainId = "astryx-app-shell-main"

  return (
    <>
      <a className={styles.skipLink} href={`#${mainId}`}>
        {t("skipToContent")}
      </a>
      <AppShell
        variant="elevated"
        height="fill"
        contentPadding={0}
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
                <RouteContent runtime={runtime} route={route} boundary={boundary} error={error}>
                  {children}
                </RouteContent>
              </div>
            </LayoutContent>
          }
        />
      </AppShell>
    </>
  )
}

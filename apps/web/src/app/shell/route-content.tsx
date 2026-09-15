import { useEffect, useState } from 'react';
import { Button } from "../../components/ui/button";
import { usePreferences } from "../../platform/preferences/use-preferences";
import { createTranslator } from "../../application/i18n";
import { BrowserConnectivityProvider } from "../../platform/connectivity/browser-connectivity-provider";
import { ExplorerPage } from "../../features/tasks/index";
import { HealthPage } from "../../features/health/index";
import { MaintenancePage } from "../../features/maintenance/index";
import { SettingsPage as OperatorSettingsPage } from "../../features/settings/index";
import type { ProductShellProps } from "./shell-contract";
import styles from "./boundary.module.css";
export function RouteContent({ runtime, route, canonicalBoardSlug, children, boundary, error, onNavigate, onReconnect, onRetry, invalidationRevision = 0, boardRevision = invalidationRevision, inspectorRevision = invalidationRevision, runsRevision = invalidationRevision, eventsRefreshRevision = invalidationRevision, eventsBatch, syncStatus, taskMutations, onVisibleCanonicalReloadChange }: ProductShellProps) {
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
  // BoardLive owns loading, empty, stale and offline presentation. Keep the
  // child mounted before generic boundaries; for board routes it remains a
  // hidden session owner and Explorer owns the visible route content below.
  if (ownsLiveBoardRoute && children) {
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

  return null
}

import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react"

import type { AppNavigationTarget } from "../../lib/router"
import { routePath } from "../../lib/router"
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "../../lib/board-slug"
import { readHealth, type HealthReport } from "../../lib/api/health-read-model"
import { presentHealthError } from "../health/health-error"
import { isCurrentHealthRequest } from "../health/health-request"
import type { WebRuntimeConfig } from "../../lib/runtime"
import {
  parseActorPreference,
  parseDensityPreference,
  parseLocalePreference,
  parseThemePreference,
} from "../../lib/preferences"
import { createTranslator } from "../../lib/i18n"
import { usePreferences } from "../../lib/use-preferences"
import { callSettingsAction } from "./settings-async-actions"
import { apiOriginForRuntime, diagnosticsText } from "./settings-diagnostics"
import type { BoardReconnectResult } from "../board/board-session-registry"
import styles from "../../shell.module.css"

export type SettingsPageProps = {
  readonly runtime: WebRuntimeConfig
  /** Canonical board resolved by the shell; a runtime selector is not trusted as one. */
  readonly boardSlug?: string
  readonly initialHealth?: HealthReport
  readonly read?: (signal?: AbortSignal) => Promise<HealthReport>
  readonly onNavigate?: (target: AppNavigationTarget) => void | Promise<unknown>
  /** Reconnects the existing canonical board session; it must not create a new transport. */
  readonly onReconnect?: () => BoardReconnectResult | boolean | void | Promise<BoardReconnectResult | boolean | void>
  readonly clipboardWrite?: (text: string) => Promise<void> | void
}

type HealthState =
  | { readonly kind: "loading" }
  | { readonly kind: "ready"; readonly report: HealthReport; readonly staleError?: unknown }
  | { readonly kind: "error"; readonly error: unknown }

function reported(value: string | null | undefined, fallback: string): string {
  return value?.trim() || fallback
}

function canonicalBoard(value: string | undefined): CanonicalBoardSlug | null {
  return parseCanonicalBoardSlug(value)
}

function browserClipboardWrite(text: string): Promise<void> {
  const clipboard = typeof navigator !== "undefined" ? navigator.clipboard : undefined
  if (!clipboard?.writeText) return Promise.reject(new Error("clipboard unavailable"))
  return clipboard.writeText(text)
}

export function SettingsPage({
  runtime,
  boardSlug: boardSlugInput,
  initialHealth,
  read,
  onNavigate,
  onReconnect,
  clipboardWrite,
}: SettingsPageProps) {
  const preferences = usePreferences()
  const t = createTranslator(preferences.locale)
  const boardSlug = useMemo(
    () => canonicalBoard(boardSlugInput),
    [boardSlugInput],
  )
  const healthURL = boardSlug ? routePath({ kind: "health", boardSlug }, { basePath: runtime.webBasePath }) : null
  const [healthState, setHealthState] = useState<HealthState>(() =>
    initialHealth ? { kind: "ready", report: initialHealth } : { kind: "loading" },
  )
  const [healthPending, setHealthPending] = useState(!initialHealth)
  const healthControllerRef = useRef<AbortController | null>(null)
  const [actorDraft, setActorDraft] = useState(() => preferences.actor || runtime.actor)
  const [actorTouched, setActorTouched] = useState(false)
  const [actorSaved, setActorSaved] = useState(false)
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle")
  const [copyPending, setCopyPending] = useState(false)
  const [reconnectState, setReconnectState] = useState<"idle" | "pending" | "done" | "already" | "unavailable">("idle")
  const mountedRef = useRef(true)
  const copyPendingRef = useRef(false)
  const reconnectPendingRef = useRef(false)

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      healthControllerRef.current?.abort()
      healthControllerRef.current = null
    }
  }, [])

  useEffect(() => {
    if (!actorTouched) setActorDraft(preferences.actor || runtime.actor)
  }, [actorTouched, preferences.actor, runtime.actor])

  const loadHealth = useCallback(async () => {
    if (healthControllerRef.current) return
    const controller = new AbortController()
    healthControllerRef.current = controller
    setHealthPending(true)
    const reader = read ?? ((signal?: AbortSignal) => readHealth({ runtime, signal }))
    try {
      const report = await reader(controller.signal)
      if (!isCurrentHealthRequest(controller, healthControllerRef.current)) return
      setHealthState({ kind: "ready", report })
    } catch (error: unknown) {
      if (!isCurrentHealthRequest(controller, healthControllerRef.current) || (error instanceof Error && error.name === "AbortError")) return
      setHealthState((previous) => previous.kind === "ready"
        ? { kind: "ready", report: previous.report, staleError: error }
        : { kind: "error", error })
    } finally {
      if (healthControllerRef.current === controller) {
        healthControllerRef.current = null
        setHealthPending(false)
      }
    }
  }, [read, runtime])

  useEffect(() => {
    if (!initialHealth) void loadHealth()
  }, [initialHealth, loadHealth])

  const actorError = actorTouched && parseActorPreference(actorDraft) === null ? t("identityActorInvalid") : null
  const saveActor = () => {
    setActorTouched(true)
    const actor = parseActorPreference(actorDraft)
    if (actor === null) {
      setActorSaved(false)
      return
    }
    preferences.setActor(actor)
    setActorDraft(actor)
    setActorSaved(true)
  }

  const resetActor = () => {
    preferences.setActor("")
    setActorDraft(runtime.actor)
    setActorTouched(false)
    setActorSaved(true)
  }

  const copyDiagnostics = useCallback(() => {
    if (copyPendingRef.current) return
    copyPendingRef.current = true
    setCopyPending(true)
    setCopyState("idle")
    const health = healthState.kind === "ready" ? healthState.report : null
    const writer = clipboardWrite ?? browserClipboardWrite
    void callSettingsAction(() => writer(diagnosticsText(runtime, health)))
      .then(() => {
        if (mountedRef.current) setCopyState("copied")
      })
      .catch(() => {
        if (mountedRef.current) setCopyState("failed")
      })
      .finally(() => {
        copyPendingRef.current = false
        if (mountedRef.current) setCopyPending(false)
      })
  }, [clipboardWrite, healthState, runtime])

  const reconnect = useCallback(() => {
    if (!boardSlug || !onReconnect || reconnectPendingRef.current) return
    reconnectPendingRef.current = true
    setReconnectState("pending")
    void callSettingsAction(() => onReconnect())
      .then((reconnected) => {
        if (!mountedRef.current) return
        setReconnectState(reconnected === false || reconnected === "unavailable"
          ? "unavailable"
          : reconnected === "already-live" ? "already" : "done")
      })
      .catch(() => {
        if (mountedRef.current) setReconnectState("unavailable")
      })
      .finally(() => {
        reconnectPendingRef.current = false
      })
  }, [boardSlug, onReconnect])

  const navigateHealth = useCallback((event: MouseEvent<HTMLAnchorElement>) => {
    if (!healthURL || !onNavigate) return
    event.preventDefault()
    void Promise.resolve(onNavigate(healthURL)).catch(() => undefined)
  }, [healthURL, onNavigate])

  const healthError = healthState.kind === "error" || healthState.kind === "ready" && healthState.staleError
    ? presentHealthError(healthState.kind === "error" ? healthState.error : healthState.staleError, t)
    : null
  const healthReport = healthState.kind === "ready" ? healthState.report : null
  const healthLoading = healthState.kind === "loading" || healthPending
  const copyFeedback = copyState === "copied" ? t("diagnosticsCopied") : copyState === "failed" ? t("diagnosticsCopyFailed") : null
  const reconnectFeedback = reconnectState === "pending"
    ? t("connectionReconnecting")
      : reconnectState === "done"
        ? t("connectionReconnected")
        : reconnectState === "already"
          ? t("connectionAlreadyConnected")
        : reconnectState === "unavailable"
        ? t("connectionReconnectUnavailable")
        : null

  return (
    <section className={styles.page} aria-labelledby="settings-heading" data-testid="settings-page">
      <div className={styles.pageHeading}>
        <h1 id="settings-heading">{t("settingsHeading")}</h1>
        <p className={styles.lede}>{t("settingsDescription")}</p>
      </div>

      <div className={styles.settingsGrid}>
        <section className={styles.settingsSection} aria-labelledby="settings-appearance-heading">
          <h2 id="settings-appearance-heading">{t("appearance")}</h2>
          <label className={styles.field} htmlFor="appearance-theme">
            <span>{t("theme")}</span>
            <select
              id="appearance-theme"
              name="theme"
              autoComplete="off"
              value={preferences.theme}
              onChange={(event) => {
                const theme = parseThemePreference(event.currentTarget.value)
                if (theme) preferences.setTheme(theme)
              }}
              data-testid="appearance-theme"
            >
              <option value="system">{t("systemTheme")}</option>
              <option value="light">{t("lightTheme")}</option>
              <option value="dark">{t("darkTheme")}</option>
            </select>
          </label>
          <label className={styles.field} htmlFor="appearance-density">
            <span>{t("density")}</span>
            <select
              id="appearance-density"
              name="density"
              autoComplete="off"
              value={preferences.density}
              onChange={(event) => {
                const density = parseDensityPreference(event.currentTarget.value)
                if (density) preferences.setDensity(density)
              }}
              data-testid="appearance-density"
            >
              <option value="comfortable">{t("comfortableDensity")}</option>
              <option value="compact">{t("compactDensity")}</option>
            </select>
          </label>
        </section>

        <section className={styles.settingsSection} aria-labelledby="settings-language-heading">
          <h2 id="settings-language-heading">{t("language")}</h2>
          <label className={styles.field} htmlFor="settings-locale">
            <span>{t("language")}</span>
            <select
              id="settings-locale"
              name="locale"
              autoComplete="off"
              value={preferences.locale}
              onChange={(event) => {
                const locale = parseLocalePreference(event.currentTarget.value)
                if (locale) preferences.setLocale(locale)
              }}
              data-testid="settings-locale"
            >
              <option value="zh">{t("chinese")}</option>
              <option value="en">{t("english")}</option>
            </select>
          </label>
        </section>

        <section className={styles.settingsSection} aria-labelledby="settings-identity-heading">
          <h2 id="settings-identity-heading">{t("identity")}</h2>
          <p className={styles.muted}>{t("identityDescription")}</p>
          <form
            className={styles.identityForm}
            onSubmit={(event) => {
              event.preventDefault()
              saveActor()
            }}
          >
            <label className={styles.field} htmlFor="identity-actor">
              <span>{t("actor")}</span>
              <input
                id="identity-actor"
                name="actor"
                value={actorDraft}
                maxLength={128}
                autoComplete="nickname"
                translate="no"
                aria-invalid={actorError ? "true" : "false"}
                aria-describedby={actorError ? "identity-actor-error" : "identity-actor-help"}
                onChange={(event) => {
                  setActorTouched(true)
                  setActorSaved(false)
                  setActorDraft(event.currentTarget.value)
                }}
                onBlur={saveActor}
                data-testid="identity-actor"
              />
            </label>
            <p id="identity-actor-help" className={styles.muted}>{t("identityActorHelp")}</p>
            {actorError ? <p id="identity-actor-error" className={styles.inlineError} role="alert" data-testid="identity-actor-error">{actorError}</p> : null}
            {actorSaved ? <p className={styles.inlineSuccess} role="status" data-testid="identity-actor-saved">{t("identitySaved")}</p> : null}
            <button type="submit" className={styles.secondaryAction} disabled={actorError !== null} data-testid="identity-actor-save">
              {t("save")}
            </button>
            <button type="button" className={styles.secondaryAction} onClick={resetActor} data-testid="identity-actor-reset">
              {t("identityReset")}
            </button>
          </form>
        </section>
      </div>

      <section className={styles.runtimeSection} aria-labelledby="settings-connection-heading" data-testid="settings-connection">
        <div className={styles.headingRow}>
          <div>
            <h2 id="settings-connection-heading">{t("connectionHeading")}</h2>
          </div>
          <button type="button" className={styles.secondaryAction} disabled={!boardSlug || !onReconnect || reconnectState === "pending"} onClick={reconnect} data-testid="connection-reconnect">
            {reconnectState === "pending" ? t("loading") : t("connectionReconnect")}
          </button>
        </div>
        <dl className={styles.runtimeFacts} data-testid="connection-facts">
          <div><dt>{t("apiOrigin")}</dt><dd translate="no" data-testid="connection-api-origin">{apiOriginForRuntime(runtime)}</dd></div>
          <div><dt>{t("defaultBoard")}</dt><dd translate="no" data-testid="connection-default-board">{reported(runtime.defaultBoard, t("reported"))}</dd></div>
          <div><dt>{t("server")}</dt><dd translate="no" data-testid="connection-server-version">{reported(runtime.serverVersion, t("reported"))}</dd></div>
          <div><dt>{t("protocol")}</dt><dd translate="no" data-testid="connection-protocol-version">{reported(runtime.protocolVersion, t("reported"))}</dd></div>
          <div><dt>{t("build")}</dt><dd translate="no" data-testid="connection-web-build">{reported(runtime.webBuildId, t("reported"))}</dd></div>
          <div><dt>{t("actor")}</dt><dd translate="no" data-testid="connection-actor">{reported(preferences.actor || runtime.actor, t("reported"))}</dd></div>
        </dl>
        {!boardSlug ? <p className={styles.muted} data-testid="settings-no-board">{t("noBoardDescription")}</p> : null}
        {reconnectFeedback ? <p className={styles.inlineStatus} role="status" aria-live="polite" data-testid="connection-feedback">{reconnectFeedback}</p> : null}
      </section>

      <section className={styles.runtimeSection} aria-labelledby="settings-diagnostics-heading" data-testid="settings-diagnostics">
        <div className={styles.headingRow}>
          <div>
            <h2 id="settings-diagnostics-heading">{t("diagnosticsHeading")}</h2>
          </div>
          {healthURL ? (
            <a className={styles.secondaryAction} href={healthURL} onClick={navigateHealth} data-testid="diagnostics-health-link">{t("openHealth")}</a>
          ) : (
            <button type="button" className={styles.secondaryAction} disabled data-testid="diagnostics-health-link">{t("openHealth")}</button>
          )}
        </div>
        {healthLoading && !healthReport ? <p className={styles.statusPanel} role="status" aria-live="polite" data-testid="settings-health-loading">{t("loading")}</p> : null}
        {healthError ? (
          <div className={styles.inlineError} role="alert" data-testid="settings-health-error">
            <strong>{healthError.title}</strong>
            <span>{healthError.detail}</span>
            <span>{healthError.nextStep}</span>
            <button type="button" className={styles.secondaryAction} disabled={healthPending} onClick={() => void loadHealth()} data-testid="settings-health-retry">{healthPending ? t("loading") : t("retry")}</button>
          </div>
        ) : null}
        {healthReport ? (
          <dl className={styles.runtimeFacts} data-testid="settings-health">
            <div><dt>{t("healthOk")}</dt><dd translate="no">{String(healthReport.ok)}</dd></div>
            <div><dt>{t("healthDb")}</dt><dd translate="no">{reported(healthReport.db, t("reported"))}</dd></div>
            <div><dt>{t("version")}</dt><dd translate="no">{reported(healthReport.version, t("reported"))}</dd></div>
            <div><dt>{t("dbFingerprint")}</dt><dd translate="no">{reported(healthReport.db_fingerprint, t("reported"))}</dd></div>
          </dl>
        ) : null}
        <div className={styles.diagnosticsActions}>
          <button type="button" className={styles.secondaryAction} disabled={copyPending} onClick={copyDiagnostics} data-testid="diagnostics-copy">{t("copyDiagnostics")}</button>
          {copyFeedback ? <span role="status" aria-live="polite" data-testid="diagnostics-feedback">{copyFeedback}</span> : null}
        </div>
      </section>
    </section>
  )
}

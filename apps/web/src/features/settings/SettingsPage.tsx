import { useResolvedTheme } from '../../platform/preferences/use-resolved-theme';
import './settings.css';
import { Dialog } from '../../components/ui/dialog';
import { Button } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { useWorkspaceOperations } from "../../application/workspace/use-workspace-operations";
import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react"

import type { AppNavigationTarget } from "../../application/navigation/router"
import { routePath } from "../../application/navigation/router"
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "../../domain/board-slug"
import { type HealthReport } from "../../application/data/health-read-model";
import { presentHealthError } from "../../application/health/health-error"
import { isCurrentHealthRequest } from "../../application/health/health-request"
import type { WebRuntimeConfig } from "../../lib/runtime"
import {
  parseActorPreference,
  parseDensityPreference,
  parseLocalePreference,
  parseThemePreference,
} from "../../platform/preferences/preferences"
import { createTranslator } from "../../application/i18n"
import { usePreferences } from "../../platform/preferences/use-preferences"
import { callSettingsAction } from "./settings-async-actions"
import { apiOriginForRuntime, diagnosticsText } from "../../application/diagnostics"
import type { BoardReconnectResult } from "../../application/workspace/board-session-registry"
import styles from "../../components/layout/boundary.module.css"

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

const reconnectMessage = { idle: null, pending: 'connectionReconnecting', done: 'connectionReconnected', already: 'connectionAlreadyConnected', unavailable: 'connectionReconnectUnavailable' } as const

function useSettingsPageState({
  runtime,
  boardSlug: boardSlugInput,
  initialHealth,
  read,
  onNavigate,
  onReconnect,
  clipboardWrite,
}: SettingsPageProps) {
  const { readHealth } = useWorkspaceOperations();
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
// 该语句位于 finally，身份判断防止旧请求清除新请求的 pending；最小复现见 build/react-doctor-regressions.test.ts。
// react-doctor-disable-next-line react-doctor/no-loading-flag-reset-outside-finally
        setHealthPending(false)
      }
    }
  }, [read, readHealth, runtime])

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
  const reconnectKey = reconnectMessage[reconnectState]
  const reconnectFeedback = reconnectKey ? t(reconnectKey) : null

  return {
    t,
    preferences,
    saveActor,
    actorDraft,
    actorError,
    setActorTouched,
    setActorSaved,
    setActorDraft,
    actorSaved,
    resetActor,
    boardSlug,
    onReconnect,
    reconnectState,
    reconnect,
    runtime,
    reconnectFeedback,
    healthURL,
    navigateHealth,
    healthLoading,
    healthReport,
    healthError,
    healthPending,
    loadHealth,
    copyPending,
    copyDiagnostics,
    copyFeedback
  };
}

export function SettingsPage(props: Parameters<typeof useSettingsPageState>[0]) {
  const {
    t,
    preferences,
    saveActor,
    actorDraft,
    actorError,
    setActorTouched,
    setActorSaved,
    setActorDraft,
    actorSaved,
    resetActor,
    boardSlug,
    onReconnect,
    reconnectState,
    reconnect,
    runtime,
    reconnectFeedback,
    healthURL,
    navigateHealth,
    healthLoading,
    healthReport,
    healthError,
    healthPending,
    loadHealth,
    copyPending,
    copyDiagnostics,
    copyFeedback
  } = useSettingsPageState(props);
  const resolvedTheme = useResolvedTheme();
  const close = () => { if (boardSlug) void props.onNavigate?.(routePath({kind:'board',boardSlug,view:'list'},{basePath:runtime.webBasePath})); };
  return <Dialog open title="设置" description="管理当前项目的连接、界面偏好与数据维护。" onClose={close} testId="settings-page">
    <div className="settings-stack">
      <section><div className="settings-row"><span className="settings-icon"><Icon name="database" size={20} /></span><div><strong>项目数据</strong><p>连接当前本机服务，修改由服务保存。</p></div><Button size="sm" onClick={reconnect} disabled={!onReconnect||reconnectState==='pending'}>重连</Button></div><div className="settings-actions"><Button size="sm" icon="shield" disabled={!healthURL} onClick={event=>{if(healthURL){event.preventDefault();void props.onNavigate?.(healthURL);}}}>健康检查</Button><Button size="sm" icon="settings" disabled={!boardSlug} onClick={()=>{if(boardSlug)void props.onNavigate?.(routePath({kind:'maintenance',boardSlug},{basePath:runtime.webBasePath}));}}>数据维护</Button></div>{reconnectFeedback&&<p role="status">{reconnectFeedback}</p>}</section>
      <section><div className="settings-row"><span className="settings-icon"><Icon name={resolvedTheme==='dark'?'moon':'sun'} size={20} /></span><div><strong>界面主题</strong><p>当前使用{resolvedTheme==='dark'?'深色':'浅色'}主题。</p></div><Button size="sm" onClick={()=>preferences.setTheme(resolvedTheme==='dark'?'light':'dark')}>切换</Button></div><details className="paper-detail-options"><summary>外观与语言</summary><AppearanceSettings t={t} preferences={preferences} /><label className={styles.field}>语言<select name="locale" value={preferences.locale} onChange={event=>{const locale=parseLocalePreference(event.target.value);if(locale)preferences.setLocale(locale);}} data-testid="settings-locale"><option value="zh">简体中文</option><option value="en">English</option></select></label></details></section>
      <section><IdentitySettings t={t} saveActor={saveActor} actorDraft={actorDraft} actorError={actorError} setActorTouched={setActorTouched} setActorSaved={setActorSaved} setActorDraft={setActorDraft} actorSaved={actorSaved} resetActor={resetActor} /></section>
      <details className="paper-detail-options"><summary>连接与诊断</summary><ConnectionSettings t={t} boardSlug={boardSlug} onReconnect={onReconnect} reconnectState={reconnectState} reconnect={reconnect} runtime={runtime} preferences={preferences} reconnectFeedback={reconnectFeedback} /><DiagnosticsSettings t={t} healthURL={healthURL} navigateHealth={navigateHealth} healthLoading={healthLoading} healthReport={healthReport} healthError={healthError} healthPending={healthPending} loadHealth={loadHealth} copyPending={copyPending} copyDiagnostics={copyDiagnostics} copyFeedback={copyFeedback} /></details>
      <p className="subtle-note">快捷键：Ctrl / ⌘ + K 搜索；C 新建任务；Escape 关闭对话框。</p>
    </div>
  </Dialog>;
}

function AppearanceSettings({ t, preferences }: Pick<ReturnType<typeof useSettingsPageState>, 't' | 'preferences'>) {
  return (<section className={styles.settingsSection} aria-labelledby="settings-appearance-heading">
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
        </section>);
}

function IdentitySettings({ t, saveActor, actorDraft, actorError, setActorTouched, setActorSaved, setActorDraft, actorSaved, resetActor }: Pick<ReturnType<typeof useSettingsPageState>, 't' | 'saveActor' | 'actorDraft' | 'actorError' | 'setActorTouched' | 'setActorSaved' | 'setActorDraft' | 'actorSaved' | 'resetActor'>) {
  return (<section className={styles.settingsSection} aria-labelledby="settings-identity-heading">
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
        </section>);
}

function ConnectionSettings({ t, boardSlug, onReconnect, reconnectState, reconnect, runtime, preferences, reconnectFeedback }: Pick<ReturnType<typeof useSettingsPageState>, 't' | 'boardSlug' | 'onReconnect' | 'reconnectState' | 'reconnect' | 'runtime' | 'preferences' | 'reconnectFeedback'>) {
  return (<section className={styles.runtimeSection} aria-labelledby="settings-connection-heading" data-testid="settings-connection">
        <div className={styles.headingRow}>
          <div>
            <p className={styles.eyebrow}>{t("connection")}</p>
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
          <div><dt>{t("sidebarState")}</dt><dd data-testid="connection-sidebar-state">{preferences.sidebarExpanded ? t("sidebarExpanded") : t("sidebarCollapsed")}</dd></div>
        </dl>
        {!boardSlug ? <p className={styles.muted} data-testid="settings-no-board">{t("noBoardDescription")}</p> : null}
        {reconnectFeedback ? <p className={styles.inlineStatus} role="status" aria-live="polite" data-testid="connection-feedback">{reconnectFeedback}</p> : null}
      </section>);
}

function DiagnosticsSettings({ t, healthURL, navigateHealth, healthLoading, healthReport, healthError, healthPending, loadHealth, copyPending, copyDiagnostics, copyFeedback }: Pick<ReturnType<typeof useSettingsPageState>, 't' | 'healthURL' | 'navigateHealth' | 'healthLoading' | 'healthReport' | 'healthError' | 'healthPending' | 'loadHealth' | 'copyPending' | 'copyDiagnostics' | 'copyFeedback'>) {
  return (<section className={styles.runtimeSection} aria-labelledby="settings-diagnostics-heading" data-testid="settings-diagnostics">
        <div className={styles.headingRow}>
          <div>
            <p className={styles.eyebrow}>{t("diagnostics")}</p>
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
      </section>);
}

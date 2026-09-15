import './settings.css';
import { SettingsView, type SettingsTab, type SettingsViewProps } from './settings-view';
import { useWorkspaceOperations } from "../../application/workspace/use-workspace-operations";
import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react"
import type { AppNavigationTarget } from "../../application/navigation/router"
import { routePath } from "../../application/navigation/router"
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "../../domain/board-slug"
import { type HealthReport } from "../../application/data/health-read-model";
import { presentHealthError } from "../../application/health/health-error"
import { isCurrentHealthRequest } from "../../application/health/health-request"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { parseActorPreference, parseDensityPreference, parseLocalePreference, parseThemePreference } from "../../platform/preferences/preferences"
import { createTranslator } from "../../application/i18n"
import { usePreferences } from "../../platform/preferences/use-preferences"
import { callSettingsAction } from "./settings-async-actions"
import { apiOriginForRuntime, diagnosticsText } from "../../application/diagnostics"
import type { BoardReconnectResult } from "../../application/workspace/board-session-registry"

export type SettingsPageProps = {
  readonly initialTab?: SettingsTab
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

function useSettingsPageState({ runtime, boardSlug: boardSlugInput, initialHealth, read, onNavigate, onReconnect, clipboardWrite }: SettingsPageProps) {
  const { readHealth } = useWorkspaceOperations();
  const preferences = usePreferences()
  const t = createTranslator(preferences.locale)
  const boardSlug = useMemo(() => canonicalBoard(boardSlugInput), [boardSlugInput])
  const healthURL = boardSlug ? routePath({ kind: "health", boardSlug }, { basePath: runtime.webBasePath }) : null
  const [healthState, setHealthState] = useState<HealthState>(() => initialHealth ? { kind: "ready", report: initialHealth } : { kind: "loading" })
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
    return () => { mountedRef.current = false; healthControllerRef.current?.abort(); healthControllerRef.current = null }
  }, [])
  useEffect(() => { if (!actorTouched) setActorDraft(preferences.actor || runtime.actor) }, [actorTouched, preferences.actor, runtime.actor])
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
      setHealthState((previous) => previous.kind === "ready" ? { kind: "ready", report: previous.report, staleError: error } : { kind: "error", error })
    } finally {
      if (healthControllerRef.current === controller) {
        healthControllerRef.current = null
        // react-doctor-disable-next-line react-doctor/no-loading-flag-reset-outside-finally
        setHealthPending(false)
      }
    }
  }, [read, readHealth, runtime])
  useEffect(() => { if (!initialHealth) void loadHealth() }, [initialHealth, loadHealth])
  const actorError = actorTouched && parseActorPreference(actorDraft) === null ? t("identityActorInvalid") : null
  const saveActor = () => {
    setActorTouched(true)
    const actor = parseActorPreference(actorDraft)
    if (actor === null) { setActorSaved(false); return }
    preferences.setActor(actor); setActorDraft(actor); setActorSaved(true)
  }
  const resetActor = () => {
    preferences.setActor(""); setActorDraft(runtime.actor); setActorTouched(false); setActorSaved(true)
  }
  const copyDiagnostics = useCallback(() => {
    if (copyPendingRef.current) return
    copyPendingRef.current = true; setCopyPending(true); setCopyState("idle")
    const health = healthState.kind === "ready" ? healthState.report : null
    const writer = clipboardWrite ?? browserClipboardWrite
    void callSettingsAction(() => writer(diagnosticsText(runtime, health)))
      .then(() => { if (mountedRef.current) setCopyState("copied") })
      .catch(() => { if (mountedRef.current) setCopyState("failed") })
      .finally(() => { copyPendingRef.current = false; if (mountedRef.current) setCopyPending(false) })
  }, [clipboardWrite, healthState, runtime])
  const reconnect = useCallback(() => {
    if (!boardSlug || !onReconnect || reconnectPendingRef.current) return
    reconnectPendingRef.current = true; setReconnectState("pending")
    void callSettingsAction(() => onReconnect())
      .then((reconnected) => {
        if (!mountedRef.current) return
        setReconnectState(reconnected === false || reconnected === "unavailable" ? "unavailable" : reconnected === "already-live" ? "already" : "done")
      })
      .catch(() => { if (mountedRef.current) setReconnectState("unavailable") })
      .finally(() => { reconnectPendingRef.current = false })
  }, [boardSlug, onReconnect])
  const navigateHealth = useCallback((event: MouseEvent<HTMLAnchorElement>) => {
    if (!healthURL || !onNavigate) return
    event.preventDefault(); void Promise.resolve(onNavigate(healthURL)).catch(() => undefined)
  }, [healthURL, onNavigate])
  const healthError = healthState.kind === "error" || healthState.kind === "ready" && healthState.staleError ? presentHealthError(healthState.kind === "error" ? healthState.error : healthState.staleError, t) : null
  const healthReport = healthState.kind === "ready" ? healthState.report : null
  const healthLoading = healthState.kind === "loading" || healthPending
  const copyFeedback = copyState === "copied" ? t("diagnosticsCopied") : copyState === "failed" ? t("diagnosticsCopyFailed") : null
  const reconnectKey = reconnectMessage[reconnectState]
  const reconnectFeedback = reconnectKey ? t(reconnectKey) : null
  return { t, preferences, saveActor, actorDraft, actorError, setActorTouched, setActorSaved, setActorDraft, actorSaved, resetActor, boardSlug, onReconnect, reconnectState, reconnect, runtime, reconnectFeedback, healthURL, navigateHealth, healthLoading, healthReport, healthError, healthPending, loadHealth, copyPending, copyDiagnostics, copyFeedback };
}

export function SettingsPage(props: SettingsPageProps) {
  const s=useSettingsPageState(props), p=s.preferences;
  const health=settingsConnection(s);
  return <SettingsView initialTab={props.initialTab} hasProject={Boolean(s.boardSlug)} theme={p.theme} density={p.density} locale={p.locale} sidebarExpanded={p.sidebarExpanded}
    onThemeChange={value=>{const theme=parseThemePreference(value);if(theme)p.setTheme(theme);}}
    onDensityChange={value=>{const density=parseDensityPreference(value);if(density)p.setDensity(density);}}
    onLocaleChange={value=>{const locale=parseLocalePreference(value);if(locale)p.setLocale(locale);}}
    onSidebarChange={p.setSidebarExpanded}
    actorDraft={s.actorDraft} actorError={s.actorError} actorSaved={s.actorSaved}
    onActorChange={value=>{s.setActorTouched(true);s.setActorSaved(false);s.setActorDraft(value);}}
    onActorSave={s.saveActor} onActorReset={s.resetActor}
    connection={health.connection} connectionMessage={health.connectionMessage} reconnectPending={s.reconnectState==='pending'}
    healthErrorNextStep={s.healthError?.nextStep??null} healthPending={s.healthPending} onHealthRetry={()=>{void s.loadHealth();}}
    reconnectEnabled={Boolean(s.boardSlug&&s.onReconnect)} reconnectFeedback={s.reconnectFeedback} onReconnect={s.reconnect}
    onHealth={s.healthURL&&props.onNavigate?()=>{void props.onNavigate?.(s.healthURL!);}:undefined}
    onMaintenance={s.boardSlug&&props.onNavigate?()=>{void props.onNavigate?.(routePath({kind:'maintenance',boardSlug:s.boardSlug!},{basePath:props.runtime.webBasePath}));}:undefined}
    onCopyDiagnostics={s.copyDiagnostics} copyPending={s.copyPending} copyFeedback={s.copyFeedback}
    diagnosticFacts={settingsDiagnosticFacts(s)}
  />;
}

function settingsConnection(s: ReturnType<typeof useSettingsPageState>): Pick<SettingsViewProps, 'connection' | 'connectionMessage'> {
  const isEnglish=s.preferences.locale==='en';
  const connection=s.healthLoading?'loading':s.healthError?(s.healthReport?'warning':'error'):s.healthReport?.ok?'available':'warning';
  const connectionMessage=s.healthError?.detail??(s.healthReport?.ok?(isEnglish?'The local service responded to its health check.':'本机服务已响应健康检查。'):(isEnglish?'Inspect system status for details.':'打开系统状态查看检查结果。'));
  return { connection, connectionMessage };
}

function settingsDiagnosticFacts(s: ReturnType<typeof useSettingsPageState>) {
  const isEnglish=s.preferences.locale==='en';
  return [
      {label:isEnglish?'Project':'当前项目',value:s.boardSlug??'—'},
      {label:'API',value:apiOriginForRuntime(s.runtime)},
      {label:isEnglish?'Server':'服务版本',value:reported(s.runtime.serverVersion,'—')},
      {label:isEnglish?'Protocol':'协议版本',value:reported(s.runtime.protocolVersion,'—')},
      {label:isEnglish?'Web build':'页面构建',value:reported(s.runtime.webBuildId,'—')},
      {label:isEnglish?'Database':'数据库',value:reported(s.healthReport?.db,'—')},
      {label:isEnglish?'Database path':'数据库路径',value:reported(s.healthReport?.db_path,'—')},
      {label:isEnglish?'Fingerprint':'数据库指纹',value:reported(s.healthReport?.db_fingerprint,'—')},
    ];
}

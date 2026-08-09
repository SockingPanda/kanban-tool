import { useCallback, useEffect, useRef, useState } from "react"

import type { WebRuntimeConfig } from "../../lib/runtime"
import { readHealth, type HealthReadError, type HealthReport } from "../../lib/api/health-read-model"
import { createTranslator } from "../../lib/i18n"
import { usePreferences } from "../../lib/use-preferences"
import { presentHealthError } from "./health-error"
import { healthMetricTone } from "./health-metrics"
import { isCurrentHealthRequest } from "./health-request"
import styles from "./health-page.module.css"

export type HealthPageProps = {
  runtime: WebRuntimeConfig
  initialReport?: HealthReport
  read?: (signal?: AbortSignal) => Promise<HealthReport>
}

type HealthState =
  | { kind: "loading" }
  | { kind: "ready"; report: HealthReport; staleError?: unknown }
  | { kind: "error"; error: unknown }

function reported(value: string | null | undefined, fallback: string): string {
  const trimmed = value?.trim()
  return trimmed || fallback
}

export function HealthPage({ runtime, initialReport, read }: HealthPageProps) {
  const { locale } = usePreferences()
  const t = createTranslator(locale)
  const [state, setState] = useState<HealthState>(() => initialReport ? { kind: "ready", report: initialReport } : { kind: "loading" })
  const [pending, setPending] = useState(!initialReport)
  const requestControllerRef = useRef<AbortController | null>(null)

  const load = useCallback(async () => {
    if (requestControllerRef.current) return
    const reader = read ?? ((nextSignal?: AbortSignal) => readHealth({ runtime, signal: nextSignal }))
    const controller = new AbortController()
    requestControllerRef.current = controller
    setPending(true)
    try {
      const report = await reader(controller.signal)
      if (!isCurrentHealthRequest(controller, requestControllerRef.current)) return
      setState({ kind: "ready", report })
    } catch (error: unknown) {
      if (!isCurrentHealthRequest(controller, requestControllerRef.current) || (error instanceof Error && error.name === "AbortError")) return
      setState((previous) => previous.kind === "ready"
        ? { kind: "ready", report: previous.report, staleError: error }
        : { kind: "error", error })
    } finally {
      if (requestControllerRef.current === controller) {
        requestControllerRef.current = null
        setPending(false)
      }
    }
  }, [read, runtime])

  useEffect(() => {
    if (!initialReport) void load()
    return () => {
      const controller = requestControllerRef.current
      controller?.abort()
      if (requestControllerRef.current === controller) requestControllerRef.current = null
    }
  }, [initialReport, load])

  const refresh = () => {
    if (pending) return
    void load()
  }

  return (
    <section className={styles.page} aria-labelledby="health-heading" data-testid="health-page">
      <div className={styles.headingRow}>
        <div className={styles.pageHeading}>
          <p className={styles.eyebrow}>{t("productKicker")}</p>
          <h1 id="health-heading">{t("healthHeading")}</h1>
          <p className={styles.lede}>{t("healthDescription")}</p>
        </div>
        <button type="button" className={styles.refresh} disabled={pending} onClick={refresh} data-testid="health-refresh">
          {pending ? t("loading") : t("refresh")}
        </button>
      </div>

      {state.kind === "loading" ? (
        <div className={styles.boundary} role="status" aria-live="polite" data-testid="health-loading">{t("loading")}</div>
      ) : null}

      {state.kind === "error" ? (
        <div className={styles.error} role="alert" data-testid="health-error">
          <HealthErrorContent error={state.error} t={t} />
          <button type="button" className={styles.retry} disabled={pending} onClick={refresh} data-testid="health-error-retry">
            {pending ? t("loading") : t("retry")}
          </button>
        </div>
      ) : null}

      {state.kind === "ready" ? (
        <>
          <HealthMetrics report={state.report} t={t} />
          {state.staleError ? (
            <div className={styles.stale} role="alert" data-testid="health-stale">
              <HealthErrorContent error={state.staleError} t={t} />
              <p>{t("healthStale")}</p>
              <button type="button" className={styles.retry} disabled={pending} onClick={refresh}>{t("retry")}</button>
            </div>
          ) : null}
        </>
      ) : null}

      <section className={styles.runtime} aria-labelledby="health-runtime-heading" data-testid="health-runtime">
        <div>
          <p className={styles.eyebrow}>{t("runtime")}</p>
          <h2 id="health-runtime-heading">{t("runtimeIdentity")}</h2>
        </div>
        <dl className={styles.runtimeGrid}>
          <RuntimeFact label={t("defaultBoard")} value={runtime.defaultBoard} fallback={t("reported")} />
          <RuntimeFact label={t("actor")} value={runtime.actor} fallback={t("reported")} />
          <RuntimeFact label={t("api")} value={runtime.apiBaseUrl || "/"} fallback={t("reported")} />
          <RuntimeFact label={t("server")} value={runtime.serverVersion} fallback={t("reported")} />
          <RuntimeFact label={t("protocol")} value={runtime.protocolVersion} fallback={t("reported")} />
          <RuntimeFact label={t("build")} value={runtime.webBuildId} fallback={t("reported")} />
        </dl>
      </section>
    </section>
  )
}

function HealthMetrics({ report, t }: { report: HealthReport; t: ReturnType<typeof createTranslator> }) {
  const metrics = [
    { id: "ok", label: t("healthOk"), value: String(report.ok), tone: styles[healthMetricTone(report.ok)] },
    { id: "db", label: t("healthDb"), value: reported(report.db, t("reported")), tone: styles[healthMetricTone(report.ok)] },
    { id: "version", label: t("version"), value: reported(report.version, t("reported")), tone: styles.neutral },
    { id: "db-path", label: t("dbPath"), value: reported(report.db_path, t("reported")), tone: styles.neutral },
    { id: "db-fingerprint", label: t("dbFingerprint"), value: reported(report.db_fingerprint, t("reported")), tone: styles.neutral },
  ] as const

  return (
    <dl className={styles.metrics} aria-label={t("healthMetrics")} data-testid="health-metrics">
      {metrics.map((metric) => (
        <div className={styles.metric} key={metric.id} data-testid={`health-metric-${metric.id}`}>
          <dt>{metric.label}</dt>
          <dd className={metric.tone} translate="no">{metric.value}</dd>
        </div>
      ))}
    </dl>
  )
}

function RuntimeFact({ label, value, fallback }: { label: string; value: string; fallback: string }) {
  return (
    <div className={styles.runtimeFact}>
      <dt>{label}</dt>
      <dd translate="no">{reported(value, fallback)}</dd>
    </div>
  )
}

function HealthErrorContent({ error, t }: { error: unknown; t: ReturnType<typeof createTranslator> }) {
  const copy = presentHealthError(error, t)
  return (
    <>
      <strong>{copy.title}</strong>
      <span data-testid="health-error-detail">{copy.detail}</span>
      <span data-testid="health-error-next-step">{copy.nextStep}</span>
    </>
  )
}

export type HealthError = HealthReadError

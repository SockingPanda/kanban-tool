import { useCallback, useEffect, useState } from "react"

import type { WebRuntimeConfig } from "../../lib/runtime"
import { readHealth, type HealthReadError, type HealthReport } from "../../lib/api/health-read-model"
import { createTranslator } from "../../lib/i18n"
import { usePreferences } from "../../lib/use-preferences"
import styles from "./health-page.module.css"

export type HealthPageProps = {
  runtime: WebRuntimeConfig
  initialReport?: HealthReport
  read?: (signal?: AbortSignal) => Promise<HealthReport>
}

type HealthState =
  | { kind: "loading" }
  | { kind: "ready"; report: HealthReport }
  | { kind: "error"; error: unknown }

const fallbackText = "—"

function reported(value: string | null | undefined): string {
  const trimmed = value?.trim()
  return trimmed || fallbackText
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  return String(error)
}

function metricTone(value: boolean | string): string {
  if (value === true || value === "ok") return styles.ready
  return styles.degraded
}

export function HealthPage({ runtime, initialReport, read }: HealthPageProps) {
  const { locale } = usePreferences()
  const t = createTranslator(locale)
  const [state, setState] = useState<HealthState>(() => initialReport ? { kind: "ready", report: initialReport } : { kind: "loading" })
  const [refreshing, setRefreshing] = useState(false)

  const load = useCallback(async (signal?: AbortSignal) => {
    const reader = read ?? ((nextSignal?: AbortSignal) => readHealth({ runtime, signal: nextSignal }))
    const report = await reader(signal)
    setState({ kind: "ready", report })
  }, [read, runtime])

  useEffect(() => {
    if (initialReport) return
    const controller = new AbortController()
    void load(controller.signal).catch((error: unknown) => {
      if (controller.signal.aborted || (error instanceof Error && error.name === "AbortError")) return
      setState({ kind: "error", error })
    })
    return () => controller.abort()
  }, [initialReport, load])

  const refresh = () => {
    if (refreshing) return
    const controller = new AbortController()
    setRefreshing(true)
    void load(controller.signal)
      .catch((error: unknown) => {
        if (controller.signal.aborted || (error instanceof Error && error.name === "AbortError")) return
        setState({ kind: "error", error })
      })
      .finally(() => setRefreshing(false))
  }

  return (
    <section className={styles.page} aria-labelledby="health-heading" data-testid="health-page">
      <div className={styles.headingRow}>
        <div className={styles.pageHeading}>
          <p className={styles.eyebrow}>{t("productKicker")}</p>
          <h1 id="health-heading">{t("healthHeading")}</h1>
          <p className={styles.lede}>{t("healthDescription")}</p>
        </div>
        <button type="button" className={styles.refresh} disabled={refreshing} onClick={refresh} data-testid="health-refresh">
          {refreshing ? t("loading") : t("refresh")}
        </button>
      </div>

      {state.kind === "loading" ? (
        <div className={styles.boundary} role="status" aria-live="polite" data-testid="health-loading">{t("loading")}</div>
      ) : null}

      {state.kind === "error" ? (
        <div className={styles.error} role="alert" data-testid="health-error">
          <strong>{t("healthUnavailable")}</strong>
          <span>{errorMessage(state.error)}</span>
          <button type="button" className={styles.retry} onClick={refresh}>{t("retry")}</button>
        </div>
      ) : null}

      {state.kind === "ready" ? (
        <HealthMetrics report={state.report} t={t} />
      ) : null}

      <section className={styles.runtime} aria-labelledby="health-runtime-heading" data-testid="health-runtime">
        <div>
          <p className={styles.eyebrow}>{t("runtime")}</p>
          <h2 id="health-runtime-heading">{t("runtimeIdentity")}</h2>
        </div>
        <dl className={styles.runtimeGrid}>
          <RuntimeFact label={t("defaultBoard")} value={runtime.defaultBoard} />
          <RuntimeFact label={t("actor")} value={runtime.actor} />
          <RuntimeFact label={t("api")} value={runtime.apiBaseUrl || "/"} />
          <RuntimeFact label={t("server")} value={runtime.serverVersion} />
          <RuntimeFact label={t("protocol")} value={runtime.protocolVersion} />
          <RuntimeFact label={t("build")} value={runtime.webBuildId} />
        </dl>
      </section>
    </section>
  )
}

function HealthMetrics({ report, t }: { report: HealthReport; t: ReturnType<typeof createTranslator> }) {
  const metrics = [
    { id: "ok", label: t("healthOk"), value: String(report.ok), tone: metricTone(report.ok) },
    { id: "db", label: t("healthDb"), value: report.db, tone: metricTone(report.db) },
    { id: "version", label: t("version"), value: report.version, tone: styles.neutral },
    { id: "db-path", label: t("dbPath"), value: reported(report.db_path), tone: styles.neutral },
    { id: "db-fingerprint", label: t("dbFingerprint"), value: reported(report.db_fingerprint), tone: styles.neutral },
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

function RuntimeFact({ label, value }: { label: string; value: string }) {
  return (
    <div className={styles.runtimeFact}>
      <dt>{label}</dt>
      <dd translate="no">{reported(value)}</dd>
    </div>
  )
}

export type HealthError = HealthReadError

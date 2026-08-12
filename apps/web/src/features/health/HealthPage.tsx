import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"
import { StatusDot } from "@astryxdesign/core/StatusDot"
import { useCallback, useEffect, useRef, useState } from "react"

import type { WebRuntimeConfig } from "../../lib/runtime"
import { readHealth, type HealthReadError, type HealthReport } from "../../lib/api/health-read-model"
import { createTranslator } from "../../lib/i18n"
import { HEALTH_REFRESH_EVENT } from "../../lib/health-refresh"
import { usePreferences } from "../../lib/use-preferences"
import { PageFrame } from "../../ui/astryx/page-frame"
import {
  SafeHStack,
  SafeMetadataList,
  SafeMetadataListItem,
  SafeVStack,
} from "../../ui/astryx/primitives"
import { presentHealthError } from "./health-error"
import { isCurrentHealthRequest } from "./health-request"
import { apiOriginForRuntime } from "../settings/settings-diagnostics"

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

  const refresh = useCallback(() => {
    if (requestControllerRef.current) return
    void load()
  }, [load])

  useEffect(() => {
    window.addEventListener(HEALTH_REFRESH_EVENT, refresh)
    return () => window.removeEventListener(HEALTH_REFRESH_EVENT, refresh)
  }, [refresh])

  return (
    <PageFrame
      frame="content"
      aria-labelledby="health-heading"
      bodyLabelledBy="health-heading"
      data-testid="health-page"
      header={(
        <SafeHStack as="header" gap={4} align="start" justify="between" wrap="wrap" aria-busy={pending || undefined}>
          <SafeVStack gap={1.5} className="min-w-0">
            <Heading level={1} id="health-heading">{t("healthHeading")}</Heading>
            <Text as="p" type="body" color="secondary" textWrap="pretty">{t("healthDescription")}</Text>
          </SafeVStack>
          <Button
            type="button"
            label={pending ? t("loading") : t("refresh")}
            variant="secondary"
            isDisabled={pending}
            onClick={refresh}
            data-testid="health-refresh"
          />
        </SafeHStack>
      )}
    >
      <SafeVStack as="section" gap={6} aria-labelledby="health-heading">
        {state.kind === "loading" ? (
          <Banner status="info" title={t("loading")} container="section" data-testid="health-loading" />
        ) : null}

        {state.kind === "error" ? (
          <HealthErrorBanner error={state.error} t={t} pending={pending} onRetry={refresh} testId="health-error" retryTestId="health-error-retry" />
        ) : null}

        {state.kind === "ready" ? (
          <>
            <HealthMetrics report={state.report} t={t} />
            {state.staleError ? (
              <HealthErrorBanner error={state.staleError} t={t} pending={pending} onRetry={refresh} stale testId="health-stale" />
            ) : null}
          </>
        ) : null}

        <SafeVStack as="section" gap={3} aria-labelledby="health-runtime-heading" data-testid="health-runtime">
          <Heading level={2} id="health-runtime-heading">{t("runtimeIdentity")}</Heading>
          <SafeMetadataList columns="multi">
            <RuntimeFact label={t("defaultBoard")} value={runtime.defaultBoard} fallback={t("reported")} />
            <RuntimeFact label={t("actor")} value={runtime.actor} fallback={t("reported")} />
            <RuntimeFact label={t("api")} value={apiOriginForRuntime(runtime)} fallback={t("reported")} />
            <RuntimeFact label={t("server")} value={runtime.serverVersion} fallback={t("reported")} />
            <RuntimeFact label={t("protocol")} value={runtime.protocolVersion} fallback={t("reported")} />
            <RuntimeFact label={t("build")} value={runtime.webBuildId} fallback={t("reported")} />
          </SafeMetadataList>
        </SafeVStack>
      </SafeVStack>
    </PageFrame>
  )
}

function HealthMetrics({ report, t }: { report: HealthReport; t: ReturnType<typeof createTranslator> }) {
  const statusVariant = report.ok ? "success" : "error"
  const metrics = [
    { id: "ok", label: t("healthOk"), value: String(report.ok), status: statusVariant },
    { id: "db", label: t("healthDb"), value: reported(report.db, t("reported")), status: statusVariant },
    { id: "version", label: t("version"), value: reported(report.version, t("reported")), status: undefined },
    { id: "db-path", label: t("dbPath"), value: reported(report.db_path, t("reported")), status: undefined },
    { id: "db-fingerprint", label: t("dbFingerprint"), value: reported(report.db_fingerprint, t("reported")), status: undefined },
  ] as const

  return (
    <SafeVStack as="section" gap={2} aria-label={t("healthMetrics")} data-testid="health-metrics">
      <SafeMetadataList columns="multi">
        {metrics.map((metric) => (
          <SafeMetadataListItem key={metric.id} label={metric.label} data-testid={`health-metric-${metric.id}`}>
            <SafeHStack gap={1} align="center" wrap="wrap">
              {metric.status ? <StatusDot variant={metric.status} label={`${metric.label}: ${metric.value}`} /> : null}
              <Text type="code" wordBreak="break-word"><span translate="no">{metric.value}</span></Text>
            </SafeHStack>
          </SafeMetadataListItem>
        ))}
      </SafeMetadataList>
    </SafeVStack>
  )
}

function RuntimeFact({ label, value, fallback }: { label: string; value: string; fallback: string }) {
  return (
    <SafeMetadataListItem label={label}>
      <Text type="code" wordBreak="break-word"><span translate="no">{reported(value, fallback)}</span></Text>
    </SafeMetadataListItem>
  )
}

function HealthErrorDescription({ error, t, stale }: { error: unknown; t: ReturnType<typeof createTranslator>; stale?: boolean }) {
  const copy = presentHealthError(error, t)
  return (
    <SafeVStack gap={0.5}>
      <Text as="p" type="body" color="inherit" data-testid="health-error-detail">{copy.detail}</Text>
      <Text as="p" type="body" color="inherit" data-testid="health-error-next-step">{copy.nextStep}</Text>
      {stale ? <Text as="p" type="body" color="inherit">{t("healthStale")}</Text> : null}
    </SafeVStack>
  )
}

function HealthErrorBanner({
  error,
  t,
  pending,
  onRetry,
  stale = false,
  testId,
  retryTestId,
}: {
  error: unknown
  t: ReturnType<typeof createTranslator>
  pending: boolean
  onRetry: () => void
  stale?: boolean
  testId: string
  retryTestId?: string
}) {
  const copy = presentHealthError(error, t)
  return (
    <Banner
      status={stale ? "warning" : "error"}
      title={copy.title}
      description={<HealthErrorDescription error={error} t={t} stale={stale} />}
      container="section"
      data-testid={testId}
      endContent={(
        <SafeHStack gap={1} aria-busy={pending || undefined}>
          <Button
            type="button"
            label={pending ? t("loading") : t("retry")}
            variant="ghost"
            isDisabled={pending}
            onClick={onRetry}
            data-testid={retryTestId}
          />
        </SafeHStack>
      )}
    />
  )
}

export type HealthError = HealthReadError

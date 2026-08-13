import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { Text, type TextProps } from "@astryxdesign/core/Text"
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react"

import type { WebRuntimeConfig } from "../../lib/runtime"
import type { Locale } from "../../lib/preferences"
import { createTranslator } from "../../lib/i18n"
import { requestHealthRefresh } from "../../lib/health-refresh"
import { usePreferences } from "../../lib/use-preferences"
import { CheckboxInput, TextInput } from "../../ui/astryx/fields"
import { Dialog } from "@/ui/astryx/overlays"
import { PageFrame } from "../../ui/astryx/page-frame"
import {
  Grid,
  SafeCard,
  SafeHStack,
  SafeSection,
  SafeStack,
  SafeVStack,
} from "../../ui/astryx/primitives"
import {
  createMaintenanceApi,
  MaintenanceApiError,
  type BackupReport,
  type CheckpointReport,
  type DoctorReport,
  type ExportReport,
  type ImportReport,
  type MaintenanceApi,
  type MaintenanceRunReport,
  type MaintenanceStatus,
  type QueueStats,
  type SearchStatus,
  type VacuumReport,
} from "../../lib/api/maintenance-api"
import { maintenanceOwnerForAction } from "./maintenance-intents"

type LoadState<T> =
  | { kind: "loading" }
  | { kind: "ready"; value: T }
  | { kind: "error"; error: unknown }

type ResultKey = "backup" | "export" | "import" | "checkpoint" | "vacuum" | "run" | "rebuild" | "cleanup"
type Result = BackupReport | ExportReport | ImportReport | CheckpointReport | VacuumReport | MaintenanceRunReport

type QueryRequest = {
  readonly generation: number
  readonly controller: AbortController
  readonly promise: Promise<boolean>
}

type QueryLoadOptions = {
  readonly generation: number
  readonly fresh?: boolean
  readonly silent?: boolean
}

type DiagnosticTarget = "all" | "stats" | "search"

type SyncNotice = "stale" | "mutation" | null

export type MaintenanceInitialState = {
  readonly status?: MaintenanceStatus
  readonly stats?: QueueStats
  readonly searchStatus?: SearchStatus
  readonly doctor?: DoctorReport
  readonly results?: Partial<Record<ResultKey, Result>>
}

export type MaintenancePageProps = {
  readonly runtime: WebRuntimeConfig
  readonly boardSlug: string
  readonly api?: MaintenanceApi
  readonly initial?: MaintenanceInitialState
  /** Integration seam used to refresh the health query after a successful host mutation. */
  readonly onHealthRefresh?: () => void
}

type ConfirmAction =
  | { kind: "checkpoint" }
  | { kind: "backup"; path: string }
  | { kind: "export"; path: string }
  | { kind: "import"; path: string; replace: boolean }
  | { kind: "run"; owner: string }
  | { kind: "rebuild"; owner: string }
  | { kind: "cleanup"; owner: string }
  | { kind: "vacuum" }

const emptyState = <T,>(): LoadState<T> => ({ kind: "loading" })

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function errorCode(error: unknown): string {
  if (error instanceof MaintenanceApiError) return error.code ?? error.kind
  if (error && typeof error === "object") {
    const kind = (error as { readonly kind?: unknown }).kind
    if (typeof kind === "string") return kind
  }
  return "unknown"
}

function safeErrorText(error: unknown, t: ReturnType<typeof createTranslator>): string {
  const messages: Record<string, string> = {
    offline: t("maintenanceErrorOffline"),
    server_unavailable: t("maintenanceErrorOffline"),
    http: t("maintenanceErrorHttp"),
    not_found: t("maintenanceErrorNotFound"),
    conflict: t("maintenanceErrorConflict"),
    idempotency_conflict: t("maintenanceErrorConflict"),
    invalid_input: t("maintenanceErrorInvalidInput"),
    feature_not_available: t("maintenanceErrorFeatureNotAvailable"),
    invalid_json: t("maintenanceErrorInvalidJson"),
    invalid_contract: t("maintenanceErrorInvalidContract"),
    cross_origin: t("maintenanceErrorCrossOrigin"),
    malformed_url: t("maintenanceErrorMalformedUrl"),
    invalid_headers: t("maintenanceErrorInvalidHeaders"),
    invalid_content_type: t("maintenanceErrorContentType"),
    invalid_bytes: t("maintenanceErrorInvalidBytes"),
    response_too_large: t("maintenanceErrorTooLarge"),
  }
  return messages[errorCode(error)] ?? t("maintenanceErrorUnknown")
}

function reported(value: string | number | boolean | null | undefined): string {
  if (value === null || value === undefined) return "—"
  if (typeof value === "string") return value.trim() || "—"
  return String(value)
}

function errorSummary(count: number, t: ReturnType<typeof createTranslator>): string {
  return count > 0 ? `${t("errorPresent")} (${count})` : t("none")
}

function diagnosticSummary(value: string | null | undefined, t: ReturnType<typeof createTranslator>): string {
  return value?.trim() ? t("serverMessagePresent") : t("none")
}

function formatTimestamp(value: number | null | undefined, locale: Locale): string {
  if (value === null || value === undefined) return "—"
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return "—"
  return new Intl.DateTimeFormat(locale === "zh" ? "zh-CN" : "en-US", { dateStyle: "medium", timeStyle: "short" }).format(date)
}

function statusTone(status: { dirty: boolean; degraded: boolean; failed: number; last_error: string | null; lifecycle_status: string }): "error" | "success" {
  return status.degraded || status.dirty || status.failed > 0 || Boolean(status.last_error) || /degraded|error|failed/i.test(status.lifecycle_status)
    ? "error"
    : "success"
}

export function MaintenancePage({ runtime, boardSlug, api: providedApi, initial, onHealthRefresh }: MaintenancePageProps) {
  const { locale, actor } = usePreferences()
  const t = createTranslator(locale)
  const api = useMemo(() => providedApi ?? createMaintenanceApi({}, runtime), [providedApi, runtime])
  const [status, setStatus] = useState<LoadState<MaintenanceStatus>>(() => initial?.status ? { kind: "ready", value: initial.status } : emptyState())
  const [stats, setStats] = useState<LoadState<QueueStats>>(() => initial?.stats ? { kind: "ready", value: initial.stats } : emptyState())
  const [searchStatus, setSearchStatus] = useState<LoadState<SearchStatus>>(() => initial?.searchStatus ? { kind: "ready", value: initial.searchStatus } : emptyState())
  const [doctor, setDoctor] = useState<LoadState<DoctorReport>>(() => initial?.doctor ? { kind: "ready", value: initial.doctor } : { kind: "loading" })
  const [results, setResults] = useState<Partial<Record<ResultKey, Result>>>(() => initial?.results ?? {})
  const [pendingAction, setPendingAction] = useState<ResultKey | "doctor" | null>(null)
  const [actionError, setActionError] = useState<{ action: ResultKey | "doctor"; error: unknown } | null>(null)
  const [syncNotice, setSyncNotice] = useState<SyncNotice>(null)
  const [confirm, setConfirm] = useState<ConfirmAction | null>(null)
  const confirmOpenerRef = useRef<HTMLElement | null>(null)
  const confirmCancelRef = useRef<HTMLButtonElement | null>(null)
  const statusRequestRef = useRef<QueryRequest | null>(null)
  const statsRequestRef = useRef<QueryRequest | null>(null)
  const searchRequestRef = useRef<QueryRequest | null>(null)
  const doctorRequestRef = useRef<QueryRequest | null>(null)
  const mountedRef = useRef(false)
  const firstMountRef = useRef(true)
  const generationRef = useRef(0)
  const mutationTokenRef = useRef(0)
  const pendingActionRef = useRef<ResultKey | "doctor" | null>(null)

  const abortRequests = useCallback(() => {
    const requests = [statusRequestRef.current, statsRequestRef.current, searchRequestRef.current, doctorRequestRef.current]
    statusRequestRef.current = null
    statsRequestRef.current = null
    searchRequestRef.current = null
    doctorRequestRef.current = null
    requests.forEach((request) => request?.controller.abort())
  }, [])

  const isCurrent = useCallback((generation: number) => mountedRef.current && generationRef.current === generation, [])

  const loadStatus = useCallback((options: QueryLoadOptions): Promise<boolean> => {
    const current = statusRequestRef.current
    if (!options.fresh && current?.generation === options.generation) return current.promise
    current?.controller.abort()
    const controller = new AbortController()
    const promise = (async () => {
      if (!options.silent && isCurrent(options.generation)) setStatus((state) => state.kind === "ready" ? state : { kind: "loading" })
      try {
        const value = await api.status(controller.signal)
        if (isCurrent(options.generation)) {
          setStatus({ kind: "ready", value })
          setSyncNotice((notice) => notice === "stale" ? null : notice)
          return true
        }
        return false
      } catch (error) {
        if (controller.signal.aborted || isAbortError(error) || !isCurrent(options.generation)) return false
        setStatus((state) => options.silent && state.kind === "ready" ? state : { kind: "error", error })
        if (options.silent) setSyncNotice((notice) => notice === "mutation" ? notice : "stale")
        return false
      } finally {
        if (statusRequestRef.current?.controller === controller) statusRequestRef.current = null
      }
    })()
    statusRequestRef.current = { generation: options.generation, controller, promise }
    return promise
  }, [api, isCurrent])

  const loadStats = useCallback((options: QueryLoadOptions): Promise<boolean> => {
    const current = statsRequestRef.current
    if (!options.fresh && current?.generation === options.generation) return current.promise
    current?.controller.abort()
    const controller = new AbortController()
    const promise = (async () => {
      if (!options.silent && isCurrent(options.generation)) setStats((state) => state.kind === "ready" ? state : { kind: "loading" })
      try {
        const value = await api.stats(boardSlug, controller.signal)
        if (isCurrent(options.generation)) {
          setStats({ kind: "ready", value })
          return true
        }
        return false
      } catch (error) {
        if (controller.signal.aborted || isAbortError(error) || !isCurrent(options.generation)) return false
        setStats({ kind: "error", error })
        return false
      } finally {
        if (statsRequestRef.current?.controller === controller) statsRequestRef.current = null
      }
    })()
    statsRequestRef.current = { generation: options.generation, controller, promise }
    return promise
  }, [api, boardSlug, isCurrent])

  const loadSearchStatus = useCallback((options: QueryLoadOptions): Promise<boolean> => {
    const current = searchRequestRef.current
    if (!options.fresh && current?.generation === options.generation) return current.promise
    current?.controller.abort()
    const controller = new AbortController()
    const promise = (async () => {
      if (!options.silent && isCurrent(options.generation)) setSearchStatus((state) => state.kind === "ready" ? state : { kind: "loading" })
      try {
        const value = await api.searchStatus(boardSlug, controller.signal)
        if (isCurrent(options.generation)) {
          setSearchStatus({ kind: "ready", value })
          return true
        }
        return false
      } catch (error) {
        if (controller.signal.aborted || isAbortError(error) || !isCurrent(options.generation)) return false
        setSearchStatus({ kind: "error", error })
        return false
      } finally {
        if (searchRequestRef.current?.controller === controller) searchRequestRef.current = null
      }
    })()
    searchRequestRef.current = { generation: options.generation, controller, promise }
    return promise
  }, [api, boardSlug, isCurrent])

  const loadBoardDiagnostics = useCallback((options: QueryLoadOptions, target: DiagnosticTarget = "all"): Promise<boolean> => {
    if (target === "stats") return loadStats(options)
    if (target === "search") return loadSearchStatus(options)
    return Promise.all([loadStats(options), loadSearchStatus(options)]).then(([statsFresh, searchFresh]) => statsFresh && searchFresh)
  }, [loadSearchStatus, loadStats])

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      generationRef.current += 1
      pendingActionRef.current = null
      abortRequests()
    }
  }, [abortRequests])

  useEffect(() => {
    const generation = generationRef.current + 1
    generationRef.current = generation
    abortRequests()
    const preserveInitial = firstMountRef.current
    firstMountRef.current = false
    mutationTokenRef.current += 1
    if (!preserveInitial || !initial?.status) setStatus({ kind: "loading" })
    if (!preserveInitial || !initial?.stats) setStats({ kind: "loading" })
    if (!preserveInitial || !initial?.searchStatus) setSearchStatus({ kind: "loading" })
    setActionError(null)
    setConfirm(null)
    setPendingAction(null)
    pendingActionRef.current = null
    setSyncNotice(null)
    void loadStatus({ generation, fresh: true })
    void loadBoardDiagnostics({ generation, fresh: true })
    return () => {
      if (generationRef.current === generation) {
        pendingActionRef.current = null
        abortRequests()
      }
    }
  }, [abortRequests, api, boardSlug, initial, loadBoardDiagnostics, loadStatus])

  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState !== "hidden") void loadStatus({ generation: generationRef.current, silent: true })
    }
    const onVisibility = () => refresh()
    window.addEventListener("focus", refresh)
    document.addEventListener("visibilitychange", onVisibility)
    const interval = window.setInterval(refresh, 5_000)
    return () => {
      window.removeEventListener("focus", refresh)
      document.removeEventListener("visibilitychange", onVisibility)
      window.clearInterval(interval)
    }
  }, [loadStatus])

  const refreshAll = useCallback(async () => {
    const generation = generationRef.current
    const [statusFresh, diagnosticsFresh] = await Promise.all([
      loadStatus({ generation, fresh: true }),
      loadBoardDiagnostics({ generation, fresh: true }),
    ])
    if (isCurrent(generation) && (!statusFresh || !diagnosticsFresh)) setSyncNotice("stale")
    else if (isCurrent(generation)) setSyncNotice(null)
  }, [isCurrent, loadBoardDiagnostics, loadStatus])

  const retryStatus = useCallback(() => {
    void loadStatus({ generation: generationRef.current, fresh: true })
  }, [loadStatus])

  const retryStats = useCallback(() => {
    void loadBoardDiagnostics({ generation: generationRef.current, fresh: true }, "stats")
  }, [loadBoardDiagnostics])

  const retrySearch = useCallback(() => {
    void loadBoardDiagnostics({ generation: generationRef.current, fresh: true }, "search")
  }, [loadBoardDiagnostics])

  const settleMutation = useCallback(async <T extends Result>(action: ResultKey, work: () => Promise<T>, onSuccess?: (value: T) => void) => {
    if (pendingActionRef.current !== null || pendingAction !== null) return
    pendingActionRef.current = action
    const generation = generationRef.current
    const mutationToken = ++mutationTokenRef.current
    const isMutationCurrent = () => isCurrent(generation) && mutationTokenRef.current === mutationToken
    setPendingAction(action)
    setActionError(null)
    setSyncNotice(null)
    abortRequests()
    setStatus({ kind: "loading" })
    setStats({ kind: "loading" })
    setSearchStatus({ kind: "loading" })
    setResults((previous) => {
      const next = { ...previous }
      delete next[action]
      return next
    })
    let committed = false
    try {
      const value = await work()
      if (!isMutationCurrent()) return
      committed = true
      setResults((previous) => ({ ...previous, [action]: value }))
      onSuccess?.(value)
    } catch (error) {
      if (isMutationCurrent()) setActionError({ action, error })
    } finally {
      if (isMutationCurrent()) {
        const [statusFresh, diagnosticsFresh] = await Promise.all([
          loadStatus({ generation, fresh: true }),
          loadBoardDiagnostics({ generation, fresh: true }),
        ])
        if (isMutationCurrent()) {
          if (committed) {
            if (!statusFresh || !diagnosticsFresh) setSyncNotice("mutation")
            const refreshHealth = onHealthRefresh ?? requestHealthRefresh
            refreshHealth()
          }
          setPendingAction(null)
          if (pendingActionRef.current === action) pendingActionRef.current = null
        }
      }
    }
  }, [abortRequests, isCurrent, loadBoardDiagnostics, loadStatus, onHealthRefresh, pendingAction])

  const runDoctor = () => {
    if (pendingActionRef.current !== null || pendingAction !== null) return
    const generation = generationRef.current
    doctorRequestRef.current?.controller.abort()
    const controller = new AbortController()
    setPendingAction("doctor")
    pendingActionRef.current = "doctor"
    setActionError(null)
    const promise = api.doctor(controller.signal)
    doctorRequestRef.current = { generation, controller, promise: promise.then(() => true, () => false) }
    void promise
      .then((value) => { if (isCurrent(generation)) setDoctor({ kind: "ready", value }) })
      .catch((error: unknown) => {
        if (!controller.signal.aborted && isCurrent(generation)) {
          setDoctor({ kind: "error", error })
          setActionError({ action: "doctor", error })
        }
      })
      .finally(() => {
        const ownsRequest = doctorRequestRef.current?.controller === controller
        if (ownsRequest) {
          doctorRequestRef.current = null
          if (pendingActionRef.current === "doctor") pendingActionRef.current = null
        }
        if (ownsRequest && isCurrent(generation)) setPendingAction(null)
      })
  }

  const confirmAction = () => {
    const action = confirm
    setConfirm(null)
    restoreConfirmFocus()
    if (!action) return
    switch (action.kind) {
      case "checkpoint":
        void settleMutation("checkpoint", () => api.checkpoint())
        break
      case "backup":
        void settleMutation("backup", () => api.backup(action.path))
        break
      case "export":
        void settleMutation("export", () => api.exportData(action.path))
        break
      case "import":
        void settleMutation("import", () => api.importData(action.path, action.replace))
        break
      case "run":
        void settleMutation("run", () => api.maintenanceRun(action.owner))
        break
      case "rebuild":
        void settleMutation("rebuild", () => api.maintenanceRebuild(action.owner))
        break
      case "cleanup":
        void settleMutation("cleanup", () => api.maintenanceCleanup(action.owner))
        break
      case "vacuum":
        void settleMutation("vacuum", () => api.vacuum())
        break
    }
  }

  const isBusy = pendingAction !== null
  const [backupPath, setBackupPath] = useState("")
  const [exportPath, setExportPath] = useState("")
  const [importPath, setImportPath] = useState("")
  const [replaceImport, setReplaceImport] = useState(false)
  const [maintenanceOwner, setMaintenanceOwner] = useState("")

  const openConfirm = (action: ConfirmAction) => {
    confirmOpenerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    setConfirm(action)
  }

  function restoreConfirmFocus() {
    const opener = confirmOpenerRef.current
    confirmOpenerRef.current = null
    if (opener) queueMicrotask(() => opener.focus())
  }

  return (
    <PageFrame
      frame="content"
      aria-labelledby="maintenance-heading"
      bodyLabelledBy="maintenance-heading"
      data-testid="maintenance-page"
      header={(
        <SafeHStack gap={4} justify="between" align="start" wrap="wrap" aria-busy={status.kind === "loading" || undefined}>
          <SafeVStack gap={1} className="min-w-0">
            <Text as="p" type="supporting" display="block">{t("productKicker")}</Text>
            <Heading level={1} id="maintenance-heading">{t("maintenanceHeading")}</Heading>
            <Text as="p" type="supporting" display="block">{t("maintenanceDescription")}</Text>
          </SafeVStack>
          <Button label={status.kind === "loading" ? t("loading") : t("refresh")} variant="secondary" size="sm" isDisabled={isBusy || status.kind === "loading"} onClick={refreshAll} data-testid="maintenance-refresh" />
        </SafeHStack>
      )}
    >
      <SafeVStack as="section" gap={6} aria-labelledby="maintenance-heading">
        {status.kind === "loading" && stats.kind === "loading" && searchStatus.kind === "loading" ? (
          <Banner status="info" title={t("loading")} container="section" role="status" aria-live="polite" data-testid="maintenance-loading" />
        ) : null}

        <SafeSection variant="transparent" padding={0} dividers={["bottom"]} role="region" aria-labelledby="maintenance-host-diagnostics-heading" data-testid="maintenance-host-diagnostics">
          <SafeVStack gap={4} paddingBlock={4}>
            <SafeVStack gap={1}>
              <Text as="p" type="supporting" display="block">{t("hostTarget")}</Text>
              <Heading level={2} id="maintenance-host-diagnostics-heading">{t("hostDiagnosticsHeading")}</Heading>
              <Text as="p" type="supporting" display="block">{t("hostDiagnosticsDescription")}</Text>
            </SafeVStack>
            <Grid label={t("hostDiagnosticsHeading")} columns="auto-md" gap={4}>
              <Panel title={t("maintenanceStatusHeading")} testId="maintenance-status">
                <StatusContent state={status} t={t} locale={locale} onRetry={retryStatus} />
              </Panel>
              <Panel title={t("doctorHeading")} testId="maintenance-doctor">
                <SafeVStack gap={3} aria-busy={pendingAction === "doctor" || undefined}>
                  <Button label={pendingAction === "doctor" ? t("loading") : t("runDoctor")} variant="secondary" size="sm" isDisabled={isBusy} onClick={runDoctor} data-testid="maintenance-doctor-submit" />
                  {doctor.kind === "ready" ? <DoctorContent report={doctor.value} t={t} /> : null}
                  {doctor.kind === "loading" ? <Boundary text={pendingAction === "doctor" ? t("loading") : t("doctorNotRun")} /> : null}
                  {doctor.kind === "error" ? <InlineError error={doctor.error} t={t} action="doctor" actionError={actionError} /> : null}
                </SafeVStack>
              </Panel>
            </Grid>
          </SafeVStack>
        </SafeSection>

        <SafeSection variant="transparent" padding={0} dividers={["top"]} role="region" aria-labelledby="maintenance-operations-heading" aria-busy={isBusy}>
              <SafeVStack gap={4} paddingBlock={4}>
                <SafeVStack gap={1}>
                  <Text as="p" type="supporting" display="block">{t("hostTarget")}</Text>
                  <Heading level={2} id="maintenance-operations-heading">{t("maintenanceOperationsHeading")}</Heading>
                  <Text as="p" type="supporting" display="block">{t("hostOperationsDescription")}</Text>
                </SafeVStack>
                <Grid label={t("maintenanceOperationsHeading")} columns="auto-md" gap={4}>
                  <PathOperation label={t("backupPathLabel")} value={backupPath} onChange={setBackupPath} buttonLabel={t("backupAction")} loadingLabel={t("loading")} disabled={isBusy || !backupPath.trim()} loading={pendingAction === "backup"} onClick={() => openConfirm({ kind: "backup", path: backupPath.trim() })} testId="maintenance-backup" />
                  <PathOperation label={t("exportPathLabel")} value={exportPath} onChange={setExportPath} buttonLabel={t("exportAction")} loadingLabel={t("loading")} disabled={isBusy || !exportPath.trim()} loading={pendingAction === "export"} onClick={() => openConfirm({ kind: "export", path: exportPath.trim() })} testId="maintenance-export" />
                  <SafeCard padding={4} role="group" aria-labelledby="maintenance-import-heading" data-testid="maintenance-import" aria-busy={pendingAction === "import"}>
                    <SafeVStack gap={3}>
                      <Heading level={3} id="maintenance-import-heading">{t("portableImportHeading")}</Heading>
                      <TextInput type="text" label={t("importPathLabel")} value={importPath} onChange={(value) => setImportPath(value)} placeholder={t("importPathPlaceholder")} htmlName="maintenance-import-path" data-testid="maintenance-import-path" />
                      <CheckboxInput label={t("replaceImportLabel")} value={replaceImport} onChange={(checked) => setReplaceImport(checked)} size="sm" htmlName="maintenance-replace-import" />
                      <Button label={pendingAction === "import" ? t("loading") : replaceImport ? t("replaceImportAction") : t("importAction")} variant={replaceImport ? "destructive" : "secondary"} size="sm" isDisabled={isBusy || !importPath.trim()} onClick={() => openConfirm({ kind: "import", path: importPath.trim(), replace: replaceImport })} data-testid="maintenance-import-submit" />
                      {resultFor(results.import, "import", t)}
                      <InlineError error={actionError?.action === "import" ? actionError.error : null} t={t} action="import" actionError={actionError} />
                    </SafeVStack>
                  </SafeCard>
                  <SafeCard padding={4} role="group" aria-labelledby="maintenance-projection-heading" data-testid="maintenance-projection" aria-busy={["run", "rebuild", "cleanup", "vacuum"].includes(pendingAction as "run" | "rebuild" | "cleanup" | "vacuum")}>
                    <SafeVStack gap={3}>
                      <Heading level={3} id="maintenance-projection-heading">{t("projectionMaintenanceHeading")}</Heading>
                      <Text as="p" type="supporting" display="block">{t("projectionMaintenanceDescription")}</Text>
                      <TextInput type="text" label={t("maintenanceOwnerLabel")} value={maintenanceOwner} onChange={(value) => setMaintenanceOwner(value)} placeholder={actor || runtime.actor} htmlName="maintenance-owner" data-testid="maintenance-owner" />
                      <SafeHStack gap={2} wrap="wrap">
                        <Button label={pendingAction === "run" ? t("loading") : t("runMaintenanceAction")} variant="secondary" size="sm" isDisabled={isBusy} onClick={() => openConfirm({ kind: "run", owner: maintenanceOwnerForAction(maintenanceOwner, actor, runtime.actor) })} data-testid="maintenance-run-submit" />
                        <Button label={pendingAction === "rebuild" ? t("loading") : t("rebuildAction")} variant="destructive" size="sm" isDisabled={isBusy} onClick={() => openConfirm({ kind: "rebuild", owner: maintenanceOwnerForAction(maintenanceOwner, actor, runtime.actor) })} data-testid="maintenance-rebuild-submit" />
                        <Button label={pendingAction === "cleanup" ? t("loading") : t("cleanupAction")} variant="destructive" size="sm" isDisabled={isBusy} onClick={() => openConfirm({ kind: "cleanup", owner: maintenanceOwnerForAction(maintenanceOwner, actor, runtime.actor) })} data-testid="maintenance-cleanup-submit" />
                        <Button label={pendingAction === "vacuum" ? t("loading") : t("vacuumAction")} variant="destructive" size="sm" isDisabled={isBusy} onClick={() => openConfirm({ kind: "vacuum" })} data-testid="maintenance-vacuum-submit" />
                      </SafeHStack>
                      {resultFor(results.run, "run", t)}
                      {resultFor(results.rebuild, "rebuild", t)}
                      {resultFor(results.cleanup, "cleanup", t)}
                      {resultFor(results.vacuum, "vacuum", t)}
                      {(["run", "rebuild", "cleanup", "vacuum"] as const).map((action) => <InlineError key={action} error={actionError?.action === action ? actionError.error : null} t={t} action={action} actionError={actionError} />)}
                    </SafeVStack>
                  </SafeCard>
                  <SafeCard padding={4} role="group" aria-labelledby="maintenance-checkpoint-heading" data-testid="maintenance-checkpoint" aria-busy={pendingAction === "checkpoint"}>
                    <SafeVStack gap={3}>
                      <Heading level={3} id="maintenance-checkpoint-heading">{t("checkpointHeading")}</Heading>
                      <Text as="p" type="supporting" display="block">{t("checkpointDescription")}</Text>
                      <Button label={pendingAction === "checkpoint" ? t("loading") : t("checkpointAction")} variant="secondary" size="sm" isDisabled={isBusy} onClick={() => openConfirm({ kind: "checkpoint" })} data-testid="maintenance-checkpoint-submit" />
                      {resultFor(results.checkpoint, "checkpoint", t)}
                      <InlineError error={actionError?.action === "checkpoint" ? actionError.error : null} t={t} action="checkpoint" actionError={actionError} />
                    </SafeVStack>
                  </SafeCard>
                  <Banner status="warning" title={t("legacyImportHeading")} description={t("legacyImportUnsupported")} container="card" data-testid="maintenance-legacy-import-unsupported" />
                </Grid>
                {resultFor(results.backup, "backup", t)}
                {resultFor(results.export, "export", t)}
                <InlineError error={actionError?.action === "backup" ? actionError.error : null} t={t} action="backup" actionError={actionError} />
                <InlineError error={actionError?.action === "export" ? actionError.error : null} t={t} action="export" actionError={actionError} />
              </SafeVStack>
            </SafeSection>

        <SafeSection variant="transparent" padding={0} dividers={["bottom"]} role="region" aria-labelledby="maintenance-board-diagnostics-heading" data-testid="maintenance-board-diagnostics">
          <SafeVStack gap={4} paddingBlock={4}>
            <SafeVStack gap={1}>
              <SafeHStack gap={2} align="center" wrap="wrap">
                <Text as="p" type="supporting" display="block">{t("currentBoard")}</Text>
                <LiteralText type="code" display="block">{boardSlug}</LiteralText>
              </SafeHStack>
              <Heading level={2} id="maintenance-board-diagnostics-heading">{t("boardDiagnosticsHeading")}</Heading>
              <Text as="p" type="supporting" display="block">{t("boardDiagnosticsDescription")}</Text>
            </SafeVStack>
            <Grid label={t("boardDiagnosticsHeading")} columns="auto-md" gap={4}>
              <Panel title={t("statsHeading")} testId="maintenance-stats">
                <StatsContent state={stats} t={t} locale={locale} onRetry={retryStats} />
              </Panel>
              <Panel title={t("searchStatusHeading")} testId="maintenance-search-status">
                <SearchContent state={searchStatus} t={t} onRetry={retrySearch} />
              </Panel>
            </Grid>
          </SafeVStack>
        </SafeSection>

        {syncNotice ? (
          <Banner
            status={syncNotice === "stale" ? "warning" : "info"}
            title={t(syncNotice === "stale" ? "maintenanceDataStale" : "mutationSubmittedSyncPending")}
            container="section"
            role="status"
            aria-live="polite"
            data-testid={syncNotice === "stale" ? "maintenance-stale" : "maintenance-sync-pending"}
            data-notice-kind={syncNotice}
            endContent={<Button label={t("retry")} variant="ghost" size="sm" isDisabled={isBusy} onClick={() => { setSyncNotice(null); void refreshAll() }} data-testid="maintenance-sync-retry" />}
          />
        ) : null}

        <Dialog
          isOpen={confirm !== null}
          onOpenChange={(isOpen) => { if (!isOpen) { setConfirm(null); restoreConfirmFocus() } }}
          role="alertdialog"
          aria-labelledby="maintenance-confirm-title"
          aria-describedby="maintenance-confirm-description"
          returnFocusRef={confirmOpenerRef}
          initialFocusRef={confirmCancelRef}
          closeOnBackdrop={false}
          data-testid="maintenance-confirm-dialog"
        >
          <SafeVStack as="section" gap={4} padding={6}>
            <SafeVStack gap={1}>
              <Heading level={2} id="maintenance-confirm-title">{confirm ? confirmTitle(confirm, t) : t("maintenanceHeading")}</Heading>
              <Text as="p" type="body" color="secondary" id="maintenance-confirm-description">{confirm ? confirmDescription(confirm, t) : ""}</Text>
            </SafeVStack>
            <SafeHStack gap={2} justify="end" wrap="wrap">
              <Button ref={confirmCancelRef} type="button" label={t("cancel")} variant="ghost" isDisabled={pendingAction !== null} data-autofocus data-testid="maintenance-confirm-cancel" onClick={() => { setConfirm(null); restoreConfirmFocus() }} />
              <Button type="button" label={pendingAction !== null ? t("loading") : t("continue")} variant={confirm && isDestructive(confirm) ? "destructive" : "primary"} isDisabled={pendingAction !== null} data-testid="maintenance-confirm-action" onClick={confirmAction} />
            </SafeHStack>
          </SafeVStack>
        </Dialog>
      </SafeVStack>
    </PageFrame>
  )
}

function Panel({ title, testId, children }: { title: string; testId: string; children: ReactNode }) {
  return <SafeCard padding={4} role="region" aria-labelledby={`${testId}-heading`} data-testid={testId}><SafeVStack gap={3}><Heading level={2} id={`${testId}-heading`}>{title}</Heading>{children}</SafeVStack></SafeCard>
}

function PathOperation({ label, value, onChange, buttonLabel, loadingLabel, disabled, loading, onClick, testId }: { label: string; value: string; onChange: (value: string) => void; buttonLabel: string; loadingLabel: string; disabled: boolean; loading: boolean; onClick: () => void; testId: string }) {
  return <SafeCard padding={4} role="group" aria-labelledby={`${testId}-heading`} data-testid={testId} aria-busy={loading}>
    <SafeVStack gap={3}>
      <Heading level={3} id={`${testId}-heading`}>{label}</Heading>
      <TextInput type="text" label={label} value={value} onChange={(next) => onChange(next)} htmlName={`${testId}-path`} data-testid={`${testId}-path`} />
      <Button label={loading ? loadingLabel : buttonLabel} variant="secondary" size="sm" isDisabled={disabled} onClick={onClick} data-testid={`${testId}-submit`} />
    </SafeVStack>
  </SafeCard>
}

function StatusContent({ state, t, locale, onRetry }: { state: LoadState<MaintenanceStatus>; t: ReturnType<typeof createTranslator>; locale: Locale; onRetry: () => void }) {
  if (state.kind === "loading") return <Boundary text={t("loading")} />
  if (state.kind === "error") return <InlineError error={state.error} t={t} action="status" actionError={null} retry={{ label: t("retryMaintenanceStatus"), onClick: onRetry, testId: "maintenance-status-retry" }} />
  const { owner } = state.value
  return <SafeVStack gap={3}>
    <MetricGrid label={t("maintenanceStatusHeading")}>
      <Metric label={t("databaseInstance")} value={state.value.database_instance_id} />
      <Metric label={t("protocolVersion")} value={state.value.protocol_version} />
      <Metric label={t("owner")} value={owner.owner ?? t("noOwner")} />
      <Metric label={t("mode")} value={owner.mode ?? "—"} />
      <Metric label={t("active")} value={owner.active} tone={owner.active ? "primary" : "muted"} status={owner.active ? "success" : undefined} />
      <Metric label={t("fenceEpoch")} value={owner.fence_epoch} />
      <Metric label={t("leaseExpiresAt")} value={formatTimestamp(owner.lease_expires_at, locale)} />
      <Metric label={t("buildIdentity")} value={owner.build_identity} />
      <Metric label={t("lastHeartbeat")} value={formatTimestamp(owner.last_heartbeat_at, locale)} />
    </MetricGrid>
    <Text as="p" type="supporting" display="block">{t("projectionStores")}: {state.value.stores.length}</Text>
    {state.value.stores.length > 0 ? <SafeVStack gap={3}>{state.value.stores.map((store) => <SafeCard variant="muted" padding={3} key={store.store_name}>
      <SafeVStack gap={2}>
        <SafeHStack gap={2} justify="between" align="center" wrap="wrap">
          <LiteralText type="code" display="block">{store.store_name}</LiteralText>
          {store.degraded || store.dirty ? <Badge variant={statusTone(store)} label={t("degraded")} /> : <LiteralText type="code" display="block">{reported(store.lifecycle_status)}</LiteralText>}
        </SafeHStack>
        <MetricGrid label={t("projectionStores")}><Metric label={t("activeGeneration")} value={store.active_generation} /><Metric label={t("activeFingerprint")} value={store.active_fingerprint} /><Metric label={t("previousGeneration")} value={store.previous_generation} /><Metric label={t("buildingGeneration")} value={store.building_generation} /><Metric label={t("storeFenceEpoch")} value={store.fence_epoch} /><Metric label={t("storeLastEvent")} value={store.last_event_id} /><Metric label={t("pending")} value={store.pending} /><Metric label={t("running")} value={store.running} /><Metric label={t("failed")} value={store.failed} /><Metric label={t("phase")} value={store.phase} /><Metric label={t("updatedAt")} value={formatTimestamp(store.updated_at, locale)} /><Metric label={t("lastError")} value={store.last_error ? t("errorPresent") : t("none")} /><Metric label={t("storeErrors")} value={errorSummary(store.errors.length, t)} /></MetricGrid>
      </SafeVStack>
    </SafeCard>)}</SafeVStack> : <Text as="p" type="supporting" display="block">{t("noProjectionStores")}</Text>}
  </SafeVStack>
}

function StatsContent({ state, t, locale, onRetry }: { state: LoadState<QueueStats>; t: ReturnType<typeof createTranslator>; locale: Locale; onRetry: () => void }) {
  if (state.kind === "loading") return <Boundary text={t("loading")} />
  if (state.kind === "error") return <InlineError error={state.error} t={t} action="stats" actionError={null} retry={{ label: t("retryQueueStats"), onClick: onRetry, testId: "maintenance-stats-retry" }} />
  return <SafeVStack gap={3}>
    <MetricGrid label={t("statsHeading")}><Metric label={t("boardId")} value={state.value.board_id} /><Metric label={t("generatedAt")} value={formatTimestamp(state.value.generated_at, locale)} /><Metric label={t("unplannedActiveTasks")} value={state.value.unplanned_active_tasks} /><Metric label={t("incompleteRequiredSteps")} value={state.value.active_parents_with_incomplete_required_steps} /></MetricGrid>
    <Heading level={3}>{t("statusCounts")}</Heading>
    {state.value.status_counts.length > 0 ? <MetricGrid label={t("statusCounts")}>{state.value.status_counts.map((entry) => <Metric key={entry.status} label={entry.status} labelTranslateNo value={entry.count} />)}</MetricGrid> : <Text as="p" type="supporting" display="block">{t("noStatusCounts")}</Text>}
    <Heading level={3}>{t("staleClaims")}</Heading>
    {state.value.stale_claims.length > 0 ? <SafeVStack gap={3}>{state.value.stale_claims.map((claim) => <SafeCard variant="muted" padding={3} key={claim.task_id}><SafeVStack gap={2}><SafeHStack gap={2} justify="between" align="center" wrap="wrap"><LiteralText type="code" display="block">#{claim.seq} {claim.title}</LiteralText><LiteralText type="code" display="block">{claim.claim_owner ?? t("noOwner")}</LiteralText></SafeHStack><MetricGrid label={t("staleClaims")}><Metric label={t("expiresAt")} value={formatTimestamp(claim.claim_expires_at, locale)} /><Metric label={t("lastHeartbeat")} value={formatTimestamp(claim.last_heartbeat_at, locale)} /><Metric label={t("run")} value={claim.current_run_id} /><Metric label={t("retry")} value={`${claim.retry_count}/${claim.max_retries ?? "—"}`} /></MetricGrid></SafeVStack></SafeCard>)}</SafeVStack> : <Text as="p" type="supporting" display="block">{t("noStaleClaims")}</Text>}
    <Heading level={3}>{t("blockedReasons")}</Heading>
    {state.value.blocked_reasons.length > 0 ? <MetricGrid label={t("blockedReasons")}>{state.value.blocked_reasons.map((entry) => <Metric key={entry.reason} label={entry.reason || t("unspecified")} labelTranslateNo value={entry.count} />)}</MetricGrid> : <Text as="p" type="supporting" display="block">{t("noBlockedReasons")}</Text>}
  </SafeVStack>
}

function SearchContent({ state, t, onRetry }: { state: LoadState<SearchStatus>; t: ReturnType<typeof createTranslator>; onRetry: () => void }) {
  if (state.kind === "loading") return <Boundary text={t("loading")} />
  if (state.kind === "error") return <InlineError error={state.error} t={t} action="search" actionError={null} retry={{ label: t("retrySearchStatus"), onClick: onRetry, testId: "maintenance-search-retry" }} />
  return <MetricGrid label={t("searchStatusHeading")}><Metric label={t("resolvedBoardId")} value={state.value.resolved_board_id} /><Metric label={t("backend")} value={state.value.backend} /><Metric label={t("derivedIndex")} value={state.value.derived_index} /><Metric label={t("stale")} value={state.value.stale} tone={state.value.stale ? "primary" : "muted"} status={state.value.stale ? "error" : undefined} /><Metric label={t("generation")} value={state.value.generation} /><Metric label={t("lastEvent")} value={state.value.last_event_id} /><Metric label={t("lagEvents")} value={state.value.index_lag_events} /><Metric label={t("message")} value={diagnosticSummary(state.value.message, t)} /></MetricGrid>
}

function DoctorContent({ report, t }: { report: DoctorReport; t: ReturnType<typeof createTranslator> }) {
  const findings = [
    [t("integrity"), report.integrity_check],
    [t("migrationVersion"), report.migration_version],
    [t("userVersion"), report.user_version],
    [t("expiredRunningTasks"), report.expired_running_tasks],
    [t("runningWithoutActiveRun"), report.running_tasks_without_active_run],
    [t("orphanRunningRuns"), report.orphan_running_runs],
    [t("dependencyCycles"), report.dependency_cycles],
    [t("archivedDependencyEdges"), report.archived_dependency_edges],
    [t("missingRunLogs"), report.missing_run_logs],
    [t("suspiciousRunLogPaths"), report.suspicious_run_log_paths],
    [t("executableDependencyViolations"), report.executable_dependency_violations],
    [t("executableSpecViolations"), report.executable_spec_violations],
    [t("executableScheduleViolations"), report.executable_schedule_violations],
    [t("unplannedActiveTasks"), report.unplanned_active_tasks],
    [t("incompleteRequiredSteps"), report.active_parents_with_incomplete_required_steps],
    [t("outboxPending"), report.outbox_pending],
    [t("outboxRunning"), report.outbox_running],
    [t("outboxFailed"), report.outbox_failed],
    [t("dirtyStores"), report.derived_dirty_stores],
    [t("errorStores"), report.derived_error_stores],
    [t("consistencyErrors"), report.consistency_errors],
    [t("consistencyWarnings"), report.consistency_warnings],
    [t("ontologyErrors"), report.ontology_ledger_errors],
    [t("ontologyWarnings"), report.ontology_ledger_warnings],
  ] as const
  return <SafeVStack gap={3}><SafeStack as="section" data-testid="maintenance-doctor-result"><Badge variant={report.ok ? "success" : "error"} label={report.ok ? t("doctorOk") : t("doctorFindings")} /></SafeStack><MetricGrid label={t("doctorHeading")}>{findings.map(([label, value]) => <Metric key={label} label={label} value={value} />)}</MetricGrid><Heading level={3}>{t("derivedStores")}</Heading>{report.derived_stores.length > 0 ? <SafeVStack gap={3}>{report.derived_stores.map((store) => <SafeCard variant="muted" padding={3} key={store.store_name}><SafeVStack gap={2}><SafeHStack gap={2} justify="between" align="center" wrap="wrap"><LiteralText type="code" display="block">{store.store_name}</LiteralText>{store.dirty ? <Badge variant="error" label={t("degraded")} /> : <LiteralText type="supporting">{t("ready")}</LiteralText>}</SafeHStack><MetricGrid label={t("derivedStores")}><Metric label={t("schemaVersion")} value={store.schema_version} /><Metric label={t("storeLastEvent")} value={store.last_event_id} /><Metric label={t("pendingOutbox")} value={store.pending_outbox} /><Metric label={t("runningOutbox")} value={store.running_outbox} /><Metric label={t("failedOutbox")} value={store.failed_outbox} /><Metric label={t("lastError")} value={store.last_error ? t("errorPresent") : t("none")} /></MetricGrid></SafeVStack></SafeCard>)}</SafeVStack> : <Text as="p" type="supporting" display="block">{t("noDerivedStores")}</Text>}</SafeVStack>
}

function MetricGrid({ children, label }: { children: ReactNode; label: string }) {
  return <Grid label={label} columns="auto-md" gap={2}>{children}</Grid>
}

function Metric({ label, value, tone = "primary", status, labelTranslateNo = false }: { label: string; value: unknown; tone?: "primary" | "muted"; status?: "success" | "error"; labelTranslateNo?: boolean }) {
  return <SafeVStack gap={0.5} className="min-w-0">{labelTranslateNo ? <LiteralText as="p" type="supporting" display="block">{label}</LiteralText> : <Text as="p" type="supporting" display="block">{label}</Text>}{status ? <Badge variant={status} label={<LiteralText type="code">{reported(value as string | number | boolean | null | undefined)}</LiteralText>} /> : <LiteralText as="p" type="code" color={tone === "muted" ? "secondary" : "primary"} display="block" wordBreak="break-word">{reported(value as string | number | boolean | null | undefined)}</LiteralText>}</SafeVStack>
}

function LiteralText({ children, ...props }: Omit<TextProps, "children"> & { children: ReactNode }) {
  return <Text {...props}><span translate="no">{children}</span></Text>
}

function Boundary({ text }: { text: string }) { return <Banner status="info" title={text} container="section" role="status" aria-live="polite" /> }

function InlineError({ error, t, action, actionError, retry }: { error: unknown; t: ReturnType<typeof createTranslator>; action: string; actionError: { action: string; error: unknown } | null; retry?: { label: string; onClick: () => void; testId: string } }) {
  if (error === null || error === undefined || (actionError !== null && actionError.action !== action)) return null
  return <Banner status="error" title={t("maintenanceActionFailed")} description={safeErrorText(error, t)} container="card" role="alert" data-testid={`maintenance-${action}-error`} endContent={retry ? <Button label={retry.label} variant="ghost" size="sm" onClick={retry.onClick} data-testid={retry.testId} /> : undefined} />
}

function resultFor(result: Result | undefined, key: ResultKey, t: ReturnType<typeof createTranslator>): ReactNode {
  if (!result) return null
  if (key === "backup" && "out_path" in result && "checksum_sha256" in result) return <Evidence key={key} testId="maintenance-backup-result" title={t("backupComplete")} rows={[[t("serverPath"), result.out_path], [t("checksum"), result.checksum_sha256], [t("bytes"), result.bytes], [t("sourceFingerprint"), result.source_fingerprint]]} />
  if (key === "export" && "out_path" in result && "checksum_sha256" in result && "record_count" in result) return <Evidence key={key} testId="maintenance-export-result" title={t("exportComplete")} rows={[[t("serverPath"), result.out_path], [t("checksum"), result.checksum_sha256], [t("bytes"), result.bytes], [t("recordCount"), result.record_count], [t("sourceFingerprint"), result.source_fingerprint]]} />
  if (key === "import" && "in_path" in result) return <Evidence key={key} testId="maintenance-import-result" title={t("importComplete")} rows={[[t("serverPath"), result.in_path], [t("sourceFingerprint"), result.source_fingerprint], [t("importedRecords"), result.imported_records], [t("skippedRecords"), result.skipped_records], [t("rebuildJobs"), result.rebuild_jobs_enqueued], [t("journal"), result.journal_id], [t("phase"), result.phase], [t("restartRequired"), result.restart_required], [t("stagedDatabasePath"), result.staged_database_path], [t("targetFingerprintBefore"), result.target_fingerprint_before], [t("stagedFingerprint"), result.staged_fingerprint], [t("publishPreconditions"), errorSummary(result.publish_preconditions.length, t)]]} />
  if (key === "checkpoint" && "checkpointed_frames" in result) return <Evidence key={key} testId="maintenance-checkpoint-result" title={t("checkpointComplete")} rows={[[t("busy"), result.busy], [t("logFrames"), result.log_frames], [t("checkpointedFrames"), result.checkpointed_frames]]} />
  if (key === "vacuum" && "before_bytes" in result) return <Evidence key={key} testId="maintenance-vacuum-result" title={t("vacuumComplete")} rows={[[t("status"), result.ok], [t("beforeBytes"), result.before_bytes], [t("afterBytes"), result.after_bytes], [t("sourceFingerprint"), result.source_fingerprint]]} />
  if ((key === "run" || key === "rebuild" || key === "cleanup") && "action" in result) return <Evidence key={key} testId={`maintenance-${key}-result`} title={t("maintenanceComplete")} rows={[[t("action"), result.action], [t("owner"), result.owner], [t("processed"), result.processed], [t("phase"), result.phase], [t("degraded"), result.degraded], [t("errors"), errorSummary(result.errors.length, t)]]} />
  return null
}

function Evidence({ title, rows, testId }: { title: string; rows: Array<[string, unknown]>; testId: string }) {
  return <Banner status="success" title={title} container="card" defaultIsExpanded data-testid={testId} aria-live="polite"><MetricGrid label={title}>{rows.map(([label, value]) => <Metric key={label} label={label} value={value} />)}</MetricGrid></Banner>
}

function isDestructive(action: ConfirmAction): boolean { return action.kind !== "checkpoint" && action.kind !== "run" }

function confirmTitle(action: ConfirmAction, t: ReturnType<typeof createTranslator>): string {
  if (action.kind === "import") return action.replace ? t("confirmReplaceImport") : t("confirmImport")
  const titles: Record<Exclude<ConfirmAction["kind"], "import">, string> = { checkpoint: t("confirmCheckpoint"), backup: t("confirmBackup"), export: t("confirmExport"), run: t("confirmRun"), rebuild: t("confirmRebuild"), cleanup: t("confirmCleanup"), vacuum: t("confirmVacuum") }
  return titles[action.kind]
}

function confirmDescription(action: ConfirmAction, t: ReturnType<typeof createTranslator>): string {
  if (action.kind === "backup" || action.kind === "export") return `${t("confirmPathPrefix")} ${action.path}`
  if (action.kind === "import") return `${action.replace ? t("confirmReplaceImportDescription") : t("confirmImportDescription")} ${t("confirmPathPrefix")} ${action.path}`
  if (action.kind === "run" || action.kind === "rebuild" || action.kind === "cleanup") {
    const descriptions = { run: t("confirmRunDescription"), rebuild: t("confirmRebuildDescription"), cleanup: t("confirmCleanupDescription") }
    return `${descriptions[action.kind]} ${t("owner")}: ${action.owner}`
  }
  const descriptions: Record<Exclude<ConfirmAction["kind"], "backup" | "export" | "import" | "run" | "rebuild" | "cleanup">, string> = { checkpoint: t("confirmCheckpointDescription"), vacuum: t("confirmVacuumDescription") }
  return descriptions[action.kind]
}

import { AlertDialog } from "@astryxdesign/core/AlertDialog"
import { Button } from "@astryxdesign/core/Button"
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react"

import type { WebRuntimeConfig } from "../../lib/runtime"
import type { Locale } from "../../lib/preferences"
import { createTranslator } from "../../lib/i18n"
import { requestHealthRefresh } from "../../lib/health-refresh"
import { usePreferences } from "../../lib/use-preferences"
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
import styles from "./maintenance-page.module.css"

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
  | { kind: "run" }
  | { kind: "rebuild" }
  | { kind: "cleanup" }
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
    invalid_content_type: t("maintenanceErrorContentType"),
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

function statusTone(status: { dirty: boolean; degraded: boolean; failed: number; last_error: string | null; lifecycle_status: string }): string {
  return status.degraded || status.dirty || status.failed > 0 || Boolean(status.last_error) || /degraded|error|failed/i.test(status.lifecycle_status)
    ? styles.degraded
    : styles.ready
}

export function MaintenancePage({ runtime, boardSlug, api: providedApi, initial, onHealthRefresh }: MaintenancePageProps) {
  const { locale } = usePreferences()
  const t = createTranslator(locale)
  const api = useMemo(() => providedApi ?? createMaintenanceApi({}, runtime), [providedApi, runtime])
  const [status, setStatus] = useState<LoadState<MaintenanceStatus>>(() => initial?.status ? { kind: "ready", value: initial.status } : emptyState())
  const [stats, setStats] = useState<LoadState<QueueStats>>(() => initial?.stats ? { kind: "ready", value: initial.stats } : emptyState())
  const [searchStatus, setSearchStatus] = useState<LoadState<SearchStatus>>(() => initial?.searchStatus ? { kind: "ready", value: initial.searchStatus } : emptyState())
  const [doctor, setDoctor] = useState<LoadState<DoctorReport>>(() => initial?.doctor ? { kind: "ready", value: initial.doctor } : { kind: "loading" })
  const [results, setResults] = useState<Partial<Record<ResultKey, Result>>>(() => initial?.results ?? {})
  const [pendingAction, setPendingAction] = useState<ResultKey | "doctor" | null>(null)
  const [actionError, setActionError] = useState<{ action: ResultKey | "doctor"; error: unknown } | null>(null)
  const [syncNotice, setSyncNotice] = useState(false)
  const [confirm, setConfirm] = useState<ConfirmAction | null>(null)
  const confirmOpenerRef = useRef<HTMLElement | null>(null)
  const statusRequestRef = useRef<QueryRequest | null>(null)
  const diagnosticsRequestRef = useRef<QueryRequest | null>(null)
  const doctorRequestRef = useRef<QueryRequest | null>(null)
  const mountedRef = useRef(false)
  const firstMountRef = useRef(true)
  const generationRef = useRef(0)
  const mutationTokenRef = useRef(0)

  const abortRequests = useCallback(() => {
    const requests = [statusRequestRef.current, diagnosticsRequestRef.current, doctorRequestRef.current]
    statusRequestRef.current = null
    diagnosticsRequestRef.current = null
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
          return true
        }
        return false
      } catch (error) {
        if (controller.signal.aborted || isAbortError(error) || !isCurrent(options.generation)) return false
        setStatus((state) => options.silent && state.kind === "ready" ? state : { kind: "error", error })
        return false
      } finally {
        if (statusRequestRef.current?.controller === controller) statusRequestRef.current = null
      }
    })()
    statusRequestRef.current = { generation: options.generation, controller, promise }
    return promise
  }, [api, isCurrent])

  const loadBoardDiagnostics = useCallback((options: QueryLoadOptions): Promise<boolean> => {
    const current = diagnosticsRequestRef.current
    if (!options.fresh && current?.generation === options.generation) return current.promise
    current?.controller.abort()
    const controller = new AbortController()
    const promise = (async () => {
      try {
        const [statsResult, searchResult] = await Promise.allSettled([api.stats(boardSlug, controller.signal), api.searchStatus(boardSlug, controller.signal)])
        if (!isCurrent(options.generation) || controller.signal.aborted) return false
        if (statsResult.status === "fulfilled") setStats({ kind: "ready", value: statsResult.value })
        else if (!isAbortError(statsResult.reason)) setStats({ kind: "error", error: statsResult.reason })
        if (searchResult.status === "fulfilled") setSearchStatus({ kind: "ready", value: searchResult.value })
        else if (!isAbortError(searchResult.reason)) setSearchStatus({ kind: "error", error: searchResult.reason })
        return statsResult.status === "fulfilled" && searchResult.status === "fulfilled"
      } finally {
        if (diagnosticsRequestRef.current?.controller === controller) diagnosticsRequestRef.current = null
      }
    })()
    diagnosticsRequestRef.current = { generation: options.generation, controller, promise }
    return promise
  }, [api, boardSlug, isCurrent])

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      generationRef.current += 1
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
    setSyncNotice(false)
    void loadStatus({ generation, fresh: true })
    void loadBoardDiagnostics({ generation, fresh: true })
    return () => {
      if (generationRef.current === generation) abortRequests()
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
    if (isCurrent(generation) && (!statusFresh || !diagnosticsFresh)) setSyncNotice(true)
  }, [isCurrent, loadBoardDiagnostics, loadStatus])

  const settleMutation = useCallback(async <T extends Result>(action: ResultKey, work: () => Promise<T>, onSuccess?: (value: T) => void) => {
    if (pendingAction !== null) return
    const generation = generationRef.current
    const mutationToken = ++mutationTokenRef.current
    const isMutationCurrent = () => isCurrent(generation) && mutationTokenRef.current === mutationToken
    setPendingAction(action)
    setActionError(null)
    setSyncNotice(false)
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
            if (!statusFresh || !diagnosticsFresh) setSyncNotice(true)
            const refreshHealth = onHealthRefresh ?? requestHealthRefresh
            refreshHealth()
          }
          setPendingAction(null)
        }
      }
    }
  }, [abortRequests, isCurrent, loadBoardDiagnostics, loadStatus, onHealthRefresh, pendingAction])

  const runDoctor = () => {
    if (pendingAction !== null) return
    const generation = generationRef.current
    doctorRequestRef.current?.controller.abort()
    const controller = new AbortController()
    setPendingAction("doctor")
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
        if (doctorRequestRef.current?.generation === generation) doctorRequestRef.current = null
        if (isCurrent(generation)) setPendingAction(null)
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
        void settleMutation("run", () => api.maintenanceRun(maintenanceOwner.trim() || runtime.actor, "run"))
        break
      case "rebuild":
        void settleMutation("rebuild", () => api.maintenanceRebuild(maintenanceOwner.trim() || runtime.actor))
        break
      case "cleanup":
        void settleMutation("cleanup", () => api.maintenanceCleanup(maintenanceOwner.trim() || runtime.actor))
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
    <section className={styles.page} aria-labelledby="maintenance-heading" data-testid="maintenance-page">
      <div className={styles.headingRow}>
        <div className={styles.pageHeading}>
          <p className={styles.eyebrow}>{t("productKicker")}</p>
          <h1 id="maintenance-heading">{t("maintenanceHeading")}</h1>
          <p className={styles.lede}>{t("maintenanceDescription")}</p>
        </div>
        <Button label={t("refresh")} variant="secondary" isDisabled={isBusy || status.kind === "loading"} isLoading={status.kind === "loading"} onClick={refreshAll} data-testid="maintenance-refresh" />
      </div>

      {status.kind === "loading" && stats.kind === "loading" && searchStatus.kind === "loading" ? (
        <div className={styles.boundary} role="status" aria-live="polite" data-testid="maintenance-loading">{t("loading")}</div>
      ) : null}

      <div className={styles.overviewGrid}>
        <Panel title={t("maintenanceStatusHeading")} testId="maintenance-status">
          <StatusContent state={status} t={t} locale={locale} />
        </Panel>
        <Panel title={t("statsHeading")} testId="maintenance-stats">
          <StatsContent state={stats} t={t} locale={locale} />
        </Panel>
        <Panel title={t("searchStatusHeading")} testId="maintenance-search-status">
          <SearchContent state={searchStatus} t={t} />
        </Panel>
        <Panel title={t("doctorHeading")} testId="maintenance-doctor">
          <Button label={pendingAction === "doctor" ? t("loading") : t("runDoctor")} variant="secondary" isDisabled={isBusy} isLoading={pendingAction === "doctor"} onClick={runDoctor} data-testid="maintenance-doctor-submit" />
          {doctor.kind === "ready" ? <DoctorContent report={doctor.value} t={t} /> : null}
          {doctor.kind === "error" ? <InlineError error={doctor.error} t={t} action="doctor" actionError={actionError} /> : null}
        </Panel>
      </div>

      {syncNotice ? <div className={styles.boundary} role="status" aria-live="polite" data-testid="maintenance-sync-pending"><span>{t("mutationSubmittedSyncPending")}</span> <Button label={t("retry")} variant="secondary" isDisabled={isBusy} onClick={() => { setSyncNotice(false); void refreshAll() }} data-testid="maintenance-sync-retry" /></div> : null}

      <section className={styles.operations} aria-labelledby="maintenance-operations-heading" aria-busy={isBusy}>
        <div className={styles.sectionHeading}>
          <div>
            <p className={styles.eyebrow}>{t("hostAdministration")}</p>
            <h2 id="maintenance-operations-heading">{t("maintenanceOperationsHeading")}</h2>
          </div>
          <span className={styles.scope} translate="no">{boardSlug}</span>
        </div>
        <div className={styles.operationGrid}>
          <PathOperation label={t("backupPathLabel")} value={backupPath} onChange={setBackupPath} buttonLabel={t("backupAction")} disabled={isBusy || !backupPath.trim()} loading={pendingAction === "backup"} onClick={() => openConfirm({ kind: "backup", path: backupPath.trim() })} testId="maintenance-backup" />
          <PathOperation label={t("exportPathLabel")} value={exportPath} onChange={setExportPath} buttonLabel={t("exportAction")} disabled={isBusy || !exportPath.trim()} loading={pendingAction === "export"} onClick={() => openConfirm({ kind: "export", path: exportPath.trim() })} testId="maintenance-export" />
          <section className={styles.operation} aria-labelledby="maintenance-import-heading" data-testid="maintenance-import" aria-busy={pendingAction === "import"}>
            <h3 id="maintenance-import-heading">{t("portableImportHeading")}</h3>
            <label className={styles.field} htmlFor="maintenance-import-path">
              <span>{t("importPathLabel")}</span>
              <input id="maintenance-import-path" name="maintenance-import-path" autoComplete="off" translate="no" value={importPath} onChange={(event) => setImportPath(event.currentTarget.value)} placeholder={t("importPathPlaceholder")} data-testid="maintenance-import-path" />
            </label>
            <label className={styles.checkbox}>
              <input type="checkbox" checked={replaceImport} onChange={(event) => setReplaceImport(event.currentTarget.checked)} />
              <span>{t("replaceImportLabel")}</span>
            </label>
            <Button label={replaceImport ? t("replaceImportAction") : t("importAction")} variant={replaceImport ? "destructive" : "secondary"} isDisabled={isBusy || !importPath.trim()} isLoading={pendingAction === "import"} onClick={() => openConfirm({ kind: "import", path: importPath.trim(), replace: replaceImport })} data-testid="maintenance-import-submit" />
            {resultFor(results.import, "import", t)}
            <InlineError error={actionError?.action === "import" ? actionError.error : null} t={t} action="import" actionError={actionError} />
          </section>
          <section className={styles.operation} aria-labelledby="maintenance-projection-heading" data-testid="maintenance-projection" aria-busy={(["run", "rebuild", "cleanup", "vacuum"] as const).includes(pendingAction as "run" | "rebuild" | "cleanup" | "vacuum")}>
            <h3 id="maintenance-projection-heading">{t("projectionMaintenanceHeading")}</h3>
            <p className={styles.muted}>{t("projectionMaintenanceDescription")}</p>
            <label className={styles.field} htmlFor="maintenance-owner">
              <span>{t("maintenanceOwnerLabel")}</span>
              <input id="maintenance-owner" name="maintenance-owner" autoComplete="off" translate="no" value={maintenanceOwner} onChange={(event) => setMaintenanceOwner(event.currentTarget.value)} placeholder={runtime.actor} data-testid="maintenance-owner" />
            </label>
            <div className={styles.buttonRow}>
              <Button label={t("runMaintenanceAction")} variant="secondary" isDisabled={isBusy} isLoading={pendingAction === "run"} onClick={() => openConfirm({ kind: "run" })} data-testid="maintenance-run-submit" />
              <Button label={t("rebuildAction")} variant="destructive" isDisabled={isBusy} isLoading={pendingAction === "rebuild"} onClick={() => openConfirm({ kind: "rebuild" })} data-testid="maintenance-rebuild-submit" />
              <Button label={t("cleanupAction")} variant="destructive" isDisabled={isBusy} isLoading={pendingAction === "cleanup"} onClick={() => openConfirm({ kind: "cleanup" })} data-testid="maintenance-cleanup-submit" />
              <Button label={t("vacuumAction")} variant="destructive" isDisabled={isBusy} isLoading={pendingAction === "vacuum"} onClick={() => openConfirm({ kind: "vacuum" })} data-testid="maintenance-vacuum-submit" />
            </div>
            {resultFor(results.run, "run", t)}
            {resultFor(results.rebuild, "rebuild", t)}
            {resultFor(results.cleanup, "cleanup", t)}
            {resultFor(results.vacuum, "vacuum", t)}
            {(["run", "rebuild", "cleanup", "vacuum"] as const).map((action) => <InlineError key={action} error={actionError?.action === action ? actionError.error : null} t={t} action={action} actionError={actionError} />)}
          </section>
          <section className={styles.operation} aria-labelledby="maintenance-checkpoint-heading" data-testid="maintenance-checkpoint" aria-busy={pendingAction === "checkpoint"}>
            <h3 id="maintenance-checkpoint-heading">{t("checkpointHeading")}</h3>
            <p className={styles.muted}>{t("checkpointDescription")}</p>
            <Button label={t("checkpointAction")} variant="secondary" isDisabled={isBusy} isLoading={pendingAction === "checkpoint"} onClick={() => openConfirm({ kind: "checkpoint" })} data-testid="maintenance-checkpoint-submit" />
            {resultFor(results.checkpoint, "checkpoint", t)}
            <InlineError error={actionError?.action === "checkpoint" ? actionError.error : null} t={t} action="checkpoint" actionError={actionError} />
          </section>
          <section className={styles.unsupported} aria-labelledby="maintenance-legacy-import-heading" data-testid="maintenance-legacy-import-unsupported">
            <h3 id="maintenance-legacy-import-heading">{t("legacyImportHeading")}</h3>
            <p>{t("legacyImportUnsupported")}</p>
          </section>
        </div>
        {resultFor(results.backup, "backup", t)}
        {resultFor(results.export, "export", t)}
        <InlineError error={actionError?.action === "backup" ? actionError.error : null} t={t} action="backup" actionError={actionError} />
        <InlineError error={actionError?.action === "export" ? actionError.error : null} t={t} action="export" actionError={actionError} />
      </section>

      <AlertDialog
        isOpen={confirm !== null}
        onOpenChange={(isOpen) => { if (!isOpen) { setConfirm(null); restoreConfirmFocus() } }}
        title={confirm ? confirmTitle(confirm, t) : t("maintenanceHeading")}
        description={confirm ? confirmDescription(confirm, t) : ""}
        cancelLabel={t("cancel")}
        actionLabel={t("continue")}
        actionVariant={confirm && isDestructive(confirm) ? "destructive" : "primary"}
        isActionLoading={pendingAction !== null}
        onAction={confirmAction}
        data-testid="maintenance-confirm-dialog"
      />
    </section>
  )
}

function Panel({ title, testId, children }: { title: string; testId: string; children: ReactNode }) {
  return <section className={styles.panel} aria-labelledby={`${testId}-heading`} data-testid={testId}><h2 id={`${testId}-heading`}>{title}</h2>{children}</section>
}

function PathOperation({ label, value, onChange, buttonLabel, disabled, loading, onClick, testId }: { label: string; value: string; onChange: (value: string) => void; buttonLabel: string; disabled: boolean; loading: boolean; onClick: () => void; testId: string }) {
  return <section className={styles.operation} aria-labelledby={`${testId}-heading`} data-testid={testId} aria-busy={loading}>
    <h3 id={`${testId}-heading`}>{label}</h3>
    <label className={styles.field} htmlFor={`${testId}-path`}><span>{label}</span><input id={`${testId}-path`} name={`${testId}-path`} autoComplete="off" translate="no" value={value} onChange={(event) => onChange(event.currentTarget.value)} data-testid={`${testId}-path`} /></label>
    <Button label={buttonLabel} variant="secondary" isDisabled={disabled} isLoading={loading} onClick={onClick} data-testid={`${testId}-submit`} />
  </section>
}

function StatusContent({ state, t, locale }: { state: LoadState<MaintenanceStatus>; t: ReturnType<typeof createTranslator>; locale: Locale }) {
  if (state.kind === "loading") return <Boundary text={t("loading")} />
  if (state.kind === "error") return <InlineError error={state.error} t={t} action="status" actionError={null} />
  const { owner } = state.value
  return <div className={styles.content}>
    <dl className={styles.metrics}>
      <Metric label={t("databaseInstance")} value={state.value.database_instance_id} />
      <Metric label={t("protocolVersion")} value={state.value.protocol_version} />
      <Metric label={t("owner")} value={owner.owner ?? t("noOwner")} />
      <Metric label={t("mode")} value={owner.mode ?? "—"} />
      <Metric label={t("active")} value={owner.active} tone={owner.active ? styles.ready : styles.mutedValue} />
      <Metric label={t("fenceEpoch")} value={owner.fence_epoch} />
      <Metric label={t("leaseExpiresAt")} value={formatTimestamp(owner.lease_expires_at, locale)} />
      <Metric label={t("buildIdentity")} value={owner.build_identity} />
      <Metric label={t("lastHeartbeat")} value={formatTimestamp(owner.last_heartbeat_at, locale)} />
    </dl>
    <p className={styles.muted}>{t("projectionStores")}: {state.value.stores.length}</p>
    {state.value.stores.length > 0 ? <div className={styles.storeList}>{state.value.stores.map((store) => <div className={styles.store} key={store.store_name}>
      <div className={styles.storeHeading}><strong translate="no">{store.store_name}</strong>{store.degraded || store.dirty ? <span className={statusTone(store)}>{t("degraded")}</span> : <span className={statusTone(store)} translate="no">{reported(store.lifecycle_status)}</span>}</div>
      <dl className={styles.detailGrid}><Metric label={t("activeGeneration")} value={store.active_generation} /><Metric label={t("activeFingerprint")} value={store.active_fingerprint} /><Metric label={t("previousGeneration")} value={store.previous_generation} /><Metric label={t("buildingGeneration")} value={store.building_generation} /><Metric label={t("storeFenceEpoch")} value={store.fence_epoch} /><Metric label={t("storeLastEvent")} value={store.last_event_id} /><Metric label={t("pending")} value={store.pending} /><Metric label={t("running")} value={store.running} /><Metric label={t("failed")} value={store.failed} /><Metric label={t("phase")} value={store.phase} /><Metric label={t("updatedAt")} value={formatTimestamp(store.updated_at, locale)} /><Metric label={t("lastError")} value={store.last_error ? t("errorPresent") : t("none")} /><Metric label={t("storeErrors")} value={errorSummary(store.errors.length, t)} /></dl>
    </div>)}</div> : <p className={styles.empty}>{t("noProjectionStores")}</p>}
  </div>
}

function StatsContent({ state, t, locale }: { state: LoadState<QueueStats>; t: ReturnType<typeof createTranslator>; locale: Locale }) {
  if (state.kind === "loading") return <Boundary text={t("loading")} />
  if (state.kind === "error") return <InlineError error={state.error} t={t} action="stats" actionError={null} />
  return <div className={styles.content}><dl className={styles.metrics}><Metric label={t("boardId")} value={state.value.board_id} /><Metric label={t("generatedAt")} value={formatTimestamp(state.value.generated_at, locale)} /><Metric label={t("unplannedActiveTasks")} value={state.value.unplanned_active_tasks} /><Metric label={t("incompleteRequiredSteps")} value={state.value.active_parents_with_incomplete_required_steps} /></dl><h3 className={styles.subheading}>{t("statusCounts")}</h3>{state.value.status_counts.length > 0 ? <dl className={styles.detailGrid}>{state.value.status_counts.map((entry) => <Metric key={entry.status} label={entry.status} labelTranslateNo value={entry.count} />)}</dl> : <p className={styles.empty}>{t("noStatusCounts")}</p>}<h3 className={styles.subheading}>{t("staleClaims")}</h3>{state.value.stale_claims.length > 0 ? <div className={styles.storeList}>{state.value.stale_claims.map((claim) => <div className={styles.store} key={claim.task_id}><div className={styles.storeHeading}><strong translate="no">#{claim.seq} {claim.title}</strong><span translate="no">{claim.claim_owner ?? t("noOwner")}</span></div><dl className={styles.detailGrid}><Metric label={t("expiresAt")} value={formatTimestamp(claim.claim_expires_at, locale)} /><Metric label={t("lastHeartbeat")} value={formatTimestamp(claim.last_heartbeat_at, locale)} /><Metric label={t("run") } value={claim.current_run_id} /><Metric label={t("retry")} value={`${claim.retry_count}/${claim.max_retries ?? "—"}`} /></dl></div>)}</div> : <p className={styles.empty}>{t("noStaleClaims")}</p>}<h3 className={styles.subheading}>{t("blockedReasons")}</h3>{state.value.blocked_reasons.length > 0 ? <dl className={styles.detailGrid}>{state.value.blocked_reasons.map((entry) => <Metric key={entry.reason} label={entry.reason || t("unspecified")} labelTranslateNo value={entry.count} />)}</dl> : <p className={styles.empty}>{t("noBlockedReasons")}</p>}</div>
}

function SearchContent({ state, t }: { state: LoadState<SearchStatus>; t: ReturnType<typeof createTranslator> }) {
  if (state.kind === "loading") return <Boundary text={t("loading")} />
  if (state.kind === "error") return <InlineError error={state.error} t={t} action="search" actionError={null} />
  return <dl className={styles.detailGrid}><Metric label={t("backend")} value={state.value.backend} /><Metric label={t("derivedIndex")} value={state.value.derived_index} /><Metric label={t("stale")} value={state.value.stale} tone={state.value.stale ? styles.degraded : styles.ready} /><Metric label={t("generation")} value={state.value.generation} /><Metric label={t("lastEvent")} value={state.value.last_event_id} /><Metric label={t("lagEvents")} value={state.value.index_lag_events} /><Metric label={t("message")} value={diagnosticSummary(state.value.message, t)} /></dl>
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
  return <div className={styles.content}><p className={report.ok ? styles.ready : styles.degraded} data-testid="maintenance-doctor-result">{report.ok ? t("doctorOk") : t("doctorFindings")}</p><dl className={styles.detailGrid}>{findings.map(([label, value]) => <Metric key={label} label={label} value={value} />)}</dl><h3 className={styles.subheading}>{t("derivedStores")}</h3>{report.derived_stores.length > 0 ? <div className={styles.storeList}>{report.derived_stores.map((store) => <div className={styles.store} key={store.store_name}><div className={styles.storeHeading}><strong translate="no">{store.store_name}</strong><span>{store.dirty ? t("degraded") : t("ready")}</span></div><dl className={styles.detailGrid}><Metric label={t("schemaVersion")} value={store.schema_version} /><Metric label={t("storeLastEvent")} value={store.last_event_id} /><Metric label={t("pendingOutbox")} value={store.pending_outbox} /><Metric label={t("runningOutbox")} value={store.running_outbox} /><Metric label={t("failedOutbox")} value={store.failed_outbox} /><Metric label={t("lastError")} value={store.last_error ? t("errorPresent") : t("none")} /></dl></div>)}</div> : <p className={styles.empty}>{t("noDerivedStores")}</p>}</div>
}

function Metric({ label, value, tone, labelTranslateNo = false }: { label: string; value: unknown; tone?: string; labelTranslateNo?: boolean }) {
  return <div className={styles.metric}><dt translate={labelTranslateNo ? "no" : undefined}>{label}</dt><dd className={tone ?? styles.value} translate="no">{reported(value as string | number | boolean | null | undefined)}</dd></div>
}

function Boundary({ text }: { text: string }) { return <div className={styles.boundary} role="status" aria-live="polite">{text}</div> }

function InlineError({ error, t, action, actionError }: { error: unknown; t: ReturnType<typeof createTranslator>; action: string; actionError: { action: string; error: unknown } | null }) {
  if (error === null || error === undefined || (actionError !== null && actionError.action !== action)) return null
  return <div className={styles.error} role="alert" data-testid={`maintenance-${action}-error`}><strong>{t("maintenanceActionFailed")}</strong><span>{safeErrorText(error, t)}</span></div>
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
  return <div className={styles.evidence} data-testid={testId} aria-live="polite"><strong>{title}</strong><dl>{rows.map(([label, value]) => <Metric key={label} label={label} value={value} />)}</dl></div>
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
  const descriptions: Record<Exclude<ConfirmAction["kind"], "backup" | "export" | "import">, string> = { checkpoint: t("confirmCheckpointDescription"), run: t("confirmRunDescription"), rebuild: t("confirmRebuildDescription"), cleanup: t("confirmCleanupDescription"), vacuum: t("confirmVacuumDescription") }
  return descriptions[action.kind]
}

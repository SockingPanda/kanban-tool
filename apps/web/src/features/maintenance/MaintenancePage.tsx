import { AlertDialog } from "@astryxdesign/core/AlertDialog"
import { Button } from "@astryxdesign/core/Button"
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react"

import type { WebRuntimeConfig } from "../../lib/runtime"
import { createTranslator } from "../../lib/i18n"
import { requestHealthRefresh } from "../../lib/health-refresh"
import { usePreferences } from "../../lib/use-preferences"
import {
  createMaintenanceApi,
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

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message
  return String(error)
}

function reported(value: string | number | boolean | null | undefined): string {
  if (value === null || value === undefined) return "—"
  if (typeof value === "string") return value.trim() || "—"
  return String(value)
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
  const [confirm, setConfirm] = useState<ConfirmAction | null>(null)
  const statusRequestRef = useRef<Promise<void> | null>(null)
  const initialLoadRef = useRef(false)

  const loadStatus = useCallback(async (silent = false) => {
    if (statusRequestRef.current) return statusRequestRef.current
    const request = (async () => {
      if (!silent && status.kind !== "ready") setStatus({ kind: "loading" })
      try {
        setStatus({ kind: "ready", value: await api.status() })
      } catch (error) {
        if (!silent || status.kind !== "ready") setStatus({ kind: "error", error })
      } finally {
        statusRequestRef.current = null
      }
    })()
    statusRequestRef.current = request
    return request
  }, [api, status.kind])

  const loadBoardDiagnostics = useCallback(async () => {
    const [statsResult, searchResult] = await Promise.allSettled([api.stats(boardSlug), api.searchStatus(boardSlug)])
    if (statsResult.status === "fulfilled") setStats({ kind: "ready", value: statsResult.value })
    else setStats({ kind: "error", error: statsResult.reason })
    if (searchResult.status === "fulfilled") setSearchStatus({ kind: "ready", value: searchResult.value })
    else setSearchStatus({ kind: "error", error: searchResult.reason })
  }, [api, boardSlug])

  useEffect(() => {
    if (initialLoadRef.current) return
    initialLoadRef.current = true
    void Promise.all([loadStatus(), loadBoardDiagnostics()])
  }, [loadBoardDiagnostics, loadStatus])

  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState !== "hidden") void loadStatus(true)
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

  const settleMutation = useCallback(async <T extends Result>(action: ResultKey, work: () => Promise<T>, onSuccess?: (value: T) => void) => {
    if (pendingAction !== null) return
    setPendingAction(action)
    setActionError(null)
    try {
      const value = await work()
      setResults((previous) => ({ ...previous, [action]: value }))
      onSuccess?.(value)
      await loadStatus()
      const refreshHealth = onHealthRefresh ?? requestHealthRefresh
      refreshHealth()
    } catch (error) {
      setActionError({ action, error })
    } finally {
      setPendingAction(null)
    }
  }, [loadStatus, onHealthRefresh, pendingAction])

  const runDoctor = () => {
    if (pendingAction !== null) return
    setPendingAction("doctor")
    setActionError(null)
    void api.doctor()
      .then((value) => setDoctor({ kind: "ready", value }))
      .catch((error: unknown) => {
        setDoctor({ kind: "error", error })
        setActionError({ action: "doctor", error })
      })
      .finally(() => setPendingAction(null))
  }

  const confirmAction = () => {
    const action = confirm
    setConfirm(null)
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

  return (
    <section className={styles.page} aria-labelledby="maintenance-heading" data-testid="maintenance-page">
      <div className={styles.headingRow}>
        <div className={styles.pageHeading}>
          <p className={styles.eyebrow}>{t("productKicker")}</p>
          <h1 id="maintenance-heading">{t("maintenanceHeading")}</h1>
          <p className={styles.lede}>{t("maintenanceDescription")}</p>
        </div>
        <Button label={t("refresh")} variant="secondary" isDisabled={isBusy || status.kind === "loading"} isLoading={status.kind === "loading"} onClick={() => { void Promise.all([loadStatus(), loadBoardDiagnostics()]) }} data-testid="maintenance-refresh" />
      </div>

      {status.kind === "loading" && stats.kind === "loading" && searchStatus.kind === "loading" ? (
        <div className={styles.boundary} role="status" aria-live="polite" data-testid="maintenance-loading">{t("loading")}</div>
      ) : null}

      <div className={styles.overviewGrid}>
        <Panel title={t("maintenanceStatusHeading")} testId="maintenance-status">
          <StatusContent state={status} t={t} />
        </Panel>
        <Panel title={t("statsHeading")} testId="maintenance-stats">
          <StatsContent state={stats} t={t} />
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

      <section className={styles.operations} aria-labelledby="maintenance-operations-heading">
        <div className={styles.sectionHeading}>
          <div>
            <p className={styles.eyebrow}>{t("hostAdministration")}</p>
            <h2 id="maintenance-operations-heading">{t("maintenanceOperationsHeading")}</h2>
          </div>
          <span className={styles.scope}>{boardSlug}</span>
        </div>
        <div className={styles.operationGrid}>
          <PathOperation label={t("backupPathLabel")} value={backupPath} onChange={setBackupPath} buttonLabel={t("backupAction")} disabled={isBusy || !backupPath.trim()} onClick={() => setConfirm({ kind: "backup", path: backupPath.trim() })} testId="maintenance-backup" />
          <PathOperation label={t("exportPathLabel")} value={exportPath} onChange={setExportPath} buttonLabel={t("exportAction")} disabled={isBusy || !exportPath.trim()} onClick={() => setConfirm({ kind: "export", path: exportPath.trim() })} testId="maintenance-export" />
          <section className={styles.operation} aria-labelledby="maintenance-import-heading" data-testid="maintenance-import">
            <h3 id="maintenance-import-heading">{t("portableImportHeading")}</h3>
            <label className={styles.field} htmlFor="maintenance-import-path">
              <span>{t("importPathLabel")}</span>
              <input id="maintenance-import-path" value={importPath} onChange={(event) => setImportPath(event.currentTarget.value)} placeholder={t("importPathPlaceholder")} data-testid="maintenance-import-path" />
            </label>
            <label className={styles.checkbox}>
              <input type="checkbox" checked={replaceImport} onChange={(event) => setReplaceImport(event.currentTarget.checked)} />
              <span>{t("replaceImportLabel")}</span>
            </label>
            <Button label={replaceImport ? t("replaceImportAction") : t("importAction")} variant={replaceImport ? "destructive" : "secondary"} isDisabled={isBusy || !importPath.trim()} onClick={() => setConfirm({ kind: "import", path: importPath.trim(), replace: replaceImport })} data-testid="maintenance-import-submit" />
            {resultFor(results.import, "import", t)}
            <InlineError error={actionError?.action === "import" ? actionError.error : null} t={t} action="import" actionError={actionError} />
          </section>
          <section className={styles.operation} aria-labelledby="maintenance-projection-heading" data-testid="maintenance-projection">
            <h3 id="maintenance-projection-heading">{t("projectionMaintenanceHeading")}</h3>
            <p className={styles.muted}>{t("projectionMaintenanceDescription")}</p>
            <label className={styles.field} htmlFor="maintenance-owner">
              <span>{t("maintenanceOwnerLabel")}</span>
              <input id="maintenance-owner" value={maintenanceOwner} onChange={(event) => setMaintenanceOwner(event.currentTarget.value)} placeholder={runtime.actor} data-testid="maintenance-owner" />
            </label>
            <div className={styles.buttonRow}>
              <Button label={t("runMaintenanceAction")} variant="secondary" isDisabled={isBusy} onClick={() => setConfirm({ kind: "run" })} data-testid="maintenance-run-submit" />
              <Button label={t("rebuildAction")} variant="destructive" isDisabled={isBusy} onClick={() => setConfirm({ kind: "rebuild" })} data-testid="maintenance-rebuild-submit" />
              <Button label={t("cleanupAction")} variant="destructive" isDisabled={isBusy} onClick={() => setConfirm({ kind: "cleanup" })} data-testid="maintenance-cleanup-submit" />
              <Button label={t("vacuumAction")} variant="destructive" isDisabled={isBusy} onClick={() => setConfirm({ kind: "vacuum" })} data-testid="maintenance-vacuum-submit" />
            </div>
            {resultFor(results.run, "run", t)}
            {resultFor(results.rebuild, "rebuild", t)}
            {resultFor(results.cleanup, "cleanup", t)}
            {resultFor(results.vacuum, "vacuum", t)}
            {(["run", "rebuild", "cleanup", "vacuum"] as const).map((action) => <InlineError key={action} error={actionError?.action === action ? actionError.error : null} t={t} action={action} actionError={actionError} />)}
          </section>
          <section className={styles.operation} aria-labelledby="maintenance-checkpoint-heading" data-testid="maintenance-checkpoint">
            <h3 id="maintenance-checkpoint-heading">{t("checkpointHeading")}</h3>
            <p className={styles.muted}>{t("checkpointDescription")}</p>
            <Button label={t("checkpointAction")} variant="secondary" isDisabled={isBusy} onClick={() => setConfirm({ kind: "checkpoint" })} data-testid="maintenance-checkpoint-submit" />
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

      {confirm ? <AlertDialog
        isOpen
        onOpenChange={(isOpen) => { if (!isOpen) setConfirm(null) }}
        title={confirmTitle(confirm, t)}
        description={confirmDescription(confirm, t)}
        cancelLabel={t("cancel")}
        actionLabel={t("continue")}
        actionVariant={isDestructive(confirm) ? "destructive" : "primary"}
        isActionLoading={pendingAction !== null}
        onAction={confirmAction}
        data-testid="maintenance-confirm-dialog"
      /> : null}
    </section>
  )
}

function Panel({ title, testId, children }: { title: string; testId: string; children: ReactNode }) {
  return <section className={styles.panel} aria-labelledby={`${testId}-heading`} data-testid={testId}><h2 id={`${testId}-heading`}>{title}</h2>{children}</section>
}

function PathOperation({ label, value, onChange, buttonLabel, disabled, onClick, testId }: { label: string; value: string; onChange: (value: string) => void; buttonLabel: string; disabled: boolean; onClick: () => void; testId: string }) {
  return <section className={styles.operation} aria-labelledby={`${testId}-heading`} data-testid={testId}>
    <h3 id={`${testId}-heading`}>{label}</h3>
    <label className={styles.field} htmlFor={`${testId}-path`}><span>{label}</span><input id={`${testId}-path`} value={value} onChange={(event) => onChange(event.currentTarget.value)} data-testid={`${testId}-path`} /></label>
    <Button label={buttonLabel} variant="secondary" isDisabled={disabled} onClick={onClick} data-testid={`${testId}-submit`} />
  </section>
}

function StatusContent({ state, t }: { state: LoadState<MaintenanceStatus>; t: ReturnType<typeof createTranslator> }) {
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
    </dl>
    <p className={styles.muted}>{t("projectionStores")}: {state.value.stores.length}</p>
    {state.value.stores.length > 0 ? <div className={styles.storeList}>{state.value.stores.map((store) => <div className={styles.store} key={store.store_name}>
      <div className={styles.storeHeading}><strong>{store.store_name}</strong><span className={statusTone(store)}>{store.degraded || store.dirty ? t("degraded") : reported(store.lifecycle_status)}</span></div>
      <dl className={styles.detailGrid}><Metric label={t("activeGeneration")} value={store.active_generation} /><Metric label={t("activeFingerprint")} value={store.active_fingerprint} /><Metric label={t("pending")} value={store.pending} /><Metric label={t("running")} value={store.running} /><Metric label={t("failed")} value={store.failed} /><Metric label={t("phase")} value={store.phase} /><Metric label={t("lastError")} value={store.last_error} /></dl>
    </div>)}</div> : <p className={styles.empty}>{t("noProjectionStores")}</p>}
  </div>
}

function StatsContent({ state, t }: { state: LoadState<QueueStats>; t: ReturnType<typeof createTranslator> }) {
  if (state.kind === "loading") return <Boundary text={t("loading")} />
  if (state.kind === "error") return <InlineError error={state.error} t={t} action="stats" actionError={null} />
  return <div className={styles.content}><dl className={styles.metrics}><Metric label={t("boardId")} value={state.value.board_id} /><Metric label={t("generatedAt")} value={state.value.generated_at} /><Metric label={t("unplannedActiveTasks")} value={state.value.unplanned_active_tasks} /><Metric label={t("incompleteRequiredSteps")} value={state.value.active_parents_with_incomplete_required_steps} /></dl><h3 className={styles.subheading}>{t("statusCounts")}</h3>{state.value.status_counts.length > 0 ? <dl className={styles.detailGrid}>{state.value.status_counts.map((entry) => <Metric key={entry.status} label={entry.status} value={entry.count} />)}</dl> : <p className={styles.empty}>{t("noStatusCounts")}</p>}<h3 className={styles.subheading}>{t("blockedReasons")}</h3>{state.value.blocked_reasons.length > 0 ? <dl className={styles.detailGrid}>{state.value.blocked_reasons.map((entry) => <Metric key={entry.reason} label={entry.reason || t("unspecified")} value={entry.count} />)}</dl> : <p className={styles.empty}>{t("noBlockedReasons")}</p>}</div>
}

function SearchContent({ state, t }: { state: LoadState<SearchStatus>; t: ReturnType<typeof createTranslator> }) {
  if (state.kind === "loading") return <Boundary text={t("loading")} />
  if (state.kind === "error") return <InlineError error={state.error} t={t} action="search" actionError={null} />
  return <dl className={styles.detailGrid}><Metric label={t("backend")} value={state.value.backend} /><Metric label={t("derivedIndex")} value={state.value.derived_index} /><Metric label={t("stale")} value={state.value.stale} tone={state.value.stale ? styles.degraded : styles.ready} /><Metric label={t("generation")} value={state.value.generation} /><Metric label={t("lastEvent")} value={state.value.last_event_id} /><Metric label={t("lagEvents")} value={state.value.index_lag_events} /><Metric label={t("message")} value={state.value.message} /></dl>
}

function DoctorContent({ report, t }: { report: DoctorReport; t: ReturnType<typeof createTranslator> }) {
  const findings = [
    [t("integrity"), report.integrity_check],
    [t("migrationVersion"), report.migration_version],
    [t("userVersion"), report.user_version],
    [t("dependencyCycles"), report.dependency_cycles],
    [t("missingRunLogs"), report.missing_run_logs],
    [t("outboxFailed"), report.outbox_failed],
    [t("dirtyStores"), report.derived_dirty_stores],
    [t("errorStores"), report.derived_error_stores],
    [t("consistencyErrors"), report.consistency_errors],
    [t("ontologyErrors"), report.ontology_ledger_errors],
  ] as const
  return <div className={styles.content}><p className={report.ok ? styles.ready : styles.degraded} data-testid="maintenance-doctor-result">{report.ok ? t("doctorOk") : t("doctorFindings")}</p><dl className={styles.detailGrid}>{findings.map(([label, value]) => <Metric key={label} label={label} value={value} />)}</dl></div>
}

function Metric({ label, value, tone }: { label: string; value: unknown; tone?: string }) {
  return <div className={styles.metric}><dt>{label}</dt><dd className={tone ?? styles.value} translate="no">{reported(value as string | number | boolean | null | undefined)}</dd></div>
}

function Boundary({ text }: { text: string }) { return <div className={styles.boundary} role="status" aria-live="polite">{text}</div> }

function InlineError({ error, t, action, actionError }: { error: unknown; t: ReturnType<typeof createTranslator>; action: string; actionError: { action: string; error: unknown } | null }) {
  if (error === null || error === undefined || (actionError !== null && actionError.action !== action)) return null
  return <div className={styles.error} role="alert" data-testid={`maintenance-${action}-error`}><strong>{t("maintenanceActionFailed")}</strong><span>{errorText(error)}</span></div>
}

function resultFor(result: Result | undefined, key: ResultKey, t: ReturnType<typeof createTranslator>): ReactNode {
  if (!result) return null
  if (key === "backup" && "out_path" in result && "checksum_sha256" in result) return <Evidence key={key} testId="maintenance-backup-result" title={t("backupComplete")} rows={[[t("serverPath"), result.out_path], [t("checksum"), result.checksum_sha256], [t("bytes"), result.bytes], [t("sourceFingerprint"), result.source_fingerprint]]} />
  if (key === "export" && "out_path" in result && "checksum_sha256" in result && "record_count" in result) return <Evidence key={key} testId="maintenance-export-result" title={t("exportComplete")} rows={[[t("serverPath"), result.out_path], [t("checksum"), result.checksum_sha256], [t("bytes"), result.bytes], [t("recordCount"), result.record_count], [t("sourceFingerprint"), result.source_fingerprint]]} />
  if (key === "import" && "in_path" in result) return <Evidence key={key} testId="maintenance-import-result" title={t("importComplete")} rows={[[t("serverPath"), result.in_path], [t("sourceFingerprint"), result.source_fingerprint], [t("importedRecords"), result.imported_records], [t("skippedRecords"), result.skipped_records], [t("rebuildJobs"), result.rebuild_jobs_enqueued], [t("journal"), result.journal_id], [t("phase"), result.phase], [t("restartRequired"), result.restart_required], [t("stagedDatabasePath"), result.staged_database_path], [t("targetFingerprintBefore"), result.target_fingerprint_before], [t("stagedFingerprint"), result.staged_fingerprint], [t("publishPreconditions"), result.publish_preconditions.length > 0 ? result.publish_preconditions.join("; ") : "—"]]} />
  if (key === "checkpoint" && "checkpointed_frames" in result) return <Evidence key={key} testId="maintenance-checkpoint-result" title={t("checkpointComplete")} rows={[[t("busy"), result.busy], [t("logFrames"), result.log_frames], [t("checkpointedFrames"), result.checkpointed_frames]]} />
  if (key === "vacuum" && "before_bytes" in result) return <Evidence key={key} testId="maintenance-vacuum-result" title={t("vacuumComplete")} rows={[[t("status"), result.ok], [t("beforeBytes"), result.before_bytes], [t("afterBytes"), result.after_bytes], [t("sourceFingerprint"), result.source_fingerprint]]} />
  if ((key === "run" || key === "rebuild" || key === "cleanup") && "action" in result) return <Evidence key={key} testId={`maintenance-${key}-result`} title={t("maintenanceComplete")} rows={[[t("action"), result.action], [t("owner"), result.owner], [t("processed"), result.processed], [t("phase"), result.phase], [t("degraded"), result.degraded], [t("errors"), result.errors.length > 0 ? result.errors.join("; ") : "—"]]} />
  return null
}

function Evidence({ title, rows, testId }: { title: string; rows: Array<[string, unknown]>; testId: string }) {
  return <div className={styles.evidence} data-testid={testId}><strong>{title}</strong><dl>{rows.map(([label, value]) => <Metric key={label} label={label} value={value} />)}</dl></div>
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

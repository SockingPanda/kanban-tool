import { useMutation, useQuery } from "@tanstack/react-query"
import { Activity, DatabaseBackup, Download, RefreshCcw, SearchCheck, Server, Stethoscope, Wrench } from "lucide-react"
import { useState, type ElementType, type ReactNode } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import { Card } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { MetricStrip, SectionCard } from "@/components/ui/composites"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Input } from "@/components/ui/input"
import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Skeleton } from "@/components/ui/skeleton"
import { useI18n } from "@/i18n"
import type {
  BackupReport,
  BoardStats,
  CheckpointReport,
  DoctorDerivedStore,
  DoctorReport,
  ExportReport,
  ImportReport,
  KanbanApi,
  LegacyImportReport,
  MaintenanceRunReport,
  MaintenanceStatusReport,
  ProjectionStoreStatus,
  SearchIndexStatus,
  StaleClaim,
  VacuumReport,
} from "@/lib/api"
import { presentApiError } from "@/lib/api/error-presentation"
import { queryKeys } from "@/lib/query-keys"

export function MaintenanceView({ api }: { api: KanbanApi | null }) {
  const { t } = useI18n()
  const [backupPath, setBackupPath] = useState("")
  const [exportPath, setExportPath] = useState("")
  const [importPath, setImportPath] = useState("")
  const [legacyImportPath, setLegacyImportPath] = useState("")
  const [attachmentRoot, setAttachmentRoot] = useState("")
  const [owner, setOwner] = useState("")
  const [replaceImport, setReplaceImport] = useState(false)
  const statsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.stats(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.stats({ signal })
    },
  })
  const searchStatusQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.searchStatus(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.searchStatus({ signal })
    },
  })
  const maintenanceStatusQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.maintenanceStatus(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.maintenanceStatus({ signal })
    },
  })
  const doctorMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.doctor()
    },
  })
  const checkpointMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.checkpoint()
    },
  })
  const backupMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.backup(backupPath.trim())
    },
  })
  const exportMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.exportData(exportPath.trim())
    },
  })
  const importMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.importData(importPath.trim(), replaceImport)
    },
    onSuccess: () => void maintenanceStatusQuery.refetch(),
  })
  const legacyImportMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.importLegacySqliteV30(legacyImportPath.trim(), attachmentRoot.trim() || null)
    },
    onSuccess: () => void maintenanceStatusQuery.refetch(),
  })
  const vacuumMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.vacuum()
    },
    onSuccess: () => void maintenanceStatusQuery.refetch(),
  })
  const runMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.maintenanceRun(owner.trim() || null, "run")
    },
    onSuccess: () => void maintenanceStatusQuery.refetch(),
  })
  const rebuildMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.maintenanceRebuild(owner.trim() || null)
    },
    onSuccess: () => void maintenanceStatusQuery.refetch(),
  })
  const cleanupMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.maintenanceCleanup(owner.trim() || null)
    },
    onSuccess: () => void maintenanceStatusQuery.refetch(),
  })

  return (
    <ScrollArea className="flex-1 bg-card p-4">
      {!api ? (
        <Alert className="mb-4 border-destructive/50">
          <AlertTitle className="text-destructive">{t("Server unavailable. Start or check kanban serve.")}</AlertTitle>
          <AlertDescription>{t("Host maintenance actions remain disabled until the server is connected.")}</AlertDescription>
        </Alert>
      ) : null}
      <div className="grid grid-cols-2 gap-4">
        <Panel title={t("Stats")} icon={Activity}>
          <StatsGrid stats={statsQuery.data} />
          {statsQuery.error ? <ErrorText error={statsQuery.error} /> : null}
        </Panel>
        <Panel title={t("Search status")} icon={SearchCheck}>
          <SearchStatus meta={searchStatusQuery.data} />
          {searchStatusQuery.error ? <ErrorText error={searchStatusQuery.error} /> : null}
        </Panel>
        <Panel title={t("Doctor")} icon={Stethoscope}>
          <Button variant="secondary" disabled={!api || doctorMutation.isPending} onClick={() => doctorMutation.mutate()}>
            {t("Run doctor")}
          </Button>
          {doctorMutation.data ? <DoctorReportView report={doctorMutation.data} /> : null}
          {doctorMutation.error ? <ErrorText error={doctorMutation.error} /> : null}
        </Panel>
        <Panel title={t("Checkpoint")} icon={DatabaseBackup}>
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="secondary" disabled={!api || checkpointMutation.isPending}>
                {t("Create checkpoint")}
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>{t("Run WAL checkpoint?")}</AlertDialogTitle>
                <AlertDialogDescription>{t("Run WAL checkpoint now?")}</AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>{t("Cancel")}</AlertDialogCancel>
                <AlertDialogAction onClick={() => checkpointMutation.mutate()}>{t("Continue")}</AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
          {checkpointMutation.data ? <CheckpointResultView result={checkpointMutation.data} /> : null}
          {checkpointMutation.error ? <ErrorText error={checkpointMutation.error} /> : null}
        </Panel>
        <Panel
          title={t("Maintenance status")}
          icon={Server}
          actions={<Button variant="ghost" size="sm" disabled={!api || maintenanceStatusQuery.isFetching} onClick={() => void maintenanceStatusQuery.refetch()}><RefreshCcw className="h-4 w-4" />{t("Refresh")}</Button>}
        >
          {maintenanceStatusQuery.data ? <MaintenanceStatusView report={maintenanceStatusQuery.data} /> : null}
          {!maintenanceStatusQuery.data && maintenanceStatusQuery.isLoading ? <Skeleton className="h-24" /> : null}
          {!maintenanceStatusQuery.data && !maintenanceStatusQuery.isLoading && !maintenanceStatusQuery.error ? <EmptyText>{t("No maintenance status returned.")}</EmptyText> : null}
          {maintenanceStatusQuery.error ? <ErrorText error={maintenanceStatusQuery.error} /> : null}
        </Panel>
        <Panel title={t("Host administration")} icon={Wrench}>
          <HostAdministration
            backupPath={backupPath}
            exportPath={exportPath}
            importPath={importPath}
            legacyImportPath={legacyImportPath}
            attachmentRoot={attachmentRoot}
            owner={owner}
            replaceImport={replaceImport}
            apiReady={Boolean(api)}
            backupMutation={backupMutation}
            exportMutation={exportMutation}
            importMutation={importMutation}
            legacyImportMutation={legacyImportMutation}
            vacuumMutation={vacuumMutation}
            runMutation={runMutation}
            rebuildMutation={rebuildMutation}
            cleanupMutation={cleanupMutation}
            onBackupPathChange={setBackupPath}
            onExportPathChange={setExportPath}
            onImportPathChange={setImportPath}
            onLegacyImportPathChange={setLegacyImportPath}
            onAttachmentRootChange={setAttachmentRoot}
            onOwnerChange={setOwner}
            onReplaceImportChange={setReplaceImport}
          />
        </Panel>
      </div>
    </ScrollArea>
  )
}

function Panel({ title, icon: Icon, actions, children }: { title: string; icon: ElementType; actions?: ReactNode; children: ReactNode }) {
  return <SectionCard title={title} icon={Icon} actions={actions}>{children}</SectionCard>
}

type MutationState<T> = {
  data: T | undefined
  error: unknown
  isPending: boolean
  mutate: () => void
}

type HostAdministrationProps = {
  backupPath: string
  exportPath: string
  importPath: string
  legacyImportPath: string
  attachmentRoot: string
  owner: string
  replaceImport: boolean
  apiReady: boolean
  backupMutation: MutationState<BackupReport>
  exportMutation: MutationState<ExportReport>
  importMutation: MutationState<ImportReport>
  legacyImportMutation: MutationState<LegacyImportReport>
  vacuumMutation: MutationState<VacuumReport>
  runMutation: MutationState<MaintenanceRunReport>
  rebuildMutation: MutationState<MaintenanceRunReport>
  cleanupMutation: MutationState<MaintenanceRunReport>
  onBackupPathChange: (value: string) => void
  onExportPathChange: (value: string) => void
  onImportPathChange: (value: string) => void
  onLegacyImportPathChange: (value: string) => void
  onAttachmentRootChange: (value: string) => void
  onOwnerChange: (value: string) => void
  onReplaceImportChange: (value: boolean) => void
}

function HostAdministration({
  backupPath,
  exportPath,
  importPath,
  legacyImportPath,
  attachmentRoot,
  owner,
  replaceImport,
  apiReady,
  backupMutation,
  exportMutation,
  importMutation,
  legacyImportMutation,
  vacuumMutation,
  runMutation,
  rebuildMutation,
  cleanupMutation,
  onBackupPathChange,
  onExportPathChange,
  onImportPathChange,
  onLegacyImportPathChange,
  onAttachmentRootChange,
  onOwnerChange,
  onReplaceImportChange,
}: HostAdministrationProps) {
  const { t } = useI18n()
  return (
    <div className="space-y-5 text-sm">
      <div className="grid gap-3 md:grid-cols-2">
        <AdminPathAction
          label={t("Backup output path")}
          value={backupPath}
          onChange={onBackupPathChange}
          buttonLabel={t("Backup database")}
          icon={DatabaseBackup}
          disabled={!apiReady || !backupPath.trim() || backupMutation.isPending}
          onSubmit={backupMutation.mutate}
        />
        <AdminPathAction
          label={t("Export output path")}
          value={exportPath}
          onChange={onExportPathChange}
          buttonLabel={t("Export portable data")}
          icon={Download}
          disabled={!apiReady || !exportPath.trim() || exportMutation.isPending}
          onSubmit={exportMutation.mutate}
        />
      </div>

      <div className="space-y-3 rounded-md border border-border p-3">
        <Subheading>{t("Portable import")}</Subheading>
        <label className="block space-y-1">
          <span className="text-xs text-muted-foreground">{t("Import source path")}</span>
          <Input value={importPath} onChange={(event) => onImportPathChange(event.target.value)} placeholder="/path/to/export.jsonl" />
        </label>
        <label className="flex items-center gap-2 text-xs text-muted-foreground">
          <Checkbox aria-label={t("Replace canonical data (requires restart)")} checked={replaceImport} onCheckedChange={(checked) => onReplaceImportChange(checked === true)} />
          <span>{t("Replace canonical data (requires restart)")}</span>
        </label>
        <ConfirmAction
          label={replaceImport ? t("Replace with portable import") : t("Import portable data")}
          title={replaceImport ? t("Replace canonical data?") : t("Import portable data?")}
          description={replaceImport ? t("This replaces canonical data and may require restarting kanban serve. Continue only after a verified backup.") : t("Import writes canonical data through kanban serve. Continue?")}
          disabled={!apiReady || !importPath.trim() || importMutation.isPending}
          destructive={replaceImport}
          onConfirm={importMutation.mutate}
        />
        {importMutation.data ? <ImportResultView report={importMutation.data} /> : null}
        {importMutation.error ? <ErrorText error={importMutation.error} /> : null}
      </div>

      <div className="space-y-3 rounded-md border border-border p-3">
        <Subheading>{t("Legacy SQLite v30 import")}</Subheading>
        <label className="block space-y-1">
          <span className="text-xs text-muted-foreground">{t("Import source path")}</span>
          <Input value={legacyImportPath} onChange={(event) => onLegacyImportPathChange(event.target.value)} placeholder="/path/to/legacy.sqlite" />
        </label>
        <label className="block space-y-1">
          <span className="text-xs text-muted-foreground">{t("Canonical attachment root (optional)")}</span>
          <Input value={attachmentRoot} onChange={(event) => onAttachmentRootChange(event.target.value)} placeholder="/path/to/attachments" />
        </label>
        <ConfirmAction
          label={t("Import legacy SQLite v30")}
          title={t("Run legacy SQLite v30 import?")}
          description={t("This host-admin import writes canonical data and can resume a journal. Continue only after checking the source path.")}
          disabled={!apiReady || !legacyImportPath.trim() || legacyImportMutation.isPending}
          destructive
          onConfirm={legacyImportMutation.mutate}
        />
        {legacyImportMutation.data ? <LegacyImportResultView report={legacyImportMutation.data} /> : null}
        {legacyImportMutation.error ? <ErrorText error={legacyImportMutation.error} /> : null}
      </div>

      <div className="space-y-3 rounded-md border border-border p-3">
        <Subheading>{t("Projection maintenance")}</Subheading>
        <label className="block space-y-1">
          <span className="text-xs text-muted-foreground">{t("Maintenance owner (optional)")}</span>
          <Input value={owner} onChange={(event) => onOwnerChange(event.target.value)} placeholder={t("Defaults to the desktop actor")} />
        </label>
        <div className="flex flex-wrap gap-2">
          <ConfirmAction
            label={t("Run maintenance")}
            title={t("Run maintenance now?")}
            description={t("This claims the host maintenance lease and may update projection stores. Continue?")}
            disabled={!apiReady || runMutation.isPending}
            onConfirm={runMutation.mutate}
          />
          <ConfirmAction
            label={t("Rebuild projections")}
            title={t("Rebuild projections now?")}
            description={t("Rebuild updates derived stores from canonical data. Continue?")}
            disabled={!apiReady || rebuildMutation.isPending}
            destructive
            onConfirm={rebuildMutation.mutate}
          />
          <ConfirmAction
            label={t("Cleanup projections")}
            title={t("Cleanup projections now?")}
            description={t("Cleanup removes retired projection artifacts through the host. Continue?")}
            disabled={!apiReady || cleanupMutation.isPending}
            destructive
            onConfirm={cleanupMutation.mutate}
          />
          <ConfirmAction
            label={t("Vacuum database")}
            title={t("Vacuum database now?")}
            description={t("Vacuum compacts the canonical database through kanban serve. Continue?")}
            disabled={!apiReady || vacuumMutation.isPending}
            destructive
            onConfirm={vacuumMutation.mutate}
          />
        </div>
        {runMutation.data ? <MaintenanceRunResultView report={runMutation.data} /> : null}
        {rebuildMutation.data ? <MaintenanceRunResultView report={rebuildMutation.data} /> : null}
        {cleanupMutation.data ? <MaintenanceRunResultView report={cleanupMutation.data} /> : null}
        {vacuumMutation.data ? <VacuumResultView report={vacuumMutation.data} /> : null}
        {runMutation.error ? <ErrorText error={runMutation.error} /> : null}
        {rebuildMutation.error ? <ErrorText error={rebuildMutation.error} /> : null}
        {cleanupMutation.error ? <ErrorText error={cleanupMutation.error} /> : null}
        {vacuumMutation.error ? <ErrorText error={vacuumMutation.error} /> : null}
      </div>

      {backupMutation.data ? <BackupResultView report={backupMutation.data} /> : null}
      {backupMutation.error ? <ErrorText error={backupMutation.error} /> : null}
      {exportMutation.data ? <ExportResultView report={exportMutation.data} /> : null}
      {exportMutation.error ? <ErrorText error={exportMutation.error} /> : null}
    </div>
  )
}

function AdminPathAction({
  label,
  value,
  onChange,
  buttonLabel,
  icon: Icon,
  disabled,
  onSubmit,
}: {
  label: string
  value: string
  onChange: (value: string) => void
  buttonLabel: string
  icon: ElementType
  disabled: boolean
  onSubmit: () => void
}) {
  return (
    <div className="space-y-2 rounded-md border border-border p-3">
      <label className="block space-y-1">
        <span className="text-xs text-muted-foreground">{label}</span>
        <Input value={value} onChange={(event) => onChange(event.target.value)} />
      </label>
      <Button variant="secondary" disabled={disabled} onClick={onSubmit}><Icon className="h-4 w-4" />{buttonLabel}</Button>
    </div>
  )
}

function ConfirmAction({
  label,
  title,
  description,
  disabled,
  destructive = false,
  onConfirm,
}: {
  label: string
  title: string
  description: string
  disabled: boolean
  destructive?: boolean
  onConfirm: () => void
}) {
  const { t } = useI18n()
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant={destructive ? "destructive" : "secondary"} disabled={disabled}>{label}</Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("Cancel")}</AlertDialogCancel>
          <AlertDialogAction variant={destructive ? "destructive" : "default"} onClick={onConfirm}>{t("Continue")}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

function MaintenanceStatusView({ report }: { report: MaintenanceStatusReport }) {
  const { t } = useI18n()
  const owner = report.owner
  return (
    <div className="space-y-4 text-sm">
      <MetricStrip
        className="grid gap-2 md:grid-cols-2"
        items={[
          { id: "database", label: t("database instance"), value: report.database_instance_id },
          { id: "protocol", label: t("protocol version"), value: String(report.protocol_version) },
          { id: "owner", label: t("owner"), value: owner.owner ?? t("no owner") },
          { id: "mode", label: t("mode"), value: owner.mode ?? "-" },
          { id: "active", label: t("active"), value: String(owner.active), tone: owner.active ? "ready" : "secondary" },
          { id: "fence", label: t("fence epoch"), value: String(owner.fence_epoch) },
        ]}
      />
      <div className="space-y-1 text-xs text-muted-foreground">
        <InfoRow label={t("lease expires") } value={nullableNumber(owner.lease_expires_at)} />
        <InfoRow label={t("build identity")} value={owner.build_identity ?? "-"} />
        <InfoRow label={t("last heartbeat")} value={nullableNumber(owner.last_heartbeat_at)} />
      </div>
      <div>
        <Subheading>{t("Projection stores")}</Subheading>
        <ProjectionStoreList stores={report.stores} />
      </div>
    </div>
  )
}

function ProjectionStoreList({ stores }: { stores: ProjectionStoreStatus[] }) {
  const { t } = useI18n()
  if (stores.length === 0) return <EmptyText>{t("No projection stores returned.")}</EmptyText>
  return (
    <div className="space-y-2">
      {stores.map((store) => {
        const degraded = store.dirty || Boolean(store.last_error) || /degraded|error|failed/i.test(store.lifecycle_status)
        return (
          <Card key={store.store_name} className="p-2">
            <div className="flex items-center justify-between gap-2">
              <span className="truncate font-medium">{store.store_name}</span>
              <Badge variant={degraded ? "blocked" : "ready"}>{degraded ? t("degraded") : store.lifecycle_status}</Badge>
            </div>
            <div className="mt-2 grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-muted-foreground">
              <span>{t("active generation")}: {store.active_generation ?? "-"}</span>
              <span>{t("active fingerprint")}: {store.active_fingerprint ?? "-"}</span>
              <span>{t("previous generation")}: {store.previous_generation ?? "-"}</span>
              <span>{t("building generation")}: {store.building_generation ?? "-"}</span>
              <span>{t("fence epoch")}: {store.fence_epoch}</span>
              <span>{t("last event")}: {store.last_event_id}</span>
              <span>{t("pending")}: {store.pending}</span>
              <span>{t("running")}: {store.running}</span>
              <span>{t("failed")}: {store.failed}</span>
              <span>{t("updated")}: {store.updated_at}</span>
              <span className="col-span-2 truncate">{t("last error")}: {store.last_error ?? "-"}</span>
            </div>
          </Card>
        )
      })}
    </div>
  )
}

function BackupResultView({ report }: { report: BackupReport }) {
  const { t } = useI18n()
  return <ResultView title={t("Backup complete")} rows={[
    [t("path"), report.out_path],
    [t("bytes"), String(report.bytes)],
    [t("checksum"), report.checksum_sha256],
    [t("source fingerprint"), report.source_fingerprint],
  ]} />
}

function ExportResultView({ report }: { report: ExportReport }) {
  const { t } = useI18n()
  return <ResultView title={t("Export complete")} rows={[
    [t("path"), report.out_path],
    [t("bytes"), String(report.bytes)],
    [t("record count"), String(report.record_count)],
    [t("checksum"), report.checksum_sha256],
    [t("source fingerprint"), report.source_fingerprint],
  ]} />
}

function ImportResultView({ report }: { report: ImportReport }) {
  const { t } = useI18n()
  return <ResultView title={t("Import complete")} rows={[
    [t("path"), report.in_path],
    [t("imported records"), String(report.imported_records)],
    [t("skipped records"), String(report.skipped_records)],
    [t("rebuild jobs"), String(report.rebuild_jobs_enqueued)],
    [t("journal"), report.journal_id],
  ]} />
}

function LegacyImportResultView({ report }: { report: LegacyImportReport }) {
  const { t } = useI18n()
  return <ResultView title={t("Legacy import complete")} rows={[
    [t("phase"), report.phase],
    [t("path"), report.source_path],
    [t("resumed"), String(report.resumed)],
    [t("attachments"), String(report.attachment_count)],
    [t("journal"), report.journal_id],
  ]} />
}

function MaintenanceRunResultView({ report }: { report: MaintenanceRunReport }) {
  const { t } = useI18n()
  return <ResultView title={t("Maintenance complete")} rows={[
    [t("action"), report.action],
    [t("owner"), report.owner],
    [t("mode"), report.mode],
    [t("processed"), String(report.processed)],
  ]} />
}

function VacuumResultView({ report }: { report: VacuumReport }) {
  const { t } = useI18n()
  return <ResultView title={t("Vacuum complete")} rows={[
    [t("status"), String(report.ok)],
    [t("before bytes"), String(report.before_bytes)],
    [t("after bytes"), String(report.after_bytes)],
    [t("source fingerprint"), report.source_fingerprint],
  ]} />
}

function ResultView({ title, rows }: { title: string; rows: Array<[string, string]> }) {
  return (
    <div className="mt-3 space-y-1 rounded-md border border-emerald-500/40 bg-emerald-500/5 p-2 text-xs">
      <div className="mb-1 font-medium text-emerald-700">{title}</div>
      {rows.map(([label, value]) => <InfoRow key={label} label={label} value={value} />)}
    </div>
  )
}

function StatsGrid({ stats }: { stats?: BoardStats }) {
  const { t } = useI18n()
  if (!stats) return <Skeleton className="h-24" />
  return (
    <div className="space-y-4 text-sm">
      <div className="grid grid-cols-2 gap-2">
        <MetricStrip
          className="contents"
          items={[
            { id: "board", label: t("board"), value: stats.board_id },
            { id: "generated", label: t("generated"), value: String(stats.generated_at) },
          ]}
        />
      </div>
      <div>
        <Subheading>{t("Status counts")}</Subheading>
        <div className="grid grid-cols-3 gap-2">
          <MetricStrip
            className="contents"
            items={stats.status_counts.map((entry) => ({ id: `status-${entry.status}`, label: entry.status, value: String(entry.count) }))}
          />
          {stats.status_counts.length === 0 ? <EmptyText>{t("No status counts returned.")}</EmptyText> : null}
        </div>
      </div>
      <div>
        <Subheading>{t("Stale claims")}</Subheading>
        <StaleClaimList claims={stats.stale_claims} />
      </div>
      <div>
        <Subheading>{t("Blocked reasons")}</Subheading>
        <div className="space-y-1">
          {stats.blocked_reasons.map((entry) => (
            <InfoRow key={entry.reason} label={entry.reason || t("unspecified")} value={String(entry.count)} />
          ))}
          {stats.blocked_reasons.length === 0 ? <EmptyText>{t("No blocked reasons returned.")}</EmptyText> : null}
        </div>
      </div>
    </div>
  )
}

function SearchStatus({ meta }: { meta?: SearchIndexStatus }) {
  const { t } = useI18n()
  if (!meta) return <Skeleton className="h-24" />
  return (
    <div className="space-y-2 text-sm">
      <InfoRow label={t("backend")} value={meta.backend} />
      <InfoRow label={t("derived index")} value={String(meta.derived_index)} />
      <InfoRow label={t("stale")} value={String(meta.stale)} />
      <InfoRow label={t("index version")} value={meta.index_version ?? "-"} />
      <InfoRow label={t("last event")} value={meta.last_event_id === null ? "-" : String(meta.last_event_id)} />
      <InfoRow label={t("lag events")} value={meta.index_lag_events === null ? "-" : String(meta.index_lag_events)} />
      <div className="text-muted-foreground">{meta.message}</div>
    </div>
  )
}

function DoctorReportView({ report }: { report: DoctorReport }) {
  const { t } = useI18n()
  const findings = [
    ["integrity", report.integrity_check],
    ["migration version", nullableNumber(report.migration_version)],
    ["user version", report.user_version],
    ["expired running", report.expired_running_tasks],
    ["running without run", report.running_tasks_without_active_run],
    ["orphan running runs", report.orphan_running_runs],
    ["dependency cycles", report.dependency_cycles],
    ["archived dependency edges", report.archived_dependency_edges],
    ["missing run logs", report.missing_run_logs],
    ["suspicious run logs", report.suspicious_run_log_paths],
    ["dependency violations", report.executable_dependency_violations],
    ["spec violations", report.executable_spec_violations],
    ["schedule violations", report.executable_schedule_violations],
    ["outbox pending", report.outbox_pending],
    ["outbox running", report.outbox_running],
    ["outbox failed", report.outbox_failed],
    ["dirty stores", report.derived_dirty_stores],
    ["error stores", report.derived_error_stores],
    ["consistency errors", report.consistency_errors],
    ["consistency warnings", report.consistency_warnings],
    ["ontology errors", report.ontology_ledger_errors],
    ["ontology warnings", report.ontology_ledger_warnings],
  ] as const
  return (
    <div className="mt-3 space-y-3 text-sm">
      <Badge variant={report.ok ? "ready" : "blocked"}>{report.ok ? t("ok") : t("findings")}</Badge>
      <div className="grid grid-cols-2 gap-2">
        <MetricStrip
          className="contents"
          items={findings.map(([label, value]) => ({ id: label.replace(/ /g, "-"), label: t(label), value: String(value) }))}
        />
      </div>
      <div>
        <Subheading>{t("Derived stores")}</Subheading>
        <DerivedStoreList stores={report.derived_stores} />
      </div>
    </div>
  )
}

function StaleClaimList({ claims }: { claims: StaleClaim[] }) {
  const { t } = useI18n()
  if (claims.length === 0) return <EmptyText>{t("No stale claims returned.")}</EmptyText>
  return (
    <div className="space-y-2">
      {claims.map((claim) => (
        <Card key={claim.task_id} className="p-2">
          <div className="flex justify-between gap-3">
            <span className="truncate font-medium">#{claim.seq} {claim.title}</span>
            <span className="shrink-0 text-muted-foreground">{claim.claim_owner ?? t("no owner")}</span>
          </div>
          <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-muted-foreground">
            <span>{t("expires {value}", { value: nullableNumber(claim.claim_expires_at) })}</span>
            <span>{t("heartbeat {value}", { value: nullableNumber(claim.last_heartbeat_at) })}</span>
            <span>{t("run {value}", { value: claim.current_run_id ?? "-" })}</span>
            <span>{t("retry {current}/{max}", { current: claim.retry_count, max: nullableNumber(claim.max_retries) })}</span>
          </div>
        </Card>
      ))}
    </div>
  )
}

function DerivedStoreList({ stores }: { stores: DoctorDerivedStore[] }) {
  const { t } = useI18n()
  if (stores.length === 0) return <EmptyText>{t("No derived stores returned.")}</EmptyText>
  return (
    <div className="space-y-2">
      {stores.map((store) => (
        <Card key={store.store_name} className="p-2">
          <div className="flex justify-between gap-3">
            <span className="truncate font-medium">{store.store_name}</span>
            <span className={store.dirty || store.last_error ? "shrink-0 text-amber-700" : "shrink-0 text-emerald-700"}>
              {store.dirty ? t("dirty") : t("clean")}
            </span>
          </div>
          <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-muted-foreground">
            <span>{t("schema {value}", { value: store.schema_version })}</span>
            <span>{t("event {value}", { value: store.last_event_id })}</span>
            <span>{t("pending {value}", { value: store.pending_outbox })}</span>
            <span>{t("running {value}", { value: store.running_outbox })}</span>
            <span>{t("failed {value}", { value: store.failed_outbox })}</span>
            <span className="truncate">{t("error {value}", { value: store.last_error ?? "-" })}</span>
          </div>
        </Card>
      ))}
    </div>
  )
}

function CheckpointResultView({ result }: { result: CheckpointReport }) {
  const { t } = useI18n()
  return (
    <div className="mt-3 space-y-2 text-sm">
      <Badge variant={result.busy === 0 ? "ready" : "blocked"}>{result.busy === 0 ? t("checkpointed") : t("busy")}</Badge>
      <InfoRow label={t("busy")} value={String(result.busy)} />
      <InfoRow label={t("log frames")} value={String(result.log_frames)} />
      <InfoRow label={t("checkpointed frames")} value={String(result.checkpointed_frames)} />
    </div>
  )
}

function Subheading({ children }: { children: ReactNode }) {
  return <div className="mb-2 text-xs font-semibold uppercase text-muted-foreground">{children}</div>
}

function EmptyText({ children }: { children: ReactNode }) {
  return (
    <Empty className="items-start p-0 text-left">
      <EmptyDescription>{children}</EmptyDescription>
    </Empty>
  )
}

function nullableNumber(value: number | null) {
  return value === null ? "-" : String(value)
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <Item className="px-0 py-0">
      <ItemContent>
        <ItemTitle className="text-sm font-normal text-muted-foreground">{label}</ItemTitle>
      </ItemContent>
      <ItemActions className="min-w-0">
        <span className="truncate font-medium">{value}</span>
      </ItemActions>
    </Item>
  )
}

function ErrorText({ error }: { error: unknown }) {
  const { t } = useI18n()
  return (
    <Alert className="mt-3 border-destructive/50">
      <AlertDescription className="text-destructive">{presentApiError(error, t)}</AlertDescription>
    </Alert>
  )
}

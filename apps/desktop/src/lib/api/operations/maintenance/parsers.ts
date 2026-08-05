import { expectArray, expectRecord, expectExactKeys, expectString, expectBoolean, expectSafeInteger, expectNullableString, expectNullableInteger } from "../../parsers"
import type {
  BackupReport,
  CheckpointReport,
  DoctorDerivedStore,
  DoctorIssue,
  DoctorReport,
  ExportReport,
  ImportReport,
  LegacyImportReport,
  LegacyImportTableCount,
  MaintenanceOwnerStatus,
  MaintenanceRunReport,
  MaintenanceStatusReport,
  ProjectionStoreStatus,
  VacuumReport,
} from "../../types"
const DOCTOR_STORE_KEYS = ["store_name", "schema_version", "last_event_id", "dirty", "last_error", "pending_outbox", "running_outbox", "failed_outbox"] as const
const DOCTOR_ISSUE_KEYS = ["severity", "code", "message", "record_ids"] as const
const DOCTOR_KEYS = ["ok", "integrity_check", "migration_version", "user_version", "expired_running_tasks", "running_tasks_without_active_run", "orphan_running_runs", "dependency_cycles", "archived_dependency_edges", "missing_run_logs", "suspicious_run_log_paths", "executable_dependency_violations", "executable_spec_violations", "executable_schedule_violations", "unplanned_active_tasks", "active_parents_with_incomplete_required_steps", "outbox_pending", "outbox_running", "outbox_failed", "derived_dirty_stores", "derived_error_stores", "derived_stores", "consistency_errors", "consistency_warnings", "consistency_issues", "ontology_ledger_errors", "ontology_ledger_warnings", "ontology_ledger_issues"] as const
const CHECKPOINT_KEYS = ["busy", "log_frames", "checkpointed_frames"] as const
const BACKUP_KEYS = ["out_path", "checksum_sha256", "bytes", "source_fingerprint"] as const
const EXPORT_KEYS = ["out_path", "checksum_sha256", "bytes", "record_count", "source_fingerprint"] as const
const IMPORT_KEYS = ["in_path", "source_fingerprint", "imported_records", "skipped_records", "rebuild_jobs_enqueued", "journal_id"] as const
const VACUUM_KEYS = ["ok", "before_bytes", "after_bytes", "source_fingerprint"] as const
const OWNER_KEYS = ["owner", "mode", "lease_expires_at", "fence_epoch", "build_identity", "last_heartbeat_at", "active"] as const
const PROJECTION_STORE_KEYS = [
  "store_name",
  "active_generation",
  "active_fingerprint",
  "previous_generation",
  "building_generation",
  "lifecycle_status",
  "fence_epoch",
  "last_event_id",
  "dirty",
  "pending",
  "running",
  "failed",
  "last_error",
  "phase",
  "degraded",
  "errors",
  "updated_at",
] as const
const MAINTENANCE_STATUS_KEYS = ["database_instance_id", "protocol_version", "owner", "stores"] as const
const MAINTENANCE_RUN_KEYS = ["database_instance_id", "protocol_version", "owner", "mode", "action", "processed", "phase", "degraded", "errors", "stores"] as const
const LEGACY_IMPORT_TABLE_KEYS = ["table", "source_rows", "target_rows"] as const
const LEGACY_IMPORT_KEYS = [
  "journal_id",
  "phase",
  "source_path",
  "source_fingerprint",
  "schema_fingerprint",
  "resumed",
  "attachment_count",
  "table_counts",
] as const

export function parseDoctorIssue(value: unknown, label: string): DoctorIssue {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, DOCTOR_ISSUE_KEYS, label)
  expectString(record.severity, label + ".severity")
  expectString(record.code, label + ".code")
  expectString(record.message, label + ".message")
  expectArray<unknown>(record.record_ids, label + ".record_ids").forEach((entry, index) => expectString(entry, label + ".record_ids[" + index + "]"))
  return record as DoctorIssue
}

export function parseDoctorStore(value: unknown, label: string): DoctorDerivedStore {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, DOCTOR_STORE_KEYS, label)
  expectString(record.store_name, label + ".store_name")
  for (const key of ["schema_version", "last_event_id", "pending_outbox", "running_outbox", "failed_outbox"] as const) expectSafeInteger(record[key], label + "." + key)
  expectBoolean(record.dirty, label + ".dirty")
  expectNullableString(record.last_error, label + ".last_error")
  return record as DoctorDerivedStore
}

export function parseDoctorReport(value: unknown): DoctorReport {
  const record = expectRecord<Record<string, unknown>>(value, "doctor response data")
  expectExactKeys(record, DOCTOR_KEYS, "doctor response data")
  expectBoolean(record.ok, "doctor response data.ok")
  expectString(record.integrity_check, "doctor response data.integrity_check")
  expectNullableInteger(record.migration_version, "doctor response data.migration_version")
  for (const key of DOCTOR_KEYS) {
    if (!["ok", "integrity_check", "migration_version", "derived_stores", "consistency_issues", "ontology_ledger_issues"].includes(key)) {
      expectSafeInteger(record[key], "doctor response data." + key)
    }
  }
  expectArray<unknown>(record.derived_stores, "doctor response data.derived_stores").forEach((entry, index) => parseDoctorStore(entry, "doctor response data.derived_stores[" + index + "]"))
  for (const key of ["consistency_issues", "ontology_ledger_issues"] as const) {
    expectArray<unknown>(record[key], "doctor response data." + key).forEach((entry, index) => parseDoctorIssue(entry, "doctor response data." + key + "[" + index + "]"))
  }
  return record as DoctorReport
}

export function parseCheckpointReport(value: unknown): CheckpointReport {
  const record = expectRecord<Record<string, unknown>>(value, "checkpoint response data")
  expectExactKeys(record, CHECKPOINT_KEYS, "checkpoint response data")
  for (const key of CHECKPOINT_KEYS) expectSafeInteger(record[key], "checkpoint response data." + key)
  return record as CheckpointReport
}

export function parseBackupReport(value: unknown): BackupReport {
  const record = expectRecord<Record<string, unknown>>(value, "backup response data")
  expectExactKeys(record, BACKUP_KEYS, "backup response data")
  expectString(record.out_path, "backup response data.out_path")
  expectString(record.checksum_sha256, "backup response data.checksum_sha256")
  expectSafeInteger(record.bytes, "backup response data.bytes", true)
  expectString(record.source_fingerprint, "backup response data.source_fingerprint")
  return record as BackupReport
}

export function parseExportReport(value: unknown): ExportReport {
  const record = expectRecord<Record<string, unknown>>(value, "export response data")
  expectExactKeys(record, EXPORT_KEYS, "export response data")
  expectString(record.out_path, "export response data.out_path")
  expectString(record.checksum_sha256, "export response data.checksum_sha256")
  expectSafeInteger(record.bytes, "export response data.bytes", true)
  expectSafeInteger(record.record_count, "export response data.record_count", true)
  expectString(record.source_fingerprint, "export response data.source_fingerprint")
  return record as ExportReport
}

export function parseImportReport(value: unknown): ImportReport {
  const record = expectRecord<Record<string, unknown>>(value, "import response data")
  expectExactKeys(record, IMPORT_KEYS, "import response data")
  expectString(record.in_path, "import response data.in_path")
  expectString(record.source_fingerprint, "import response data.source_fingerprint")
  expectSafeInteger(record.imported_records, "import response data.imported_records", true)
  expectSafeInteger(record.skipped_records, "import response data.skipped_records", true)
  expectSafeInteger(record.rebuild_jobs_enqueued, "import response data.rebuild_jobs_enqueued", true)
  expectString(record.journal_id, "import response data.journal_id")
  return record as ImportReport
}

export function parseVacuumReport(value: unknown): VacuumReport {
  const record = expectRecord<Record<string, unknown>>(value, "vacuum response data")
  expectExactKeys(record, VACUUM_KEYS, "vacuum response data")
  expectBoolean(record.ok, "vacuum response data.ok")
  expectSafeInteger(record.before_bytes, "vacuum response data.before_bytes", true)
  expectSafeInteger(record.after_bytes, "vacuum response data.after_bytes", true)
  expectString(record.source_fingerprint, "vacuum response data.source_fingerprint")
  return record as VacuumReport
}

export function parseMaintenanceOwner(value: unknown, label = "maintenance status response data.owner"): MaintenanceOwnerStatus {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, OWNER_KEYS, label)
  expectNullableString(record.owner, `${label}.owner`)
  expectNullableString(record.mode, `${label}.mode`)
  expectNullableInteger(record.lease_expires_at, `${label}.lease_expires_at`)
  expectSafeInteger(record.fence_epoch, `${label}.fence_epoch`)
  expectNullableString(record.build_identity, `${label}.build_identity`)
  expectNullableInteger(record.last_heartbeat_at, `${label}.last_heartbeat_at`)
  expectBoolean(record.active, `${label}.active`)
  return record as MaintenanceOwnerStatus
}

export function parseProjectionStoreStatus(value: unknown, label: string): ProjectionStoreStatus {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, PROJECTION_STORE_KEYS, label)
  expectString(record.store_name, `${label}.store_name`)
  for (const key of ["fence_epoch", "last_event_id", "pending", "running", "failed", "updated_at"] as const) {
    expectSafeInteger(record[key], `${label}.${key}`)
  }
  for (const key of ["active_generation", "active_fingerprint", "previous_generation", "building_generation", "last_error"] as const) {
    expectNullableString(record[key], `${label}.${key}`)
  }
  expectString(record.lifecycle_status, `${label}.lifecycle_status`)
  expectBoolean(record.dirty, `${label}.dirty`)
  expectString(record.phase, `${label}.phase`)
  expectBoolean(record.degraded, `${label}.degraded`)
  expectArray<unknown>(record.errors, `${label}.errors`).forEach((entry, index) => expectString(entry, `${label}.errors[${index}]`))
  return record as ProjectionStoreStatus
}

export function parseMaintenanceStatusReport(value: unknown): MaintenanceStatusReport {
  const record = expectRecord<Record<string, unknown>>(value, "maintenance status response data")
  expectExactKeys(record, MAINTENANCE_STATUS_KEYS, "maintenance status response data")
  const databaseInstanceId = expectString(record.database_instance_id, "maintenance status response data.database_instance_id")
  const protocolVersion = expectSafeInteger(record.protocol_version, "maintenance status response data.protocol_version", true)
  const stores = expectArray<unknown>(record.stores, "maintenance status response data.stores")
  return {
    database_instance_id: databaseInstanceId,
    protocol_version: protocolVersion,
    owner: parseMaintenanceOwner(record.owner),
    stores: stores.map((entry, index) => parseProjectionStoreStatus(entry, `maintenance status response data.stores[${index}]`)),
  }
}

export function parseMaintenanceRunReport(value: unknown): MaintenanceRunReport {
  const record = expectRecord<Record<string, unknown>>(value, "maintenance run response data")
  expectExactKeys(record, MAINTENANCE_RUN_KEYS, "maintenance run response data")
  const databaseInstanceId = expectString(record.database_instance_id, "maintenance run response data.database_instance_id")
  const protocolVersion = expectSafeInteger(record.protocol_version, "maintenance run response data.protocol_version", true)
  const owner = expectString(record.owner, "maintenance run response data.owner")
  const mode = expectString(record.mode, "maintenance run response data.mode")
  const action = expectString(record.action, "maintenance run response data.action")
  const processed = expectSafeInteger(record.processed, "maintenance run response data.processed", true)
  const phase = expectString(record.phase, "maintenance run response data.phase")
  const degraded = expectBoolean(record.degraded, "maintenance run response data.degraded")
  const errors = expectArray<unknown>(record.errors, "maintenance run response data.errors")
    .map((entry, index) => expectString(entry, `maintenance run response data.errors[${index}]`))
  const stores = expectArray<unknown>(record.stores, "maintenance run response data.stores")
  return {
    database_instance_id: databaseInstanceId,
    protocol_version: protocolVersion,
    owner,
    mode,
    action,
    processed,
    phase,
    degraded,
    errors,
    stores: stores.map((entry, index) => parseProjectionStoreStatus(entry, `maintenance run response data.stores[${index}]`)),
  }
}

export function parseLegacyImportTableCount(value: unknown, label: string): LegacyImportTableCount {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, LEGACY_IMPORT_TABLE_KEYS, label)
  expectString(record.table, `${label}.table`)
  expectSafeInteger(record.source_rows, `${label}.source_rows`, true)
  expectSafeInteger(record.target_rows, `${label}.target_rows`, true)
  return record as LegacyImportTableCount
}

export function parseLegacyImportReport(value: unknown): LegacyImportReport {
  const record = expectRecord<Record<string, unknown>>(value, "legacy import response data")
  expectExactKeys(record, LEGACY_IMPORT_KEYS, "legacy import response data")
  expectString(record.journal_id, "legacy import response data.journal_id")
  expectString(record.phase, "legacy import response data.phase")
  expectString(record.source_path, "legacy import response data.source_path")
  expectString(record.source_fingerprint, "legacy import response data.source_fingerprint")
  expectString(record.schema_fingerprint, "legacy import response data.schema_fingerprint")
  expectBoolean(record.resumed, "legacy import response data.resumed")
  expectSafeInteger(record.attachment_count, "legacy import response data.attachment_count", true)
  expectArray<unknown>(record.table_counts, "legacy import response data.table_counts").forEach((entry, index) =>
    parseLegacyImportTableCount(entry, `legacy import response data.table_counts[${index}]`),
  )
  return record as LegacyImportReport
}

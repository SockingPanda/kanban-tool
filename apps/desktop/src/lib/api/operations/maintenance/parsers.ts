import { expectArray, expectRecord, expectExactKeys, expectString, expectBoolean, expectSafeInteger, expectNullableString, expectNullableInteger } from "../../parsers"
import type { CheckpointReport, DoctorDerivedStore, DoctorIssue, DoctorReport } from "../../types"
const DOCTOR_STORE_KEYS = ["store_name", "schema_version", "last_event_id", "dirty", "last_error", "pending_outbox", "running_outbox", "failed_outbox"] as const
const DOCTOR_ISSUE_KEYS = ["severity", "code", "message", "record_ids"] as const
const DOCTOR_KEYS = ["ok", "integrity_check", "migration_version", "user_version", "expired_running_tasks", "running_tasks_without_active_run", "orphan_running_runs", "dependency_cycles", "archived_dependency_edges", "missing_run_logs", "suspicious_run_log_paths", "executable_dependency_violations", "executable_spec_violations", "executable_schedule_violations", "unplanned_active_tasks", "active_parents_with_incomplete_required_steps", "outbox_pending", "outbox_running", "outbox_failed", "derived_dirty_stores", "derived_error_stores", "derived_stores", "consistency_errors", "consistency_warnings", "consistency_issues", "ontology_ledger_errors", "ontology_ledger_warnings", "ontology_ledger_issues"] as const
const CHECKPOINT_KEYS = ["busy", "log_frames", "checkpointed_frames"] as const

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

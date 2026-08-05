use serde::{Deserialize, Serialize};

use crate::DataEnvelope;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DoctorDerivedStore {
    pub store_name: String,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_error: Option<String>,
    pub pending_outbox: i64,
    pub running_outbox: i64,
    pub failed_outbox: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DoctorIssue {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub record_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DoctorReport {
    pub ok: bool,
    pub integrity_check: String,
    pub migration_version: Option<i64>,
    pub user_version: i64,
    pub expired_running_tasks: i64,
    pub running_tasks_without_active_run: i64,
    pub orphan_running_runs: i64,
    pub dependency_cycles: i64,
    pub archived_dependency_edges: i64,
    pub missing_run_logs: i64,
    pub suspicious_run_log_paths: i64,
    pub executable_dependency_violations: i64,
    pub executable_spec_violations: i64,
    pub executable_schedule_violations: i64,
    pub unplanned_active_tasks: i64,
    pub active_parents_with_incomplete_required_steps: i64,
    pub outbox_pending: i64,
    pub outbox_running: i64,
    pub outbox_failed: i64,
    pub derived_dirty_stores: i64,
    pub derived_error_stores: i64,
    pub derived_stores: Vec<DoctorDerivedStore>,
    pub consistency_errors: i64,
    pub consistency_warnings: i64,
    pub consistency_issues: Vec<DoctorIssue>,
    pub ontology_ledger_errors: i64,
    pub ontology_ledger_warnings: i64,
    pub ontology_ledger_issues: Vec<DoctorIssue>,
}

pub type DoctorResponse = DataEnvelope<DoctorReport>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CheckpointReport {
    pub busy: i64,
    pub log_frames: i64,
    pub checkpointed_frames: i64,
}

pub type CheckpointResponse = DataEnvelope<CheckpointReport>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BackupReport {
    pub out_path: String,
    pub checksum_sha256: String,
    pub bytes: u64,
    pub source_fingerprint: String,
}

pub type BackupResponse = DataEnvelope<BackupReport>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExportReport {
    pub out_path: String,
    pub checksum_sha256: String,
    pub bytes: u64,
    pub record_count: u64,
    pub source_fingerprint: String,
}

pub type ExportResponse = DataEnvelope<ExportReport>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ImportReport {
    pub in_path: String,
    pub source_fingerprint: String,
    pub imported_records: u64,
    pub skipped_records: u64,
    pub rebuild_jobs_enqueued: u64,
    pub journal_id: String,
    pub phase: String,
    pub restart_required: bool,
    pub staged_database_path: Option<String>,
    pub target_fingerprint_before: Option<String>,
    pub staged_fingerprint: Option<String>,
    pub publish_preconditions: Vec<String>,
}

pub type ImportResponse = DataEnvelope<ImportReport>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VacuumReport {
    pub ok: bool,
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub source_fingerprint: String,
}

pub type VacuumResponse = DataEnvelope<VacuumReport>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaintenanceOwnerStatus {
    pub owner: Option<String>,
    pub mode: Option<String>,
    pub lease_expires_at: Option<i64>,
    pub fence_epoch: i64,
    pub build_identity: Option<String>,
    pub last_heartbeat_at: Option<i64>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionStoreStatus {
    pub store_name: String,
    pub active_generation: Option<String>,
    pub active_fingerprint: Option<String>,
    pub previous_generation: Option<String>,
    pub building_generation: Option<String>,
    pub lifecycle_status: String,
    pub fence_epoch: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub pending: i64,
    pub running: i64,
    pub failed: i64,
    pub last_error: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaintenanceStatusReport {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub owner: MaintenanceOwnerStatus,
    pub stores: Vec<ProjectionStoreStatus>,
}

pub type MaintenanceStatusResponse = DataEnvelope<MaintenanceStatusReport>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaintenanceRunReport {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub owner: String,
    pub mode: String,
    pub action: String,
    pub processed: u64,
    pub stores: Vec<ProjectionStoreStatus>,
}

pub type MaintenanceRunResponse = DataEnvelope<MaintenanceRunReport>;
pub type MaintenanceRebuildResponse = DataEnvelope<MaintenanceRunReport>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaintenancePathRequest {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaintenanceImportRequest {
    pub path: String,
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaintenanceRunRequest {
    pub owner: Option<String>,
    pub action: Option<String>,
}

/// Legacy SQLite v30 import request.  This host-admin path is deliberately
/// separate from portable JSONL import because it reads an old on-disk schema
/// and may need an explicit attachment-root mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LegacyImportRequest {
    pub path: String,
    #[serde(default)]
    pub canonical_attachment_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LegacyImportTableCount {
    pub table: String,
    pub source_rows: u64,
    pub target_rows: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LegacyImportReport {
    pub journal_id: String,
    pub phase: String,
    pub source_path: String,
    pub source_fingerprint: String,
    pub schema_fingerprint: String,
    pub resumed: bool,
    pub attachment_count: u64,
    pub table_counts: Vec<LegacyImportTableCount>,
}

pub type LegacyImportResponse = DataEnvelope<LegacyImportReport>;

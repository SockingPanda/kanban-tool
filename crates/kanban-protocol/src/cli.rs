use serde::{Deserialize, Serialize};

use crate::{
    ApiAttachment, ApiClaim, ApiComment, ApiExecutionPlan, ApiRun, ApiTask, ApiTaskStatus,
    ApiTaskStep, ApiTaskSteps, CheckpointReport, ContractSurface, DataEnvelope, DoctorReport,
    GetTaskResponse, MigrationState, ProjectionCorpusMetadata, QueueStats, SearchStatus,
    surface_operation_catalog,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliBackupResult {
    pub out_path: String,
}

pub type CliBackupOutput = DataEnvelope<CliBackupResult>;
pub type CliCheckpointOutput = DataEnvelope<CheckpointReport>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliVacuumResult {
    pub ok: bool,
}

pub type CliVacuumOutput = DataEnvelope<CliVacuumResult>;
pub type CliDoctorOutput = DataEnvelope<DoctorReport>;
pub type CliStatsOutput = DataEnvelope<QueueStats>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliInitResult {
    pub db_path: String,
    pub board_id: String,
    pub board_slug: String,
    /// Project config path created or reused by `kanban init`; database ownership remains with `serve`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    /// Whether this invocation changed the project config file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<bool>,
}

pub type CliInitOutput = DataEnvelope<CliInitResult>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliConfigShow {
    pub db: CliResolvedConfigValue,
    pub board: CliResolvedConfigValue,
    pub locale: CliResolvedLocaleValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliResolvedConfigValue {
    pub value: String,
    pub source: CliConfigSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliResolvedLocaleValue {
    pub value: String,
    pub source: CliConfigSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CliConfigSource {
    Flag { name: String },
    Env { name: String },
    ProjectConfig { path: String, key: String },
    GlobalConfig { path: String, key: String },
    Default,
}

pub type CliConfigShowOutput = DataEnvelope<CliConfigShow>;
pub type CliIndexStatusOutput = DataEnvelope<SearchStatus>;
pub type CliIndexDoctorOutput = DataEnvelope<SearchStatus>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliMaintenanceOwnerStatus {
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub owner: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub mode: Option<String>,
    pub capabilities: Vec<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub build_identity: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub lease_expires_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub last_heartbeat_at: Option<i64>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliProjectionStoreStatus {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub control_plane: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub active_generation: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub active_fingerprint: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub active_fence_epoch: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub active_provider: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub active_provider_fingerprint: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_projection_corpus_schema")
    )]
    pub active_corpus: Option<ProjectionCorpusMetadata>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub previous_generation: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub previous_fingerprint: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub previous_fence_epoch: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_projection_corpus_schema")
    )]
    pub previous_corpus: Option<ProjectionCorpusMetadata>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub building_generation: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub building_fingerprint: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub building_fence_epoch: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub building_provider: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub building_provider_fingerprint: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_projection_corpus_schema")
    )]
    pub building_corpus: Option<ProjectionCorpusMetadata>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub building_phase: Option<String>,
    pub snapshot_cursor: i64,
    pub checkpoint_cursor: i64,
    pub legacy_checkpoint_cursor: i64,
    pub lifecycle_status: String,
    pub runtime_availability: CliProjectionRuntimeAvailability,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub owner: Option<String>,
    pub fence_epoch: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub lease_expires_at: Option<i64>,
    pub pending: i64,
    pub running: i64,
    pub failed: i64,
    pub legacy_done: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub pending_age_ms: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub last_success_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub last_error: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub fallback_reason: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliMaintenanceStatus {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub maintenance_owner: CliMaintenanceOwnerStatus,
    pub stores: Vec<CliProjectionStoreStatus>,
}

pub type CliMaintenanceStatusOutput = DataEnvelope<CliMaintenanceStatus>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CliProjectionRuntimeAvailability {
    Available,
    Unavailable,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CliMaintenanceMode {
    Once,
    Continuous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CliMaintenanceStoreFailureKind {
    Provider,
    Backend,
    Delivery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CliMaintenanceStoreResult {
    Succeeded {
        action: String,
        processed: usize,
    },
    Failed {
        kind: CliMaintenanceStoreFailureKind,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliMaintenanceStoreRun {
    pub store_name: String,
    pub result: CliMaintenanceStoreResult,
    pub lifecycle_status: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliMaintenanceRun {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub owner: String,
    pub mode: CliMaintenanceMode,
    pub stores: Vec<CliMaintenanceStoreRun>,
}

pub type CliMaintenanceRunOutput = DataEnvelope<CliMaintenanceRun>;
pub type CliMaintenanceRebuildOutput = DataEnvelope<CliMaintenanceRun>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CliMaintenanceLegacyCleanupAction {
    Inventory,
    Apply,
    Verify,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CliLegacyProjectionRootKind {
    TantivyV1,
    OxigraphV1,
    LanceDbV1,
    TantivyUnscopedV2,
    OxigraphUnscopedV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliMaintenanceLegacyCleanupRoot {
    pub kind: CliLegacyProjectionRootKind,
    pub relative_path: String,
    pub absolute_path: String,
    pub present: bool,
    pub file_count: u64,
    pub directory_count: u64,
    pub byte_count: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliMaintenanceLegacyCleanup {
    pub action: CliMaintenanceLegacyCleanupAction,
    pub dry_run: bool,
    pub resumed: bool,
    pub format_version: u32,
    pub database_instance_id: String,
    pub database_path: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub backup_dir: Option<String>,
    pub inventory_digest: String,
    pub roots: Vec<CliMaintenanceLegacyCleanupRoot>,
}

pub type CliMaintenanceLegacyCleanupOutput = DataEnvelope<CliMaintenanceLegacyCleanup>;

macro_rules! legacy_cleanup_output {
    (
        $payload:ident,
        $output:ident,
        $expected_action:ident,
        $action_literal:literal,
        $expected_dry_run:expr
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $payload(pub CliMaintenanceLegacyCleanup);

        impl Serialize for $payload {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        impl<'de> Deserialize<'de> for $payload {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let report = CliMaintenanceLegacyCleanup::deserialize(deserializer)?;
                if report.action != CliMaintenanceLegacyCleanupAction::$expected_action
                    || report.dry_run != $expected_dry_run
                {
                    return Err(serde::de::Error::custom(concat!(
                        "expected cleanup-legacy ",
                        $action_literal,
                        " output"
                    )));
                }
                Ok(Self(report))
            }
        }

        #[cfg(feature = "schema")]
        impl schemars::JsonSchema for $payload {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($payload).into()
            }

            fn inline_schema() -> bool {
                true
            }

            fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                legacy_cleanup_schema(generator, $action_literal, $expected_dry_run)
            }
        }

        pub type $output = DataEnvelope<$payload>;
    };
}

legacy_cleanup_output!(
    CliMaintenanceLegacyCleanupInventory,
    CliMaintenanceLegacyCleanupInventoryOutput,
    Inventory,
    "inventory",
    true
);
legacy_cleanup_output!(
    CliMaintenanceLegacyCleanupApply,
    CliMaintenanceLegacyCleanupApplyOutput,
    Apply,
    "apply",
    false
);
legacy_cleanup_output!(
    CliMaintenanceLegacyCleanupVerify,
    CliMaintenanceLegacyCleanupVerifyOutput,
    Verify,
    "verify",
    false
);
legacy_cleanup_output!(
    CliMaintenanceLegacyCleanupRestore,
    CliMaintenanceLegacyCleanupRestoreOutput,
    Restore,
    "restore",
    false
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliOutboxItem {
    pub id: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub source_event_id: Option<i64>,
    pub target: String,
    pub entity_uri: String,
    pub action: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub attempts: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub type CliOutboxListOutput = DataEnvelope<Vec<CliOutboxItem>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliEntity {
    pub uri: String,
    pub kind: String,
    pub source_table: String,
    pub source_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub board_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub task_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub title: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub summary: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub content_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub archived_at: Option<i64>,
}

pub type CliEntityListOutput = DataEnvelope<Vec<CliEntity>>;
pub type CliEntityShowOutput = DataEnvelope<CliEntity>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliDerivedStoreStatus {
    pub store_name: String,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub last_rebuild_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub last_sync_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub last_error: Option<String>,
    pub updated_at: i64,
}

pub type CliDerivedStatusOutput = DataEnvelope<Vec<CliDerivedStoreStatus>>;

pub type CliTaskListOutput = DataEnvelope<Vec<ApiTask>>;
pub type CliTaskShowOutput = GetTaskResponse;
pub type CliTaskCreateOutput = DataEnvelope<ApiTask>;
pub type CliTaskUpdateOutput = DataEnvelope<ApiTask>;
pub type CliTaskPromoteOutput = DataEnvelope<ApiTask>;
pub type CliTaskReopenOutput = DataEnvelope<ApiTask>;
pub type CliTaskHeartbeatOutput = DataEnvelope<ApiTask>;
pub type CliTaskReleaseOutput = DataEnvelope<ApiTask>;
pub type CliTaskDoneOutput = DataEnvelope<ApiTask>;
pub type CliTaskCompleteOutput = DataEnvelope<ApiTask>;
pub type CliTaskReviewOutput = DataEnvelope<ApiTask>;
pub type CliTaskBlockOutput = DataEnvelope<ApiTask>;
pub type CliTaskUnblockOutput = DataEnvelope<ApiTask>;
pub type CliTaskArchiveOutput = DataEnvelope<ApiTask>;
pub type CliTaskClaimOutput = DataEnvelope<ApiClaim>;
pub type CliTaskStartOutput = DataEnvelope<ApiClaim>;
pub type CliTaskReclaimOutput = DataEnvelope<ApiTask>;
pub type CliCommentListOutput = DataEnvelope<Vec<ApiComment>>;
pub type CliCommentAddOutput = DataEnvelope<ApiComment>;
pub type CliAttachmentListOutput = DataEnvelope<Vec<ApiAttachment>>;
pub type CliAttachmentAddOutput = DataEnvelope<ApiAttachment>;
pub type CliAttachmentRemoveOutput = DataEnvelope<crate::DeleteResult>;
pub type CliTaskStepListOutput = DataEnvelope<ApiTaskSteps>;
pub type CliTaskStepAddOutput = DataEnvelope<ApiTaskSteps>;
pub type CliTaskStepUpdateOutput = DataEnvelope<ApiTaskSteps>;
pub type CliTaskStepDoneOutput = DataEnvelope<ApiTaskStep>;
pub type CliTaskStepSkipOutput = DataEnvelope<ApiTaskStep>;
pub type CliTaskStepReopenOutput = DataEnvelope<ApiTaskStep>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliTaskStepRemoveResult {
    pub removed: bool,
    pub step: ApiTaskStep,
}

pub type CliTaskStepRemoveOutput = DataEnvelope<CliTaskStepRemoveResult>;
pub type CliTaskStepNotRequiredOutput = DataEnvelope<ApiExecutionPlan>;
pub type CliRunsOutput = DataEnvelope<Vec<ApiRun>>;
pub type CliRunShowOutput = DataEnvelope<ApiRun>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliRunLog {
    pub run_id: String,
    pub content: String,
    pub truncated: bool,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_u64_schema")
    )]
    pub tail_bytes: Option<u64>,
}

pub type CliRunLogsOutput = DataEnvelope<CliRunLog>;

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[cfg(feature = "schema")]
fn required_nullable_u64_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<u64>>()
}

#[cfg(feature = "schema")]
fn legacy_cleanup_schema(
    generator: &mut schemars::SchemaGenerator,
    action: &str,
    dry_run: bool,
) -> schemars::Schema {
    let mut schema = serde_json::to_value(
        <CliMaintenanceLegacyCleanup as schemars::JsonSchema>::json_schema(generator),
    )
    .expect("cleanup schema serializes");
    let properties = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
        .expect("cleanup schema has properties");
    properties.insert("action".to_owned(), serde_json::json!({ "const": action }));
    properties.insert(
        "dry_run".to_owned(),
        serde_json::json!({ "const": dry_run }),
    );
    schema.try_into().expect("cleanup schema remains valid")
}

#[cfg(feature = "schema")]
fn required_nullable_i64_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<i64>>()
}

#[cfg(feature = "schema")]
fn required_nullable_string_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<String>>()
}

#[cfg(feature = "schema")]
fn required_nullable_projection_corpus_schema(
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    generator.subschema_for::<Option<ProjectionCorpusMetadata>>()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliDependencyTask {
    pub id: String,
    pub board_id: String,
    pub board_slug: String,
    #[serde(rename = "ref")]
    pub task_ref: String,
    pub title: String,
    pub status: ApiTaskStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliDependencyEdge {
    pub parent: CliDependencyTask,
    pub child: CliDependencyTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliDependencySnapshot {
    pub task: CliDependencyTask,
    pub parents: Vec<CliDependencyTask>,
    pub children: Vec<CliDependencyTask>,
    pub edges: Vec<CliDependencyEdge>,
}

pub type CliDependencyListOutput = DataEnvelope<CliDependencySnapshot>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliDependencyMutation {
    pub edge: CliDependencyEdge,
    pub dependencies: CliDependencySnapshot,
}

pub type CliDependencyAddOutput = DataEnvelope<CliDependencyMutation>;
pub type CliDependencyRemoveOutput = DataEnvelope<CliDependencyMutation>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliEvent {
    pub id: i64,
    pub event_id: String,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub kind: String,
    pub actor: Option<String>,
    pub payload: serde_json::Value,
    pub created_at: i64,
}

pub type CliEventsOutput = DataEnvelope<Vec<CliEvent>>;

/// 本地项目配置中的 active board 选择。
///
/// 该 DTO 不包含 canonical board record；`board use/current` 只读写 `.kb/config.toml`，
/// 因此不能伪装成需要 host/数据库查询的 `ApiBoard`。`created` 和 `updated` 记录本次
/// 配置侧调用是否创建了文件、是否实际改变了 `board` 选择。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliBoardConfigSelection {
    pub board: String,
    pub config_path: String,
    pub source: CliConfigSource,
    pub created: bool,
    pub updated: bool,
}

pub type CliBoardUseOutput = DataEnvelope<CliBoardConfigSelection>;
pub type CliBoardCurrentOutput = DataEnvelope<CliBoardConfigSelection>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CliMachineOutput {
    Todo,
    Contract { id: &'static str },
    Excluded { reason: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliOperationDescriptor {
    pub key: String,
    pub machine_output: CliMachineOutput,
}

pub fn cli_operation_catalog() -> Vec<CliOperationDescriptor> {
    surface_operation_catalog()
        .into_iter()
        .filter(|operation| operation.surface == ContractSurface::Cli)
        .map(|operation| CliOperationDescriptor {
            key: operation.key,
            machine_output: match operation.migration {
                MigrationState::Excluded => CliMachineOutput::Excluded {
                    reason: operation
                        .exclusion
                        .expect("excluded CLI operation must explain its boundary"),
                },
                MigrationState::Generated | MigrationState::Adopted => {
                    let [id] = operation.contracts.as_slice() else {
                        panic!(
                            "generated/adopted CLI operation must have one exact output contract"
                        )
                    };
                    CliMachineOutput::Contract { id }
                }
                MigrationState::Planned => CliMachineOutput::Todo,
            },
        })
        .collect()
}

//! Projection v2 helper subprocess wire contracts.
//!
//! These types own only the strict stdin/stdout JSON shape. SQLite lease,
//! fencing, digest validation, generation publication and cleanup safety remain
//! runtime responsibilities.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const VECTOR_PROJECTION_PROTOCOL_VERSION: i64 = 2;

/// Schema-only marker for a key that is required but may contain JSON null.
#[cfg(feature = "schema")]
#[derive(schemars::JsonSchema)]
#[serde(transparent)]
struct RequiredNullableSchema<T>(Option<T>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionArtifactManifest {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub generation: String,
    pub fence_epoch: i64,
    pub snapshot_cursor: i64,
    pub provider: String,
    pub provider_fingerprint: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "RequiredNullableSchema<ProjectionCorpusMetadata>")
    )]
    pub corpus: Option<ProjectionCorpusMetadata>,
    pub canonical_item_count: i64,
    pub canonical_digest: String,
    pub delivery_item_count: i64,
    pub delivery_digest: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionCorpusMetadata {
    pub corpus_schema: String,
    pub corpus_fingerprint: String,
    pub embedding_model: String,
    pub embedding_dimensions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionSnapshotRecord {
    pub board_id: String,
    pub identity: String,
    pub payload_json: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionSnapshot {
    pub manifest: ProjectionArtifactManifest,
    pub records: Vec<ProjectionSnapshotRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionArtifactEvidence {
    pub manifest: ProjectionArtifactManifest,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionDelivery {
    pub id: i64,
    pub outbox_id: i64,
    pub store_name: String,
    pub generation_id: String,
    pub board_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub source_event_id: Option<i64>,
    pub cursor: i64,
    pub action: ProjectionDeliveryAction,
    pub entity_uri: String,
    pub payload_json: String,
    pub attempts: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProjectionDeliveryAction {
    Upsert,
    Delete,
    Rebuild,
}

/// Exact stdin representation of a fenced Projection v2 delivery batch.
///
/// `lease_token` and `claim_token` are opaque capabilities. Callers must pass
/// them over stdin only and must not log, persist or echo them through helper
/// stdout.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionBatch {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub provider: String,
    pub provider_fingerprint: String,
    pub owner: String,
    pub lease_token: String,
    pub fence_epoch: i64,
    pub target_generation: String,
    pub claim_token: String,
    pub claim_expires_at: i64,
    pub items: Vec<ProjectionDelivery>,
}

impl fmt::Debug for ProjectionBatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionBatch")
            .field("store_name", &self.store_name)
            .field("database_instance_id", &self.database_instance_id)
            .field("protocol_version", &self.protocol_version)
            .field("schema_version", &self.schema_version)
            .field("provider", &self.provider)
            .field("provider_fingerprint", &self.provider_fingerprint)
            .field("owner", &self.owner)
            .field("lease_token", &"[REDACTED]")
            .field("fence_epoch", &self.fence_epoch)
            .field("target_generation", &self.target_generation)
            .field("claim_token", &"[REDACTED]")
            .field("claim_expires_at", &self.claim_expires_at)
            .field("item_count", &self.items.len())
            .finish()
    }
}

/// In-process receipt mirror used to validate a request batch.
///
/// This DTO contains opaque capabilities because the current SQLite service
/// receipt does. It is not a helper stdout payload; the subprocess response
/// uses [`VectorProjectionBatchApplicationReceipt`] and never echoes either
/// token.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionBatchReceipt {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub provider: String,
    pub provider_fingerprint: String,
    pub target_generation: String,
    pub lease_token: String,
    pub fence_epoch: i64,
    pub claim_token: String,
    pub applied_item_count: usize,
}

impl fmt::Debug for ProjectionBatchReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionBatchReceipt")
            .field("store_name", &self.store_name)
            .field("database_instance_id", &self.database_instance_id)
            .field("protocol_version", &self.protocol_version)
            .field("schema_version", &self.schema_version)
            .field("provider", &self.provider)
            .field("provider_fingerprint", &self.provider_fingerprint)
            .field("target_generation", &self.target_generation)
            .field("lease_token", &"[REDACTED]")
            .field("fence_epoch", &self.fence_epoch)
            .field("claim_token", &"[REDACTED]")
            .field("applied_item_count", &self.applied_item_count)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionStoreDescriptor {
    pub store_name: String,
    pub schema_version: i64,
    pub provider: String,
    pub provider_fingerprint: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "RequiredNullableSchema<ProjectionCorpusMetadata>")
    )]
    pub corpus: Option<ProjectionCorpusMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionPublishReceipt {
    pub active: ProjectionArtifactEvidence,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "RequiredNullableSchema<ProjectionArtifactEvidence>")
    )]
    pub retained_previous: Option<ProjectionArtifactEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VectorProjectionHelperOperation {
    Descriptor,
    PrepareSnapshot,
    ApplyBatch,
    Publish,
    InspectActive,
    InspectGeneration,
    ValidateGenerationPublication,
    ValidateActiveContents,
    RepairPublication,
    Quarantine,
    Abort,
    Inventory,
    Cleanup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionDescriptorRequest {
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionMutationContext {
    pub request_id: String,
    pub projection_store: String,
    pub generation_id: String,
    pub delivery_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionMutationAck {
    pub request_id: String,
    pub projection_store: String,
    pub generation_id: String,
    pub delivery_digest: String,
}

pub type VectorProjectionPrepareMetadata = ProjectionCorpusMetadata;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionPrepareSnapshotRequest {
    pub context: VectorProjectionMutationContext,
    pub snapshot: ProjectionSnapshot,
    pub metadata: VectorProjectionPrepareMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionApplyBatchRequest {
    pub context: VectorProjectionMutationContext,
    pub batch: ProjectionBatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionPublishRequest {
    pub context: VectorProjectionMutationContext,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "RequiredNullableSchema<ProjectionArtifactEvidence>")
    )]
    pub expected_active: Option<ProjectionArtifactEvidence>,
    pub prepared: ProjectionArtifactEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionInspectActiveRequest {
    pub request_id: String,
    pub projection_store: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionInspectGenerationRequest {
    pub request_id: String,
    pub projection_store: String,
    pub generation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionValidateGenerationRequest {
    pub request_id: String,
    pub projection_store: String,
    pub expected: ProjectionArtifactEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionValidateActiveRequest {
    pub request_id: String,
    pub projection_store: String,
    pub active: ProjectionArtifactEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionRepairPublicationRequest {
    pub context: VectorProjectionMutationContext,
    pub expected: ProjectionArtifactEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionGenerationMutationRequest {
    pub context: VectorProjectionMutationContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionInventoryRequest {
    pub request_id: String,
    pub projection_store: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionCleanupProtection {
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub active_generation: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub previous_generation: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub building_generation: Option<String>,
    pub additional_generations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionCleanupRequest {
    pub context: VectorProjectionMutationContext,
    pub dry_run: bool,
    pub protection: VectorProjectionCleanupProtection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "operation",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum VectorProjectionHelperRequest {
    Descriptor(VectorProjectionDescriptorRequest),
    PrepareSnapshot(VectorProjectionPrepareSnapshotRequest),
    ApplyBatch(VectorProjectionApplyBatchRequest),
    Publish(Box<VectorProjectionPublishRequest>),
    InspectActive(VectorProjectionInspectActiveRequest),
    InspectGeneration(VectorProjectionInspectGenerationRequest),
    ValidateGenerationPublication(VectorProjectionValidateGenerationRequest),
    ValidateActiveContents(VectorProjectionValidateActiveRequest),
    RepairPublication(VectorProjectionRepairPublicationRequest),
    Quarantine(VectorProjectionGenerationMutationRequest),
    Abort(VectorProjectionGenerationMutationRequest),
    Inventory(VectorProjectionInventoryRequest),
    Cleanup(VectorProjectionCleanupRequest),
}

impl VectorProjectionHelperRequest {
    pub const fn operation(&self) -> VectorProjectionHelperOperation {
        match self {
            Self::Descriptor(_) => VectorProjectionHelperOperation::Descriptor,
            Self::PrepareSnapshot(_) => VectorProjectionHelperOperation::PrepareSnapshot,
            Self::ApplyBatch(_) => VectorProjectionHelperOperation::ApplyBatch,
            Self::Publish(_) => VectorProjectionHelperOperation::Publish,
            Self::InspectActive(_) => VectorProjectionHelperOperation::InspectActive,
            Self::InspectGeneration(_) => VectorProjectionHelperOperation::InspectGeneration,
            Self::ValidateGenerationPublication(_) => {
                VectorProjectionHelperOperation::ValidateGenerationPublication
            }
            Self::ValidateActiveContents(_) => {
                VectorProjectionHelperOperation::ValidateActiveContents
            }
            Self::RepairPublication(_) => VectorProjectionHelperOperation::RepairPublication,
            Self::Quarantine(_) => VectorProjectionHelperOperation::Quarantine,
            Self::Abort(_) => VectorProjectionHelperOperation::Abort,
            Self::Inventory(_) => VectorProjectionHelperOperation::Inventory,
            Self::Cleanup(_) => VectorProjectionHelperOperation::Cleanup,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionHelperDescriptor {
    pub request_id: String,
    pub protocol_version: i64,
    pub build_identity: String,
    pub supported_stores: Vec<ProjectionStoreDescriptor>,
    pub supported_operations: Vec<VectorProjectionHelperOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionPrepareSnapshotResponse {
    pub ack: VectorProjectionMutationAck,
    pub evidence: ProjectionArtifactEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionApplyBatchResponse {
    pub ack: VectorProjectionMutationAck,
    pub receipt: VectorProjectionBatchApplicationReceipt,
}

/// Capability-free batch application evidence returned over helper stdout.
///
/// The caller reconstructs its in-process [`ProjectionBatchReceipt`] from the
/// original request plus this evidence, so lease and claim tokens never cross
/// the stdout boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionBatchApplicationReceipt {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub provider: String,
    pub provider_fingerprint: String,
    pub target_generation: String,
    pub fence_epoch: i64,
    pub applied_item_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionPublishResponse {
    pub ack: VectorProjectionMutationAck,
    pub receipt: ProjectionPublishReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionInspectActiveResponse {
    pub request_id: String,
    pub projection_store: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "RequiredNullableSchema<ProjectionArtifactEvidence>")
    )]
    pub active: Option<ProjectionArtifactEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionInspectGenerationResponse {
    pub request_id: String,
    pub projection_store: String,
    pub generation_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "RequiredNullableSchema<ProjectionArtifactEvidence>")
    )]
    pub evidence: Option<ProjectionArtifactEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionValidationResponse {
    pub request_id: String,
    pub projection_store: String,
    pub valid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VectorProjectionGenerationState {
    Active,
    Previous,
    Building,
    Prepared,
    Quarantined,
    Orphaned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionGenerationInventoryEntry {
    pub generation_id: String,
    pub state: VectorProjectionGenerationState,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "RequiredNullableSchema<ProjectionArtifactEvidence>")
    )]
    pub evidence: Option<ProjectionArtifactEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionInventoryResponse {
    pub request_id: String,
    pub projection_store: String,
    pub generations: Vec<VectorProjectionGenerationInventoryEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VectorProjectionProtectionReason {
    Active,
    Previous,
    Building,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionProtectedGeneration {
    pub generation_id: String,
    pub reason: VectorProjectionProtectionReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionSkippedGeneration {
    pub generation_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionCleanupResponse {
    pub ack: VectorProjectionMutationAck,
    pub dry_run: bool,
    pub removed_generations: Vec<String>,
    pub protected_generations: Vec<VectorProjectionProtectedGeneration>,
    pub skipped_generations: Vec<VectorProjectionSkippedGeneration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionHelperError {
    pub kind: VectorProjectionHelperErrorKind,
    pub code: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub provider: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub backend: Option<String>,
    pub retryable: bool,
    pub message: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub request_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub delivery_digest: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub projection_store: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub generation_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VectorProjectionHelperErrorKind {
    Provider,
    Backend,
    Delivery,
    Protocol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "operation",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum VectorProjectionHelperResponse {
    Descriptor(VectorProjectionHelperDescriptor),
    PrepareSnapshot(VectorProjectionPrepareSnapshotResponse),
    ApplyBatch(VectorProjectionApplyBatchResponse),
    Publish(Box<VectorProjectionPublishResponse>),
    InspectActive(VectorProjectionInspectActiveResponse),
    InspectGeneration(VectorProjectionInspectGenerationResponse),
    ValidateGenerationPublication(VectorProjectionValidationResponse),
    ValidateActiveContents(VectorProjectionValidationResponse),
    RepairPublication(VectorProjectionMutationAck),
    Quarantine(VectorProjectionMutationAck),
    Abort(VectorProjectionMutationAck),
    Inventory(VectorProjectionInventoryResponse),
    Cleanup(VectorProjectionCleanupResponse),
    Error(VectorProjectionHelperError),
}

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

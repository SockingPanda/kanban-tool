//! Project configuration and derived-helper subprocess wire contracts.
//!
//! These DTOs own only the decoded TOML/JSON shape. Path resolution, provider
//! availability, derived-store algorithms and dispatcher policy remain in the
//! runtime adapters that consume them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Schema-only marker for a key that is required but may contain JSON null.
#[cfg(feature = "schema")]
#[derive(schemars::JsonSchema)]
#[serde(transparent)]
struct RequiredNullableSchema<T>(Option<T>);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectConfigInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector: Option<ProjectVectorConfigInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectVectorConfigInput {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub dimensions: usize,
}

impl Default for ProjectVectorConfigInput {
    fn default() -> Self {
        Self {
            provider: "ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            model: "qwen3-embedding:0.6b".to_owned(),
            dimensions: 1024,
        }
    }
}

/// Strict decoded shape for the single `[workers.<profile>]` section selected by the CLI.
/// Selection and opaque handling of other profile sections remain adapter concerns.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WorkerProfileInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_ttl_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_interval_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_success: Option<WorkerFinishPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<WorkerFinishPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WorkerFinishPolicy {
    Done,
    Review,
    Blocked,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphHelperHandshakeResponse {
    pub helper: String,
    pub protocol: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphHelperErrorResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphHelperStatusResponse {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
}

pub type GraphHelperRebuildResponse = GraphHelperStatusResponse;
pub type GraphHelperSyncResponse = GraphHelperStatusResponse;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphHelperRelationProvenance {
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub source_table: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub source_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub source_event_id: Option<i64>,
    pub authoritative_store: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphHelperRelation {
    pub subject_uri: String,
    pub predicate: String,
    pub object_uri: String,
    pub graph_uri: String,
    pub provenance: GraphHelperRelationProvenance,
    pub metadata: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
}

pub type GraphHelperNeighborsResponse = Vec<GraphHelperRelation>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphHelperQueryBinding {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphHelperQueryRow {
    pub bindings: Vec<GraphHelperQueryBinding>,
}

pub type GraphHelperQueryResponse = Vec<GraphHelperQueryRow>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorHelperHandshakeResponse {
    pub helper: String,
    pub protocol: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorHelperErrorResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorHelperCheckProviderResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorHelperStatusResponse {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<bool>"))]
    pub dirty: Option<bool>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<bool>"))]
    pub board_dirty: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<i64>,
}

pub type VectorHelperRebuildResponse = VectorHelperStatusResponse;
pub type VectorHelperSyncResponse = VectorHelperStatusResponse;
pub type VectorHelperLabelAtomsStatusResponse = VectorHelperStatusResponse;
pub type VectorHelperRebuildLabelAtomsResponse = VectorHelperStatusResponse;
pub type VectorHelperSyncLabelAtomsResponse = VectorHelperStatusResponse;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorHelperChunkRef {
    pub uri: String,
    pub entity_uri: String,
    pub ordinal: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorHelperChunkHit {
    pub chunk: VectorHelperChunkRef,
    pub score: f32,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub text: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub summary: Option<String>,
}

pub type VectorHelperQueryChunksResponse = Vec<VectorHelperChunkHit>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorHelperLabelAtomHit {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub board_id: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorHelperLabelAtomVectorHit {
    pub hit: VectorHelperLabelAtomHit,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "RequiredNullableSchema<Vec<f32>>")
    )]
    pub vector: Option<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum VectorHelperQueryLabelAtomsItem {
    Hit(VectorHelperLabelAtomHit),
    WithVector(VectorHelperLabelAtomVectorHit),
}

pub type VectorHelperQueryLabelAtomsResponse = Vec<VectorHelperQueryLabelAtomsItem>;
pub type VectorHelperEmbedQueryResponse = Vec<f32>;

/// Canonical host vector status query。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorStatusQuery {
    #[serde(default = "default_vector_board")]
    pub board: String,
}

fn default_vector_board() -> String {
    "default".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorConfigureRequest {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub dimensions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionRequest {
    #[serde(default = "default_vector_board")]
    pub board: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorQuery {
    #[serde(default = "default_vector_board")]
    pub board: String,
    pub q: String,
    #[serde(default = "default_vector_limit")]
    pub limit: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polarity: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub include_vector: bool,
}

fn default_vector_limit() -> usize {
    20
}
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorChunkResult {
    pub id: String,
    pub entity_uri: Option<String>,
    pub source_kind: String,
    pub content: String,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f32,
    pub score: f32,
}

pub type VectorConfigureResponse = crate::DataEnvelope<VectorConfigureRequest>;
pub type VectorProjectionResponse = crate::DataEnvelope<crate::VectorStatus>;
pub type VectorQueryChunksResponse = crate::DataEnvelope<Vec<VectorChunkResult>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorLabelAtomResult {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub board_id: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
}

pub type VectorQueryLabelAtomsResponse = crate::DataEnvelope<Vec<VectorLabelAtomResult>>;

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

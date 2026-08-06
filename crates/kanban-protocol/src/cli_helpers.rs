//! CLI graph、vector、context、search 与 index 机器契约。
//!
//! 每个 alias 对应一个精确 Clap leaf 的有限 `--json` output root；运行时 adapter
//! 只负责把 domain record 映射到这些 wire DTO。

use serde::{Deserialize, Serialize};

use crate::{ContextPack, DataEnvelope, MetadataEnvelope, SearchMeta, SearchStatus, SearchTaskHit};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliGraphStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
}

pub type CliGraphStatusOutput = DataEnvelope<CliGraphStatus>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliGraphMaintenance {
    pub mode: String,
    pub board_id: String,
    pub generation: String,
    pub fingerprint: String,
    pub validated_tasks: i64,
    pub validated_entities: i64,
    pub validated_relations: i64,
    pub pending_jobs: i64,
    pub consumed_jobs: i64,
    pub updated_at: i64,
    pub message: String,
}

pub type CliGraphRebuildOutput = DataEnvelope<CliGraphMaintenance>;
pub type CliGraphSyncOutput = DataEnvelope<CliGraphMaintenance>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliGraphRelationProvenance {
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub source_table: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub source_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub source_event_id: Option<i64>,
    pub authoritative_store: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliGraphRelation {
    pub subject_uri: String,
    pub predicate: String,
    pub object_uri: String,
    pub graph_uri: String,
    pub provenance: CliGraphRelationProvenance,
    pub metadata: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
}

pub type CliGraphNeighborsOutput = DataEnvelope<Vec<CliGraphRelation>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliGraphQueryBinding {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliGraphQueryRow {
    pub bindings: Vec<CliGraphQueryBinding>,
}

pub type CliGraphQueryOutput = DataEnvelope<Vec<CliGraphQueryRow>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliVectorConfig {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub dimensions: usize,
}

pub type CliVectorConfigureOutput = DataEnvelope<CliVectorConfig>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliVectorStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    pub diagnostics: Vec<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_bool_schema")
    )]
    pub dirty: Option<bool>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_bool_schema")
    )]
    pub board_dirty: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<i64>,
}

pub type CliVectorStatusOutput = DataEnvelope<CliVectorStatus>;
pub type CliVectorRebuildOutput = DataEnvelope<CliVectorStatus>;
pub type CliVectorSyncOutput = DataEnvelope<CliVectorStatus>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliChunkRef {
    pub uri: String,
    pub entity_uri: String,
    pub ordinal: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliVectorChunkHit {
    pub chunk: CliChunkRef,
    pub score: f32,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub text: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub summary: Option<String>,
}

pub type CliVectorQueryChunksOutput = DataEnvelope<Vec<CliVectorChunkHit>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliLabelAtomHit {
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
pub struct CliLabelAtomVectorHit {
    pub hit: CliLabelAtomHit,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_f32_vec_schema")
    )]
    pub vector: Option<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum CliVectorLabelAtomHit {
    Hit(CliLabelAtomHit),
    WithVector(CliLabelAtomVectorHit),
}

pub type CliVectorQueryLabelAtomsOutput = DataEnvelope<Vec<CliVectorLabelAtomHit>>;

pub type CliContextBuildOutput = DataEnvelope<ContextPack>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliSearchData {
    pub hits: Vec<SearchTaskHit>,
}

pub type CliSearchOutput = MetadataEnvelope<CliSearchData, SearchMeta>;
pub type CliIndexRebuildOutput = DataEnvelope<SearchStatus>;
pub type CliIndexSyncOutput = DataEnvelope<SearchStatus>;

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[cfg(feature = "schema")]
fn required_nullable_string_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<String>>()
}

#[cfg(feature = "schema")]
fn required_nullable_i64_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<i64>>()
}

#[cfg(feature = "schema")]
fn required_nullable_bool_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<bool>>()
}

#[cfg(feature = "schema")]
fn required_nullable_f32_vec_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<Vec<f32>>>()
}

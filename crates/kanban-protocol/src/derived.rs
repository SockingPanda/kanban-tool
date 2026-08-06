use serde::{Deserialize, Serialize};

use crate::{
    ApiTask, ApiTaskStatus, CliEntityListOutput, CliEntityShowOutput, DataEnvelope,
    MetadataEnvelope, OffsetPaginationMeta,
};
use serde::de::{self, Visitor};

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

#[cfg(feature = "schema")]
fn required_nullable_string_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    g.subschema_for::<Option<String>>()
}
#[cfg(feature = "schema")]
fn required_nullable_i64_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    g.subschema_for::<Option<i64>>()
}
#[cfg(feature = "schema")]
fn required_nullable_f64_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    g.subschema_for::<Option<f64>>()
}
#[cfg(feature = "schema")]
fn required_nullable_usize_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    g.subschema_for::<Option<usize>>()
}
#[cfg(feature = "schema")]
fn required_nullable_bool_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    g.subschema_for::<Option<bool>>()
}

fn default_board() -> String {
    "default".to_owned()
}
fn default_search_limit() -> usize {
    20
}
fn default_context_lexical_limit() -> usize {
    5
}
fn default_context_graph_limit() -> usize {
    10
}
fn default_context_vector_limit() -> usize {
    5
}
fn default_context_max_items() -> usize {
    20
}
fn default_context_depth() -> usize {
    1
}
fn default_events_limit() -> usize {
    100
}
fn default_graph_neighbors_limit() -> usize {
    50
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BoardQuery {
    #[serde(default = "default_board")]
    pub board: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchTasksQuery {
    #[serde(default = "default_board")]
    pub board: String,
    pub q: Option<String>,
    #[serde(default)]
    pub status: Vec<ApiTaskStatus>,
    #[serde(default)]
    pub label: Vec<String>,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub assignee: Option<String>,
}

impl<'de> Deserialize<'de> for SearchTasksQuery {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SearchTasksQueryVisitor;

        impl<'de> Visitor<'de> for SearchTasksQueryVisitor {
            type Value = SearchTasksQuery;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("search task query parameters")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut board = None;
                let mut q = None;
                let mut status = Vec::new();
                let mut label = Vec::new();
                let mut include_archived = None;
                let mut limit = None;
                let mut offset = None;
                let mut assignee = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "board" => {
                            if board.is_some() {
                                return Err(de::Error::duplicate_field("board"));
                            }
                            board = Some(map.next_value()?);
                        }
                        "q" => {
                            if q.is_some() {
                                return Err(de::Error::duplicate_field("q"));
                            }
                            q = Some(map.next_value()?);
                        }
                        "status" => match map.next_value::<OneOrMany<ApiTaskStatus>>()? {
                            OneOrMany::One(value) => status.push(value),
                            OneOrMany::Many(values) => status.extend(values),
                        },
                        "label" => match map.next_value::<OneOrMany<String>>()? {
                            OneOrMany::One(value) => label.push(value),
                            OneOrMany::Many(values) => label.extend(values),
                        },
                        "include_archived" => {
                            if include_archived.is_some() {
                                return Err(de::Error::duplicate_field("include_archived"));
                            }
                            include_archived = Some(map.next_value()?);
                        }
                        "limit" => {
                            if limit.is_some() {
                                return Err(de::Error::duplicate_field("limit"));
                            }
                            limit = Some(map.next_value()?);
                        }
                        "offset" => {
                            if offset.is_some() {
                                return Err(de::Error::duplicate_field("offset"));
                            }
                            offset = Some(map.next_value()?);
                        }
                        "assignee" => {
                            if assignee.is_some() {
                                return Err(de::Error::duplicate_field("assignee"));
                            }
                            assignee = Some(map.next_value()?);
                        }
                        other => {
                            return Err(de::Error::unknown_field(
                                other,
                                &[
                                    "board",
                                    "q",
                                    "status",
                                    "label",
                                    "include_archived",
                                    "limit",
                                    "offset",
                                    "assignee",
                                ],
                            ));
                        }
                    }
                }

                Ok(SearchTasksQuery {
                    board: board.unwrap_or_else(default_board),
                    q: q.flatten(),
                    status,
                    label,
                    include_archived: include_archived.unwrap_or(false),
                    limit: limit.unwrap_or_else(default_search_limit),
                    offset: offset.unwrap_or(0),
                    assignee: assignee.flatten(),
                })
            }
        }

        deserializer.deserialize_map(SearchTasksQueryVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BuildContextPath {
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BuildContextQuery {
    #[serde(default = "default_board")]
    pub board: String,
    #[serde(default = "default_context_lexical_limit")]
    pub lexical_limit: usize,
    #[serde(default = "default_context_graph_limit")]
    pub graph_limit: usize,
    #[serde(default = "default_context_vector_limit")]
    pub vector_limit: usize,
    #[serde(default = "default_context_max_items")]
    pub max_items: usize,
    /// 可选的全局任务 ID。省略时由 `reference` 或 `query` 选择上下文主体。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// 看板内任务引用，例如 `kanban-tool#12`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// 用于词法/向量主体解析的自由文本查询。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default = "default_context_depth")]
    pub depth: usize,
    /// 条目总预算。`max_items` 仍作为 v1 别名接受。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListEventsQuery {
    #[serde(default = "default_board")]
    pub board: String,
    pub task_id: Option<String>,
    #[serde(default)]
    pub after: i64,
    #[serde(default = "default_events_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchMeta {
    pub backend: String,
    pub stale: bool,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub database_instance_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub protocol_version: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub generation: Option<String>,
    pub resolved_board_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub fallback_reason: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub index_version: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub last_event_id: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub index_lag_events: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchTaskHit {
    pub task_id: String,
    pub seq: i64,
    pub score: f64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub snippet: Option<String>,
    pub task: ApiTask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchTasksData {
    pub hits: Vec<SearchTaskHit>,
    pub meta: SearchMeta,
}
pub type SearchTasksResponse = MetadataEnvelope<SearchTasksData, OffsetPaginationMeta>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchPageMeta {
    pub limit: usize,
    pub offset: usize,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_usize_schema")
    )]
    pub total: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchTaskStatusWindow {
    pub status: ApiTaskStatus,
    pub tasks: Vec<ApiTask>,
    pub search_meta: SearchMeta,
    pub page: SearchPageMeta,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchTaskStatusWindows {
    pub statuses: Vec<SearchTaskStatusWindow>,
}
pub type SearchTasksByStatusResponse =
    MetadataEnvelope<SearchTaskStatusWindows, OffsetPaginationMeta>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchStatus {
    pub backend: String,
    pub derived_index: bool,
    pub stale: bool,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub database_instance_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub protocol_version: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub generation: Option<String>,
    pub resolved_board_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub fallback_reason: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub index_version: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub last_event_id: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub index_lag_events: Option<i64>,
    pub message: String,
}
pub type SearchStatusResponse = DataEnvelope<SearchStatus>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ContextPolicy {
    #[serde(default = "default_context_depth")]
    pub depth: usize,
    pub lexical_limit: usize,
    pub graph_limit: usize,
    pub vector_limit: usize,
    pub max_items: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ContextItem {
    pub entity_uri: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_f64_schema")
    )]
    pub score: Option<f64>,
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
    pub snippet: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub rank: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<ContextEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ContextEvidence {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ContextDiagnostic {
    pub source: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ContextPack {
    pub subject: String,
    pub policy: ContextPolicy,
    pub items: Vec<ContextItem>,
    pub degraded: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ContextDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<ContextProviderStatus>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ContextProviderStatus {
    pub provider: String,
    pub capability: String,
    pub available: bool,
    pub degraded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
pub type BuildContextResponse = DataEnvelope<ContextPack>;

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StatusCount {
    pub status: ApiTaskStatus,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StaleClaim {
    pub task_id: String,
    pub seq: i64,
    pub title: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub claim_owner: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub claim_expires_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub last_heartbeat_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub current_run_id: Option<String>,
    pub retry_count: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub max_retries: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BlockedReasonCount {
    pub reason: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct QueueStats {
    pub board_id: String,
    pub generated_at: i64,
    pub status_counts: Vec<StatusCount>,
    pub stale_claims: Vec<StaleClaim>,
    pub blocked_reasons: Vec<BlockedReasonCount>,
    pub unplanned_active_tasks: i64,
    pub active_parents_with_incomplete_required_steps: i64,
}
pub type StatsResponse = DataEnvelope<QueueStats>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
}
pub type GraphStatusResponse = DataEnvelope<GraphStatus>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct GraphQueryQuery {
    pub board: String,
    pub query: String,
    pub limit: usize,
}

impl Default for GraphQueryQuery {
    fn default() -> Self {
        Self {
            board: default_board(),
            query: "SELECT ?subject ?predicate ?object WHERE { ?subject ?predicate ?object }"
                .to_owned(),
            limit: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphMaintenance {
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

pub type GraphMaintenanceResponse = DataEnvelope<GraphMaintenance>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct EntityListQuery {
    pub board: Option<String>,
    pub kind: Option<String>,
    pub limit: usize,
}

impl Default for EntityListQuery {
    fn default() -> Self {
        Self {
            board: None,
            kind: None,
            limit: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EntityPath {
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EntityUpsertRequest {
    pub uri: String,
    pub kind: String,
    pub source_table: String,
    pub source_id: String,
    pub board: Option<String>,
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub archived_at: Option<i64>,
}

pub type EntityListResponse = CliEntityListOutput;
pub type EntityResponse = CliEntityShowOutput;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphNeighborsQuery {
    #[serde(default = "default_board")]
    pub board: String,
    pub entity_uri: String,
    pub predicate: Option<String>,
    #[serde(default = "default_graph_neighbors_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiRelationProvenance {
    pub source_table: Option<String>,
    pub source_id: Option<String>,
    pub source_event_id: Option<i64>,
    pub authoritative_store: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiRelation {
    pub subject_uri: String,
    pub predicate: String,
    pub object_uri: String,
    pub graph_uri: String,
    pub provenance: ApiRelationProvenance,
    pub metadata: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
}

pub type GraphNeighborsResponse = MetadataEnvelope<Vec<ApiRelation>, crate::LimitMeta>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    #[serde(default)]
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
pub type VectorStatusResponse = DataEnvelope<VectorStatus>;

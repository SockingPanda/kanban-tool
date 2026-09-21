//! CLI/MCP 的具名模块与迭代 DTO；不暴露内部对象属性或存储类型。
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiObjectVersion {
    pub object: i64,
    pub source: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiPlanningReceipt {
    pub request_id: String,
    pub ids: Vec<String>,
    pub event_sequence: i64,
    pub catalog_version: i64,
    pub replayed: bool,
    pub versions: BTreeMap<String, ApiObjectVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum ApiCycleStatus {
    Planned,
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum PlanningReadSource {
    Current,
    FrozenSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PlanningBoardPath {
    pub board: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PlanningObjectPath {
    pub board: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PlanningMemberPath {
    pub board: String,
    pub id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiModule {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    pub body: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiModuleSummary {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    pub parent_id: Option<String>,
}

pub type GetModuleResponse = crate::DataEnvelope<ApiModule>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListModulesResponse {
    pub data: Vec<ApiModuleSummary>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ModuleMutationData {
    pub object: ApiModule,
    pub receipt: ApiPlanningReceipt,
}

pub type ModuleMutationResponse = crate::DataEnvelope<ModuleMutationData>;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ModuleOverview {
    pub object: ApiModule,
    pub progress: ApiPlanningProgress,
    pub source: PlanningReadSource,
    pub captured_at: Option<i64>,
}

pub type ModuleOverviewResponse = crate::DataEnvelope<ModuleOverview>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListModulesQuery {
    pub q: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default = "default_planning_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateModuleRequest {
    pub title: String,
    pub body: Option<String>,
    pub parent_id: Option<String>,
    pub request_id: Option<String>,
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expected_versions: BTreeMap<String, ApiObjectVersion>,
    pub expected_catalog_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateModuleRequest {
    #[serde(
        default,
        deserialize_with = "crate::task_core::deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::task_core::deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub body: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::task_core::deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub parent_id: Option<Option<String>>,
    pub request_id: Option<String>,
    pub actor: Option<String>,
    pub expected_version: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expected_versions: BTreeMap<String, ApiObjectVersion>,
    pub expected_catalog_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiCycle {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    pub body: Option<String>,
    pub status: ApiCycleStatus,
    pub starts_at: i64,
    pub ends_at: i64,
    pub started_at: Option<i64>,
    pub closed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiCycleSummary {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    pub status: ApiCycleStatus,
    pub starts_at: i64,
    pub ends_at: i64,
    pub started_at: Option<i64>,
    pub closed_at: Option<i64>,
}

pub type GetCycleResponse = crate::DataEnvelope<ApiCycle>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListCyclesResponse {
    pub data: Vec<ApiCycleSummary>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CycleMutationData {
    pub object: ApiCycle,
    pub receipt: ApiPlanningReceipt,
}

pub type CycleMutationResponse = crate::DataEnvelope<CycleMutationData>;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CycleOverview {
    pub object: ApiCycle,
    pub progress: ApiPlanningProgress,
    pub source: PlanningReadSource,
    pub captured_at: Option<i64>,
}

pub type CycleOverviewResponse = crate::DataEnvelope<CycleOverview>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListCyclesQuery {
    pub q: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default = "default_planning_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub status: Option<ApiCycleStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateCycleRequest {
    pub title: String,
    pub body: Option<String>,
    pub starts_at: i64,
    pub ends_at: i64,
    pub request_id: Option<String>,
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expected_versions: BTreeMap<String, ApiObjectVersion>,
    pub expected_catalog_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateCycleRequest {
    #[serde(
        default,
        deserialize_with = "crate::task_core::deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::task_core::deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub body: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::task_core::deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub starts_at: Option<i64>,
    #[serde(
        default,
        deserialize_with = "crate::task_core::deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub ends_at: Option<i64>,
    pub request_id: Option<String>,
    pub actor: Option<String>,
    pub expected_version: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expected_versions: BTreeMap<String, ApiObjectVersion>,
    pub expected_catalog_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PlanningActionRequest {
    pub request_id: Option<String>,
    pub actor: Option<String>,
    pub expected_version: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expected_versions: BTreeMap<String, ApiObjectVersion>,
    pub expected_catalog_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CloseCycleRequest {
    pub carry_to: Option<String>,
    pub request_id: Option<String>,
    pub actor: Option<String>,
    pub expected_version: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expected_versions: BTreeMap<String, ApiObjectVersion>,
    pub expected_catalog_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PlanningMembersQuery {
    #[serde(default = "default_planning_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiPlanningMember {
    pub id: String,
    pub title: String,
    pub status: crate::ApiTaskStatus,
    pub version: ApiObjectVersion,
    pub carried_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PlanningMembersResponse {
    pub data: Vec<ApiPlanningMember>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub source: PlanningReadSource,
    pub captured_at: Option<i64>,
    pub object_version: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiPlanningProgress {
    pub total: i64,
    pub done: i64,
    pub archived: i64,
    pub blocked: i64,
    pub completion_ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PlanningCapabilities {
    pub version: u32,
    pub task_membership: bool,
    pub task_membership_filters: bool,
}

pub type PlanningCapabilitiesResponse = crate::DataEnvelope<PlanningCapabilities>;

fn default_planning_limit() -> usize {
    100
}

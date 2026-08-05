use crate::{OffsetPaginationMeta, TotalPaginationMeta};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, de};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApiTaskStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
    Running,
    Blocked,
    Review,
    Done,
    Archived,
}

impl ApiTaskStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Triage => "triage",
            Self::Todo => "todo",
            Self::Scheduled => "scheduled",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::Review => "review",
            Self::Done => "done",
            Self::Archived => "archived",
        }
    }
}

impl FromStr for ApiTaskStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "triage" => Ok(Self::Triage),
            "todo" => Ok(Self::Todo),
            "scheduled" => Ok(Self::Scheduled),
            "ready" => Ok(Self::Ready),
            "running" => Ok(Self::Running),
            "blocked" => Ok(Self::Blocked),
            "review" => Ok(Self::Review),
            "done" => Ok(Self::Done),
            "archived" => Ok(Self::Archived),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApiExecutionPlanState {
    Unplanned,
    Planned,
    NotRequired,
}

impl ApiExecutionPlanState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unplanned => "unplanned",
            Self::Planned => "planned",
            Self::NotRequired => "not_required",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct ApiTaskPriority(#[cfg_attr(feature = "schema", schemars(range(min = 0, max = 3)))] u8);

impl<'de> Deserialize<'de> for ApiTaskPriority {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = i64::deserialize(deserializer)?;
        Self::try_from(value).map_err(|value| {
            de::Error::custom(format!("task priority must be in 0..=3, got {value}"))
        })
    }
}

impl ApiTaskPriority {
    pub const fn new(value: u8) -> Option<Self> {
        if value <= 3 { Some(Self(value)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Default for ApiTaskPriority {
    fn default() -> Self {
        Self(3)
    }
}

impl TryFrom<i64> for ApiTaskPriority {
    type Error = i64;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match u8::try_from(value).ok().and_then(Self::new) {
            Some(priority) => Ok(priority),
            None => Err(value),
        }
    }
}

#[cfg(feature = "schema")]
fn required_nullable_string_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<String>>()
}

#[cfg(feature = "schema")]
fn required_nullable_value_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<serde_json::Value>>()
}

#[cfg(feature = "schema")]
fn required_nullable_i64_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<i64>>()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiLabel {
    pub id: String,
    pub board_id: String,
    pub name: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub color: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiTask {
    pub id: String,
    pub board_id: String,
    pub board_slug: String,
    #[serde(rename = "ref")]
    pub task_ref: String,
    pub seq: i64,
    pub title: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub description: Option<String>,
    pub status: ApiTaskStatus,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub status_reason: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub assignee: Option<String>,
    pub priority: ApiTaskPriority,
    pub position: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub scheduled_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub due_at: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub started_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub completed_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub archived_at: Option<i64>,
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
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub result_summary: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_value_schema")
    )]
    pub result: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
    pub lock_version: i64,
    pub dependency_blocked: bool,
    pub unfinished_parent_count: i64,
    pub execution_plan_state: ApiExecutionPlanState,
    pub required_step_count: i64,
    pub completed_required_step_count: i64,
    pub optional_step_count: i64,
    pub labels: Vec<ApiLabel>,
}

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListTasksResponse {
    pub data: Vec<ApiTask>,
    pub meta: TotalPaginationMeta,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListTasksByStatusResponse {
    pub data: ListTasksByStatusData,
    pub meta: OffsetPaginationMeta,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListTasksByStatusData {
    pub statuses: Vec<ListTasksStatusWindow>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListTasksStatusWindow {
    pub status: ApiTaskStatus,
    pub tasks: Vec<ApiTask>,
    pub page: TotalPaginationMeta,
}

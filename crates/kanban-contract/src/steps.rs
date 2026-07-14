use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApiStepStatus {
    Todo,
    Done,
    Skipped,
}
impl ApiStepStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Done => "done",
            Self::Skipped => "skipped",
        }
    }
}
macro_rules! task_path {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub task_id: String,
        }
    };
}
macro_rules! step_path {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub task_id: String,
            pub step_id: String,
        }
    };
}
task_path!(ListStepsPath);
task_path!(CreateStepPath);
task_path!(MarkExecutionPlanNotRequiredPath);
step_path!(UpdateStepPath);
step_path!(RemoveStepPath);
step_path!(CompleteStepPath);
step_path!(SkipStepPath);
step_path!(ReopenStepPath);
fn default_required() -> bool {
    true
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateStepRequest {
    pub title: String,
    pub body: Option<String>,
    pub linked_task_ref: Option<String>,
    pub position: Option<i64>,
    #[serde(default = "default_required")]
    pub required: bool,
    pub actor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateStepRequest {
    pub title: Option<String>,
    pub body: Option<String>,
    pub linked_task_ref: Option<String>,
    #[serde(default)]
    pub unlink_task: bool,
    pub position: Option<i64>,
    pub required: Option<bool>,
    pub actor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompleteStepRequest {
    pub note: String,
    pub actor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SkipStepRequest {
    pub reason: String,
    pub actor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReopenStepRequest {
    pub reason: String,
    pub actor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarkExecutionPlanNotRequiredRequest {
    pub reason: String,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiTaskStep {
    pub id: String,
    pub parent_task_id: String,
    pub title: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub body: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_task_schema")
    )]
    pub linked_task: Option<crate::ApiTask>,
    pub position: i64,
    pub required: bool,
    pub status: ApiStepStatus,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub resolution_note: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub resolved_by: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub resolved_at: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_by: String,
    pub updated_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiExecutionPlan {
    pub board_id: String,
    pub task_id: String,
    pub state: crate::ApiExecutionPlanState,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub reason: Option<String>,
    pub updated_by: String,
    pub updated_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiTaskSteps {
    pub task_id: String,
    pub steps: Vec<ApiTaskStep>,
    pub execution_plan: ApiExecutionPlan,
}
macro_rules! steps_response {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub data: ApiTaskSteps,
        }
    };
}
steps_response!(ListStepsResponse);
steps_response!(CreateStepResponse);
steps_response!(UpdateStepResponse);
steps_response!(RemoveStepResponse);
steps_response!(CompleteStepResponse);
steps_response!(SkipStepResponse);
steps_response!(ReopenStepResponse);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarkExecutionPlanNotRequiredResponse {
    pub data: ApiExecutionPlan,
}
fn deserialize_required_nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
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
fn required_nullable_task_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    g.subschema_for::<Option<crate::ApiTask>>()
}

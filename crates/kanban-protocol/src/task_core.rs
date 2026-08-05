use serde::{Deserialize, Serialize};

use crate::{
    ApiComment, ApiDependencies, ApiExecutionPlan, ApiLabel, ApiRun, ApiTask, ApiTaskStep,
    DataEnvelope, OptionalMetadataEnvelope, StreamEventData, TaskOntologyDetailsMeta,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GetTaskPath {
    pub task_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct GetTaskQuery {
    pub include: Option<String>,
}

/// Derived ontology metadata remains opaque at the transport boundary.
pub type GetTaskResponse =
    OptionalMetadataEnvelope<ApiTask, TaskOntologyDetailsMeta<Option<TaskOntologySummary>>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskOntologySummary {
    pub task_id: String,
    pub observation_count: i64,
    pub signal_count: i64,
    pub open_count: i64,
    pub confirmed_count: i64,
    pub resolved_count: i64,
    pub rejected_count: i64,
    pub superseded_count: i64,
    pub degraded_count: i64,
    pub stale_count: i64,
    pub suggest_input_drift_count: i64,
    pub legacy_incomparable_count: i64,
    pub incomparable_count: i64,
    pub action_count: i64,
    pub oldest_open_confirmed_signal_at: Option<i64>,
    pub oldest_open_confirmed_signal_age_ms: Option<i64>,
    pub latest_signal_at: Option<i64>,
    pub latest_action_at: Option<i64>,
    pub current_suggest_input_hash: String,
    pub sample_signals: Vec<TaskOntologySignalSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskOntologySignalSummary {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub proposed_action: String,
    pub target_label_id: Option<String>,
    pub target_label_name: Option<String>,
    pub candidate_atom_polarity: Option<String>,
    pub candidate_atom_kind: Option<String>,
    pub candidate_text: Option<String>,
    pub candidate_content_hash: Option<String>,
    pub proposed_label_name: Option<String>,
    pub proposed_label_name_normalized: Option<String>,
    pub suggest_score: Option<f64>,
    pub suggest_rank: Option<i64>,
    pub degraded: bool,
    pub stale: bool,
    pub legacy_incomparable: bool,
    pub suggest_input_drift: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub latest_action_at: Option<i64>,
    pub action_count: i64,
}

/// Explicit opt-in task detail aggregate. The default task show response stays
/// intentionally small; this shape is used by `include=details`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskDetailAggregate {
    pub task: ApiTask,
    pub labels: Vec<ApiLabel>,
    pub dependencies: ApiDependencies,
    pub execution_plan: ApiExecutionPlan,
    pub steps: Vec<ApiTaskStep>,
    pub comments: Vec<ApiComment>,
    pub runs: Vec<ApiRun>,
    pub events: Vec<StreamEventData>,
    pub ontology: TaskDetailOntology,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskDetailOntology {
    pub summary: Option<TaskOntologySummary>,
    pub degraded: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GetTaskDetailsResponse {
    pub data: TaskDetailAggregate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateTaskPath {
    pub task_id: String,
}

/// Explicitly writable PATCH fields. Canonical status, claim credentials,
/// current run identity, and completion timestamp are intentionally absent.
fn deserialize_patch_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

fn deserialize_patch_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateTaskRequest {
    #[serde(
        default,
        deserialize_with = "deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub title: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub assignee: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "schema", schemars(with = "i64"))]
    pub priority: Option<i64>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub scheduled_at: Option<Option<i64>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub due_at: Option<Option<i64>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_retries: Option<Option<i64>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub metadata: Option<Option<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "schema", schemars(with = "i64"))]
    pub expected_lock_version: Option<i64>,
}

pub type UpdateTaskResponse = DataEnvelope<ApiTask>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_task_query_defaults_and_rejects_unknown_keys() {
        assert_eq!(GetTaskQuery::default().include, None);
        let parsed: GetTaskQuery = serde_json::from_value(json!({})).expect("default query");
        assert_eq!(parsed, GetTaskQuery::default());
        assert!(
            serde_json::from_value::<GetTaskQuery>(json!({"include":"ontology","x":1})).is_err()
        );
    }

    #[test]
    fn update_request_has_only_safe_mutation_fields() {
        let request: UpdateTaskRequest = serde_json::from_value(json!({
            "title": "new title",
            "description": null,
            "priority": 1,
            "metadata": {"source": "fixture"},
            "expected_lock_version": 7
        }))
        .expect("valid patch");
        assert_eq!(request.title.as_deref(), Some("new title"));
        assert_eq!(request.description, Some(None));
        assert_eq!(request.priority, Some(1));
        assert_eq!(request.metadata, Some(Some(json!({"source": "fixture"}))));
        assert_eq!(request.expected_lock_version, Some(7));

        let cleared: UpdateTaskRequest =
            serde_json::from_value(json!({"metadata": null})).expect("nullable metadata patch");
        assert_eq!(cleared.metadata, Some(None));
        let omitted: UpdateTaskRequest =
            serde_json::from_value(json!({})).expect("missing metadata patch");
        assert_eq!(omitted.metadata, None);

        for forbidden in [
            "status",
            "claim_token",
            "current_run_id",
            "completed_at",
            "metadata_json",
        ] {
            assert!(
                serde_json::from_value::<UpdateTaskRequest>(json!({forbidden: "forbidden"}))
                    .is_err(),
                "{forbidden}"
            );
        }
        for non_nullable in ["title", "priority", "expected_lock_version"] {
            assert!(
                serde_json::from_value::<UpdateTaskRequest>(json!({non_nullable: null})).is_err(),
                "{non_nullable} must reject explicit null"
            );
        }
    }

    #[test]
    fn response_aliases_keep_envelope_boundaries() {
        let _ = std::any::type_name::<GetTaskResponse>();
        let _ = std::any::type_name::<UpdateTaskResponse>();
    }
}

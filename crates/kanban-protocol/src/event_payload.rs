//! Typed known-event payloads and the lossless unknown-kind fallback.

use serde::{Deserialize, Serialize};
use serde_json::Value;

macro_rules! payload {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $name { $(pub $field: $ty),* }
    };
}

payload!(EmptyPayload {});
payload!(BoardCreatedPayload { slug: String });
payload!(DependencyPayload {
    parent_task_id: String
});
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LabelCreatedPayload {
    pub label_id: String,
    pub label: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub color: Option<String>,
}
payload!(LabelDeletedPayload {
    label_id: String,
    label: String,
    forced: bool,
    removed_task_bindings: usize,
    removed_semantics: bool,
    removed_atoms: usize,
});
payload!(SignalRecordedPayload {
    signal_id: String,
    observation_id: String,
    kind: String,
    status: SignalStatus,
});
payload!(SignalReviewedPayload {
    signal_id: String,
    status: SignalStatus,
    reason: String,
});
payload!(TaskReasonPayload { reason: String });
payload!(TaskClaimedPayload {
    claim_owner: String,
    metadata: Value,
});
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskCommentCreatedPayload {
    pub comment_id: String,
    pub kind: CommentKind,
    pub author_type: CommentAuthorType,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub agent_type: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskResultPayload {
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_value_schema")
    )]
    pub result: Option<Value>,
}
payload!(TaskStatusPayload { status: TaskStatus });
payload!(TaskToStatusPayload {
    to_status: TaskStatus
});
payload!(ExecutionPlanPayload {
    state: ExecutionPlanState
});
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HeartbeatPayload {
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub note: Option<String>,
}
payload!(TaskLabelPayload {
    label_id: String,
    label: String
});
payload!(LabelProposalPayload {
    proposal_id: String,
    name: String,
    status: LabelProposalStatus,
});
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskReclaimedPayload {
    pub retry_count: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub max_retries: Option<i64>,
    pub to_status: TaskStatus,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskRetryPayload {
    pub retry_count: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub max_retries: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskReopenedPayload {
    pub from: TaskStatus,
    pub to: TaskStatus,
    pub reason: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub original_completed_at: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RetryPolicyPayload {
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub max_retries: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskStepPayload {
    pub step_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub linked_task_id: Option<String>,
    pub position: i64,
    pub required: bool,
    pub status: StepStatus,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskExportSanitizedPayload {
    pub from_status: TaskStatus,
    pub to_status: TaskStatus,
    pub run_status: RunStatus,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub original_run_id: Option<String>,
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
    pub reason: String,
}

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
fn required_nullable_value_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<Value>>()
}

fn payload_mismatch(message: impl std::fmt::Display) -> serde_json::Error {
    <serde_json::Error as serde::de::Error>::custom(message)
}

fn execution_plan_payload(
    value: Value,
    expected: ExecutionPlanState,
) -> Result<EventPayload, serde_json::Error> {
    let payload = serde_json::from_value::<ExecutionPlanPayload>(value)?;
    if payload.state != expected {
        return Err(payload_mismatch(format!(
            "execution plan event requires state {expected:?}"
        )));
    }
    Ok(EventPayload::ExecutionPlan(payload))
}

fn label_proposal_payload(
    value: Value,
    expected: LabelProposalStatus,
) -> Result<EventPayload, serde_json::Error> {
    let payload = serde_json::from_value::<LabelProposalPayload>(value)?;
    if payload.status != expected {
        return Err(payload_mismatch(format!(
            "label proposal event requires status {expected:?}"
        )));
    }
    Ok(EventPayload::LabelProposal(payload))
}

fn task_step_payload(
    value: Value,
    expected: Option<StepStatus>,
) -> Result<EventPayload, serde_json::Error> {
    let payload = serde_json::from_value::<TaskStepPayload>(value)?;
    if expected.is_some_and(|expected| payload.status != expected) {
        return Err(payload_mismatch(format!(
            "task step event requires status {expected:?}"
        )));
    }
    Ok(EventPayload::TaskStep(payload))
}

pub const KNOWN_EVENT_KINDS: &[&str] = &[
    "board.created",
    "board.archived",
    "dependency.added",
    "dependency.removed",
    "label.created",
    "label.deleted",
    "signal.recorded",
    "signal.reviewed",
    "task.archived",
    "task.blocked",
    "task.claimed",
    "task.comment.created",
    "task.completed",
    "task.created",
    "task.execution_plan.not_required",
    "task.execution_plan.planned",
    "task.execution_plan.unplanned",
    "task.heartbeat",
    "task.label.added",
    "task.label.removed",
    "task.label_proposal.accepted",
    "task.label_proposal.proposed",
    "task.label_proposal.rejected",
    "task.promoted",
    "task.reclaimed",
    "task.recomputed",
    "task.released",
    "task.reopened",
    "task.retry_policy.updated",
    "task.specified",
    "task.step.created",
    "task.step.done",
    "task.step.removed",
    "task.step.reopened",
    "task.step.skipped",
    "task.step.updated",
    "task.submitted_for_review",
    "task.unblocked",
    "task.updated",
    "task.export_sanitized",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Todo,
    Done,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPlanState {
    Planned,
    NotRequired,
    Unplanned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SignalStatus {
    Open,
    Confirmed,
    Rejected,
    Superseded,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LabelProposalStatus {
    Proposed,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CommentKind {
    Note,
    Decision,
    Signal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CommentAuthorType {
    User,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum EventPayload {
    Empty(EmptyPayload),
    BoardCreated(BoardCreatedPayload),
    Dependency(DependencyPayload),
    LabelCreated(LabelCreatedPayload),
    LabelDeleted(LabelDeletedPayload),
    SignalRecorded(SignalRecordedPayload),
    SignalReviewed(SignalReviewedPayload),
    TaskReason(TaskReasonPayload),
    TaskClaimed(TaskClaimedPayload),
    TaskCommentCreated(TaskCommentCreatedPayload),
    TaskResult(TaskResultPayload),
    TaskStatus(TaskStatusPayload),
    TaskToStatus(TaskToStatusPayload),
    ExecutionPlan(ExecutionPlanPayload),
    Heartbeat(HeartbeatPayload),
    TaskLabel(TaskLabelPayload),
    LabelProposal(LabelProposalPayload),
    TaskReclaimed(TaskReclaimedPayload),
    TaskRetry(TaskRetryPayload),
    TaskReopened(TaskReopenedPayload),
    RetryPolicy(RetryPolicyPayload),
    TaskStep(TaskStepPayload),
    TaskExportSanitized(TaskExportSanitizedPayload),
    Unknown(Value),
}

impl EventPayload {
    pub fn from_kind_and_value(kind: &str, value: Value) -> Result<Self, serde_json::Error> {
        macro_rules! decode {
            ($variant:ident, $ty:ty) => {
                serde_json::from_value::<$ty>(value).map(Self::$variant)
            };
        }
        match kind {
            "board.created" => serde_json::from_value::<BoardCreatedPayload>(value.clone())
                .map(Self::BoardCreated)
                .or_else(|_| decode!(Empty, EmptyPayload)),
            "board.archived" | "task.archived" | "task.updated" => {
                decode!(Empty, EmptyPayload)
            }
            "dependency.added" | "dependency.removed" => {
                decode!(Dependency, DependencyPayload)
            }
            "label.created" => decode!(LabelCreated, LabelCreatedPayload),
            "label.deleted" => decode!(LabelDeleted, LabelDeletedPayload),
            "signal.recorded" => decode!(SignalRecorded, SignalRecordedPayload),
            "signal.reviewed" => decode!(SignalReviewed, SignalReviewedPayload),
            "task.blocked" => serde_json::from_value::<TaskReasonPayload>(value.clone())
                .map(Self::TaskReason)
                .or_else(|_| {
                    serde_json::from_value::<TaskRetryPayload>(value.clone()).map(Self::TaskRetry)
                })
                .or_else(|_| decode!(TaskResult, TaskResultPayload)),
            "task.claimed" => decode!(TaskClaimed, TaskClaimedPayload),
            "task.comment.created" => decode!(TaskCommentCreated, TaskCommentCreatedPayload),
            "task.completed" | "task.submitted_for_review" => {
                decode!(TaskResult, TaskResultPayload)
            }
            "task.created" => decode!(TaskStatus, TaskStatusPayload),
            "task.promoted" | "task.recomputed" | "task.released" | "task.specified"
            | "task.unblocked" => {
                decode!(TaskToStatus, TaskToStatusPayload)
            }
            "task.execution_plan.not_required" => {
                execution_plan_payload(value, ExecutionPlanState::NotRequired)
            }
            "task.execution_plan.planned" => {
                execution_plan_payload(value, ExecutionPlanState::Planned)
            }
            "task.execution_plan.unplanned" => {
                execution_plan_payload(value, ExecutionPlanState::Unplanned)
            }
            "task.heartbeat" => decode!(Heartbeat, HeartbeatPayload),
            "task.label.added" | "task.label.removed" => decode!(TaskLabel, TaskLabelPayload),
            "task.label_proposal.accepted" => {
                label_proposal_payload(value, LabelProposalStatus::Accepted)
            }
            "task.label_proposal.proposed" => {
                label_proposal_payload(value, LabelProposalStatus::Proposed)
            }
            "task.label_proposal.rejected" => {
                label_proposal_payload(value, LabelProposalStatus::Rejected)
            }
            "task.reclaimed" => serde_json::from_value::<TaskReclaimedPayload>(value.clone())
                .map(Self::TaskReclaimed)
                .or_else(|_| decode!(TaskRetry, TaskRetryPayload)),
            "task.reopened" => decode!(TaskReopened, TaskReopenedPayload),
            "task.retry_policy.updated" => decode!(RetryPolicy, RetryPolicyPayload),
            "task.step.created" | "task.step.reopened" => {
                task_step_payload(value, Some(StepStatus::Todo))
            }
            "task.step.done" => task_step_payload(value, Some(StepStatus::Done)),
            "task.step.skipped" => task_step_payload(value, Some(StepStatus::Skipped)),
            "task.step.removed" | "task.step.updated" => task_step_payload(value, None),
            "task.export_sanitized" => {
                decode!(TaskExportSanitized, TaskExportSanitizedPayload)
            }
            _ => Ok(Self::Unknown(value)),
        }
    }
}

impl PartialEq<Value> for EventPayload {
    fn eq(&self, other: &Value) -> bool {
        serde_json::to_value(self).is_ok_and(|value| value == *other)
    }
}

impl PartialEq<EventPayload> for Value {
    fn eq(&self, other: &EventPayload) -> bool {
        other == self
    }
}

#[cfg(test)]
mod tests {
    use super::EventPayload;
    use crate::StreamEventData;
    use serde_json::{Value, json};

    const KNOWN_EVENT_KINDS: &[(&str, &str)] = &[
        ("board.created", r#"{"slug":"default"}"#),
        ("board.archived", "{}"),
        ("dependency.added", r#"{"parent_task_id":"t_parent"}"#),
        ("dependency.removed", r#"{"parent_task_id":"t_parent"}"#),
        (
            "label.created",
            r#"{"label_id":"l_1","label":"cli","color":null}"#,
        ),
        (
            "label.deleted",
            r#"{"label_id":"l_1","label":"cli","forced":false,"removed_task_bindings":0,"removed_semantics":false,"removed_atoms":0}"#,
        ),
        (
            "signal.recorded",
            r#"{"signal_id":"sig_1","observation_id":"obs_1","kind":"bug","status":"open"}"#,
        ),
        (
            "signal.reviewed",
            r#"{"signal_id":"sig_1","status":"confirmed","reason":"verified"}"#,
        ),
        ("task.archived", "{}"),
        ("task.blocked", r#"{"reason":"waiting"}"#),
        (
            "task.claimed",
            r#"{"claim_owner":"worker","metadata":{"lane":"test"}}"#,
        ),
        (
            "task.comment.created",
            r#"{"comment_id":"c_1","kind":"note","author_type":"user","agent_type":null}"#,
        ),
        ("task.completed", r#"{"result":{"ok":true}}"#),
        ("task.created", r#"{"status":"todo"}"#),
        (
            "task.execution_plan.not_required",
            r#"{"state":"not_required"}"#,
        ),
        ("task.execution_plan.planned", r#"{"state":"planned"}"#),
        ("task.execution_plan.unplanned", r#"{"state":"unplanned"}"#),
        ("task.heartbeat", r#"{"note":null}"#),
        ("task.label.added", r#"{"label_id":"l_1","label":"cli"}"#),
        ("task.label.removed", r#"{"label_id":"l_1","label":"cli"}"#),
        (
            "task.label_proposal.accepted",
            r#"{"proposal_id":"lp_1","name":"cli","status":"accepted"}"#,
        ),
        (
            "task.label_proposal.proposed",
            r#"{"proposal_id":"lp_1","name":"cli","status":"proposed"}"#,
        ),
        (
            "task.label_proposal.rejected",
            r#"{"proposal_id":"lp_1","name":"cli","status":"rejected"}"#,
        ),
        ("task.promoted", r#"{"to_status":"ready"}"#),
        (
            "task.reclaimed",
            r#"{"retry_count":1,"max_retries":3,"to_status":"ready","reason":"expired"}"#,
        ),
        ("task.recomputed", r#"{"to_status":"todo"}"#),
        ("task.released", r#"{"to_status":"ready"}"#),
        (
            "task.reopened",
            r#"{"from":"done","to":"ready","reason":"follow-up","original_completed_at":123}"#,
        ),
        ("task.retry_policy.updated", r#"{"max_retries":3}"#),
        ("task.specified", r#"{"to_status":"todo"}"#),
        (
            "task.step.created",
            r#"{"step_id":"s_1","linked_task_id":null,"position":0,"required":true,"status":"todo"}"#,
        ),
        (
            "task.step.done",
            r#"{"step_id":"s_1","linked_task_id":null,"position":0,"required":true,"status":"done"}"#,
        ),
        (
            "task.step.removed",
            r#"{"step_id":"s_1","linked_task_id":null,"position":0,"required":true,"status":"todo"}"#,
        ),
        (
            "task.step.reopened",
            r#"{"step_id":"s_1","linked_task_id":null,"position":0,"required":true,"status":"todo"}"#,
        ),
        (
            "task.step.skipped",
            r#"{"step_id":"s_1","linked_task_id":null,"position":0,"required":true,"status":"skipped"}"#,
        ),
        (
            "task.step.updated",
            r#"{"step_id":"s_1","linked_task_id":null,"position":0,"required":false,"status":"todo"}"#,
        ),
        ("task.submitted_for_review", r#"{"result":null}"#),
        ("task.unblocked", r#"{"to_status":"ready"}"#),
        ("task.updated", "{}"),
        (
            "task.export_sanitized",
            r#"{"from_status":"running","to_status":"ready","run_status":"canceled","original_run_id":null,"claim_owner":null,"claim_expires_at":null,"reason":"jsonl export clears non-portable live claim"}"#,
        ),
    ];

    #[test]
    fn all_40_real_event_kinds_have_bounded_payloads() {
        assert_eq!(KNOWN_EVENT_KINDS.len(), 40);
        assert_eq!(
            KNOWN_EVENT_KINDS
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<std::collections::BTreeSet<_>>(),
            super::KNOWN_EVENT_KINDS
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
        );
        for (kind, raw) in KNOWN_EVENT_KINDS {
            let value: Value = serde_json::from_str(raw).unwrap();
            let payload = EventPayload::from_kind_and_value(kind, value.clone())
                .unwrap_or_else(|error| panic!("{kind}: {error}"));
            assert_eq!(serde_json::to_value(payload).unwrap(), value, "{kind}");
            assert!(
                EventPayload::from_kind_and_value(kind, json!({"fabricated": true})).is_err(),
                "known kind must reject wrong shape: {kind}"
            );
        }
    }

    #[test]
    fn unknown_event_kind_is_lossless_but_known_kind_mismatch_fails() {
        let unknown = json!({"nested": [1, {"keep": true}], "opaque": "value"});
        let payload =
            EventPayload::from_kind_and_value("plugin.future.event", unknown.clone()).unwrap();
        assert_eq!(serde_json::to_value(payload).unwrap(), unknown);

        let event = json!({
            "id": 1, "event_id": "e_1", "board_id": "b_1", "task_id": null,
            "run_id": null, "kind": "task.created", "actor": null,
            "payload": {"status": "not-a-status"}, "created_at": 1
        });
        assert!(serde_json::from_value::<StreamEventData>(event).is_err());
    }

    #[test]
    fn retry_payloads_accept_required_nullable_max_retries() {
        for kind in ["task.blocked", "task.reclaimed"] {
            let value = json!({"retry_count": 1, "max_retries": null});
            let payload = EventPayload::from_kind_and_value(kind, value.clone()).unwrap();
            assert_eq!(serde_json::to_value(payload).unwrap(), value, "{kind}");
            assert!(
                EventPayload::from_kind_and_value(kind, json!({"retry_count": 1})).is_err(),
                "missing max_retries must fail: {kind}"
            );
        }
    }

    #[test]
    fn producer_present_nullable_payload_fields_require_explicit_null() {
        let cases = [
            (
                "label.created",
                json!({"label_id": "l_1", "label": "cli", "color": null}),
                "color",
            ),
            (
                "task.comment.created",
                json!({"comment_id": "c_1", "kind": "note", "author_type": "user", "agent_type": null}),
                "agent_type",
            ),
            ("task.completed", json!({"result": null}), "result"),
            ("task.heartbeat", json!({"note": null}), "note"),
            (
                "task.reopened",
                json!({"from": "done", "to": "ready", "reason": "follow-up", "original_completed_at": null}),
                "original_completed_at",
            ),
            (
                "task.step.created",
                json!({"step_id": "s_1", "linked_task_id": null, "position": 0, "required": true, "status": "todo"}),
                "linked_task_id",
            ),
            (
                "task.retry_policy.updated",
                json!({"max_retries": null}),
                "max_retries",
            ),
        ];

        for (kind, value, nullable_field) in cases {
            EventPayload::from_kind_and_value(kind, value.clone())
                .unwrap_or_else(|error| panic!("explicit null rejected for {kind}: {error}"));
            let mut missing = value;
            missing
                .as_object_mut()
                .expect("payload is an object")
                .remove(nullable_field);
            assert!(
                EventPayload::from_kind_and_value(kind, missing).is_err(),
                "missing {nullable_field} must fail for {kind}"
            );
        }
    }

    #[test]
    fn stream_outer_nullable_fields_require_explicit_null() {
        let event = json!({
            "id": 1, "event_id": "e_1", "board_id": "b_1", "task_id": null,
            "run_id": null, "kind": "task.created", "actor": null,
            "payload": {"status": "todo"}, "created_at": 1
        });
        serde_json::from_value::<StreamEventData>(event.clone()).unwrap();
        for field in ["task_id", "run_id", "actor"] {
            let mut missing = event.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<StreamEventData>(missing).is_err(),
                "missing {field} must fail"
            );
        }
    }

    #[test]
    fn signal_recorded_observation_id_is_required_and_non_null() {
        let valid = json!({
            "signal_id": "sig_1", "observation_id": "obs_1", "kind": "bug", "status": "open"
        });
        EventPayload::from_kind_and_value("signal.recorded", valid.clone()).unwrap();
        for invalid in [
            json!({"signal_id": "sig_1", "kind": "bug", "status": "open"}),
            json!({"signal_id": "sig_1", "observation_id": null, "kind": "bug", "status": "open"}),
        ] {
            assert!(EventPayload::from_kind_and_value("signal.recorded", invalid).is_err());
        }
    }

    #[test]
    fn kind_suffix_and_payload_state_must_correlate_exactly() {
        let mismatches = [
            ("task.execution_plan.planned", json!({"state": "unplanned"})),
            (
                "task.execution_plan.not_required",
                json!({"state": "planned"}),
            ),
            (
                "task.label_proposal.accepted",
                json!({"proposal_id": "lp_1", "name": "cli", "status": "proposed"}),
            ),
            (
                "task.label_proposal.rejected",
                json!({"proposal_id": "lp_1", "name": "cli", "status": "accepted"}),
            ),
            (
                "task.step.done",
                json!({"step_id": "s_1", "linked_task_id": null, "position": 0, "required": true, "status": "todo"}),
            ),
            (
                "task.step.skipped",
                json!({"step_id": "s_1", "linked_task_id": null, "position": 0, "required": true, "status": "done"}),
            ),
            (
                "task.step.reopened",
                json!({"step_id": "s_1", "linked_task_id": null, "position": 0, "required": true, "status": "skipped"}),
            ),
        ];
        for (kind, payload) in mismatches {
            assert!(
                EventPayload::from_kind_and_value(kind, payload).is_err(),
                "mismatched payload must fail for {kind}"
            );
        }
    }

    #[cfg(feature = "schema")]
    #[test]
    fn schemas_require_producer_present_nullable_fields_and_exact_siblings() {
        use super::{
            HeartbeatPayload, LabelCreatedPayload, RetryPolicyPayload, TaskCommentCreatedPayload,
            TaskReopenedPayload, TaskResultPayload, TaskStepPayload,
        };

        fn assert_required<T: schemars::JsonSchema>(field: &str) {
            let schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
            assert!(
                schema["required"]
                    .as_array()
                    .is_some_and(|required| required.iter().any(|name| name == field)),
                "schema must require {field}: {schema}"
            );
            let property = &schema["properties"][field];
            assert!(
                schema_allows_null(property),
                "schema must allow explicit null for {field}: {schema}"
            );
        }
        fn schema_allows_null(schema: &Value) -> bool {
            match schema {
                Value::Bool(value) => *value,
                Value::Object(object) => {
                    let nullable_type = match object.get("type") {
                        Some(Value::String(kind)) => kind == "null",
                        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == "null"),
                        _ => false,
                    };
                    let nullable_const = object.get("const") == Some(&Value::Null);
                    let nullable_enum = object
                        .get("enum")
                        .and_then(Value::as_array)
                        .is_some_and(|values| values.contains(&Value::Null));
                    let nullable_union = ["anyOf", "oneOf"].into_iter().any(|keyword| {
                        object
                            .get(keyword)
                            .and_then(Value::as_array)
                            .is_some_and(|branches| branches.iter().any(schema_allows_null))
                    });
                    let nullable_intersection = object
                        .get("allOf")
                        .and_then(Value::as_array)
                        .filter(|branches| !branches.is_empty())
                        .is_some_and(|branches| branches.iter().all(schema_allows_null));
                    nullable_type
                        || nullable_const
                        || nullable_enum
                        || nullable_union
                        || nullable_intersection
                }
                _ => false,
            }
        }
        assert_required::<LabelCreatedPayload>("color");
        assert_required::<TaskCommentCreatedPayload>("agent_type");
        assert_required::<TaskResultPayload>("result");
        assert_required::<HeartbeatPayload>("note");
        assert_required::<TaskReopenedPayload>("original_completed_at");
        assert_required::<TaskStepPayload>("linked_task_id");
        assert_required::<RetryPolicyPayload>("max_retries");

        let schema = serde_json::to_value(schemars::schema_for!(StreamEventData)).unwrap();
        let branches = schema["oneOf"].as_array().expect("stream schema oneOf");
        for (kind, expected) in [
            ("task.execution_plan.planned", "planned"),
            ("task.execution_plan.not_required", "not_required"),
            ("task.execution_plan.unplanned", "unplanned"),
            ("task.label_proposal.accepted", "accepted"),
            ("task.label_proposal.proposed", "proposed"),
            ("task.label_proposal.rejected", "rejected"),
            ("task.step.created", "todo"),
            ("task.step.reopened", "todo"),
            ("task.step.done", "done"),
            ("task.step.skipped", "skipped"),
        ] {
            let branch = branches
                .iter()
                .find(|branch| branch["properties"]["kind"]["const"] == kind)
                .unwrap_or_else(|| panic!("missing schema branch for {kind}"));
            assert!(
                branch["properties"]["payload"]
                    .to_string()
                    .contains(&format!("\"const\":\"{expected}\"")),
                "schema branch must bind {kind} to {expected}: {branch}"
            );
            for field in ["task_id", "run_id", "actor"] {
                assert!(
                    branch["required"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|v| v == field)
                );
            }
        }
    }
}

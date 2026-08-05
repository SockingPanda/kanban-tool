use serde::{Deserialize, Serialize};

use crate::event_payload::EventPayload;

fn default_board() -> String {
    "default".to_owned()
}

fn default_limit() -> usize {
    100
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StreamEventsQuery {
    #[serde(default = "default_board")]
    pub board: String,
    pub task_id: Option<String>,
    #[serde(default)]
    pub after: i64,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StreamEventData {
    pub id: i64,
    pub event_id: String,
    pub board_id: String,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub kind: String,
    pub actor: Option<String>,
    pub payload: EventPayload,
    pub created_at: i64,
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for StreamEventData {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "StreamEventData".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        use crate::event_payload::*;

        fn schema_for<T: schemars::JsonSchema>(
            generator: &mut schemars::SchemaGenerator,
        ) -> serde_json::Value {
            serde_json::to_value(generator.subschema_for::<T>()).expect("schema serializes")
        }
        fn union(values: Vec<serde_json::Value>) -> serde_json::Value {
            serde_json::json!({"oneOf": values})
        }
        fn with_const(schema: serde_json::Value, field: &str, value: &str) -> serde_json::Value {
            serde_json::json!({
                "allOf": [
                    schema,
                    {"type": "object", "required": [field], "properties": {field: {"const": value}}}
                ]
            })
        }
        fn branch(kind: &str, payload: serde_json::Value) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "event_id", "board_id", "task_id", "run_id", "kind", "actor", "payload", "created_at"],
                "properties": {
                    "id": {"type": "integer"},
                    "event_id": {"type": "string"},
                    "board_id": {"type": "string"},
                    "task_id": {"type": ["string", "null"]},
                    "run_id": {"type": ["string", "null"]},
                    "kind": {"const": kind},
                    "actor": {"type": ["string", "null"]},
                    "payload": payload,
                    "created_at": {"type": "integer"}
                }
            })
        }

        let mut branches = Vec::with_capacity(KNOWN_EVENT_KINDS.len() + 1);
        for kind in KNOWN_EVENT_KINDS {
            let payload = match *kind {
                "board.created" => union(vec![
                    schema_for::<BoardCreatedPayload>(generator),
                    schema_for::<EmptyPayload>(generator),
                ]),
                "board.archived" | "task.archived" | "task.updated" => {
                    schema_for::<EmptyPayload>(generator)
                }
                "dependency.added" | "dependency.removed" => {
                    schema_for::<DependencyPayload>(generator)
                }
                "label.created" => schema_for::<LabelCreatedPayload>(generator),
                "label.deleted" => schema_for::<LabelDeletedPayload>(generator),
                "signal.recorded" => schema_for::<SignalRecordedPayload>(generator),
                "signal.reviewed" => schema_for::<SignalReviewedPayload>(generator),
                "task.blocked" => union(vec![
                    schema_for::<TaskReasonPayload>(generator),
                    schema_for::<TaskRetryPayload>(generator),
                    schema_for::<TaskResultPayload>(generator),
                ]),
                "task.claimed" => schema_for::<TaskClaimedPayload>(generator),
                "task.comment.created" => schema_for::<TaskCommentCreatedPayload>(generator),
                "task.completed" | "task.submitted_for_review" => {
                    schema_for::<TaskResultPayload>(generator)
                }
                "task.created" => schema_for::<TaskStatusPayload>(generator),
                "task.promoted" | "task.recomputed" | "task.specified" | "task.unblocked" => {
                    schema_for::<TaskToStatusPayload>(generator)
                }
                "task.released" => with_const(
                    schema_for::<TaskToStatusPayload>(generator),
                    "to_status",
                    "ready",
                ),
                "task.execution_plan.not_required" => with_const(
                    schema_for::<ExecutionPlanPayload>(generator),
                    "state",
                    "not_required",
                ),
                "task.execution_plan.planned" => with_const(
                    schema_for::<ExecutionPlanPayload>(generator),
                    "state",
                    "planned",
                ),
                "task.execution_plan.unplanned" => with_const(
                    schema_for::<ExecutionPlanPayload>(generator),
                    "state",
                    "unplanned",
                ),
                "task.heartbeat" => schema_for::<HeartbeatPayload>(generator),
                "task.label.added" | "task.label.removed" => {
                    schema_for::<TaskLabelPayload>(generator)
                }
                "task.label_proposal.accepted" => with_const(
                    schema_for::<LabelProposalPayload>(generator),
                    "status",
                    "accepted",
                ),
                "task.label_proposal.proposed" => with_const(
                    schema_for::<LabelProposalPayload>(generator),
                    "status",
                    "proposed",
                ),
                "task.label_proposal.rejected" => with_const(
                    schema_for::<LabelProposalPayload>(generator),
                    "status",
                    "rejected",
                ),
                "task.reclaimed" => union(vec![
                    schema_for::<TaskReclaimedPayload>(generator),
                    schema_for::<TaskRetryPayload>(generator),
                ]),
                "task.reopened" => schema_for::<TaskReopenedPayload>(generator),
                "task.retry_policy.updated" => schema_for::<RetryPolicyPayload>(generator),
                "task.step.created" | "task.step.reopened" => {
                    with_const(schema_for::<TaskStepPayload>(generator), "status", "todo")
                }
                "task.step.done" => {
                    with_const(schema_for::<TaskStepPayload>(generator), "status", "done")
                }
                "task.step.skipped" => with_const(
                    schema_for::<TaskStepPayload>(generator),
                    "status",
                    "skipped",
                ),
                "task.step.removed" | "task.step.updated" => {
                    schema_for::<TaskStepPayload>(generator)
                }
                "task.export_sanitized" => schema_for::<TaskExportSanitizedPayload>(generator),
                _ => unreachable!("known event kind table and schema match must stay aligned"),
            };
            branches.push(branch(kind, payload));
        }
        branches.push(serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "event_id", "board_id", "task_id", "run_id", "kind", "actor", "payload", "created_at"],
            "properties": {
                "id": {"type": "integer"},
                "event_id": {"type": "string"},
                "board_id": {"type": "string"},
                "task_id": {"type": ["string", "null"]},
                "run_id": {"type": ["string", "null"]},
                "kind": {"type": "string", "not": {"enum": KNOWN_EVENT_KINDS}},
                "actor": {"type": ["string", "null"]},
                "payload": true,
                "created_at": {"type": "integer"}
            }
        }));
        serde_json::json!({"oneOf": branches})
            .try_into()
            .expect("StreamEventData schema is an object")
    }
}

impl<'de> Deserialize<'de> for StreamEventData {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            id: i64,
            event_id: String,
            board_id: String,
            #[serde(deserialize_with = "deserialize_required_nullable")]
            task_id: Option<String>,
            #[serde(deserialize_with = "deserialize_required_nullable")]
            run_id: Option<String>,
            kind: String,
            #[serde(deserialize_with = "deserialize_required_nullable")]
            actor: Option<String>,
            payload: serde_json::Value,
            created_at: i64,
        }
        let raw = Raw::deserialize(deserializer)?;
        let payload = EventPayload::from_kind_and_value(&raw.kind, raw.payload)
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            id: raw.id,
            event_id: raw.event_id,
            board_id: raw.board_id,
            task_id: raw.task_id,
            run_id: raw.run_id,
            kind: raw.kind,
            actor: raw.actor,
            payload,
            created_at: raw.created_at,
        })
    }
}

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

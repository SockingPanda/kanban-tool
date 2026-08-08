use serde::{Deserialize, Serialize};

use crate::event_payload::EventPayload;

/// JavaScript 可以无损表示的非负 SSE cursor 上限。
pub const MAX_SAFE_EVENT_CURSOR: i64 = 9_007_199_254_740_991;

/// transport-control heartbeat 的精确 SSE event name。
pub const SSE_HEARTBEAT_EVENT: &str = "kb-heartbeat";

/// 业务 SSE envelope 的 canonical 顶层字段顺序。
pub const STREAM_EVENT_ENVELOPE_FIELDS: &[&str] = &[
    "id",
    "event_id",
    "board_id",
    "task_id",
    "run_id",
    "kind",
    "actor",
    "payload",
    "created_at",
];

/// 需要非空 `task_id` 的精确 task-scoped known kind 集合。
pub const TASK_SCOPED_EVENT_KINDS: &[&str] = &[
    "dependency.added",
    "dependency.removed",
    "task.archived",
    "task.blocked",
    "task.claimed",
    "task.comment.created",
    "task.completed",
    "task.created",
    "task.execution_plan.not_required",
    "task.execution_plan.planned",
    "task.execution_plan.unplanned",
    "task.export_sanitized",
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
];

/// 连接保活 frame 的 typed data。它没有业务 cursor 或 envelope 字段。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SseHeartbeatData {}

/// `/api/v1/stream/events` 的 exact request headers。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct StreamEventsHeaders {
    #[serde(rename = "Accept-Language")]
    pub accept_language: Option<String>,
    #[serde(rename = "Last-Event-ID")]
    pub last_event_id: Option<String>,
}

/// 校验非负、且可被浏览器安全整数无损表示的 cursor。
pub fn validate_event_cursor(value: i64) -> Result<i64, &'static str> {
    if value < 0 {
        return Err("cursor 不能为负数");
    }
    if value > MAX_SAFE_EVENT_CURSOR {
        return Err("cursor 超出 JavaScript 安全整数范围");
    }
    Ok(value)
}

/// 解析 `Last-Event-ID` 等文本 cursor。
pub fn parse_event_cursor(value: &str) -> Result<i64, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("cursor 不能为空".to_owned());
    }
    let cursor = value
        .parse::<i64>()
        .map_err(|_| "cursor 必须是十进制整数".to_owned())?;
    validate_event_cursor(cursor).map_err(str::to_owned)
}

/// 返回 exact metadata，而不是依赖开放的 `starts_with("task.")` 规则。
pub fn task_scoped_event_kind(kind: &str) -> bool {
    TASK_SCOPED_EVENT_KINDS.contains(&kind)
}

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
    #[serde(default, deserialize_with = "deserialize_cursor")]
    #[cfg_attr(
        feature = "schema",
        schemars(range(min = 0, max = 9_007_199_254_740_991u64))
    )]
    pub after: i64,
    #[serde(
        default = "default_limit",
        deserialize_with = "deserialize_positive_limit"
    )]
    #[cfg_attr(feature = "schema", schemars(range(min = 1)))]
    pub limit: usize,
}

fn deserialize_cursor<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let cursor = i64::deserialize(deserializer)?;
    validate_event_cursor(cursor).map_err(serde::de::Error::custom)
}

fn deserialize_positive_limit<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let limit = usize::deserialize(deserializer)?;
    if limit == 0 {
        return Err(serde::de::Error::custom("limit 必须至少为 1"));
    }
    Ok(limit)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_query_rejects_zero_limit_and_unsafe_cursor() {
        assert!(
            serde_json::from_value::<StreamEventsQuery>(serde_json::json!({
                "limit": 0
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<StreamEventsQuery>(serde_json::json!({
                "after": -1
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<StreamEventsQuery>(serde_json::json!({
                "after": 9_007_199_254_740_992_i64
            }))
            .is_err()
        );
    }

    #[test]
    fn transport_heartbeat_is_not_a_business_envelope() {
        let heartbeat = SseHeartbeatData::default();
        assert_eq!(
            serde_json::to_value(heartbeat).unwrap(),
            serde_json::json!({})
        );
        assert_eq!(SSE_HEARTBEAT_EVENT, "kb-heartbeat");
        assert!(
            serde_json::to_value(SseHeartbeatData::default())
                .unwrap()
                .get("id")
                .is_none()
        );
    }

    #[test]
    fn task_scope_metadata_is_exact_and_canonical_fields_are_stable() {
        assert!(task_scoped_event_kind("task.heartbeat"));
        assert!(task_scoped_event_kind("dependency.added"));
        assert!(!task_scoped_event_kind("task.attachment.created"));
        assert_eq!(
            STREAM_EVENT_ENVELOPE_FIELDS,
            [
                "id",
                "event_id",
                "board_id",
                "task_id",
                "run_id",
                "kind",
                "actor",
                "payload",
                "created_at",
            ]
        );
        assert!(
            TASK_SCOPED_EVENT_KINDS
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        assert!(
            TASK_SCOPED_EVENT_KINDS
                .iter()
                .all(|kind| crate::event_payload::KNOWN_EVENT_KINDS.contains(kind))
        );
    }
}

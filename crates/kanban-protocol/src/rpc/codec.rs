//! 无损自然 JSON、必填 presence 和稳定业务错误。

use prost::Message;

use super::v1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcCodecError {
    message: String,
}

impl RpcCodecError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RpcCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RpcCodecError {}

impl From<RpcCodecError> for tonic::Status {
    fn from(error: RpcCodecError) -> Self {
        encode_status(crate::ErrorBody {
            code: crate::ApiErrorCode::InvalidInput,
            message: error.to_string(),
        })
    }
}

pub(super) fn required<T>(value: Option<T>, field: &str) -> Result<T, RpcCodecError> {
    value.ok_or_else(|| RpcCodecError::invalid(format!("缺少必填字段 {field}")))
}

pub(super) trait Finite: Sized {
    fn finite(self) -> bool;
}
impl Finite for f32 {
    fn finite(self) -> bool {
        self.is_finite()
    }
}
impl Finite for f64 {
    fn finite(self) -> bool {
        self.is_finite()
    }
}

pub(super) fn finite<T: Finite + Copy>(value: T) -> Result<T, RpcCodecError> {
    if value.finite() {
        Ok(value)
    } else {
        Err(RpcCodecError::invalid("浮点值必须有限"))
    }
}

// google.rpc.Status / google.protobuf.Any 的标准 wire 布局。
// 状态 envelope 的 message name 不参与编码，Any 内的 type_url 指向公开 ErrorDetail。
#[derive(Clone, PartialEq, Message)]
struct StandardStatus {
    #[prost(int32, tag = "1")]
    code: i32,
    #[prost(string, tag = "2")]
    message: String,
    #[prost(message, repeated, tag = "3")]
    details: Vec<StandardAny>,
}

#[derive(Clone, PartialEq, Message)]
struct StandardAny {
    #[prost(string, tag = "1")]
    type_url: String,
    #[prost(bytes = "vec", tag = "2")]
    value: Vec<u8>,
}

const ERROR_DETAIL_TYPE_URL: &str = "type.googleapis.com/kanban.v1.ErrorDetail";

/// 只转换真实动态字段，整数始终保持 i64/u64 精度。
pub fn encode_json(value: serde_json::Value) -> Result<v1::JsonValue, RpcCodecError> {
    encode_json_at(value, 0)
}

fn encode_json_at(value: serde_json::Value, depth: usize) -> Result<v1::JsonValue, RpcCodecError> {
    use v1::json_value::Kind;
    if depth > 128 {
        return Err(RpcCodecError::invalid("JSON 嵌套超过 128 层"));
    }
    let kind = match value {
        serde_json::Value::Null => Kind::NullValue(v1::Empty {}),
        serde_json::Value::Bool(v) => Kind::BoolValue(v),
        serde_json::Value::String(v) => Kind::StringValue(v),
        serde_json::Value::Number(v) => {
            if v.is_f64() {
                Kind::FloatValue(finite(
                    v.as_f64()
                        .ok_or_else(|| RpcCodecError::invalid("无效 JSON float"))?,
                )?)
            } else if let Some(n) = v.as_i64() {
                Kind::SignedValue(n)
            } else if let Some(n) = v.as_u64() {
                Kind::UnsignedValue(n)
            } else {
                return Err(RpcCodecError::invalid("无效 JSON number"));
            }
        }
        serde_json::Value::Array(v) => Kind::ArrayValue(v1::JsonArray {
            items: v
                .into_iter()
                .map(|v| encode_json_at(v, depth + 1))
                .collect::<Result<_, _>>()?,
        }),
        serde_json::Value::Object(v) => Kind::ObjectValue(v1::JsonObject {
            entries: v
                .into_iter()
                .map(|(k, v)| Ok((k, encode_json_at(v, depth + 1)?)))
                .collect::<Result<_, RpcCodecError>>()?,
        }),
    };
    Ok(v1::JsonValue { kind: Some(kind) })
}

pub fn decode_json(value: v1::JsonValue) -> Result<serde_json::Value, RpcCodecError> {
    decode_json_at(value, 0)
}

fn decode_json_at(value: v1::JsonValue, depth: usize) -> Result<serde_json::Value, RpcCodecError> {
    use v1::json_value::Kind;
    if depth > 128 {
        return Err(RpcCodecError::invalid("JSON 嵌套超过 128 层"));
    }
    Ok(match required(value.kind, "JsonValue.kind")? {
        Kind::NullValue(_) => serde_json::Value::Null,
        Kind::BoolValue(v) => serde_json::Value::Bool(v),
        Kind::StringValue(v) => serde_json::Value::String(v),
        Kind::SignedValue(v) => serde_json::Value::Number(v.into()),
        Kind::UnsignedValue(v) => serde_json::Value::Number(v.into()),
        Kind::FloatValue(v) => serde_json::Value::Number(
            serde_json::Number::from_f64(v)
                .ok_or_else(|| RpcCodecError::invalid("JSON float 必须有限"))?,
        ),
        Kind::ArrayValue(v) => serde_json::Value::Array(
            v.items
                .into_iter()
                .map(|v| decode_json_at(v, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Kind::ObjectValue(v) => serde_json::Value::Object(
            v.entries
                .into_iter()
                .map(|(k, v)| Ok((k, decode_json_at(v, depth + 1)?)))
                .collect::<Result<_, RpcCodecError>>()?,
        ),
    })
}

fn status_code(code: crate::ApiErrorCode) -> tonic::Code {
    use crate::ApiErrorCode as E;
    match code {
        E::NotFound => tonic::Code::NotFound,
        E::Conflict | E::IdempotencyConflict | E::ClaimConflict => tonic::Code::Aborted,
        E::DependencyCycle | E::InvalidInput => tonic::Code::InvalidArgument,
        E::FeatureNotAvailable => tonic::Code::Unimplemented,
        E::ServerUnavailable => tonic::Code::Unavailable,
        E::ExecutionPlanRequired
        | E::StepsIncomplete
        | E::ClaimTokenMismatch
        | E::DependencyBlocked
        | E::InvalidTransition => tonic::Code::FailedPrecondition,
        E::Internal => tonic::Code::Internal,
    }
}

/// 业务 error code 始终装入正式 ErrorDetail，不依赖 message 文本反推。
pub fn encode_status(error: crate::ErrorBody) -> tonic::Status {
    let code = status_code(error.code);
    let detail = v1::ErrorDetail {
        code: v1::DtoApiErrorCode::try_from(error.code)
            .expect("封闭 error enum 映射")
            .into(),
        message: error.message.clone(),
        retryable: error.code == crate::ApiErrorCode::ServerUnavailable,
    };
    let envelope = StandardStatus {
        code: code as i32,
        message: error.message.clone(),
        details: vec![StandardAny {
            type_url: ERROR_DETAIL_TYPE_URL.into(),
            value: detail.encode_to_vec(),
        }],
    };
    tonic::Status::with_details(code, error.message, envelope.encode_to_vec().into())
}

/// 缺少或损坏的 detail 返回 None，client 可按 transport 错误处理。
pub fn decode_status(status: &tonic::Status) -> Option<crate::ErrorBody> {
    let envelope = StandardStatus::decode(status.details()).ok()?;
    if envelope.code != status.code() as i32 {
        return None;
    }
    let any = envelope
        .details
        .into_iter()
        .find(|detail| detail.type_url == ERROR_DETAIL_TYPE_URL)?;
    let detail = v1::ErrorDetail::decode(any.value.as_slice()).ok()?;
    let code = v1::DtoApiErrorCode::try_from(detail.code)
        .ok()?
        .try_into()
        .ok()?;
    if status_code(code) != status.code() {
        return None;
    }
    Some(crate::ErrorBody {
        code,
        message: detail.message,
    })
}

impl TryFrom<(crate::ApiAttachment, Vec<u8>)> for v1::DownloadAttachmentResponse {
    type Error = RpcCodecError;

    fn try_from(value: (crate::ApiAttachment, Vec<u8>)) -> Result<Self, Self::Error> {
        super::dto::AttachmentDownload::from(value).try_into()
    }
}

pub(super) fn validate_stream_event(event: &crate::StreamEventData) -> Result<(), RpcCodecError> {
    if event.id < 0 {
        return Err(RpcCodecError::invalid("event id 不能为负数"));
    }
    if event.event_id.is_empty() || event.board_id.is_empty() || event.kind.is_empty() {
        return Err(RpcCodecError::invalid("event identity 不能为空"));
    }
    if crate::task_scoped_event_kind(&event.kind)
        && event.task_id.as_ref().is_none_or(|id| id.is_empty())
    {
        return Err(RpcCodecError::invalid("task-scoped event 缺少 task_id"));
    }
    use crate::event_payload::{
        EventPayload as P, ExecutionPlanState as E, LabelProposalStatus as L, StepStatus as S,
    };
    let valid = match event.kind.as_str() {
        "board.created" => matches!(event.payload, P::BoardCreated(_) | P::Empty(_)),
        "board.archived" | "task.archived" | "task.updated" => matches!(event.payload, P::Empty(_)),
        "dependency.added" | "dependency.removed" => matches!(event.payload, P::Dependency(_)),
        "label.created" => matches!(event.payload, P::LabelCreated(_)),
        "label.deleted" => matches!(event.payload, P::LabelDeleted(_)),
        "label.ontology.action.created" => {
            matches!(event.payload, P::LabelOntologyActionCreated(_))
        }
        "label.ontology.observation.recorded" => {
            matches!(event.payload, P::LabelOntologyObservationRecorded(_))
        }
        "label.ontology.signal.reviewed" => {
            matches!(event.payload, P::LabelOntologySignalReviewed(_))
        }
        "signal.recorded" => matches!(event.payload, P::SignalRecorded(_)),
        "signal.reviewed" => matches!(event.payload, P::SignalReviewed(_)),
        "task.blocked" => matches!(
            event.payload,
            P::TaskReason(_) | P::TaskRetry(_) | P::TaskResult(_)
        ),
        "task.claimed" => matches!(event.payload, P::TaskClaimed(_)),
        "task.comment.created" => matches!(event.payload, P::TaskCommentCreated(_)),
        "task.completed" | "task.submitted_for_review" => matches!(event.payload, P::TaskResult(_)),
        "task.created" => matches!(event.payload, P::TaskStatus(_)),
        "task.promoted" | "task.recomputed" | "task.released" | "task.specified"
        | "task.unblocked" => matches!(event.payload, P::TaskToStatus(_)),
        "task.execution_plan.not_required" => {
            matches!(&event.payload,P::ExecutionPlan(p) if p.state==E::NotRequired)
        }
        "task.execution_plan.planned" => {
            matches!(&event.payload,P::ExecutionPlan(p) if p.state==E::Planned)
        }
        "task.execution_plan.unplanned" => {
            matches!(&event.payload,P::ExecutionPlan(p) if p.state==E::Unplanned)
        }
        "task.heartbeat" => matches!(event.payload, P::Heartbeat(_)),
        "task.label.added" | "task.label.removed" => matches!(event.payload, P::TaskLabel(_)),
        "task.label_proposal.accepted" => {
            matches!(&event.payload,P::LabelProposal(p) if p.status==L::Accepted)
        }
        "task.label_proposal.proposed" => {
            matches!(&event.payload,P::LabelProposal(p) if p.status==L::Proposed)
        }
        "task.label_proposal.rejected" => {
            matches!(&event.payload,P::LabelProposal(p) if p.status==L::Rejected)
        }
        "task.reclaimed" => matches!(event.payload, P::TaskReclaimed(_) | P::TaskRetry(_)),
        "task.reopened" => matches!(event.payload, P::TaskReopened(_)),
        "task.retry_policy.updated" => matches!(event.payload, P::RetryPolicy(_)),
        "task.step.created" | "task.step.reopened" => {
            matches!(&event.payload,P::TaskStep(p) if p.status==S::Todo)
        }
        "task.step.done" => matches!(&event.payload,P::TaskStep(p) if p.status==S::Done),
        "task.step.skipped" => matches!(&event.payload,P::TaskStep(p) if p.status==S::Skipped),
        "task.step.removed" | "task.step.updated" => matches!(event.payload, P::TaskStep(_)),
        "task.export_sanitized" => matches!(event.payload, P::TaskExportSanitized(_)),
        _ => matches!(event.payload, P::Unknown(_)),
    };
    if !valid {
        return Err(RpcCodecError::invalid("event kind 与 typed payload 不一致"));
    }
    Ok(())
}

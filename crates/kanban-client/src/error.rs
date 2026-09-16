use kanban_protocol::ApiErrorCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("输入无效：{0}")]
    InvalidInput(String),
    #[error("服务端 URL 无效：{0}")]
    InvalidServerUrl(String),
    #[error("服务端不可用：请检查服务端 URL，并确认已运行 `kanban serve`：{0}")]
    ServerUnavailable(String),
    #[error("{code:?}: {message}")]
    Api {
        status: u16,
        code: ApiErrorCode,
        message: String,
    },
    #[error("服务端响应无效：{0}")]
    InvalidResponse(String),
    #[error("事件 stream 已关闭")]
    StreamClosed,
    #[error("事件 stream 读取失败：{0}")]
    StreamRead(String),
}

impl ClientError {
    pub(crate) fn request_codec(error: kanban_protocol::rpc::RpcCodecError) -> Self {
        Self::InvalidInput(error.to_string())
    }

    pub(crate) fn response_codec(error: kanban_protocol::rpc::RpcCodecError) -> Self {
        Self::InvalidResponse(error.to_string())
    }

    pub(crate) fn status(status: tonic::Status) -> Self {
        if let Some(error) = kanban_protocol::rpc::decode_status(&status) {
            return Self::Api {
                status: compatible_status(error.code),
                code: error.code,
                message: error.message,
            };
        }
        match status.code() {
            tonic::Code::Unavailable | tonic::Code::DeadlineExceeded | tonic::Code::Cancelled => {
                Self::ServerUnavailable(status.to_string())
            }
            _ => Self::InvalidResponse(format!("RPC 状态缺少有效业务 detail：{status}")),
        }
    }

    pub(crate) fn unary_timeout() -> Self {
        Self::ServerUnavailable("RPC 调用超过 30 秒预算".to_owned())
    }

    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) | Self::InvalidServerUrl(_) => "invalid_input",
            Self::ServerUnavailable(_) => "server_unavailable",
            Self::Api { code, .. } => api_error_code(*code),
            Self::InvalidResponse(_) => "invalid_response",
            Self::StreamClosed => "stream_closed",
            Self::StreamRead(_) => "stream_error",
        }
    }
}

const fn compatible_status(code: ApiErrorCode) -> u16 {
    match code {
        ApiErrorCode::NotFound => 404,
        ApiErrorCode::ClaimTokenMismatch => 403,
        ApiErrorCode::Conflict
        | ApiErrorCode::IdempotencyConflict
        | ApiErrorCode::DependencyCycle
        | ApiErrorCode::ExecutionPlanRequired
        | ApiErrorCode::StepsIncomplete
        | ApiErrorCode::DependencyBlocked
        | ApiErrorCode::ClaimConflict
        | ApiErrorCode::InvalidTransition => 409,
        ApiErrorCode::InvalidInput => 400,
        ApiErrorCode::FeatureNotAvailable => 501,
        ApiErrorCode::ServerUnavailable => 503,
        ApiErrorCode::Internal => 500,
    }
}

const fn api_error_code(code: ApiErrorCode) -> &'static str {
    match code {
        ApiErrorCode::NotFound => "not_found",
        ApiErrorCode::Conflict => "conflict",
        ApiErrorCode::IdempotencyConflict => "idempotency_conflict",
        ApiErrorCode::DependencyCycle => "dependency_cycle",
        ApiErrorCode::InvalidInput => "invalid_input",
        ApiErrorCode::FeatureNotAvailable => "feature_not_available",
        ApiErrorCode::ServerUnavailable => "server_unavailable",
        ApiErrorCode::ExecutionPlanRequired => "execution_plan_required",
        ApiErrorCode::StepsIncomplete => "steps_incomplete",
        ApiErrorCode::ClaimTokenMismatch => "claim_token_mismatch",
        ApiErrorCode::DependencyBlocked => "dependency_blocked",
        ApiErrorCode::ClaimConflict => "claim_conflict",
        ApiErrorCode::InvalidTransition => "invalid_transition",
        ApiErrorCode::Internal => "internal",
    }
}

#[cfg(test)]
mod tests {
    use super::ClientError;

    #[test]
    fn server_unavailable_display_includes_actionable_host_hint() {
        let error =
            ClientError::ServerUnavailable("http://127.0.0.1:8721: connection refused".to_owned());
        let message = error.to_string();

        assert_eq!(error.code(), "server_unavailable");
        assert!(message.contains("服务端不可用"));
        assert!(message.contains("服务端 URL"));
        assert!(message.contains("kanban serve"));
        assert!(message.contains("http://127.0.0.1:8721"));
    }

    #[test]
    fn standard_rpc_details_keep_all_business_codes_and_compatible_statuses() {
        use kanban_protocol::{ApiErrorCode as E, ErrorBody, rpc::encode_status};
        for (code, status) in [
            (E::NotFound, 404),
            (E::Conflict, 409),
            (E::IdempotencyConflict, 409),
            (E::DependencyCycle, 409),
            (E::InvalidInput, 400),
            (E::FeatureNotAvailable, 501),
            (E::ServerUnavailable, 503),
            (E::ExecutionPlanRequired, 409),
            (E::StepsIncomplete, 409),
            (E::ClaimTokenMismatch, 403),
            (E::DependencyBlocked, 409),
            (E::ClaimConflict, 409),
            (E::InvalidTransition, 409),
            (E::Internal, 500),
        ] {
            let error = ClientError::status(encode_status(ErrorBody {
                code,
                message: "稳定业务错误".to_owned(),
            }));
            match error {
                ClientError::Api {
                    status: actual_status,
                    code: actual_code,
                    message,
                } => {
                    assert_eq!(actual_status, status);
                    assert_eq!(actual_code, code);
                    assert_eq!(message, "稳定业务错误");
                }
                error => panic!("未保留业务错误：{error:?}"),
            }
        }
        assert!(matches!(
            ClientError::status(tonic::Status::not_found("缺少 detail")),
            ClientError::InvalidResponse(_)
        ));
    }
}

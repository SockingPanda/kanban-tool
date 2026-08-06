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
}

impl ClientError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) | Self::InvalidServerUrl(_) => "invalid_input",
            Self::ServerUnavailable(_) => "server_unavailable",
            Self::Api { code, .. } => api_error_code(*code),
            Self::InvalidResponse(_) => "invalid_response",
        }
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
        let error = ClientError::ServerUnavailable(
            "http://127.0.0.1:8721/api/v1/boards/default: connection refused".to_owned(),
        );
        let message = error.to_string();

        assert_eq!(error.code(), "server_unavailable");
        assert!(message.contains("服务端不可用"));
        assert!(message.contains("服务端 URL"));
        assert!(message.contains("kanban serve"));
        assert!(message.contains("http://127.0.0.1:8721"));
    }
}

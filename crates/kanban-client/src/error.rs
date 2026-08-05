use kanban_contract::ApiErrorCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("invalid server URL: {0}")]
    InvalidServerUrl(String),
    #[error("server unavailable: {0}")]
    ServerUnavailable(String),
    #[error("{code:?}: {message}")]
    Api {
        status: u16,
        code: ApiErrorCode,
        message: String,
    },
    #[error("invalid server response: {0}")]
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

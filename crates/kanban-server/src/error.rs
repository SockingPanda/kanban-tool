use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use kanban_contract::{ApiErrorCode, ErrorBody, ErrorEnvelope};
use kanban_core::KanbanError;

#[derive(Debug)]
pub(crate) struct ApiError(pub(crate) KanbanError);

impl From<KanbanError> for ApiError {
    fn from(error: KanbanError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            KanbanError::NotFound(_) => (StatusCode::NOT_FOUND, ApiErrorCode::NotFound),
            KanbanError::Conflict(_) => (StatusCode::CONFLICT, ApiErrorCode::Conflict),
            KanbanError::IdempotencyConflict(_) => {
                (StatusCode::CONFLICT, ApiErrorCode::IdempotencyConflict)
            }
            KanbanError::FeatureNotAvailable(_) => (
                StatusCode::NOT_IMPLEMENTED,
                ApiErrorCode::FeatureNotAvailable,
            ),
            KanbanError::InvalidInput(_) | KanbanError::InvalidStatus(_) => {
                (StatusCode::BAD_REQUEST, ApiErrorCode::InvalidInput)
            }
            KanbanError::ExecutionPlanRequired(_) => {
                (StatusCode::CONFLICT, ApiErrorCode::ExecutionPlanRequired)
            }
            KanbanError::StepsIncomplete(_) => {
                (StatusCode::CONFLICT, ApiErrorCode::StepsIncomplete)
            }
            KanbanError::InvalidTransition(message)
                if message.contains("claim token mismatch")
                    || message.contains("claim owner mismatch") =>
            {
                (StatusCode::FORBIDDEN, ApiErrorCode::ClaimTokenMismatch)
            }
            KanbanError::InvalidTransition(message) if message.contains("dependency blocked") => {
                (StatusCode::CONFLICT, ApiErrorCode::DependencyBlocked)
            }
            KanbanError::InvalidTransition(message) if message.contains("claim conflict") => {
                (StatusCode::CONFLICT, ApiErrorCode::ClaimConflict)
            }
            KanbanError::InvalidTransition(_) => {
                (StatusCode::CONFLICT, ApiErrorCode::InvalidTransition)
            }
            KanbanError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, ApiErrorCode::Internal),
        };
        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody {
                    code,
                    message: self.0.to_string(),
                },
            }),
        )
            .into_response()
    }
}

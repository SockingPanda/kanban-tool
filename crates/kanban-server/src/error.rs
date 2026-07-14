use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use kanban_core::KanbanError;

use crate::i18n::current_request_locale;
use kanban_contract::{ApiErrorCode, ErrorBody, ErrorEnvelope};

pub(super) fn extractor_error(error: impl std::fmt::Display) -> ApiError {
    invalid_input(error.to_string())
}

pub(super) fn invalid_input(message: impl Into<String>) -> ApiError {
    ApiError(KanbanError::InvalidInput(message.into()))
}

pub(super) fn validate_page_bounds(
    limit: usize,
    max_limit: usize,
    offset: usize,
) -> Result<(), ApiError> {
    if limit > max_limit {
        return Err(invalid_input(format!("limit must be <= {max_limit}")));
    }
    if offset > i64::MAX as usize {
        return Err(invalid_input(format!("offset must be <= {}", i64::MAX)));
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct ApiError(pub(super) KanbanError);

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl From<KanbanError> for ApiError {
    fn from(value: KanbanError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = self.0.to_string();
        let (status, code) = match &self.0 {
            KanbanError::NotFound(_) => (StatusCode::NOT_FOUND, ApiErrorCode::NotFound),
            KanbanError::Conflict(_) => (StatusCode::CONFLICT, ApiErrorCode::Conflict),
            KanbanError::InvalidInput(_) if message.contains("dependency cycle") => {
                (StatusCode::CONFLICT, ApiErrorCode::DependencyCycle)
            }
            KanbanError::InvalidInput(_) | KanbanError::InvalidStatus(_) => {
                (StatusCode::BAD_REQUEST, ApiErrorCode::InvalidInput)
            }
            KanbanError::ExecutionPlanRequired(_) => {
                (StatusCode::CONFLICT, ApiErrorCode::ExecutionPlanRequired)
            }
            KanbanError::StepsIncomplete(_) => {
                (StatusCode::CONFLICT, ApiErrorCode::StepsIncomplete)
            }
            KanbanError::InvalidTransition(_) if message.contains("claim token mismatch") => {
                (StatusCode::FORBIDDEN, ApiErrorCode::ClaimTokenMismatch)
            }
            KanbanError::InvalidTransition(_) if message.contains("dependency blocked") => {
                (StatusCode::CONFLICT, ApiErrorCode::DependencyBlocked)
            }
            KanbanError::InvalidTransition(_) if message.contains("claim conflict") => {
                (StatusCode::CONFLICT, ApiErrorCode::ClaimConflict)
            }
            KanbanError::InvalidTransition(_) => {
                (StatusCode::CONFLICT, ApiErrorCode::InvalidTransition)
            }
            KanbanError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, ApiErrorCode::Internal),
        };
        let message = kanban_core::i18n::render_error(current_request_locale(), &self.0);
        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use kanban_contract::{ApiErrorCode, ErrorEnvelope};

    use super::{ApiError, IntoResponse, KanbanError, StatusCode};

    #[tokio::test]
    async fn api_error_maps_each_stable_code_to_its_status() {
        let cases = [
            (
                KanbanError::NotFound("missing".into()),
                StatusCode::NOT_FOUND,
                ApiErrorCode::NotFound,
            ),
            (
                KanbanError::Conflict("conflict".into()),
                StatusCode::CONFLICT,
                ApiErrorCode::Conflict,
            ),
            (
                KanbanError::InvalidInput("dependency cycle".into()),
                StatusCode::CONFLICT,
                ApiErrorCode::DependencyCycle,
            ),
            (
                KanbanError::InvalidInput("bad input".into()),
                StatusCode::BAD_REQUEST,
                ApiErrorCode::InvalidInput,
            ),
            (
                KanbanError::ExecutionPlanRequired("plan".into()),
                StatusCode::CONFLICT,
                ApiErrorCode::ExecutionPlanRequired,
            ),
            (
                KanbanError::StepsIncomplete("steps".into()),
                StatusCode::CONFLICT,
                ApiErrorCode::StepsIncomplete,
            ),
            (
                KanbanError::InvalidTransition("claim token mismatch".into()),
                StatusCode::FORBIDDEN,
                ApiErrorCode::ClaimTokenMismatch,
            ),
            (
                KanbanError::InvalidTransition("dependency blocked".into()),
                StatusCode::CONFLICT,
                ApiErrorCode::DependencyBlocked,
            ),
            (
                KanbanError::InvalidTransition("claim conflict".into()),
                StatusCode::CONFLICT,
                ApiErrorCode::ClaimConflict,
            ),
            (
                KanbanError::InvalidTransition("task is not claimable".into()),
                StatusCode::CONFLICT,
                ApiErrorCode::InvalidTransition,
            ),
            (
                KanbanError::InvalidTransition("other".into()),
                StatusCode::CONFLICT,
                ApiErrorCode::InvalidTransition,
            ),
            (
                KanbanError::Storage("storage".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorCode::Internal,
            ),
        ];
        for (error, status, code) in cases {
            let response = ApiError(error).into_response();
            assert_eq!(response.status(), status);
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let envelope: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
            assert_eq!(envelope.error.code, code);
        }
    }
}

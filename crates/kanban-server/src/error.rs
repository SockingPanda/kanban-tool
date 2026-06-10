use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use kanban_core::KanbanError;

use crate::dto::{ErrorBody, ErrorEnvelope};

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

impl From<KanbanError> for ApiError {
    fn from(value: KanbanError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = self.0.to_string();
        let (status, code) = match self.0 {
            KanbanError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            KanbanError::InvalidInput(_) if message.contains("dependency cycle") => {
                (StatusCode::CONFLICT, "dependency_cycle")
            }
            KanbanError::InvalidInput(_) | KanbanError::InvalidStatus(_) => {
                (StatusCode::BAD_REQUEST, "invalid_input")
            }
            KanbanError::InvalidTransition(_) if message.contains("claim token mismatch") => {
                (StatusCode::FORBIDDEN, "claim_token_mismatch")
            }
            KanbanError::InvalidTransition(_) if message.contains("dependency blocked") => {
                (StatusCode::CONFLICT, "dependency_blocked")
            }
            KanbanError::InvalidTransition(_) if message.contains("claim conflict") => {
                (StatusCode::CONFLICT, "claim_conflict")
            }
            KanbanError::InvalidTransition(_) => (StatusCode::CONFLICT, "invalid_transition"),
            KanbanError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}

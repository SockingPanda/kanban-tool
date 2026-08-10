use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use kanban_protocol::{ApiErrorCode, ErrorBody, ErrorEnvelope};
use kanban_service::KanbanError;

const INTERNAL_ERROR_MESSAGE: &str = "服务端内部错误，请稍后重试。";

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
            KanbanError::IdempotencyConflict(_) => {
                (StatusCode::CONFLICT, ApiErrorCode::IdempotencyConflict)
            }
            KanbanError::FeatureNotAvailable(_) => (
                StatusCode::NOT_IMPLEMENTED,
                ApiErrorCode::FeatureNotAvailable,
            ),
            KanbanError::Conflict(message) if message.contains("dependency cycle") => {
                (StatusCode::CONFLICT, ApiErrorCode::DependencyCycle)
            }
            KanbanError::Conflict(_) => (StatusCode::CONFLICT, ApiErrorCode::Conflict),
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
        let message = if code == ApiErrorCode::Internal {
            INTERNAL_ERROR_MESSAGE.to_owned()
        } else {
            self.0.to_string()
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

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, response::IntoResponse};
    use http_body_util::BodyExt;
    use kanban_protocol::{ApiErrorCode, ErrorEnvelope};
    use kanban_service::KanbanError;

    use super::ApiError;

    async fn response_error(error: KanbanError) -> (StatusCode, ErrorEnvelope, String) {
        let response = ApiError(error).into_response();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_text = String::from_utf8(body.to_vec()).unwrap();
        let envelope = serde_json::from_str(&body_text).unwrap();
        (status, envelope, body_text)
    }

    #[tokio::test]
    async fn storage_errors_are_internal_without_run_log_path_details() {
        let secret_path = "/srv/kanban/run-logs/run-123.log";
        let (status, envelope, body) = response_error(KanbanError::Storage(format!(
            "failed to read {secret_path}: permission denied"
        )))
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(envelope.error.code, ApiErrorCode::Internal);
        assert!(!body.contains(secret_path));
        assert!(!envelope.error.message.contains("permission denied"));
    }

    #[tokio::test]
    async fn storage_errors_are_internal_without_attachment_path_details() {
        let secret_path = "/var/lib/kanban/attachments/task-123/file.bin";
        let (status, envelope, body) = response_error(KanbanError::Storage(format!(
            "failed to open {secret_path}: no such file"
        )))
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(envelope.error.code, ApiErrorCode::Internal);
        assert!(!body.contains(secret_path));
        assert!(!envelope.error.message.contains("no such file"));
    }

    #[tokio::test]
    async fn client_actionable_errors_keep_their_messages() {
        let (_, invalid_input, _) =
            response_error(KanbanError::InvalidInput("title is required".to_owned())).await;
        assert_eq!(invalid_input.error.code, ApiErrorCode::InvalidInput);
        assert_eq!(
            invalid_input.error.message,
            "invalid input: title is required"
        );

        let (_, not_found, _) =
            response_error(KanbanError::NotFound("task t_123".to_owned())).await;
        assert_eq!(not_found.error.code, ApiErrorCode::NotFound);
        assert_eq!(not_found.error.message, "not found: task t_123");
    }
}

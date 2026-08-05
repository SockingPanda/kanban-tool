use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use kanban_service::{
    CommentAuthorType as ApplicationCommentAuthorType, CommentKind as ApplicationCommentKind,
    CreateCommentCommand,
};
use kanban_core::KanbanError;
use kanban_protocol::{CreateCommentPath, CreateCommentRequest, CreateCommentResponse};

pub(crate) async fn create_comment(
    State(state): State<AppState>,
    Path(CreateCommentPath { task_id }): Path<CreateCommentPath>,
    headers: HeaderMap,
    body: Result<Json<CreateCommentRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateCommentResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.author.as_deref(), &headers, state.default_actor())?;
    let metadata = body.metadata.unwrap_or_else(|| serde_json::json!({}));
    let metadata = metadata.as_object().cloned().ok_or_else(|| {
        ApiError(KanbanError::InvalidInput(
            "metadata must be a JSON object".to_owned(),
        ))
    })?;
    let comment = state
        .application()
        .create_comment(CreateCommentCommand {
            task_id,
            idempotency_key: body.idempotency_key,
            author: actor,
            author_type: body
                .author_type
                .map(|value| match value {
                    kanban_protocol::CommentAuthorType::User => ApplicationCommentAuthorType::User,
                    kanban_protocol::CommentAuthorType::Agent => {
                        ApplicationCommentAuthorType::Agent
                    }
                })
                .unwrap_or(ApplicationCommentAuthorType::User),
            agent_type: body.agent_type,
            body: body.body,
            kind: body
                .kind
                .map(|value| match value {
                    kanban_protocol::CommentKind::Note => ApplicationCommentKind::Note,
                    kanban_protocol::CommentKind::Decision => ApplicationCommentKind::Decision,
                    kanban_protocol::CommentKind::Signal => ApplicationCommentKind::Signal,
                })
                .unwrap_or(ApplicationCommentKind::Note),
            metadata: metadata.into_iter().collect(),
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateCommentResponse {
            data: api_comment(comment)?,
        }),
    ))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id/comments", post(create_comment))
}
#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn comment_create_uses_application_path_and_entity_local_idempotency() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let task = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_http_comment",
                    "idempotency_key": "http-comment-task",
                    "title": "HTTP comment",
                    "description": "comment test",
                    "priority": 1,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "seed"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(task.status(), StatusCode::CREATED);

        let request = serde_json::json!({
            "idempotency_key": "comment-retry",
            "author": "alice",
            "body": "handoff",
            "kind": "note",
            "author_type": "user",
            "metadata": {"source": "router-test"}
        });
        let first = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_comment/comments",
                request.clone(),
            ))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        let first_body = first.into_body().collect().await.unwrap().to_bytes();
        let first: kanban_protocol::CreateCommentResponse =
            serde_json::from_slice(&first_body).unwrap();

        let replay = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_comment/comments",
                request,
            ))
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::CREATED);
        let replay_body = replay.into_body().collect().await.unwrap().to_bytes();
        let replay: kanban_protocol::CreateCommentResponse =
            serde_json::from_slice(&replay_body).unwrap();
        assert_eq!(replay.data.id, first.data.id);

        let listed = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/tasks/t_http_comment/comments")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(listed.status(), StatusCode::OK);
        let listed_body = listed.into_body().collect().await.unwrap().to_bytes();
        let listed: kanban_protocol::ListCommentsResponse =
            serde_json::from_slice(&listed_body).unwrap();
        assert_eq!(listed.data.len(), 1);
        assert_eq!(listed.data[0].id, first.data.id);

        let missing = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/tasks/t_http_missing/comments")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        let conflict = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_comment/comments",
                serde_json::json!({
                    "idempotency_key": "comment-retry",
                    "author": "alice",
                    "body": "changed",
                    "kind": "note",
                    "author_type": "user",
                    "metadata": {"source": "router-test"}
                }),
            ))
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        let conflict_body = conflict.into_body().collect().await.unwrap().to_bytes();
        let error: kanban_protocol::ErrorEnvelope = serde_json::from_slice(&conflict_body).unwrap();
        assert_eq!(
            error.error.code,
            kanban_protocol::ApiErrorCode::IdempotencyConflict
        );

        let signal = router
            .oneshot(json_request(
                "/api/v1/tasks/t_http_comment/comments",
                serde_json::json!({
                    "idempotency_key": "comment-signal",
                    "author": "alice",
                    "body": "signal",
                    "kind": "signal",
                    "metadata": {}
                }),
            ))
            .await
            .unwrap();
        assert_eq!(signal.status(), StatusCode::NOT_IMPLEMENTED);
        let signal_body = signal.into_body().collect().await.unwrap().to_bytes();
        let error: kanban_protocol::ErrorEnvelope = serde_json::from_slice(&signal_body).unwrap();
        assert_eq!(
            error.error.code,
            kanban_protocol::ApiErrorCode::FeatureNotAvailable
        );
    }
}

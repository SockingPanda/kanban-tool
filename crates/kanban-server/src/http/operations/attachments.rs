use super::support::request_actor;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
    routing::get,
};
use kanban_protocol::{
    ApiAttachment, CreateAttachmentPath, CreateAttachmentRequest, CreateAttachmentResponse,
    DeleteAttachmentPath, DeleteAttachmentResponse, GetAttachmentPath, ListAttachmentsPath,
    ListAttachmentsResponse,
};
use kanban_service::KanbanError;
use kanban_service::{CreateAttachmentCommand, DeleteAttachmentCommand};

pub(crate) async fn list_attachments(
    State(state): State<AppState>,
    Path(ListAttachmentsPath { task_id }): Path<ListAttachmentsPath>,
) -> Result<Json<ListAttachmentsResponse>, ApiError> {
    let data = state
        .application()
        .list_attachments(&task_id)
        .await?
        .into_iter()
        .map(api_attachment)
        .collect();
    Ok(Json(ListAttachmentsResponse { data }))
}

pub(crate) async fn create_attachment(
    State(state): State<AppState>,
    Path(CreateAttachmentPath { task_id }): Path<CreateAttachmentPath>,
    headers: HeaderMap,
    body: Result<Json<CreateAttachmentRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateAttachmentResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let attachment = state
        .application()
        .create_attachment(CreateAttachmentCommand {
            task_id,
            id: body.id,
            filename: body.filename,
            rel_path: body.rel_path,
            content_type: body.content_type,
            content: body.content,
            sha256: body.sha256,
            created_by: actor,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateAttachmentResponse {
            data: api_attachment(attachment),
        }),
    ))
}

pub(crate) async fn download_attachment(
    State(state): State<AppState>,
    Path(GetAttachmentPath {
        task_id,
        attachment_id,
    }): Path<GetAttachmentPath>,
) -> Result<Response, ApiError> {
    let content = state
        .application()
        .read_attachment(&task_id, &attachment_id)
        .await?;
    let mut response = Response::new(Body::from(content.content));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        content
            .attachment
            .content_type
            .as_deref()
            .and_then(|value| HeaderValue::from_str(value).ok())
            .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream")),
    );
    if let Ok(value) = HeaderValue::from_str(&content.attachment.size_bytes.to_string()) {
        headers.insert(header::CONTENT_LENGTH, value);
    }
    if let Ok(value) = HeaderValue::from_str(&content.attachment.id) {
        headers.insert("x-kb-attachment-id", value);
    }
    if let Some(sha256) = content.attachment.sha256.as_deref()
        && let Ok(value) = HeaderValue::from_str(sha256)
    {
        headers.insert("x-kb-attachment-sha256", value);
    }
    Ok(response)
}

pub(crate) async fn delete_attachment(
    State(state): State<AppState>,
    Path(DeleteAttachmentPath {
        task_id,
        attachment_id,
    }): Path<DeleteAttachmentPath>,
    headers: HeaderMap,
) -> Result<Json<DeleteAttachmentResponse>, ApiError> {
    let actor = request_actor(None, &headers, state.default_actor())?;
    let deleted = state
        .application()
        .delete_attachment(DeleteAttachmentCommand {
            task_id,
            attachment_id,
            actor,
        })
        .await?;
    Ok(Json(DeleteAttachmentResponse {
        data: kanban_protocol::DeleteResult { deleted },
    }))
}

fn api_attachment(attachment: kanban_service::AttachmentRecord) -> ApiAttachment {
    ApiAttachment {
        id: attachment.id,
        board_id: attachment.board_id,
        task_id: attachment.task_id,
        filename: attachment.filename,
        rel_path: attachment.rel_path,
        content_type: attachment.content_type,
        size_bytes: attachment.size_bytes,
        sha256: attachment.sha256,
        created_by: attachment.created_by,
        created_at: attachment.created_at,
    }
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/tasks/:task_id/attachments",
            get(list_attachments).post(create_attachment),
        )
        .route(
            "/api/v1/tasks/:task_id/attachments/:attachment_id",
            get(download_attachment).delete(delete_attachment),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::operations::test_support::*;

    async fn create_task(router: &axum::Router, id: &str) {
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": id,
                    "idempotency_key": format!("attachment-task-{id}"),
                    "title": "attachment task",
                    "description": "attachment test",
                    "priority": 1,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "seed"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn attachment_round_trip_is_metadata_typed_and_file_backed() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        create_task(&router, "t_attachment_http").await;

        let create = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_attachment_http/attachments",
                serde_json::json!({
                    "id": "a_http_attachment",
                    "filename": "hello.txt",
                    "content": [104, 101, 108, 108, 111],
                    "content_type": "text/plain",
                    "actor": "alice"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::CREATED);
        let body = create.into_body().collect().await.unwrap().to_bytes();
        let created: CreateAttachmentResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(created.data.size_bytes, 5);
        assert!(
            created
                .data
                .rel_path
                .starts_with("b_default/t_attachment_http/")
        );
        assert!(
            state
                .attachment_root()
                .join(&created.data.rel_path)
                .is_file()
        );

        let replay = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_attachment_http/attachments",
                serde_json::json!({
                    "id": "a_http_attachment",
                    "filename": "hello.txt",
                    "content": [104, 101, 108, 108, 111],
                    "content_type": "text/plain",
                    "actor": "alice"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::CREATED);
        let body = replay.into_body().collect().await.unwrap().to_bytes();
        let replayed: CreateAttachmentResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(replayed.data, created.data);

        let conflict = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_attachment_http/attachments",
                serde_json::json!({
                    "id": "a_http_attachment",
                    "filename": "hello.txt",
                    "content": [119, 111, 114, 108, 100],
                    "actor": "alice"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);

        let checksum = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_attachment_http/attachments",
                serde_json::json!({
                    "id": "a_bad_checksum",
                    "filename": "bad.txt",
                    "content": [1, 2, 3],
                    "sha256": "00"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(checksum.status(), StatusCode::BAD_REQUEST);

        let listed = router
            .clone()
            .oneshot(
                Request::get("/api/v1/tasks/t_attachment_http/attachments")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(listed.status(), StatusCode::OK);
        let body = listed.into_body().collect().await.unwrap().to_bytes();
        let listed: ListAttachmentsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(listed.data, vec![created.data.clone()]);

        let download = router
            .clone()
            .oneshot(
                Request::get("/api/v1/tasks/t_attachment_http/attachments/a_http_attachment")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(download.status(), StatusCode::OK);
        assert_eq!(download.headers()[header::CONTENT_TYPE], "text/plain");
        assert_eq!(
            download.into_body().collect().await.unwrap().to_bytes(),
            "hello"
        );

        let removed = router
            .clone()
            .oneshot(
                Request::delete("/api/v1/tasks/t_attachment_http/attachments/a_http_attachment")
                    .header("X-KB-Actor", "alice")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(removed.status(), StatusCode::OK);
        let body = removed.into_body().collect().await.unwrap().to_bytes();
        let removed: DeleteAttachmentResponse = serde_json::from_slice(&body).unwrap();
        assert!(removed.data.deleted);
        assert!(
            !state
                .attachment_root()
                .join(&created.data.rel_path)
                .exists()
        );
        assert!(
            std::fs::read_dir(state.attachment_root().join(".trash"))
                .unwrap()
                .next()
                .is_some()
        );
    }

    #[tokio::test]
    async fn attachment_rejects_traversal_and_cross_task_access() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        create_task(&router, "t_attachment_guard").await;
        let escaped = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_attachment_guard/attachments",
                serde_json::json!({
                    "id": "a_escape",
                    "filename": "safe.txt",
                    "rel_path": "../escape.txt",
                    "content": [1]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(escaped.status(), StatusCode::BAD_REQUEST);

        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(state.attachment_root().join("b_default/t_attachment_guard"))
                .unwrap();
            std::os::unix::fs::symlink(
                outside.path(),
                state
                    .attachment_root()
                    .join("b_default/t_attachment_guard/linked"),
            )
            .unwrap();
            let symlink_escape = router
                .clone()
                .oneshot(json_request(
                    "/api/v1/tasks/t_attachment_guard/attachments",
                    serde_json::json!({
                        "id": "a_symlink_escape",
                        "filename": "safe.txt",
                        "rel_path": "b_default/t_attachment_guard/linked/safe.txt",
                        "content": [1]
                    }),
                ))
                .await
                .unwrap();
            assert_eq!(symlink_escape.status(), StatusCode::BAD_REQUEST);
        }

        create_task(&router, "t_attachment_other").await;
        let source = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_attachment_guard/attachments",
                serde_json::json!({
                    "id": "a_cross_task",
                    "filename": "safe.txt",
                    "content": [1]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(source.status(), StatusCode::CREATED);
        let cross_task = router
            .clone()
            .oneshot(
                Request::get("/api/v1/tasks/t_attachment_other/attachments/a_cross_task")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cross_task.status(), StatusCode::NOT_FOUND);

        let missing_body = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_attachment_guard/attachments",
                serde_json::json!({
                    "id": "a_missing_file",
                    "filename": "missing.txt",
                    "content": [1]
                }),
            ))
            .await
            .unwrap();
        let body = missing_body.into_body().collect().await.unwrap().to_bytes();
        let missing: CreateAttachmentResponse = serde_json::from_slice(&body).unwrap();
        std::fs::remove_file(state.attachment_root().join(&missing.data.rel_path)).unwrap();
        let missing_download = router
            .clone()
            .oneshot(
                Request::get("/api/v1/tasks/t_attachment_guard/attachments/a_missing_file")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_download.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let missing = router
            .oneshot(
                Request::get("/api/v1/tasks/t_attachment_guard/attachments/a_other")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }
}
